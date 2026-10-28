use crate::Polymath;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    dark: bool,
    reduce_motion: Option<bool>,
    recent: Vec<String>,
    #[serde(deserialize_with = "crate::local_folders::deserialize")]
    folders: Vec<String>,
    reader: String,
    reading_memory: crate::reader_memory::ReaderMemory,
    writer: wordcraft_ui_egui::UiState,
    sheets: gridcraft_ui_egui::UiState,
    slides: deckcraft_ui_egui::UiState,
    images: Option<lightcraft_ui_egui::state::UiState>,
    voice_language: String,
    global_dictation: bool,
    narration_voice: String,
}
fn path() -> Option<PathBuf> {
    if cfg!(test)
        || (std::env::var_os("POLYMATH_NO_PREFS").is_some()
            || std::env::var_os("IRIS_NO_PREFS").is_some())
    {
        return None;
    }
    if let Some(path) = std::env::var_os("POLYMATH_PREFS_PATH") {
        return Some(PathBuf::from(path));
    }
    std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join("Library/Application Support/Polymath/preferences.json"))
}
impl Polymath {
    /// Remove paths that no longer name files from every Recent list. Favorites
    /// stay separate so an offline volume can be reconnected later.
    pub(crate) fn prune_missing_recent(&mut self) -> bool {
        let mut changed = false;
        let mut keep = |path: &str| {
            let exists = Path::new(path).is_file();
            changed |= !exists;
            exists
        };
        self.recent.retain(|path| keep(path));
        self.reader.recent.retain(|file| keep(&file.path));
        self.writer.ui.recent.retain(|path| keep(path));
        self.sheets.ui.recent.retain(|path| keep(path));
        self.slides.ui.recent.retain(|path| keep(path));
        changed
    }

    pub(crate) fn load_preferences(&mut self) {
        let Some(path) = path() else {
            return;
        };
        // Read the previous application's settings on first launch, then save under Polymath.
        let source = if path.exists() {
            path
        } else {
            std::env::var_os("HOME")
                .map(|h| {
                    PathBuf::from(h).join("Library/Application Support/Iris Suite/preferences.json")
                })
                .unwrap_or(path)
        };
        let Some(p) = fs::read(source)
            .ok()
            .and_then(|b| serde_json::from_slice::<Preferences>(&b).ok())
        else {
            return;
        };
        self.dark = p.dark;
        if let Some(reduce_motion) = p.reduce_motion {
            self.home_state.reduce_motion = reduce_motion;
        }
        self.recent = p.recent.into_iter().take(16).collect();
        self.folders = p.folders;
        self.reader.restore(&p.reader);
        self.reading_memory = p.reading_memory.load();
        self.writer.apply_prefs(p.writer);
        self.sheets.ui = p.sheets;
        self.slides.ui = p.slides;
        if let Some(images) = p.images {
            self.images.ui = images;
            self.images.ui.left_width = self.images.ui.left_width.max(200.0);
            self.images.ui.right_width = self.images.ui.right_width.max(250.0);
        }
        if crate::voice::Voice::languages()
            .iter()
            .any(|(_, code)| *code == p.voice_language)
        {
            self.voice.language = p.voice_language;
        }
        self.global_dictation.enabled = p.global_dictation;
        if self
            .voice
            .voices
            .iter()
            .any(|(name, _)| name == &p.narration_voice)
        {
            self.voice.voice = p.narration_voice;
        }
    }
    pub(crate) fn save_preferences(&self) {
        let Some(path) = path() else {
            return;
        };
        let p = Preferences {
            dark: self.dark,
            reduce_motion: Some(self.home_state.reduce_motion),
            recent: self.recent.clone(),
            folders: self.folders.clone(),
            reader: self.reader.persist(),
            reading_memory: self
                .reading_memory
                .snapshot(&self.reader, &self.office_previews),
            writer: self.writer.prefs(),
            sheets: self.sheets.ui.clone(),
            slides: self.slides.ui.clone(),
            images: Some(self.images.ui.clone()),
            voice_language: self.voice.language.clone(),
            global_dictation: self.global_dictation.enabled,
            narration_voice: self.voice.voice.clone(),
        };
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let temp = path.with_extension(format!("{}.tmp", std::process::id()));
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temp)?;
            file.write_all(&serde_json::to_vec_pretty(&p)?)?;
            file.sync_all()?;
            fs::rename(temp, path)?;
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("Polymath preferences could not be saved: {e}");
        }
    }
}
