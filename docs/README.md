# Polymath guides

Polymath is one local macOS app. Home opens a workspace; the shared top bar returns to Home without closing the documents in other workspaces. **Open file** opens a read-only Reader tab first. Choose Edit to hand the original to its editor.

For UI implementation and compatible egui libraries, see [Native UI stack](UI_STACK.md) and the [design guide](../DESIGN.md).

| Area | Guide | What it covers |
| --- | --- | --- |
| Home and Settings | [Home and Settings](HOME_AND_SETTINGS.md) | Library, chosen folders, theme, local access |
| Reader | [Reader](READER.md) | PDF, office, Markdown, text and code previews; edit handoff |
| PDF Tools | [PDF Tools](PDF_TOOLS.md) | Local PDF operations and current capability gaps |
| OCR | [Local OCR](OCR.md) | On-demand English OCR and model choices for this Mac |
| Writer | [Writer](WRITER.md) | DOCX, Markdown and plain text editing and Save As |
| Sheets | [Sheets](SHEETS.md) | Spreadsheet and CSV editing |
| Slides | [Slides](SLIDES.md) | Presentation editing and export |
| Images | [Images](IMAGES.md) | Local photo library and non-destructive edits |
| Media | [Media](MEDIA.md) | Local audio/video player, supported codecs and open-source options |
| Video editor | [Video editor](VIDEO_EDITOR.md) | Trim and arrange clips, manage audio, compress and export an MP4 |
| Voice | [Voice](VOICE.md) | Dictation, Indian-language choices, PDF narration |

For coding tools, start with [CODEX.md](../CODEX.md) or the [shared Claude/Codex guide](USING_WITH_AGENTS.md). The [architecture](ARCHITECTURE.md) explains the combined process, the [validation record](VALIDATION.md) lists tested behavior and limits, and the [upstream feature check](UPSTREAM_STATUS.md) records additions in the original repositories. These guides describe Polymath's combined UI; each linked sibling repository retains its own deeper engine and command documentation.
