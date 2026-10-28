# Local validation — Polymath 0.4.18, 2026-10-28

The editing toolbar microphone is now limited to Writer, Sheets, Slides and PDF Tools; Home, Reader, Images and Video no longer offer a click-to-dictate action with nowhere to put its result. The systemwide hold shortcut accepts Command + Option from either side of the keyboard, including mixed pairs. A short hold delay and key-down cancellation keep ordinary Command-Option-key shortcuts from starting a recording.

The installed 0.4.17 helper received synthetic left Command and left Option events but never started dictation, confirming the reported failure: it listened only for the right-side keys. The revised helper observed start and stop with the left pair, and a quick Command-Option-key sequence produced no start. All 34 Rust tests passed, including an accessibility check of microphone visibility across every workspace; the debug app passed the global-helper lifecycle test. Live speech capture in an external app still needs a spoken user test after installation.

The signed 0.4.18 bundle passed synthetic left-side shortcut detection, cross-app text insertion, helper lifecycle, and the full native smoke test. Home and Writer screenshots were inspected to confirm that the microphone is absent on Home and present in Writer. The DMG and bundle signature verified. The previous 0.4.17 app and installer are archived under `../installers/previous-versions/`. The installed 0.4.18 helper passed synthetic hold tests for both left and right Command + Option; `/Applications/Polymath.app` is running with the helper enabled. All 119 configured file defaults still resolve to Polymath. The installed executable and shared installer hashes match the tested files.

Installed and signed-bundle executable SHA-256: `52ee07066992118abb8ae031fa1cd4499645b3361a9c2a374cd1d798ebc60155`

DMG SHA-256: `2e9b3898f4c98313e79614dd89bdb4384c6a6975a5f155f1de172bc4cb737783`

## Previous Polymath 0.4.17 validation — 2026-10-28

Polymath now has an opt-in systemwide hold-to-dictate shortcut in Settings → Voice. Holding right Command + right Option starts the bundled native recorder; releasing either key runs the existing local Whisper model and sends the transcript to the app that had focus at recording start. The helper shows a small recording/transcription pill, preserves the clipboard after paste when possible, and falls back to copying the transcript if automatic insertion is unavailable. The helper exits when the setting is turned off or Polymath quits. It uses no network service and loads the speech model only after recording.

The Swift helper compiled and its synthetic modifier-key check observed start and stop. A separate native text editor received a transcript through the helper's paste path, both before and after signing. The authenticated Polymath lifecycle test passed with the signed app: enable, permission/ready status, disable. The focused Rust event-parser test, formatting, Python syntax, whitespace check, optimized locked build and full native smoke test passed. The native screenshot `target/native-qa/global-dictation-settings.png` was inspected; the new Voice card, switch and shortcut instructions fit inside Settings. Live microphone recording and insertion into individual third-party editors still require a real user-granted dictation session.

The signed bundle and DMG verified. The prior 0.4.16 app and DMG are archived under `../installers/previous-versions/`. `/Applications/Polymath.app` was installed and launched; its saved preference enables global dictation, its helper runs and reports “Ready,” and its signed executable hash matches the tested bundle. A new Launch Services process confirmed all 119 configured document extensions resolve to the installed app, while website links remain with the browser. The shared installer copy matches `dist/Polymath.dmg`.

Installed and signed-bundle executable SHA-256: `5b8f2b894fa9e60b5b8de56856fc7e69408eba81261101411198753f43e723ba`

DMG SHA-256: `15fda4bc4178bb933471d99ed3fc613cead74feaaa83db01d2f61c167d351e9d`

## Previous Polymath 0.4.16 validation — 2026-10-28

Recent files are checked on launch and at most once every three seconds when the UI updates. Paths that no longer point to files are removed from Home, Reader, and the Reader/Writer/Sheets/Slides editor histories, then the cleaned lists are saved. Clicking a stale Recent row before the next check removes it without a file-not-found popup. Favorites are kept separately so an unavailable volume can be reconnected without losing pins. Open Reader tabs and document data are not changed.

The focused Rust regression tests passed for silent removal across every Recent history and for the existing missing-file open path. A native test with isolated preferences passed both launch-time cleanup and removal after a file was moved while Polymath was running; the updated preferences file contained no stale entry.

The locked optimized build, formatting, Python syntax and whitespace checks passed. The signed 0.4.16 bundle passed the native Recent cleanup test and the app-wide smoke test; the DMG passed `hdiutil verify`. The 0.4.15 app and installer are archived under `../installers/previous-versions/`. `/Applications/Polymath.app` is running 0.4.16 with an executable hash matching the signed bundle. Its actual saved preferences now have zero missing paths in Home/Reader, Writer, Sheets and Slides Recent histories. A fresh Launch Services check confirmed all 119 configured document extensions still resolve to the installed app. The installer copy in `../installers/Polymath.dmg` matches `dist/Polymath.dmg`.

Installed and signed-bundle executable SHA-256: `3909bd33593d587378be2f21ed9a54c3d2b9b354a77aabc5199924b1e525f321`

DMG SHA-256: `13577d941bbb55336f3b319b836f1f8104b4827d4a19e17dd4107d0171bd6c63`

## Previous Polymath 0.4.15 validation — 2026-10-28

Polymath 0.4.15 is an app-wide native UI audit. Home now reveals keyboard-focused actions and keeps long folder names compact; Reduce motion also stops animated focus scrolling. Settings has a bounded, scrollable dialog with shared typography, reachable folder removal and accurate file-handling and narration states. Reader's landing page uses the full window width. Its active file tab scrolls into view, SVG source and output remain side by side, CSV search says “Filter rows,” and PDF Tools names its complete sidebar “Tool catalog.” The image viewer starts at the image instead of repeating the tab filename, and compact windows reserve space for wrapped controls.

