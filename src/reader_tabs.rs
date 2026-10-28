//! One visible tab strip for every file opened in Reader. PDF documents retain the Reader
//! engine's save and recovery path; read-only previews keep their live viewer state in memory.

use crate::{Polymath, Workspace, home_fonts, image_view, media_view, source_view};
use egui::{Align, Color32, Layout, RichText, Stroke, TextWrapMode, vec2};
use pdfcraft_engine::DocId;
use serde_json::{Value, json};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TabId {
    Pdf(DocId),
    Preview(u64),
}

impl TabId {
    pub(crate) fn key(self) -> String {
        match self {
            Self::Pdf(id) => format!("pdf:{}", id.0),
            Self::Preview(id) => format!("preview:{id}"),
        }
    }

    pub(crate) fn parse(key: &str) -> Option<Self> {
        let (kind, id) = key.split_once(':')?;
        let id = id.parse::<u64>().ok()?;
        match kind {
            "pdf" => Some(Self::Pdf(DocId(id))),
            "preview" => Some(Self::Preview(id)),
            _ => None,
        }
    }
}

pub(crate) enum Preview {
    Source(source_view::SourceFile),
    Media(media_view::MediaView),
    Image(image_view::ImageView),
}

impl Preview {
    fn path(&self) -> &str {
        match self {
            Self::Source(file) => &file.path,
            Self::Media(file) => &file.path,
            Self::Image(file) => &file.path,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::Source(_) => "Source",
            Self::Media(_) => "Media",
            Self::Image(_) => "Image",
        }
    }
}

pub(crate) struct PreviewSlot {
    id: u64,
    preview: Preview,
}

#[derive(Clone)]
pub(crate) struct TabInfo {
    pub id: TabId,
    pub title: String,
    pub kind: &'static str,
    pub path: Option<String>,
    pub active: bool,
    pub dirty: bool,
}

impl TabInfo {
    fn json(&self) -> Value {
        json!({"id":self.id.key(),"title":self.title,"kind":self.kind,"path":self.path,"active":self.active,"dirty":self.dirty})
    }
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(path)
        .to_owned()
}

fn compact_tab_title(title: &str) -> String {
    const MAX_CHARS: usize = 36;
    let title: String = title
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    if title.chars().count() <= MAX_CHARS {
        return title;
    }
    let suffix = Path::new(&title)
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| extension.chars().count() <= 7)
        .map(|extension| format!(".{extension}"))
        .unwrap_or_default();
    let prefix_len = MAX_CHARS.saturating_sub(suffix.chars().count() + 1);
    format!(
        "{}…{suffix}",
        title.chars().take(prefix_len).collect::<String>()
    )
}

impl Polymath {
    fn active_preview_path(&self) -> Option<&str> {
        self.source
            .as_ref()
            .map(|file| file.path.as_str())
            .or_else(|| self.media.as_ref().map(|file| file.path.as_str()))
            .or_else(|| self.image_preview.as_ref().map(|file| file.path.as_str()))
    }

    pub(crate) fn active_reader_tab(&self) -> Option<TabId> {
        if self.active_preview_path().is_some() {
            return self.active_preview_id.map(TabId::Preview);
        }
        self.reader.active_ids().map(|(_, id)| TabId::Pdf(id))
    }

    fn take_active_preview(&mut self, ctx: &egui::Context) -> Option<PreviewSlot> {
        let id = self.active_preview_id.take()?;
        let preview = if let Some(file) = self.source.take() {
            Preview::Source(file)
        } else if let Some(mut file) = self.media.take() {
            file.pause();
            if file.fullscreen {
                let _ = file.set_fullscreen(ctx, false);
            }
            Preview::Media(file)
        } else if let Some(mut file) = self.image_preview.take() {
            if file.fullscreen {
                file.set_fullscreen(ctx, false);
            }
            Preview::Image(file)
        } else {
            return None;
        };
        Some(PreviewSlot { id, preview })
    }

    pub(crate) fn stash_reader_preview(&mut self, ctx: &egui::Context) {
        if let Some(slot) = self.take_active_preview(ctx) {
            self.inactive_reader_previews.push(slot);
        }
    }

    fn activate_preview_slot(&mut self, slot: PreviewSlot, ctx: &egui::Context) {
        self.active_preview_id = Some(slot.id);
        match slot.preview {
            Preview::Source(file) => self.source = Some(file),
            Preview::Media(file) => self.media = Some(file),
            Preview::Image(file) => self.image_preview = Some(file),
        }
        if self.active == Workspace::Reader {
            home_fonts::install(ctx);
            self.home_theme(ctx);
        } else {
            self.activate(Workspace::Reader, ctx);
        }
        ctx.request_repaint();
    }

