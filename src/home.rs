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
        let chrome = crate::ui_kit::chrome(self.dark);
        egui::Panel::top("polymath-workspaces")
            .exact_size(48.0)
            .frame(
                egui::Frame::new()
                    .fill(chrome.surface)
                    .stroke(Stroke::new(1.0, chrome.line))
                    .inner_margin(egui::Margin::symmetric(8, 5)),
            )
            .show(ui, |ui| {
                for font in ui.style_mut().text_styles.values_mut() {
                    font.family = egui::FontFamily::Proportional;
                }
                let text = chrome.ink;
                ui.visuals_mut().override_text_color = Some(text);
                ui.visuals_mut().selection.bg_fill =
                    Color32::from_gray(if self.dark { 64 } else { 220 });
                ui.visuals_mut().selection.stroke = Stroke::new(1.0, text);
                ui.style_mut().spacing.button_padding = vec2(10.0, 6.0);
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Button, egui::FontId::proportional(13.5));
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Body, egui::FontId::proportional(13.0));
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
                ui.horizontal_centered(|ui| {
                    ui.add_space(4.0);
                    ui.add(
                        egui::Image::new(egui::include_image!("../assets/icons/polymath.svg"))
                            .fit_to_exact_size(vec2(28.0, 28.0)),
                    );
                    ui.label(RichText::new("Polymath").size(17.0).strong());
                    ui.add_space(9.0);
                    ui.separator();
                    ui.add_space(5.0);
                    if self.active == Workspace::Home {
                        ui.label(RichText::new("Home").size(13.0).color(chrome.muted));
                    } else {
                        let image = egui::Image::new(Workspace::Home.icon())
                            .fit_to_exact_size(vec2(16.0, 16.0))
                            .tint(ui.visuals().text_color());
                        if ui
                            .add(
                                egui::Button::image_and_text(image, "Home")
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::NONE)
                                    .corner_radius(8)
                                    .min_size(vec2(74.0, 34.0)),
                            )
                            .on_hover_text("Return to your workspace · ⌘0")
                            .clicked()
                        {
                            self.next = Some(Workspace::Home);
                        }
                        ui.label(RichText::new("/").size(13.0).color(chrome.muted));
                        let accent = self.active.accent(self.dark);
                        let capsule = chrome
                            .surface
                            .lerp_to_gamma(accent, if self.dark { 0.24 } else { 0.11 });
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
                        if crate::theme_button::icon(
                            ui,
                            "star",
                            label,
                            self.dark,
                            favorite,
                            Workspace::Reader.accent(self.dark),
                        )
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
                        if crate::theme_button::icon(
                            ui,
                            "settings",
                            "Settings",
                            self.dark,
                            self.voice.panel_open,
                            self.active.accent(self.dark),
                        )
                        .on_hover_text("Polymath settings · appearance, voice, folders · ⌘,")
                        .clicked()
                        {
                            self.voice.panel_open = !self.voice.panel_open;
                        }
                        if crate::theme_button::icon(
                            ui,
                            "copy",
                            "Clipboard History",
                            self.dark,
                            false,
                            self.active.accent(self.dark),
                        )
                        .on_hover_text(format!(
                            "Open Clipboard History · {}",
                            self.clipboard_history.display_shortcut()
                        ))
                        .clicked()
                        {
                            self.clipboard_history.show(&ctx);
                            self.save_preferences();
                        }
                        if matches!(
                            self.active,
                            Workspace::Tools
                                | Workspace::Writer
                                | Workspace::Sheets
                                | Workspace::Slides
                        ) || self.voice.recording()
                        {
                            let mic_label = if self.voice.recording() {
                                "Stop dictation and insert"
                            } else if self.voice.transcribing() {
                                "Transcribing locally…"
                            } else {
                                "Dictate into the selected text field"
                            };
                            let mic = ui
                                .add_enabled_ui(!self.voice.transcribing(), |ui| {
                                    crate::theme_button::icon(
                                        ui,
                                        "mic",
                                        mic_label,
                                        self.dark,
                                        self.voice.recording(),
                                        self.active.accent(self.dark),
                                    )
                                })
                                .inner;
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
                        }
                        if self.active == Workspace::Reader {
                            if let Some(path) = self
                                .media
                                .as_ref()
                                .filter(|media| {
                                    crate::workspace::is_video_path(std::path::Path::new(
                                        &media.path,
                                    ))
                                })
                                .map(|media| media.path.clone())
                            {
                                if egui_shadcn::Button::new("Edit video")
                                    .variant(egui_shadcn::ButtonVariant::Solid)
                                    .size(egui_shadcn::ButtonSize::Sm)
                                    .show(ui, &crate::ui_kit::theme(self.dark, Workspace::Video))
                                    .on_hover_text("Add this video to the editing timeline")
                                    .clicked()
                                    && let Err(error) = self.open_for_edit(&path, &ctx)
                                {
                                    self.notice = Some(error);
                                }
                            } else if let Some(path) = self.editable_reader_path() {
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
                                && self.archive_preview.is_none()
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
                            .on_hover_text("Open any supported file in a Reader tab · ⌘O")
                            .clicked()
                        {
                            self.pick(false, frame, &ctx);
                        }
                    });
                });
            });
        self.settings_window(&ui.ctx().clone(), frame);
    }

    pub(crate) fn close_reader_item(&mut self, ctx: &egui::Context) -> bool {
        if self.active != Workspace::Reader {
            return false;
        }
        self.active_reader_tab()
            .is_some_and(|id| self.close_reader_tab(id, ctx))
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
            Some(home_view::Action::OpenArchiveDialog) => {
                self.pick_filtered(false, frame, &ctx, crate::archive_view::EXTENSIONS);
                Ok(())
            }
            Some(home_view::Action::OpenMusicDialog) => {
                self.pick_filtered(false, frame, &ctx, crate::workspace::AUDIO_EXTENSIONS);
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
            Some(home_view::Action::Open(path)) => self.open_recent_or_prune(&path, &ctx),
            None => Ok(()),
        };
        if let Err(error) = result {
            self.notice = Some(error);
        }
    }

    pub(crate) fn reader_landing(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        let colors = ReaderColors {
            chrome: crate::ui_kit::chrome(self.dark),
            accent: Workspace::Reader.accent(self.dark),
            reduce_motion: self.home_state.reduce_motion,
        };
        let recent: Vec<_> = self
            .recent
            .iter()
            .map(|path| ReaderFile {
                path: path.clone(),
                page: self.reading_memory.page(path),
                available: true,
            })
            .collect();
        let favorites: Vec<_> = self
            .reading_memory
            .favorites()
            .iter()
            .map(|path| ReaderFile {
                path: path.clone(),
                page: self.reading_memory.page(path),
                available: std::path::Path::new(path).is_file(),
            })
            .collect();
        let mut choose = false;
        let mut selected = None;
        let mut unfavorite = None;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(colors.chrome.canvas).inner_margin(0))
            .show(ui, |ui| {
                for font in ui.style_mut().text_styles.values_mut() {
                    font.family = egui::FontFamily::Proportional;
                }
                ui.visuals_mut().override_text_color = Some(colors.chrome.ink);
                ui.visuals_mut().weak_text_color = Some(colors.chrome.muted);
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                egui::ScrollArea::vertical()
                    .id_salt("reader-landing")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let padding = if ui.available_width() < 760.0 { 20 } else { 30 };
                        egui::Frame::new().inner_margin(padding).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal_top(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(RichText::new("Reader").size(27.0).strong());
                                    ui.label(RichText::new("Documents, images and media, in one place.").size(13.0).color(colors.chrome.muted));
                                });
                                ui.allocate_ui_with_layout(
                                    vec2(ui.available_width(), 44.0),
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let open = egui_shadcn::Button::new("Open a file…")
                                            .variant(egui_shadcn::ButtonVariant::Solid)
                                            .min_width(138.0)
                                            .show(ui, &crate::ui_kit::theme(self.dark, Workspace::Reader))
                                            .on_hover_text("Open a file in Reader · ⌘O");
                                        reader_reveal_focus(&open, colors.reduce_motion);
                                        if open.clicked() {
                                            choose = true;
                                        }
                                    },
                                );
                            });
                            ui.add_space(18.0);
                            reader_open_hint(ui, colors);
                            ui.add_space(24.0);
                            reader_favorites(ui, colors, &favorites, &mut selected, &mut unfavorite);
                            ui.add_space(22.0);
                            reader_section_heading(ui, "Recent files", "clock", recent.len(), colors);
                            ui.add_space(8.0);
                            if recent.is_empty() {
                                egui::Frame::new().fill(colors.chrome.surface).corner_radius(12).inner_margin(20).show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.label(RichText::new("Start with a file on your Mac").size(15.0).strong());
                                    ui.add_space(4.0);
                                    ui.label(RichText::new("Files you open will appear here, with saved reading positions for PDFs and office documents.").size(12.5).color(colors.chrome.muted));
                                });
                            } else {
                                reader_file_list(ui, colors, &recent, false, &mut selected, &mut unfavorite);
                            }
                            ui.add_space(16.0);
                        });
                    });
            });
        if let Some(path) = unfavorite {
            self.reading_memory.remove_favorite(&path);
            self.save_preferences();
        }
        if choose {
            self.pick(false, frame, &ui.ctx().clone());
        }
        if let Some(path) = selected
            && let Err(error) = self.open_recent_or_prune(&path, &ui.ctx().clone())
        {
            self.notice = Some(error);
        }
    }
}

