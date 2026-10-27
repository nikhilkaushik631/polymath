# Polymath asset attribution

The original assets below are contributor-original Polymath artwork, licensed under MIT OR Apache-2.0 (LICENSE-MIT and LICENSE-APACHE). SVG masters are the source; packaging/icons.sh produces PNG and ICNS. No upstream publisher artwork is reused.

| File | Author / source | License | SHA-256 | Use |
| --- | --- | --- | --- | --- |
| assets/icons/home.svg | Original project contributors / local SVG | MIT OR Apache-2.0 | 5b058e1f43effffebc882d71ed7f5e336f3edfbacf0721d18cf6476b9efbbba0 | Application / workspace icon |
| assets/icons/images.svg | Polymath contributors / original local SVG | MIT OR Apache-2.0 | 3c0d1c667eec8b60297bae8b5645d9fd1532492bcb479d42313f434802397c27 | Images workspace symbol |
| assets/icons/polymath-1024.png | Original project contributors / local SVG | MIT OR Apache-2.0 | 1b9a3ee6df90bbc15f71615285aa083668f464c08c023b933b9402648d405464 | Application / workspace icon |
| assets/icons/polymath.icns | Original project contributors / local SVG | MIT OR Apache-2.0 | 708ff304a3d09d3510a766ec597482dbe440ab9c48bc669985169a5f74f11c35 | Application / workspace icon |
| assets/icons/polymath.svg | Original project contributors / local SVG | MIT OR Apache-2.0 | 6f27e55ced86bdfb2ab30d9fd0e81c6a5fd4afac1213369938a1798fd6b22b36 | Application / workspace icon |
| assets/icons/reader.svg | Original project contributors / local SVG | MIT OR Apache-2.0 | 0489aca32556dc7a7468c044fbfe88089b57caffbb7a2c8f35866d94400df771 | Application / workspace icon |
| assets/icons/sheets.svg | Original project contributors / local SVG | MIT OR Apache-2.0 | bfd1fec00b3d0e0a47e6dd7e2682a36c4a5fa1e43d66a359ae72157637e6843f | Application / workspace icon |
| assets/icons/slides.svg | Original project contributors / local SVG | MIT OR Apache-2.0 | 94914025362737dc151563b5a95cb3d68e3952b4c38e8260ad31f109e09d55f1 | Application / workspace icon |
| assets/icons/tools.svg | Polymath contributors / original local SVG | MIT OR Apache-2.0 | 601fcefa55176de4fd04aae8d5803ed222b2c6a0e1198ad117751199989dcb54 | PDF Tools workspace symbol |
| assets/icons/video.svg | Polymath contributors / original local SVG | MIT OR Apache-2.0 | e40203988c325425f21becd9a760c96eadfa2aad5aa8d37601f55cb5d0ce8ad2 | Video workspace symbol |
| assets/icons/writer.svg | Original project contributors / local SVG | MIT OR Apache-2.0 | 267c370e36f570548c521eb8cc770ebf5a0eda2e2e0259d4c35b589e945a5da1 | Application / workspace icon |

The document previews on Home are original geometric drawings in `src/home_art.rs`, also licensed under MIT OR Apache-2.0. They do not embed screenshots or artwork from another application.

