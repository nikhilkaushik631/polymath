use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Workspace {
    #[default]
    Home,
    Reader,
    Tools,
    Writer,
    Sheets,
    Slides,
    Images,
    Video,
}
impl Workspace {
    pub const ALL: [Self; 8] = [
        Self::Home,
        Self::Reader,
        Self::Tools,
        Self::Writer,
        Self::Sheets,
        Self::Slides,
        Self::Images,
        Self::Video,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Reader => "Reader",
            Self::Tools => "PDF Tools",
            Self::Writer => "Writer",
            Self::Sheets => "Sheets",
            Self::Slides => "Slides",
            Self::Images => "Images",
            Self::Video => "Video",
        }
    }
    pub fn accent(self, dark: bool) -> egui::Color32 {
        let (light, night) = match self {
            Self::Home => ((38, 88, 116), (150, 200, 225)),
            Self::Reader | Self::Tools => ((42, 95, 126), (135, 188, 218)),
            Self::Writer => ((44, 105, 113), (137, 204, 207)),
            Self::Sheets => ((45, 111, 73), (135, 209, 162)),
            Self::Slides => ((161, 85, 64), (233, 161, 139)),
            Self::Images => ((142, 104, 49), (224, 192, 128)),
            Self::Video => ((150, 71, 64), (236, 143, 129)),
        };
        let (r, g, b) = if dark { night } else { light };
        egui::Color32::from_rgb(r, g, b)
    }
    pub fn parse(s: &str) -> Result<Self, String> {
        if s.eq_ignore_ascii_case("tools") || s.eq_ignore_ascii_case("pdf-tools") {
            return Ok(Self::Tools);
        }
        Self::ALL
            .into_iter()
            .find(|w| w.label().eq_ignore_ascii_case(s))
            .ok_or_else(|| {
                "workspace must be home, reader, PDF Tools, writer, sheets, slides, images, or video"
                    .into()
            })
    }
    pub fn for_path(path: &Path) -> Result<Self, String> {
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "pdf" => Ok(Self::Reader),
            _ if MEDIA_EXTENSIONS.contains(&ext.as_str()) => Ok(Self::Reader),
            "docx" | "docm" | "dotx" | "dotm" | "doc" | "dot" | "odt" | "rtf" | "txt" | "md" => {
                Ok(Self::Writer)
            }
            "xlsx" | "xlsm" | "xltx" | "xltm" | "xlsb" | "ods" | "csv" | "tsv" => Ok(Self::Sheets),
            "pptx" | "potx" | "ppsx" | "deckcraft" => Ok(Self::Slides),
            "jpg" | "jpeg" | "png" | "tif" | "tiff" | "webp" | "jxl" | "bmp" | "gif" | "dng"
            | "cr2" | "cr3" | "nef" | "arw" | "raf" | "rw2" | "orf" | "pef" | "srw" => {
                Ok(Self::Images)
            }
            _ if SOURCE_EXTENSIONS.contains(&ext.as_str()) => Ok(Self::Reader),
            _ => Err(format!(
                "Polymath cannot choose an editor for .{ext}. Open a supported PDF, document, spreadsheet, presentation, image, audio, or video."
            )),
        }
    }
    pub fn icon(self) -> egui::ImageSource<'static> {
        match self {
            Self::Home => egui::include_image!("../assets/icons/home.svg"),
            Self::Reader => egui::include_image!("../assets/icons/reader.svg"),
            Self::Tools => egui::include_image!("../assets/icons/tools.svg"),
            Self::Writer => egui::include_image!("../assets/icons/writer.svg"),
            Self::Sheets => egui::include_image!("../assets/icons/sheets.svg"),
            Self::Slides => egui::include_image!("../assets/icons/slides.svg"),
            Self::Images => egui::include_image!("../assets/icons/images.svg"),
            Self::Video => egui::include_image!("../assets/icons/video.svg"),
        }
    }
}
pub const VIDEO_EXTENSIONS: &[&str] = &["mp4", "m4v", "mov", "webm", "mkv"];

