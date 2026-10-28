//! Read-only archive browsing and explicit, path-safe extraction.
use crate::{Workspace, ui_kit};
use egui::{Align, Color32, Layout, RichText, Stroke, vec2};
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    time::Duration,
};

pub const EXTENSIONS: &[&str] = &["zip", "rar", "7z"];

pub fn icon(size: f32, tint: Color32) -> egui::Image<'static> {
    egui::Image::new(egui::include_image!("../assets/icons/archive.svg"))
        .fit_to_exact_size(vec2(size, size))
        .tint(tint)
}

pub fn is_archive(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| EXTENSIONS.iter().any(|item| ext.eq_ignore_ascii_case(item)))
}

#[derive(Clone)]
struct Entry {
    name: String,
    relative: Option<PathBuf>,
    directory: bool,
    regular: bool,
}

impl Entry {
    fn extractable(&self) -> bool {
        self.relative.is_some() && (self.directory || self.regular)
    }
}

enum Phase {
    Loading,
    Ready,
    Failed(String),
}

enum Progress {
    Item(usize, usize),
    Done(Result<PathBuf, String>),
}

struct Extraction {
    receiver: Receiver<Progress>,
    cancel: Arc<AtomicBool>,
    current: usize,
    total: usize,
}

pub struct ArchiveView {
    pub path: String,
    entries: Vec<Entry>,
    selected: HashSet<usize>,
    search: String,
    phase: Phase,
    listing: Option<Receiver<Result<Vec<Entry>, String>>>,
    destination: Option<(Vec<Entry>, Receiver<Option<PathBuf>>)>,
    extraction: Option<Extraction>,
    notice: String,
}

impl ArchiveView {
    pub fn open(path: &Path, ctx: &egui::Context) -> Result<Self, String> {
        if !is_archive(path) || !path.is_file() {
            return Err("Choose an existing ZIP, RAR, or 7z archive.".into());
        }
        let (tx, rx) = mpsc::channel();
        let archive = path.to_path_buf();
        let wake = ctx.clone();
        std::thread::Builder::new()
            .name("Polymath archive listing".into())
            .spawn(move || {
                let _ = tx.send(list_entries(&archive));
                wake.request_repaint();
            })
            .map_err(|error| format!("Could not start archive listing: {error}"))?;
        Ok(Self {
            path: path.to_string_lossy().into_owned(),
            entries: Vec::new(),
            selected: HashSet::new(),
            search: String::new(),
            phase: Phase::Loading,
            listing: Some(rx),
            destination: None,
            extraction: None,
            notice: String::new(),
        })
    }

    pub fn inspect(&self) -> serde_json::Value {
        serde_json::json!({
            "path": self.path,
            "state": match &self.phase { Phase::Loading => "loading", Phase::Ready => "ready", Phase::Failed(_) => "error" },
            "entries": self.entries.len(),
            "selected": self.selected.len(),
            "extracting": self.extraction.is_some(),
            "notice": self.notice,
        })
    }

