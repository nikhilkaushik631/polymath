# Use Polymath with Claude or Codex

The combined Polymath app keeps all five editors in one process. Use editor commands to author files, then inspect and render the result. Do not modify the application source for ordinary presentation, document, spreadsheet or image work. `AGENTS.md` is the shared instruction source; `CLAUDE.md` includes it, and [CODEX.md](../CODEX.md) provides the Codex quick start. The [workspace guides](README.md) cover the GUI and file formats.

## Connect to the combined application

The project-local `.mcp.json` (Claude) and `.codex/config.toml` (Codex) use `packaging/agent_bridge.py --launch`. Normal Polymath launches start no server. The configured MCP bridge starts an agent-enabled Polymath window only on its first document-tool call, then reuses it. Open your documents in that window. It does not attach to or close an existing normal Polymath window with your unsaved work. Restart/reload the client to pick up project MCP settings when needed.

The bridge uses Python 3's standard library. It reads a random token from the owner-only `~/Library/Application Support/Polymath/agent-control.json`; never print or paste that token. The app binds only to `127.0.0.1` on a random port. A clean exit removes the endpoint file. If Polymath crashed, check the recorded PID before removing a stale endpoint. The bridge refuses to overwrite it automatically.

To start a dedicated instance yourself:

```sh
./dist/Polymath.app/Contents/MacOS/Polymath --control /tmp/polymath-session.json
python3 packaging/agent_bridge.py --endpoint /tmp/polymath-session.json --call app.inspect
```

A control file must not already exist. `POLYMATH_NO_PREFS=1` disables preference reads/writes for synthetic QA instances.

## Authoring workflow

1. `polymath_inspect` shows the current workspace and every dirty workspace. Preserve unrelated work.
2. `polymath_open {path}` opens a read-only Reader tab for every supported file type. `edit:true` deliberately opens the appropriate Writer, Sheets, Slides, Images, or Video editor. CSV and TSV appear as tables in Reader; office formats appear as in-memory PDF previews. ZIP, RAR and 7z archives open in Reader for browsing and explicit extraction. Audio and video play locally in Reader. `polymath_switch {workspace:"images"}` activates the editor. Workspaces are `reader`, `tools` (PDF Tools), `writer`, `sheets`, `slides`, `images`, `video`, `home`.
3. `polymath_commands {workspace}` returns the real command catalog and parameter descriptions. Inspect this before guessing command names or arguments.
4. `polymath_execute {workspace,command,params}` runs a command in that editor. `polymath_document {workspace}` inspects the result.
5. Save to a user-requested path. Reopen or inspect the saved file and render/check it. `polymath_screenshot {path}` captures the whole Polymath window; use a synthetic document for published evidence.

Writer example: `text.insert {text:"Hello from Polymath."}`, then `file.saveAs {path:"/absolute/path/letter.docx"}`. To save Markdown or plain text, use a path ending in `.md` or `.txt`; the interactive Save As dialog offers both formats. Sheets example: `cell.set {cell:"A1",input:"20"}`, `cell.set {cell:"A2",input:"22"}`, `cell.set {cell:"A3",input:"=SUM(A1:A2)"}`; verify with `cell.get`/`sheet.read`. Slides exposes the complete existing presentation command catalog, including slide/shape/text insertion and PPTX/PDF export. Discover its schemas using `polymath_commands`.

`polymath_call {method,params}` exposes the existing editor UI protocols for rendering, pointer/key input, selection, dialogs and views. Include `workspace` in params to address an editor. Reader maps PDF `document.inspect` to `ui.state`, `engine.commands` to `ui.commands`, and `engine.execute {command}` to `ui.command {id}`. While a source file is open, `polymath_document {workspace:"reader"}` returns its path, language, line count and text; source view is read-only. PDF Tools shares the Reader PDF session, so `workspace:"tools"` reaches the same PDF command adapter. Reader's `ui.set {key,value}` and other native `ui.*` methods remain available for PDFs. Deep headless PDF automation remains in `../reader/target/release/iris-reader-cli`; the combined GUI command adapter does not accept arbitrary PDF engine parameters.

Images exposes Lightcraft's `engine.commands`, `engine.execute {command,params}`, `ui.inspect`, `ui.widgets`, and `ui.clickWidget` through the same authenticated Polymath endpoint. `polymath_open {path}` previews an image read-only in Reader; `polymath_open {path,edit:true}` adds it in place to the local Images library and opens the editor. `polymath_document {workspace:"images"}` returns the selected photo's metadata and UI state. Discover valid adjustment and export parameters before running them; verify exported files on disk. The image library is local and does not move originals on ordinary Open.

Full engine/control references remain in the sibling projects:

- [Reader agent guide](../../reader/docs/USING_WITH_AGENTS.md)
- [Writer control protocol](../../writer/docs/control-protocol.md)
- [Sheets control protocol](../../sheets/docs/control-protocol.md)
- [Slides control protocol](../../slides/docs/control-protocol.md)
- [Images control protocol](../../lightcraft/docs/control-protocol.md)