Video removes the duplicate Now Playing heading and codec footer, uses more of the available height at the source aspect ratio, and keeps its seek, centered transport, time and volume visible. Full-screen and Fit are named icon actions; Fit appears only after zooming. The separate Video timeline also hides its Fit action until zoomed. Embedded Sheets and Slides no longer repeat the host Home button; Slides ribbon groups can scroll when the window is narrow. An empty Images library no longer shows an empty filmstrip or photo-only footer controls. Writer retained its existing editing ribbon and Save As action after visual review.

The locked debug build and native Reader visual QA passed. Light and dark captures show the full-width landing page, overflow tabs, source/SVG previews, and a 1280 × 800 synthetic video in normal and full-screen playback. The `reader-source-last-line.png` capture shows JSX line 75 fully visible at the bottom, addressing the reported blank lower viewport. Native smoke passed routing, previews, workspace switching, save/reopen and the unsaved-change guard with screenshots. The Home UI suite passed 12 tests, and the Settings gear regression passed. A focused new Images empty-library capture confirmed only library navigation remains in its footer. The signed bundle also passed Video Studio's trim, reorder, merge, audio and export end-to-end test. No personal media files were modified by these checks.

`tests/reader_resource_probe.py` opened 15 synthetic 400-line JSX files, 15 copies of a 243 KB PDF and one paused 4 MB video in the signed app. An isolated second run reported a macOS physical footprint of 203 MiB with Reader empty, 197 MiB with 15 source tabs, 203 MiB with 30 document tabs, and 207 MiB with the video tab. After closing the PDF and video tabs and idling for 20 seconds, it was 197 MiB at 0% sampled CPU. A first run shortly after launch and tab changes showed 237–306 MiB and 62–84% sampled CPU. macOS memory compression, rendering state and recent activity make these snapshots variable; they do not establish a fixed cost per tab or foreground playback load.

The final locked optimized build, Rust formatting, Python syntax and whitespace checks passed. Native Reader visual QA and the full native smoke test passed again on the final signed bundle. The final DMG passed `hdiutil verify`; `dist/Polymath.dmg` matches `../installers/Polymath.dmg`. The previous 0.4.14 app and DMG are archived under `../installers/previous-versions/`. `/Applications/Polymath.app` was installed and launched as 0.4.15; its executable hash matches the tested bundle. A fresh Launch Services check confirmed all 119 configured document extensions resolve to the installed app, while HTTP/HTTPS remain with the browser. The app is ad-hoc signed and is not Apple-notarized.

Installed and signed-bundle executable SHA-256: `d5ed2891a73e6830fd8ee70d166f4fa9874757b45d2cc2c9c458cbff6e29fcfc`

DMG SHA-256: `26c087817db8b79aff729488b3a01bff10d183630393ee139cde2b0f811287ae`

## Previous Polymath 0.4.14 validation — 2026-10-28

Polymath 0.4.14 replaces the two stacked Reader source bars with one integrated file strip at normal window widths. The active file uses a flat editor tab with a Reader-blue underline; file type, line count, read-only hint, Find, Copy and Outline sit immediately beside the tabs. A narrow window retains a compact secondary row so the tabs and actions remain reachable. The source and rendered SVG preview now begin directly below the shared strip.

The locked debug and optimized release builds passed. Native Reader visual QA passed on the signed release bundle. Screenshots of one SVG tab and three source tabs were inspected in dark mode; the QA also covered the light theme, Reader landing and video playback. The fixture uses the reported `wiredleap_id_card_vertical (1).svg` filename. `tests/native_smoke.py` passed its routing, preview, editing and quit-guard workflow with screenshot capture disabled; the focused Reader screenshots were captured separately. Rust formatting, Python syntax, and whitespace checks passed. No personal files were modified.

The 0.4.14 app passed strict code-signature verification. `hdiutil verify` passed for the DMG, and `dist/Polymath.dmg` matches `../installers/Polymath.dmg`. The previous 0.4.13 app and installer were archived under `../installers/previous-versions/`. The installed app quit normally before replacement, and `/Applications/Polymath.app` is running 0.4.14 with an executable hash matching the tested bundle. A fresh Launch Services check confirmed all 119 configured document extensions resolve to the installed app; HTTP and HTTPS remain with the browser. The app is ad-hoc signed and is not Apple-notarized.

Signed bundle and installed executable SHA-256: `42e0bfce839297523b32879d05c1ce6f21b0374a61925fbbefc25d2dc5ea28a5`

DMG SHA-256: `c7ad228eb993aaae2113b17bf54f670cd4800275a3380627386f5466c5fd5e63`

## Previous Polymath 0.4.13 validation — 2026-10-28

Polymath 0.4.13 redesigns Reader's source bar. The open tab now carries the filename without a second, oversized title below it. The source toolbar is 43 points tall on a wide window and groups file type, line or table count, read-only state, search, copy and outline controls. Long tab labels allow 36 characters, enough to show `wiredleap_id_card_vertical (1).svg` in full. Narrow windows stack the controls in a 75-point bar. The copy and outline controls have accessible names and hover help.

The locked debug and optimized release builds passed. Native Reader visual QA passed on the signed release bundle. Synthetic SVG with the reported filename was opened beside shell and JSX tabs; light and dark screenshots were inspected for bar alignment, tab labels, and the side-by-side SVG source and preview. `tests/native_smoke.py` passed its routing, preview, editing and quit-guard workflow with screenshot capture disabled; the focused Reader screenshots were captured separately. `cargo fmt --all --check`, Python syntax checks and `git diff --check` passed. No personal files were modified by these checks.

