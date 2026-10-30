//! Local audio and video playback inside Reader. Decoding begins only after a file is opened.
use crate::audio::CpalOut;
use crate::media_queue::{Kind, Queue, Sort};
use deckcraft_media::{Bytes, ClipParams, MediaInfo, PlayState, Player, VideoFeed, VoiceSpec};
use egui::{
    Color32, ColorImage, Rect, Sense, Stroke, TextureHandle, TextureOptions, Vec2, pos2, vec2,
};
use egui_extras::{Column, TableBuilder};
use egui_shadcn::{Button, ButtonRadius, ButtonSize, ButtonVariant};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
        mpsc::{self, Receiver},
    },
    time::Duration,
};

const MAX_MEDIA_BYTES: u64 = 512 * 1024 * 1024;
const VOICE: u64 = 1;

struct Loaded {
    bytes: Bytes,
    info: MediaInfo,
    feed: Option<VideoFeed>,
    texture: Option<TextureHandle>,
    frame_time: Option<f64>,
}

enum State {
    Loading(Receiver<Result<(Bytes, MediaInfo), String>>),
    Ready(Loaded),
    Error(String),
}

pub struct MediaView {
    pub path: String,
    state: State,
    player: Player,
    output_gain: Arc<AtomicU32>,
    speed: f64,
    queue: Queue,
    seek_to: f64,
    auto_play_when_ready: bool,
    end_handled: bool,
    pub fullscreen: bool,
    video_zoom: f32,
    video_pan: Vec2,
    queue_open: bool,
    notice: Option<String>,
}

enum Action {
    Play,
    Pause,
    Seek(f64),
    Previous,
    Next,
    Fullscreen(bool),
    Sort(Sort),
    Select(std::path::PathBuf),
    Queue(bool),
    Volume(f32),
    Speed(f64),
    FitVideo,
}

impl MediaView {
    pub fn open(path: &Path) -> Result<Self, String> {
        let state = Self::load_state(path)?;
        let queue = Queue::new(path);
        let queue_open = false;
        let output_gain = Arc::new(AtomicU32::new(1.0_f32.to_bits()));
        Ok(Self {
            path: path.to_string_lossy().into_owned(),
            state,
            player: Player::new(Some(Box::new(CpalOut::with_gain(output_gain.clone())))),
            output_gain,
            speed: 1.0,
            queue,
            seek_to: 0.0,
            auto_play_when_ready: false,
            end_handled: false,
            fullscreen: false,
            video_zoom: 1.0,
            video_pan: Vec2::ZERO,
            queue_open,
            notice: None,
        })
    }

    fn load_state(path: &Path) -> Result<State, String> {
        let metadata = std::fs::metadata(path).map_err(|e| format!("Could not open media: {e}"))?;
        if !metadata.is_file() {
            return Err("Choose a media file.".into());
        }
        if metadata.len() > MAX_MEDIA_BYTES {
            return Err("This file is over the 512 MB in-app playback limit.".into());
        }
        let file = path.to_path_buf();
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("polymath-media-open".into())
            .spawn(move || {
                let result = std::fs::read(&file)
                    .map(Arc::new)
                    .map_err(|e| format!("Could not read media: {e}"))
                    .and_then(|bytes| {
                        deckcraft_media::probe(&bytes)
                            .map_err(|e| e.to_string())
                            .and_then(|info| {
                                info.playable().then_some((bytes, info)).ok_or_else(|| {
                                    "This audio or video codec is not supported yet.".into()
                                })
                            })
                    });
                let _ = tx.send(result);
            })
            .map_err(|e| format!("Could not start media reader: {e}"))?;
        Ok(State::Loading(rx))
    }

    pub fn inspect(&self) -> serde_json::Value {
        let status = self.player.status(VOICE);
        let item = self.queue.current(Path::new(&self.path));
        let queue = serde_json::json!({
            "folder":self.queue.folder,"count":self.queue.items.len(),"ready":self.queue.ready,
            "index":self.queue.index(Path::new(&self.path)),"sort":self.queue.sort.label(),
            "title":item.map(|item| item.title.as_str()),
            "artist":item.and_then(|item| item.artist.as_deref()),
            "album":item.and_then(|item| item.album.as_deref()),
            "fullscreen":self.fullscreen,
            "videoZoom":self.video_zoom,
            "volume":self.volume(),
            "speed":self.speed,
        });
        match &self.state {
            State::Loading(_) => {
                serde_json::json!({"kind":"media","path":self.path,"state":"loading","readOnly":true,"queue":queue})
            }
            State::Error(error) => {
                serde_json::json!({"kind":"media","path":self.path,"state":"error","error":error,"readOnly":true,"queue":queue})
            }
            State::Ready(loaded) => serde_json::json!({
                "kind":"media","path":self.path,"state":status.as_ref().map_or("ready", |s| s.state.name()),
                "position":status.as_ref().map_or(0.0, |s| s.position),"duration":loaded.info.duration(),
                "container":loaded.info.container,"audio":loaded.info.audio.as_ref().map(|a| &a.codec),
                "video":loaded.info.video.as_ref().map(|v| &v.codec),"readOnly":true,"queue":queue,
            }),
        }
    }

    pub fn queue_page(&self, offset: usize, limit: usize) -> serde_json::Value {
        let items: Vec<_> = self.queue.items.iter().skip(offset).take(limit.min(100)).map(|item| serde_json::json!({
            "path":item.path,"name":item.name,"title":item.title,"artist":item.artist,"album":item.album,
        })).collect();
        serde_json::json!({"folder":self.queue.folder,"ready":self.queue.ready,"sort":self.queue.sort.label(),"total":self.queue.items.len(),"items":items})
    }

    pub fn set_sort(&mut self, sort: Sort) {
        self.queue.set_sort(sort);
    }

    pub fn sort_from_label(label: &str) -> Option<Sort> {
        match label.to_ascii_lowercase().as_str() {
            "name" => Some(Sort::Name),
            "artist" => Some(Sort::Artist),
            "album" => Some(Sort::Album),
            _ => None,
        }
    }

    pub fn adjacent(&mut self, direction: i8) -> Result<(), String> {
        let Some(path) = self
            .queue
            .adjacent(Path::new(&self.path), direction)
            .map(Path::to_path_buf)
        else {
            return Err(if self.queue.ready {
                "No more files in this folder."
            } else {
                "The folder queue is still loading."
            }
            .into());
        };
        self.select(path, true)
    }

    fn select(&mut self, path: std::path::PathBuf, play: bool) -> Result<(), String> {
        let state = Self::load_state(&path)?;
        self.player.stop_all();
        self.player = Player::new(Some(Box::new(CpalOut::with_gain(self.output_gain.clone()))));
        self.path = path.to_string_lossy().into_owned();
        self.state = state;
        self.seek_to = 0.0;
        self.auto_play_when_ready = play;
        self.end_handled = false;
        self.video_zoom = 1.0;
        self.video_pan = Vec2::ZERO;
        self.notice = None;
        Ok(())
    }

