# Video Studio

Video Studio is Polymath's local MP4 assembly workspace. It uses the macOS file picker to add source clips and audio, keeps those source files untouched, and runs the installed FFmpeg CLI only when you choose **Export video**.

## Edit a video

1. Open **Video** from Home, or choose **Edit video** while viewing a video in Reader.
2. Choose **Add videos**. You can select several files at once. Polymath reads their durations and builds frame thumbnails in the background.
3. Drag a clip on the bottom timeline to reorder it. Drag either coral edge to trim its beginning or end. **Properties** offers a named clip selector, precise in/out seconds and Move left/right controls, so the same edits are available from the keyboard. The ruler and the frame scrubber show a still frame at the selected time.
4. Choose **Add soundtrack** to place local audio on a separate timeline track. In **Properties**, set its timeline start and level. **Strip original clip sound** removes the source audio, with or without a new soundtrack.
5. Choose **Export video** to select an output `.mp4` path the first time. Export options are also in **Properties**. After choosing the destination, choose **Export video** again to begin encoding. The progress row shows encoding, joining, audio and publishing; **Cancel** stops the job.

The timeline has **Fit**, **+**, and **−** zoom controls. Fit shows the whole assembly. A horizontal scroll bar appears when clips extend beyond the visible width. Selected clips have a coral outline; the coral vertical line is the current frame position. Soundtrack placement and the separate audio lane follow the same time ruler.

On smaller windows, the workspace scrolls vertically and the heading actions wrap to another row. The Properties pane starts closed and can be opened when you need precise clip, audio or export settings.

## What the presets mean

**Original** preserves source dimensions when practical. **High** favors clarity, **Balanced** balances quality and file size, and **Small** favors a smaller MP4. Video Studio creates a new file at the output path you selected. It never saves over a source video or audio file.

## Local behavior and current limits

- Import and frame preview read files locally. Export runs FFmpeg locally; there is no account, upload, or cloud service.
- FFmpeg and ffprobe must be installed locally. Polymath checks standard Homebrew locations and the process PATH when you inspect or export media; neither executable is added to the app bundle.
- The frame preview supports scrubbing to a still image. It does not yet play a live composite of all trims and transitions. Use Reader to play an exported file.
- Clips are arranged sequentially, without transitions, captions, overlays or multi-layer video. One optional soundtrack can be mixed in or used after stripping original clip sound.
- Keep source files in place until export completes. Polymath reads them from their original paths.
- Export targets MP4. The renderer normalizes clips as needed before joining them; a video with mixed input dimensions may be resized according to the selected quality preset.

The timeline interaction follows the [official Clipchamp timeline guide](https://support.microsoft.com/en-us/clipchamp/how-to-work-with-the-timeline-in-clipchamp) and [official editing guide](https://support.microsoft.com/en-us/topic/how-to-edit-a-video-in-clipchamp-fedb7b6c-4d31-496b-837f-474bc9aa2367): select a clip, drag edge handles to trim, drag clips to reorder, use a separate audio track, and explicitly export. Polymath uses its own native egui layout and visual identity.

## Agent integration

`VideoEditor` is a native module. Agents should operate through Polymath's authenticated loopback control bridge when the owner enables it; the ordinary app starts no server. The editor exposes ordered clips and export status through `inspect()`, and narrow methods for adding clips, setting trims, moving clips, adding audio, changing quality, choosing the output path, and starting export. Paths must refer to local files the owner has chosen or explicitly supplied. Wait until probes report each clip ready before setting trims or exporting. An export is always a separate explicit action.