The 0.4.13 bundle passed strict code-signature verification. `hdiutil verify` passed for the DMG, and `dist/Polymath.dmg` matches `../installers/Polymath.dmg`. The previous 0.4.12 app and installer were archived under `../installers/previous-versions/`. The installed app quit normally before replacement, and `/Applications/Polymath.app` is running 0.4.13 with an executable hash matching the tested bundle. A fresh Launch Services check confirmed all 119 configured document extensions resolve to the installed app; HTTP and HTTPS remain with the browser. The app is ad-hoc signed and is not Apple-notarized.

Signed bundle and installed executable SHA-256: `6e0b913a0f6a35b90f509f7865c50810b6e74516dc407e6650a801617ed0b8bf`

DMG SHA-256: `274e3577a51fb8695b08421901f7cd2ff0a60ead9e68658b425af47c089dc24e`

## Previous Polymath 0.4.12 validation — 2026-10-28

Polymath 0.4.12 tightens the native Reader interface. File tabs remain on one line, keep the extension visible when shortened, and have a shared new-tab button. Source preview has a compact file header and one copy action. Reader's landing page replaces the oversized empty Favorites panel with an inline state and shows recent files as full-width rows with type, location and saved page. Video preview now uses the available height at the source aspect ratio, with denser controls. Normal and full-screen playback share centered icon transport, including 30-second jumps. Reader-first opens, saved documents and the Video Studio export workflow remain intact.

The one-job locked release check and optimized build, Rust formatting, Python syntax and whitespace checks passed. `tests/reader_visual_qa.py` passed against the final signed release bundle and captured light/dark Reader landing, multiple source tabs including a long name, a populated recent/favorite list, actual video playback, and full-screen video. The screenshots were inspected. `tests/native_smoke.py` passed its full functional workflow on the final signed bundle with screenshot capture disabled; its earlier screenshot run stopped at an intermittent macOS image-fullscreen capture timeout before the source and video checks. The focused capture succeeded for the changed surfaces. No personal files were modified by these tests.

The 0.4.12 bundle passed strict code-signature verification, and `hdiutil verify` passed for the DMG. `dist/Polymath.dmg` and `../installers/Polymath.dmg` have identical SHA-256 hashes; the prior DMG and installed app are archived as `../installers/previous-versions/Polymath-0.4.11.*`. The installed `/Applications/Polymath.app` quit normally before replacement and is running as 0.4.12. Its executable hash matches the tested bundle. A fresh Launch Services check confirmed all 119 configured document extensions resolve to the installed app; HTTP/HTTPS remain with the browser. The app remains ad-hoc signed, without Apple notarization.

Signed bundle and installed executable SHA-256: `ff6381d0cd786408c3f490d302ee54bf5c1b93c4d7e59960bd1355cc18f87097`

DMG SHA-256: `da5497b3412680a0b3331330d5735b4ce58bed5ec41ca689bfb0eca37360c60f`

## Previous Polymath 0.4.11 validation — 2026-10-28

Polymath 0.4.11 adds a separate native Video workspace. Videos still open in Reader first; Edit video deliberately adds a clip to a timeline. The editor supports frame thumbnails, trim handles and precise in/out values, drag or keyboard reorder, a separate soundtrack, original-audio stripping or mixing, quality presets, an explicit MP4 destination, progress and cancellation. The preview is a still frame scrubber; live composite playback, transitions and multiple soundtrack layers are not implemented. FFmpeg and ffprobe are installed locally and invoked on demand, not bundled.

`CARGO_BUILD_JOBS=1 cargo check --release --locked`, the optimized one-job build, Rust formatting, Python syntax and whitespace checks passed. `tests/video_editor_e2e.py` passed on the final signed bundle: two synthetic two-second clips were trimmed to 1.5 seconds each, reordered and joined; decoded output pixels confirmed blue then red. The Small preset produced an 854 × 480 H.264/AAC MP4 of 3.021 seconds. The test verified source-audio stripping, a soundtrack delayed by 0.5 seconds, a second export that mixed the source and soundtrack, the finished file appearing in Home Recent, and unchanged SHA-256 hashes of every input. The light timeline screenshot was inspected after short clips were made to fill the available lane; its audio track and export control are visible.

`tests/native_smoke.py` passed with light/dark screenshots and functional checks for Home, Reader, Writer, Sheets, Slides, Images, Video Studio, PDF and office previews, Markdown, CSV, SVG/source, media, save/reopen, tabs and quit guards. The empty Video Studio screenshot was inspected after its stage and track-lane redesign. `tests/downloads_e2e.py` passed on copies of local PDF, DOCX, MD, XLSX, CSV, PPTX, PNG, JPG and WebP files and PDF Tools operations; originals were not modified.

The 0.4.11 bundle passed strict code-signature verification. `hdiutil verify` passed for its DMG; `dist/Polymath.dmg` and `../installers/Polymath.dmg` have the same SHA-256. The previous shared DMG was archived as `../installers/previous-versions/Polymath-0.4.10.dmg`. The old `/Applications/Polymath.app` process remained running and was left untouched, so this turn did not replace the installed app. A read-only Launch Services check confirmed all 119 configured extension defaults still resolve to that installed app; defaults have not yet been rechecked against an installed 0.4.11 bundle. The new app remains ad-hoc signed, without Apple notarization.

Signed bundle executable SHA-256: `a35d0ebed7ba517d8ff1ef3b904efd8f85b6f453397c6f988209c64d43252d87`

DMG SHA-256: `5de2931dd50cbb50ae9eba1951a5c9d5270a3746eb29e65822ccd6806c70f8c2`

## Previous Polymath 0.4.10 validation — 2026-10-28

Reader gained persistent tabs across PDF, office, code/text, CSV/TSV, image, audio and video previews. Ordinary opens route through Reader; editing requires an explicit handoff. The native CSV table preserves columns and horizontal scrolling. SVG previews render beside source, the long-source scrolling gap was removed, and PDF/office reading positions and Favorites remain local. Images and video gained pinch zoom and full screen; media queue and editor sidebars start closed, and Slides Notes start closed. The signed 0.4.10 bundle passed the native smoke suite, the Downloads copy-based regression suite, strict code-signature verification and DMG verification. The 0.4.10 DMG is now preserved under `../installers/previous-versions/`.