    fn volume(&self) -> f32 {
        f32::from_bits(self.output_gain.load(Ordering::Relaxed)).clamp(0.0, 1.0)
    }

    pub fn set_volume(&mut self, volume: f32) -> Result<(), String> {
        if !volume.is_finite() {
            return Err("Volume must be a finite number.".into());
        }
        self.output_gain
            .store(volume.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
        Ok(())
    }

    pub fn set_speed(&mut self, speed: f64) -> Result<(), String> {
        if !speed.is_finite() || !(0.25..=4.0).contains(&speed) {
            return Err("Playback speed must be from 0.25× to 4×.".into());
        }
        self.speed = speed;
        self.player.set_speed(VOICE, speed);
        Ok(())
    }

    pub fn set_fullscreen(&mut self, ctx: &egui::Context, enabled: bool) -> Result<(), String> {
        self.fullscreen = enabled;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(enabled));
        Ok(())
    }

    pub fn play(&mut self) -> Result<(), String> {
        let State::Ready(loaded) = &mut self.state else {
            return Err("Media is still opening.".into());
        };
        if loaded.feed.is_none() && loaded.info.video.as_ref().is_some_and(|v| v.decodable) {
            loaded.feed = Some(VideoFeed::new(loaded.bytes.clone()).map_err(|e| e.to_string())?);
        }
        self.player.play(
            VOICE,
            VOICE,
            &loaded.bytes,
            VoiceSpec {
                clip: ClipParams::default(),
                looping: false,
                duration: loaded.info.duration(),
                audio: loaded.info.audio.as_ref().is_some_and(|a| a.decodable),
            },
            self.player.status(VOICE).is_none().then_some(self.seek_to),
        );
        self.player.set_speed(VOICE, self.speed);
        self.end_handled = false;
        Ok(())
    }

    pub fn pause(&mut self) {
        self.player.pause(VOICE);
    }

    pub fn seek(&mut self, seconds: f64) -> Result<(), String> {
        let State::Ready(loaded) = &mut self.state else {
            return Err("Media is still opening.".into());
        };
        if !seconds.is_finite() || seconds < 0.0 {
            return Err("Seek time must be a nonnegative number.".into());
        }
        let position = seconds.min(loaded.info.duration());
        self.seek_to = position;
        if self.player.status(VOICE).is_some() {
            self.player.seek(VOICE, position);
        }
        if let Some(feed) = &mut loaded.feed {
            feed.seek(position);
        }
        Ok(())
    }

