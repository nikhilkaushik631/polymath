//! Native adapters retained from the four upstream desktop hosts.
use wordcraft_ui_egui::paste_picture::ClipboardPicture;

pub fn images(ctx: &egui::Context) -> lightcraft_ui_egui::Services {
    use lightcraft_ui_egui::pick::{PickKind, PickRequest};
    let wake = ctx.clone();
    lightcraft_ui_egui::Services {
        picker: Some(Box::new(move |request: PickRequest| {
            let (tx, rx) = std::sync::mpsc::channel();
            let wake = wake.clone();
            std::thread::Builder::new()
                .name("Polymath image picker".into())
                .spawn(move || {
                    let mut dialog = rfd::FileDialog::new();
                    if let Some(title) = request.title {
                        dialog = dialog.set_title(title);
                    }
                    if let Some((name, extensions)) = request.filter {
                        dialog = dialog.add_filter(name, extensions);
                    }
                    if let Some(name) = request.file_name {
                        dialog = dialog.set_file_name(name);
                    }
                    let chosen = match request.kind {
                        PickKind::Files => dialog.pick_files().unwrap_or_default(),
                        PickKind::File => dialog.pick_file().into_iter().collect(),
                        PickKind::Folder => dialog.pick_folder().into_iter().collect(),
                        PickKind::Save => dialog.save_file().into_iter().collect(),
                    };
                    let _ = tx.send(
                        chosen
                            .into_iter()
                            .map(|p| p.to_string_lossy().into_owned())
                            .collect(),
                    );
                    wake.request_repaint();
                })
                .map_err(|e| format!("Could not open the image file picker: {e}"))?;
            Ok(rx)
        })),
        pick_files: Some(Box::new(|| {
            rfd::FileDialog::new()
                .add_filter("Images", crate::workspace::IMAGE_EXTENSIONS)
                .pick_files()
                .unwrap_or_default()
                .into_iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect()
        })),
        pick_folder: Some(Box::new(|| {
            rfd::FileDialog::new()
                .set_title("Choose an image library")
                .pick_folder()
                .map(|p| p.to_string_lossy().into_owned())
        })),
        write_shared: Some(std::sync::Arc::new(lightcraft_engine::export::write_file)),
        write: Some(Box::new(lightcraft_engine::export::write_file)),
        png: Some(Box::new(|image: &lightcraft_raster::Rgba8| {
            lightcraft_codecs::encode_png(
                &lightcraft_codecs::EncodeImage::rgba8(image),
                &lightcraft_codecs::EncodeMeta::default(),
            )
            .unwrap_or_default()
        })),
        ..Default::default()
    }
}
pub fn writer(file_dialog: crate::file_dialogs::Hook) -> wordcraft_ui_egui::Services {
    wordcraft_ui_egui::Services {
        file_dialog: Some(file_dialog),
        print: Some(crate::print::hook()),
        clipboard_picture: Some(Box::new(clipboard_picture)),
        ..Default::default()
    }
}

/// The picture on the system clipboard (#45): copied files, else image pixels. The UI checks the
/// sizes before inserting anything.
fn clipboard_picture() -> Option<ClipboardPicture> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|e| log::warn!("clipboard: {e}"))
        .ok()?;
    if let Ok(files) = clipboard.get().file_list()
        && !files.is_empty()
    {
        return Some(ClipboardPicture::Files(files));
    }
    let image = clipboard.get_image().ok()?;
    Some(ClipboardPicture::Pixels {
        width: image.width,
        height: image.height,
        rgba: image.bytes.into_owned(),
    })
}