## Previous Polymath 0.4.9 validation — 2026-10-28

Polymath 0.4.9 uses the local egui-shadcn port for Reader's media transport, queue and sorting controls, and an egui_extras table for long folder queues. Reader opens local images as read-only previews with an Edit action, and plays local audio/video in its own view. A folder queue reads music title, artist and album tags, sorts by name/artist/album, advances to the next file automatically, and provides a native full-screen video view. The final light/dark music and full-screen video screenshots were inspected; an overlapping transport time label found in the first candidate was fixed before release.

The optimized one-job release build passed. Native checks passed for PNG/JPG previews, MP3/MP4 playback, seek, pause, workspace changes, a three-song tag/sort/auto-next queue, a two-video queue, full-screen video, unchanged input hashes, and clean quit. The bundled app also passed the synthetic native smoke check for Reader, Writer, Sheets, Slides, Markdown, CSV, source files, read-only office previews, light/dark rendering, save/reopen and unsaved-work guards. All 12 Reader document-link tests passed, including the combined host's browser handoff. The signed 0.4.9 bundle passed strict code-signature verification; its DMG passed `hdiutil verify`. The local egui component port and craft-quit licenses are included in the bundle.

The old installed app quit normally and was archived before `/Applications/Polymath.app` was replaced and launched. The installed executable hash matches the tested signed bundle, and the shared `../installers/Polymath.dmg` hash matches the verified build. The explicit document-default utility backed up prior handlers, then a fresh process verified all 119 supported extensions resolving to the installed app. HTML and HTTP/HTTPS still resolve to Google Chrome. Web links clicked in documents show a destination prompt and use that default browser after confirmation. The app is ad-hoc signed and is not Apple-notarized.

Signed bundle executable SHA-256: `74f480e39cbfb145d12dbde8be10d3ddeba98d93cf2d4ca0f3dfc108c8eb2d2f`

DMG SHA-256: `e9e8e6c7abd676c286cf2f6f3df65166124ba601029458fe6cca6422dee7a78e`

## Previous Polymath 0.4.8 validation — 2026-10-27

Polymath 0.4.8 (build 15) fixes PPTX paragraphs that put font size, weight, family and color in `a:pPr/a:defRPr` while leaving runs unstyled. The fix is a focused port of DeckCraft upstream commit `f9d5a431`; the other fetched upstream branches were reviewed but not merged. The PPTX round-trip suite passed (16/16), and an unmodified 17-slide local deck rendered with restored headings, body hierarchy and dark-slide title. A copy saved and reopened by the updated Slides CLI produced identical pixels on all 17 slides; the original file's SHA-256 did not change. Against an independent LibreOffice PDF export, mean absolute pixel error at half size improved from 8.66 to 6.21 across all slides. This is a targeted compatibility check, not proof of complete PowerPoint parity.

The signed 0.4.8 bundle opened that deck in Slides (17 slides) and Reader (read-only office preview), and its first-slide native screenshot was inspected. Native smoke passed its functional routing/editing checks but terminated with macOS AppKit's `_NSTouchBarFinderObservation` exception during the automated quit; the same intermittent quit issue appeared in earlier short QA sessions. The running 0.4.7 app accepted a normal quit request before replacement. The signed 0.4.8 bundle and verified DMG are installed at `/Applications/Polymath.app` and `../installers/Polymath.dmg`, with the prior 0.4.7 copies under `../installers/previous-versions/`. All 106 declared macOS file handlers still resolve to the installed app. The app remains ad-hoc signed without Apple notarization.

Signed bundle and installed executable SHA-256: `36839afa55da7f9fa1c2591b48dbd72e7f45f6005feda57e4bfde2382760303b`

DMG SHA-256: `f3b3adf919076c21d84020cd8db165064b33ea994afe4d31ad6d6fc1523767b7`

## Previous Polymath 0.4.7 validation — 2026-10-27

Polymath 0.4.7 (build 14) is installed and running at `/Applications/Polymath.app`. The signed installed executable matches `dist/Polymath.app`, and the verified DMG is at `dist/Polymath.dmg` and `../installers/Polymath.dmg`. The prior 0.4.6 app and DMG are archived under `../installers/previous-versions/`. No personal documents or app preferences were removed.

Home no longer has a fixed library sidebar. Its Open action has a document glyph and a separate keyboard shortcut, and the recent/folder controls sit with the file listing. A short-window search scroll issue found during testing was fixed. Reader now remembers the last page of each file and supports Favorites. The full Home UI suite passed (11/11), Reader memory unit tests passed (2/2), and the native Reader restart test passed with an isolated preferences file. The signed bundle passed `tests/native_smoke.py` before the final Home scroll adjustment. Rust formatting and Swift association utility typechecks passed.

The explicit association utility backed up 106 previous file handlers to `~/Library/Application Support/Polymath/default-handlers-before-20261027-185025.json`, set every extension declared by the installed bundle, then independently checked all 106 resolving to `/Applications/Polymath.app`. This includes PDF, office, source, image, Markdown, text and CSV types. `.zip` and legacy `.xls` remain outside the declared list because their workflows are not implemented. No macOS security setting was changed. The app remains ad-hoc signed rather than Apple-notarized.

Signed bundle and installed executable SHA-256: `2327cd826d71bfc2750096f14c65963adad585659eb951e5b70f3a49fb0244bf`

DMG SHA-256: `97988966d3d3dc8e50fa6a98f1d7b70097da55d43f48647242aed2a1a3e9614b`

## Previous Polymath 0.4.6 validation — 2026-10-15

