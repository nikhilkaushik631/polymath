//! Read-only source and plain-text viewing inside Reader. Files never execute.
use egui::{Align, Color32, FontId, Layout, RichText, Stroke, TextWrapMode, vec2};
use egui_dock::{DockArea, DockState, NodeIndex, Style, TabViewer};
use egui_extras::syntax_highlighting::{self, CodeTheme};
use egui_extras::{Column, TableBuilder};
use std::{
    fs,
    hash::{Hash, Hasher},
    path::Path,
    sync::Arc,
};

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

struct SvgPreview {
    uri: String,
    bytes: Arc<[u8]>,
}

struct DelimitedTable {
    intro: Vec<Vec<String>>,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    widths: Vec<f32>,
    visible_rows: Vec<usize>,
}

pub struct SourceFile {
    pub path: String,
    pub language: String,
    pub text: String,
    lines: Vec<String>,
    symbols: Vec<Symbol>,
    svg: Option<SvgPreview>,
    table: Option<DelimitedTable>,
    dock: DockState<SourcePane>,
    show_outline: bool,
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
        let svg = if language == "SVG" {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            text.hash(&mut hasher);
            Some(SvgPreview {
                uri: format!("bytes://polymath-svg-{:016x}.svg", hasher.finish()),
                bytes: Arc::from(text.as_bytes()),
            })
        } else {
            None
        };
        let table = if language == "CSV" || language == "TSV" {
            Some(DelimitedTable::parse(&text, language == "TSV")?)
        } else {
            None
        };
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
            svg,
            table,
            dock,
            show_outline: false,
            query: String::new(),
            last_query: String::new(),
            jump_to_line: None,
        })
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

impl DelimitedTable {
    fn parse(text: &str, tsv: bool) -> Result<Self, String> {
        let delimiter = if tsv { b'\t' } else { b',' };
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(delimiter)
            .has_headers(false)
            .flexible(true)
            .from_reader(text.as_bytes());
        let records = reader
            .records()
            .map(|record| {
                record
                    .map(|record| record.iter().map(str::to_owned).collect::<Vec<_>>())
                    .map_err(|error| format!("Could not parse the table: {error}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if records.is_empty() {
            return Ok(Self {
                intro: Vec::new(),
                headers: Vec::new(),
                rows: Vec::new(),
                widths: Vec::new(),
                visible_rows: Vec::new(),
            });
        }
        // Exports sometimes start with a few metadata lines before the real
        // header. Pick the first row with the widest schema in the opening
        // sample, keeping those metadata lines visible above the table.
        let widest = records.iter().take(32).map(Vec::len).max().unwrap_or(1);
        let header_index = records
            .iter()
            .take(32)
            .position(|row| row.len() == widest)
            .unwrap_or(0);
        let intro = records[..header_index].to_vec();
        let mut headers = records[header_index].clone();
        let rows = records[header_index + 1..].to_vec();
        let columns = records.iter().map(Vec::len).max().unwrap_or(1).min(256);
        headers.truncate(columns);
        headers.resize_with(columns, String::new);
        for (index, header) in headers.iter_mut().enumerate() {
            if header.trim().is_empty() {
                *header = format!("Column {}", index + 1);
            }
        }
        let mut widths = headers
            .iter()
            .map(|header| 16.0 + header.chars().count() as f32 * 7.5)
            .collect::<Vec<_>>();
        for row in &rows {
            for (index, value) in row.iter().take(columns).enumerate() {
                widths[index] = widths[index].max(18.0 + value.chars().count() as f32 * 7.1);
            }
        }
        for width in &mut widths {
            *width = width.clamp(108.0, 340.0);
        }
        let visible_rows = (0..rows.len()).collect();
        Ok(Self {
            intro,
            headers,
            rows,
            widths,
            visible_rows,
        })
    }

    fn filter(&mut self, query: &str) {
        let needle = query.trim().to_lowercase();
        self.visible_rows = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                (needle.is_empty() || row.iter().any(|cell| cell.to_lowercase().contains(&needle)))
                    .then_some(index)
            })
            .collect();
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
                let count = file.table.as_ref().map_or_else(
                    || format!("{} lines", file.line_count()),
                    |table| {
                        format!(
                            "{} rows · {} columns",
                            table.rows.len(),
                            table.headers.len()
                        )
                    },
                );
                ui.label(RichText::new(count).size(11.0).color(muted));
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
        if let Some(table) = file.table.as_mut() {
            table.filter(&query);
        } else if !query.is_empty() {
            file.jump_to_line = file
                .lines
                .iter()
                .position(|line| line.to_lowercase().contains(&query));
        }
    }
    let filename = Path::new(&file.path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Document");
    let has_outline = !file.symbols.is_empty();
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(canvas)
                .inner_margin(if has_outline {
                    egui::Margin::symmetric(14, 12)
                } else {
                    egui::Margin::symmetric(18, 10)
                }),
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
                filename,
                language: &file.language,
                dark,
            };
            if let Some(table) = &file.table {
                table_preview(ui, table, dark);
            } else if let Some(svg) = &file.svg {
                ui.columns(2, |columns| {
                    viewer.document(&mut columns[0]);
                    svg_preview(&mut columns[1], svg, dark);
                });
            } else if has_outline && file.show_outline {
                DockArea::new(&mut file.dock)
                    .style(style)
                    .show_close_buttons(false)
                    .show_add_buttons(false)
                    .show_inside(ui, &mut viewer);
            } else {
                viewer.document(ui);
            }
        });
    open
}

