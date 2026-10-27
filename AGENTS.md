# Polymath — shared instructions for Claude and Codex

This project combines Reader, Writer, Sheets, Slides, and Images in one native application. The sibling repositories supply the existing engines and editor UI libraries; Images comes from Lightcraft. Preserve those checkouts and all uncommitted work. Do not alter the unrelated `../../iris` project.

Voice input uses the bundled multilingual whisper.cpp CLI and model from sibling `speech-engine`; PDF narration uses installed macOS voices. Request microphone access through the normal macOS prompt. Speech and document text remain local. Keep the bundled model and executable licenses and checksums documented.

Read PRODUCT.md, DESIGN.md, README.md, and docs/USING_WITH_AGENTS.md before relevant work. CODEX.md is the Codex quick start; docs/README.md indexes guides for every workspace. Keep the application local, use neutral light/dark surfaces with restrained workspace accents as described in DESIGN.md, and preserve the 125% interface scale with narrow sidebars. Reader previews office and text files without editing the original; Markdown and plain text use Writer as a common editor, with a Save As format choice. Motion remains separate unless explicitly requested.

Use one Cargo build at a time with CARGO_BUILD_JOBS=1 on this 8 GB Mac. The target cache is shared with Reader. No unsafe code or panic shortcuts in production. Preserve documents and undo histories when changing workspaces. Closing Polymath must guard unsaved work in every workspace. All automatic folder listings are limited to folders selected by the user through the system picker.

Normal launches start no server. Agent control is explicitly enabled, authenticated, and bound to loopback. Never change global client configuration. Keep licenses and attribution from all five upstream projects. Original new artwork uses MIT OR Apache-2.0; no proprietary fonts or vendor logos. Keep the asset ledger accurate. Do not commit, push, or publish without a request.
