//! Native, local video assembly workspace. The timeline holds references to source files;
//! FFmpeg only writes a new file after the owner chooses an output and presses Export.

use crate::video_pipeline::{
    self, AudioTrack, Clip, ExportJob, ExportPhase, ExportRequest, ExportStatus, Probe, Quality,
};
use egui::{
    Color32, ColorImage, Pos2, Rect, Sense, Stroke, TextureHandle, TextureOptions, pos2, vec2,
};
use egui_shadcn::{Button, ButtonRadius, ButtonSize, ButtonStyle, ButtonVariant};
use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::mpsc::{self, Receiver, Sender},
    time::Duration,
};

const MAX_CLIPS: usize = 64;
const MIN_TRIM: f64 = 0.25;

struct UiClip {
    id: u64,
    path: PathBuf,
    probe: Option<Probe>,
    start: f64,
    end: f64,
    thumbnail: Option<TextureHandle>,
    error: Option<String>,
}

impl UiClip {
    fn length(&self) -> f64 {
        (self.end - self.start).max(0.0)
    }
    fn filename(&self) -> String {
        self.path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Video")
            .to_owned()
    }
    fn ready(&self) -> bool {
        self.probe.is_some() && self.error.is_none() && self.length() >= MIN_TRIM
    }
}

struct UiAudio {
    path: PathBuf,
    duration: Option<f64>,
    start: f64,
    gain: f32,
    error: Option<String>,
}

enum WorkerMessage {
    Clip {
        id: u64,
        result: Result<(Probe, Option<Vec<u8>>), String>,
    },
    Audio {
        path: PathBuf,
        result: Result<Probe, String>,
    },
    Preview {
        key: (u64, u32),
        result: Result<Vec<u8>, String>,
    },
}

#[derive(Clone, Copy)]
enum PickerKind {
    Clips,
    Audio,
    Output,
}

pub(crate) struct VideoEditor {
    clips: Vec<UiClip>,
    audio: Option<UiAudio>,
    selected: Option<u64>,
    next_id: u64,
    playhead: f64,
    preview_source_time: f64,
    preview_key: Option<(u64, u32)>,
    preview_pending: bool,
    preview: Option<TextureHandle>,
    timeline_scale: f32,
    timeline_auto_fit: bool,
    output: Option<PathBuf>,
    quality: Quality,
    mute_original: bool,
    inspector_open: bool,
    exported_path: Option<PathBuf>,
    job: Option<ExportJob>,
    export_status: Option<ExportStatus>,
    picker: Option<(PickerKind, Receiver<Vec<PathBuf>>)>,
    worker_tx: Sender<WorkerMessage>,
    worker_rx: Receiver<WorkerMessage>,
    notice: Option<String>,
    dirty: bool,
}

impl Default for VideoEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoEditor {
    pub(crate) fn new() -> Self {
        let (worker_tx, worker_rx) = mpsc::channel();
        Self {
            clips: Vec::new(),
            audio: None,
            selected: None,
            next_id: 1,
            playhead: 0.0,
            preview_source_time: 0.0,
            preview_key: None,
            preview_pending: false,
            preview: None,
            timeline_scale: 18.0,
            timeline_auto_fit: true,
            output: None,
            quality: Quality::Balanced,
            mute_original: false,
            inspector_open: false,
            exported_path: None,
            job: None,
            export_status: None,
            picker: None,
            worker_tx,
            worker_rx,
            notice: None,
            dirty: false,
        }
    }

    pub(crate) fn has_unsaved_work(&self) -> bool {
        self.dirty || self.job.is_some()
    }
    pub(crate) fn has_export_running(&self) -> bool {
        self.job.is_some()
    }
    pub(crate) fn tick(&mut self, ctx: &egui::Context) {
        self.poll_background(ctx);
    }
    pub(crate) fn take_exported_path(&mut self) -> Option<PathBuf> {
        self.exported_path.take()
    }

    pub(crate) fn inspect(&self) -> serde_json::Value {
        serde_json::json!({
            "kind":"videoEditor",
            "clipCount":self.clips.len(),
            "clips":self.clips.iter().map(|clip| serde_json::json!({
                "path":clip.path.display().to_string(),"start":clip.start,"end":clip.end,"duration":clip.length(),
                "ready":clip.ready(),"error":clip.error.as_deref(),
            })).collect::<Vec<_>>(),
            "timelineSeconds":self.total_length(),
            "selectedClip":self.selected,
            "audio":self.audio.as_ref().map(|track| serde_json::json!({
                "path":track.path.display().to_string(),"start":track.start,"gain":track.gain,
                "ready":track.duration.is_some() && track.error.is_none(),"error":track.error.as_deref(),
            })),
            "muteOriginal":self.mute_original,
            "audioMode":if self.mute_original { "strip original" } else if self.audio.is_some() { "mix" } else { "original" },
            "output":self.output.as_ref(),
            "quality":self.quality.label(),
            "exporting":self.job.is_some(),
            "phase":self.export_status.as_ref().map(|status| status.phase.label()),
            "progress":self.export_status.as_ref().map(|status| status.fraction),
            "exportError":self.export_status.as_ref().and_then(|status| status.error.as_deref()),
        })
    }

    pub(crate) fn add_clip_path(
        &mut self,
        path: PathBuf,
        ctx: &egui::Context,
    ) -> Result<(), String> {
        if self.clips.len() >= MAX_CLIPS {
            return Err(format!("This project supports up to {MAX_CLIPS} clips."));
        }
        if !path.is_file() {
            return Err(format!("Video is unavailable: {}", path.display()));
        }
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.clips.push(UiClip {
            id,
            path: path.clone(),
            probe: None,
            start: 0.0,
            end: 0.0,
            thumbnail: None,
            error: None,
        });
        if self.selected.is_none() {
            self.selected = Some(id);
        }
        self.dirty = true;
        let sender = self.worker_tx.clone();
        let wake = ctx.clone();
        std::thread::Builder::new()
            .name("polymath-video-probe".into())
            .spawn(move || {
                let result = video_pipeline::probe(&path).and_then(|probe| {
                    if !probe.has_video {
                        return Err("This file has no video track.".into());
                    }
                    let frame =
                        video_pipeline::thumbnail(&path, (probe.duration * 0.3).min(5.0), 240).ok();
                    Ok((probe, frame))
                });
                let _ = sender.send(WorkerMessage::Clip { id, result });
                wake.request_repaint();
            })
            .map_err(|error| format!("Could not inspect video: {error}"))?;
        Ok(())
    }