    fn selected_entries(&self) -> Vec<Entry> {
        let folders: Vec<_> = self
            .selected
            .iter()
            .filter_map(|index| self.entries.get(*index))
            .filter(|entry| entry.directory && entry.extractable())
            .filter_map(|entry| entry.relative.as_ref())
            .collect();
        self.entries
            .iter()
            .enumerate()
            .filter(|(index, entry)| {
                entry.extractable()
                    && (self.selected.contains(index)
                        || entry.relative.as_ref().is_some_and(|path| {
                            folders.iter().any(|folder| path.starts_with(folder))
                        }))
            })
            .map(|(_, entry)| entry.clone())
            .collect()
    }

    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(receiver) = &self.listing {
            match receiver.try_recv() {
                Ok(Ok(entries)) => {
                    self.entries = entries;
                    self.phase = Phase::Ready;
                    self.listing = None;
                }
                Ok(Err(error)) => {
                    self.phase = Phase::Failed(error);
                    self.listing = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.phase = Phase::Failed("Archive listing stopped unexpectedly.".into());
                    self.listing = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some((entries, receiver)) = &self.destination {
            match receiver.try_recv() {
                Ok(Some(folder)) => {
                    let entries = entries.clone();
                    self.destination = None;
                    self.start_extract(folder, entries, ctx);
                }
                Ok(None) | Err(mpsc::TryRecvError::Disconnected) => self.destination = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(extraction) = &mut self.extraction {
            loop {
                match extraction.receiver.try_recv() {
                    Ok(Progress::Item(current, total)) => {
                        extraction.current = current;
                        extraction.total = total;
                    }
                    Ok(Progress::Done(result)) => {
                        self.notice = match result {
                            Ok(path) => format!("Extracted to {}", path.display()),
                            Err(error) => error,
                        };
                        self.extraction = None;
                        break;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.notice = "Archive extraction stopped unexpectedly.".into();
                        self.extraction = None;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                }
            }
        }
        if self.listing.is_some() || self.destination.is_some() || self.extraction.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn choose_destination(
        &mut self,
        entries: Vec<Entry>,
        frame: &eframe::Frame,
        ctx: &egui::Context,
    ) {
        if entries.is_empty() || self.destination.is_some() || self.extraction.is_some() {
            return;
        }
        let dialog = rfd::AsyncFileDialog::new()
            .set_parent(frame)
            .set_title("Choose where to extract the archive");
        let (tx, rx) = mpsc::channel();
        let wake = ctx.clone();
        match std::thread::Builder::new()
            .name("Polymath extraction destination".into())
            .spawn(move || {
                let answer =
                    pollster::block_on(dialog.pick_folder()).map(|item| item.path().to_path_buf());
                let _ = tx.send(answer);
                wake.request_repaint();
            }) {
            Ok(_) => self.destination = Some((entries, rx)),
            Err(error) => self.notice = format!("Could not open folder picker: {error}"),
        }
    }

    fn start_extract(&mut self, folder: PathBuf, entries: Vec<Entry>, ctx: &egui::Context) {
        let archive = PathBuf::from(&self.path);
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let wake = ctx.clone();
        let total = entries.len();
        match std::thread::Builder::new()
            .name("Polymath archive extraction".into())
            .spawn(move || {
                let result = extract_entries(&archive, &folder, &entries, &stop, &tx);
                let _ = tx.send(Progress::Done(result));
                wake.request_repaint();
            }) {
            Ok(_) => {
                self.notice.clear();
                self.extraction = Some(Extraction {
                    receiver: rx,
                    cancel,
                    current: 0,
                    total,
                });
            }
            Err(error) => self.notice = format!("Could not start extraction: {error}"),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame, dark: bool) {
        self.poll(ui.ctx());
        let chrome = ui_kit::chrome(dark);
        let accent = Workspace::Reader.accent(dark);
        let theme = ui_kit::theme(dark, Workspace::Reader);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(chrome.canvas).inner_margin(0))
            .show(ui, |ui| {
                egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 18)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new("Archive").size(24.0).strong());
                            let count = self.entries.len();
                            let kind = Path::new(&self.path).extension().and_then(|ext| ext.to_str()).unwrap_or("Archive").to_ascii_uppercase();
                            ui.label(RichText::new(format!("{kind} · {count} items · Read only")).size(12.0).color(chrome.muted));
                            let skipped = self.entries.iter().filter(|entry| !entry.extractable()).count();
                            if skipped > 0 {
                                ui.label(RichText::new(format!("{skipped} unsafe or unsupported items will be skipped"))
                                    .size(11.5)
                                    .color(if dark { Color32::from_rgb(232, 190, 119) } else { Color32::from_rgb(139, 88, 31) }));
                            }
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let all: Vec<_> = self.entries.iter().filter(|entry| entry.extractable()).cloned().collect();
                            let button = ui.add_enabled_ui(
                                matches!(self.phase, Phase::Ready) && !all.is_empty() && self.extraction.is_none(),
                                |ui| egui_shadcn::Button::new("Extract all…")
                                    .variant(egui_shadcn::ButtonVariant::Solid)
                                    .show(ui, &theme),
                            ).inner;
                            if button.clicked() { self.choose_destination(all, frame, ui.ctx()); }
                        });
                    });
                    ui.add_space(20.0);
                    match &self.phase {
                        Phase::Loading => { ui.spinner(); ui.label("Reading archive contents…"); }
                        Phase::Failed(error) => {
                            ui.colored_label(chrome.muted, "This archive could not be opened.");
                            ui.label(error);
                        }
                        Phase::Ready => {
                            egui::Frame::new()
                                .fill(chrome.surface)
                                .stroke(Stroke::new(1.0, chrome.line))
                                .corner_radius(12)
                                .inner_margin(14)
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.add(pdfcraft_ui_egui::icons::image("search", 17.0, chrome.muted));
                                        ui.add(egui::TextEdit::singleline(&mut self.search)
                                            .hint_text("Find in archive")
                                            .desired_width((ui.available_width() - 170.0).max(130.0))
                                            .frame(egui::Frame::NONE));
                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            let selected = self.selected_entries();
                                            let button = ui.add_enabled_ui(
                                                !selected.is_empty() && self.extraction.is_none(),
                                                |ui| egui_shadcn::Button::new(format!("Extract selected ({})…", selected.len()))
                                                    .variant(egui_shadcn::ButtonVariant::Outline)
                                                    .size(egui_shadcn::ButtonSize::Sm)
                                                    .show(ui, &theme)
                                            ).inner;
                                            if button.clicked() { self.choose_destination(selected, frame, ui.ctx()); }
                                        });
                                    });
                                    ui.separator();
                                    let query = self.search.to_ascii_lowercase();
                                    let visible: Vec<_> = self.entries.iter().enumerate()
                                        .filter(|(_, entry)| entry.name.to_ascii_lowercase().contains(&query))
                                        .collect();
                                    if visible.is_empty() {
                                        ui.label(RichText::new("No matching files").color(chrome.muted));
                                    } else {
                                        egui::ScrollArea::vertical()
                                            .id_salt(("archive-files", &self.path))
                                            .max_height((ui.available_height() - 112.0).max(160.0))
                                            .show_rows(ui, 32.0, visible.len(), |ui, rows| {
                                                for row_index in rows {
                                                    let (index, entry) = visible[row_index];
                                                    let usable = entry.extractable();
                                                    let selected = self.selected.contains(&index);
                                                    let icon = if entry.directory { "folder" } else { "file-text" };
                                                    let row = ui.add_enabled(usable, egui::Button::image_and_text(
                                                        pdfcraft_ui_egui::icons::image(icon, 16.0, if usable { accent } else { chrome.muted }),
                                                        RichText::new(&entry.name).size(12.5).color(if usable { chrome.ink } else { chrome.muted }),
                                                    ).selected(selected)
                                                        .fill(if selected { chrome.hover } else { Color32::TRANSPARENT })
                                                        .stroke(Stroke::NONE)
                                                        .wrap_mode(egui::TextWrapMode::Truncate)
                                                        .min_size(vec2(ui.available_width(), 31.0)))
                                                        .on_hover_text(if usable { &entry.name } else { "Unsafe path or unsupported entry type" });
                                                    if row.clicked() {
                                                        if selected { self.selected.remove(&index); }
                                                        else { self.selected.insert(index); }
                                                    }
                                                }
                                            });
                                    }
                                });
                        }
                    }
                    if let Some(extraction) = &self.extraction {
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(format!("Extracting {} of {}", extraction.current, extraction.total));
                            if ui.small_button("Cancel").clicked() { extraction.cancel.store(true, Ordering::Relaxed); }
                        });
                    } else if !self.notice.is_empty() {
                        ui.add_space(13.0);
                        ui.label(RichText::new(&self.notice).size(12.0).color(chrome.muted));
                    }
                });
            });
    }
}