Polymath 0.4.6 (build 13) integrates a local egui 0.36 port of egui-shadcn for selected shared controls, uses egui_dock for a resizable Document/Outline split in Reader's source preview, and uses egui_extras to virtualize the outline list. The Home primary action, Reader toolbar, and Settings controls were visually inspected in the signed app's light and dark screenshots. The Home action's foreground contrast was corrected after an initial render showed dark text on a dark button.

The focused source tests passed (2/2), and the full Home UI tests passed (11/11) before the final button-color correction. The locked optimized release build passed with one Cargo job. `cargo fmt --all --check`, shell packaging syntax checks, and Python syntax checks passed. The signed 0.4.6 bundle passed `tests/native_smoke.py` with screenshots: PDF and office routing, Markdown/CSV/source handling, read-only previews, workspace switching, save/reopen, and unsaved-work protection. The post-build screenshots confirmed the button-color correction and visible source outline. Strict code-signature verification and `hdiutil verify` passed.

The previous 0.4.5 app quit normally before the tested 0.4.6 bundle was installed at `/Applications/Polymath.app` and launched. Its executable hash matches the signed bundle. The new verified DMG is at `dist/Polymath.dmg` and `../installers/Polymath.dmg`; the 0.4.5 installer and app were archived under `../installers/previous-versions/`. Spot checks of current macOS defaults showed DOCX, XLSX, Markdown, plain text, CSV and TSX resolving to `/Applications/Polymath.app`. The app remains ad-hoc signed rather than Apple-notarized.

Signed bundle and installed executable SHA-256: `4520a68e5b3ee7f86d0ae04b4578f881defe5fc3f920cd1eb0915412c4665e9a`

DMG SHA-256: `71cec74816297fd4eda090476cdcfd435742a6cc66e01dda3089c72af885112c`

## Previous Polymath 0.4.5 validation — 2026-10-15

Polymath 0.4.5 (build 12) is packaged as the signed `dist/Polymath.app` and verified `dist/Polymath.dmg`, with a matching copy at `../installers/Polymath.dmg`. The prior 0.4.4 app and installer were archived under `../installers/previous-versions/`. The previous installed process had exited, so `/Applications/Polymath.app` was updated through a signed staged bundle and launched. Its executable hash matches the tested signed bundle, and all 18 existing document defaults still resolve to the installed path.

Five selected upstream ports were integrated without merging the newer branches: Reader form CSV preserves carriage returns; Writer uses macOS line-start/end shortcuts; Sheets can disable value AutoComplete and dismiss a proposal with Backspace/Escape; Slides preserves detail in cropped pictures; and Images zooms to the point clicked in either Before/After pane. The focused Reader, Writer, Slides and Images regressions passed, as did three Sheets interaction tests. `cargo fmt --all --check` passed in all six source trees, and the locked offline release build passed with one Cargo job.

The signed 0.4.5 bundle passed `tests/native_smoke.py` for PDF, office, Markdown, CSV and source routing, office previews, workspace switching, save/reopen and unsaved-work protection. The first run timed out during screenshot capture while the installed 0.4.4 app was in the foreground; rerunning the same functional checks with `POLYMATH_SKIP_SCREENSHOTS=1` passed. Strict bundle code-signature verification and `hdiutil verify` passed. Zip/Archive Manager and Drawboard/Excalidraw remain unimplemented.

Signed bundle executable SHA-256: `502df3153c6f91e408b73e83052696852a9cbb9cc7c74416f2cc79048c2030c0`

DMG SHA-256: `87edd7797bd516946260755bd3d1614436836df9475fe804d9d4e460ae94e15b`

## Previous Polymath 0.4.4 validation — 2026-10-15

Polymath 0.4.4 (build 11) is installed and running at `/Applications/Polymath.app`. The matching signed bundle is `dist/Polymath.app`; the verified installer is `dist/Polymath.dmg`, copied to `../installers/Polymath.dmg`. The 0.4.3 application and DMG are archived under `../installers/previous-versions/`.

Home now uses larger illustrated editor surfaces, stacked Reader/PDF Tools cards in narrow windows, a visible recent-work strip, and rail navigation that scrolls to the selected file list. The shared header marks the active workspace with its icon and accent color. The full Home test suite passed (11 tests), including light/dark and wide/narrow rendered screenshots, keyboard entry, search and filter behavior, and reduced motion; the snapshots were inspected. The focused Reader OCR UI test passed for model-loading feedback, page progress and cancellation. OCR remains the local English `ocrs` engine; [model comparison and limitations](OCR.md) are documented. The compatible `egui_dock` dependency was added, and [UI library compatibility](UI_STACK.md) is recorded without changing the existing editor panel structure.

The locked offline release build and `tests/native_smoke.py` passed on the signed bundle. Native smoke covered PDF, DOCX, XLSX, PPTX, Markdown, CSV and source routing; Reader office previews; light/dark rendering; repeated workspace switches; save/reopen; and inactive unsaved-work protection. Rust formatting, whitespace, packaging syntax and strict bundle code signing passed. `hdiutil verify` passed for the new DMG. The installed executable hash matches the tested bundle, and macOS resolves all 18 supported Word, text and spreadsheet defaults to `/Applications/Polymath.app`. No `.xls` default was changed. The app remains ad-hoc signed rather than Apple-notarized.

Installed executable SHA-256: `65786db78b1a2d1a0bf9ed5c2ae9dc797a142899fd4450cfd11a50ade675cd81`

DMG SHA-256: `3300b843e5de8d224fd1a09c2eba6c1df36d6b07ee95cfb5b6bf83c75707ccd5`

## Previous Polymath 0.4.3 validation — 2026-10-15

Polymath 0.4.3 (build 10) is installed and running at `/Applications/Polymath.app`. The matching signed bundle is `dist/Polymath.app`; the verified installer is `dist/Polymath.dmg` and `../installers/Polymath.dmg`. The previous 0.4.2 app and installer are archived under `../installers/previous-versions/`.