#[derive(Clone, Copy)]
struct ReaderColors {
    chrome: crate::ui_kit::Chrome,
    accent: Color32,
    reduce_motion: bool,
}

struct ReaderFile {
    path: String,
    page: Option<usize>,
    available: bool,
}

fn reader_open_hint(ui: &mut egui::Ui, colors: ReaderColors) {
    let tint = colors.chrome.surface.lerp_to_gamma(colors.accent, 0.09);
    egui::Frame::new()
        .fill(tint)
        .corner_radius(12)
        .inner_margin(16)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.add(
                    egui::Image::new(Workspace::Reader.icon())
                        .fit_to_exact_size(vec2(27.0, 27.0))
                        .tint(colors.accent),
                );
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Drop a file here to open it")
                            .size(14.0)
                            .strong()
                            .color(colors.chrome.ink),
                    );
                    ui.label(
                        RichText::new(
                            "PDF and office files · Text and code · Images, audio and video",
                        )
                        .size(12.0)
                        .color(colors.chrome.muted),
                    );
                });
            });
        });
}

fn reader_section_heading(
    ui: &mut egui::Ui,
    title: &str,
    icon: &str,
    count: usize,
    colors: ReaderColors,
) {
    ui.horizontal(|ui| {
        ui.add(pdfcraft_ui_egui::icons::image(
            icon,
            17.0,
            colors.chrome.muted,
        ));
        ui.label(
            RichText::new(title)
                .size(16.0)
                .strong()
                .color(colors.chrome.ink),
        );
        if count > 0 {
            ui.label(
                RichText::new(count.to_string())
                    .size(12.0)
                    .color(colors.chrome.muted),
            );
        }
    });
}