    pub fn tick(&mut self, ctx: &egui::Context) {
        self.queue.poll(Path::new(&self.path));
        if !self.queue.ready {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        let mut loaded_now = false;
        if let State::Loading(rx) = &self.state {
            match rx.try_recv() {
                Ok(Ok((bytes, info))) => {
                    self.state = State::Ready(Loaded {
                        bytes,
                        info,
                        feed: None,
                        texture: None,
                        frame_time: None,
                    });
                    loaded_now = true;
                }
                Ok(Err(error)) => self.state = State::Error(error),
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.state = State::Error("Media reader stopped unexpectedly.".into())
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(50))
                }
            }
        }
        if loaded_now && self.auto_play_when_ready {
            self.auto_play_when_ready = false;
            self.notice = self.play().err();
        }
        self.player.tick(ctx.input(|i| i.time));
        if self
            .player
            .status(VOICE)
            .is_some_and(|status| status.state == PlayState::Ended)
            && !self.end_handled
            && self.queue.ready
        {
            self.end_handled = true;
            if self.queue.adjacent(Path::new(&self.path), 1).is_some() {
                self.notice = self.adjacent(1).err();
            }
        }
        if self.player.any_playing() {
            ctx.request_repaint_after(Duration::from_millis(33));
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, dark: bool) {
        if self.fullscreen {
            self.show_fullscreen(ui);
            return;
        }
        let mut action = None;
        let mut volume = self.volume();
        let canvas = if dark {
            Color32::from_rgb(22, 24, 27)
        } else {
            Color32::from_rgb(250, 250, 250)
        };
        let ink = if dark {
            Color32::from_rgb(241, 245, 247)
        } else {
            Color32::from_rgb(25, 27, 30)
        };
        let muted = if dark {
            Color32::from_rgb(178, 191, 198)
        } else {
            Color32::from_rgb(102, 108, 115)
        };
        let reader_blue = if dark {
            Color32::from_rgb(137, 199, 225)
        } else {
            Color32::from_rgb(36, 101, 132)
        };
        let play_color = Color32::from_rgb(235, 101, 72);
        let error_ink = if dark {
            Color32::from_rgb(255, 186, 169)
        } else {
            Color32::from_rgb(151, 49, 38)
        };
        let filename = Path::new(&self.path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("Media file")
            .to_owned();
        let current_item = self.queue.current(Path::new(&self.path));
        let display_title = current_item.map_or(filename.clone(), |item| item.title.clone());
        let display_credit =
            current_item.map_or(String::new(), |item| match (&item.artist, &item.album) {
                (Some(artist), Some(album)) => format!("{artist}  ·  {album}"),
                (Some(artist), None) => artist.clone(),
                (None, Some(album)) => album.clone(),
                _ => String::new(),
            });
        let video_view = self.queue.kind == Kind::Video;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(canvas).inner_margin(if video_view {
                egui::Margin::symmetric(18, 8)
            } else {
                egui::Margin::symmetric(22, 14)
            }))
            .show(ui, |ui| {
                ui.visuals_mut().override_text_color = Some(ink);
                let theme = crate::ui_kit::theme(dark, crate::Workspace::Reader);
                let can_show_queue = ui.available_width() > 740.0;
                if self.queue_open && can_show_queue {
                    let queue = &self.queue;
                    let current = Path::new(&self.path);
                    egui::Panel::right("polymath-media-queue")
                        .resizable(true)
                        .default_size(292.0)
                        .min_size(235.0)
                        .max_size(380.0)
                        .frame(
                            egui::Frame::new()
                                .fill(if dark {
                                    Color32::from_rgb(31, 33, 37)
                                } else {
                                    Color32::from_rgb(242, 244, 246)
                                })
                                .inner_margin(egui::Margin::same(16)),
                        )
                        .show(ui, |ui| {
                            if let Some(chosen) = show_queue(ui, queue, current, dark) {
                                action = Some(chosen);
                            }
                        });
                }
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(match self.queue.kind {
                            Kind::Audio => "MUSIC / LOCAL FOLDER",
                            Kind::Video => "VIDEO / LOCAL FOLDER",
                        })
                        .size(11.0)
                        .color(reader_blue)
                        .strong(),
                    );
                    let queue_open = self.queue_open;
                    let queue_icon = move |p: &egui::Painter, center, _, color| {
                        let direction = if queue_open { 1.0 } else { -1.0 };
                        let tip = center + vec2(4.0 * direction, 0.0);
                        let stroke = Stroke::new(2.0, color);
                        p.line_segment([center + vec2(-3.0 * direction, -5.0), tip], stroke);
                        p.line_segment([center + vec2(-3.0 * direction, 5.0), tip], stroke);
                    };
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if Button::new(if self.queue_open {
                            "Hide queue"
                        } else {
                            "Show queue"
                        })
                        .variant(if self.queue_open {
                            ButtonVariant::Soft
                        } else {
                            ButtonVariant::Ghost
                        })
                        .size(ButtonSize::IconSm)
                        .radius(ButtonRadius::Medium)
                        .icon(&queue_icon)
                        .show(ui, &theme)
                        .on_hover_text(if self.queue_open {
                            "Hide Up Next playlist"
                        } else {
                            "Show Up Next playlist"
                        })
                        .clicked()
                        {
                            action = Some(Action::Queue(!self.queue_open));
                        }
                        if self.queue.kind == Kind::Video && self.video_zoom > 1.01 {
                            ui.add_space(8.0);
                            let fit_icon = |p: &egui::Painter, center, _, color| {
                                draw_fit_icon(p, center, color);
                            };
                            if Button::new("Fit video to window")
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::IconSm)
                                .radius(ButtonRadius::Medium)
                                .icon(&fit_icon)
                                .show(ui, &theme)
                                .on_hover_text("Fit video to window")
                                .clicked()
                            {
                                action = Some(Action::FitVideo);
                            }
                        }
                    });
                });
                // Measure outside the scroll area: its inner Ui has unbounded height.
                // Video already has a filename in the Reader tab, so its stage can
                // use the space once occupied by a duplicate title and detail row.
                let stage_height_budget =
                    (ui.available_height() - if video_view { 120.0 } else { 200.0 }).max(190.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if !video_view {
                        ui.add_space(4.0);
                    }
                    ui.vertical_centered(|ui| {
                        let viewport_width = ui.available_width().max(280.0);
                        let video_aspect = match &self.state {
                            State::Ready(loaded) if self.queue.kind == Kind::Video => loaded
                                .info
                                .video
                                .as_ref()
                                .filter(|v| v.width > 0 && v.height > 0)
                                .map_or(16.0 / 9.0, |v| v.width as f32 / v.height as f32),
                            _ => 16.0 / 9.0,
                        };
                        let width = if self.queue.kind == Kind::Video {
                            viewport_width.min((stage_height_budget * video_aspect).max(280.0))
                        } else {
                            viewport_width.min(1060.0)
                        };
                        let painter = ui.painter().clone();
                        if !video_view {
                            let (header, response) =
                                ui.allocate_exact_size(vec2(width, 57.0), Sense::hover());
                            response.on_hover_text(&self.path);
                            painter.text(
                                header.left_top(),
                                egui::Align2::LEFT_TOP,
                                "NOW PLAYING",
                                egui::FontId::proportional(11.0),
                                reader_blue,
                            );
                            painter.text(
                                header.left_top() + vec2(0.0, 18.0),
                                egui::Align2::LEFT_TOP,
                                shorten(&filename, ((width - 30.0) / 26.0) as usize),
                                egui::FontId::proportional(25.0),
                                ink,
                            );
                        }
                        match &mut self.state {
                            State::Loading(_) => {
                                let (rect, _) =
                                    ui.allocate_exact_size(vec2(width, 280.0), Sense::hover());
                                painter.rect_filled(
                                    rect,
                                    14.0,
                                    if dark {
                                        Color32::from_rgb(30, 38, 44)
                                    } else {
                                        Color32::from_rgb(229, 236, 240)
                                    },
                                );
                                painter.text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "Opening local media…",
                                    egui::FontId::proportional(17.0),
                                    muted,
                                );
                            }
                            State::Error(error) => {
                                let (rect, _) =
                                    ui.allocate_exact_size(vec2(width, 180.0), Sense::hover());
                                painter.rect_filled(
                                    rect,
                                    14.0,
                                    if dark {
                                        Color32::from_rgb(47, 34, 34)
                                    } else {
                                        Color32::from_rgb(252, 237, 234)
                                    },
                                );
                                painter.text(
                                    rect.left_center() + vec2(24.0, 0.0),
                                    egui::Align2::LEFT_CENTER,
                                    shorten(error, ((width - 50.0) / 15.0) as usize),
                                    egui::FontId::proportional(15.0),
                                    error_ink,
                                );
                            }
                            State::Ready(loaded) => {
                                let video = loaded.info.video.as_ref().is_some_and(|v| v.decodable);
                                let status = self.player.status(VOICE);
                                let playing = status.as_ref().is_some_and(|s| {
                                    matches!(s.state, PlayState::Playing | PlayState::Loading)
                                });
                                let position = status.as_ref().map_or(self.seek_to, |s| s.position);
                                let duration = loaded.info.duration();
                                let height = if video {
                                    (width / video_aspect).min(stage_height_budget).max(160.0)
                                } else if width < 620.0 {
                                    280.0_f32.min(stage_height_budget)
                                } else {
                                    300.0_f32.min(stage_height_budget)
                                };
                                let (stage, stage_response) = ui.allocate_exact_size(
                                    vec2(width, height),
                                    Sense::click_and_drag(),
                                );
                                let stage_response = stage_response.on_hover_text(if video {
                                    "Pinch to zoom · Drag to pan · Double-click for full screen"
                                } else if playing {
                                    "Pause · Space"
                                } else {
                                    "Play · Space"
                                });
                                if video && stage_response.double_clicked() {
                                    action = Some(Action::Fullscreen(true));
                                } else if stage_response.clicked() {
                                    action =
                                        Some(if playing { Action::Pause } else { Action::Play });
                                }
                                if video {
                                    let (pinch, pointer) = ui.input(|input| {
                                        (input.zoom_delta(), input.pointer.hover_pos())
                                    });
                                    if let Some(pointer) =
                                        pointer.filter(|point| stage.contains(*point))
                                        && (pinch - 1.0).abs() > 0.001
                                    {
                                        apply_anchored_zoom(
                                            &mut self.video_zoom,
                                            &mut self.video_pan,
                                            stage,
                                            pointer,
                                            pinch,
                                        );
                                    }
                                    if stage_response.dragged() && self.video_zoom > 1.0 {
                                        self.video_pan += stage_response.drag_delta();
                                    }
                                }
                                let painter = ui.painter().clone();
                                painter.rect_filled(stage, 14.0, Color32::from_rgb(15, 28, 37));
                                if video {
                                    render_video_frame(
                                        loaded,
                                        ui,
                                        stage,
                                        position,
                                        self.video_zoom,
                                        &mut self.video_pan,
                                    );
                                    if !playing {
                                        let center = stage.center();
                                        painter.circle_filled(
                                            center,
                                            38.0,
                                            Color32::from_rgba_unmultiplied(12, 23, 30, 222),
                                        );
                                        draw_play_icon(&painter, center, false, Color32::WHITE);
                                    }
                                } else {
                                    paint_audio_art(
                                        &painter,
                                        stage,
                                        &display_title,
                                        &display_credit,
                                        &loaded.info,
                                        width < 620.0,
                                    );
                                }
                                ui.add_space(if video { 3.0 } else { 17.0 });
                                let (seek_rect, seek_response) = ui.allocate_exact_size(
                                    vec2(width, if video { 18.0 } else { 30.0 }),
                                    Sense::click_and_drag(),
                                );
                                let seek_response = seek_response
                                    .on_hover_text("Seek through media · ← / → move 5 seconds");
                                let track =
                                    Rect::from_center_size(seek_rect.center(), vec2(width, 5.0));
                                painter.rect_filled(
                                    track,
                                    3.0,
                                    if dark {
                                        Color32::from_rgb(68, 83, 92)
                                    } else {
                                        Color32::from_rgb(196, 209, 216)
                                    },
                                );
                                let ratio = if duration > 0.0 {
                                    (position / duration).clamp(0.0, 1.0) as f32
                                } else {
                                    0.0
                                };
                                if ratio > 0.0 {
                                    painter.rect_filled(
                                        Rect::from_min_max(
                                            track.min,
                                            pos2(
                                                track.left() + track.width() * ratio,
                                                track.bottom(),
                                            ),
                                        ),
                                        3.0,
                                        play_color,
                                    );
                                }
                                painter.circle_filled(
                                    pos2(track.left() + track.width() * ratio, track.center().y),
                                    if seek_response.hovered() || seek_response.dragged() {
                                        7.0
                                    } else {
                                        5.0
                                    },
                                    play_color,
                                );
                                if (seek_response.clicked() || seek_response.dragged())
                                    && duration > 0.0
                                    && let Some(pointer) = seek_response.interact_pointer_pos()
                                {
                                    let fraction = ((pointer.x - track.left()) / track.width())
                                        .clamp(0.0, 1.0);
                                    action = Some(Action::Seek(fraction as f64 * duration));
                                }
                                ui.allocate_ui_with_layout(
                                    vec2(width, if video { 44.0 } else { 64.0 }),
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        ui.add_space(((width - 238.0) * 0.5).max(0.0));
                                        show_transport(
                                            ui,
                                            &theme,
                                            playing,
                                            position,
                                            duration,
                                            &mut action,
                                        );
                                    },
                                );
                                ui.allocate_ui_with_layout(
                                    vec2(width, if video { 24.0 } else { 30.0 }),
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "{}  /  {}",
                                                timestamp(position),
                                                timestamp(duration)
                                            ))
                                            .font(egui::FontId::monospace(12.0))
                                            .color(muted),
                                        );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.label(
                                                    egui::RichText::new(format!(
                                                        "{}%",
                                                        (volume * 100.0).round() as u32
                                                    ))
                                                    .size(11.0)
                                                    .color(muted),
                                                );
                                                if ui
                                                    .add_sized(
                                                        [110.0, 20.0],
                                                        egui::Slider::new(&mut volume, 0.0..=1.0)
                                                            .show_value(false),
                                                    )
                                                    .on_hover_text("Volume")
                                                    .changed()
                                                {
                                                    action = Some(Action::Volume(volume));
                                                }
                                                ui.label(
                                                    egui::RichText::new("VOL")
                                                        .size(10.0)
                                                        .strong()
                                                        .color(muted),
                                                );
                                                ui.add_space(8.0);
                                                show_speed(ui, self.speed, &mut action);
                                                let full_screen_icon =
                                                    |p: &egui::Painter, center, _, color| {
                                                        draw_fullscreen_enter_icon(
                                                            p, center, color,
                                                        );
                                                    };
                                                if Button::new("Full screen")
                                                    .variant(ButtonVariant::Ghost)
                                                    .size(ButtonSize::IconSm)
                                                    .radius(ButtonRadius::Medium)
                                                    .icon(&full_screen_icon)
                                                    .show(ui, &theme)
                                                    .on_hover_text(
                                                        "Full screen · double-click video",
                                                    )
                                                    .clicked()
                                                {
                                                    action = Some(Action::Fullscreen(true));
                                                }
                                            },
                                        );
                                    },
                                );
                                if !video {
                                    let (detail, _) =
                                        ui.allocate_exact_size(vec2(width, 45.0), Sense::hover());
                                    painter.hline(
                                        detail.x_range(),
                                        detail.top() + 3.0,
                                        Stroke::new(
                                            1.0,
                                            if dark {
                                                Color32::from_rgb(58, 70, 78)
                                            } else {
                                                Color32::from_rgb(210, 220, 225)
                                            },
                                        ),
                                    );
                                    let codec = match (&loaded.info.video, &loaded.info.audio) {
                                        (Some(v), Some(a)) => format!(
                                            "{} video  ·  {} audio  ·  {} × {}",
                                            v.codec, a.codec, v.width, v.height
                                        ),
                                        (Some(v), None) => {
                                            format!(
                                                "{} video  ·  {} × {}",
                                                v.codec, v.width, v.height
                                            )
                                        }
                                        (None, Some(a)) => {
                                            format!("{} audio  ·  {} channels", a.codec, a.channels)
                                        }
                                        _ => loaded.info.container.to_owned(),
                                    };
                                    painter.text(
                                        pos2(detail.left(), detail.top() + 22.0),
                                        egui::Align2::LEFT_CENTER,
                                        shorten(&codec, ((width - 120.0) / 12.0) as usize),
                                        egui::FontId::proportional(12.0),
                                        muted,
                                    );
                                    if width > 510.0 {
                                        painter.text(
                                            pos2(detail.right() - 12.0, detail.top() + 22.0),
                                            egui::Align2::RIGHT_CENTER,
                                            "Local playback",
                                            egui::FontId::proportional(12.0),
                                            muted,
                                        );
                                    }
                                }
                                if let Some(notice) = &self.notice {
                                    ui.colored_label(error_ink, notice);
                                }
                                let keys = ui.ctx().input(|i| {
                                    (
                                        i.key_pressed(egui::Key::Space),
                                        i.key_pressed(egui::Key::ArrowLeft),
                                        i.key_pressed(egui::Key::ArrowRight),
                                    )
                                });
                                if keys.0 {
                                    action =
                                        Some(if playing { Action::Pause } else { Action::Play });
                                }
                                if keys.1 {
                                    action = Some(Action::Seek((position - 5.0).max(0.0)));
                                }
                                if keys.2 {
                                    action = Some(Action::Seek((position + 5.0).min(duration)));
                                }
                            }
                        }
                    });
                    if self.queue_open && !can_show_queue {
                        ui.add_space(18.0);
                        ui.allocate_ui(vec2(ui.available_width(), 360.0), |ui| {
                            if let Some(chosen) =
                                show_queue(ui, &self.queue, Path::new(&self.path), dark)
                            {
                                action = Some(chosen);
                            }
                        });
                    }
                });
            });
        if let Some(action) = action {
            self.apply_action(action, ui.ctx());
        }
    }

    fn show_fullscreen(&mut self, ui: &mut egui::Ui) {
        let theme = crate::ui_kit::theme(true, crate::Workspace::Reader);
        let mut volume = self.volume();
        let kind = self.queue.kind;
        let filename = Path::new(&self.path)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Media")
            .to_owned();
        let current = self.queue.current(Path::new(&self.path));
        let title = current.map_or(filename, |item| item.title.clone());
        let credit = current.map_or(String::new(), |item| match (&item.artist, &item.album) {
            (Some(artist), Some(album)) => format!("{artist}  ·  {album}"),
            (Some(artist), None) => artist.clone(),
            (None, Some(album)) => album.clone(),
            _ => String::new(),
        });
        let status = self.player.status(VOICE);
        let playing = status
            .as_ref()
            .is_some_and(|s| matches!(s.state, PlayState::Playing | PlayState::Loading));
        let position = status.as_ref().map_or(self.seek_to, |s| s.position);
        let duration = match &self.state {
            State::Ready(loaded) => loaded.info.duration(),
            _ => 0.0,
        };
        let mut action = None;
        let keys = ui.ctx().input(|input| {
            (
                input.key_pressed(egui::Key::Escape),
                input.key_pressed(egui::Key::Space),
                input.key_pressed(egui::Key::ArrowLeft),
                input.key_pressed(egui::Key::ArrowRight),
            )
        });
        if keys.0 {
            action = Some(Action::Fullscreen(false));
        } else if keys.1 {
            action = Some(if playing { Action::Pause } else { Action::Play });
        } else if keys.2 {
            action = Some(Action::Seek((position - 5.0).max(0.0)));
        } else if keys.3 {
            action = Some(Action::Seek((position + 5.0).min(duration)));
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(Color32::BLACK)
                    .inner_margin(egui::Margin::same(0)),
            )
            .show(ui, |ui| {
                *ui.visuals_mut() = egui::Visuals::dark();
                ui.visuals_mut().override_text_color = Some(Color32::WHITE);
                let width = ui.available_width().max(320.0);
                let narrow_controls = width < 900.0;
                let height = (ui.available_height() - if narrow_controls { 142.0 } else { 100.0 })
                    .max(180.0);
                let (stage, response) =
                    ui.allocate_exact_size(vec2(width, height), Sense::click_and_drag());
                let response = response.on_hover_text(if kind == Kind::Video {
                    "Pinch to zoom · Drag to pan · Click to play or pause"
                } else {
                    "Click to play or pause"
                });
                if response.clicked() {
                    action = Some(if playing { Action::Pause } else { Action::Play });
                }
                if kind == Kind::Video {
                    let (pinch, pointer) =
                        ui.input(|input| (input.zoom_delta(), input.pointer.hover_pos()));
                    if let Some(pointer) = pointer.filter(|point| stage.contains(*point))
                        && (pinch - 1.0).abs() > 0.001
                    {
                        apply_anchored_zoom(
                            &mut self.video_zoom,
                            &mut self.video_pan,
                            stage,
                            pointer,
                            pinch,
                        );
                    }
                    if response.dragged() && self.video_zoom > 1.0 {
                        self.video_pan += response.drag_delta();
                    }
                }
                ui.painter().rect_filled(
                    stage,
                    0.0,
                    if kind == Kind::Video {
                        Color32::BLACK
                    } else {
                        Color32::from_rgb(12, 27, 36)
                    },
                );
                match &mut self.state {
                    State::Ready(loaded) => {
                        if kind == Kind::Video {
                            render_video_frame(
                                loaded,
                                ui,
                                stage,
                                position,
                                self.video_zoom,
                                &mut self.video_pan,
                            );
                        } else {
                            paint_audio_art(
                                ui.painter(),
                                stage,
                                &title,
                                &credit,
                                &loaded.info,
                                false,
                            );
                        }
                        if !playing && kind == Kind::Video {
                            ui.painter().circle_filled(
                                stage.center(),
                                38.0,
                                Color32::from_rgba_unmultiplied(28, 42, 50, 210),
                            );
                            draw_play_icon(ui.painter(), stage.center(), false, Color32::WHITE);
                        }
                    }
                    State::Loading(_) => {
                        ui.painter().text(
                            stage.center(),
                            egui::Align2::CENTER_CENTER,
                            if kind == Kind::Video {
                                "Opening video…"
                            } else {
                                "Opening music…"
                            },
                            egui::FontId::proportional(18.0),
                            Color32::WHITE,
                        );
                    }
                    State::Error(error) => {
                        ui.painter().text(
                            stage.center(),
                            egui::Align2::CENTER_CENTER,
                            shorten(error, 70),
                            egui::FontId::proportional(16.0),
                            Color32::from_rgb(255, 180, 170),
                        );
                    }
                };
                let (track_rect, track_response) =
                    ui.allocate_exact_size(vec2(width, 20.0), Sense::click_and_drag());
                let track = Rect::from_center_size(track_rect.center(), vec2(width - 38.0, 5.0));
                ui.painter()
                    .rect_filled(track, 3.0, Color32::from_rgb(73, 83, 91));
                let ratio = if duration > 0.0 {
                    (position / duration).clamp(0.0, 1.0) as f32
                } else {
                    0.0
                };
                if ratio > 0.0 {
                    ui.painter().rect_filled(
                        Rect::from_min_max(
                            track.min,
                            pos2(track.left() + track.width() * ratio, track.bottom()),
                        ),
                        3.0,
                        Color32::from_rgb(235, 101, 72),
                    );
                }
                if (track_response.clicked() || track_response.dragged())
                    && duration > 0.0
                    && let Some(pointer) = track_response.interact_pointer_pos()
                {
                    let ratio = ((pointer.x - track.left()) / track.width()).clamp(0.0, 1.0);
                    action = Some(Action::Seek(ratio as f64 * duration));
                }
                ui.add_space(4.0);
                if narrow_controls {
                    ui.horizontal(|ui| {
                        ui.add_space(((width - 238.0) * 0.5).max(0.0));
                        show_transport(ui, &theme, playing, position, duration, &mut action);
                    });
                    ui.horizontal(|ui| {
                        show_fullscreen_time(ui, position, duration);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            show_fullscreen_volume(
                                ui,
                                &theme,
                                kind == Kind::Video && self.video_zoom > 1.01,
                                &mut volume,
                                self.speed,
                                &mut action,
                            );
                        });
                    });
                } else {
                    ui.columns(3, |columns| {
                        show_fullscreen_time(&mut columns[0], position, duration);
                        columns[1].horizontal(|ui| {
                            ui.add_space(((ui.available_width() - 238.0) * 0.5).max(0.0));
                            show_transport(ui, &theme, playing, position, duration, &mut action);
                        });
                        columns[2].with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                show_fullscreen_volume(
                                    ui,
                                    &theme,
                                    kind == Kind::Video && self.video_zoom > 1.01,
                                    &mut volume,
                                    self.speed,
                                    &mut action,
                                );
                            },
                        );
                    });
                }
            });
        if let Some(action) = action {
            self.apply_action(action, ui.ctx());
        }
    }

    fn apply_action(&mut self, action: Action, ctx: &egui::Context) {
        let result = match action {
            Action::Play => self.play(),
            Action::Pause => {
                self.pause();
                Ok(())
            }
            Action::Seek(seconds) => self.seek(seconds),
            Action::Previous => self.adjacent(-1),
            Action::Next => self.adjacent(1),
            Action::Fullscreen(enabled) => self.set_fullscreen(ctx, enabled),
            Action::Sort(sort) => {
                self.queue.set_sort(sort);
                Ok(())
            }
            Action::Select(path) => self.select(path, true),
            Action::Queue(open) => {
                self.queue_open = open;
                Ok(())
            }
            Action::Volume(volume) => self.set_volume(volume),
            Action::Speed(speed) => self.set_speed(speed),
            Action::FitVideo => {
                self.video_zoom = 1.0;
                self.video_pan = Vec2::ZERO;
                Ok(())
            }
        };
        self.notice = result.err();
        ctx.request_repaint();
    }
}