The app now routes Excel templates (`.xltx`, `.xltm`) to Sheets and previews them in Reader. Sheets treats `.xlsm`, `.xltx`, and `.xltm` as imports: Save asks for a new path and Save As refuses to overwrite those original formats, requiring `.xlsx` instead. The focused Sheets engine test passed for all three formats. The signed native app smoke test passed, including both new template routes, office previews, text/source formats, light/dark rendering, save/reopen and quit guards. Formatting, Swift/Python syntax, app code-signature verification, and DMG checksum verification passed.

Using Launch Services, `packaging/default-documents.swift --set` assigned Polymath as the macOS default for 18 supported Word, text, and spreadsheet extensions. `--check` resolved every type to `/Applications/Polymath.app`. A separate Finder test opened synthetic `.docx`, `.xlsx`, and `.xltx` files with plain `open` and verified that the installed app received each file in Writer or Sheets. The utility changes defaults only when explicitly invoked. Legacy `.xls` remains assigned to LibreOffice because Sheets does not import that binary format yet. The app is ad-hoc signed rather than Apple-notarized; no security setting was changed.

Installed executable SHA-256: `f7387652cc725c0ba026ccf0d5a81de6af4343ad8308b92b8d9efe2f8c00b852`

DMG SHA-256: `1c526fda778ff4550ee6392dba448dad59288815408473e819ca95fc2e633179` (260,201,726 bytes)

The [upstream feature check](UPSTREAM_STATUS.md) records the current source-repository heads and representative new features without merging them into Polymath.

## Previous Polymath 0.4.2 validation — 2026-10-15

Polymath 0.4.2 aligns the shared header's brand divider with the fixed Home library rail and aligns the logo with the rail's content inset. The brand region and rail use one width constant so they stay aligned as the Home layout changes. The 0.4.1 changes carried forward make Writer's File tab a compact menu, show recently saved Writer files on Home without scrolling, and merge the editors' existing recent-file histories into Home.

The locked offline release build passed with one Cargo job. Native light and dark Home screenshots were inspected at 2880 × 1800: the header and rail strokes meet at the same pixel column, and the logo starts at the Library text inset. `tests/writer_save_as_native.py` passed for DOCX/PDF output and Home recent-file visibility. `tests/native_smoke.py` passed on the signed bundle for Reader previews, Markdown/CSV/source routing, editing, save/reopen, both themes, and the unsaved-work quit guard. Formatting, bundle code-signature verification, and DMG checksum verification passed.

Polymath 0.4.2 (build 9) is installed and running at `/Applications/Polymath.app`. Its executable hash matches the tested signed bundle. The 0.4.1 app and DMG are archived in `../installers/previous-versions/`; the current DMG is `../installers/Polymath.dmg` with a matching `dist/Polymath.dmg`. The normal macOS default handlers for `.md` and `.csv` still resolve to the installed app. The app remains ad-hoc signed rather than Apple-notarized; no security setting was changed.

Installed executable SHA-256: `32af01c418836de5aa84be472059b76ab334651e20a0434e36ce7f4360d0c3d8`

DMG SHA-256: `49d2a1f268d3d701ba40ba90121ab0fb5088191b2a4f8346e237dfc0f10736bd` (260,205,411 bytes)

## Previous Polymath 0.4.1 validation — 2026-10-15

The 0.4.1 release bundle and DMG were built and ad-hoc signed. The Writer Save As native check passed for DOCX and PDF output, the Home recent list, and light/dark screenshots; the synthetic four-editor native smoke test passed. The executable was installed at `/Applications/Polymath.app` and launched. Strict code signing and DMG verification passed; the installed executable matched the tested release binary. Normal macOS default handlers for `.md` and `.csv` still resolved to that installed app. The earlier 0.4.0 DMG and app were archived in `../installers/previous-versions/`.

## Previous Polymath 0.4.0 validation — 2026-10-14

Polymath 0.4.0 was the combined native macOS application built from this source tree. Its installer was `dist/Polymath.dmg` with a matching copy in `../installers/Polymath.dmg` at the time of validation; that installer collection is linked from `~/Desktop/Polymath Installers`. At the time, the installed `/Applications/Polymath.app` remained the running 0.3.0 build so the new build did not replace a possibly unsaved session.

The update adds the six-workspace Home design, color-coded but neutral document surfaces, and consistent softer card, panel, menu and control corners in both themes. Reader now previews office documents and source files, with PDF Tools in a separate workspace. The workspace guides, `CODEX.md`, and the local agent bridge describe the supported workflows.

The app is ad-hoc signed, not Apple-notarized. No system security setting was changed. Live dictation still requires the owner's normal macOS microphone permission. This build omits optional `craft-fonts`, so Chinese and Japanese text in Images may display as empty boxes.

## Previous Polymath 0.3.0 validation — 2026-10-14

At that time, the signed ARM64 application was installed at `/Applications/Polymath.app` (version 0.3.0, build 6). Its installer was `../installers/Polymath.dmg`, also linked from `~/Desktop/Polymath Installers`; its source app was `dist/Polymath.app`. The installed and source executables had the same SHA-256, as did the project and shared DMGs.