fn reader_favorites(
    ui: &mut egui::Ui,
    colors: ReaderColors,
    favorites: &[ReaderFile],
    selected: &mut Option<String>,
    unfavorite: &mut Option<String>,
) {
    if favorites.is_empty() {
        // A compact inline explanation, without reserving a panel for nonexistent files.
        ui.horizontal(|ui| {
            ui.add(pdfcraft_ui_egui::icons::image("star", 18.0, colors.accent));
            ui.add_space(2.0);
            ui.label(
                RichText::new("Favorites")
                    .size(13.0)
                    .strong()
                    .color(colors.chrome.ink),
            );
            ui.add(
                egui::Label::new(
                    RichText::new("Use the star on an open file to keep it here.")
                        .size(12.0)
                        .color(colors.chrome.muted),
                )
                .wrap(),
            );
        });
        return;
    }
    reader_section_heading(ui, "Favorites", "star", favorites.len(), colors);
    ui.add_space(8.0);
    egui::ScrollArea::vertical()
        .id_salt("reader-favorites")
        .max_height(264.0)
        .show(ui, |ui| {
            reader_file_list(ui, colors, favorites, true, selected, unfavorite);
        });
}

fn reader_file_list(
    ui: &mut egui::Ui,
    colors: ReaderColors,
    files: &[ReaderFile],
    favorites: bool,
    selected: &mut Option<String>,
    unfavorite: &mut Option<String>,
) {
    egui::Frame::new()
        .fill(colors.chrome.surface)
        .stroke(Stroke::new(1.0, colors.chrome.line))
        .corner_radius(12)
        .inner_margin(5)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            for (index, file) in files.iter().enumerate() {
                ui.push_id((&file.path, favorites), |ui| {
                    reader_file_row(ui, colors, file, favorites, selected, unfavorite);
                });
                if index + 1 < files.len() {
                    let x = ui.max_rect().x_range();
                    ui.painter().hline(
                        (x.min + 48.0)..=(x.max - 10.0),
                        ui.cursor().top(),
                        Stroke::new(0.5, colors.chrome.line),
                    );
                }
            }
        });
}

