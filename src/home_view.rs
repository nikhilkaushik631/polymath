//! The shared file library. Rendering is separate from native dialogs and document engines.
use crate::{home_fonts, workspace::Workspace};
#[path = "home_art.rs"]
mod art;
use egui::{
    Align, Align2, Color32, FontId, Layout, Rect, RichText, Sense, Stroke, StrokeKind, vec2,
};
use std::path::Path;

#[derive(Default)]
pub struct HomeState {
    pub query: String,
    pub kind: Option<Workspace>,
    pub folder: Option<String>,
    pub reduce_motion: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FolderStatus {
    Loading,
    Ready,
    Unavailable,
}
pub struct Folder {
    pub path: String,
    pub files: Vec<String>,
    pub status: FolderStatus,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    OpenDialog,
    OpenArchiveDialog,
    OpenMusicDialog,
    Enter(Workspace),
    ChooseFolder,
    RemoveFolder(String),
    Refresh,
    Open(String),
}

#[derive(Clone, Copy)]
struct Palette {
    dark: bool,
    canvas: Color32,
    surface: Color32,
    ink: Color32,
    muted: Color32,
    accent: Color32,
    line: Color32,
    hover: Color32,
}
#[derive(Clone, Copy)]
struct WorkspaceCard<'a> {
    workspace: Workspace,
    label: &'a str,
    detail: &'a str,
}
impl Palette {
    fn new(dark: bool) -> Self {
        let gray = Color32::from_gray;
        if dark {
            Self {
                dark,
                canvas: gray(22),
                surface: gray(35),
                ink: gray(240),
                muted: gray(172),
                accent: Color32::from_rgb(135, 188, 218),
                line: gray(62),
                hover: gray(48),
            }
        } else {
            Self {
                dark,
                canvas: gray(250),
                surface: gray(237),
                ink: gray(25),
                muted: gray(102),
                accent: Color32::from_rgb(42, 95, 126),
                line: gray(218),
                hover: gray(231),
            }
        }
    }
}

pub fn show(
    ui: &mut egui::Ui,
    state: &mut HomeState,
    recent: &[String],
    folders: &[Folder],
    dark: bool,
) -> Option<Action> {
    // Editors replace the context's font families. New definitions take effect
    // on the next egui pass, so never draw named Home fonts in the old atlas.
    if !home_fonts::ready(ui.ctx()) {
        home_fonts::install(ui.ctx());
        ui.ctx()
            .request_discard("Load Home typography after workspace switch");
        ui.ctx().request_repaint();
        return None;
    }
    let p = Palette::new(dark);
    let mut action = None;
    if state
        .folder
        .as_ref()
        .is_some_and(|path| !folders.iter().any(|f| &f.path == path))
    {
        state.folder = None;
    }
    ui.visuals_mut().override_text_color = Some(p.ink);
    ui.visuals_mut().weak_text_color = Some(p.muted);
    ui.visuals_mut().selection.bg_fill = p.hover;
    ui.visuals_mut().selection.stroke = Stroke::new(1.0, p.ink);
    ui.style_mut().spacing.item_spacing = vec2(8.0, 8.0);
    ui.style_mut().spacing.button_padding = vec2(12.0, 8.0);

    egui::CentralPanel::default().frame(egui::Frame::new().fill(p.canvas).inner_margin(0)).show(ui, |ui| {
        egui::ScrollArea::vertical().id_salt("home-content").auto_shrink([false, false]).show(ui, |ui| {
            let padding = if ui.available_width() < 620.0 { 18 } else { 30 };
            egui::Frame::new().inner_margin(padding).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Your workspace").font(home_fonts::heading(30.0)));
                        ui.label(RichText::new("Open a file. Start something new.").size(13.5).color(p.muted));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let open = open_file_action(ui, p);
                        reveal_focus(&open, state.reduce_motion);
                        if open.clicked() {
                            action = Some(Action::OpenDialog);
                        }
                    });
                });
                if !recent.is_empty() {
                    ui.add_space(24.0);
                    recent_work(ui, recent, state.reduce_motion, p, &mut action);
                }
                ui.add_space(28.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Workspaces").font(home_fonts::heading(19.0)));
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("Pick up the right tool for your file")
                            .font(home_fonts::medium(11.5))
                            .color(p.muted),
                    );
                });
                ui.add_space(12.0);
                launchpad(ui, p, state.reduce_motion, &mut action);
                ui.add_space(30.0);
                let selected_folder = state.folder.as_ref().and_then(|path| folders.iter().find(|f| &f.path == path));
                let title = selected_folder.map_or_else(|| if recent.is_empty() {"Your library".to_string()} else {"Recent files".to_string()}, |f| basename(&f.path));
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(RichText::new(title).font(home_fonts::heading(19.0))).truncate());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(folder) = selected_folder {
                            ui.menu_button("Folder…", |ui| {
                                if ui.button("Refresh files").clicked() { action = Some(Action::Refresh); ui.close(); }
                                if ui.button("Remove from library").clicked() { action = Some(Action::RemoveFolder(folder.path.clone())); ui.close(); }
                            });
                        }
                    });
                });
                ui.add_space(12.0);
                library_sources(ui, state, folders, p, &mut action);
                ui.add_space(12.0);
                let show_filters = !recent.is_empty() || selected_folder.is_some() || !state.query.is_empty() || state.kind.is_some();
                if show_filters { library_toolbar(ui, state, p); }
                ui.add_space(10.0);
                let paths = selected_folder.map_or(recent, |f| f.files.as_slice());
                let visible = filtered(paths, state);
                let status = selected_folder.map(|f| f.status);
                if status == Some(FolderStatus::Unavailable) {
                    empty(ui, p, "This folder is unavailable", "Choose it again to grant access, or remove it from your library.", Some(("Choose folder…", Action::ChooseFolder)), &mut action);
                } else if status == Some(FolderStatus::Loading) {
                    ui.add_space(30.0);
                    ui.label("Looking for files…");
                    for width in [0.65, 0.45, 0.55] {
                        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width() * width, 12.0), Sense::hover());
                        ui.painter().rect_filled(rect, 3, p.surface);
                        ui.add_space(15.0);
                    }
                } else if visible.is_empty() {
                    if !state.query.is_empty() || state.kind.is_some() {
                        if egui_shadcn::Button::new("Clear filters")
                            .variant(egui_shadcn::ButtonVariant::Outline)
                            .size(egui_shadcn::ButtonSize::Sm)
                            .show(ui, &crate::ui_kit::theme(p.dark, Workspace::Home))
                            .clicked()
                        {
                            state.query.clear();
                            state.kind = None;
                        }
                        empty(ui, p, "No matching files", "Try another name or show all file types.", None, &mut action);
                    } else if selected_folder.is_some() {
                        empty(ui, p, "No supported files here", "PDFs, source code, documents, spreadsheets, presentations and images in this folder will appear here.", Some(("Refresh files", Action::Refresh)), &mut action);
                    } else {
                        welcome_library(ui, p, &mut action);
                    }
                } else {
                    file_header(ui, p);
                    for path in &visible {
                        if file_row(ui, path, state.reduce_motion, p).clicked() { action = Some(Action::Open((*path).clone())); }
                    }
                    ui.add_space(10.0);
                    ui.label(RichText::new(format!("{} {}{}", visible.len(), if visible.len() == 1 {"file"} else {"files"}, if selected_folder.is_some() {" · Newest first · Up to 100 files per folder"} else {" · Recently opened"})).size(11.5).color(p.muted));
                }
            });
        });
    });
    action
}