- The optimized offline release build passed with Rust 1.95.0 on macOS 26.6.2, using one Cargo job. Strict release Clippy (`--all-targets --no-deps -- -D warnings`) and formatting passed. All 13 application tests and 11 Home UI tests passed. The Settings gear test opens Appearance, Voice and Files & privacy through the real egui accessibility tree.
- `tests/native_smoke.py` passed on the signed release: Settings screenshots in light and dark, PDF/DOCX/XLSX/PPTX opening, Markdown and CSV content, theme changes, repeated workspace switches, inactive dirty-workspace quit guards, save and reopen. Settings screenshots were inspected. The Home empty-search Clear filters button was moved above the empty state after the UI test exposed it below the visible area.
- `tests/downloads_e2e.py` passed on anonymized copies of real Downloads files for PDF, DOCX, Markdown, XLSX, CSV, PPTX, PNG, JPG and WebP. The PDF tools check passed for local manipulation and conversion workflows, including split and OCR. Original files were unchanged. `tests/images_smoke.py` passed for image import, a non-destructive adjustment and PNG export. `tests/macos_bundle.py` passed six Finder Open With routes and real MCP initialize/list/document requests.
- `tests/voice_smoke.py` passed with the bundled multilingual Whisper CLI and model on synthetic Hindi and Tamil speech; the results used Devanagari and Tamil script. It also checked installed macOS voices, microphone metadata and the model checksum. Reader PDF text extraction has targeted tests. A synthetic Bengali sample previously produced Devanagari even with Bengali selected; the language picker indicates model support, not guaranteed accuracy. Live microphone capture and insertion still require the owner's normal macOS microphone permission prompt and hands-on acceptance.
- `codesign --verify --deep --strict` passed for the source and installed bundles; `hdiutil verify` passed for the generated DMG. The installed bundle includes the recorder, speech CLI, model, licenses and `NSMicrophoneUsageDescription`. The app is ad-hoc signed, not Apple-notarized. No system security setting was changed.
- The normal macOS default-handler API now resolves `.md` and `.csv` to `/Applications/Polymath.app`. Plain Finder `open` on synthetic files reached the installed app and routed Markdown to Writer and CSV to Sheets; the CSV value was checked. The setup script now detects a build-tree bundle with the same identifier and checks the exact resolved path.

Executable SHA-256: `016aba0504266ce67b1181520dd7b355b1b6ddcffc184c55efe9dee0f34f704c`

DMG SHA-256: `1607a03aa6f3b3eb01ad79f1cbba81a072f75e469936435e14af2525b2c453ee` (260,084,563 bytes)

## Previous Polymath 0.1.2 validation — 2026-10-14

At that time, the installed application was `/Applications/Polymath.app` (version 0.1.2, build 3, bundle identifier `local.polymath.app`). Its installer is preserved as `../installers/previous-versions/Polymath-0.1.2.dmg`; the shared installer contained 0.3.0 when the validation above was recorded. Earlier installers and their checksums are preserved in `../installers/previous-versions/`.

- The optimized macOS ARM64 release build passed with Rust 1.95.0, thin LTO and one Cargo job. The executable reports `Polymath 0.1.2`.
- All 10 Home/routing tests passed, including light/dark × wide/narrow × empty/populated layouts, keyboard interaction, short-window scrolling, search and type filters, reduced motion, accessible theme icons, case-insensitive Markdown/CSV routing, and restoration after another editor replaces Home's font families. The latter guards the native crash found during this update. The test harness uses `cargo rustc --locked --offline --release --test home_ui -- -C opt-level=1 -C lto=thin`; the application itself uses the normal release profile.
- `cargo clippy --locked --offline --release --all-targets --no-deps -- -D warnings`, Rust formatting, packaging shell/Python syntax, and whitespace checks passed. Font/icon attribution hashes and reproducible Manrope static font generation passed.
- `python3 tests/native_smoke.py` passed against the final bundle: PDF/DOCX/XLSX/PPTX opening, Markdown heading content, quoted and numeric CSV cells, both themes, repeated editor/Home switching, all four dirty-workspace quit guards, saving and reopening. Authentication rejection and owner-only endpoint permissions passed. Synthetic test documents and native screenshots are in `target/native-qa/`; no personal files were used.
- `python3 tests/macos_bundle.py` passed against the final bundle: all six Finder routes reached one application instance, and MCP initialize, discovery and a real `polymath_document` request succeeded. Home and representative editor screenshots were inspected; the moon icon no longer draws a long stroke outside its target.
- The application was installed in `/Applications`, and `codesign --verify --deep --strict` passed for the source and installed bundles. Their executable hashes match. `hdiutil verify` passed for the generated DMG; the shared installer copy has the same hash.
- The normal macOS `NSWorkspace` API set Polymath as default for Markdown and CSV. A final `--check` resolved both types to `/Applications/Polymath.app`. Opening synthetic `.md` and `.csv` files with plain `open` (without naming an application) reached the installed Polymath process, routed to Writer/Sheets, and returned the expected document/cell content. Both QA files closed cleanly.

Executable SHA-256: `d7c8bf97917c0085fafa94c4f6fc19e609e8ed4ee69c110a32e81c2c5bfafaf9`

DMG SHA-256: `f894da58d759b392e7e972eb4a6ff536857b093967c6af7a534bfbf2adb721ef` (57,516,451 bytes)

### Folder layout at 0.1.2

The outer folder is `~/Projects/polymath`; its independent repositories are `polymath`, `reader`, `writer`, `sheets`, `slides`, and `motion`. The combined application's source is the inner `polymath` directory. The shared Cargo cache remains `reader/target`. The previous `Projects/pdfcraft` and `iris-*` source directory names no longer exist.

All repository directory identities, Git histories and pre-existing uncommitted work were retained. Locked offline Cargo metadata resolves all dependencies from the renamed layout. Project-local Claude/Codex command paths and both Desktop shortcuts resolve. Motion's TypeScript check passed after its folder rename. Legacy engine package/binary names, asset provenance and compatibility identifiers remain internal; they are not the combined application's product name. No commits, pushes or global coding-client changes were made.

The following records describe earlier builds and their original names. Their limitations are historical; current native checks are recorded above. This remains targeted native macOS validation, not full upstream CI, Windows/Linux/wasm coverage or Apple notarization.

