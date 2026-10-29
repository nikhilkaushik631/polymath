# Clipboard History

Polymath keeps recent copies in a small native panel that can appear over another Mac app. Press **Command+D** to open it, or use the clipboard button in Polymath's top bar. Change the shortcut in **Settings → Clipboard** by pressing **Change shortcut** and then the new key combination. The shortcut needs Command, Option or Control plus one letter or number.

The panel lists **Pinned** clips above **Recent** clips. Search by content or source app, filter by text, image or file, select a row for preview, then press Return or **Paste** to insert it into the app that had focus. **Copy** puts it on the clipboard without pasting. **Pin** (or Command+.), **Unpin**, **Delete** and **Clear recents** manage the list; Clear recents keeps pinned clips. Esc closes the panel. Automatic paste needs macOS Accessibility permission for the bundled helper; without it, the chosen clip is copied and can be pasted with Command+V.

Capture runs while Polymath is open and **Capture copies** is enabled in Settings. Text up to 64 KiB, images up to 8 MiB as PNG, and groups of up to 40 local file links are retained. History is bounded to 100 pinned and 300 recent entries. Transient or concealed clipboard items and copies from common password managers are skipped. Clips are stored only in `~/Library/Application Support/Polymath/Clipboard History/`, with owner-only file permissions. Polymath does not separately encrypt this JSON/PNG history. No clip is sent to a server. Turning capture off stops the helper; it does not delete saved pins. **Clear recents** deletes saved unpinned entries and their image files.

## Moving pins from Raycast

Polymath does not silently read Raycast's private clipboard database. Raycast stores its history in an encrypted database, and the installed Raycast v1 export does not contain Clipboard History. Its documented Clipboard History action can copy an individual item, but that action opens and changes the visible desktop. Polymath therefore cannot claim to have imported all existing Raycast pins automatically. Keep Raycast installed until you have copied and pinned the entries you need in Polymath. New Polymath pins persist across app updates.

The [Raycast Clipboard History manual](https://manual.raycast.com/clipboard-history) documents its copy and pin actions. The [Raycast Import & Export manual](https://manual.raycast.com/import-export) explains that v1-to-v2 migration carries Clipboard History separately from the v1 `.rayconfig` export.