fn show_transport(
    ui: &mut egui::Ui,
    theme: &egui_shadcn::Theme,
    playing: bool,
    position: f64,
    duration: f64,
    action: &mut Option<Action>,
) {
    let previous_icon = |p: &egui::Painter, center, _, color| {
        draw_skip_icon(p, center, -1, color);
    };
    let back_icon = |p: &egui::Painter, center, _, color| {
        draw_jump_icon(p, center, -1, color);
    };
    let play_icon = |p: &egui::Painter, center, _, color| {
        draw_play_icon(p, center, playing, color);
    };
    let forward_icon = |p: &egui::Painter, center, _, color| {
        draw_jump_icon(p, center, 1, color);
    };
    let next_icon = |p: &egui::Painter, center, _, color| {
        draw_skip_icon(p, center, 1, color);
    };
    if Button::new("Previous file")
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::IconSm)
        .radius(ButtonRadius::Full)
        .icon(&previous_icon)
        .show(ui, theme)
        .on_hover_text("Previous file")
        .clicked()
    {
        *action = Some(Action::Previous);
    }
    if Button::new("Back 30 seconds")
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::IconSm)
        .radius(ButtonRadius::Full)
        .icon(&back_icon)
        .show(ui, theme)
        .on_hover_text("Back 30 seconds")
        .clicked()
    {
        *action = Some(Action::Seek((position - 30.0).max(0.0)));
    }
    if Button::new(if playing { "Pause" } else { "Play" })
        .variant(ButtonVariant::Solid)
        .size(ButtonSize::IconLg)
        .radius(ButtonRadius::Full)
        .accent_color(Color32::from_rgb(235, 101, 72))
        .icon(&play_icon)
        .show(ui, theme)
        .on_hover_text(if playing {
            "Pause · Space"
        } else {
            "Play · Space"
        })
        .clicked()
    {
        *action = Some(if playing { Action::Pause } else { Action::Play });
    }
    if Button::new("Forward 30 seconds")
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::IconSm)
        .radius(ButtonRadius::Full)
        .icon(&forward_icon)
        .show(ui, theme)
        .on_hover_text("Forward 30 seconds")
        .clicked()
    {
        *action = Some(Action::Seek((position + 30.0).min(duration)));
    }
    if Button::new("Next file")
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::IconSm)
        .radius(ButtonRadius::Full)
        .icon(&next_icon)
        .show(ui, theme)
        .on_hover_text("Next file")
        .clicked()
    {
        *action = Some(Action::Next);
    }
}

