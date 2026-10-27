//! Local file discovery, limited to folders explicitly selected in the native folder picker.
//! Scanning rules match the editor modules; the suite renders its own library UI.

use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, SystemTime};

const MAX_FOLDERS: usize = 8;
const MAX_ENTRIES: usize = 20_000;
const MAX_FILES: usize = 100;

#[derive(Clone, Debug, serde::Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    #[serde(skip)]
    modified: Option<SystemTime>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Listing {
    pub folder: String,
    pub files: Vec<FileEntry>,
    pub unavailable: bool,
}

#[derive(Default)]
pub struct LocalFolders {
    paths: Vec<String>,
    pub listings: Vec<Listing>,
    pending: Option<Receiver<Vec<Listing>>>,
    last_scan: Option<f64>,
}

pub fn add(folders: &mut Vec<String>, path: String) -> Result<(), String> {
    if path.len() > 4096 || path.contains('\0') || !Path::new(&path).is_absolute() {
        return Err("Choose a folder using the folder picker.".into());
    }
    if folders.contains(&path) {
        return Ok(());
    }
    if folders.len() >= MAX_FOLDERS {
        return Err("You can choose up to eight folders. Remove one before adding another.".into());
    }
    folders.push(path);
    Ok(())
}

/// Malformed saved folder preferences must not discard the app's other preferences.
pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    use serde::Deserialize;
    let value = serde_json::Value::deserialize(d)?;
    let mut folders = Vec::new();
    if let Some(items) = value.as_array() {
        for path in items
            .iter()
            .filter_map(serde_json::Value::as_str)
            .take(MAX_ENTRIES)
        {
            let _ = add(&mut folders, path.to_owned());
            if folders.len() == MAX_FOLDERS {
                break;
            }
        }
    }
    Ok(folders)
}

fn list(folder: &str, extensions: &[&str]) -> Listing {
    let mut listing = Listing {
        folder: folder.into(),
        files: Vec::new(),
        unavailable: false,
    };
    let entries = match std::fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(_) => {
            listing.unavailable = true;
            return listing;
        }
    };
    for entry in entries.take(MAX_ENTRIES) {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        // No recursion, hidden files or symlinks into other folders. Listing reads metadata only.
        if name.starts_with('.')
            || !entry.file_type().is_ok_and(|kind| kind.is_file())
            || !path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                extensions
                    .iter()
                    .any(|allowed| e.eq_ignore_ascii_case(allowed))
            })
        {
            continue;
        }
        listing.files.push(FileEntry {
            name,
            path: path.to_string_lossy().into_owned(),
            modified: entry.metadata().ok().and_then(|m| m.modified().ok()),
        });
    }
    listing.files.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.name.cmp(&b.name))
    });
    listing.files.truncate(MAX_FILES);
    listing
}

impl LocalFolders {
    pub fn refresh(&mut self) {
        self.last_scan = None;
    }

    pub fn forget(&mut self, path: &str) {
        self.listings.retain(|listing| listing.folder != path);
        self.paths.clear();
        self.pending = None;
        self.refresh();
    }

    pub fn update(
        &mut self,
        ctx: &egui::Context,
        folders: &[String],
        extensions: &'static [&'static str],
    ) {
        if self.paths != folders {
            self.paths = folders.to_vec();
            // Dropping an older result prevents removed folders from reappearing.
            self.pending = None;
            self.listings.clear();
            self.refresh();
        }
        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok(listings) => {
                    self.listings = listings;
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.pending = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if folders.is_empty() {
            return;
        }
        ctx.request_repaint_after(Duration::from_secs(5));
        let now = ctx.input(|input| input.time);
        if self.pending.is_some()
            || self
                .last_scan
                .is_some_and(|last| now >= last && now - last < 5.0)
        {
            return;
        }
        self.last_scan = Some(now);
        let paths = folders.to_vec();
        let wake = ctx.clone();
        let (tx, rx) = mpsc::channel();
        match std::thread::Builder::new()
            .name("Polymath local folders".into())
            .spawn(move || {
                let listings = paths.iter().map(|path| list(path, extensions)).collect();
                let _ = tx.send(listings);
                wake.request_repaint();
            }) {
            Ok(_) => self.pending = Some(rx),
            Err(_) => {
                self.listings = folders
                    .iter()
                    .map(|path| Listing {
                        folder: path.clone(),
                        files: Vec::new(),
                        unavailable: true,
                    })
                    .collect()
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn preferences_are_bounded_and_invalid_paths_are_ignored() {
        #[derive(serde::Deserialize)]
        struct Prefs {
            #[serde(deserialize_with = "deserialize")]
            folders: Vec<String>,
        }
        let root = std::env::temp_dir();
        let path = root
            .join("iris-picked-folder")
            .to_string_lossy()
            .into_owned();
        let prefs: Prefs =
            serde_json::from_value(serde_json::json!({"folders":[null, "relative", path, path]}))
                .unwrap();
        assert_eq!(prefs.folders, vec![path]);
        let mut folders = Vec::new();
        for i in 0..100 {
            let _ = add(
                &mut folders,
                root.join(format!("iris-folder-{i}"))
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        assert_eq!(folders.len(), MAX_FOLDERS);
    }

    #[test]
    fn listing_stays_inside_the_chosen_folder_and_filters_files() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("iris-folders-{}-{unique}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let picked = root.join("picked");
        std::fs::create_dir(&picked).unwrap();
        for name in ["visible.DOCX", ".hidden.docx", "notes.txt", "component.TSX"] {
            std::fs::write(picked.join(name), b"fixture").unwrap();
        }
        std::fs::create_dir(picked.join("nested")).unwrap();
        std::fs::write(picked.join("nested/child.docx"), b"fixture").unwrap();
        std::fs::write(root.join("outside.docx"), b"fixture").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("outside.docx"), picked.join("linked.docx")).unwrap();
        let listing = list(&picked.to_string_lossy(), &["docx"]);
        assert!(!listing.unavailable);
        assert_eq!(
            listing
                .files
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>(),
            ["visible.DOCX"]
        );
        let source_listing = list(&picked.to_string_lossy(), crate::workspace::EXTENSIONS);
        assert!(
            source_listing
                .files
                .iter()
                .any(|file| file.name == "component.TSX")
        );
        std::fs::remove_dir_all(&root).unwrap();
        assert!(list(&picked.to_string_lossy(), &["docx"]).unavailable);
    }
}
