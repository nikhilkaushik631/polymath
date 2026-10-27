//! Read-only source and plain-text viewing inside Reader. Files never execute.
use egui::{Align, Color32, FontId, Layout, RichText, Stroke, TextWrapMode, vec2};
use egui_dock::{DockArea, DockState, NodeIndex, Style, TabViewer};
use egui_extras::{Column, TableBuilder};
use std::{fs, path::Path};

const MAX_SOURCE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum SourcePane {
    Document,
    Outline,
}

struct Symbol {
    line: usize,
    label: String,
}

pub struct SourceFile {
    pub path: String,
    pub language: String,
    pub text: String,
    lines: Vec<String>,
    symbols: Vec<Symbol>,
    dock: DockState<SourcePane>,
    pub query: String,
    last_query: String,
    jump_to_line: Option<usize>,
}

impl SourceFile {
    pub fn load(path: &Path) -> Result<Self, String> {
        let metadata =
            fs::metadata(path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
        if !metadata.is_file() {
            return Err(format!("{} is not a file", path.display()));
        }
        if metadata.len() > MAX_SOURCE_BYTES {
            return Err(format!(
                "{} is over the 16 MB source-view limit",
                path.display()
            ));
        }
        let bytes =
            fs::read(path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
        let text = decode_text(&bytes)?;
        let lines: Vec<String> = text
            .split('\n')
            .map(|line| line.trim_end_matches('\r').to_owned())
            .collect();
        let language = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("text")
            .to_ascii_uppercase();
        let symbols = collect_symbols(&lines, &language);
        let mut dock = DockState::new(vec![SourcePane::Document]);
        if !symbols.is_empty() {
            dock.main_surface_mut()
                .split_right(NodeIndex::root(), 0.81, vec![SourcePane::Outline]);
        }
        Ok(Self {
            path: path.to_string_lossy().into_owned(),
            language,
            text,
            lines,
            symbols,
            dock,
            query: String::new(),
            last_query: String::new(),
            jump_to_line: None,
        })
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

fn collect_symbols(lines: &[String], language: &str) -> Vec<Symbol> {
    lines
        .iter()
        .enumerate()
        .filter_map(|(line, value)| {
            let trimmed = value.trim_start();
            let label = if language == "MD" || language == "MARKDOWN" {
                trimmed
                    .starts_with('#')
                    .then_some(trimmed.trim_start_matches('#').trim())
                    .filter(|heading| !heading.is_empty())
            } else if matches!(
                language,
                "PY" | "RS"
                    | "JS"
                    | "JSX"
                    | "TS"
                    | "TSX"
                    | "C"
                    | "H"
                    | "CPP"
                    | "JAVA"
                    | "GO"
                    | "SWIFT"
            ) {
                let declaration = trimmed
                    .strip_prefix("export default ")
                    .or_else(|| trimmed.strip_prefix("export "))
                    .unwrap_or(trimmed);
                let declaration = declaration
                    .strip_prefix("pub(crate) ")
                    .or_else(|| declaration.strip_prefix("pub "))
                    .unwrap_or(declaration);
                let declaration = declaration.strip_prefix("async ").unwrap_or(declaration);
                [
                    "fn ",
                    "def ",
                    "class ",
                    "struct ",
                    "enum ",
                    "interface ",
                    "function ",
                    "impl ",
                    "trait ",
                ]
                .iter()
                .any(|prefix| declaration.starts_with(prefix))
                .then_some(declaration)
            } else {
                None
            };
            label.map(|label| Symbol {
                line,
                label: label.chars().take(90).collect(),
            })
        })
        .collect()
}

fn decode_text(bytes: &[u8]) -> Result<String, String> {
    let text = if let Some(rest) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        std::str::from_utf8(rest)
            .map(str::to_owned)
            .map_err(|_| "The file is not valid UTF-8 text".to_owned())?
    } else if let Some(rest) = bytes.strip_prefix(&[0xff, 0xfe]) {
        decode_utf16(rest, true)?
    } else if let Some(rest) = bytes.strip_prefix(&[0xfe, 0xff]) {
        decode_utf16(rest, false)?
    } else {
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| "The file is not UTF-8 or UTF-16 text".to_owned())?
    };
    if text.contains('\0') {
        return Err("This looks like a binary file, not source text".into());
    }
    Ok(text)
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> Result<String, String> {
    if !bytes.len().is_multiple_of(2) {
        return Err("The UTF-16 file has an incomplete final character".into());
    }
    let units = bytes.chunks_exact(2).map(|pair| {
        let pair = [pair[0], pair[1]];
        if little_endian {
            u16::from_le_bytes(pair)
        } else {
            u16::from_be_bytes(pair)
        }
    });
    String::from_utf16(&units.collect::<Vec<_>>())
        .map_err(|_| "The file contains invalid UTF-16 text".into())
}

pub fn show(ui: &mut egui::Ui, file: &mut SourceFile, dark: bool) -> bool {
    let mut open = false;
    let (canvas, panel, ink, muted, rule) = if dark {
        (
            Color32::from_gray(22),
            Color32::from_gray(30),
            Color32::from_gray(237),
            Color32::from_gray(172),
            Color32::from_gray(58),
        )
    } else {
        (
            Color32::from_gray(250),
            Color32::from_gray(240),
            Color32::from_gray(27),
            Color32::from_gray(95),
            Color32::from_gray(215),
        )
    };
    let compact = ui.available_width() < 760.0;
    egui::Panel::top("source-toolbar")
        .exact_size(if compact { 86.0 } else { 56.0 })
        .frame(
            egui::Frame::new()
                .fill(panel)
                .inner_margin(egui::Margin::symmetric(18, 8))
                .stroke(Stroke::new(1.0, rule)),
        )
        .show(ui, |ui| {
            let name = Path::new(&file.path).file_name().map_or_else(
                || file.path.as_str().to_owned(),
                |n| n.to_string_lossy().into_owned(),
            );
            ui.horizontal_centered(|ui| {
                let name_width = if compact {
                    (ui.available_width() - 170.0).clamp(100.0, 340.0)
                } else {
                    (ui.available_width() - 430.0).clamp(100.0, 340.0)
                };
                ui.add_sized(
                    [name_width, 25.0],
                    egui::Label::new(RichText::new(name).size(15.0).strong().color(ink))
                        .truncate()
                        .halign(Align::LEFT),
                )
                .on_hover_text(&file.path);
                egui_shadcn::badge(
                    ui,
                    &crate::ui_kit::theme(dark, crate::Workspace::Reader),
                    egui_shadcn::BadgeProps::new(&file.language)
                        .variant(egui_shadcn::BadgeVariant::Outline)
                        .size(egui_shadcn::BadgeSize::Size2),
                );
                ui.label(
                    RichText::new(format!("{} lines", file.line_count()))
                        .size(11.0)
                        .color(muted),
                );
                if !compact {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        source_actions(ui, file, &mut open, dark);
                    });
                }
            });
            if compact {
                ui.add_space(4.0);
                ui.horizontal(|ui| source_actions(ui, file, &mut open, dark));
            }
        });

    if file.query != file.last_query {
        file.last_query = file.query.clone();
        let query = file.query.trim().to_lowercase();
        if !query.is_empty() {
            file.jump_to_line = file
                .lines
                .iter()
                .position(|line| line.to_lowercase().contains(&query));
        }
    }
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(canvas)
                .inner_margin(egui::Margin::symmetric(14, 12)),
        )
        .show(ui, |ui| {
            let mut style = Style::from_egui(ui.style().as_ref());
            style.main_surface_border_stroke = Stroke::NONE;
            style.tab_bar.bg_fill = panel;
            style.tab_bar.hline_color = rule;
            style.tab_bar.height = 32.0;
            style.tab.tab_body.bg_fill = canvas;
            style.tab.tab_body.inner_margin = egui::Margin::symmetric(8, 8);
            style.separator.color_idle = rule;
            style.separator.color_hovered = crate::Workspace::Reader.accent(dark);
            let mut viewer = SourceViewer {
                lines: &file.lines,
                symbols: &file.symbols,
                query: &file.query,
                jump_to_line: &mut file.jump_to_line,
                dark,
            };
            DockArea::new(&mut file.dock)
                .style(style)
                .show_close_buttons(false)
                .show_add_buttons(false)
                .show_inside(ui, &mut viewer);
        });
    open
}

