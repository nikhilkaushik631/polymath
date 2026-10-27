# Upstream feature check — 2026-10-27

The five Storytold remotes were fetched on October 27. A fetch updated the remote pointers only; the Polymath source trees keep their local branches and uncommitted integration work. The gap counts below include documentation, tests, merges and refactors, so they are not feature counts.

| Source | Local `HEAD` | Fetched `origin/main` | Commits after local `HEAD` | Recent upstream work | Incorporated in this pass |
| --- | --- | --- | ---: | --- | --- |
| Reader / PDFCraft | `68e91d48` | `1dd2737b` | 2,738 | UI file layout refactor and test/doc path updates | None from this fetch |
| Writer / WordCraft | `1d4f741f` | `436d444e` | 2,134 | Dropdown menus recover after a search with no matches; UI layout refactor | None from this fetch |
| Sheets / GridCraft | `9f896c12` | `63038650` | 1,781 | Same dropdown recovery; UI layout refactor | None from this fetch |
| Slides / DeckCraft | `a98df49b` | `2c9edcc3` | 1,434 | Shared ComboBox controls and UI layout refactor | Paragraph default text style import/export from [`f9d5a431`](https://github.com/storytold/deckcraft/commit/f9d5a431740d74c1da59487cd7651c4037ca1e0d) |
| Images / LightCraft | `a4b53b28` | `3a3e49af` | 1,700 | Same dropdown recovery; queued file-drop handling; UI layout refactor | None from this fetch |

The focused Slides port preserves `a:pPr/a:defRPr` font size, weight, color and family in PPTX paragraphs and round-trips them when saved. It fixed the user's 17-slide forensic deck. The 16 PPTX round-trip tests passed, and all 17 saved/reopened slides matched their pre-save renders. The other fetched commits are not merged. The existing DeckCraft media crate is reused for Reader audio/video playback; that is an integration of code already present in the sibling Slides checkout, not a claim that the latest DeckCraft branch was merged. Earlier selected ports remain listed below.

## Earlier upstream review — 2026-10-15

The local source repositories were compared with the current `main` branches of the Storytold repositories supplied for Polymath. Fetching updated only Git remote references; it did not merge, rebase, or change any working tree. Counts below are commits after each local `HEAD`, including fixes, documentation, and merge commits—not counts of new features. All local `HEAD`s are ancestors of their current upstream `main`.

| Workspace | Local `HEAD` | Upstream `main` | Commits ahead | Examples of additions upstream |
|---|---|---|---:|---|
| Reader ([PDFCraft](https://github.com/storytold/pdfcraft)) | `68e91d4` | `afa8e184` | 616 | [Edit existing text and grow text boxes](https://github.com/storytold/pdfcraft/commit/d1bc2001156c8036d8bc97aaac1a41091975fd36); selectable list boxes; more picture-menu actions including redact and OCR. |
| Writer ([WordCraft](https://github.com/storytold/wordcraft)) | `1d4f741` | `fd0bb2f` | 540 | [Table formulas with cell references and F9 recalculation](https://github.com/storytold/wordcraft/commit/6a2bad8afaa53a3618531a9e80722a7868e5d935); ruler tab-stop editing and table-style editing. |
| Sheets ([GridCraft](https://github.com/storytold/gridcraft)) | `9f896c1` | `e6b6b34` | 767 | [Cell-value AutoComplete](https://github.com/storytold/gridcraft/commit/7a8235a); row autofit and recent/modified-file work; release 0.7.0. |
| Slides ([DeckCraft](https://github.com/storytold/deckcraft)) | `a98df49` | `9859da3` | 823 | [Table merges grow to include intersecting merged cells](https://github.com/storytold/deckcraft/commit/0222a90e5d3cdab164ad8a7aecc739a1e09baeac); gray list markers, improved cropped-image rendering, shared PDF writer. |
| Images ([LightCraft](https://github.com/storytold/lightcraft)) | `a4b53b28` | `0da998f2` | 560 | [Pressure-sensitive straight brush strokes](https://github.com/storytold/lightcraft/commit/00dced3dae9878634e5d128c46965e35f543663e); [point-centered Before/After zoom](https://github.com/storytold/lightcraft/commit/728471fc), more camera-profile coverage. |
| Motion ([Spark](https://github.com/storytold/spark)) | `b7b3d5c` | `b7b3d5c` | 0 | No upstream commits beyond the local checkout at this check. Motion remains separate from Polymath. |

The local repositories contain substantial uncommitted Polymath changes. A read-only three-way apply audit against the newer branches found conflicts in all five repositories, including release and UI files. The temporary audit worktrees were removed. A wholesale branch update would risk replacing Polymath's local-only behavior and needs dedicated reconciliation and compatibility testing. The selected ports below are packaged in the 0.4.5 DMG and installed in `/Applications/Polymath.app`.

## Integrated and verified

- The Slides table-merge expansion from [DeckCraft `0222a90`](https://github.com/storytold/deckcraft/commit/0222a90e5d3cdab164ad8a7aecc739a1e09baeac) was ported into the local table model without changing the branch or unrelated work. A regression test verifies that a new merge crossing an existing merge includes its whole span and text. The targeted release test passed.
- Reader now quotes carriage returns in exported form CSV, so field names and values survive reimport ([PDFCraft `53b9bc10`](https://github.com/storytold/pdfcraft/commit/53b9bc10)); the local round-trip regression passed.
- Writer now binds macOS ⌘←/⌘→ to line start/end while Option+arrows continue to move by word ([WordCraft `fd0bb2f`](https://github.com/storytold/wordcraft/commit/fd0bb2f)); the shortcut registry test passed.
- Sheets now lets users disable column-value AutoComplete in the View ribbon or through `options.autoComplete`, and Backspace/Escape dismiss the proposal before the edit ([GridCraft `7a8235a`](https://github.com/storytold/gridcraft/commit/7a8235a)); three interaction tests passed.
- Slides decodes cropped pictures for the whole image's displayed size, preserving detail ([DeckCraft `57c03d3`](https://github.com/storytold/deckcraft/commit/57c03d3)); the fine-stripe render regression passed.
- Images maps clicks in either Before/After pane to the same zoom center ([LightCraft `728471fc`](https://github.com/storytold/lightcraft/commit/728471fc)); its headless regression passed.

This is a selected integration, **not** a full merge of the upstream branches. Reader text editing, Writer table formulas, LightCraft pressure-sensitive strokes and many other features remain upstream-only pending dependency and UI reconciliation. Motion is intentionally left out. Archive Manager/ZIP and Drawboard/Excalidraw are separate requested features and have not been implemented in the installed app.