The shared UI uses a local egui 0.36 compatibility port of [egui-shadcn 0.5.0](https://github.com/FerrisMind/shadcn-rs) by FerrisMind, MIT licensed. Its source is in `vendor/egui-shadcn`, its notice is `vendor/egui-shadcn/LICENSE-MIT`, and the bundle copies that notice to `Contents/Resources/licenses/egui-shadcn`. Polymath's changes are limited to egui API compatibility and local palette integration.

The macOS close path uses [craft-quit](https://github.com/storytold/craft-libs/tree/a01382de4f5415b5a7a9133093be47c9488c1b6b/crates/quit) by Storytold contributors, MIT OR Apache-2.0 licensed. Its license texts are in `assets/licenses/craft-quit` and copied into the application bundle.

Embedded editor fonts, icons and technical data retain the attribution ledgers and notices in ../reader, ../writer, ../sheets, ../slides and ../lightcraft. The bundle copies these ledgers and their license directories into Contents/Resources/licenses. Lightcraft's ArtCraft trademark artwork is not embedded in Polymath. The UI libraries embed their permitted fonts and icons locally.

## Local speech

Dictation uses [whisper.cpp](https://github.com/ggml-org/whisper.cpp) at commit `0c7be696371069e0baa2515ed7aa08d5a5473aa6`, MIT licensed. The statically linked `whisper-cli` is built without Metal and copied into the app as `PolymathWhisper`; SHA-256 of this local build is `d6473db6989998dd39d030718e24312f4ffff562a11e2b0d268b6a96b076d9d3`. Its license is copied into `Contents/Resources/licenses/whisper/LICENSE`.

The bundled multilingual `ggml-small-q5_1.bin` is a converted [OpenAI Whisper](https://github.com/openai/whisper) model from [ggerganov/whisper.cpp on Hugging Face](https://huggingface.co/ggerganov/whisper.cpp), MIT licensed. SHA-1 `6fe57ddcfdd1c6b07cdcc73aaf620810ce5fc771` matches the publisher's model card; SHA-256 is `ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb`. OpenAI's license is kept in `assets/licenses/Whisper-MIT.txt` and copied into the app. No model download or speech network request occurs at runtime.

`packaging/PolymathRecorder.swift` is a Polymath-authored microphone adapter, licensed MIT OR Apache-2.0. It requests ordinary macOS microphone permission and records a temporary 16 kHz mono WAV while Dictate is active. Reader's extracted PDF text is passed to the installed `/usr/bin/say` voice chosen by the user; the narration text file is deleted after playback or Stop.

## Inter interface font and Lucide controls

Home embeds Inter Regular, Medium and Semibold from `../reader/assets/fonts`, licensed under SIL OFL-1.1 by the Inter Project Authors (including Rasmus Andersson), source https://github.com/rsms/inter. The license is `../reader/assets/fonts/OFL-Inter.txt` and is copied into the bundle. The theme controls use the vendored Lucide `sun` and `moon` SVGs; the library uses `clock`, `folder`, `folder-plus` and `search`. These retain the Lucide/Feather ISC and MIT notices in `../reader/assets/icons/LICENSE-lucide.txt`. Source, version and individual hashes are recorded in `../reader/ATTRIBUTION.toml`, included in every bundle. All assets load locally.

## Previous Manrope interface font

Copyright 2018 The Manrope Project Authors. Source: Google Fonts, commit `66928797f271fed5ee428544e7229685e2d90127`, [upstream font directory](https://github.com/google/fonts/tree/66928797f271fed5ee428544e7229685e2d90127/ofl/manrope). Licensed under SIL OFL-1.1; see `assets/fonts/OFL-Manrope.txt`. The static weights are instances of the upstream variable font, produced by FontTools 4.66.1 using `packaging/home-fonts.py`. Fonts are embedded locally and no download happens at runtime.

| File | SHA-256 | Use |
| --- | --- | --- |
| assets/fonts/Manrope-Variable.ttf | 3ae11c49db0455a3cc33e37d380f20fdb8c7f8b41dc07625c177e3d87a9d6ae6 | Source font for reproducible instances |
| assets/fonts/Manrope-Regular.ttf | e9e1400397ddf980ac722fa3963cbc649be0607c20800e183b20812eb9d27692 | Previous Home font; retained source, no longer embedded by Home |
| assets/fonts/Manrope-Medium.ttf | f45d1fec9ad5bb21d11e66d03f1e0db2a74f2b5ec68261fa7fbe0fcad587c86b | Previous Home font; retained source, no longer embedded by Home |
| assets/fonts/Manrope-SemiBold.ttf | bd145da72692c9d0416a1330b6f47e5056b272ee66f4261b3f73b7e9a0326dd2 | Previous Home font; retained source, no longer embedded by Home |
| assets/fonts/OFL-Manrope.txt | d6a309fcfb963d329a93baf9df66fa5f3c23a4be2b5f67edf24d6cafecde12ac | Font license |