impl Drop for ArchiveView {
    fn drop(&mut self) {
        if let Some(extraction) = &self.extraction {
            extraction.cancel.store(true, Ordering::Relaxed);
        }
    }
}

fn listing_output(archive: &Path, mode: &str) -> Result<Vec<u8>, String> {
    const MAX_BYTES: u64 = 16 * 1024 * 1024;
    let mut child = Command::new("/usr/bin/bsdtar")
        .arg(mode)
        .arg(archive)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("Could not run macOS archive reader: {error}"))?;
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("Archive reader did not return a listing.".into());
    };
    let mut bytes = Vec::new();
    if let Err(error) = stdout.take(MAX_BYTES + 1).read_to_end(&mut bytes) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!("Could not read archive listing: {error}"));
    }
    if bytes.len() as u64 > MAX_BYTES {
        let _ = child.kill();
        let _ = child.wait();
        return Err("Archive has too many entries to list.".into());
    }
    let status = child
        .wait()
        .map_err(|error| format!("Archive reader stopped: {error}"))?;
    if !status.success() {
        return Err("This archive is damaged, password protected, or unsupported.".into());
    }
    Ok(bytes)
}

fn list_entries(archive: &Path) -> Result<Vec<Entry>, String> {
    let list = listing_output(archive, "-tf")?;
    let verbose = listing_output(archive, "-tvf")?;
    let names =
        String::from_utf8(list).map_err(|_| "Archive has names that are not UTF-8.".to_owned())?;
    let types = String::from_utf8(verbose)
        .map_err(|_| "Archive entry details are not UTF-8.".to_owned())?;
    let names: Vec<_> = names.lines().collect();
    let types: Vec<_> = types.lines().collect();
    if names.len() != types.len() || names.len() > 20_000 {
        return Err("Archive has too many entries or an unsupported listing.".into());
    }
    let mut entries: Vec<Entry> = names
        .into_iter()
        .zip(types)
        .map(|(name, detail)| {
            let kind = detail.chars().next().unwrap_or('?');
            Entry {
                name: name.to_owned(),
                relative: safe_relative(name),
                directory: kind == 'd' || name.ends_with('/'),
                regular: kind == '-',
            }
        })
        .collect();
    let mut seen = HashSet::new();
    let mut duplicates = HashSet::new();
    for entry in &entries {
        if let Some(path) = &entry.relative
            && !seen.insert(path.clone())
        {
            duplicates.insert(path.clone());
        }
    }
    for entry in &mut entries {
        if entry
            .relative
            .as_ref()
            .is_some_and(|path| duplicates.contains(path))
        {
            entry.relative = None;
        }
    }
    Ok(entries)
}

