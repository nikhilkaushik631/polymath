//! Local reading positions and favorite files. Neither is written into documents.
use pdfcraft_engine::DocId;
use pdfcraft_ui_egui::PdfCraftApp;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

const MAX_FAVORITES: usize = 100;
const MAX_POSITIONS: usize = 1000;

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ReaderMemory {
    positions: HashMap<String, usize>,
    favorites: Vec<String>,
    #[serde(skip)]
    seen: HashSet<DocId>,
}

fn key(path: &str) -> String {
    std::path::absolute(path).map_or_else(
        |_| path.to_owned(),
        |path| path.to_string_lossy().into_owned(),
    )
}

impl ReaderMemory {
    pub(crate) fn load(mut self) -> Self {
        self.positions = self
            .positions
            .into_iter()
            .filter(|(path, page)| !path.is_empty() && *page < 1_000_000)
            .take(MAX_POSITIONS)
            .map(|(path, page)| (key(&path), page))
            .collect();
        let mut seen = HashSet::new();
        self.favorites
            .retain(|path| !path.is_empty() && seen.insert(key(path)));
        self.favorites.truncate(MAX_FAVORITES);
        self.favorites = self.favorites.into_iter().map(|path| key(&path)).collect();
        self
    }

    pub(crate) fn favorites(&self) -> &[String] {
        &self.favorites
    }

    pub(crate) fn is_favorite(&self, path: &str) -> bool {
        self.favorites.iter().any(|favorite| favorite == &key(path))
    }

    pub(crate) fn toggle_favorite(&mut self, path: &str) -> bool {
        let path = key(path);
        if let Some(index) = self.favorites.iter().position(|favorite| favorite == &path) {
            self.favorites.remove(index);
            false
        } else {
            if self.favorites.len() == MAX_FAVORITES {
                self.favorites.pop();
            }
            self.favorites.insert(0, path);
            true
        }
    }

    pub(crate) fn set_favorite(&mut self, path: &str, favorite: bool) {
        if self.is_favorite(path) != favorite {
            self.toggle_favorite(path);
        }
    }

    pub(crate) fn remove_favorite(&mut self, path: &str) {
        let path = key(path);
        self.favorites.retain(|favorite| favorite != &path);
    }

    pub(crate) fn page(&self, path: &str) -> Option<usize> {
        self.positions.get(&key(path)).copied()
    }

    fn view_path<'a>(
        reader: &'a PdfCraftApp,
        previews: &'a HashMap<DocId, String>,
        id: DocId,
    ) -> Option<&'a str> {
        previews
            .get(&id)
            .map(String::as_str)
            .or_else(|| reader.session.get(id).and_then(|doc| doc.path.as_deref()))
    }

    pub(crate) fn sync(
        &mut self,
        reader: &mut PdfCraftApp,
        previews: &HashMap<DocId, String>,
    ) -> bool {
        let live: HashSet<_> = reader.views.iter().map(|view| view.id).collect();
        self.seen.retain(|id| live.contains(id));
        let mut changed = false;
        for view in &mut reader.views {
            let Some(path) = previews.get(&view.id).map(String::as_str).or_else(|| {
                reader
                    .session
                    .get(view.id)
                    .and_then(|doc| doc.path.as_deref())
            }) else {
                continue;
            };
            if !Path::new(path).is_absolute() {
                continue;
            }
            let path = key(path);
            if self.seen.insert(view.id)
                && let Some(page) = self.positions.get(&path)
            {
                view.go_to_page(*page);
            }
            if self.positions.get(&path) != Some(&view.current) {
                self.positions.insert(path, view.current);
                changed = true;
            }
        }
        if self.positions.len() > MAX_POSITIONS {
            // Keep the bound simple and deterministic; favorited files stay.
            let favorites: HashSet<_> = self.favorites.iter().cloned().collect();
            self.positions.retain(|path, _| {
                favorites.contains(path)
                    || live.iter().any(|id| {
                        Self::view_path(reader, previews, *id).is_some_and(|p| key(p) == *path)
                    })
            });
        }
        changed
    }

    pub(crate) fn snapshot(&self, reader: &PdfCraftApp, previews: &HashMap<DocId, String>) -> Self {
        let mut positions = self.positions.clone();
        for view in &reader.views {
            if let Some(path) = Self::view_path(reader, previews, view.id)
                && Path::new(path).is_absolute()
            {
                positions.insert(key(path), view.current);
            }
        }
        Self {
            positions,
            favorites: self.favorites.clone(),
            seen: HashSet::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn favorites_are_unique_and_removable() {
        let mut memory = ReaderMemory::default();
        assert!(memory.toggle_favorite("/tmp/book.pdf"));
        assert!(memory.is_favorite("/tmp/book.pdf"));
        assert!(!memory.toggle_favorite("/tmp/book.pdf"));
        assert!(!memory.is_favorite("/tmp/book.pdf"));
    }

    #[test]
    fn reopened_pdf_starts_on_saved_page() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../reader/target/interface-scale/bookmarks.pdf"
        );
        let mut memory = ReaderMemory::default();
        memory.positions.insert(key(path), 2);
        let mut reader = PdfCraftApp::new();
        reader.open_path(path);
        memory.sync(&mut reader, &HashMap::new());
        assert_eq!(reader.views[0].current, 2);
        reader.views[0].go_to_page(1);
        assert!(memory.sync(&mut reader, &HashMap::new()));
        let saved = serde_json::to_string(&memory.snapshot(&reader, &HashMap::new())).unwrap();
        let mut restored: ReaderMemory = serde_json::from_str(&saved).unwrap();
        restored = restored.load();
        let mut second = PdfCraftApp::new();
        second.open_path(path);
        restored.sync(&mut second, &HashMap::new());
        assert_eq!(second.views[0].current, 1);
    }
}