fn show_fullscreen_time(ui: &mut egui::Ui, position: f64, duration: f64) {
    ui.label(
        egui::RichText::new(format!("{} / {}", timestamp(position), timestamp(duration)))
            .monospace()
            .size(12.0)
            .color(Color32::from_rgb(210, 220, 225)),
    );
}

fn show_fullscreen_volume(
    ui: &mut egui::Ui,
    theme: &egui_shadcn::Theme,
    show_fit: bool,
    volume: &mut f32,
    speed: f64,
    action: &mut Option<Action>,
) {
    let exit_icon = |p: &egui::Painter, center, _, color| {
        draw_fullscreen_exit_icon(p, center, color);
    };
    if Button::new("Exit full screen")
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::IconSm)
        .radius(ButtonRadius::Medium)
        .icon(&exit_icon)
        .show(ui, theme)
        .on_hover_text("Exit full screen · Esc")
        .clicked()
    {
        *action = Some(Action::Fullscreen(false));
    }
    if show_fit {
        let fit_icon = |p: &egui::Painter, center, _, color| {
            draw_fit_icon(p, center, color);
        };
        if Button::new("Fit video to window")
            .variant(ButtonVariant::Ghost)
            .size(ButtonSize::IconSm)
            .radius(ButtonRadius::Medium)
            .icon(&fit_icon)
            .show(ui, theme)
            .on_hover_text("Fit video to window")
            .clicked()
        {
            *action = Some(Action::FitVideo);
        }
    }
    show_speed(ui, speed, action);
    if ui
        .add_sized(
            [92.0, 20.0],
            egui::Slider::new(volume, 0.0..=1.0).show_value(false),
        )
        .on_hover_text("Volume")
        .changed()
    {
        *action = Some(Action::Volume(*volume));
    }
    ui.label(
        egui::RichText::new("VOL")
            .size(10.0)
            .strong()
            .color(Color32::from_rgb(174, 190, 198)),
    );
}