fn open_file_action(ui: &mut egui::Ui, p: Palette) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(208.0, 46.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "Open a file")
    });
    let response = response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Choose a local file · ⌘O");
    let bg = if p.dark {
        if response.hovered() {
            Color32::from_rgb(48, 89, 111)
        } else {
            Color32::from_rgb(35, 70, 90)
        }
    } else if response.hovered() {
        Color32::from_rgb(43, 68, 82)
    } else {
        Color32::from_rgb(29, 48, 60)
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 10, bg);
    painter.rect_stroke(
        rect,
        10,
        Stroke::new(
            if response.has_focus() { 2.0 } else { 1.0 },
            if response.has_focus() {
                p.accent
            } else {
                Color32::from_rgb(73, 108, 126)
            },
        ),
        StrokeKind::Inside,
    );
    let icon = Color32::from_rgb(151, 204, 229);
    let x = rect.left() + 16.0;
    let y = rect.center().y - 9.0;
    let s = Stroke::new(1.8, icon);
    painter.line_segment([egui::pos2(x, y), egui::pos2(x + 10.0, y)], s);
    painter.line_segment([egui::pos2(x + 10.0, y), egui::pos2(x + 15.0, y + 5.0)], s);
    painter.line_segment(
        [
            egui::pos2(x + 15.0, y + 5.0),
            egui::pos2(x + 15.0, y + 18.0),
        ],
        s,
    );
    painter.line_segment([egui::pos2(x + 15.0, y + 18.0), egui::pos2(x, y + 18.0)], s);
    painter.line_segment([egui::pos2(x, y + 18.0), egui::pos2(x, y)], s);
    painter.line_segment(
        [
            egui::pos2(x + 3.0, y + 11.0),
            egui::pos2(x + 12.0, y + 11.0),
        ],
        Stroke::new(1.3, icon),
    );
    painter.text(
        rect.left_center() + vec2(43.0, 0.0),
        Align2::LEFT_CENTER,
        "Open a file",
        home_fonts::medium(13.0),
        Color32::WHITE,
    );
    let divider = rect.right() - 52.0;
    painter.vline(
        divider,
        (rect.top() + 9.0)..=(rect.bottom() - 9.0),
        Stroke::new(1.0, Color32::from_rgb(95, 127, 144)),
    );
    painter.text(
        egui::pos2(rect.right() - 26.0, rect.center().y),
        Align2::CENTER_CENTER,
        "⌘O",
        home_fonts::medium(11.0),
        Color32::from_rgb(218, 232, 239),
    );
    response
}