pub fn sheets() -> gridcraft_ui_egui::Services {
    // Keep the owner alive: X11/Wayland serve clipboard data from this handle.
    let mut clipboard: Option<arboard::Clipboard> = None;
    gridcraft_ui_egui::Services {
        pick_folder: Some(Box::new(|| {
            rfd::FileDialog::new()
                .set_title("Choose a folder for Polymath")
                .pick_folder()
                .map(|p| p.to_string_lossy().into_owned())
        })),
        copy_html: Some(Box::new(move |html, text| {
            if clipboard.is_none() {
                clipboard = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
            }
            clipboard
                .as_mut()
                .ok_or("clipboard unavailable")?
                .set_html(html, Some(text))
                .map_err(|e| e.to_string())
        })),
        pick_open: Some(Box::new(|| {
            rfd::FileDialog::new()
                .add_filter(
                    "Spreadsheets",
                    &[
                        "xlsx", "xlsm", "xltx", "xltm", "xlsb", "ods", "csv", "tsv", "txt", "json",
                    ],
                )
                .add_filter("Excel Workbook", &["xlsx", "xlsm", "xltx", "xltm"])
                .add_filter("Excel Binary Workbook (data import)", &["xlsb"])
                .add_filter("OpenDocument Spreadsheet (data import)", &["ods"])
                .add_filter("CSV", &["csv"])
                .pick_file()
                .and_then(|p| p.to_str().map(str::to_string))
        })),
        pick_picture: Some(Box::new(|| {
            rfd::FileDialog::new()
                .add_filter("Pictures (PNG, JPEG)", &["png", "jpg", "jpeg"])
                .pick_file()
                .and_then(|p| p.to_str().map(str::to_string))
        })),
        pick_save: Some(Box::new(|suggested: &str| {
            let stem = std::path::Path::new(suggested)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Book1")
                .to_string();
            rfd::FileDialog::new()
                .set_file_name(format!("{stem}.xlsx"))
                .add_filter("Excel Workbook (.xlsx)", &["xlsx"])
                .add_filter("CSV UTF-8 (.csv)", &["csv"])
                .add_filter("Web Page (.html)", &["html"])
                .save_file()
                .and_then(|p| p.to_str().map(str::to_string))
        })),
        ..Default::default()
    }
}

pub fn slides() -> deckcraft_ui_egui::Services {
    deckcraft_ui_egui::Services {
        pick_folder: Some(Box::new(|| {
            rfd::FileDialog::new()
                .set_title("Choose a folder for Polymath")
                .pick_folder()
                .map(|p| p.to_string_lossy().into_owned())
        })),
        pick_open: Some(Box::new(|purpose: &str| {
            let d = rfd::FileDialog::new();
            let d = match purpose {
                "picture" => d.add_filter(
                    "Pictures",
                    &["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"],
                ),
                "audio" => d.add_filter(
                    "Audio",
                    &[
                        "wav", "mp3", "m4a", "m4b", "aac", "flac", "ogg", "oga", "opus", "aif",
                        "aiff", "aifc", "caf", "wma", "weba", "mka",
                    ],
                ),
                "video" => d.add_filter("Video", &["mp4", "m4v", "mov", "webm", "mkv", "wmv"]),
                _ => d
                    .add_filter("Presentations", &["deckcraft", "pptx", "potx", "ppsx"])
                    .add_filter("Polymath Slides", &["deckcraft"])
                    .add_filter("PowerPoint", &["pptx", "potx", "ppsx"])
                    .add_filter("Outline", &["txt", "md"]),
            };
            d.pick_file().map(|p| p.to_string_lossy().to_string())
        })),
        pick_save: Some(Box::new(|name: &str| {
            rfd::FileDialog::new()
                .set_file_name(name)
                .save_file()
                .map(|p| p.to_string_lossy().to_string())
        })),
        read: Some(Box::new(|p: &str| {
            std::fs::read(p).map_err(|e| e.to_string())
        })),
        write: Some(Box::new(|p: &str, b: &[u8]| {
            std::fs::write(p, b).map_err(|e| e.to_string())
        })),
        clipboard_image: Some(Box::new(|| {
            let mut cb = arboard::Clipboard::new().ok()?;
            let img = cb.get_image().ok()?;
            let rgba = image::RgbaImage::from_raw(
                img.width as u32,
                img.height as u32,
                img.bytes.into_owned(),
            )?;
            let mut out = Vec::new();
            rgba.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
                .ok()?;
            Some(out)
        })),
        // Media is mixed and clocked by the output device; `DECKCRAFT_NO_AUDIO` plays silently.
        audio_out: std::env::var_os("DECKCRAFT_NO_AUDIO").is_none().then(|| {
            Box::new(crate::audio::CpalOut::default()) as Box<dyn deckcraft_media::AudioOut>
        }),
        ..Default::default()
    }
}