    pub(crate) fn set_audio_path(
        &mut self,
        path: PathBuf,
        ctx: &egui::Context,
    ) -> Result<(), String> {
        if !path.is_file() {
            return Err(format!("Audio is unavailable: {}", path.display()));
        }
        self.audio = Some(UiAudio {
            path: path.clone(),
            duration: None,
            start: 0.0,
            gain: 0.7,
            error: None,
        });
        self.dirty = true;
        let sender = self.worker_tx.clone();
        let wake = ctx.clone();
        std::thread::Builder::new()
            .name("polymath-audio-probe".into())
            .spawn(move || {
                let result = video_pipeline::probe(&path).and_then(|probe| {
                    if !probe.has_audio {
                        return Err("This file has no audio track.".into());
                    }
                    Ok(probe)
                });
                let _ = sender.send(WorkerMessage::Audio { path, result });
                wake.request_repaint();
            })
            .map_err(|error| format!("Could not inspect audio: {error}"))?;
        Ok(())
    }

    fn total_length(&self) -> f64 {
        self.clips.iter().map(UiClip::length).sum()
    }

    pub(crate) fn set_clip_trim(
        &mut self,
        index: usize,
        start: f64,
        end: f64,
    ) -> Result<(), String> {
        let Some(clip) = self.clips.get_mut(index) else {
            return Err("Clip index is out of range.".into());
        };
        let Some(probe) = &clip.probe else {
            return Err("Clip is still loading.".into());
        };
        if !start.is_finite()
            || !end.is_finite()
            || start < 0.0
            || end > probe.duration
            || end - start < MIN_TRIM
        {
            return Err("Trim must stay inside the source and keep at least 0.25 seconds.".into());
        }
        clip.start = start;
        clip.end = end;
        if self.selected == Some(clip.id) {
            self.preview_source_time = self.preview_source_time.clamp(start, end);
        }
        self.playhead = self.playhead.min(self.total_length());
        self.dirty = true;
        Ok(())
    }

    pub(crate) fn move_clip(&mut self, from: usize, to: usize) -> Result<(), String> {
        if from >= self.clips.len() || to >= self.clips.len() {
            return Err("Clip index is out of range.".into());
        }
        if from != to {
            let clip = self.clips.remove(from);
            self.clips.insert(to, clip);
            if let Some(id) = self.selected {
                self.select_clip(id);
            }
            self.dirty = true;
        }
        Ok(())
    }

    pub(crate) fn set_audio_options(&mut self, start: f64, gain: f32) -> Result<(), String> {
        if !start.is_finite()
            || start < 0.0
            || start >= self.total_length()
            || !gain.is_finite()
            || !(0.0..=1.0).contains(&gain)
        {
            return Err("Audio start must be on the timeline and level between 0 and 1.".into());
        }
        let Some(track) = &mut self.audio else {
            return Err("Add an audio track first.".into());
        };
        track.start = start;
        track.gain = gain;
        self.dirty = true;
        Ok(())
    }

    pub(crate) fn set_mute_original(&mut self, mute: bool) {
        self.mute_original = mute;
        self.dirty = true;
    }
    pub(crate) fn set_quality(&mut self, quality: Quality) {
        self.quality = quality;
        self.dirty = true;
    }