fn library_toolbar(ui: &mut egui::Ui, state: &mut HomeState, p: Palette) {
    let wide = ui.available_width() >= 1060.0;
    let filters = [
        (None, "All files"),
        (Some(Workspace::Reader), "Reader files"),
        (Some(Workspace::Writer), "Documents"),
        (Some(Workspace::Sheets), "Spreadsheets"),
        (Some(Workspace::Slides), "Presentations"),
        (Some(Workspace::Images), "Pictures"),
    ];
    let choices =
        |ui: &mut egui::Ui, state: &mut HomeState, items: &[(Option<Workspace>, &str)]| {
            ui.spacing_mut().item_spacing.x = 8.0;
            for &(kind, label) in items {
                let response = filter_pill(ui, label, state.kind == kind, p);
                reveal_focus(&response, state.reduce_motion);
                if response.clicked() {
                    state.kind = kind;
                }
            }
        };
    if wide {
        ui.horizontal(|ui| {
            choices(ui, state, &filters);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                library_search(ui, &mut state.query, 230.0, p);
            });
        });
    } else {
        for group in filters.chunks(3) {
            ui.horizontal(|ui| choices(ui, state, group));
        }
        ui.add_space(4.0);
        library_search(ui, &mut state.query, ui.available_width(), p);
    }
}

fn filter_pill(ui: &mut egui::Ui, label: &str, selected: bool, p: Palette) -> egui::Response {
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().button_padding = vec2(14.0, 7.0);
            let visuals = ui.visuals_mut();
            visuals.selection.bg_fill = p.ink;
            visuals.selection.stroke = Stroke::new(1.0, p.ink);
            visuals.widgets.inactive.weak_bg_fill = p.canvas;
            visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, p.line);
            visuals.widgets.hovered.weak_bg_fill = p.hover;
            visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, p.muted);
            visuals.widgets.active.weak_bg_fill = p.hover.lerp_to_gamma(p.ink, 0.1);
            visuals.widgets.active.bg_stroke = Stroke::new(1.0, p.ink);
            ui.add(
                egui::Button::new(
                    RichText::new(label)
                        .font(home_fonts::medium(12.0))
                        .color(if selected { p.canvas } else { p.ink }),
                )
                .selected(selected)
                .corner_radius(255)
                .min_size(vec2(0.0, 32.0)),
            )
        })
        .inner;
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.expand(2.0),
            255,
            Stroke::new(1.0, p.ink),
            StrokeKind::Outside,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn library_search(ui: &mut egui::Ui, query: &mut String, width: f32, p: Palette) {
    let frame = egui::Frame::new()
        .fill(p.canvas)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(9)
        .inner_margin(egui::Margin::symmetric(10, 7));
    let response = frame.show(ui, |ui| {
        ui.set_width((width - 22.0).max(104.0));
        ui.allocate_ui_with_layout(
            vec2((width - 22.0).max(104.0), 18.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.add(pdfcraft_ui_egui::icons::image("search", 16.0, p.muted));
                ui.add(
                    egui::TextEdit::singleline(query)
                        .id_salt("library-search")
                        .hint_text(RichText::new("Search files…").color(p.muted))
                        .font(egui::FontId::proportional(12.0))
                        .frame(egui::Frame::NONE)
                        .desired_width((width - 46.0).max(80.0))
                        .margin(vec2(0.0, 1.0)),
                )
                .on_hover_text("Search files by name")
            },
        )
        .inner
    });
    if response.inner.has_focus() {
        ui.painter().rect_stroke(
            response.response.rect,
            9,
            Stroke::new(1.0, p.ink),
            StrokeKind::Inside,
        );
    }
}

fn reveal_focus(response: &egui::Response, reduce_motion: bool) {
    if response.gained_focus() {
        if reduce_motion {
            response
                .scroll_to_me_animation(Some(Align::Center), egui::style::ScrollAnimation::none());
        } else {
            response.scroll_to_me(Some(Align::Center));
        }
    }
}

