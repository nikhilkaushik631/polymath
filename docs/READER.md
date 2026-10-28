# Reader

Reader is for inspection. Open a PDF, office file, Markdown/plain-text file or source code from Reader to keep it read-only. The Reader landing screen has a local file picker and recent items. Home's **Open file** also opens Markdown and plain text in Reader; other editable formats go to their editors.

PDFs use the native PDF viewer. Word documents, spreadsheets/CSV and presentations are rendered locally into an in-memory PDF preview; the original file is not changed by previewing. The visible **Edit in Writer**, **Edit in Sheets** or **Edit in Slides** action opens the original in its editor. For Markdown and `.txt`, **Edit in Writer** uses the same text editor. Conversion may differ from the editable layout, so use the corresponding editor to check final formatting.

Code and other plain-text files open in a bounded read-only viewer with line numbers, find-in-file search and **Copy all**. It accepts UTF-8 and BOM-marked UTF-16 text up to 16 MB and rejects binary content. Examples include `.py`, `.js`, `.tsx`, `.html`, `.css`, `.json`, `.toml`, `.rs` and many other text extensions. An unknown extension can still open if its contents decode as text. Files are displayed; they are never executed.

Images open read-only with Fit, zoom and pan; **Edit in Images** in the top bar imports the original into the editor without moving it. Camera RAW files with an embedded preview can be viewed; RAWs without one need the Images editor. The viewer limits a source file to 256 MB and a preview to 2400 pixels on its longest edge.

Audio and video files open in Reader's embedded local player with Play, Pause and seek controls. See [Media](MEDIA.md) for supported formats and current limits.

ZIP, RAR and 7z archives open in Reader tabs with a searchable file list. Use **Extract all** or select items and choose **Extract selected**; Polymath creates a new output folder. See [Archives](ARCHIVES.md) for the supported entries and safety rules.

PDF editing, conversion, signing and the full command catalog live in the separate [PDF Tools](PDF_TOOLS.md) workspace. PDF narration lives under **Settings → Voice**; see [Voice](VOICE.md). For coding agents, `app.preview {path}` is always read-only; `app.open {path,edit:true}` opens Markdown or plain text in Writer. See [the agent guide](USING_WITH_AGENTS.md).
