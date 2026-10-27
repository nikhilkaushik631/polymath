//! Render supported office files into an in-memory PDF for read-only viewing.
//! The source file is never written to or opened in a live editor session.
use std::{fs, path::Path};

pub fn pdf_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "The filename is not valid UTF-8".to_owned())?;
    match ext.as_str() {
        "docx" | "docm" | "dotx" | "dotm" | "doc" | "dot" | "odt" | "rtf" => {
            let document = wordcraft_engine::io::open_path(path)?;
            wordcraft_engine::io::save_bytes("preview.pdf", &document)
        }
        "xlsx" | "xlsm" | "xltx" | "xltm" | "xlsb" | "ods" | "csv" | "tsv" => {
            let mut session = gridcraft_engine::Session::new();
            session
                .execute("file.open", serde_json::json!({"path":path}))
                .map_err(|error| error.to_string())?;
            let result = session
                .execute("file.exportPdf", serde_json::json!({"sheets":"all"}))
                .map_err(|error| error.to_string())?;
            let encoded = result["base64"]
                .as_str()
                .ok_or_else(|| "The spreadsheet exporter returned no PDF".to_owned())?;
            use base64::Engine as _;
            base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|error| error.to_string())
        }
        "pptx" | "potx" | "ppsx" | "deckcraft" => {
            let bytes = fs::read(path).map_err(|error| format!("{name}: {error}"))?;
            let presentation = deckcraft_engine::cmd::file::open_presentation(name, &bytes)
                .map_err(|error| error.to_string())?;
            deckcraft_engine::cmd::file::save_bytes(&presentation, "pdf")
                .map_err(|error| error.to_string())
        }
        _ => Err(format!("Reader has no office preview for .{ext}")),
    }
}
