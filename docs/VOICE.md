# Voice in Polymath

Dictation and PDF read-aloud run on this Mac. By default, no speech service, account, network request, or agent-control server starts during a normal Polymath launch. Voicebox's global shortcut and focus-aware paste informed Polymath's small native companion; Polymath continues to use its bundled whisper.cpp model for speech input and installed macOS voices for narration. The global companion runs only when **Dictate anywhere** is enabled.

## Dictate into an editor

The toolbar microphone appears in Writer, Sheets, Slides and PDF Tools: click to start, then click to stop. It inserts into the selected editor field. It stays off Home and the read-only Reader view. Its language selection and last-transcript view are shared with global dictation.

Click a text field in Writer, Sheets, Slides, a Reader form/comment, or another editable Polymath field. Click the microphone in the shared toolbar, speak, then click it again. Polymath transcribes the recording with its bundled multilingual Whisper model and sends the text to the selected field or editor. Settings → Voice retains the last transcript so you can copy it if a custom editor field does not accept paste. Use the gear to choose a language before recording. Automatic language detection is the default; selecting the language is more reliable for short recordings.

The picker includes Hindi, Assamese, Bengali, Gujarati, Marathi, Tamil, Telugu, Kannada, Malayalam, Nepali, Punjabi, Sanskrit, Sindhi, and Urdu, along with English and other common languages. These codes are supported by the bundled whisper.cpp language table. Polymath uses installed macOS script fonts to display Indian-language transcripts in its interface; the fonts stay on your Mac and are not copied into the bundle. Accuracy varies by language, accent, microphone and audio quality. A short local Hindi speech sample produced Devanagari text with the bundled model; it was not word-perfect. Dictation creates text in the current editor; saving as DOCX, CSV, PPTX or PDF uses that editor's normal commands and format limits.

On first use, allow Polymath microphone access through the normal macOS prompt. The recorder writes a private temporary 16 kHz WAV while it is active. On Stop, the bundled `PolymathWhisper` process reads that WAV and writes a temporary transcript; Polymath then deletes both files. The model is packaged with the app, so no model download is needed after installation. Mac microphone permission is required to verify live recording; the app never changes Privacy & Security settings itself.

## Dictate into another app

Open **Settings → Voice → Dictate anywhere** and enable the shortcut. Select a text field in any Mac app, hold **Command + Option on either side of the keyboard**, speak, then release either key. A small floating status indicator shows recording and local transcription. Polymath sends the result back to the app and field that were focused when recording started. The last transcript also appears in Polymath's Voice settings. Keep Polymath running to use the shortcut. Turn the setting off to stop the companion process immediately.

macOS may ask for **Input Monitoring**, **Accessibility**, and **Microphone** access for Polymath and its bundled helpers. Voice settings shows the shortcut's current state. If it says **Setup needed**, use **Open Input Monitoring** or **Open Accessibility**, enable the Polymath dictation entry in System Settings, then return and click **Check access**. The button restarts the companion to recheck permission. If System Settings does not open, go to Privacy & Security manually. Accessibility is needed to return to the original app and paste. If that fails, Polymath leaves the transcript on the clipboard for manual paste. The companion temporarily stages the transcript on the clipboard for Command-V and restores the prior clipboard items after the paste event if nothing else changed it. No speech data leaves this Mac; the WAV and text files are removed after transcription. Hold both keys briefly; a quick Command-Option-key shortcut will not start dictation. A conflicting global shortcut may still intercept the keys.

## Read a PDF aloud

Open a PDF in Reader, turn to the page where you want to begin, and open the gear → **Voice**. Choose one of the voices installed in macOS, then select **Read aloud**. Polymath extracts the current working PDF text in reading order from that page through the end of the book, including unsaved text edits, and `/usr/bin/say` speaks it locally. **Stop** ends playback. The voice choice is remembered in Polymath preferences.

Image-only scans need a text layer first. Run Reader's OCR tool, then use Read aloud. For a book with a mix of text and image-only pages, narration skips pages without text. Polymath keeps a private temporary text file only while preparing and speaking; Stop or completion removes it.

## Cloned voices

Polymath 0.4.0 lists voices installed in macOS; it does not yet clone a voice or play Reader narration through a Voicebox profile. The sibling [Voicebox checkout](../../voicebox/docs/content/docs/overview/voice-cloning.mdx) can create a local voice profile from 10–30 seconds of clear reference audio and exposes local profile and speech-generation APIs. That makes an opt-in cloned-voice source for Reader feasible without a cloud service. Voicebox is not bundled with Polymath or running as a Polymath background service. Its documented multilingual cloning includes Hindi, but does not cover every Indian language in the dictation picker.

## Build and verification

The sibling `../speech-engine` checkout is [whisper.cpp](https://github.com/ggml-org/whisper.cpp) at `0c7be696371069e0baa2515ed7aa08d5a5473aa6`. Build its static CLI and download the multilingual `small-q5_1` model as shown in the root README. `packaging/bundle.sh` compiles `PolymathRecorder.swift` and `PolymathGlobalDictation.swift` with the macOS SDK, bundles the speech binary and model, copies their MIT licenses, and signs the app. The exact model checksum is in `ATTRIBUTION.md`.

The bundled CLI was checked on local synthetic English, Spanish, Hindi and Tamil audio; Hindi produced readable Devanagari with minor errors, and Tamil produced Tamil script. A synthetic Bengali sample produced Devanagari even with Bengali selected; a larger Whisper model did not correct that sample. Indian language choices indicate model language-code support, not guaranteed accuracy or script fidelity. Reader's in-memory text extraction has a targeted test. A native cross-app text target verifies the helper can paste into another application. Live microphone capture and individual third-party editors still require user-granted permissions and acceptance on this Mac.
