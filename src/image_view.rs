//! Read-only local image viewing; editing is an explicit handoff to Lightcraft.
use egui::{Color32, ColorImage, Rect, Sense, TextureHandle, TextureOptions, Vec2, pos2, vec2};
use egui_shadcn::{Button, ButtonRadius, ButtonSize, ButtonVariant};
use std::{
    path::Path,
    sync::mpsc::{self, Receiver},
    time::Duration,
};

const MAX_IMAGE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_PREVIEW_EDGE: u32 = 2400;

struct DecodedImage {
    rgba: Vec<u8>,
    width: usize,
    height: usize,
    original_width: u32,
    original_height: u32,
}

struct ReadyImage {
    texture: TextureHandle,
    width: usize,
    height: usize,
    original_width: u32,
    original_height: u32,
}

enum State {
    Loading(Receiver<Result<DecodedImage, String>>),
    Ready(ReadyImage),
    Error(String),
}

pub struct ImageView {
    pub path: String,
    pub fullscreen: bool,
    state: State,
    zoom: f32,
    pan: Vec2,
}

impl ImageView {
    pub fn open(path: &Path) -> Result<Self, String> {
        let metadata = std::fs::metadata(path).map_err(|e| format!("Could not open image: {e}"))?;
        if !metadata.is_file() {
            return Err("Choose an image file.".into());
        }
        if metadata.len() > MAX_IMAGE_BYTES {
            return Err(
                "This image is over the 256 MB viewer limit. Open it in Images to edit.".into(),
            );
        }
        let file = path.to_path_buf();
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new().name("polymath-image-preview".into()).spawn(move || {
            let result = std::fs::read(&file).map_err(|e| format!("Could not read image: {e}"))
                .and_then(|bytes| {
                    let preview = if lightcraft_codecs::sniff(&bytes).is_some_and(|f| f.is_raw()) {
                        Some(lightcraft_raw::embedded_preview_covering(&bytes, MAX_PREVIEW_EDGE)
                            .ok_or_else(|| "This camera RAW has no embedded preview. Open it in Images to edit.".to_owned())?)
                    } else { None };
                    let image_bytes = preview.as_deref().unwrap_or(&bytes);
                    let decoded = lightcraft_codecs::decode_thumbnail(image_bytes, MAX_PREVIEW_EDGE)
                        .map_err(|e| format!("Could not display image: {e}"))?;
                    let orientation = lightcraft_geom::Orientation::from_exif(decoded.orientation);
                    let (original_width, original_height) = if orientation.swaps_axes() {
                        (decoded.source_height, decoded.source_width)
                    } else { (decoded.source_width, decoded.source_height) };
                    let upright = decoded.image.into_oriented(orientation);
                    let rgba = upright.data.into_iter().flat_map(|pixel| pixel.into_iter()).collect();
                    Ok(DecodedImage { rgba, width: upright.width, height: upright.height, original_width, original_height })
                });
            let _ = tx.send(result);
        }).map_err(|e| format!("Could not start image viewer: {e}"))?;
        Ok(Self {
            path: path.to_string_lossy().into_owned(),
            fullscreen: false,
            state: State::Loading(rx),
            zoom: 1.0,
            pan: Vec2::ZERO,
        })
    }

    pub fn inspect(&self) -> serde_json::Value {
        match &self.state {
            State::Loading(_) => {
                serde_json::json!({"kind":"imagePreview","path":self.path,"state":"loading","readOnly":true,"fullscreen":self.fullscreen})
            }
            State::Error(error) => {
                serde_json::json!({"kind":"imagePreview","path":self.path,"state":"error","error":error,"readOnly":true,"fullscreen":self.fullscreen})
            }
            State::Ready(image) => {
                serde_json::json!({"kind":"imagePreview","path":self.path,"state":"ready","width":image.original_width,"height":image.original_height,"readOnly":true,"fullscreen":self.fullscreen})
            }
        }
    }

