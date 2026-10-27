//! Read-only local image viewing; editing is an explicit handoff to Lightcraft.
use egui::{Color32, ColorImage, Rect, Sense, TextureHandle, TextureOptions, Vec2, pos2, vec2};
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
            state: State::Loading(rx),
            zoom: 1.0,
            pan: Vec2::ZERO,
        })
    }

    pub fn inspect(&self) -> serde_json::Value {
        match &self.state {
            State::Loading(_) => {
                serde_json::json!({"kind":"imagePreview","path":self.path,"state":"loading","readOnly":true})
            }
            State::Error(error) => {
                serde_json::json!({"kind":"imagePreview","path":self.path,"state":"error","error":error,"readOnly":true})
            }
            State::Ready(image) => {
                serde_json::json!({"kind":"imagePreview","path":self.path,"state":"ready","width":image.original_width,"height":image.original_height,"readOnly":true})
            }
        }
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
        let canvas = if dark {
            Color32::from_rgb(21, 26, 31)
        } else {
            Color32::from_rgb(244, 247, 249)
        };
        let ink = if dark {
            Color32::from_rgb(239, 244, 246)
        } else {
            Color32::from_rgb(28, 44, 54)
        };
        let muted = if dark {
            Color32::from_rgb(178, 190, 196)
        } else {
            Color32::from_rgb(83, 101, 112)
        };
        let accent = if dark {
            Color32::from_rgb(137, 199, 225)
        } else {
            Color32::from_rgb(36, 101, 132)
        };
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(canvas)
                    .inner_margin(egui::Margin::symmetric(24, 17)),
            )
            .show(ui, |ui| {
                ui.visuals_mut().override_text_color = Some(ink);
                let name = Path::new(&self.path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Image");
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Image viewer").size(12.0).color(accent));
                        ui.label(egui::RichText::new(name).size(23.0).strong().color(ink))
                            .on_hover_text(&self.path);
                    });
                    if let State::Ready(image) = &self.state {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} × {}",
                                    image.original_width, image.original_height
                                ))
                                .size(12.0)
                                .color(muted),
                            );
                        });
                    }
                });
                ui.add_space(12.0);
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
                        let viewport = vec2(
                            ui.available_width(),
                            (ui.available_height() - 70.0).max(220.0),
                        );
                        let (rect, response) = ui.allocate_exact_size(viewport, Sense::drag());
                        let painter = ui.painter().with_clip_rect(rect);
                        painter.rect_filled(
                            rect,
                            10.0,
                            if dark {
                                Color32::from_rgb(12, 16, 19)
                            } else {
                                Color32::from_rgb(226, 234, 238)
                            },
                        );
                        let size = vec2(image.width as f32, image.height as f32);
                        let fit = ((rect.width() - 28.0) / size.x)
                            .min((rect.height() - 28.0) / size.y)
                            .min(1.0);
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
                        ui.add_space(12.0);
                        ui.horizontal_centered(|ui| {
                            if ui.button("Fit").clicked() {
                                self.zoom = 1.0;
                                self.pan = Vec2::ZERO;
                            }
                            if ui.button("−").on_hover_text("Zoom out").clicked() {
                                self.zoom = (self.zoom / 1.25).max(1.0);
                            }
                            ui.label(
                                egui::RichText::new(if self.zoom == 1.0 {
                                    "Fit".to_owned()
                                } else {
                                    format!("{:.0}%", self.zoom * 100.0)
                                })
                                .color(muted),
                            );
                            if ui.button("+").on_hover_text("Zoom in").clicked() {
                                self.zoom = (self.zoom * 1.25).min(8.0);
                            }
                            ui.add_space(12.0);
                            ui.label(
                                egui::RichText::new("Drag to pan when zoomed")
                                    .size(11.0)
                                    .color(muted),
                            );
                        });
                    }
                }
            });
    }
}