fn library_sources(
    ui: &mut egui::Ui,
    state: &mut HomeState,
    folders: &[Folder],
    p: Palette,
    action: &mut Option<Action>,
) {
    ui.horizontal_wrapped(|ui| {
        let recent = state.folder.is_none();
        let recent_button =
            egui::Button::new(RichText::new("Recent files").font(home_fonts::medium(11.5)))
                .corner_radius(18)
                .fill(if recent { p.hover } else { p.surface })
                .stroke(Stroke::new(1.0, if recent { p.accent } else { p.line }));
        let recent_response = ui.add(recent_button);
        if recent_response.clicked() {
            state.folder = None;
        }
        if recent_response.gained_focus() || recent_response.clicked() {
            reveal_source(&recent_response, state.reduce_motion);
        }
        for folder in folders {
            let selected = state.folder.as_ref() == Some(&folder.path);
            let name = basename(&folder.path);
            let button = egui::Button::new(RichText::new(&name).font(home_fonts::medium(11.5)))
                .corner_radius(18)
                .fill(if selected { p.hover } else { p.surface })
                .stroke(Stroke::new(1.0, if selected { p.accent } else { p.line }));
            let width = ui.fonts_mut(|fonts| {
                fonts
                    .layout_no_wrap(name.clone(), home_fonts::medium(11.5), p.ink)
                    .size()
                    .x
            }) + 24.0;
            let response = ui
                .add_sized([width.min(240.0), 32.0], button.truncate())
                .on_hover_text(&folder.path);
            if response.clicked() {
                state.folder = Some(folder.path.clone());
            }
            if response.gained_focus() || response.clicked() {
                reveal_source(&response, state.reduce_motion);
            }
            response.context_menu(|ui| {
                if ui
                    .button("Remove from library")
                    .on_hover_text("The files stay where they are.")
                    .clicked()
                {
                    *action = Some(Action::RemoveFolder(folder.path.clone()));
                    ui.close();
                }
            });
        }
        let add = egui::Button::new(RichText::new("+ Add folder").font(home_fonts::medium(11.5)))
            .corner_radius(18)
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, p.line));
        let response = ui
            .add(add)
            .on_hover_text("Choose a folder using the macOS picker");
        reveal_focus(&response, state.reduce_motion);
        if response.clicked() {
            *action = Some(Action::ChooseFolder);
        }
    });
}

fn reveal_source(response: &egui::Response, reduce_motion: bool) {
    if reduce_motion {
        response.scroll_to_me_animation(Some(Align::Min), egui::style::ScrollAnimation::none());
    } else {
        response.scroll_to_me(Some(Align::Min));
    }
}

pub fn filtered<'a>(paths: &'a [String], state: &HomeState) -> Vec<&'a String> {
    let query = state.query.trim().to_lowercase();
    paths
        .iter()
        .filter(|path| {
            state
                .kind
                .is_none_or(|kind| Workspace::for_path(Path::new(path)).ok() == Some(kind))
                && basename(path).to_lowercase().contains(&query)
        })
        .collect()
}
fn basename(path: &str) -> String {
    Path::new(path).file_name().map_or_else(
        || path.to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn recent_work(
    ui: &mut egui::Ui,
    recent: &[String],
    reduce_motion: bool,
    p: Palette,
    action: &mut Option<Action>,
) {
    ui.label(RichText::new("Continue working").font(home_fonts::heading(19.0)));
    ui.add_space(10.0);
    let columns = if ui.available_width() >= 760.0 { 3 } else { 2 };
    let gap = 12.0;
    let width = (ui.available_width() - gap * (columns - 1) as f32) / columns as f32;
    ui.spacing_mut().item_spacing.x = gap;
    ui.horizontal(|ui| {
        for path in recent.iter().take(columns) {
            let name = basename(path);
            let (rect, response) = ui.allocate_exact_size(vec2(width, 70.0), Sense::click());
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    ui.is_enabled(),
                    format!("Open {name}"),
                )
            });
            reveal_focus(&response, reduce_motion);
            ui.painter().rect_filled(
                rect,
                9,
                if response.hovered() {
                    p.hover
                } else {
                    p.surface
                },
            );
            ui.painter().rect_stroke(
                rect,
                9,
                Stroke::new(1.0, if response.has_focus() { p.ink } else { p.line }),
                StrokeKind::Inside,
            );
            let workspace = Workspace::for_path(Path::new(path)).unwrap_or(Workspace::Reader);
            egui::Image::new(workspace.icon())
                .tint(workspace.accent(p.dark))
                .paint_at(
                    ui,
                    Rect::from_center_size(rect.left_center() + vec2(25.0, 0.0), vec2(21.0, 21.0)),
                );
            let label_width = (rect.width() - 78.0).max(64.0);
            text_clipped(
                ui,
                rect.min + vec2(47.0, 15.0),
                &name,
                13.0,
                p.ink,
                label_width,
            );
            let parent = Path::new(path)
                .parent()
                .map(|folder| basename(&folder.to_string_lossy()))
                .unwrap_or_default();
            text_clipped(
                ui,
                rect.min + vec2(47.0, 39.0),
                &parent,
                11.0,
                p.muted,
                label_width,
            );
            if response
                .on_hover_text(path)
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                *action = Some(Action::Open(path.clone()));
            }
        }
    });
    ui.spacing_mut().item_spacing.x = 8.0;
}