    pub(crate) fn open_reader_preview(&mut self, preview: Preview, ctx: &egui::Context) {
        let path = preview.path().to_owned();
        if let Some(id) = self.find_reader_tab_by_path(&path) {
            let _ = self.select_reader_tab(id, ctx);
            return;
        }
        self.stash_reader_preview(ctx);
        let id = self.next_reader_preview_id;
        self.next_reader_preview_id = self.next_reader_preview_id.saturating_add(1);
        self.reader_tab_order.push(TabId::Preview(id));
        self.activate_preview_slot(PreviewSlot { id, preview }, ctx);
    }

    pub(crate) fn find_reader_tab_by_path(&self, path: &str) -> Option<TabId> {
        if self.active_preview_path() == Some(path) {
            return self.active_preview_id.map(TabId::Preview);
        }
        if let Some(slot) = self
            .inactive_reader_previews
            .iter()
            .find(|slot| slot.preview.path() == path)
        {
            return Some(TabId::Preview(slot.id));
        }
        self.reader.views.iter().find_map(|view| {
            let doc = self.reader.session.get(view.id)?;
            let open_path = self
                .office_previews
                .get(&view.id)
                .map(String::as_str)
                .or(doc.path.as_deref());
            (open_path == Some(path)).then_some(TabId::Pdf(view.id))
        })
    }

    pub(crate) fn select_reader_path(&mut self, path: &str, ctx: &egui::Context) -> bool {
        self.find_reader_tab_by_path(path)
            .is_some_and(|id| self.select_reader_tab(id, ctx))
    }

    pub(crate) fn select_reader_tab(&mut self, id: TabId, ctx: &egui::Context) -> bool {
        match id {
            TabId::Pdf(id) => {
                let Some(index) = self.reader.views.iter().position(|view| view.id == id) else {
                    return false;
                };
                self.stash_reader_preview(ctx);
                self.reader.active = Some(index);
                if self.active == Workspace::Reader {
                    self.reader.activate_in_host();
                } else {
                    self.activate(Workspace::Reader, ctx);
                }
            }
            TabId::Preview(id) => {
                if self.active_preview_id == Some(id) {
                    if self.active != Workspace::Reader {
                        self.activate(Workspace::Reader, ctx);
                    }
                    return true;
                }
                let Some(index) = self
                    .inactive_reader_previews
                    .iter()
                    .position(|slot| slot.id == id)
                else {
                    return false;
                };
                self.stash_reader_preview(ctx);
                let slot = self.inactive_reader_previews.remove(index);
                self.activate_preview_slot(slot, ctx);
            }
        }
        ctx.request_repaint();
        true
    }

    pub(crate) fn sync_reader_tabs(&mut self) {
        let mut live: Vec<TabId> = self
            .reader
            .views
            .iter()
            .map(|view| TabId::Pdf(view.id))
            .collect();
        live.extend(
            self.inactive_reader_previews
                .iter()
                .map(|slot| TabId::Preview(slot.id)),
        );
        if let Some(id) = self.active_preview_id {
            live.push(TabId::Preview(id));
        }
        self.reader_tab_order.retain(|id| live.contains(id));
        for id in live {
            if !self.reader_tab_order.contains(&id) {
                self.reader_tab_order.push(id);
            }
        }
        self.office_previews
            .retain(|id, _| self.reader.views.iter().any(|view| view.id == *id));
    }

    pub(crate) fn reader_tabs(&self) -> Vec<TabInfo> {
        let active = self.active_reader_tab();
        self.reader_tab_order
            .iter()
            .filter_map(|id| match id {
                TabId::Pdf(id) => {
                    let doc = self.reader.session.get(*id)?;
                    let path = self
                        .office_previews
                        .get(id)
                        .cloned()
                        .or_else(|| doc.path.clone());
                    let title = path
                        .as_deref()
                        .map(file_name)
                        .unwrap_or_else(|| doc.display_name());
                    Some(TabInfo {
                        id: TabId::Pdf(*id),
                        title,
                        kind: if self.office_previews.contains_key(id) {
                            "Preview"
                        } else {
                            "PDF"
                        },
                        path,
                        active: active == Some(TabId::Pdf(*id)),
                        dirty: doc.dirty,
                    })
                }
                TabId::Preview(id) => {
                    let (path, kind) = if self.active_preview_id == Some(*id) {
                        if let Some(file) = &self.source {
                            (file.path.as_str(), "Source")
                        } else if let Some(file) = &self.media {
                            (file.path.as_str(), "Media")
                        } else if let Some(file) = &self.image_preview {
                            (file.path.as_str(), "Image")
                        } else {
                            return None;
                        }
                    } else {
                        let slot = self
                            .inactive_reader_previews
                            .iter()
                            .find(|slot| slot.id == *id)?;
                        (slot.preview.path(), slot.preview.kind())
                    };
                    Some(TabInfo {
                        id: TabId::Preview(*id),
                        title: file_name(path),
                        kind,
                        path: Some(path.to_owned()),
                        active: active == Some(TabId::Preview(*id)),
                        dirty: false,
                    })
                }
            })
            .collect()
    }