fn show_speed(ui: &mut egui::Ui, speed: f64, action: &mut Option<Action>) {
    egui::ComboBox::from_id_salt("reader-playback-speed")
        .selected_text(format!("{speed}×"))
        .width(60.0)
        .show_ui(ui, |ui| {
            for preset in [0.5_f64, 0.75, 1.0, 1.25, 1.5, 2.0] {
                if ui
                    .selectable_label((speed - preset).abs() < 0.001, format!("{preset}×"))
                    .clicked()
                {
                    *action = Some(Action::Speed(preset));
                }
            }
        });
}

fn apply_anchored_zoom(
    zoom: &mut f32,
    pan: &mut Vec2,
    stage: Rect,
    pointer: egui::Pos2,
    delta: f32,
) {
    let next = (*zoom * delta).clamp(1.0, 5.0);
    let scale = next / *zoom;
    let anchor = pointer - stage.center();
    *pan = anchor - (anchor - *pan) * scale;
    *zoom = next;
}

fn render_video_frame(
    loaded: &mut Loaded,
    ui: &egui::Ui,
    stage: Rect,
    position: f64,
    zoom: f32,
    pan: &mut Vec2,
) {
    if let Some(feed) = &mut loaded.feed
        && let Some(frame) = feed.frame_at(position)
        && loaded.frame_time != Some(frame.time)
    {
        let image = ColorImage::from_rgba_unmultiplied(
            [frame.width as usize, frame.height as usize],
            &frame.rgba,
        );
        if let Some(texture) = &mut loaded.texture {
            texture.set(image, TextureOptions::LINEAR);
        } else {
            loaded.texture = Some(ui.ctx().load_texture(
                "polymath-media-frame",
                image,
                TextureOptions::LINEAR,
            ));
        }
        loaded.frame_time = Some(frame.time);
    }
    if let Some(texture) = &loaded.texture {
        let size = texture.size_vec2();
        if size.x > 0.0 && size.y > 0.0 {
            let scale = (stage.width() / size.x).min(stage.height() / size.y);
            let display = size * scale * zoom;
            let limit = vec2(
                ((display.x - stage.width()) * 0.5).max(0.0),
                ((display.y - stage.height()) * 0.5).max(0.0),
            );
            pan.x = pan.x.clamp(-limit.x, limit.x);
            pan.y = pan.y.clamp(-limit.y, limit.y);
            let fitted = Rect::from_center_size(stage.center() + *pan, display);
            ui.painter().with_clip_rect(stage).image(
                texture.id(),
                fitted,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }
}

fn show_queue(ui: &mut egui::Ui, queue: &Queue, current: &Path, dark: bool) -> Option<Action> {
    let theme = crate::ui_kit::theme(dark, crate::Workspace::Reader);
    let ink = if dark {
        Color32::from_rgb(235, 242, 246)
    } else {
        Color32::from_rgb(24, 42, 52)
    };
    let muted = if dark {
        Color32::from_rgb(153, 171, 181)
    } else {
        Color32::from_rgb(91, 110, 119)
    };
    let accent = if dark {
        Color32::from_rgb(142, 203, 226)
    } else {
        Color32::from_rgb(40, 105, 134)
    };
    let mut action = None;
    ui.add_space(12.0);
    ui.label(
        egui::RichText::new("UP NEXT")
            .size(11.0)
            .strong()
            .color(accent),
    );
    ui.add_space(5.0);
    ui.label(
        egui::RichText::new(&queue.folder)
            .size(21.0)
            .strong()
            .color(ink),
    );
    let count = queue.items.len();
    ui.label(
        egui::RichText::new(if queue.ready {
            format!("{count} files · plays in order")
        } else {
            "Finding files and reading music tags…".into()
        })
        .size(12.0)
        .color(muted),
    );
    ui.add_space(14.0);
    if queue.kind == Kind::Audio {
        ui.label(
            egui::RichText::new("SORT BY")
                .size(10.0)
                .strong()
                .color(muted),
        );
        ui.horizontal_wrapped(|ui| {
            for sort in [Sort::Name, Sort::Artist, Sort::Album] {
                if Button::new(sort.label())
                    .variant(if queue.sort == sort {
                        ButtonVariant::Soft
                    } else {
                        ButtonVariant::Ghost
                    })
                    .size(ButtonSize::Sm)
                    .radius(ButtonRadius::Full)
                    .show(ui, &theme)
                    .clicked()
                {
                    action = Some(Action::Sort(sort));
                }
            }
        });
        ui.add_space(10.0);
    }
    ui.separator();
    let height = ui.available_height().max(120.0);
    TableBuilder::new(ui)
        .id_salt("media-queue")
        .auto_shrink([false, false])
        .max_scroll_height(height)
        .column(Column::remainder().at_least(180.0))
        .body(|body| {
            body.rows(62.0, queue.items.len(), |mut row| {
                let index = row.index();
                row.col(|ui| {
                    let width = ui.available_width();
                    let Some(item) = queue.items.get(index) else {
                        return;
                    };
                    let selected = item.path == current;
                    let (rect, response) =
                        ui.allocate_exact_size(vec2(width, 62.0), Sense::click());
                    let response = response.on_hover_text(item.path.display().to_string());
                    let fill = if selected {
                        if dark {
                            Color32::from_rgb(44, 72, 84)
                        } else {
                            Color32::from_rgb(207, 228, 237)
                        }
                    } else if response.hovered() {
                        if dark {
                            Color32::from_rgb(37, 49, 55)
                        } else {
                            Color32::from_rgb(220, 232, 238)
                        }
                    } else {
                        Color32::TRANSPARENT
                    };
                    ui.painter()
                        .rect_filled(rect.shrink2(vec2(0.0, 3.0)), 10.0, fill);
                    if selected {
                        ui.painter().rect_filled(
                            Rect::from_min_size(
                                pos2(rect.left(), rect.top() + 12.0),
                                vec2(3.0, 38.0),
                            ),
                            2.0,
                            accent,
                        );
                    }
                    ui.painter().text(
                        pos2(rect.left() + 12.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        format!("{:02}", index + 1),
                        egui::FontId::monospace(12.0),
                        if selected { accent } else { muted },
                    );
                    let text_x = rect.left() + 49.0;
                    ui.painter().text(
                        pos2(text_x, rect.top() + 15.0),
                        egui::Align2::LEFT_TOP,
                        shorten(&item.title, ((width - 58.0) / 8.5) as usize),
                        egui::FontId::proportional(14.0),
                        ink,
                    );
                    let detail = match (&item.artist, &item.album) {
                        (Some(artist), Some(album)) => format!("{artist}  ·  {album}"),
                        (Some(artist), None) => artist.clone(),
                        (None, Some(album)) => album.clone(),
                        _ => item
                            .path
                            .extension()
                            .and_then(|value| value.to_str())
                            .unwrap_or("Local file")
                            .to_uppercase(),
                    };
                    ui.painter().text(
                        pos2(text_x, rect.top() + 36.0),
                        egui::Align2::LEFT_TOP,
                        shorten(&detail, ((width - 58.0) / 9.0) as usize),
                        egui::FontId::proportional(11.0),
                        muted,
                    );
                    if response.clicked() && !selected {
                        action = Some(Action::Select(item.path.clone()));
                    }
                });
            });
        });
    action
}

fn shorten(value: &str, max_chars: usize) -> String {
    if max_chars < 6 {
        return value.chars().take(max_chars).collect();
    }
    let count = value.chars().count();
    if count <= max_chars {
        return value.to_owned();
    }
    format!("{}…", value.chars().take(max_chars - 1).collect::<String>())
}

fn draw_play_icon(painter: &egui::Painter, center: egui::Pos2, paused: bool, color: Color32) {
    if paused {
        painter.rect_filled(
            Rect::from_center_size(center + vec2(-5.0, 0.0), vec2(4.0, 18.0)),
            1.0,
            color,
        );
        painter.rect_filled(
            Rect::from_center_size(center + vec2(5.0, 0.0), vec2(4.0, 18.0)),
            1.0,
            color,
        );
    } else {
        painter.add(egui::Shape::convex_polygon(
            vec![
                center + vec2(-5.0, -11.0),
                center + vec2(11.0, 0.0),
                center + vec2(-5.0, 11.0),
            ],
            color,
            Stroke::NONE,
        ));
    }
}

fn draw_skip_icon(painter: &egui::Painter, center: egui::Pos2, direction: i8, color: Color32) {
    let sign = if direction < 0 { -1.0 } else { 1.0 };
    let edge = center.x + sign * 9.0;
    painter.line_segment(
        [pos2(edge, center.y - 9.0), pos2(edge, center.y + 9.0)],
        Stroke::new(2.0, color),
    );
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos2(center.x - sign * 8.0, center.y - 8.0),
            pos2(center.x - sign * 8.0, center.y + 8.0),
            pos2(center.x + sign * 6.0, center.y),
        ],
        color,
        Stroke::NONE,
    ));
}

