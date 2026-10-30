# Local media in Reader

Reader opens common audio and video files inside Polymath's own window. Use Home → Music for an audio-filtered picker, or Home → Open file, Finder Open With, or a file drop. The Music card opens the selected song in a Reader tab. It reads and probes the file locally on a worker thread. Press **Play** to begin decoding and audio output; **Pause** and the timeline work in the same view. Space toggles playback and the left/right arrows seek five seconds. Click the video stage to play or pause. Switching away from Reader pauses playback. Images open in Reader's viewer first; **Edit in Images** opens the non-destructive editor.

Opening a song or video builds a non-recursive queue from other files of the same media type in that folder. The queue reads music tags in the background, displays real title, artist and album when present, and lets you sort songs by those fields or name. Unknown tags fall back to the filename. Use Previous or Next to move through the queue; when a file ends, the next one starts automatically. Audio and video both offer 0.5×, 0.75×, 1×, 1.25×, 1.5× and 2× playback presets; speed changes audio pitch. The full-screen action sits beside the bottom playback controls, and Escape leaves full screen. No folder is watched continuously and nothing is uploaded.

The video view uses a dark viewing stage and a full-width transport bar. The audio view has original record-inspired art and shows real music tags where available. Transport, queue and sorting controls use the local egui-shadcn port, and egui_extras virtualizes long folder queues. It does not invent album art, artist names or track details.

Audio extensions: `.mp3`, `.m4a`, `.aac`, `.wav`, `.aiff`, `.aif`, `.caf`, `.flac`, `.ogg`, `.opus`. Video extensions: `.mp4`, `.m4v`, `.mov`, `.webm`, `.mkv`. Supported tracks depend on the actual container and codec; the existing [DeckCraft media engine](https://github.com/storytold/deckcraft/tree/main/crates/media) handles MP3, AAC, ALAC, PCM, FLAC, Ogg Vorbis and Opus audio, plus H.264, HEVC, VP9 and AV1 video. Unsupported tracks report an error. The player loads at most 512 MB per file and queues at most 5,000 files from one folder. It has no subtitles, saved playlists or hardware video decoding.

## Open-source choices

| Project | Fit for Polymath |
| --- | --- |
| [DeckCraft media](https://github.com/storytold/deckcraft/tree/main/crates/media) | Already a dependency, with Rust decoders, audio clock and video frame feed. Reused for the first in-window player without adding a runtime. |
| [egui-video](https://github.com/n00kii/egui-video) | Designed for egui, but its README requires FFmpeg 6 or 7 and SDL2. This Mac currently has FFmpeg 8, and bundling FFmpeg/SDL2 would increase the app and packaging work. |
| [lumina-video](https://github.com/lumina-video/lumina-video) | Promising hardware video decode and zero-copy rendering. Its current README targets egui 0.31; Polymath uses egui 0.36, so it needs an adapter or upstream compatibility work before integration. |

The current player prioritizes a small, local integration. For large files, subtitles or broad VLC-like codec coverage, a future streaming/hardware backend would be appropriate. Its licensing, bundle size and macOS deployment need to be evaluated before adding it.

For coding agents, `app.open {path}` routes media to Reader. `app.media.inspect`, `app.media.queue {offset,limit}`, `app.media.play`, `app.media.pause`, `app.media.seek {seconds}`, `app.media.previous`, `app.media.next`, `app.media.sort {by:"name"|"artist"|"album"}`, and `app.media.fullscreen {enabled}` are available through the authenticated local control bridge. `document.inspect` in Reader also reports the current media file and playback state.

Video editing is a separate workspace. Open a video in Reader, then choose **Edit video**, or enter Video from Home and add clips. The timeline trims and arranges sources and writes a new MP4 through the locally installed FFmpeg CLI only on export. See [Video editor](VIDEO_EDITOR.md).