fn table_preview(ui: &mut egui::Ui, table: &DelimitedTable, dark: bool) {
    let chrome = crate::ui_kit::chrome(dark);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("TABLE PREVIEW")
                .size(11.0)
                .strong()
                .color(crate::Workspace::Reader.accent(dark)),
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new(format!(
                "{} of {} rows · {} columns",
                table.visible_rows.len(),
                table.rows.len(),
                table.headers.len()
            ))
            .size(11.0)
            .color(chrome.muted),
        );
    });
    if !table.intro.is_empty() {
        ui.add_space(5.0);
        egui::Frame::new()
            .fill(chrome.surface)
            .stroke(Stroke::new(1.0, chrome.line))
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for row in table.intro.iter().take(5) {
                        let summary = row
                            .iter()
                            .filter(|cell| !cell.trim().is_empty())
                            .map(String::as_str)
                            .collect::<Vec<_>>()
                            .join(" · ");
                        if !summary.is_empty() {
                            ui.label(RichText::new(summary).size(11.0).color(chrome.muted));
                        }
                    }
                });
            });
    }
    ui.add_space(10.0);
    if table.headers.is_empty() {
        ui.label(RichText::new("This table is empty.").color(chrome.muted));
        return;
    }
    let total_width: f32 = table.widths.iter().sum::<f32>() + table.widths.len() as f32 * 8.0;
    egui::ScrollArea::horizontal()
        .id_salt("source-table-horizontal")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_min_width(total_width.max(ui.available_width()));
            let mut builder = TableBuilder::new(ui)
                .id_salt("source-table")
                .striped(true)
                .resizable(true)
                .auto_shrink([false, false])
                .cell_layout(Layout::left_to_right(Align::Center));
            for width in &table.widths {
                builder = builder.column(Column::initial(*width).at_least(92.0).at_most(520.0));
            }
            builder
                .header(34.0, |mut header| {
                    for label in &table.headers {
                        header.col(|ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(label).size(12.0).strong().color(chrome.ink),
                                )
                                .truncate(),
                            )
                            .on_hover_text(label);
                        });
                    }
                })
                .body(|body| {
                    body.rows(32.0, table.visible_rows.len(), |mut row| {
                        let record = &table.rows[table.visible_rows[row.index()]];
                        for col in 0..table.headers.len() {
                            row.col(|ui| {
                                let value = record.get(col).map(String::as_str).unwrap_or("");
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(value).size(12.0).color(chrome.ink),
                                    )
                                    .truncate()
                                    .selectable(true),
                                )
                                .on_hover_text(value);
                            });
                        }
                    });
                });
        });
}

