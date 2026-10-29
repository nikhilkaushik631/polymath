// One-time, local migration of pinned Raycast v1 Clipboard History entries.
// Run while Polymath is closed: swift packaging/import-raycast-pins.swift
// Uses Raycast's visible Copy action. Clipboard contents are never logged.
import AppKit
import Foundation

private struct Clip: Codable {
    let id: String
    var kind: String
    var title: String
    var text: String?
    var files: [String]?
    var imageName: String?
    var source: String
    var copiedAt: Date
    var pinned: Bool
}

private struct Captured {
    var clip: Clip
    var image: Data?
}

private enum ImportError: Error, CustomStringConvertible {
    case message(String)
    var description: String { if case .message(let value) = self { return value }; return "Import failed" }
}

private let board = NSPasteboard.general
private let original = (board.pasteboardItems ?? []).map { item -> [NSPasteboard.PasteboardType: Data] in
    var values: [NSPasteboard.PasteboardType: Data] = [:]
    for type in item.types {
        if let bytes = item.data(forType: type) { values[type] = bytes }
    }
    return values
}

private func restoreClipboard() {
    board.clearContents()
    let items = original.map { values -> NSPasteboardItem in
        let item = NSPasteboardItem()
        for (type, bytes) in values { item.setData(bytes, forType: type) }
        return item
    }
    if !items.isEmpty { _ = board.writeObjects(items) }
}

private func runAppleScript(_ source: String) throws -> String {
    guard let script = NSAppleScript(source: source) else {
        throw ImportError.message("Could not create the Raycast accessibility command.")
    }
    var details: NSDictionary?
    let result = script.executeAndReturnError(&details)
    if let details {
        let number = details[NSAppleScript.errorNumber] ?? "unknown"
        let detail = details[NSAppleScript.errorMessage] as? String ?? ""
        throw ImportError.message("Raycast accessibility action failed (\(number)): \(detail)")
    }
    return result.stringValue ?? ""
}

private let raycastURL = URL(string: "raycast://extensions/raycast/clipboard-history/clipboard-history")!

private func openRaycastHistory() throws {
    let check = """
    tell application "System Events" to tell process "Raycast"
        set frontmost to true
        return value of UI element 1 of UI element 1 of row 1 of table 1 of scroll area 1 of window 1
    end tell
    """
    guard NSWorkspace.shared.open(raycastURL) else {
        throw ImportError.message("Could not open Raycast Clipboard History.")
    }
    for _ in 0..<40 {
        let heading = try? runAppleScript(check)
        if heading == "Pinned" { return }
        Thread.sleep(forTimeInterval: 0.15)
    }
    throw ImportError.message("Raycast Clipboard History did not become ready.")
}

private func pinnedCount() throws -> Int {
    try openRaycastHistory()
    let value = try runAppleScript("""
    tell application "System Events" to tell process "Raycast"
        set frontmost to true
        repeat 4 times
            set value of scroll bar 1 of scroll area 1 of window 1 to 1
            delay 0.08
        end repeat
        set firstHeading to value of UI element 1 of UI element 1 of row 1 of table 1 of scroll area 1 of window 1
        if firstHeading is not "Pinned" then error "Pinned section not found" number 9001
        set total to count of rows of table 1 of scroll area 1 of window 1
        repeat with n from 2 to total
            try
                set label to value of UI element 1 of UI element 1 of row n of table 1 of scroll area 1 of window 1
                if label is "Today" or label is "Yesterday" or label is "Earlier" then return (n - 2) as text
            end try
        end repeat
        error "Recent section not found" number 9002
    end tell
    """)
    guard let count = Int(value), count > 0, count <= 100 else {
        throw ImportError.message("Raycast returned an invalid pinned count.")
    }
    // Leave the counting view closed; Raycast's deeplink toggles an open panel.
    _ = try runAppleScript("tell application \"System Events\" to tell process \"Raycast\" to key code 53")
    Thread.sleep(forTimeInterval: 0.1)
    return count
}

private func copyPinnedRow(_ index: Int) throws {
    try openRaycastHistory()
    let before = board.changeCount
    _ = try runAppleScript("""
    tell application "System Events" to tell process "Raycast"
        set frontmost to true
        set sectionName to value of UI element 1 of UI element 1 of row 1 of table 1 of scroll area 1 of window 1
        if sectionName is not "Pinned" then error "Pinned section changed" number 9003
        repeat \(index) times
            key code 125
            delay 0.025
        end repeat
        delay 0.06
        key code 36 using {command down}
    end tell
    """)
    for _ in 0..<20 {
        if board.changeCount != before { return }
        Thread.sleep(forTimeInterval: 0.05)
    }
    throw ImportError.message("Raycast did not copy pinned item \(index + 1).")
}