fn draw_jump_icon(painter: &egui::Painter, center: egui::Pos2, direction: i8, color: Color32) {
    painter.circle_stroke(center, 10.0, Stroke::new(1.6, color));
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        "30",
        egui::FontId::proportional(8.0),
        color,
    );
    let sign = if direction < 0 { -1.0 } else { 1.0 };
    let tip = center + vec2(sign * 9.0, -9.0);
    painter.add(egui::Shape::convex_polygon(
        vec![
            tip,
            tip + vec2(-sign * 6.0, -1.0),
            tip + vec2(-sign * 1.0, 6.0),
        ],
        color,
        Stroke::NONE,
    ));
}

fn draw_fullscreen_exit_icon(painter: &egui::Painter, center: egui::Pos2, color: Color32) {
    let stroke = Stroke::new(1.8, color);
    for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let inner = center + vec2(x * 3.0, y * 3.0);
        painter.line_segment([inner, inner + vec2(x * 7.0, 0.0)], stroke);
        painter.line_segment([inner, inner + vec2(0.0, y * 7.0)], stroke);
    }
}

fn draw_fullscreen_enter_icon(painter: &egui::Painter, center: egui::Pos2, color: Color32) {
    let stroke = Stroke::new(1.8, color);
    for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let corner = center + vec2(x * 10.0, y * 10.0);
        painter.line_segment([corner, corner - vec2(x * 7.0, 0.0)], stroke);
        painter.line_segment([corner, corner - vec2(0.0, y * 7.0)], stroke);
    }
}

