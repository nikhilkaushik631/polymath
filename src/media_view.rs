//! Local audio and video playback inside Reader. Decoding begins only after a file is opened.
use crate::audio::CpalOut;
use crate::media_queue::{Kind, Queue, Sort};
use deckcraft_media::{Bytes, ClipParams, MediaInfo, PlayState, Player, VideoFeed, VoiceSpec};
use egui::{Color32, ColorImage, Rect, Sense, Stroke, TextureHandle, TextureOptions, pos2, vec2};
use egui_extras::{Column, TableBuilder};
use egui_shadcn::{Button, ButtonRadius, ButtonSize, ButtonVariant};
use std::{
    path::Path,
    sync::{
        Arc,
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
    queue: Queue,
    seek_to: f64,
    auto_play_when_ready: bool,
    end_handled: bool,
    pub fullscreen: bool,
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
}

impl MediaView {
    pub fn open(path: &Path) -> Result<Self, String> {
        let state = Self::load_state(path)?;
        Ok(Self {
            path: path.to_string_lossy().into_owned(),
            state,
            player: Player::new(Some(Box::new(CpalOut::default()))),
            queue: Queue::new(path),
            seek_to: 0.0,
            auto_play_when_ready: false,
            end_handled: false,
            fullscreen: false,
            queue_open: true,
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
        self.player = Player::new(Some(Box::new(CpalOut::default())));
        self.path = path.to_string_lossy().into_owned();
        self.state = state;
        self.seek_to = 0.0;
        self.auto_play_when_ready = play;
        self.end_handled = false;
        self.notice = None;
        Ok(())
    }

    pub fn set_fullscreen(&mut self, ctx: &egui::Context, enabled: bool) -> Result<(), String> {
        if enabled && self.queue.kind != Kind::Video {
            return Err("Full screen is available for video files.".into());
        }
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
        let canvas = if dark {
            Color32::from_rgb(20, 25, 30)
        } else {
            Color32::from_rgb(244, 247, 249)
        };
        let ink = if dark {
            Color32::from_rgb(241, 245, 247)
        } else {
            Color32::from_rgb(25, 40, 48)
        };
        let muted = if dark {
            Color32::from_rgb(178, 191, 198)
        } else {
            Color32::from_rgb(80, 99, 110)
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
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(canvas)
                    .inner_margin(egui::Margin::symmetric(28, 18)),
            )
            .show(ui, |ui| {
                ui.visuals_mut().override_text_color = Some(ink);
                let theme = crate::ui_kit::theme(dark, crate::Workspace::Reader);
                if self.queue_open && ui.available_width() > 740.0 {
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
                                    Color32::from_rgb(26, 35, 41)
                                } else {
                                    Color32::from_rgb(232, 240, 244)
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
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if Button::new(if self.queue_open {
                            "Hide queue"
                        } else {
                            "Show queue"
                        })
                        .variant(ButtonVariant::Outline)
                        .size(ButtonSize::Sm)
                        .radius(ButtonRadius::Full)
                        .show(ui, &theme)
                        .clicked()
                        {
                            action = Some(Action::Queue(!self.queue_open));
                        }
                    });
                });
                // Keep the transport visible with the video at ordinary window heights.
                // ScrollArea's inner Ui can report an unbounded height, so measure here.
                let video_height_budget = (ui.available_height() - 310.0).max(230.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(22.0);
                    ui.vertical_centered(|ui| {
                        let width = ui.available_width().min(1060.0).max(280.0);
                        let (header, response) =
                            ui.allocate_exact_size(vec2(width, 84.0), Sense::hover());
                        response.on_hover_text(&self.path);
                        let painter = ui.painter().clone();
                        painter.text(
                            header.left_top(),
                            egui::Align2::LEFT_TOP,
                            "Now playing",
                            egui::FontId::proportional(13.0),
                            reader_blue,
                        );
                        painter.text(
                            header.left_top() + vec2(0.0, 24.0),
                            egui::Align2::LEFT_TOP,
                            shorten(&filename, ((width - 30.0) / 26.0) as usize),
                            egui::FontId::proportional(28.0),
                            ink,
                        );
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
                                let aspect = loaded
                                    .info
                                    .video
                                    .as_ref()
                                    .filter(|v| v.width > 0 && v.height > 0)
                                    .map_or(16.0 / 9.0, |v| v.width as f32 / v.height as f32);
                                let height = if video {
                                    (width / aspect)
                                        .clamp(230.0, 560.0)
                                        .min(video_height_budget)
                                } else if width < 620.0 {
                                    330.0
                                } else {
                                    268.0
                                };
                                let (stage, stage_response) =
                                    ui.allocate_exact_size(vec2(width, height), Sense::click());
                                let stage_response = stage_response.on_hover_text(if playing {
                                    "Pause · Space"
                                } else {
                                    "Play · Space"
                                });
                                if stage_response.clicked() {
                                    action =
                                        Some(if playing { Action::Pause } else { Action::Play });
                                }
                                let painter = ui.painter().clone();
                                painter.rect_filled(stage, 14.0, Color32::from_rgb(15, 28, 37));
                                if video {
                                    render_video_frame(loaded, ui, stage, position);
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
                                ui.add_space(17.0);
                                let (seek_rect, seek_response) = ui.allocate_exact_size(
                                    vec2(width, 30.0),
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
                                let play_icon = |p: &egui::Painter, center, _, color| {
                                    draw_play_icon(p, center, playing, color);
                                };
                                let previous_icon = |p: &egui::Painter, center, _, color| {
                                    draw_skip_icon(p, center, -1, color);
                                };
                                let next_icon = |p: &egui::Painter, center, _, color| {
                                    draw_skip_icon(p, center, 1, color);
                                };
                                ui.allocate_ui_with_layout(
                                    vec2(width, 64.0),
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        if Button::new("")
                                            .variant(ButtonVariant::Solid)
                                            .size(ButtonSize::IconLg)
                                            .radius(ButtonRadius::Full)
                                            .accent_color(play_color)
                                            .icon(&play_icon)
                                            .show(ui, &theme)
                                            .on_hover_text(if playing {
                                                "Pause · Space"
                                            } else {
                                                "Play · Space"
                                            })
                                            .clicked()
                                        {
                                            action = Some(if playing {
                                                Action::Pause
                                            } else {
                                                Action::Play
                                            });
                                        }
                                        if Button::new("")
                                            .variant(ButtonVariant::Ghost)
                                            .size(ButtonSize::IconLg)
                                            .radius(ButtonRadius::Full)
                                            .icon(&previous_icon)
                                            .show(ui, &theme)
                                            .on_hover_text("Previous file")
                                            .clicked()
                                        {
                                            action = Some(Action::Previous);
                                        }
                                        if Button::new("")
                                            .variant(ButtonVariant::Ghost)
                                            .size(ButtonSize::IconLg)
                                            .radius(ButtonRadius::Full)
                                            .icon(&next_icon)
                                            .show(ui, &theme)
                                            .on_hover_text("Next file")
                                            .clicked()
                                        {
                                            action = Some(Action::Next);
                                        }
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.add_space(12.0);
                                                ui.label(
                                                    egui::RichText::new(format!(
                                                        "{}  /  {}",
                                                        timestamp(position),
                                                        timestamp(duration)
                                                    ))
                                                    .font(egui::FontId::monospace(14.0))
                                                    .color(ink),
                                                );
                                            },
                                        );
                                    },
                                );
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
                                        format!("{} video  ·  {} × {}", v.codec, v.width, v.height)
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
                                if video
                                    && Button::new("Full screen")
                                        .variant(ButtonVariant::Outline)
                                        .size(ButtonSize::Sm)
                                        .radius(ButtonRadius::Full)
                                        .show(ui, &theme)
                                        .clicked()
                                {
                                    action = Some(Action::Fullscreen(true));
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
                });
            });
        if let Some(action) = action {
            self.apply_action(action, ui.ctx());
        }
    }

    fn show_fullscreen(&mut self, ui: &mut egui::Ui) {
        let theme = crate::ui_kit::theme(true, crate::Workspace::Reader);
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
                ui.visuals_mut().override_text_color = Some(Color32::WHITE);
                let width = ui.available_width().max(320.0);
                let height = (ui.available_height() - 105.0).max(220.0);
                let (stage, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
                if response.clicked() {
                    action = Some(if playing { Action::Pause } else { Action::Play });
                }
                ui.painter().rect_filled(stage, 0.0, Color32::BLACK);
                match &mut self.state {
                    State::Ready(loaded) => {
                        render_video_frame(loaded, ui, stage, position);
                        if !playing {
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
                            "Opening video…",
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
                    ui.allocate_exact_size(vec2(width, 23.0), Sense::click_and_drag());
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
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    for (label, desired) in [
                        ("Previous", Action::Previous),
                        (
                            if playing { "Pause" } else { "Play" },
                            if playing { Action::Pause } else { Action::Play },
                        ),
                        ("Next", Action::Next),
                    ] {
                        let button = Button::new(label)
                            .variant(if label == "Play" || label == "Pause" {
                                ButtonVariant::Solid
                            } else {
                                ButtonVariant::Outline
                            })
                            .size(ButtonSize::Sm)
                            .radius(ButtonRadius::Full);
                        let button = if label == "Play" || label == "Pause" {
                            button.accent_color(Color32::from_rgb(235, 101, 72))
                        } else {
                            button
                        };
                        if button.show(ui, &theme).clicked() {
                            action = Some(desired);
                        }
                    }
                    ui.add_space(14.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "{} / {}",
                            timestamp(position),
                            timestamp(duration)
                        ))
                        .monospace()
                        .color(Color32::from_rgb(210, 220, 225)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(16.0);
                        if Button::new("Exit full screen")
                            .variant(ButtonVariant::Outline)
                            .size(ButtonSize::Sm)
                            .radius(ButtonRadius::Full)
                            .show(ui, &theme)
                            .clicked()
                        {
                            action = Some(Action::Fullscreen(false));
                        }
                        ui.label(
                            egui::RichText::new(shorten(
                                Path::new(&self.path)
                                    .file_name()
                                    .and_then(|value| value.to_str())
                                    .unwrap_or("Video"),
                                45,
                            ))
                            .color(Color32::from_rgb(174, 190, 198)),
                        );
                    });
                });
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
        };
        self.notice = result.err();
        ctx.request_repaint();
    }
}