struct SourceViewer<'a> {
    lines: &'a [String],
    symbols: &'a [Symbol],
    query: &'a str,
    jump_to_line: &'a mut Option<usize>,
    dark: bool,
}

impl TabViewer for SourceViewer<'_> {
    type Tab = SourcePane;

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        egui::Id::new(*tab)
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab {
            SourcePane::Document => "Document".into(),
            SourcePane::Outline => "Outline".into(),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            SourcePane::Document => self.document(ui),
            SourcePane::Outline => self.outline(ui),
        }
    }

    fn is_closeable(&self, _tab: &Self::Tab) -> bool {
        false
    }

    fn allowed_in_windows(&self, _tab: &mut Self::Tab) -> bool {
        false
    }

    fn scroll_bars(&self, _tab: &Self::Tab) -> [bool; 2] {
        [false, false]
    }
}

impl SourceViewer<'_> {
    fn document(&mut self, ui: &mut egui::Ui) {
        let dark = self.dark;
        let canvas = Color32::from_gray(if dark { 22 } else { 250 });
        let ink = Color32::from_gray(if dark { 237 } else { 27 });
        let muted = Color32::from_gray(if dark { 172 } else { 95 });
        let rule = Color32::from_gray(if dark { 58 } else { 215 });
        ui.label(
            RichText::new("SOURCE  ·  READ ONLY")
                .size(10.0)
                .strong()
                .color(crate::Workspace::Reader.accent(dark)),
        );
        ui.add_space(9.0);
        let query = self.query.trim().to_lowercase();
        let row_height = 22.0;
        let mut scroll = egui::ScrollArea::both()
            .id_salt("source-lines")
            .auto_shrink([false, false]);
        if let Some(index) = self.jump_to_line.take() {
            scroll = scroll.vertical_scroll_offset(index as f32 * row_height);
        }
        egui::Frame::new()
            .fill(canvas)
            .stroke(Stroke::new(1.0, rule))
            .corner_radius(10)
            .inner_margin(egui::Margin::symmetric(14, 12))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                scroll.show_rows(ui, row_height, self.lines.len(), |ui, range| {
                    ui.spacing_mut().item_spacing = vec2(10.0, 0.0);
                    for index in range {
                        let line = &self.lines[index];
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [44.0, row_height],
                                egui::Label::new(
                                    RichText::new((index + 1).to_string())
                                        .font(FontId::monospace(11.0))
                                        .color(muted),
                                )
                                .halign(Align::RIGHT),
                            );
                            let color = if query.is_empty() || line.to_lowercase().contains(&query)
                            {
                                ink
                            } else {
                                muted
                            };
                            ui.add(
                                egui::Label::new(
                                    RichText::new(if line.is_empty() { " " } else { line })
                                        .font(FontId::monospace(12.5))
                                        .color(color),
                                )
                                .selectable(true)
                                .wrap_mode(TextWrapMode::Extend),
                            );
                        });
                    }
                });
            });
    }

    fn outline(&mut self, ui: &mut egui::Ui) {
        let dark = self.dark;
        let ink = Color32::from_gray(if dark { 237 } else { 27 });
        let muted = Color32::from_gray(if dark { 172 } else { 95 });
        ui.add_space(5.0);
        ui.label(
            RichText::new("Jump to a heading or definition")
                .size(11.0)
                .color(muted),
        );
        ui.add_space(10.0);
        TableBuilder::new(ui)
            .id_salt("source-outline")
            .auto_shrink([false, false])
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::exact(45.0))
            .column(Column::remainder().at_least(90.0))
            .body(|body| {
                body.rows(30.0, self.symbols.len(), |mut row| {
                    let symbol = &self.symbols[row.index()];
                    row.col(|ui| {
                        ui.label(
                            RichText::new((symbol.line + 1).to_string())
                                .font(FontId::monospace(10.5))
                                .color(muted),
                        );
                    });
                    row.col(|ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(&symbol.label).size(11.5).color(ink),
                                )
                                .frame(false),
                            )
                            .on_hover_text(format!("Go to line {}", symbol.line + 1))
                            .clicked()
                        {
                            *self.jump_to_line = Some(symbol.line);
                            ui.ctx().request_repaint();
                        }
                    });
                });
            });
    }
}