fn draw_fit_icon(painter: &egui::Painter, center: egui::Pos2, color: Color32) {
    let stroke = Stroke::new(1.8, color);
    painter.rect_stroke(
        Rect::from_center_size(center, vec2(19.0, 14.0)),
        2.0,
        stroke,
        egui::StrokeKind::Inside,
    );
    painter.line_segment([center + vec2(-4.0, 0.0), center + vec2(4.0, 0.0)], stroke);
}

fn paint_audio_art(
    painter: &egui::Painter,
    stage: Rect,
    title: &str,
    credit: &str,
    info: &MediaInfo,
    compact: bool,
) {
    let painter = painter.with_clip_rect(stage);
    let immersive = stage.height() > 520.0;
    let art_size = if immersive {
        300.0
    } else {
        let text_height = if compact { 74.0 } else { 95.0 };
        (stage.height() - text_height - 28.0).clamp(120.0, if compact { 174.0 } else { 244.0 })
    };
    let text_height = if compact { 74.0 } else { 95.0 };
    let art_top = stage.top() + ((stage.height() - art_size - text_height) * 0.5).max(10.0);
    let art = Rect::from_center_size(
        pos2(
            stage.center().x,
            if immersive {
                stage.center().y - 55.0
            } else {
                art_top + art_size * 0.5
            },
        ),
        vec2(art_size, art_size),
    );
    painter.rect_filled(
        art.translate(vec2(0.0, 8.0)),
        14.0,
        Color32::from_rgb(8, 16, 22),
    );
    painter.rect_filled(art, 14.0, Color32::from_rgb(25, 91, 118));
    let center = art.center();
    painter.circle_filled(center, art_size * 0.39, Color32::from_rgb(11, 37, 52));
    for radius in [0.27, 0.31, 0.35] {
        painter.circle_stroke(
            center,
            art_size * radius,
            Stroke::new(1.0, Color32::from_rgb(76, 137, 158)),
        );
    }
    painter.circle_filled(center, art_size * 0.105, Color32::from_rgb(235, 101, 72));
    painter.circle_filled(center, art_size * 0.022, Color32::from_rgb(18, 40, 51));
    let text_top = art.bottom() + if immersive { 23.0 } else { 12.0 };
    painter.text(
        pos2(stage.center().x, text_top),
        egui::Align2::CENTER_TOP,
        "LOCAL MUSIC",
        egui::FontId::proportional(11.0),
        Color32::from_rgb(138, 205, 222),
    );
    let remaining = stage.width() - 48.0;
    painter.text(
        pos2(
            stage.center().x,
            text_top + if immersive { 23.0 } else { 18.0 },
        ),
        egui::Align2::CENTER_TOP,
        shorten(
            title,
            (remaining / if compact { 16.0 } else { 20.0 }) as usize,
        ),
        egui::FontId::proportional(if compact { 19.0 } else { 24.0 }),
        Color32::WHITE,
    );
    if !credit.is_empty() {
        painter.text(
            pos2(
                stage.center().x,
                text_top + if immersive { 58.0 } else { 48.0 },
            ),
            egui::Align2::CENTER_TOP,
            shorten(credit, (remaining / 11.0) as usize),
            egui::FontId::proportional(13.0),
            Color32::from_rgb(181, 204, 214),
        );
    }
    if !compact {
        let codec = info
            .audio
            .as_ref()
            .map_or(info.container, |a| a.codec.as_str());
        painter.text(
            pos2(
                stage.center().x,
                text_top
                    + if immersive {
                        if credit.is_empty() { 60.0 } else { 82.0 }
                    } else if credit.is_empty() {
                        50.0
                    } else {
                        70.0
                    },
            ),
            egui::Align2::CENTER_TOP,
            format!("{}  ·  {}", codec, timestamp(info.duration())),
            egui::FontId::proportional(12.0),
            Color32::from_rgb(139, 173, 188),
        );
    }
}

fn timestamp(seconds: f64) -> String {
    let total = seconds.max(0.0) as u64;
    format!("{:02}:{:02}", total / 60, total % 60)
}