    pub(crate) fn set_output_path(&mut self, output: PathBuf) -> Result<(), String> {
        if !output
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("mp4"))
        {
            return Err("Choose an .mp4 output file.".into());
        }
        if self.clips.iter().any(|clip| clip.path == output)
            || self
                .audio
                .as_ref()
                .is_some_and(|track| track.path == output)
        {
            return Err("Choose a new file; source files cannot be overwritten.".into());
        }
        self.output = Some(output);
        self.dirty = true;
        Ok(())
    }
    fn selected_index(&self) -> Option<usize> {
        self.clips
            .iter()
            .position(|clip| Some(clip.id) == self.selected)
    }

    fn selected_timeline_start(&self) -> f64 {
        self.clips
            .iter()
            .take(self.selected_index().unwrap_or(0))
            .map(UiClip::length)
            .sum()
    }

    fn set_playhead(&mut self, seconds: f64) {
        let target = seconds.clamp(0.0, self.total_length());
        self.playhead = target;
        let mut cursor = 0.0;
        for clip in &self.clips {
            let end = cursor + clip.length();
            if target <= end || Some(clip.id) == self.clips.last().map(|last| last.id) {
                if clip.ready() {
                    self.selected = Some(clip.id);
                    self.preview_source_time =
                        (clip.start + (target - cursor)).clamp(clip.start, clip.end);
                }
                break;
            }
            cursor = end;
        }
    }

    fn select_clip(&mut self, id: u64) {
        self.selected = Some(id);
        self.preview = None;
        self.preview_key = None;
        if let Some(index) = self.selected_index() {
            self.playhead = self.clips.iter().take(index).map(UiClip::length).sum();
            self.preview_source_time = self.clips[index].start;
        }
    }

    fn poll_background(&mut self, ctx: &egui::Context) {
        while let Ok(message) = self.worker_rx.try_recv() {
            match message {
                WorkerMessage::Clip { id, result } => {
                    if let Some(clip) = self.clips.iter_mut().find(|clip| clip.id == id) {
                        match result {
                            Ok((probe, thumbnail)) => {
                                clip.end = probe.duration.max(0.0);
                                clip.probe = Some(probe);
                                if let Some(bytes) = thumbnail {
                                    clip.thumbnail =
                                        texture_from_png(ctx, &bytes, &format!("video-clip-{id}"))
                                            .ok();
                                }
                                if clip.end < MIN_TRIM {
                                    clip.error = Some("Video is too short to trim.".into());
                                }
                                if self.selected == Some(id) {
                                    self.preview_source_time = clip.start;
                                }
                            }
                            Err(error) => clip.error = Some(error),
                        }
                    }
                }
                WorkerMessage::Audio { path, result } => {
                    if let Some(track) = &mut self.audio
                        && track.path == path
                    {
                        match result {
                            Ok(probe) => track.duration = Some(probe.duration),
                            Err(error) => track.error = Some(error),
                        }
                    }
                }
                WorkerMessage::Preview { key, result } => {
                    self.preview_pending = false;
                    let current = self.selected == Some(key.0)
                        && (self.preview_source_time * 2.0).round() as u32 == key.1;
                    if current {
                        self.preview = result.ok().and_then(|bytes| {
                            texture_from_png(ctx, &bytes, "video-frame-preview").ok()
                        });
                    }
                }
            }
            ctx.request_repaint();
        }
        if let Some((kind, receiver)) = &self.picker {
            let result = receiver.try_recv();
            if let Ok(paths) = result {
                let kind = *kind;
                self.picker = None;
                match kind {
                    PickerKind::Clips => {
                        for path in paths {
                            if let Err(error) = self.add_clip_path(path, ctx) {
                                self.notice = Some(error);
                            }
                        }
                    }
                    PickerKind::Audio => {
                        if let Some(path) = paths.into_iter().next()
                            && let Err(error) = self.set_audio_path(path, ctx)
                        {
                            self.notice = Some(error);
                        }
                    }
                    PickerKind::Output => {
                        if let Some(path) = paths.into_iter().next() {
                            if let Err(error) = self.set_output_path(path) {
                                self.notice = Some(error);
                            }
                        }
                    }
                }
            } else if matches!(result, Err(mpsc::TryRecvError::Disconnected)) {
                self.picker = None;
            }
        }
        if let Some(job) = &mut self.job {
            let status = job.poll();
            let finished = status.phase.is_terminal();
            if matches!(status.phase, ExportPhase::Done) {
                self.dirty = false;
                self.exported_path = status.output.clone();
            }
            self.export_status = Some(status);
            if finished {
                self.job = None;
            } else {
                ctx.request_repaint_after(Duration::from_millis(120));
            }
        }
    }

    fn request_preview(&mut self, ctx: &egui::Context) {
        let Some(clip) = self
            .selected_index()
            .and_then(|index| self.clips.get(index))
        else {
            return;
        };
        if !clip.ready() || self.preview_pending {
            return;
        }
        let source_time = self.preview_source_time.clamp(clip.start, clip.end);
        let key = (clip.id, (source_time * 2.0).round() as u32);
        if self.preview_key == Some(key) {
            return;
        }
        self.preview_key = Some(key);
        self.preview_pending = true;
        let path = clip.path.clone();
        let sender = self.worker_tx.clone();
        let wake = ctx.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("polymath-video-frame".into())
            .spawn(move || {
                let result = video_pipeline::thumbnail(&path, source_time, 1000);
                let _ = sender.send(WorkerMessage::Preview { key, result });
                wake.request_repaint();
            })
        {
            self.preview_pending = false;
            self.notice = Some(format!("Could not preview frame: {error}"));
        }
    }

    fn start_picker(&mut self, kind: PickerKind, frame: &eframe::Frame, ctx: &egui::Context) {
        if self.picker.is_some() {
            return;
        }
        let dialog = rfd::AsyncFileDialog::new().set_parent(frame);
        let future: Pin<Box<dyn Future<Output = Vec<PathBuf>> + Send>> = match kind {
            PickerKind::Clips => Box::pin(async move {
                dialog
                    .set_title("Add videos to Polymath")
                    .add_filter("Videos", &["mp4", "m4v", "mov", "mkv", "webm"])
                    .pick_files()
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|handle| handle.path().to_path_buf())
                    .collect()
            }),
            PickerKind::Audio => Box::pin(async move {
                dialog
                    .set_title("Add an audio track")
                    .add_filter(
                        "Audio",
                        &["mp3", "m4a", "aac", "wav", "aiff", "flac", "ogg", "opus"],
                    )
                    .pick_file()
                    .await
                    .into_iter()
                    .map(|handle| handle.path().to_path_buf())
                    .collect()
            }),
            PickerKind::Output => Box::pin(async move {
                dialog
                    .set_title("Export video to")
                    .add_filter("MP4 video", &["mp4"])
                    .set_file_name("Polymath export.mp4")
                    .save_file()
                    .await
                    .into_iter()
                    .map(|handle| handle.path().to_path_buf())
                    .collect()
            }),
        };
        let (sender, receiver) = mpsc::channel();
        let wake = ctx.clone();
        match std::thread::Builder::new()
            .name("polymath-video-picker".into())
            .spawn(move || {
                let paths = pollster::block_on(future);
                let _ = sender.send(paths);
                wake.request_repaint();
            }) {
            Ok(_) => self.picker = Some((kind, receiver)),
            Err(error) => self.notice = Some(format!("Could not open file picker: {error}")),
        }
    }

    pub(crate) fn start_export(&mut self) -> Result<(), String> {
        if self.job.is_some() {
            return Err("An export is already running.".into());
        }
        let Some(output) = self.output.clone() else {
            return Err("Choose an output file first.".into());
        };
        if self.clips.is_empty() || self.clips.iter().any(|clip| !clip.ready()) {
            return Err("Wait for every clip to load before exporting.".into());
        }
        if self
            .audio
            .as_ref()
            .is_some_and(|track| track.duration.is_none() || track.error.is_some())
        {
            return Err("Wait for the audio track to load, or remove it.".into());
        }
        let request = ExportRequest {
            clips: self
                .clips
                .iter()
                .map(|clip| Clip {
                    path: clip.path.clone(),
                    start: clip.start,
                    end: clip.end,
                })
                .collect(),
            audio: self.audio.as_ref().and_then(|track| {
                track.duration.map(|_| AudioTrack {
                    path: track.path.clone(),
                    start: track.start,
                    gain: track.gain,
                    replace_original: false,
                })
            }),
            output,
            quality: self.quality,
            mute_original: self.mute_original,
        };
        let job = ExportJob::spawn(request)?;
        self.job = Some(job);
        self.export_status = None;
        self.notice = None;
        Ok(())
    }
}

fn texture_from_png(
    ctx: &egui::Context,
    bytes: &[u8],
    name: &str,
) -> Result<TextureHandle, String> {
    let image = image::load_from_memory(bytes)
        .map_err(|error| format!("Could not decode preview: {error}"))?
        .to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let color = ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    Ok(ctx.load_texture(name, color, TextureOptions::LINEAR))
}

#[derive(Clone, Copy)]
struct Palette {
    canvas: Color32,
    surface: Color32,
    raised: Color32,
    line: Color32,
    ink: Color32,
    muted: Color32,
    accent: Color32,
    video: Color32,
    audio: Color32,
}

fn palette(dark: bool) -> Palette {
    let chrome = crate::ui_kit::chrome(dark);
    Palette {
        canvas: chrome.canvas,
        surface: chrome.surface,
        raised: chrome.hover,
        line: chrome.line,
        ink: chrome.ink,
        muted: chrome.muted,
        accent: if dark {
            Color32::from_rgb(255, 158, 124)
        } else {
            Color32::from_rgb(192, 69, 52)
        },
        video: if dark {
            Color32::from_rgb(65, 98, 114)
        } else {
            Color32::from_rgb(207, 224, 231)
        },
        audio: if dark {
            Color32::from_rgb(56, 111, 97)
        } else {
            Color32::from_rgb(209, 234, 224)
        },
    }
}