fn svg_preview(ui: &mut egui::Ui, svg: &SvgPreview, dark: bool) {
    let chrome = crate::ui_kit::chrome(dark);
    ui.label(
        RichText::new("Preview")
            .size(13.0)
            .strong()
            .color(chrome.ink),
    );
    ui.add_space(8.0);
    let size = ui.available_size();
    egui::Frame::new()
        .fill(if dark {
            Color32::from_gray(36)
        } else {
            Color32::WHITE
        })
        .stroke(Stroke::new(1.0, chrome.line))
        .corner_radius(8)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_min_width((size.x - 24.0).max(100.0));
            ui.set_min_height((size.y - 24.0).max(180.0));
            let rect = ui.available_rect_before_wrap();
            let grid = Color32::from_gray(if dark { 48 } else { 237 });
            let step = 24.0;
            let mut x = rect.left();
            while x < rect.right() {
                ui.painter()
                    .vline(x, rect.top()..=rect.bottom(), Stroke::new(0.5, grid));
                x += step;
            }
            let mut y = rect.top();
            while y < rect.bottom() {
                ui.painter()
                    .hline(rect.left()..=rect.right(), y, Stroke::new(0.5, grid));
                y += step;
            }
            ui.centered_and_justified(|ui| {
                ui.add(
                    egui::Image::from_bytes(svg.uri.clone(), svg.bytes.clone())
                        .fit_to_exact_size(ui.available_size() * 0.88)
                        .maintain_aspect_ratio(true)
                        .alt_text("Rendered SVG preview"),
                );
            });
        });
}

struct SourceViewer<'a> {
    lines: &'a [String],
    symbols: &'a [Symbol],
    query: &'a str,
    jump_to_line: &'a mut Option<usize>,
    filename: &'a str,
    language: &'a str,
    dark: bool,
}

impl TabViewer for SourceViewer<'_> {
    type Tab = SourcePane;

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        egui::Id::new(*tab)
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab {
            SourcePane::Document => self.filename.into(),
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
        let muted = Color32::from_gray(if dark { 172 } else { 95 });
        let query = self.query.trim().to_lowercase();
        let row_height = 22.0;
        let code_theme = if dark {
            CodeTheme::dark(12.5)
        } else {
            CodeTheme::light(12.5)
        };
        // Keep the viewer lightweight. Related formats reuse the bundled
        // highlighter's string/comment rules without a grammar runtime.
        let language = match self.language {
            "C" | "H" | "CC" | "CPP" | "HPP" | "JS" | "JSX" | "MJS" | "CJS" | "TS" | "TSX"
            | "CSS" | "SCSS" | "HTML" | "SVG" | "XML" => "cpp",
            "JSON" | "YAML" | "YML" => "toml",
            "RS" => "rust",
            "PY" => "python",
            "TOML" => "toml",
            _ => "text",
        };
        let mut scroll = egui::ScrollArea::both()
            .id_salt("source-lines")
            .auto_shrink([false, false]);
        if let Some(index) = self.jump_to_line.take() {
            scroll = scroll.vertical_scroll_offset(index as f32 * row_height);
        }
        egui::Frame::new()
            .fill(if dark {
                Color32::from_gray(26)
            } else {
                Color32::WHITE
            })
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.set_min_height(ui.available_height());
                // show_rows uses the spacing at call time to calculate visible
                // indices. Keep it identical to the spacing used by each row.
                ui.spacing_mut().item_spacing = vec2(10.0, 0.0);
                scroll.show_rows(ui, row_height, self.lines.len(), |ui, range| {
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
                            let mut highlighted = syntax_highlighting::highlight(
                                ui.ctx(),
                                ui.style(),
                                &code_theme,
                                line,
                                language,
                            );
                            if !query.is_empty() && !line.to_lowercase().contains(&query) {
                                for section in &mut highlighted.sections {
                                    section.format.color = muted;
                                }
                            }
                            ui.add(
                                egui::Label::new(highlighted)
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
    if !file.symbols.is_empty() {
        let label = if file.show_outline { "›" } else { "‹" };
        if egui_shadcn::Button::new(label)
            .variant(egui_shadcn::ButtonVariant::Outline)
            .size(egui_shadcn::ButtonSize::Sm)
            .show(ui, &theme)
            .on_hover_text(if file.show_outline {
                "Hide outline"
            } else {
                "Show outline"
            })
            .clicked()
        {
            file.show_outline = !file.show_outline;
        }
    }
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

    #[test]
    fn csv_preview_preserves_quoted_cells_and_finds_export_header() {
        let mut table = DelimitedTable::parse(
            "Campaign Export\nCampaign Name,example\nid,camera,location\n1,\"North, Gate\",A\n2,South,B\n",
            false,
        )
        .unwrap();
        assert_eq!(table.intro.len(), 2);
        assert_eq!(table.headers, ["id", "camera", "location"]);
        assert_eq!(table.rows[0][1], "North, Gate");
        assert!(table.widths.iter().all(|width| *width >= 108.0));
        table.filter("south");
        assert_eq!(table.visible_rows, [1]);
        table.filter("");
        assert_eq!(table.visible_rows, [0, 1]);
    }
}