pub fn is_video_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            VIDEO_EXTENSIONS
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
}
pub const EXTENSIONS: &[&str] = &[
    "pdf",
    "docx",
    "docm",
    "dotx",
    "dotm",
    "doc",
    "dot",
    "odt",
    "rtf",
    "txt",
    "md",
    "py",
    "pyi",
    "js",
    "mjs",
    "cjs",
    "jsx",
    "ts",
    "tsx",
    "html",
    "htm",
    "css",
    "scss",
    "sass",
    "less",
    "json",
    "jsonl",
    "yaml",
    "yml",
    "toml",
    "xml",
    "svg",
    "rs",
    "go",
    "java",
    "kt",
    "swift",
    "c",
    "h",
    "cc",
    "cpp",
    "hpp",
    "cs",
    "fs",
    "fsx",
    "php",
    "rb",
    "pl",
    "lua",
    "dart",
    "r",
    "jl",
    "scala",
    "sql",
    "sh",
    "bash",
    "zsh",
    "fish",
    "ps1",
    "bat",
    "cmd",
    "tex",
    "mdx",
    "vue",
    "svelte",
    "astro",
    "graphql",
    "gql",
    "proto",
    "ini",
    "cfg",
    "conf",
    "properties",
    "gradle",
    "ipynb",
    "xlsx",
    "xlsm",
    "xltx",
    "xltm",
    "xlsb",
    "ods",
    "csv",
    "tsv",
    "pptx",
    "potx",
    "ppsx",
    "deckcraft",
    "jpg",
    "jpeg",
    "png",
    "tif",
    "tiff",
    "webp",
    "jxl",
    "bmp",
    "gif",
    "dng",
    "cr2",
    "cr3",
    "nef",
    "arw",
    "raf",
    "rw2",
    "orf",
    "pef",
    "srw",
    "mp3",
    "m4a",
    "aac",
    "wav",
    "aiff",
    "aif",
    "caf",
    "flac",
    "ogg",
    "opus",
    "mp4",
    "m4v",
    "mov",
    "webm",
    "mkv",
];

pub const MEDIA_EXTENSIONS: &[&str] = &[
    "mp3", "m4a", "aac", "wav", "aiff", "aif", "caf", "flac", "ogg", "opus", "mp4", "m4v", "mov",
    "webm", "mkv",
];

pub const SOURCE_EXTENSIONS: &[&str] = &[
    "py",
    "pyi",
    "js",
    "mjs",
    "cjs",
    "jsx",
    "ts",
    "tsx",
    "html",
    "htm",
    "css",
    "scss",
    "sass",
    "less",
    "json",
    "jsonl",
    "yaml",
    "yml",
    "toml",
    "xml",
    "svg",
    "rs",
    "go",
    "java",
    "kt",
    "swift",
    "c",
    "h",
    "cc",
    "cpp",
    "hpp",
    "cs",
    "fs",
    "fsx",
    "php",
    "rb",
    "pl",
    "lua",
    "dart",
    "r",
    "jl",
    "scala",
    "sql",
    "sh",
    "bash",
    "zsh",
    "fish",
    "ps1",
    "bat",
    "cmd",
    "tex",
    "mdx",
    "vue",
    "svelte",
    "astro",
    "graphql",
    "gql",
    "proto",
    "ini",
    "cfg",
    "conf",
    "properties",
    "gradle",
    "ipynb",
];

pub const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "tif", "tiff", "webp", "jxl", "bmp", "gif", "dng", "cr2", "cr3", "nef",
    "arw", "raf", "rw2", "orf", "pef", "srw",
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn routes_case_insensitively_without_guessing_unknown_types() {
        for (p, w) in [
            ("/tmp/Annual.PDF", Workspace::Reader),
            ("/tmp/test.docx", Workspace::Writer),
            ("/tmp/Notes.MD", Workspace::Writer),
            ("/tmp/script.PY", Workspace::Reader),
            ("/tmp/component.TSX", Workspace::Reader),
            ("/tmp/index.HTML", Workspace::Reader),
            ("/tmp/Values.CSV", Workspace::Sheets),
            ("/tmp/Budget.XLSX", Workspace::Sheets),
            ("/tmp/Talk.pptx", Workspace::Slides),
            ("/tmp/Portrait.JPEG", Workspace::Images),
        ] {
            assert_eq!(Workspace::for_path(Path::new(p)), Ok(w));
        }
        assert_eq!(
            Workspace::for_path(Path::new("/tmp/private.json")),
            Ok(Workspace::Reader)
        );
        assert!(Workspace::for_path(Path::new("/tmp/unknown")).is_err());
    }
}