fn launchpad(ui: &mut egui::Ui, p: Palette, reduce_motion: bool, action: &mut Option<Action>) {
    let width = ui.available_width();
    let gap = 12.0;
    ui.spacing_mut().item_spacing.x = gap;
    let reader = WorkspaceCard {
        workspace: Workspace::Reader,
        label: "Reader",
        detail: "Read PDFs, documents and code",
    };
    let tools = WorkspaceCard {
        workspace: Workspace::Tools,
        label: "PDF Tools",
        detail: "Edit, convert and sign",
    };
    if width >= 780.0 {
        let reader_width = (width - gap) * 0.55;
        ui.horizontal(|ui| {
            workspace_feature(
                ui,
                p,
                reduce_motion,
                reader,
                reader_width,
                182.0,
                false,
                action,
            );
            workspace_feature(
                ui,
                p,
                reduce_motion,
                tools,
                width - reader_width - gap,
                182.0,
                true,
                action,
            );
        });
    } else {
        workspace_feature(ui, p, reduce_motion, reader, width, 150.0, false, action);
        ui.add_space(5.0);
        workspace_feature(ui, p, reduce_motion, tools, width, 150.0, true, action);
    }
    ui.add_space(5.0);
    let choices = [
        (Workspace::Writer, "Writer", "Documents and notes"),
        (Workspace::Sheets, "Sheets", "Data and tables"),
        (Workspace::Slides, "Slides", "Presentations"),
        (Workspace::Images, "Images", "Photos and graphics"),
        (Workspace::Video, "Video", "Trim, arrange and export"),
    ];
    // Give each editor enough room to show the work it creates. Four identical
    // icon tiles flatten the hierarchy and leave no space for a useful preview.
    let columns = if width >= 650.0 { 2 } else { 1 };
    let card_width = (width - (columns - 1) as f32 * gap) / columns as f32;
    for row in choices.chunks(columns) {
        ui.horizontal(|ui| {
            for &(workspace, label, detail) in row {
                workspace_compact(
                    ui,
                    p,
                    reduce_motion,
                    WorkspaceCard {
                        workspace,
                        label,
                        detail,
                    },
                    if row.len() == 1 { width } else { card_width },
                    action,
                );
            }
        });
        ui.add_space(5.0);
    }
    if width >= 650.0 {
        ui.horizontal(|ui| {
            music_launch_card(ui, p, reduce_motion, card_width, action);
            archive_launch_card(ui, p, reduce_motion, card_width, action);
        });
    } else {
        music_launch_card(ui, p, reduce_motion, width, action);
        ui.add_space(5.0);
        archive_launch_card(ui, p, reduce_motion, width, action);
    }
    ui.spacing_mut().item_spacing.x = 8.0;
}

fn music_launch_card(
    ui: &mut egui::Ui,
    p: Palette,
    reduce_motion: bool,
    width: f32,
    action: &mut Option<Action>,
) {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 95.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "Open Music")
    });
    reveal_focus(&response, reduce_motion);
    let hover = card_hover(ui, &response, reduce_motion);
    let tone = if p.dark {
        Color32::from_rgb(145, 211, 180)
    } else {
        Color32::from_rgb(45, 111, 80)
    };
    let base = if p.dark {
        Color32::from_rgb(27, 48, 42)
    } else {
        Color32::from_rgb(230, 244, 237)
    };
    ui.painter()
        .rect_filled(rect, 12, base.lerp_to_gamma(p.hover, hover * 0.15));
    ui.painter()
        .rect_stroke(rect, 12, Stroke::new(1.0, p.line), StrokeKind::Inside);
    card_focus(ui, &response, p);
    egui::Image::new(egui::include_image!("../assets/icons/music.svg"))
        .fit_to_exact_size(vec2(23.0, 23.0))
        .tint(tone)
        .paint_at(
            ui,
            Rect::from_min_size(rect.left_top() + vec2(20.0, 18.0), vec2(23.0, 23.0)),
        );
    ui.painter().text(
        rect.left_top() + vec2(56.0, 18.0),
        Align2::LEFT_TOP,
        "Music",
        home_fonts::heading(19.0),
        p.ink,
    );
    ui.painter().text(
        rect.left_top() + vec2(56.0, 48.0),
        Align2::LEFT_TOP,
        "Play local songs and albums",
        FontId::proportional(12.0),
        p.muted,
    );
    if width >= 560.0 {
        ui.painter().text(
            rect.right_center() - vec2(112.0, 0.0),
            Align2::LEFT_CENTER,
            "Choose music  →",
            home_fonts::medium(11.5),
            tone,
        );
    }
    if response.clicked() {
        *action = Some(Action::OpenMusicDialog);
    }
}