private func capturePinned() throws -> Captured {
    let id = UUID().uuidString
    let stamp = Date()
    if let urls = board.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL],
       !urls.isEmpty {
        let paths = urls.map(\.path)
        let name = urls.count == 1 ? urls[0].lastPathComponent : "\(urls.count) files"
        return Captured(clip: Clip(id: id, kind: "Files", title: name, text: nil,
                                   files: paths, imageName: nil, source: "Raycast",
                                   copiedAt: stamp, pinned: true), image: nil)
    }
    if let image = NSImage(pasteboard: board),
       let tiff = image.tiffRepresentation,
       let bitmap = NSBitmapImageRep(data: tiff),
       let png = bitmap.representation(using: .png, properties: [:]) {
        return Captured(clip: Clip(id: id, kind: "Images",
                                   title: "Image · \(bitmap.pixelsWide) × \(bitmap.pixelsHigh)",
                                   text: nil, files: nil, imageName: id + ".png",
                                   source: "Raycast", copiedAt: stamp, pinned: true), image: png)
    }
    if let content = board.string(forType: .string), !content.isEmpty {
        let first = content.split(whereSeparator: \.isNewline).first.map(String.init) ?? content
        let label = String(first.prefix(100)).trimmingCharacters(in: .whitespacesAndNewlines)
        return Captured(clip: Clip(id: id, kind: "Text", title: label.isEmpty ? "Text" : label,
                                   text: content, files: nil, imageName: nil, source: "Raycast",
                                   copiedAt: stamp, pinned: true), image: nil)
    }
    throw ImportError.message("Pinned entry has a format Polymath cannot store.")
}

private func matches(_ existing: Clip, _ incoming: Captured, images: URL) -> Bool {
    guard existing.kind == incoming.clip.kind else { return false }
    switch existing.kind {
    case "Text": return existing.text == incoming.clip.text
    case "Files": return existing.files == incoming.clip.files
    case "Images":
        guard let name = existing.imageName, let image = incoming.image else { return false }
        return (try? Data(contentsOf: images.appendingPathComponent(name))) == image
    default: return false
    }
}

private func samePayload(_ left: Captured, _ right: Captured) -> Bool {
    guard left.clip.kind == right.clip.kind else { return false }
    switch left.clip.kind {
    case "Text": return left.clip.text == right.clip.text
    case "Files": return left.clip.files == right.clip.files
    case "Images": return left.image == right.image
    default: return false
    }
}

private func migrate() throws {
    let processes = NSRunningApplication.runningApplications(withBundleIdentifier: "local.polymath.app")
    guard processes.isEmpty else {
        throw ImportError.message("Quit Polymath before importing so its clipboard watcher cannot capture migration copies.")
    }
    let folder = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent("Library/Application Support/Polymath/Clipboard History", isDirectory: true)
    let images = folder.appendingPathComponent("Images", isDirectory: true)
    let index = folder.appendingPathComponent("history.json")
    let decoder = JSONDecoder()
    decoder.dateDecodingStrategy = .iso8601
    var history: [Clip] = []
    if FileManager.default.fileExists(atPath: index.path) {
        history = try decoder.decode([Clip].self, from: Data(contentsOf: index))
    }
    let count = try pinnedCount()
    print("Raycast pinned entries: \(count)")
    var captured: [Captured] = []
    for offset in 0..<count {
        try copyPinnedRow(offset)
        do { captured.append(try capturePinned()) }
        catch { throw ImportError.message("Could not read pinned item \(offset + 1): \(error)") }
    }
    guard captured.count == count else { throw ImportError.message("Raycast pin count changed during import.") }
    var distinct: [Captured] = []
    for item in captured where !distinct.contains(where: { samePayload($0, item) }) {
        distinct.append(item)
    }
    guard distinct.count > 1 || count == 1 else {
        throw ImportError.message("Raycast copied the same selected item repeatedly; Polymath history was left untouched.")
    }

    if CommandLine.arguments.contains("--repair-duplicate-import") {
        let previous = history.filter { $0.pinned && $0.source == "Raycast" }
        guard previous.count == count, let first = previous.first,
              first.kind == "Text",
              previous.allSatisfy({ $0.kind == "Text" && $0.text == first.text }) else {
            throw ImportError.message("The previous import is not the expected repeated-text set; no pins were removed.")
        }
        let previousIDs = Set(previous.map(\.id))
        history.removeAll { previousIDs.contains($0.id) }
    }

    var consumed = Set<String>()
    var inserted: [Captured] = []
    var reused = 0
    for item in captured {
        if let position = history.firstIndex(where: { !consumed.contains($0.id) && matches($0, item, images: images) }) {
            history[position].pinned = true
            consumed.insert(history[position].id)
            reused += 1
        } else {
            inserted.append(item)
        }
    }
    guard history.filter(\.pinned).count + inserted.count <= 100 else {
        throw ImportError.message("Polymath's 100-pin limit would be exceeded.")
    }
    try FileManager.default.createDirectory(at: images, withIntermediateDirectories: true,
                                            attributes: [.posixPermissions: 0o700])
    for item in inserted {
        if let png = item.image, let name = item.clip.imageName {
            let path = images.appendingPathComponent(name)
            try png.write(to: path, options: .atomic)
            try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: path.path)
        }
    }
    let pins = inserted.map(\.clip) + history.filter(\.pinned)
    let recent = Array(history.filter { !$0.pinned }.prefix(300))
    let result = Array(pins.prefix(100)) + recent
    let encoder = JSONEncoder()
    encoder.dateEncodingStrategy = .iso8601
    let data = try encoder.encode(result)
    try data.write(to: index, options: .atomic)
    try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: index.path)
    print("Imported: \(inserted.count), distinct: \(distinct.count), matched existing: \(reused), Polymath pins: \(result.filter(\.pinned).count)")
    // Avoid logging titles, paths, text, previews, or image bytes.
}

do {
    defer { restoreClipboard() }
    try migrate()
} catch {
    fputs("Raycast pin import failed: \(error)\n", stderr)
    exit(1)
}