impl VideoEditor {
    pub(crate) fn show(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame, dark: bool) {
        let ctx = ui.ctx().clone();
        self.poll_background(&ctx);
        self.request_preview(&ctx);
        let p = palette(dark);
        let theme = crate::ui_kit::theme(dark, crate::Workspace::Video);
        let mut picker_request = None;
        let mut begin_export = false;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(p.canvas)
                    .inner_margin(egui::Margin::symmetric(22, 16)),
            )
            .show(ui, |ui| {
                let viewport_height = ui.available_height();
                let viewport_width = ui.available_width();
                egui::ScrollArea::vertical()
                    .id_salt("polymath-video-workspace")
                    .show(ui, |ui| {
                        ui.visuals_mut().override_text_color = Some(p.ink);
                        let heading = |ui: &mut egui::Ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new("POLYMATH  /  VIDEO STUDIO")
                                        .size(10.0)
                                        .strong()
                                        .color(p.accent),
                                );
                                ui.label(
                                    egui::RichText::new("Your cut")
                                        .size(26.0)
                                        .strong()
                                        .color(p.ink),
                                );
                            });
                        };
                        if viewport_width < 650.0 {
                            heading(ui);
                            if !self.clips.is_empty() {
                                ui.add_space(9.0);
                                ui.horizontal_wrapped(|ui| {
                                    self.show_header_actions(
                                        ui,
                                        p,
                                        &theme,
                                        &mut picker_request,
                                        &mut begin_export,
                                    )
                                });
                            }
                        } else {
                            ui.horizontal(|ui| {
                                heading(ui);
                                if !self.clips.is_empty() {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            self.show_header_actions(
                                                ui,
                                                p,
                                                &theme,
                                                &mut picker_request,
                                                &mut begin_export,
                                            );
                                        },
                                    );
                                }
                            });
                        }
                        ui.add_space(14.0);
                        if self.clips.is_empty() {
                            self.show_empty(ui, p, &theme, &mut picker_request, viewport_height);
                        } else {
                            let width = viewport_width;
                            let top_height = (viewport_height * 0.48).clamp(260.0, 430.0);
                            if self.inspector_open && width >= 850.0 {
                                ui.horizontal(|ui| {
                                    let inspector_width = 286.0;
                                    let preview_width = (width - inspector_width - 12.0).max(320.0);
                                    ui.allocate_ui(vec2(preview_width, top_height), |ui| {
                                        self.show_preview(ui, p)
                                    });
                                    ui.add_space(12.0);
                                    ui.allocate_ui(vec2(inspector_width, top_height), |ui| {
                                        self.show_inspector(ui, p, &theme, &mut picker_request)
                                    });
                                });
                            } else if self.inspector_open {
                                ui.allocate_ui(vec2(width, 320.0), |ui| self.show_preview(ui, p));
                                ui.add_space(8.0);
                                self.show_inspector(ui, p, &theme, &mut picker_request);
                            } else {
                                ui.allocate_ui(vec2(width, top_height), |ui| {
                                    self.show_preview(ui, p)
                                });
                            }
                            ui.add_space(12.0);
                            self.show_timeline(ui, p, &theme, &mut picker_request);
                        }
                        if let Some(status) = &self.export_status {
                            ui.add_space(10.0);
                            self.show_export_status(ui, status, p, &theme);
                        }
                        if let Some(notice) = &self.notice {
                            ui.add_space(8.0);
                            ui.colored_label(p.accent, notice);
                        }
                    });
            });
        if let Some(kind) = picker_request {
            self.start_picker(kind, frame, &ctx);
        }
        if begin_export && let Err(error) = self.start_export() {
            self.notice = Some(error);
        }
    }

    fn show_header_actions(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        theme: &egui_shadcn::Theme,
        picker_request: &mut Option<PickerKind>,
        begin_export: &mut bool,
    ) {
        let ready = self.clips.iter().all(UiClip::ready)
            && !self.clips.is_empty()
            && self.job.is_none()
            && self.picker.is_none()
            && self
                .audio
                .as_ref()
                .is_none_or(|audio| audio.duration.is_some() && audio.error.is_none());
        let export_style = primary_button_style(theme, p.accent);
        if Button::new("Export video")
            .variant(ButtonVariant::Solid)
            .size(ButtonSize::Default)
            .radius(ButtonRadius::Medium)
            .style(export_style)
            .enabled(ready)
            .loading(self.job.is_some())
            .show(ui, theme)
            .on_hover_text(if self.output.is_some() {
                "Export to the chosen MP4 file"
            } else {
                "Choose a destination, then export your MP4"
            })
            .clicked()
        {
            if self.output.is_some() {
                *begin_export = true;
            } else {
                self.inspector_open = true;
                *picker_request = Some(PickerKind::Output);
            }
        }
        ui.add_space(8.0);
        if Button::new("Add videos")
            .variant(ButtonVariant::Outline)
            .size(ButtonSize::Default)
            .radius(ButtonRadius::Medium)
            .enabled(self.picker.is_none() && self.clips.len() < MAX_CLIPS)
            .show(ui, theme)
            .clicked()
        {
            *picker_request = Some(PickerKind::Clips);
        }
        ui.add_space(8.0);
        if Button::new(if self.inspector_open {
            "Close properties"
        } else {
            "Properties"
        })
        .variant(if self.inspector_open {
            ButtonVariant::Soft
        } else {
            ButtonVariant::Ghost
        })
        .size(ButtonSize::Default)
        .radius(ButtonRadius::Medium)
        .show(ui, theme)
        .clicked()
        {
            self.inspector_open = !self.inspector_open;
        }
    }

    fn show_empty(
        &self,
        ui: &mut egui::Ui,
        p: Palette,
        theme: &egui_shadcn::Theme,
        picker_request: &mut Option<PickerKind>,
        viewport_height: f32,
    ) {
        let stage_height = (viewport_height * 0.48).clamp(260.0, 430.0);
        egui::Frame::new()
            .fill(p.surface)
            .stroke(Stroke::new(1.0, p.line))
            .corner_radius(12)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("FRAME PREVIEW")
                        .size(10.0)
                        .strong()
                        .color(p.muted),
                );
                ui.add_space(10.0);
                egui::Frame::new()
                    .fill(Color32::from_rgb(13, 18, 24))
                    .corner_radius(10)
                    .inner_margin(egui::Margin::same(16))
                    .show(ui, |ui| {
                        ui.set_min_height((stage_height - 47.0).max(200.0));
                        ui.add_space(((stage_height - 220.0) * 0.38).max(12.0));
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new("01  /  IMPORT FOOTAGE")
                                    .size(10.0)
                                    .strong()
                                    .color(p.accent),
                            );
                            ui.add_space(9.0);
                            ui.label(
                                egui::RichText::new("Every cut starts here")
                                    .size(23.0)
                                    .strong()
                                    .color(Color32::WHITE),
                            );
                            ui.label(
                                egui::RichText::new(
                                    "Choose videos from your Mac. Arrange and trim them below.",
                                )
                                .size(12.0)
                                .color(Color32::from_rgb(174, 185, 195)),
                            );
                            ui.add_space(18.0);
                            if Button::new("Choose videos")
                                .variant(ButtonVariant::Solid)
                                .size(ButtonSize::Lg)
                                .radius(ButtonRadius::Medium)
                                .style(primary_button_style(theme, p.accent))
                                .show(ui, theme)
                                .clicked()
                            {
                                *picker_request = Some(PickerKind::Clips);
                            }
                            ui.add_space(10.0);
                            ui.label(
                                egui::RichText::new("Your source files remain untouched")
                                    .size(10.0)
                                    .color(Color32::from_rgb(137, 151, 164)),
                            );
                        });
                    });
            });
        ui.add_space(12.0);
        egui::Frame::new()
            .fill(p.surface)
            .stroke(Stroke::new(1.0, p.line))
            .corner_radius(12)
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("TIMELINE")
                        .size(10.0)
                        .strong()
                        .color(p.accent),
                );
                ui.add_space(10.0);
                for (lane, hint) in [
                    ("VIDEO", "Your clips will appear here"),
                    ("AUDIO", "Add a soundtrack after importing video"),
                ] {
                    egui::Frame::new()
                        .fill(p.raised)
                        .corner_radius(7)
                        .inner_margin(egui::Margin::symmetric(12, 9))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(lane).size(10.0).strong().color(p.muted),
                                );
                                ui.add_space(10.0);
                                ui.label(egui::RichText::new(hint).size(11.0).color(p.muted));
                            });
                        });
                    ui.add_space(6.0);
                }
            });
    }

    fn show_preview(&mut self, ui: &mut egui::Ui, p: Palette) {
        egui::Frame::new()
            .fill(p.surface)
            .stroke(Stroke::new(1.0, p.line))
            .corner_radius(12)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                let selected = self.selected_index();
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("FRAME PREVIEW")
                            .size(10.0)
                            .strong()
                            .color(p.muted),
                    );
                    if let Some(index) = selected {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(self.clips[index].filename())
                                    .size(12.0)
                                    .color(p.muted),
                            )
                            .on_hover_text(self.clips[index].path.display().to_string());
                        });
                    }
                });
                ui.add_space(8.0);
                let size = vec2(
                    ui.available_width(),
                    (ui.available_height() - 54.0).max(190.0),
                );
                let (stage, _) = ui.allocate_exact_size(size, Sense::hover());
                let painter = ui.painter().with_clip_rect(stage);
                painter.rect_filled(stage, 8.0, Color32::from_rgb(13, 18, 22));
                if let Some(texture) = &self.preview {
                    let source = texture.size_vec2();
                    let fit = (stage.width() / source.x).min(stage.height() / source.y);
                    let image_rect = Rect::from_center_size(stage.center(), source * fit);
                    painter.image(
                        texture.id(),
                        image_rect,
                        Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                } else {
                    painter.text(
                        stage.center(),
                        egui::Align2::CENTER_CENTER,
                        if self.preview_pending {
                            "Reading frame…"
                        } else {
                            "Select a video clip"
                        },
                        egui::FontId::proportional(15.0),
                        Color32::from_rgb(169, 180, 187),
                    );
                }
                if let Some(index) = selected
                    && self.clips[index].ready()
                {
                    let start = self.clips[index].start;
                    let end = self.clips[index].end;
                    let timeline_start = self.selected_timeline_start();
                    let mut source_time = self.preview_source_time.clamp(start, end);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(timecode(source_time))
                                .monospace()
                                .size(12.0)
                                .color(p.ink),
                        );
                        if ui
                            .add_sized(
                                [ui.available_width() - 80.0, 22.0],
                                egui::Slider::new(&mut source_time, start..=end).show_value(false),
                            )
                            .on_hover_text("Scrub frames in the selected clip")
                            .changed()
                        {
                            self.preview_source_time = source_time;
                            self.playhead = timeline_start + (source_time - start);
                        }
                    });
                }
            });
    }

    fn show_inspector(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        theme: &egui_shadcn::Theme,
        picker_request: &mut Option<PickerKind>,
    ) {
        let selected = self.selected_index();
        let clip_count = self.clips.len();
        let total = self.total_length();
        let mut move_clip = 0_i8;
        let mut remove_clip = false;
        let mut trim_changed = false;
        let mut remove_audio = false;
        let mut selected_choice = self.selected;
        egui::Frame::new()
            .fill(p.surface)
            .stroke(Stroke::new(1.0, p.line))
            .corner_radius(12)
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("CLIP PROPERTIES")
                            .size(10.0)
                            .strong()
                            .color(p.accent),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("SELECT CLIP")
                            .size(10.0)
                            .strong()
                            .color(p.muted),
                    );
                    let selected_label = selected
                        .and_then(|index| self.clips.get(index))
                        .map_or_else(
                            || "Choose clip".to_owned(),
                            |clip| {
                                format!("Clip {} · {}", selected.unwrap_or(0) + 1, clip.filename())
                            },
                        );
                    egui::ComboBox::from_id_salt("polymath-video-selected-clip")
                        .selected_text(selected_label)
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for (index, clip) in self.clips.iter().enumerate() {
                                ui.selectable_value(
                                    &mut selected_choice,
                                    Some(clip.id),
                                    format!("Clip {} · {}", index + 1, clip.filename()),
                                );
                            }
                        });
                    ui.add_space(8.0);
                    if let Some(index) = selected {
                        let clip = &mut self.clips[index];
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(clip.filename())
                                    .size(14.0)
                                    .strong()
                                    .color(p.ink),
                            )
                            .truncate(),
                        )
                        .on_hover_text(clip.path.display().to_string());
                        if let Some(probe) = &clip.probe {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} × {}  ·  {} source",
                                    probe.width,
                                    probe.height,
                                    timecode(probe.duration)
                                ))
                                .size(11.0)
                                .color(p.muted),
                            );
                            let video_codec = probe.video_codec.as_deref().unwrap_or("Video");
                            let codec = if let Some(audio_codec) = probe.audio_codec.as_deref() {
                                format!("{video_codec} video  ·  {audio_codec} audio")
                            } else if probe.has_audio {
                                format!("{video_codec} video  ·  audio track")
                            } else {
                                format!("{video_codec} video  ·  no audio")
                            };
                            ui.label(egui::RichText::new(codec).size(11.0).color(p.muted));
                            ui.add_space(12.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("IN").size(10.0).strong().color(p.muted),
                                );
                                let changed = ui
                                    .add(
                                        egui::DragValue::new(&mut clip.start)
                                            .range(0.0..=(clip.end - MIN_TRIM).max(0.0))
                                            .speed(0.05)
                                            .fixed_decimals(2)
                                            .suffix(" s"),
                                    )
                                    .changed();
                                trim_changed |= changed;
                            });
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("OUT")
                                        .size(10.0)
                                        .strong()
                                        .color(p.muted),
                                );
                                let changed = ui
                                    .add(
                                        egui::DragValue::new(&mut clip.end)
                                            .range((clip.start + MIN_TRIM)..=probe.duration)
                                            .speed(0.05)
                                            .fixed_decimals(2)
                                            .suffix(" s"),
                                    )
                                    .changed();
                                trim_changed |= changed;
                            });
                            ui.label(
                                egui::RichText::new(format!(
                                    "Using {} of {}",
                                    timecode(clip.length()),
                                    timecode(probe.duration)
                                ))
                                .size(11.0)
                                .color(p.muted),
                            );
                        } else if let Some(error) = &clip.error {
                            ui.colored_label(p.accent, error);
                        } else {
                            ui.spinner();
                            ui.label("Reading source…");
                        }
                        ui.add_space(11.0);
                        ui.horizontal(|ui| {
                            if Button::new("Move left")
                                .variant(ButtonVariant::Outline)
                                .size(ButtonSize::Sm)
                                .radius(ButtonRadius::Medium)
                                .enabled(index > 0)
                                .show(ui, theme)
                                .clicked()
                            {
                                move_clip = -1;
                            }
                            if Button::new("Move right")
                                .variant(ButtonVariant::Outline)
                                .size(ButtonSize::Sm)
                                .radius(ButtonRadius::Medium)
                                .enabled(index + 1 < clip_count)
                                .show(ui, theme)
                                .clicked()
                            {
                                move_clip = 1;
                            }
                        });
                        if Button::new("Remove clip")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .radius(ButtonRadius::Medium)
                            .show(ui, theme)
                            .clicked()
                        {
                            remove_clip = true;
                        }
                    } else {
                        ui.label(
                            egui::RichText::new("Select a clip on the timeline to trim it.")
                                .size(12.0)
                                .color(p.muted),
                        );
                    }
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("AUDIO TRACK")
                            .size(10.0)
                            .strong()
                            .color(p.accent),
                    );
                    if let Some(audio) = &mut self.audio {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(
                                    audio
                                        .path
                                        .file_name()
                                        .and_then(|name| name.to_str())
                                        .unwrap_or("Audio"),
                                )
                                .size(12.0),
                            )
                            .truncate(),
                        )
                        .on_hover_text(audio.path.display().to_string());
                        if let Some(error) = &audio.error {
                            ui.colored_label(p.accent, error);
                        } else if audio.duration.is_none() {
                            ui.label(
                                egui::RichText::new("Reading audio…")
                                    .size(11.0)
                                    .color(p.muted),
                            );
                        }
                        let max_start = total.max(0.0);
                        if ui
                            .add(
                                egui::Slider::new(&mut audio.start, 0.0..=max_start)
                                    .text("Start")
                                    .suffix(" s"),
                            )
                            .changed()
                        {
                            self.dirty = true;
                        }
                        if ui
                            .add(
                                egui::Slider::new(&mut audio.gain, 0.0..=1.0)
                                    .text("Level")
                                    .show_value(true),
                            )
                            .changed()
                        {
                            self.dirty = true;
                        }
                        if Button::new("Remove audio")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .radius(ButtonRadius::Medium)
                            .show(ui, theme)
                            .clicked()
                        {
                            remove_audio = true;
                        }
                    } else if Button::new("Add audio")
                        .variant(ButtonVariant::Outline)
                        .size(ButtonSize::Sm)
                        .radius(ButtonRadius::Medium)
                        .enabled(self.picker.is_none())
                        .show(ui, theme)
                        .clicked()
                    {
                        *picker_request = Some(PickerKind::Audio);
                    }
                    if ui
                        .checkbox(&mut self.mute_original, "Strip original clip sound")
                        .changed()
                    {
                        self.dirty = true;
                    }
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("EXPORT")
                            .size(10.0)
                            .strong()
                            .color(p.accent),
                    );
                    egui::ComboBox::from_id_salt("polymath-video-quality")
                        .selected_text(quality_label(self.quality))
                        .show_ui(ui, |ui| {
                            for quality in [
                                Quality::Original,
                                Quality::High,
                                Quality::Balanced,
                                Quality::Small,
                            ] {
                                if ui
                                    .selectable_value(
                                        &mut self.quality,
                                        quality,
                                        quality_label(quality),
                                    )
                                    .changed()
                                {
                                    self.dirty = true;
                                }
                            }
                        });
                    ui.add_space(5.0);
                    if Button::new(if self.output.is_some() {
                        "Change destination"
                    } else {
                        "Choose destination"
                    })
                    .variant(ButtonVariant::Outline)
                    .size(ButtonSize::Sm)
                    .radius(ButtonRadius::Medium)
                    .enabled(self.picker.is_none())
                    .show(ui, theme)
                    .clicked()
                    {
                        *picker_request = Some(PickerKind::Output);
                    }
                    if let Some(output) = &self.output {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(output.display().to_string())
                                    .size(11.0)
                                    .color(p.muted),
                            )
                            .truncate(),
                        )
                        .on_hover_text(output.display().to_string());
                    }
                });
            });
        if let Some(index) = selected {
            if trim_changed {
                self.preview_source_time = self
                    .preview_source_time
                    .clamp(self.clips[index].start, self.clips[index].end);
                self.playhead = self.selected_timeline_start() + self.preview_source_time
                    - self.clips[index].start;
                self.dirty = true;
            }
            if move_clip != 0 {
                let target = (index as isize + move_clip as isize) as usize;
                self.clips.swap(index, target);
                self.dirty = true;
                let moved_id = self.clips[target].id;
                self.select_clip(moved_id);
            }
            if remove_clip {
                self.clips.remove(index);
                self.selected = self
                    .clips
                    .get(index.min(self.clips.len().saturating_sub(1)))
                    .map(|clip| clip.id);
                self.preview = None;
                self.preview_key = None;
                self.dirty = true;
            }
        }
        if remove_audio {
            self.audio = None;
            self.dirty = true;
        }
        if selected_choice != self.selected
            && let Some(id) = selected_choice
        {
            self.select_clip(id);
        }
    }

    fn show_timeline(
        &mut self,
        ui: &mut egui::Ui,
        p: Palette,
        theme: &egui_shadcn::Theme,
        picker_request: &mut Option<PickerKind>,
    ) {
        let mut select = None;
        let mut reorder = None;
        let mut seek_to = None;
        let mut trimmed = false;
        let total_length = self.total_length();
        let selected = self.selected;
        let timeline_viewport_width = (ui.available_width() - 28.0).max(160.0);
        if self.timeline_auto_fit {
            self.timeline_scale = fit_timeline_scale(&self.clips, timeline_viewport_width);
        }
        egui::Frame::new()
            .fill(p.surface)
            .stroke(Stroke::new(1.0, p.line))
            .corner_radius(12)
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("TIMELINE")
                            .size(10.0)
                            .strong()
                            .color(p.accent),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "{} clips  ·  {}",
                            self.clips.len(),
                            timecode(total_length)
                        ))
                        .size(11.0)
                        .color(p.muted),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if Button::new("+")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::IconSm)
                            .radius(ButtonRadius::Medium)
                            .show(ui, theme)
                            .on_hover_text("Zoom into timeline")
                            .clicked()
                        {
                            self.timeline_auto_fit = false;
                            self.timeline_scale = (self.timeline_scale * 1.3).min(8192.0);
                        }
                        if Button::new("−")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::IconSm)
                            .radius(ButtonRadius::Medium)
                            .show(ui, theme)
                            .on_hover_text("Zoom out of timeline")
                            .clicked()
                        {
                            self.timeline_auto_fit = false;
                            self.timeline_scale = (self.timeline_scale / 1.3).max(0.01);
                        }
                        if Button::new("Fit")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .radius(ButtonRadius::Medium)
                            .show(ui, theme)
                            .on_hover_text("Fit entire timeline")
                            .clicked()
                        {
                            self.timeline_auto_fit = true;
                            self.timeline_scale =
                                fit_timeline_scale(&self.clips, timeline_viewport_width);
                        }
                    });
                });
                ui.add_space(9.0);
                let scale = self.timeline_scale;
                let widths: Vec<f32> = self
                    .clips
                    .iter()
                    .map(|clip| clip_width(clip, scale))
                    .collect();
                let track_width =
                    widths.iter().sum::<f32>() + (widths.len().saturating_sub(1) as f32 * 7.0);
                egui::ScrollArea::horizontal()
                    .id_salt("polymath-video-timeline")
                    .show(ui, |ui| {
                        let width = track_width.max(timeline_viewport_width);
                        let (ruler, ruler_response) =
                            ui.allocate_exact_size(vec2(width, 29.0), Sense::click_and_drag());
                        let (video_lane, _) =
                            ui.allocate_exact_size(vec2(width, 92.0), Sense::hover());
                        let (audio_lane, _) =
                            ui.allocate_exact_size(vec2(width, 49.0), Sense::hover());
                        let painter = ui.painter().clone();
                        painter.rect_filled(ruler, 4.0, p.raised);
                        painter.rect_filled(video_lane, 7.0, p.raised);
                        painter.rect_filled(audio_lane, 7.0, p.raised);
                        painter.text(
                            pos2(audio_lane.left() + 9.0, audio_lane.center().y),
                            egui::Align2::LEFT_CENTER,
                            "AUDIO",
                            egui::FontId::proportional(10.0),
                            p.muted,
                        );
                        let mut positions: Vec<(u64, Rect, f64, f64)> =
                            Vec::with_capacity(self.clips.len());
                        let mut x = video_lane.left();
                        let mut elapsed = 0.0;
                        for (index, clip) in self.clips.iter_mut().enumerate() {
                            let w = widths[index];
                            let rect =
                                Rect::from_min_size(pos2(x, video_lane.top() + 4.0), vec2(w, 84.0));
                            painter.rect_filled(rect, 8.0, p.video);
                            if let Some(texture) = &clip.thumbnail {
                                let card_painter = painter.with_clip_rect(rect.shrink(1.0));
                                let mut tile_x = rect.left();
                                while tile_x < rect.right() {
                                    let tile = Rect::from_min_size(
                                        pos2(tile_x, rect.top()),
                                        vec2(112.0_f32.min(rect.right() - tile_x), rect.height()),
                                    );
                                    card_painter.image(
                                        texture.id(),
                                        tile,
                                        Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                                        Color32::WHITE,
                                    );
                                    tile_x += 112.0;
                                }
                            }
                            painter.rect_filled(
                                Rect::from_min_max(
                                    pos2(rect.left(), rect.bottom() - 30.0),
                                    rect.max,
                                ),
                                0.0,
                                Color32::from_rgba_unmultiplied(10, 18, 24, 215),
                            );
                            painter.text(
                                pos2(rect.left() + 13.0, rect.bottom() - 17.0),
                                egui::Align2::LEFT_CENTER,
                                shorten_label(&clip.filename(), (w / 8.5) as usize),
                                egui::FontId::proportional(12.0),
                                Color32::WHITE,
                            );
                            painter.rect_stroke(
                                rect,
                                8.0,
                                Stroke::new(
                                    if selected == Some(clip.id) { 2.0 } else { 1.0 },
                                    if selected == Some(clip.id) {
                                        p.accent
                                    } else {
                                        p.line
                                    },
                                ),
                                egui::StrokeKind::Inside,
                            );
                            let left_handle = Rect::from_min_max(
                                rect.min,
                                pos2(rect.left() + 11.0, rect.bottom()),
                            );
                            let right_handle =
                                Rect::from_min_max(pos2(rect.right() - 11.0, rect.top()), rect.max);
                            painter.rect_filled(left_handle, 6.0, p.accent);
                            painter.rect_filled(right_handle, 6.0, p.accent);
                            let body = ui
                                .interact(
                                    rect.shrink2(vec2(12.0, 0.0)),
                                    ui.make_persistent_id(("video-clip", clip.id)),
                                    Sense::click_and_drag(),
                                )
                                .on_hover_text("Select or drag to reorder");
                            if body.clicked() {
                                select = Some(clip.id);
                            }
                            if body.drag_stopped()
                                && let Some(pointer) = body.interact_pointer_pos()
                            {
                                reorder = Some((clip.id, pointer.x - video_lane.left()));
                            }
                            if let Some(probe) = &clip.probe {
                                let left = ui
                                    .interact(
                                        left_handle,
                                        ui.make_persistent_id(("trim-in", clip.id)),
                                        Sense::drag(),
                                    )
                                    .on_hover_text("Drag to trim the beginning");
                                let right = ui
                                    .interact(
                                        right_handle,
                                        ui.make_persistent_id(("trim-out", clip.id)),
                                        Sense::drag(),
                                    )
                                    .on_hover_text("Drag to trim the end");
                                if left.dragged() {
                                    self.timeline_auto_fit = false;
                                    clip.start = (clip.start
                                        + left.drag_delta().x as f64 / scale as f64)
                                        .clamp(0.0, (clip.end - MIN_TRIM).max(0.0));
                                    trimmed = true;
                                }
                                if right.dragged() {
                                    self.timeline_auto_fit = false;
                                    clip.end = (clip.end
                                        + right.drag_delta().x as f64 / scale as f64)
                                        .clamp(clip.start + MIN_TRIM, probe.duration);
                                    trimmed = true;
                                }
                            }
                            painter.text(
                                pos2(x + 5.0, ruler.top() + 7.0),
                                egui::Align2::LEFT_TOP,
                                timecode(elapsed),
                                egui::FontId::monospace(10.0),
                                p.muted,
                            );
                            positions.push((clip.id, rect, elapsed, clip.length()));
                            elapsed += clip.length();
                            x += w + 7.0;
                        }
                        if let Some(track) = &self.audio {
                            let start_x = timeline_x_for_time(&positions, track.start);
                            let end_x = timeline_x_for_time(&positions, total_length);
                            if end_x > start_x {
                                let bar = Rect::from_min_max(
                                    pos2(start_x, audio_lane.top() + 6.0),
                                    pos2(end_x, audio_lane.bottom() - 6.0),
                                );
                                painter.rect_filled(bar, 6.0, p.audio);
                                painter.text(
                                    pos2(bar.left() + 12.0, bar.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    shorten_label(
                                        track
                                            .path
                                            .file_name()
                                            .and_then(|name| name.to_str())
                                            .unwrap_or("Audio"),
                                        (bar.width() / 8.0) as usize,
                                    ),
                                    egui::FontId::proportional(11.0),
                                    p.ink,
                                );
                            }
                        }
                        let playhead_x = timeline_x_for_time(&positions, self.playhead);
                        painter.line_segment(
                            [
                                pos2(playhead_x, ruler.top()),
                                pos2(playhead_x, audio_lane.bottom()),
                            ],
                            Stroke::new(2.0, p.accent),
                        );
                        painter.circle_filled(pos2(playhead_x, ruler.top() + 5.0), 5.0, p.accent);
                        if (ruler_response.clicked() || ruler_response.dragged())
                            && let Some(pointer) = ruler_response.interact_pointer_pos()
                        {
                            seek_to = Some(timeline_time_for_x(&positions, pointer.x));
                        }
                    });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Drag clip edges to trim  ·  Drag a clip to reorder")
                            .size(11.0)
                            .color(p.muted),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if Button::new("Add soundtrack")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .radius(ButtonRadius::Medium)
                            .enabled(self.picker.is_none())
                            .show(ui, theme)
                            .clicked()
                        {
                            *picker_request = Some(PickerKind::Audio);
                        }
                    });
                });
            });
        if let Some(id) = select {
            self.select_clip(id);
        }
        if let Some(seconds) = seek_to {
            self.set_playhead(seconds);
        }
        if trimmed {
            self.dirty = true;
            if let Some(index) = self.selected_index() {
                self.preview_source_time = self
                    .preview_source_time
                    .clamp(self.clips[index].start, self.clips[index].end);
                self.playhead = self.selected_timeline_start() + self.preview_source_time
                    - self.clips[index].start;
            }
        }
        if let Some((id, pointer_x)) = reorder {
            let widths: Vec<f32> = self
                .clips
                .iter()
                .map(|clip| clip_width(clip, self.timeline_scale))
                .collect();
            let mut cursor = 0.0;
            let mut destination = self.clips.len();
            for (index, width) in widths.iter().enumerate() {
                if pointer_x < cursor + *width * 0.5 {
                    destination = index;
                    break;
                }
                cursor += *width + 7.0;
            }
            if let Some(from) = self.clips.iter().position(|clip| clip.id == id) {
                let insertion = if destination > from {
                    destination - 1
                } else {
                    destination
                };
                if insertion != from {
                    let clip = self.clips.remove(from);
                    self.clips.insert(insertion, clip);
                    self.dirty = true;
                    self.select_clip(id);
                }
            }
        }
    }

    fn show_export_status(
        &self,
        ui: &mut egui::Ui,
        status: &ExportStatus,
        p: Palette,
        theme: &egui_shadcn::Theme,
    ) {
        egui::Frame::new()
            .fill(p.surface)
            .stroke(Stroke::new(1.0, p.line))
            .corner_radius(9)
            .inner_margin(egui::Margin::symmetric(13, 9))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(status.phase.label())
                            .size(12.0)
                            .strong()
                            .color(p.ink),
                    );
                    if self.job.is_some() {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if Button::new("Cancel")
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Sm)
                                .radius(ButtonRadius::Medium)
                                .show(ui, theme)
                                .clicked()
                            {
                                if let Some(job) = &self.job {
                                    job.cancel();
                                }
                            }
                        });
                    }
                });
                ui.add(
                    egui::ProgressBar::new(status.fraction.clamp(0.0, 1.0))
                        .desired_width(ui.available_width()),
                );
                if let Some(error) = &status.error {
                    ui.colored_label(p.accent, error);
                }
                if let Some(path) = &status.output {
                    ui.label(
                        egui::RichText::new(path.display().to_string())
                            .size(11.0)
                            .color(p.muted),
                    );
                }
            });
    }
}

