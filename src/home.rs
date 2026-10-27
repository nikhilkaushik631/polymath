use crate::{Polymath, Workspace, home_view};
use egui::{Color32, RichText, Stroke, vec2};

pub(crate) fn system_reduce_motion() -> bool {
    if cfg!(test)
        || (std::env::var_os("POLYMATH_NO_PREFS").is_some()
            || std::env::var_os("IRIS_NO_PREFS").is_some())
    {
        return false;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("/usr/bin/defaults")
            .args(["read", "com.apple.universalaccess", "reduceMotion"])
            .output()
            .is_ok_and(|out| {
                out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "1"
            })
    }
    #[cfg(not(target_os = "macos"))]
    false
}
impl Polymath {
    pub(crate) fn bar(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        // The toolbar button can take focus on click. Capture the editor field first.
        let focused_before_toolbar = ui.ctx().memory(|memory| memory.focused());
        egui::Panel::top("iris-workspaces")
            .exact_size(48.0)
            .show(ui, |ui| {
                for font in ui.style_mut().text_styles.values_mut() {
                    font.family = egui::FontFamily::Proportional;
                }
                let text = Color32::from_gray(if self.dark { 238 } else { 24 });
                ui.visuals_mut().override_text_color = Some(text);
                ui.visuals_mut().selection.bg_fill =
                    Color32::from_gray(if self.dark { 64 } else { 220 });
                ui.visuals_mut().selection.stroke = Stroke::new(1.0, text);
                ui.style_mut().spacing.button_padding = vec2(10.0, 6.0);
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Button, egui::FontId::proportional(13.5));
                ui.horizontal_centered(|ui| {
                    ui.add_space(4.0);
                    ui.add(
                        egui::Image::new(egui::include_image!("../assets/icons/polymath.svg"))
                            .fit_to_exact_size(vec2(28.0, 28.0)),
                    );
                    ui.label(RichText::new("Polymath").size(18.0).strong());
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);
                    if self.active == Workspace::Home {
                        ui.label(
                            RichText::new("Workspace")
                                .size(13.0)
                                .color(ui.visuals().weak_text_color()),
                        );
                    } else {
                        let image = egui::Image::new(Workspace::Home.icon())
                            .fit_to_exact_size(vec2(16.0, 16.0))
                            .tint(ui.visuals().text_color());
                        if ui
                            .add(
                                egui::Button::image_and_text(image, "Home")
                                    .min_size(vec2(74.0, 32.0)),
                            )
                            .on_hover_text("Return to your workspace · ⌘0")
                            .clicked()
                        {
                            self.next = Some(Workspace::Home);
                        }
                        let accent = self.active.accent(self.dark);
                        let capsule = ui
                            .visuals()
                            .widgets
                            .inactive
                            .bg_fill
                            .lerp_to_gamma(accent, if self.dark { 0.24 } else { 0.13 });
                        egui::Frame::new()
                            .fill(capsule)
                            .corner_radius(8)
                            .inner_margin(egui::Margin::symmetric(10, 5))
                            .show(ui, |ui| {
                                ui.horizontal_centered(|ui| {
                                    ui.add(
                                        egui::Image::new(self.active.icon())
                                            .fit_to_exact_size(vec2(16.0, 16.0))
                                            .tint(accent),
                                    );
                                    ui.label(
                                        RichText::new(self.active.label())
                                            .size(13.0)
                                            .strong()
                                            .color(accent),
                                    );
                                    if self.dirty(self.active) {
                                        ui.label(RichText::new("•").size(15.0).color(accent));
                                    }
                                });
                            });
                    }
                    if self.active == Workspace::Reader
                        && let Some(path) = self.current_reader_path()
                    {
                        let favorite = self.reading_memory.is_favorite(&path);
                        let label = if favorite {
                            "Remove from Favorites"
                        } else {
                            "Add to Favorites"
                        };
                        let icon = pdfcraft_ui_egui::icons::image(
                            "star",
                            18.0,
                            if favorite {
                                Workspace::Reader.accent(self.dark)
                            } else {
                                text
                            },
                        )
                        .alt_text(label);
                        if ui
                            .add(
                                egui::Button::image(icon)
                                    .selected(favorite)
                                    .min_size(vec2(34.0, 34.0)),
                            )
                            .on_hover_text(label)
                            .clicked()
                        {
                            self.reading_memory.toggle_favorite(&path);
                            self.save_preferences();
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let ctx = ui.ctx().clone();
                        if crate::theme_button::show(ui, self.dark).clicked() {
                            self.set_dark(!self.dark, &ctx);
                        }
                        let settings_icon = pdfcraft_ui_egui::icons::image(
                            "settings",
                            18.0,
                            ui.visuals().text_color(),
                        )
                        .alt_text("Settings");
                        if ui
                            .add(egui::Button::image(settings_icon).min_size(vec2(34.0, 34.0)))
                            .on_hover_text("Polymath settings · appearance, voice, folders · ⌘,")
                            .clicked()
                        {
                            self.voice.panel_open = !self.voice.panel_open;
                        }
                        let mic_label = if self.voice.recording() {
                            "Stop dictation and insert"
                        } else if self.voice.transcribing() {
                            "Transcribing locally…"
                        } else {
                            "Dictate into the selected text field"
                        };
                        let mic_icon =
                            pdfcraft_ui_egui::icons::image("mic", 18.0, ui.visuals().text_color())
                                .alt_text(mic_label);
                        let mic = ui
                            .add_enabled(
                                !self.voice.transcribing(),
                                egui::Button::image(mic_icon)
                                    .selected(self.voice.recording())
                                    .min_size(vec2(34.0, 34.0)),
                            )
                            .on_hover_text(mic_label);
                        if mic.clicked() {
                            let result = if self.voice.recording() {
                                self.voice.stop_dictation()
                            } else {
                                self.voice
                                    .start_dictation(self.active, focused_before_toolbar)
                            };
                            if let Err(error) = result {
                                self.voice.notice = error;
                                self.voice.panel_open = true;
                                self.settings_page = crate::settings::Page::Voice;
                            }
                            ctx.request_repaint();
                        }
                        if self.active == Workspace::Reader {
                            if let Some(path) = self.editable_reader_path() {
                                let editor = Workspace::for_path(std::path::Path::new(&path))
                                    .map(|workspace| workspace.label())
                                    .unwrap_or("Writer");
                                if egui_shadcn::Button::new(format!("Edit in {editor}"))
                                    .variant(egui_shadcn::ButtonVariant::Solid)
                                    .size(egui_shadcn::ButtonSize::Sm)
                                    .show(ui, &crate::ui_kit::theme(self.dark, self.active))
                                    .on_hover_text("Open the original file for editing")
                                    .clicked()
                                    && let Err(error) = self.open_for_edit(&path, &ctx)
                                {
                                    self.notice = Some(error);
                                }
                            } else if self.source.is_none()
                                && self.media.is_none()
                                && self.image_preview.is_none()
                                && self.active_office_preview_path().is_none()
                                && self.reader.first_dirty().is_some()
                                && ui.button("Save PDF").clicked()
                            {
                                self.reader.run_command("file.save");
                            }
                        }
                        if !matches!(
                            self.active,
                            Workspace::Home | Workspace::Reader | Workspace::Tools
                        ) && egui_shadcn::Button::new("Open…")
                            .variant(egui_shadcn::ButtonVariant::Outline)
                            .size(egui_shadcn::ButtonSize::Sm)
                            .show(ui, &crate::ui_kit::theme(self.dark, self.active))
                            .on_hover_text("Open any supported file · ⌘O")
                            .clicked()
                        {
                            self.pick(false, frame, &ctx);
                        }
                    });
                });
            });
        self.settings_window(&ui.ctx().clone(), frame);
    }
    pub(crate) fn home(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.listing
            .update(&ctx, &self.folders, crate::workspace::EXTENSIONS);
        let folders: Vec<_> = self
            .folders
            .iter()
            .map(|path| {
                let listing = self.listing.listings.iter().find(|l| &l.folder == path);
                home_view::Folder {
                    path: path.clone(),
                    files: listing
                        .map(|l| l.files.iter().map(|f| f.path.clone()).collect())
                        .unwrap_or_default(),
                    status: match listing {
                        None => home_view::FolderStatus::Loading,
                        Some(l) if l.unavailable => home_view::FolderStatus::Unavailable,
                        Some(_) => home_view::FolderStatus::Ready,
                    },
                }
            })
            .collect();
        let action = home_view::show(ui, &mut self.home_state, &self.recent, &folders, self.dark);
        let result = match action {
            Some(home_view::Action::Enter(workspace)) => {
                self.activate(workspace, &ctx);
                Ok(())
            }
            Some(home_view::Action::OpenDialog) => {
                self.pick(false, frame, &ctx);
                Ok(())
            }
            Some(home_view::Action::ChooseFolder) => {
                self.pick(true, frame, &ctx);
                Ok(())
            }
            Some(home_view::Action::RemoveFolder(path)) => {
                self.folders.retain(|f| f != &path);
                self.listing.forget(&path);
                Ok(())
            }
            Some(home_view::Action::Refresh) => {
                self.listing.refresh();
                Ok(())
            }
            Some(home_view::Action::Open(path)) => self.open(&path, &ctx).map(|_| ()),
            None => Ok(()),
        };
        if let Err(error) = result {
            self.notice = Some(error);
        }
    }

    pub(crate) fn reader_landing(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        let dark = self.dark;
        let canvas = Color32::from_gray(if dark { 22 } else { 248 });
        let surface = Color32::from_gray(if dark { 34 } else { 255 });
        let ink = Color32::from_gray(if dark { 240 } else { 26 });
        let muted = Color32::from_gray(if dark { 168 } else { 101 });
        let line = Color32::from_gray(if dark { 61 } else { 218 });
        let accent = if dark {
            Color32::from_rgb(135, 188, 218)
        } else {
            Color32::from_rgb(42, 95, 126)
        };
        let colors = ReaderColors {
            dark,
            surface,
            ink,
            muted,
            line,
            accent,
        };
        let mut choose = false;
        let mut selected = None;
        let mut unfavorite = None;
        let recent = self.recent.clone();
        let favorites = self.reading_memory.favorites().to_vec();
        let favorite_pages: Vec<_> = favorites
            .iter()
            .map(|path| self.reading_memory.page(path))
            .collect();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(canvas).inner_margin(30))
            .show(ui, |ui| {
                ui.add_space(18.0);
                ui.label(RichText::new("Reader").size(29.0).strong().color(ink));
                ui.add_space(5.0);
                ui.label(
                    RichText::new("A place to read before you edit.")
                        .size(13.0)
                        .color(muted),
                );
                ui.add_space(24.0);
                let available = ui.available_width().min(1000.0);
                let primary = if available >= 740.0 {
                    (available - 14.0) * 0.59
                } else {
                    available
                };
                let cards =
                    |ui: &mut egui::Ui, choose: &mut bool, selected: &mut Option<String>| {
                        reader_open_card(ui, primary, colors, choose);
                        if available >= 740.0 {
                            ui.add_space(14.0);
                        } else {
                            ui.add_space(16.0);
                        }
                        reader_recent_card(
                            ui,
                            if available >= 740.0 {
                                available - primary - 14.0
                            } else {
                                available
                            },
                            colors,
                            &recent,
                            selected,
                        );
                    };
                if available >= 740.0 {
                    ui.horizontal_top(|ui| cards(ui, &mut choose, &mut selected));
                } else {
                    ui.vertical(|ui| cards(ui, &mut choose, &mut selected));
                }
                ui.add_space(16.0);
                reader_favorites_card(
                    ui,
                    available,
                    colors,
                    &favorites,
                    &favorite_pages,
                    &mut selected,
                    &mut unfavorite,
                );
            });
        if let Some(path) = unfavorite {
            self.reading_memory.remove_favorite(&path);
            self.save_preferences();
        }
        if choose {
            self.pick(false, frame, &ui.ctx().clone());
        }
        if let Some(path) = selected
            && let Err(error) = self.open_in_reader(&path, &ui.ctx().clone())
        {
            self.notice = Some(error);
        }
    }
}

