# Archives

Polymath opens ZIP, RAR and 7z files as read-only Reader tabs. Use the **Archives** card on Home to choose one, or open it from Finder. The archive view lists files and folders and offers a name filter. Select files or folders to extract a subset; selecting a folder includes its contents. **Extract all** takes every supported file and folder.

Choose a destination folder when prompted. Polymath creates a new “archive name extracted” folder there, adding a number if that name already exists. It never overwrites an existing file or changes the original archive. Extraction runs in the background with progress and Cancel. If an archive is damaged, unsupported or password protected, the view reports the error. A cancelled or failed extraction may leave already completed files in the new folder.

The app uses macOS’s bundled `/usr/bin/bsdtar` (libarchive) only when listing or extracting an archive. It reads regular files and directories. Symbolic links, hard links, absolute paths, parent traversal, wildcard names and duplicate paths are shown but cannot be extracted. Data is streamed into the new folder without letting the archive choose a filesystem destination. Archive contents stay on this Mac.

The native archive viewer is intentionally an extractor. Creating, updating, encrypting, or password-unlocking archives is not available yet. Format support can vary with the macOS libarchive build; the bundled system tool on the development Mac was checked against ZIP, RAR and 7z samples.