## Shared application methods

| Method | Parameters | Effect |
| --- | --- | --- |
| `app.inspect` | `{}` | Current workspace, theme, dirty workspaces and open counts |
| `app.workspace` | `{workspace}` | Preserve sessions and change visible editor |
| `app.open` | `{path, edit?: bool}` | Open a local file in a Reader tab; `edit:true` enters the appropriate editor |
| `app.preview` | `{path}` | View a local PDF, office, image, media, archive, source or text file read-only in Reader; office conversion stays in memory |
| `app.video.inspect` | `{}` | Inspect the Video editor timeline, audio choice and export state |
| `app.video.add` | `{path}` | Add a local video clip to the editor timeline and enter Video |
| `app.video.trim` | `{index:0,start:0.5,end:8.0}` | Set source-relative in/out seconds for a loaded clip |
| `app.video.move` | `{from:1,to:0}` | Reorder a clip in the timeline |
| `app.video.audio` / `app.video.audio.options` | `{path}` / `{start:0.5,gain:0.8}` | Add one local soundtrack; place it on the output timeline and set its level |
| `app.video.mute` | `{original:true/false}` | Strip or keep the source clips' audio |
| `app.video.quality` | `{preset:"original"|"high"|"balanced"|"small"}` | Choose an MP4 quality and size preset |
| `app.video.output` / `app.video.export` | `{path:"/absolute/output.mp4"}` / `{}` | Choose a new destination, then explicitly start local FFmpeg export; inspect progress with `app.video.inspect` |
| `app.reader.favorites` | `{}` | List local Reader favorites and saved one-based reading pages |
| `app.reader.favorite` | `{value:true/false, path?:"/absolute/file"}` | Pin or unpin a local file; omitted path uses the active Reader file |
| `app.reader.tabs` | `{}` | List Reader tabs with stable IDs, paths, active and dirty state |
| `app.reader.select` | `{id?: string, path?: string}` | Activate an existing Reader tab |
| `app.reader.close` | `{id?: string, path?: string}` | Close one Reader tab, defaulting to the active tab; unsaved PDFs ask before closing |
| `app.source.goto` | `{line: 75}` | Jump to a one-based line in the active read-only source preview |
| `app.media.inspect` | `{}` | Inspect the active audio/video file, codec, duration, playback state and folder queue summary |
| `app.media.queue` | `{offset?: number, limit?: number}` | List up to 100 local queue entries per call, including real music tags when present |
| `app.media.play` / `app.media.pause` | `{}` | Start or pause local playback in Reader |
| `app.media.seek` | `{seconds: number}` | Seek to a nonnegative time in the active media file |
| `app.media.volume` | `{value: number}` | Set local player volume from 0 (mute) to 1 (full); reflected in `app.media.inspect` |
| `app.media.speed` | `{value: number}` | Set audio or video playback speed from 0.25× to 4×; the UI offers six presets |
| `app.media.previous` / `app.media.next` | `{}` | Play the previous or next file in the opened folder's current order |
| `app.media.sort` | `{by:"name"|"artist"|"album"}` | Reorder the local music queue without changing files |
| `app.media.fullscreen` | `{enabled: bool}` | Enter or leave video or music full screen |
| `app.image.fullscreen` | `{enabled: bool}` | Enter or leave the active read-only image's full-screen view |
| `app.theme` | `{dark:true/false}` | Select the shared light/dark theme |
| `app.settings` | `{page:"appearance"/"voice"/"clipboard"/"files"/"info",visible:true/false}` | Open or close the shared Settings window for inspection; Info lists Reader formats and default handlers |
| `app.dictation.global` | `{enabled?: bool}` | Inspect or change the opt-in systemwide hold-to-dictate shortcut; only enabled in an authorized app session |
| `app.folders.choose` | `{}` | Show the native chooser; owner must answer |
| `app.folders.list` | `{}` | Already selected folders and available listings |
| `app.folders.remove` | `{path}` | Stop listing; files are untouched |
| `app.quit` | `{}` | Request a normal quit, guarded across all workspaces |
| `app.quit.cancel` | `{}` | Dismiss the shared quit prompt |
| `ui.screenshot` | `{path}` | PNG of the whole Polymath window |

Home library scans read metadata only inside folders selected by the owner, without recursion or symlink traversal. Opening a media file also reads supported sibling files in that file's folder for its temporary playback queue; it does not recurse or follow symlinks. Cancel changes nothing. Do not choose folders or bypass permissions on the owner's behalf. Local agent control is not a filesystem sandbox; stay within the user's task and save scope. Document web links require an explicit destination confirmation and then use the macOS default browser.

Keep Polymath visible for screenshots. After a theme/workspace change allow rendering to finish; if a first native capture omits cached document pixels, take another and verify the document through the engine as well. A screenshot alone is not evidence that saved content is correct.

Existing `iris_*` tool calls are accepted as compatibility aliases, while discovery lists the `polymath_*` names. The native bundle is `dist/Polymath.app`; the source repository is `polymath`.