    pub fn set_fullscreen(&mut self, ctx: &egui::Context, enabled: bool) {
        self.fullscreen = enabled;
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(enabled));
        ctx.request_repaint();
    }

    pub fn tick(&mut self, ctx: &egui::Context) {
        if let State::Loading(rx) = &self.state {
            match rx.try_recv() {
                Ok(Ok(image)) => {
                    let pixels = ColorImage::from_rgba_unmultiplied(
                        [image.width, image.height],
                        &image.rgba,
                    );
                    let texture =
                        ctx.load_texture("polymath-image-view", pixels, TextureOptions::LINEAR);
                    self.state = State::Ready(ReadyImage {
                        texture,
                        width: image.width,
                        height: image.height,
                        original_width: image.original_width,
                        original_height: image.original_height,
                    });
                }
                Ok(Err(error)) => self.state = State::Error(error),
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.state = State::Error("Image viewer stopped unexpectedly.".into())
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(60))
                }
            }
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, dark: bool) {
        let fullscreen = self.fullscreen;
        let mut toggle_fullscreen = false;
        if fullscreen && ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            toggle_fullscreen = true;
        }
        let canvas = if fullscreen {
            Color32::from_rgb(13, 15, 18)
        } else if dark {
            Color32::from_rgb(22, 24, 27)
        } else {
            Color32::from_rgb(250, 250, 250)
        };
        let ink = if dark {
            Color32::from_rgb(239, 244, 246)
        } else {
            Color32::from_rgb(25, 27, 30)
        };
        let muted = if dark {
            Color32::from_rgb(178, 190, 196)
        } else {
            Color32::from_rgb(102, 108, 115)
        };
        let theme = crate::ui_kit::theme(dark || fullscreen, crate::Workspace::Reader);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(canvas).inner_margin(if fullscreen {
                egui::Margin::symmetric(12, 8)
            } else {
                egui::Margin::symmetric(24, 16)
            }))
            .show(ui, |ui| {
                ui.visuals_mut().override_text_color =
                    Some(if fullscreen { Color32::WHITE } else { ink });
                match &self.state {
                    State::Loading(_) => {
                        ui.centered_and_justified(|ui| {
                            ui.spinner();
                            ui.label("Opening image…");
                        });
                    }
                    State::Error(error) => {
                        ui.colored_label(
                            if dark {
                                Color32::from_rgb(255, 186, 169)
                            } else {
                                Color32::from_rgb(151, 49, 38)
                            },
                            error,
                        );
                    }
                    State::Ready(image) => {
                        // Leave room for the controls even when the window is narrow
                        // enough to wrap them onto two or three rows.
                        let controls_height = if ui.available_width() < 410.0 {
                            150.0
                        } else if ui.available_width() < 600.0 {
                            108.0
                        } else {
                            70.0
                        };
                        let viewport = vec2(
                            ui.available_width(),
                            (ui.available_height() - controls_height).max(80.0),
                        );
                        let (rect, response) =
                            ui.allocate_exact_size(viewport, Sense::click_and_drag());
                        let response = response.on_hover_text(
                            "Pinch to zoom · Drag or scroll to pan · Double-click for full screen",
                        );
                        let painter = ui.painter().with_clip_rect(rect);
                        painter.rect_filled(
                            rect,
                            if fullscreen { 0.0 } else { 12.0 },
                            if fullscreen {
                                Color32::BLACK
                            } else if dark {
                                Color32::from_rgb(13, 16, 19)
                            } else {
                                Color32::from_rgb(235, 237, 239)
                            },
                        );
                        if response.double_clicked() {
                            toggle_fullscreen = true;
                        }
                        let size = vec2(image.width.max(1) as f32, image.height.max(1) as f32);
                        let fit = ((rect.width() - 28.0).max(1.0) / size.x)
                            .min((rect.height() - 28.0).max(1.0) / size.y);
                        let (pinch, pointer, scroll) = ui.input(|input| {
                            (
                                input.zoom_delta(),
                                input.pointer.hover_pos(),
                                input.smooth_scroll_delta,
                            )
                        });
                        if let Some(pointer) = pointer.filter(|point| rect.contains(*point)) {
                            if (pinch - 1.0).abs() > 0.001 {
                                let next_zoom = (self.zoom * pinch).clamp(1.0, 8.0);
                                let scale = next_zoom / self.zoom;
                                let anchor = pointer - rect.center();
                                self.pan = anchor - (anchor - self.pan) * scale;
                                self.zoom = next_zoom;
                            } else if self.zoom > 1.0 {
                                self.pan += scroll;
                            }
                        }
                        let display = size * fit * self.zoom;
                        let limit = vec2(
                            ((display.x - rect.width()) * 0.5).max(0.0),
                            ((display.y - rect.height()) * 0.5).max(0.0),
                        );
                        if response.dragged() && self.zoom > 1.0 {
                            self.pan += response.drag_delta();
                        }
                        self.pan.x = self.pan.x.clamp(-limit.x, limit.x);
                        self.pan.y = self.pan.y.clamp(-limit.y, limit.y);
                        let fitted = Rect::from_center_size(rect.center() + self.pan, display);
                        painter.image(
                            image.texture.id(),
                            fitted,
                            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                        ui.add_space(9.0);
                        let strip = if fullscreen {
                            Color32::from_rgb(27, 29, 33)
                        } else if dark {
                            Color32::from_rgb(35, 38, 42)
                        } else {
                            Color32::from_rgb(242, 244, 246)
                        };
                        egui::Frame::new()
                            .fill(strip)
                            .corner_radius(10.0)
                            .inner_margin(egui::Margin::symmetric(12, 6))
                            .show(ui, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{} × {}",
                                            image.original_width, image.original_height
                                        ))
                                        .size(11.0)
                                        .color(
                                            if fullscreen {
                                                Color32::from_rgb(190, 194, 199)
                                            } else {
                                                muted
                                            },
                                        ),
                                    );
                                    ui.add_space(14.0);
                                    if Button::new("Fit to window")
                                        .variant(ButtonVariant::Outline)
                                        .size(ButtonSize::Sm)
                                        .radius(ButtonRadius::Medium)
                                        .show(ui, &theme)
                                        .clicked()
                                    {
                                        self.zoom = 1.0;
                                        self.pan = Vec2::ZERO;
                                    }
                                    ui.add_space(8.0);
                                    if Button::new("−")
                                        .variant(ButtonVariant::Outline)
                                        .size(ButtonSize::Sm)
                                        .radius(ButtonRadius::Medium)
                                        .show(ui, &theme)
                                        .on_hover_text("Zoom out")
                                        .clicked()
                                    {
                                        self.zoom = (self.zoom / 1.25).max(1.0);
                                    }
                                    ui.label(
                                        egui::RichText::new(format!("{:.2}×", self.zoom))
                                            .monospace()
                                            .color(if fullscreen {
                                                Color32::from_rgb(190, 194, 199)
                                            } else {
                                                muted
                                            }),
                                    )
                                    .on_hover_text("Zoom relative to Fit to window");
                                    if Button::new("+")
                                        .variant(ButtonVariant::Outline)
                                        .size(ButtonSize::Sm)
                                        .radius(ButtonRadius::Medium)
                                        .show(ui, &theme)
                                        .on_hover_text("Zoom in")
                                        .clicked()
                                    {
                                        self.zoom = (self.zoom * 1.25).min(8.0);
                                    }
                                    ui.add_space(14.0);
                                    if Button::new(if fullscreen {
                                        "Exit full screen"
                                    } else {
                                        "Full screen"
                                    })
                                    .variant(ButtonVariant::Outline)
                                    .size(ButtonSize::Sm)
                                    .radius(ButtonRadius::Medium)
                                    .show(ui, &theme)
                                    .on_hover_text(if fullscreen {
                                        "Exit full screen · Esc"
                                    } else {
                                        "View image full screen · double-click image"
                                    })
                                    .clicked()
                                    {
                                        toggle_fullscreen = true;
                                    }
                                })
                            });
                    }
                }
            });
        if toggle_fullscreen {
            self.set_fullscreen(ui.ctx(), !fullscreen);
        }
    }
}