fn safe_relative(name: &str) -> Option<PathBuf> {
    if name.is_empty()
        || name.chars().any(char::is_control)
        || name.contains(['*', '?', '[', ']', '\\', ':'])
    {
        return None;
    }
    let mut result = PathBuf::new();
    for component in Path::new(name).components() {
        match component {
            Component::Normal(part) => result.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!result.as_os_str().is_empty()).then_some(result)
}

fn extract_entries(
    archive: &Path,
    destination: &Path,
    entries: &[Entry],
    cancel: &AtomicBool,
    progress: &mpsc::Sender<Progress>,
) -> Result<PathBuf, String> {
    let stem = archive
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Archive");
    let mut output = None;
    for number in 1..=1000 {
        let name = if number == 1 {
            format!("{stem} extracted")
        } else {
            format!("{stem} extracted {number}")
        };
        let path = destination.join(name);
        match fs::create_dir(&path) {
            Ok(()) => {
                output = Some(path);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(format!("Could not create extraction folder: {error}")),
        }
    }
    let root = output.ok_or_else(|| "Could not find a free extraction folder name.".to_owned())?;
    for (index, entry) in entries.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(format!(
                "Extraction cancelled. Partial files remain in {}",
                root.display()
            ));
        }
        let relative = entry
            .relative
            .as_ref()
            .ok_or_else(|| "An unsafe archive path was selected.".to_owned())?;
        let target = root.join(relative);
        if entry.directory {
            fs::create_dir_all(&target).map_err(|error| {
                format!("Could not create folder {}: {error}", target.display())
            })?;
        } else if entry.regular {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    format!("Could not create folder {}: {error}", parent.display())
                })?;
            }
            let mut child = Command::new("/usr/bin/bsdtar")
                .arg("-xOf")
                .arg(archive)
                .arg("--")
                .arg(&entry.name)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|error| format!("Could not extract {}: {error}", entry.name))?;
            let Some(mut stdout) = child.stdout.take() else {
                return Err("Extractor did not return file data.".into());
            };
            let mut file = match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
            {
                Ok(file) => file,
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("Could not create {}: {error}", target.display()));
                }
            };
            let result = (|| -> Result<(), String> {
                let mut buffer = [0u8; 64 * 1024];
                loop {
                    if cancel.load(Ordering::Relaxed) {
                        let _ = child.kill();
                        return Err("Extraction cancelled.".into());
                    }
                    let count = stdout
                        .read(&mut buffer)
                        .map_err(|error| error.to_string())?;
                    if count == 0 {
                        break;
                    }
                    file.write_all(&buffer[..count])
                        .map_err(|error| error.to_string())?;
                }
                let status = child.wait().map_err(|error| error.to_string())?;
                if !status.success() {
                    return Err(format!(
                        "Could not extract {}. The archive may be damaged or password protected.",
                        entry.name
                    ));
                }
                Ok(())
            })();
            if let Err(error) = result {
                let _ = child.kill();
                let _ = child.wait();
                drop(file);
                let _ = fs::remove_file(&target);
                return Err(format!(
                    "{error} Partial files remain in {}",
                    root.display()
                ));
            }
        }
        let _ = progress.send(Progress::Item(index + 1, entries.len()));
    }
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_traversal_and_archive_globs() {
        for path in ["../bad", "/absolute", "a/../../bad", "a*.txt", "a\\bad", ""] {
            assert!(safe_relative(path).is_none(), "{path}");
        }
        assert_eq!(safe_relative("./a/b.txt"), Some(PathBuf::from("a/b.txt")));
    }

    #[test]
    fn selecting_a_folder_includes_its_children() {
        let view = ArchiveView {
            path: "sample.zip".into(),
            entries: vec![
                Entry {
                    name: "photos/".into(),
                    relative: safe_relative("photos/"),
                    directory: true,
                    regular: false,
                },
                Entry {
                    name: "photos/one.txt".into(),
                    relative: safe_relative("photos/one.txt"),
                    directory: false,
                    regular: true,
                },
                Entry {
                    name: "other.txt".into(),
                    relative: safe_relative("other.txt"),
                    directory: false,
                    regular: true,
                },
            ],
            selected: [0].into_iter().collect(),
            search: String::new(),
            phase: Phase::Ready,
            listing: None,
            destination: None,
            extraction: None,
            notice: String::new(),
        };
        assert_eq!(view.selected_entries().len(), 2);
    }

    #[test]
    fn lists_and_extracts_nested_zip_to_a_new_folder() {
        let root = std::env::temp_dir().join(format!(
            "polymath-archive-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos()),
        ));
        let source = root.join("source");
        let nested = source.join("nested");
        assert!(fs::create_dir_all(&nested).is_ok());
        assert!(fs::write(nested.join("note.txt"), b"from archive").is_ok());
        let archive = root.join("sample.zip");
        let status = Command::new("/usr/bin/zip")
            .current_dir(&source)
            .arg("-qr")
            .arg(&archive)
            .arg("nested")
            .status();
        assert!(status.is_ok_and(|status| status.success()));
        let entries = list_entries(&archive);
        assert!(entries.is_ok());
        let entries = entries.unwrap_or_default();
        assert!(
            entries
                .iter()
                .any(|entry| entry.name == "nested/note.txt" && entry.extractable())
        );
        let (tx, _) = mpsc::channel();
        let output = extract_entries(&archive, &root, &entries, &AtomicBool::new(false), &tx);
        assert!(output.is_ok());
        let content = output
            .ok()
            .and_then(|folder| fs::read(folder.join("nested/note.txt")).ok());
        assert_eq!(content.as_deref(), Some(b"from archive".as_slice()));
        let _ = fs::remove_dir_all(root);
    }
}