    pub(crate) fn reader_tabs_json(&self) -> Value {
        json!({"tabs":self.reader_tabs().iter().map(TabInfo::json).collect::<Vec<_>>(),"active":self.active_reader_tab().map(TabId::key)})
    }

    /// A confirmed save/discard can close the last PDF while preview tabs are still open.
    /// Keep one of those tabs in view when the PDF save prompt has finished.
    pub(crate) fn ensure_reader_tab(&mut self, ctx: &egui::Context) {
        self.sync_reader_tabs();
        if self.active_reader_tab().is_none() && self.reader.close_request.is_none() {
            if let Some(id) = self.reader_tab_order.first().copied() {
                let _ = self.select_reader_tab(id, ctx);
            }
        }
    }

    pub(crate) fn close_reader_tab(&mut self, id: TabId, ctx: &egui::Context) -> bool {
        self.sync_reader_tabs();
        let next = self
            .reader_tab_order
            .iter()
            .position(|tab| *tab == id)
            .and_then(|index| {
                self.reader_tab_order
                    .get(index + 1)
                    .or_else(|| {
                        index
                            .checked_sub(1)
                            .and_then(|previous| self.reader_tab_order.get(previous))
                    })
                    .copied()
            });
        match id {
            TabId::Pdf(id) => {
                let Some(index) = self.reader.views.iter().position(|view| view.id == id) else {
                    return false;
                };
                let _ = self.select_reader_tab(TabId::Pdf(id), ctx);
                self.reader.request_close_tab(index);
                if self.reader.close_request.is_none() {
                    if let Some(next) = next.filter(|next| *next != TabId::Pdf(id)) {
                        let _ = self.select_reader_tab(next, ctx);
                    }
                }
            }
            TabId::Preview(id) => {
                if self.active_preview_id == Some(id) {
                    let _ = self.take_active_preview(ctx);
                    self.reader_tab_order
                        .retain(|tab| *tab != TabId::Preview(id));
                    if let Some(next) = next.filter(|next| *next != TabId::Preview(id)) {
                        let _ = self.select_reader_tab(next, ctx);
                    } else if self.active == Workspace::Reader {
                        home_fonts::install(ctx);
                        self.home_theme(ctx);
                    }
                } else if let Some(index) = self
                    .inactive_reader_previews
                    .iter()
                    .position(|slot| slot.id == id)
                {
                    self.inactive_reader_previews.remove(index);
                    self.reader_tab_order
                        .retain(|tab| *tab != TabId::Preview(id));
                } else {
                    return false;
                }
            }
        }
        self.sync_reader_tabs();
        ctx.request_repaint();
        true
    }