fn archive_launch_card(
    ui: &mut egui::Ui,
    p: Palette,
    reduce_motion: bool,
    width: f32,
    action: &mut Option<Action>,
) {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 95.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "Open Archives")
    });
    reveal_focus(&response, reduce_motion);
    let hover = card_hover(ui, &response, reduce_motion);
    let tone = if p.dark {
        Color32::from_rgb(223, 188, 132)
    } else {
        Color32::from_rgb(127, 88, 43)
    };
    let base = if p.dark {
        Color32::from_rgb(47, 39, 32)
    } else {
        Color32::from_rgb(247, 239, 225)
    };
    ui.painter()
        .rect_filled(rect, 12, base.lerp_to_gamma(p.hover, hover * 0.15));
    ui.painter()
        .rect_stroke(rect, 12, Stroke::new(1.0, p.line), StrokeKind::Inside);
    card_focus(ui, &response, p);
    crate::archive_view::icon(23.0, tone).paint_at(
        ui,
        Rect::from_min_size(rect.left_top() + vec2(20.0, 18.0), vec2(23.0, 23.0)),
    );
    ui.painter().text(
        rect.left_top() + vec2(56.0, 18.0),
        Align2::LEFT_TOP,
        "Archives",
        home_fonts::heading(19.0),
        p.ink,
    );
    ui.painter().text(
        rect.left_top() + vec2(56.0, 48.0),
        Align2::LEFT_TOP,
        "Browse and extract ZIP, RAR and 7z",
        FontId::proportional(12.0),
        p.muted,
    );
    if width >= 560.0 {
        ui.painter().text(
            rect.right_center() - vec2(100.0, 0.0),
            Align2::LEFT_CENTER,
            "Open archive  →",
            home_fonts::medium(11.5),
            tone,
        );
    }
    if response.clicked() {
        *action = Some(Action::OpenArchiveDialog);
    }
}

fn workspace_feature(
    ui: &mut egui::Ui,
    p: Palette,
    reduce_motion: bool,
    card: WorkspaceCard<'_>,
    width: f32,
    height: f32,
    dark_card: bool,
    action: &mut Option<Action>,
) {
    let WorkspaceCard {
        workspace,
        label,
        detail,
    } = card;
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    reveal_focus(&response, reduce_motion);
    let hover = card_hover(ui, &response, reduce_motion);
    let ink = if dark_card {
        Color32::from_gray(247)
    } else {
        p.ink
    };
    let muted = if dark_card {
        Color32::from_gray(199)
    } else {
        p.muted
    };
    let base = if dark_card {
        Color32::from_rgb(
            if p.dark { 35 } else { 31 },
            if p.dark { 49 } else { 43 },
            if p.dark { 56 } else { 51 },
        )
    } else if p.dark {
        Color32::from_rgb(31, 48, 59)
    } else {
        Color32::from_rgb(231, 241, 246)
    };
    ui.painter()
        .rect_filled(rect, 14, base.lerp_to_gamma(p.hover, hover * 0.12));
    ui.painter().rect_stroke(
        rect,
        14,
        Stroke::new(
            1.0,
            if dark_card {
                Color32::from_gray(80)
            } else {
                p.line
            },
        ),
        StrokeKind::Inside,
    );
    card_focus(ui, &response, p);
    let x = rect.left() + 21.0;
    ui.painter().text(
        egui::pos2(x, rect.top() + 24.0),
        Align2::LEFT_TOP,
        label,
        home_fonts::heading(22.0),
        ink,
    );
    ui.painter().text(
        egui::pos2(x, rect.top() + 58.0),
        Align2::LEFT_TOP,
        detail,
        FontId::proportional(12.5),
        muted,
    );
    ui.painter().text(
        egui::pos2(x, rect.bottom() - 27.0),
        Align2::LEFT_CENTER,
        format!("Open {label}"),
        home_fonts::medium(11.5),
        ink,
    );
    art::arrow(
        ui.painter(),
        egui::pos2(rect.right() - 20.0, rect.bottom() - 27.0),
        ink,
    );
    if width >= 350.0 {
        art::document(
            &ui.painter().with_clip_rect(rect.shrink(1.0)),
            egui::pos2(rect.right() - 112.0, rect.center().y + 6.0 - 3.0 * hover),
            workspace,
            p.dark,
        );
    } else {
        ui.painter().text(
            egui::pos2(rect.right() - 25.0, rect.top() + 18.0),
            Align2::RIGHT_TOP,
            if dark_card { "PDF" } else { "READ" },
            home_fonts::heading(26.0),
            if dark_card {
                Color32::from_gray(120)
            } else {
                p.accent
            },
        );
    }
    if response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        *action = Some(Action::Enter(workspace));
    }
}