fn reader_file_row(
    ui: &mut egui::Ui,
    colors: ReaderColors,
    file: &ReaderFile,
    favorite: bool,
    selected: &mut Option<String>,
    unfavorite: &mut Option<String>,
) {
    let path = std::path::Path::new(&file.path);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&file.path);
    let parent = path
        .parent()
        .map(|parent| parent.to_string_lossy())
        .unwrap_or_default();
    let kind = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("File")
        .to_ascii_uppercase();
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 62.0), egui::Sense::hover());
    let open_rect = egui::Rect::from_min_max(
        rect.min,
        egui::pos2(
            rect.right() - if favorite { 40.0 } else { 0.0 },
            rect.bottom(),
        ),
    );
    let response = ui.interact(
        open_rect,
        ui.id().with("open"),
        if file.available {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            file.available && ui.is_enabled(),
            name,
        )
    });
    reader_reveal_focus(&response, colors.reduce_motion);
    if file.available && (response.hovered() || response.has_focus()) {
        ui.painter().rect_filled(open_rect, 8, colors.chrome.hover);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            open_rect.shrink(1.0),
            8,
            Stroke::new(1.5, colors.accent),
            egui::StrokeKind::Inside,
        );
    }
    let ink = if file.available {
        colors.chrome.ink
    } else {
        colors.chrome.muted
    };
    let workspace = Workspace::for_path(path).unwrap_or(Workspace::Reader);
    egui::Image::new(workspace.icon())
        .fit_to_exact_size(vec2(22.0, 22.0))
        .tint(colors.accent)
        .paint_at(
            ui,
            egui::Rect::from_center_size(
                open_rect.left_center() + vec2(25.0, 0.0),
                vec2(22.0, 22.0),
            ),
        );
    let detail = if file.available {
        parent.as_ref()
    } else {
        "File unavailable"
    };
    let text_width = (open_rect.width() - 166.0).max(70.0);
    reader_row_text(
        ui,
        open_rect.min + vec2(48.0, 11.0),
        name,
        13.0,
        ink,
        text_width,
    );
    reader_row_text(
        ui,
        open_rect.min + vec2(48.0, 33.0),
        detail,
        11.5,
        colors.chrome.muted,
        text_width,
    );
    ui.painter().text(
        open_rect.right_top() + vec2(-12.0, 14.0),
        egui::Align2::RIGHT_TOP,
        kind,
        egui::FontId::proportional(11.0),
        colors.chrome.muted,
    );
    if file.available
        && let Some(page) = file.page
    {
        ui.painter().text(
            open_rect.right_bottom() - vec2(12.0, 12.0),
            egui::Align2::RIGHT_BOTTOM,
            format!("Page {}", page + 1),
            egui::FontId::proportional(11.5),
            colors.accent,
        );
    }
    if response.clicked() {
        *selected = Some(file.path.clone());
    }
    response
        .on_hover_text(&file.path)
        .on_hover_cursor(if file.available {
            egui::CursorIcon::PointingHand
        } else {
            egui::CursorIcon::Default
        });
    if favorite {
        let remove_rect =
            egui::Rect::from_center_size(rect.right_center() - vec2(20.0, 0.0), vec2(34.0, 34.0));
        let label = format!("Remove {name} from Favorites");
        let remove = ui
            .place(
                remove_rect,
                egui::Button::image(
                    pdfcraft_ui_egui::icons::image("x", 15.0, colors.chrome.muted).alt_text(&label),
                )
                .frame(false)
                .corner_radius(8),
            )
            .on_hover_text(label);
        reader_reveal_focus(&remove, colors.reduce_motion);
        if remove.clicked() {
            *unfavorite = Some(file.path.clone());
        }
    }
}

fn reader_reveal_focus(response: &egui::Response, reduce_motion: bool) {
    if response.gained_focus() {
        if reduce_motion {
            response.scroll_to_me_animation(
                Some(egui::Align::Center),
                egui::style::ScrollAnimation::none(),
            );
        } else {
            response.scroll_to_me(Some(egui::Align::Center));
        }
    }
}

fn reader_row_text(
    ui: &egui::Ui,
    pos: egui::Pos2,
    text: &str,
    size: f32,
    color: Color32,
    width: f32,
) {
    let mut job = egui::text::LayoutJob::simple_singleline(
        text.into(),
        egui::FontId::proportional(size),
        color,
    );
    job.wrap.max_width = width;
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    ui.painter().galley(pos, galley, color);
}