fn render_video_frame(loaded: &mut Loaded, ui: &egui::Ui, stage: Rect, position: f64) {
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
            let fitted = Rect::from_center_size(stage.center(), size * scale);
            ui.painter().image(
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

fn paint_audio_art(
    painter: &egui::Painter,
    stage: Rect,
    title: &str,
    credit: &str,
    info: &MediaInfo,
    compact: bool,
) {
    let art_size = if compact { 168.0 } else { 216.0 };
    let art = Rect::from_min_size(
        if compact {
            pos2(stage.center().x - art_size * 0.5, stage.top() + 18.0)
        } else {
            stage.left_top() + vec2(26.0, 26.0)
        },
        vec2(art_size, art_size),
    );
    painter.rect_filled(art, 11.0, Color32::from_rgb(25, 91, 118));
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
    let text_left = if compact {
        stage.left() + 24.0
    } else {
        art.right() + 34.0
    };
    let text_top = if compact {
        art.bottom() + 17.0
    } else {
        stage.top() + 63.0
    };
    painter.text(
        pos2(text_left, text_top),
        egui::Align2::LEFT_TOP,
        "LOCAL MUSIC",
        egui::FontId::proportional(14.0),
        Color32::from_rgb(138, 205, 222),
    );
    let remaining = if compact {
        stage.width() - 48.0
    } else {
        stage.right() - text_left - 24.0
    };
    painter.text(
        pos2(text_left, text_top + 31.0),
        egui::Align2::LEFT_TOP,
        shorten(
            title,
            (remaining / if compact { 20.0 } else { 26.0 }) as usize,
        ),
        egui::FontId::proportional(if compact { 20.0 } else { 26.0 }),
        Color32::WHITE,
    );
    if !credit.is_empty() {
        painter.text(
            pos2(text_left, text_top + 75.0),
            egui::Align2::LEFT_TOP,
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
                text_left,
                text_top + if credit.is_empty() { 75.0 } else { 101.0 },
            ),
            egui::Align2::LEFT_TOP,
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