fn workspace_compact(
    ui: &mut egui::Ui,
    p: Palette,
    reduce_motion: bool,
    card: WorkspaceCard<'_>,
    width: f32,
    action: &mut Option<Action>,
) {
    let WorkspaceCard {
        workspace,
        label,
        detail,
    } = card;
    let (rect, response) = ui.allocate_exact_size(vec2(width, 136.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    reveal_focus(&response, reduce_motion);
    let hover = card_hover(ui, &response, reduce_motion);
    let base_tone = match workspace {
        Workspace::Writer => Color32::from_rgb(59, 116, 126),
        Workspace::Sheets => Color32::from_rgb(49, 115, 82),
        Workspace::Slides => Color32::from_rgb(159, 93, 71),
        Workspace::Images => Color32::from_rgb(131, 104, 57),
        Workspace::Video => Color32::from_rgb(151, 70, 64),
        _ => p.accent,
    };
    let tone = workspace.accent(p.dark);
    let fill = if p.dark {
        p.surface.lerp_to_gamma(base_tone, 0.18)
    } else {
        Color32::WHITE.lerp_to_gamma(base_tone, 0.065)
    };
    ui.painter()
        .rect_filled(rect, 12, fill.lerp_to_gamma(p.hover, hover * 0.16));
    ui.painter()
        .rect_stroke(rect, 12, Stroke::new(1.0, p.line), StrokeKind::Inside);
    card_focus(ui, &response, p);
    if width >= 340.0 {
        let icon_rect = Rect::from_min_size(rect.min + vec2(20.0, 18.0), vec2(25.0, 25.0));
        egui::Image::new(workspace.icon())
            .tint(tone)
            .paint_at(ui, icon_rect);
        ui.painter().text(
            rect.min + vec2(20.0, 51.0),
            Align2::LEFT_TOP,
            label,
            home_fonts::heading(20.0),
            p.ink,
        );
        ui.painter().text(
            rect.min + vec2(20.0, 80.0),
            Align2::LEFT_TOP,
            detail,
            FontId::proportional(12.0),
            p.muted,
        );
        ui.painter().text(
            rect.left_bottom() + vec2(20.0, -19.0),
            Align2::LEFT_CENTER,
            format!("Open {label}"),
            home_fonts::medium(11.5),
            tone,
        );
        let art_center = egui::pos2(rect.right() - 87.0, rect.center().y - 1.0 - hover * 3.0);
        art::document(
            &ui.painter().with_clip_rect(rect.shrink(1.0)),
            art_center,
            workspace,
            p.dark,
        );
        art::arrow(ui.painter(), rect.right_bottom() + vec2(-20.0, -19.0), tone);
    } else {
        let icon_rect = Rect::from_min_size(rect.min + vec2(18.0, 16.0), vec2(35.0, 35.0));
        ui.painter().rect_filled(
            icon_rect,
            7,
            tone.gamma_multiply(if p.dark { 0.5 } else { 0.13 }),
        );
        egui::Image::new(workspace.icon())
            .tint(if p.dark { Color32::WHITE } else { tone })
            .paint_at(ui, icon_rect.shrink(8.0));
        ui.painter().text(
            rect.min + vec2(18.0, 68.0),
            Align2::LEFT_TOP,
            label,
            home_fonts::heading(15.5),
            p.ink,
        );
        ui.painter().text(
            rect.min + vec2(18.0, 94.0),
            Align2::LEFT_TOP,
            detail,
            FontId::proportional(11.5),
            p.muted,
        );
        art::arrow(ui.painter(), rect.right_top() + vec2(-18.0, 19.0), tone);
    }
    if response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        *action = Some(Action::Enter(workspace));
    }
}

fn card_hover(ui: &egui::Ui, response: &egui::Response, reduce_motion: bool) -> f32 {
    let active = response.hovered() || response.has_focus();
    if reduce_motion {
        if active { 1.0 } else { 0.0 }
    } else {
        ui.ctx().animate_bool_with_time_and_easing(
            response.id.with("lift"),
            active,
            0.18,
            egui::emath::easing::cubic_out,
        )
    }
}

fn card_focus(ui: &egui::Ui, response: &egui::Response, p: Palette) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.expand(3.0),
            14,
            Stroke::new(1.5, p.accent),
            StrokeKind::Inside,
        );
    }
}