## Historical Iris 0.1.1 validation — 2026-10-13

The redesigned Home is built in `dist/Iris.app`. The updated `dist/Iris.dmg` is also at `~/Desktop/Iris Installers/Iris.dmg`. This turn did not install or replace an application in `/Applications`.

- Optimized macOS ARM64 release build passed with Rust 1.95.0 and one Cargo job. The executable reports `Iris 0.1.1`; the bundle is version 0.1.1, build 2.
- All six Home/routing tests passed, including eight light/dark × wide/narrow × empty/populated layout cases, long filenames, case-insensitive search and combined type filters, folder navigation, creation actions, and keyboard activation. The real `home_view` component is exercised through egui/accessibility, with synthetic in-memory paths only.
- Eight previews of the actual egui meshes were rendered with a test-only CPU renderer, then inspected in `target/home-design/`. They are component previews without the shared workspace bar, not native-window screenshots. The renderer is outside the shipped application. `IRIS_RENDER_HOME_TESTS=1` enables capture when executing the `home_ui` test binary.
- `cargo clippy --locked --offline --release --all-targets --no-deps -- -D warnings`, Rust formatting, packaging shell/Python syntax, and attribution hashes passed.
- The app passed `codesign --verify --deep --strict --verbose=4`. The DMG passed `hdiutil verify`; desktop/project installer hashes match. Packaging now uses `hdiutil makehybrid -hfs` followed by UDZO conversion, without attaching a temporary device. The original device-backed creation command failed with `Device not configured` in this session.
- Live native validation remains outstanding for 0.1.1. The QA process aborted before opening its control endpoint, and this execution session could not connect to the macOS GUI/service manager. The WGPU test renderer also failed device creation. The native four-editor smoke test, Finder routing, and mounted-installer inspection were therefore **not successfully rerun** for this version. The earlier 0.1.0 checks below are historical, not evidence for a live 0.1.1 launch.

Application SHA-256: `5a95cd0c330b746c1b76c8a51d3faab6994fbbddec9da5a4bdf4a8620034a958`

DMG SHA-256: `597b3a405833e8ae508b0c0f035785a3bb266bdb0ecdcc2051fdb77d55aa8d44`

## Workspace relocation — 2026-10-14

All six repositories and the installer collection now live under `Projects/polymath`. Reader is `iris-reader`; this combined app is `iris-suite`. `../installers/Iris.dmg` is the shared installer, with `~/Desktop/Iris Installers` pointing to that folder. The Desktop Motion launcher also points to the relocated source.

- All repository directory identities and Git HEADs were preserved; pre-existing work was retained.
- Local Cargo dependencies resolve from the new layout. The combined dependency graph passed locked, offline metadata resolution for `aarch64-apple-darwin`; its target directory is `../iris-reader/target`.
- Project-local Claude/Codex configuration paths resolve, including the parent workspace's combined bridge. No global client settings changed.
- Motion's local TypeScript check, packaging shell/Python syntax checks, and whitespace checks passed.
- The existing Reader CLI can find its OCR models via a relative `target/release/models` link. Use Cargo to invoke developer tasks/tests so compiled source paths are refreshed.
- The combined executable and shared DMG hashes still match the 0.1.1 values above. This relocation did not rebuild, install, or run native GUI validation; the limitations above still apply.

## Previous 0.1.0 baseline — 2026-10-11

Validated on macOS 26.6.2, Apple silicon. The installed application is `/Applications/Iris.app`; the installer is also in `~/Desktop/Iris Installers/Iris.dmg`.

- Full release application build passed with Rust 1.95.0 and one Cargo job.
- `cargo clippy --locked --offline --release --all-targets --no-deps -- -D warnings` passed; Rust formatting and Python/shell syntax checks passed.
- All 10 Rust tests passed: document and undo preservation, native close interception with an inactive dirty editor, repeated UI/font switches, file routing, bounded folder discovery, printing and audio error handling. The test harness was compiled with `cargo rustc --locked --offline --release --tests -- -C opt-level=1 -C lto=thin` and executed with `--test-threads=1`. The shipped application uses the normal optimized release profile.
- `python3 tests/native_smoke.py` passed against the final bundle. It opens synthetic PDF/DOCX/XLSX/PPTX files, checks both themes, rotates a PDF, edits the other three formats, switches repeatedly, verifies the shared quit guard includes all four dirty workspaces, saves, and reopens all four formats. It also checks token rejection and owner-only endpoint permissions.
- `python3 tests/macos_bundle.py` passed against the final bundle: all four Finder Open With routes reached the same instance; MCP initialize, tools/list and a real document inspection succeeded.
- Home and all four workspaces were visually inspected in light/dark screenshots. Home alignment and selected-tab contrast were corrected and checked again.
- Icon attribution hashes passed. The bundle contains all four original notices/asset ledgers, source asset licenses, dependency font/CMap license texts and OCR models.
- The app signature passed `codesign --verify --deep --strict`. The DMG checksum passed `hdiutil verify`; the mounted installer contained the matching executable, document associations, resources and Applications link.

Application SHA-256: `f6b0dd9806d0793314b8813e2cba9d4a9d945414045c897413c79a62a02c0a90`

DMG SHA-256: `d65bf8a481a1a09facd9bf3644af2da8fb048298e851edd071d0d61acb51e694`

The GUI checks copy contributor-created fixtures from the sibling repositories' `target/agent-integration/` folders and Reader's `target/interface-scale/bookmarks.pdf`. They change only their copies under `target/native-qa/`. Screenshots and inspection JSON are retained there. They do not request access to personal folders or change macOS security settings.

This is native macOS integration coverage, not full CI for all four upstream workspaces, Windows, Linux or wasm. The local app is ad-hoc signed without Apple notarization. Active-editor autosave/playback and Writer's one-document model remain as described in ARCHITECTURE.md.