fn timecode(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    format!(
        "{:02}:{:02}:{:02}",
        total / 3600,
        (total / 60) % 60,
        total % 60
    )
}

fn quality_label(quality: Quality) -> &'static str {
    quality.label()
}

fn clip_width(clip: &UiClip, scale: f32) -> f32 {
    (clip.length() as f32 * scale).clamp(128.0, 2400.0)
}

fn fit_timeline_scale(clips: &[UiClip], viewport_width: f32) -> f32 {
    if clips.is_empty() {
        return 18.0;
    }
    let target = (viewport_width - 10.0).max(160.0);
    let content_width = |scale: f32| {
        clips
            .iter()
            .map(|clip| clip_width(clip, scale))
            .sum::<f32>()
            + clips.len().saturating_sub(1) as f32 * 7.0
    };
    let mut low = 0.01_f32;
    let mut high = 8192.0_f32;
    for _ in 0..22 {
        let middle = (low + high) * 0.5;
        if content_width(middle) <= target {
            low = middle;
        } else {
            high = middle;
        }
    }
    low
}

fn primary_button_style(theme: &egui_shadcn::Theme, accent: Color32) -> ButtonStyle {
    let mut style =
        ButtonStyle::from_variant_with_accent(&theme.palette, ButtonVariant::Solid, accent);
    let ink = if accent.r() > 220 {
        Color32::from_rgb(25, 27, 30)
    } else {
        Color32::WHITE
    };
    style.text = ink;
    style.text_hover = ink;
    style.text_active = ink;
    style
}

fn timeline_x_for_time(positions: &[(u64, Rect, f64, f64)], seconds: f64) -> f32 {
    if let Some((_, rect, start, length)) = positions
        .iter()
        .find(|(_, _, start, length)| seconds <= start + length)
    {
        return rect.left()
            + ((seconds - start) / length.max(MIN_TRIM)).clamp(0.0, 1.0) as f32 * rect.width();
    }
    positions.last().map_or(0.0, |(_, rect, _, _)| rect.right())
}

fn timeline_time_for_x(positions: &[(u64, Rect, f64, f64)], x: f32) -> f64 {
    for (_, rect, start, length) in positions {
        if x <= rect.right() {
            return start + ((x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64 * length;
        }
    }
    positions
        .last()
        .map_or(0.0, |(_, _, start, length)| start + length)
}

fn shorten_label(value: &str, max_chars: usize) -> String {
    let limit = max_chars.max(4);
    if value.chars().count() <= limit {
        return value.to_owned();
    }
    let head: String = value.chars().take(limit.saturating_sub(1)).collect();
    format!("{head}…")
}