#[derive(Clone, Copy)]
struct ReaderColors {
    dark: bool,
    surface: Color32,
    ink: Color32,
    muted: Color32,
    line: Color32,
    accent: Color32,
}

fn reader_open_card(ui: &mut egui::Ui, width: f32, colors: ReaderColors, choose: &mut bool) {
    let ReaderColors {
        dark,
        surface,
        ink,
        muted,
        line,
        accent,
    } = colors;
    let tint = if dark {
        Color32::from_rgb(31, 48, 59)
    } else {
        Color32::from_rgb(231, 241, 246)
    };
    egui::Frame::new()
        .fill(tint)
        .stroke(Stroke::new(1.0, line))
        .corner_radius(14)
        .inner_margin(22)
        .show(ui, |ui| {
            ui.set_width(width - 44.0);
            ui.vertical(|ui| {
                ui.set_min_height(213.0);
                ui.add(
                    egui::Image::new(Workspace::Reader.icon())
                        .fit_to_exact_size(vec2(32.0, 32.0))
                        .tint(accent),
                );
                ui.add_space(14.0);
                ui.label(
                    RichText::new("Open a file to preview")
                        .size(18.0)
                        .strong()
                        .color(ink),
                );
                ui.add_space(5.0);
                ui.label(
                    RichText::new("PDF, Word, Excel, PowerPoint, Markdown and source code.")
                        .size(12.0)
                        .color(muted),
                );
                ui.add_space(17.0);
                if ui
                    .add(
                        egui::Button::new(RichText::new("Choose a file…").color(surface))
                            .fill(ink)
                            .stroke(Stroke::NONE)
                            .min_size(vec2(145.0, 37.0)),
                    )
                    .clicked()
                {
                    *choose = true;
                }
            });
        });
}