fn welcome_library(ui: &mut egui::Ui, p: Palette, action: &mut Option<Action>) {
    let wide = ui.available_width() > 680.0;
    let ink = Color32::from_gray(245);
    let muted = Color32::from_gray(190);
    egui::Frame::new().fill(Color32::from_gray(if p.dark {39} else {32})).corner_radius(14).inner_margin(24).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_top(|ui| {
            let width = if wide { ui.available_width() * 0.64 } else { ui.available_width() };
            ui.allocate_ui_with_layout(vec2(width, 130.0), Layout::top_down(Align::LEFT), |ui| {
                ui.label(RichText::new("Keep your work within reach.").font(home_fonts::heading(20.0)).color(ink));
                ui.add_space(6.0);
                ui.label(RichText::new("Choose a folder to browse your files here.\nEverything stays on your Mac.").size(13.0).color(muted));
                ui.add_space(14.0);
                let mut theme = crate::ui_kit::theme(p.dark, Workspace::Home);
                theme.palette.primary = ink;
                theme.palette.primary_foreground = Color32::from_gray(28);
                if egui_shadcn::Button::new("Choose folder…")
                    .variant(egui_shadcn::ButtonVariant::Solid)
                    .size(egui_shadcn::ButtonSize::Default)
                    .min_width(143.0)
                    .show(ui, &theme)
                    .clicked()
                {
                    *action = Some(Action::ChooseFolder);
                }
            });
            if wide {
                ui.separator();
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    ui.add_space(5.0);
                    ui.label(RichText::new("Already have a file?").font(home_fonts::medium(14.0)).color(ink));
                    ui.label(RichText::new("Drop it anywhere in Polymath.").size(12.0).color(muted));
                    ui.add_space(24.0);
                    ui.label(RichText::new("PDF   DOCX   XLSX   PPTX\nMD   CSV").size(11.0).color(muted));
                });
            }
        });
    });
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new("Or drag a file into this window")
                .size(12.0)
                .color(p.muted),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new("⌘O  Open file").size(12.0).color(p.muted));
        });
    });
}

fn empty(
    ui: &mut egui::Ui,
    p: Palette,
    title: &str,
    detail: &str,
    button: Option<(&str, Action)>,
    action: &mut Option<Action>,
) {
    egui::Frame::new()
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(14)
        .inner_margin(24)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(148.0);
            ui.vertical_centered(|ui| {
                ui.add_space(9.0);
                let (rect, _) = ui.allocate_exact_size(vec2(38.0, 38.0), Sense::hover());
                ui.painter().rect_filled(rect, 10, p.surface);
                egui::Image::new(Workspace::Reader.icon())
                    .tint(p.muted)
                    .paint_at(ui, rect.shrink(8.0));
                ui.add_space(6.0);
                ui.label(RichText::new(title).size(17.0).strong());
                ui.add(egui::Label::new(RichText::new(detail).size(13.0).color(p.muted)).wrap());
                if let Some((label, value)) = button {
                    ui.add_space(5.0);
                    if egui_shadcn::Button::new(label)
                        .variant(egui_shadcn::ButtonVariant::Outline)
                        .size(egui_shadcn::ButtonSize::Sm)
                        .show(ui, &crate::ui_kit::theme(p.dark, Workspace::Home))
                        .clicked()
                    {
                        *action = Some(value);
                    }
                }
            });
        });
}
fn file_header(ui: &mut egui::Ui, p: Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
    ui.painter().text(
        rect.left_center() + vec2(43.0, 0.0),
        Align2::LEFT_CENTER,
        "Name",
        FontId::proportional(11.5),
        p.muted,
    );
    ui.painter().text(
        rect.right_center() - vec2(15.0, 0.0),
        Align2::RIGHT_CENTER,
        "Kind",
        FontId::proportional(11.5),
        p.muted,
    );
    ui.painter()
        .hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, p.line));
}
fn file_row(ui: &mut egui::Ui, path: &str, reduce_motion: bool, p: Palette) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 57.0), Sense::click());
    let name = basename(path);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &name)
    });
    reveal_focus(&response, reduce_motion);
    if response.hovered() || response.has_focus() {
        ui.painter().rect_filled(rect, 9, p.surface);
    }
    if response.has_focus() {
        ui.painter()
            .rect_stroke(rect, 9, Stroke::new(1.0, p.ink), StrokeKind::Inside);
    }
    let workspace = Workspace::for_path(Path::new(path)).unwrap_or(Workspace::Reader);
    egui::Image::new(workspace.icon()).tint(p.ink).paint_at(
        ui,
        Rect::from_center_size(rect.left_center() + vec2(20.0, 0.0), vec2(22.0, 22.0)),
    );
    let parent = Path::new(path)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let width = (rect.width() - 145.0).max(50.0);
    text_clipped(ui, rect.min + vec2(43.0, 10.0), &name, 14.0, p.ink, width);
    text_clipped(
        ui,
        rect.min + vec2(43.0, 31.0),
        &parent,
        11.5,
        p.muted,
        width,
    );
    let kind = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_uppercase();
    ui.painter().text(
        rect.right_center() - vec2(15.0, 0.0),
        Align2::RIGHT_CENTER,
        kind,
        FontId::proportional(11.5),
        p.muted,
    );
    ui.painter()
        .hline(rect.x_range(), rect.bottom(), Stroke::new(0.5, p.line));
    response
        .on_hover_text(path)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}
fn text_clipped(ui: &egui::Ui, pos: egui::Pos2, text: &str, size: f32, color: Color32, width: f32) {
    let mut job =
        egui::text::LayoutJob::simple_singleline(text.into(), FontId::proportional(size), color);
    job.wrap.max_width = width;
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    ui.painter().galley(pos, galley, color);
}