fn source_actions(ui: &mut egui::Ui, file: &mut SourceFile, open: &mut bool, dark: bool) {
    let theme = crate::ui_kit::theme(dark, crate::Workspace::Reader);
    if egui_shadcn::Button::new("Open another…")
        .variant(egui_shadcn::ButtonVariant::Outline)
        .size(egui_shadcn::ButtonSize::Sm)
        .show(ui, &theme)
        .clicked()
    {
        *open = true;
    }
    if egui_shadcn::Button::new("Copy all")
        .variant(egui_shadcn::ButtonVariant::Ghost)
        .size(egui_shadcn::ButtonSize::Sm)
        .show(ui, &theme)
        .on_hover_text("Copy the source text")
        .clicked()
    {
        ui.ctx().copy_text(file.text.clone());
    }
    egui_shadcn::input_with_props(
        ui,
        &theme,
        egui_shadcn::InputProps::new("source-find", &mut file.query)
            .placeholder("Find in file")
            .width(150.0),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_utf8_utf16_and_rejects_binary() {
        assert_eq!(
            decode_text(b"print('hello')\n"),
            Ok("print('hello')\n".into())
        );
        assert_eq!(decode_text(&[0xff, 0xfe, b'a', 0]), Ok("a".into()));
        assert!(decode_text(b"hello\0world").is_err());
        assert!(decode_text(&[0xff, 0xfe, b'a']).is_err());
    }

    #[test]
    fn outlines_markdown_and_code_without_treating_plain_text_as_code() {
        let lines = vec!["# Chapter".to_owned(), "pub fn read() {}".to_owned()];
        assert_eq!(collect_symbols(&lines, "MD").len(), 1);
        assert_eq!(collect_symbols(&lines, "RS").len(), 1);
        assert!(collect_symbols(&lines, "TXT").is_empty());
    }
}