    pub(crate) fn reader_tabs_ui(&mut self, ui: &mut egui::Ui) -> bool {
        self.ensure_reader_tab(ui.ctx());
        let tabs = self.reader_tabs();
        if tabs.is_empty() {
            return false;
        }
        let dark = self.dark;
        let chrome = crate::ui_kit::chrome(dark);
        let accent = Workspace::Reader.accent(dark);
        let integrated_source = self.source.is_some() && ui.available_width() >= 740.0;
        if let Some(source) = self.source.as_mut() {
            source.toolbar_in_tab = integrated_source;
        }
        let tab_strip_width = if integrated_source {
            let content_width = tabs
                .iter()
                .map(|tab| {
                    (compact_tab_title(&tab.title).chars().count() as f32 * 7.3).max(78.0) + 63.0
                })
                .sum::<f32>()
                + tabs.len() as f32 * 5.0
                + 38.0;
            content_width.min((ui.available_width() - 365.0).max(200.0))
        } else {
            ui.available_width() - 18.0
        };
        let mut selected = None;
        let mut closed = None;
        let mut open_requested = false;
        egui::Panel::top("polymath-reader-tabs")
            .exact_size(40.0)
            .frame(
                egui::Frame::NONE
                    .fill(chrome.surface)
                    .stroke(Stroke::new(1.0, chrome.line))
                    .inner_margin(egui::Margin::symmetric(9, 3)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.allocate_ui_with_layout(
                        vec2(tab_strip_width.min(ui.available_width()), 32.0),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            egui::ScrollArea::horizontal()
                                .id_salt("reader-open-files")
                                .auto_shrink([false, true])
                                .show(ui, |ui| {
                                    ui.horizontal_centered(|ui| {
                                        ui.spacing_mut().item_spacing.x = 5.0;
                                        for tab in &tabs {
                                            let fill = if tab.active {
                                                chrome.canvas
                                            } else {
                                                chrome.surface
                                            };
                                            let tab_frame = egui::Frame::NONE
                                                .fill(fill)
                                                .inner_margin(egui::Margin::symmetric(7, 3))
                                                .show(ui, |ui| {
                                                    ui.horizontal_centered(|ui| {
                                                        ui.spacing_mut().item_spacing.x = 2.0;
                                                        let title = compact_tab_title(&tab.title);
                                                        let icon = match tab.kind {
                                                            "Source" => "file-code",
                                                            "Image" => "image",
                                                            "Media" => "film",
                                                            _ => "file-text",
                                                        };
                                                        ui.add(pdfcraft_ui_egui::icons::image(
                                                            icon,
                                                            14.0,
                                                            if tab.active {
                                                                accent
                                                            } else {
                                                                chrome.muted
                                                            },
                                                        ));
                                                        if ui
                                                            .add(
                                                                egui::Button::new(
                                                                    RichText::new(title)
                                                                        .size(13.0)
                                                                        .color(if tab.active {
                                                                            chrome.ink
                                                                        } else {
                                                                            chrome.muted
                                                                        }),
                                                                )
                                                                .fill(Color32::TRANSPARENT)
                                                                .stroke(Stroke::NONE)
                                                                .wrap_mode(TextWrapMode::Extend)
                                                                .min_size(vec2(78.0, 25.0)),
                                                            )
                                                            .on_hover_text(
                                                                tab.path
                                                                    .as_deref()
                                                                    .unwrap_or(&tab.title),
                                                            )
                                                            .clicked()
                                                        {
                                                            selected = Some(tab.id);
                                                        }
                                                        if tab.dirty {
                                                            ui.label(
                                                                RichText::new("•")
                                                                    .size(12.0)
                                                                    .color(accent),
                                                            );
                                                        }
                                                        let close_label =
                                                            format!("Close {}", tab.title);
                                                        let response = ui.add(
                                                            egui::Button::image(
                                                                pdfcraft_ui_egui::icons::image(
                                                                    "x",
                                                                    13.0,
                                                                    chrome.muted,
                                                                ),
                                                            )
                                                            .frame(false)
                                                            .min_size(vec2(22.0, 24.0)),
                                                        );
                                                        response.widget_info(|| {
                                                            egui::WidgetInfo::labeled(
                                                                egui::WidgetType::Button,
                                                                true,
                                                                &close_label,
                                                            )
                                                        });
                                                        if response
                                                            .on_hover_text(close_label)
                                                            .clicked()
                                                        {
                                                            closed = Some(tab.id);
                                                        }
                                                    });
                                                });
                                            if tab.active {
                                                let rect = tab_frame.response.rect;
                                                ui.painter().hline(
                                                    rect.left()..=rect.right(),
                                                    rect.bottom() - 1.0,
                                                    Stroke::new(2.0, accent),
                                                );
                                            }
                                        }
                                        let response = ui.add(
                                            egui::Button::image(pdfcraft_ui_egui::icons::image(
                                                "plus",
                                                15.0,
                                                chrome.muted,
                                            ))
                                            .frame(false)
                                            .min_size(vec2(30.0, 28.0)),
                                        );
                                        response.widget_info(|| {
                                            egui::WidgetInfo::labeled(
                                                egui::WidgetType::Button,
                                                true,
                                                "Open file in a new Reader tab",
                                            )
                                        });
                                        open_requested = response
                                            .on_hover_text("Open file in a new tab · ⌘O")
                                            .clicked();
                                    });
                                });
                        },
                    );
                    if integrated_source {
                        ui.separator();
                        if let Some(source) = self.source.as_mut() {
                            source_view::tab_toolbar(ui, source, dark);
                        }
                    }
                });
            });
        let ctx = ui.ctx().clone();
        if let Some(id) = closed {
            let _ = self.close_reader_tab(id, &ctx);
        } else if let Some(id) = selected {
            let _ = self.select_reader_tab(id, &ctx);
        }
        open_requested
    }
}
