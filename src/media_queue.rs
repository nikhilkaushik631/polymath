//! A bounded, non-recursive queue of local media beside the file the owner opened.
use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
};
use symphonia::core::{
    formats::FormatOptions,
    io::{MediaSourceStream, MediaSourceStreamOptions},
    meta::{MetadataOptions, StandardTagKey, Tag},
    probe::Hint,
};

const MAX_ITEMS: usize = 5_000;
const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
const AUDIO: &[&str] = &[
    "mp3", "m4a", "aac", "wav", "aiff", "aif", "caf", "flac", "ogg", "opus",
];
const VIDEO: &[&str] = &["mp4", "m4v", "mov", "webm", "mkv"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Audio,
    Video,
}

impl Kind {
    fn accepts(self, path: &Path) -> bool {
        let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
            return false;
        };
        let allowed = if self == Self::Audio { AUDIO } else { VIDEO };
        allowed.iter().any(|known| ext.eq_ignore_ascii_case(known))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    Name,
    Artist,
    Album,
}

impl Sort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Artist => "Artist",
            Self::Album => "Album",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Item {
    pub path: PathBuf,
    pub name: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
}

impl Item {
    fn from_path(path: PathBuf) -> Self {
        let name = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Untitled")
            .to_owned();
        Self {
            path,
            title: name.clone(),
            name,
            artist: None,
            album: None,
        }
    }
}

pub struct Queue {
    pub items: Vec<Item>,
    pub sort: Sort,
    pub kind: Kind,
    pub folder: String,
    pub ready: bool,
    receiver: Option<Receiver<Vec<Item>>>,
}

impl Queue {
    pub fn new(path: &Path) -> Self {
        let kind = if Kind::Audio.accepts(path) {
            Kind::Audio
        } else {
            Kind::Video
        };
        let folder = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("Folder")
            .to_owned();
        let current = Item::from_path(path.to_path_buf());
        let (tx, rx) = mpsc::channel();
        let parent = path.parent().map(Path::to_path_buf);
        let started = std::thread::Builder::new()
            .name("polymath-media-queue".into())
            .spawn(move || {
                let mut items = Vec::new();
                if let Some(parent) = parent
                    && let Ok(entries) = std::fs::read_dir(parent)
                {
                    for entry in entries.flatten() {
                        if items.len() >= MAX_ITEMS {
                            break;
                        }
                        let path = entry.path();
                        if !kind.accepts(&path)
                            || !entry.file_type().is_ok_and(|kind| kind.is_file())
                        {
                            continue;
                        }
                        if !entry
                            .metadata()
                            .is_ok_and(|meta| meta.len() <= MAX_FILE_BYTES)
                        {
                            continue;
                        }
                        let mut item = Item::from_path(path);
                        if kind == Kind::Audio {
                            read_tags(&mut item);
                        }
                        items.push(item);
                    }
                }
                let _ = tx.send(items);
            })
            .ok();
        Self {
            items: vec![current],
            sort: Sort::Name,
            kind,
            folder,
            ready: started.is_none(),
            receiver: started.map(|_| rx),
        }
    }

    pub fn poll(&mut self, current: &Path) {
        let Some(rx) = &self.receiver else { return };
        match rx.try_recv() {
            Ok(mut items) => {
                if !items.iter().any(|item| item.path == current) {
                    items.push(Item::from_path(current.to_path_buf()));
                }
                self.items = items;
                self.sort_items();
                self.ready = true;
                self.receiver = None;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.ready = true;
                self.receiver = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    pub fn set_sort(&mut self, sort: Sort) {
        self.sort = sort;
        self.sort_items();
    }

    fn sort_items(&mut self) {
        let sort = self.sort;
        self.items.sort_by(|a, b| {
            let a_field = match sort {
                Sort::Name => a.name.as_str(),
                Sort::Artist => a.artist.as_deref().unwrap_or("~"),
                Sort::Album => a.album.as_deref().unwrap_or("~"),
            };
            let b_field = match sort {
                Sort::Name => b.name.as_str(),
                Sort::Artist => b.artist.as_deref().unwrap_or("~"),
                Sort::Album => b.album.as_deref().unwrap_or("~"),
            };
            a_field
                .to_lowercase()
                .cmp(&b_field.to_lowercase())
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
                .then_with(|| a.path.cmp(&b.path))
        });
    }

    pub fn index(&self, current: &Path) -> Option<usize> {
        self.items.iter().position(|item| item.path == current)
    }

    pub fn adjacent(&self, current: &Path, direction: i8) -> Option<&Path> {
        let index = self.index(current)?;
        let next = if direction < 0 {
            index.checked_sub(1)?
        } else {
            index.checked_add(1)?
        };
        self.items.get(next).map(|item| item.path.as_path())
    }

    pub fn current(&self, path: &Path) -> Option<&Item> {
        self.index(path).and_then(|index| self.items.get(index))
    }
}

fn read_tags(item: &mut Item) {
    let Ok(file) = File::open(&item.path) else {
        return;
    };
    let stream = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
    let mut hint = Hint::new();
    if let Some(ext) = item.path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(ext);
    }
    let Ok(mut probe) = symphonia::default::get_probe().format(
        &hint,
        stream,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    ) else {
        return;
    };
    if let Some(mut metadata) = probe.metadata.get()
        && let Some(revision) = metadata.skip_to_latest()
    {
        apply_tags(item, revision.tags());
    }
    if let Some(revision) = probe.format.metadata().skip_to_latest() {
        apply_tags(item, revision.tags());
    }
}

fn apply_tags(item: &mut Item, tags: &[Tag]) {
    for tag in tags {
        let value = tag.value.to_string();
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match tag.std_key {
            Some(StandardTagKey::TrackTitle) => item.title = value.to_owned(),
            Some(StandardTagKey::Artist | StandardTagKey::AlbumArtist) => {
                item.artist = Some(value.to_owned())
            }
            Some(StandardTagKey::Album) => item.album = Some(value.to_owned()),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artist_and_album_sort_reorder_the_queue_without_losing_the_current_track() {
        let mut queue = Queue {
            items: vec![
                Item {
                    path: "b.mp3".into(),
                    name: "b".into(),
                    title: "Second".into(),
                    artist: Some("Zara".into()),
                    album: Some("First".into()),
                },
                Item {
                    path: "a.mp3".into(),
                    name: "a".into(),
                    title: "First".into(),
                    artist: Some("Asha".into()),
                    album: Some("Last".into()),
                },
            ],
            sort: Sort::Name,
            kind: Kind::Audio,
            folder: "Songs".into(),
            ready: true,
            receiver: None,
        };
        queue.set_sort(Sort::Artist);
        assert_eq!(queue.items[0].path, PathBuf::from("a.mp3"));
        assert_eq!(
            queue.adjacent(Path::new("a.mp3"), 1),
            Some(Path::new("b.mp3"))
        );
        queue.set_sort(Sort::Album);
        assert_eq!(queue.items[0].path, PathBuf::from("b.mp3"));
        assert_eq!(queue.index(Path::new("a.mp3")), Some(1));
        assert_eq!(queue.adjacent(Path::new("a.mp3"), 1), None);
    }
}
