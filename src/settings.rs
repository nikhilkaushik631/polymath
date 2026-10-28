//! One place for Polymath-wide preferences and local access.
use crate::{Polymath, Workspace, voice::Voice};
use egui::{Align, Color32, Layout, RichText, vec2};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Page {
    #[default]
    Appearance,
    Voice,
    Files,
}

impl Page {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "appearance" => Some(Self::Appearance),
            "voice" => Some(Self::Voice),
            "files" => Some(Self::Files),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Voice => "Voice",
            Self::Files => "Files & privacy",
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Self::Appearance => "palette",
            Self::Voice => "mic",
            Self::Files => "folder",
        }
    }
}

impl Polymath {
    pub(crate) fn settings_window(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        if !self.voice.panel_open {
            return;
        }
        let mut open = self.voice.panel_open;
        let chrome = crate::ui_kit::chrome(self.dark);
        let muted = chrome.muted;
        let selected_fill = if self.dark {
            Color32::from_rgb(37, 62, 75)
        } else {
            Color32::from_rgb(225, 238, 245)
        };
        let selected_ink = if self.dark {
            Color32::from_rgb(154, 207, 231)
        } else {
            Color32::from_rgb(39, 91, 120)
        };
        egui::Window::new("Settings")
            .id(egui::Id::new("polymath-settings"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(vec2(700.0, 520.0))
            .min_width(650.0)
            .max_width(760.0)
            .max_height((ctx.content_rect().height() - 64.0).max(260.0))
            .frame(
                egui::Frame::window(&ctx.style_of(if self.dark {
                    egui::Theme::Dark
                } else {
                    egui::Theme::Light
                }))
                .fill(chrome.canvas)
                .stroke(egui::Stroke::new(1.0, chrome.line)),
            )
            .show(ctx, |ui| {
                // Editors install their own fonts and control styles. Settings
                // keeps shared typography without replacing the editor fonts.
                for font in ui.style_mut().text_styles.values_mut() {
                    font.family = egui::FontFamily::Proportional;
                }
                for (style, size) in [
                    (egui::TextStyle::Body, 13.0),
                    (egui::TextStyle::Button, 13.0),
                    (egui::TextStyle::Small, 11.0),
                ] {
                    ui.style_mut()
                        .text_styles
                        .insert(style, egui::FontId::proportional(size));
                }
                ui.visuals_mut().override_text_color = Some(chrome.ink);
                ui.visuals_mut().weak_text_color = Some(chrome.muted);
                ui.visuals_mut().selection.bg_fill = selected_fill;
                ui.visuals_mut().selection.stroke = egui::Stroke::new(1.0, selected_ink);
                ui.style_mut().spacing.button_padding = vec2(12.0, 8.0);
                ui.style_mut().spacing.item_spacing = vec2(8.0, 8.0);
                let body_height = ui.available_height().clamp(240.0, 430.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(158.0);
                        ui.set_min_height(body_height);
                        ui.add_space(8.0);
                        ui.label(RichText::new("Preferences").size(11.5).color(muted));
                        ui.add_space(12.0);
                        for page in [Page::Appearance, Page::Voice, Page::Files] {
                            let icon = pdfcraft_ui_egui::icons::image(
                                page.icon(),
                                16.0,
                                if self.settings_page == page {
                                    selected_ink
                                } else {
                                    ui.visuals().text_color()
                                },
                            );
                            if ui
                                .add(
                                    egui::Button::image_and_text(icon, page.label())
                                        .selected(self.settings_page == page)
                                        .fill(if self.settings_page == page {
                                            selected_fill
                                        } else {
                                            Color32::TRANSPARENT
                                        })
                                        .corner_radius(8)
                                        .min_size(vec2(145.0, 35.0)),
                                )
                                .clicked()
                            {
                                self.settings_page = page;
                            }
                            ui.add_space(3.0);
                        }
                        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                            ui.label(
                                RichText::new(concat!("Polymath ", env!("CARGO_PKG_VERSION")))
                                    .size(11.0)
                                    .color(muted),
                            );
                        });
                    });
                    ui.add(egui::Separator::default().vertical());
                    ui.vertical(|ui| {
                        // A bounded content column keeps ScrollArea and its cards
                        // from growing this dialog to the whole desktop width.
                        ui.set_width(470.0);
                        ui.set_max_width(470.0);
                        ui.set_min_height(body_height);
                        let content_top = ui.cursor().top();
                        if !self.voice.notice.is_empty() && self.settings_page != Page::Voice {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&self.voice.notice).size(11.0).color(muted));
                                if ui.small_button("Voice").clicked() {
                                    self.settings_page = Page::Voice;
                                }
                            });
                            ui.separator();
                        }
                        egui::ScrollArea::vertical()
                            .id_salt(("settings-content", self.settings_page.label()))
                            .max_height(
                                (body_height - (ui.cursor().top() - content_top)).max(100.0),
                            )
                            .auto_shrink([false, false])
                            .show(ui, |ui| match self.settings_page {
                                Page::Appearance => self.appearance_settings(ui, ctx, muted),
                                Page::Voice => self.voice_settings(ui, muted),
                                Page::Files => self.file_settings(ui, frame, ctx, muted),
                            });
                    });
                });
            });
        self.voice.panel_open = open;
    }

    fn appearance_settings(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, muted: Color32) {
        heading(
            ui,
            "Appearance",
            "Choose a theme and control interface motion.",
            muted,
        );
        ui.add_space(18.0);
        settings_card(ui, self.dark, |ui| {
            ui.label(RichText::new("Theme").size(15.0).strong());
            ui.add_space(4.0);
            ui.label(
                RichText::new("Applies to menus and tools. Document colors stay as authored.")
                    .size(12.0)
                    .color(muted),
            );
            ui.add_space(13.0);
            ui.horizontal(|ui| {
                for (dark, name, icon_name) in [(false, "Light", "sun"), (true, "Dark", "moon")] {
                    let chosen = self.dark == dark;
                    let fill = if chosen {
                        if self.dark {
                            Color32::from_rgb(41, 68, 82)
                        } else {
                            Color32::from_rgb(225, 238, 245)
                        }
                    } else {
                        ui.visuals().widgets.inactive.bg_fill
                    };
                    let icon_ink = if chosen {
                        Workspace::Reader.accent(self.dark)
                    } else {
                        ui.visuals().text_color()
                    };
                    let icon = pdfcraft_ui_egui::icons::image(icon_name, 19.0, icon_ink);
                    if ui
                        .add(
                            egui::Button::image_and_text(icon, name)
                                .selected(chosen)
                                .fill(fill)
                                .min_size(vec2(145.0, 44.0)),
                        )
                        .clicked()
                    {
                        self.set_dark(dark, ctx);
                    }
                }
            });
        });
        ui.add_space(12.0);
        settings_card(ui, self.dark, |ui| {
            ui.label(RichText::new("Motion").size(15.0).strong());
            ui.add_space(7.0);
            let reduce_motion = self.home_state.reduce_motion;
            let switch_theme = crate::ui_kit::theme(self.dark, Workspace::Reader);
            ui.horizontal(|ui| {
                egui_shadcn::switch_with_props(
                    ui,
                    &switch_theme,
                    egui_shadcn::SwitchProps::new(
                        egui::Id::new("polymath-reduce-motion"),
                        &mut self.home_state.reduce_motion,
                        "Reduce motion",
                    )
                    .animate(!reduce_motion)
                    .accent(Workspace::Reader.accent(self.dark)),
                );
            });
            ui.label(
                RichText::new("Turns off card transitions and animated focus scrolling.")
                    .size(12.0)
                    .color(muted),
            );
        });
    }

    fn voice_settings(&mut self, ui: &mut egui::Ui, muted: Color32) {
        heading(
            ui,
            "Voice",
            "Speak to write. Listen to a PDF with a voice on this Mac.",
            muted,
        );
        ui.add_space(16.0);
        settings_card(ui, self.dark, |ui| {
            ui.label(RichText::new("Dictation").size(15.0).strong());
            ui.label(
                RichText::new(
                    "Select a field, press the toolbar microphone, speak, then press it again.",
                )
                .size(12.0)
                .color(muted),
            );
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label("Language");
                egui::ComboBox::from_id_salt("polymath-dictation-language")
                    .selected_text(
                        Voice::languages()
                            .iter()
                            .find(|(_, code)| *code == self.voice.language)
                            .map(|(name, _)| *name)
                            .unwrap_or("Detect language"),
                    )
                    .show_ui(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(250.0)
                            .show(ui, |ui| {
                                for (name, code) in Voice::languages() {
                                    ui.selectable_value(
                                        &mut self.voice.language,
                                        (*code).into(),
                                        *name,
                                    );
                                }
                            });
                    });
            });
            if !self.voice.notice.is_empty() {
                ui.add_space(8.0);
                ui.label(RichText::new(&self.voice.notice).size(12.0));
            }
            if !self.voice.last_transcript.is_empty() {
                ui.add_space(10.0);
                ui.label(RichText::new("Last transcript").size(12.0).strong());
                ui.add(
                    egui::TextEdit::multiline(&mut self.voice.last_transcript)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                if egui_shadcn::Button::new("Copy transcript")
                    .variant(egui_shadcn::ButtonVariant::Outline)
                    .size(egui_shadcn::ButtonSize::Sm)
                    .show(ui, &crate::ui_kit::theme(self.dark, Workspace::Reader))
                    .clicked()
                {
                    ui.ctx().copy_text(self.voice.last_transcript.clone());
                }
            }
        });
        ui.add_space(12.0);
        settings_card(ui, self.dark, |ui| {
            ui.label(RichText::new("PDF read aloud").size(15.0).strong());
            let active_pdf = (self.active == Workspace::Tools
                || (self.active == Workspace::Reader
                    && self.source.is_none()
                    && self.media.is_none()
                    && self.image_preview.is_none()))
                && self.reader.active_ids().is_some();
            let page = self
                .reader
                .active
                .and_then(|index| self.reader.views.get(index))
                .map(|view| view.current + 1)
                .unwrap_or(1);
            ui.label(
                RichText::new(if active_pdf {
                    format!("Read from page {page} to the end of the open PDF.")
                } else {
                    "Open a PDF in Reader to use narration.".to_owned()
                })
                .size(12.0)
                .color(muted),
            );
            ui.add_space(9.0);
            ui.horizontal(|ui| {
                ui.label("Voice");
                egui::ComboBox::from_id_salt("polymath-narration-voice")
                    .selected_text(if self.voice.voice.is_empty() {
                        "No installed voices"
                    } else {
                        &self.voice.voice
                    })
                    .show_ui(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(250.0)
                            .show(ui, |ui| {
                                for (name, language) in &self.voice.voices {
                                    ui.selectable_value(
                                        &mut self.voice.voice,
                                        name.clone(),
                                        format!("{name} · {language}"),
                                    );
                                }
                            });
                    });
            });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        active_pdf && !self.voice.voice.is_empty(),
                        egui::Button::new("Read aloud"),
                    )
                    .clicked()
                    && let Some((index, id)) = self.reader.active_ids()
                    && let Some(doc) = self.reader.session.docs().iter().find(|doc| doc.id == id)
                {
                    let page = self.reader.views[index].current;
                    self.voice
                        .narration
                        .start(doc.export_source(), page, &self.voice.voice);
                }
                if ui
                    .add_enabled(self.voice.narration.busy(), egui::Button::new("Stop"))
                    .clicked()
                {
                    self.voice.narration.stop();
                }
            });
            if !self.voice.narration.status.is_empty() {
                ui.label(
                    RichText::new(&self.voice.narration.status)
                        .size(12.0)
                        .color(muted),
                );
            }
        });
        ui.add_space(12.0);
        ui.label(
            RichText::new("Dictation uses the bundled model. Narration uses installed macOS voices. Audio stays on this Mac.")
                .size(11.0)
                .color(muted),
        );
    }

    fn file_settings(
        &mut self,
        ui: &mut egui::Ui,
        frame: &eframe::Frame,
        ctx: &egui::Context,
        muted: Color32,
    ) {
        heading(
            ui,
            "Files & privacy",
            "Choose exactly which folders appear in your library.",
            muted,
        );
        ui.add_space(16.0);
        settings_card(ui, self.dark, |ui| {
            ui.label(RichText::new("Library folders").size(15.0).strong());
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if egui_shadcn::Button::new("Add folder…")
                    .variant(egui_shadcn::ButtonVariant::Solid)
                    .size(egui_shadcn::ButtonSize::Sm)
                    .show(ui, &crate::ui_kit::theme(self.dark, Workspace::Home))
                    .clicked()
                {
                    self.pick(true, frame, ctx);
                }
                if egui_shadcn::Button::new("Refresh")
                    .variant(egui_shadcn::ButtonVariant::Outline)
                    .size(egui_shadcn::ButtonSize::Sm)
                    .show(ui, &crate::ui_kit::theme(self.dark, Workspace::Home))
                    .clicked()
                {
                    self.listing.refresh();
                }
            });
            ui.add_space(10.0);
            if self.folders.is_empty() {
                ui.label(
                    RichText::new("No folders selected yet.")
                        .size(12.0)
                        .color(muted),
                );
            } else {
                let mut removed = None;
                for path in &self.folders {
                    ui.push_id(path, |ui| {
                        ui.horizontal(|ui| {
                            let label_width = (ui.available_width() - 82.0).max(100.0);
                            ui.allocate_ui_with_layout(
                                vec2(label_width, 40.0),
                                Layout::top_down(Align::LEFT),
                                |ui| {
                                    ui.set_width(label_width);
                                    let name = std::path::Path::new(path)
                                        .file_name()
                                        .and_then(|name| name.to_str())
                                        .unwrap_or(path);
                                    ui.add(
                                        egui::Label::new(RichText::new(name).size(12.5).strong())
                                            .truncate(),
                                    )
                                    .on_hover_text(path);
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(path).size(11.0).color(muted),
                                        )
                                        .truncate(),
                                    )
                                    .on_hover_text(path);
                                },
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui
                                    .small_button("Remove")
                                    .on_hover_text(format!(
                                        "Stop listing {path}. Its files are kept."
                                    ))
                                    .clicked()
                                {
                                    removed = Some(path.clone());
                                }
                            });
                        })
                    });
                    if path != self.folders.last().unwrap_or(path) {
                        ui.separator();
                    }
                }
                if let Some(path) = removed {
                    self.folders.retain(|folder| folder != &path);
                    self.listing.forget(&path);
                }
            }
        });
        ui.add_space(12.0);
        settings_card(ui, self.dark, |ui| {
            ui.label(RichText::new("Local file handling").size(15.0).strong());
            ui.label(
                RichText::new(
                    "Files open in Reader first. Use Edit to open the original in its editor.",
                )
                .size(12.0),
            );
            ui.add_space(8.0);
            ui.label(
            RichText::new("Polymath lists only folders you choose. Normal use starts no account, cloud sync, or control server.")
                .size(11.0)
                .color(muted),
        );
        });
    }
}

fn heading(ui: &mut egui::Ui, title: &str, subtitle: &str, muted: Color32) {
    ui.add_space(7.0);
    ui.label(RichText::new(title).size(23.0).strong());
    ui.add_space(3.0);
    ui.label(RichText::new(subtitle).size(12.0).color(muted));
    ui.add_space(4.0);
}

fn settings_card(ui: &mut egui::Ui, dark: bool, body: impl FnOnce(&mut egui::Ui)) {
    let width = ui.available_width();
    let chrome = crate::ui_kit::chrome(dark);
    egui::Frame::new()
        .fill(chrome.surface)
        .stroke(egui::Stroke::new(1.0, chrome.line))
        .corner_radius(14)
        .inner_margin(16)
        .show(ui, |ui| {
            ui.set_width((width - 34.0).max(100.0));
            body(ui);
        });
}