fn reader_recent_card(
    ui: &mut egui::Ui,
    width: f32,
    colors: ReaderColors,
    recent: &[String],
    selected: &mut Option<String>,
) {
    let ReaderColors {
        surface,
        ink,
        muted,
        line,
        ..
    } = colors;
    egui::Frame::new()
        .fill(surface)
        .stroke(Stroke::new(1.0, line))
        .corner_radius(14)
        .inner_margin(20)
        .show(ui, |ui| {
            ui.set_width((width - 40.0).max(150.0));
            ui.vertical(|ui| {
                ui.set_min_height(217.0);
                ui.label(
                    RichText::new("Recently opened")
                        .size(15.0)
                        .strong()
                        .color(ink),
                );
                ui.add_space(11.0);
                let files: Vec<_> = recent
                    .iter()
                    .filter(|path| {
                        Workspace::for_path(std::path::Path::new(path)).ok()
                            != Some(Workspace::Images)
                    })
                    .take(4)
                    .collect();
                if files.is_empty() {
                    ui.label(
                        RichText::new("Your reading list will appear here after you open a file.")
                            .size(12.0)
                            .color(muted),
                    );
                }
                for path in files {
                    let name = std::path::Path::new(path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(path);
                    let response = ui
                        .add_sized(
                            [ui.available_width(), 34.0],
                            egui::Button::new(RichText::new(name).size(12.0)).frame(false),
                        )
                        .on_hover_text(path);
                    if response.clicked() {
                        *selected = Some(path.clone());
                    }
                    ui.separator();
                }
            });
        });
}

fn reader_favorites_card(
    ui: &mut egui::Ui,
    width: f32,
    colors: ReaderColors,
    favorites: &[String],
    pages: &[Option<usize>],
    selected: &mut Option<String>,
    unfavorite: &mut Option<String>,
) {
    egui::Frame::new()
        .fill(colors.surface)
        .stroke(Stroke::new(1.0, colors.line))
        .corner_radius(14)
        .inner_margin(20)
        .show(ui, |ui| {
            ui.set_width((width - 40.0).max(150.0));
            ui.horizontal_centered(|ui| {
                let star = pdfcraft_ui_egui::icons::image("star", 19.0, colors.accent)
                    .alt_text("Favorites");
                ui.add(star);
                ui.label(
                    RichText::new("Favorites")
                        .size(15.0)
                        .strong()
                        .color(colors.ink),
                );
                if !favorites.is_empty() {
                    ui.label(
                        RichText::new(favorites.len().to_string())
                            .size(11.0)
                            .color(colors.muted),
                    );
                }
            });
            ui.add_space(6.0);
            if favorites.is_empty() {
                ui.label(
                    RichText::new(
                        "Save files you return to. Use the star in Reader while a file is open.",
                    )
                    .size(12.0)
                    .color(colors.muted),
                );
                return;
            }
            egui::ScrollArea::vertical()
                .id_salt("reader-favorites")
                .max_height(270.0)
                .show(ui, |ui| {
                    for (index, path) in favorites.iter().enumerate() {
                        let name = std::path::Path::new(path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(path);
                        let available = std::path::Path::new(path).is_file();
                        let detail = if !available {
                            "File unavailable".to_owned()
                        } else if let Some(Some(page)) = pages.get(index) {
                            format!("Continue at page {}", page + 1)
                        } else {
                            "Open in Reader".to_owned()
                        };
                        ui.horizontal_centered(|ui| {
                            let button = egui::Button::new(format!("{name}  ·  {detail}"))
                                .frame(false)
                                .min_size(vec2((ui.available_width() - 45.0).max(120.0), 34.0));
                            if ui
                                .add_enabled(available, button)
                                .on_hover_text(path)
                                .clicked()
                            {
                                *selected = Some(path.clone());
                            }
                            let remove = pdfcraft_ui_egui::icons::image("x", 16.0, colors.muted)
                                .alt_text("Remove favorite");
                            if ui
                                .add(
                                    egui::Button::image(remove)
                                        .frame(false)
                                        .min_size(vec2(30.0, 30.0)),
                                )
                                .on_hover_text(format!("Remove {name} from Favorites"))
                                .clicked()
                            {
                                *unfavorite = Some(path.clone());
                            }
                        });
                        if index + 1 < favorites.len() {
                            ui.separator();
                        }
                    }
                });
        });
}
