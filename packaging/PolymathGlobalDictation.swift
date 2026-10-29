import AppKit
import ApplicationServices
import CoreGraphics
import Foundation
import OSLog

// A small companion process keeps global keyboard and Accessibility permissions
// together. It has no server, account, model runtime, or network connection.
private final class DictationBridge {
    private let logger = Logger(subsystem: "local.polymath.app", category: "global-dictation")
    private var monitor: Any?
    private var held = false
    private var modifiersHeld = false
    private var pending = false
    private var pendingGeneration = 0
    private var suppressedUntilRelease = false
    private var busy = false
    private var recorder: Process?
    private var recorderInput: Pipe?
    private var recorderError: Pipe?
    private var recordingURL: URL?
    private var targetPID: pid_t?
    private var language: String
    private var panel: NSPanel?
    private var panelLabel: NSTextField?
    private var hideGeneration = 0
    private var lastStatus: String?
    private let parentPID: pid_t
    private let hotkeyTest: Bool

    init(language: String, parentPID: pid_t, hotkeyTest: Bool = false) {
        self.language = Self.validLanguage(language) ? language : "auto"
        self.parentPID = parentPID
        self.hotkeyTest = hotkeyTest
    }

    private static func validLanguage(_ value: String) -> Bool {
        value == "auto" || (2...5).contains(value.count) && value.allSatisfy { $0.isASCII && ($0.isLetter || $0 == "-") }
    }

    private func send(_ kind: String, _ message: String, extra: [String: Any] = [:]) {
        if kind == "status" {
            if lastStatus == message { return }
            lastStatus = message
            logger.info("\(message, privacy: .public)")
        }
        var payload: [String: Any] = ["kind": kind, "text": message]
        for (key, value) in extra { payload[key] = value }
        guard let bytes = try? JSONSerialization.data(withJSONObject: payload),
              var line = String(data: bytes, encoding: .utf8) else { return }
        line.append("\n")
        FileHandle.standardOutput.write(Data(line.utf8))
    }

    func run() {
        NSApplication.shared.setActivationPolicy(.accessory)
        makePanel()
        installInputCommands()
        let options = ["AXTrustedCheckOptionPrompt": true] as CFDictionary
        // Accessibility already grants event listening as well as the ability
        // to insert the transcript. Asking for Input Monitoring unconditionally
        // can leave this helper waiting on a second, unnecessary TCC grant.
        if !AXIsProcessTrustedWithOptions(options) {
            _ = CGRequestListenEventAccess()
        }
        checkPermissions()
        Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { [weak self] _ in
            guard let self else { return }
            if self.parentPID > 0 && kill(self.parentPID, 0) != 0 {
                self.shutdown()
                return
            }
            self.checkPermissions()
        }
        NSApplication.shared.run()
    }

    private func checkPermissions() {
        let accessibility = AXIsProcessTrusted()
        let inputMonitoring = CGPreflightListenEventAccess()
        if !accessibility && !inputMonitoring {
            send("status", "Allow Accessibility for Polymath in macOS Privacy & Security, then check access here.")
            return
        }
        if monitor == nil {
            monitor = NSEvent.addGlobalMonitorForEvents(matching: [.flagsChanged, .keyDown]) { [weak self] event in
                if self?.hotkeyTest == true {
                    self?.send("status", "TEST_EVENT_\(event.keyCode)_\(event.modifierFlags.rawValue)")
                }
                if event.type == .flagsChanged {
                    self?.modifierChanged(event)
                } else if event.type == .keyDown {
                    if self?.pending == true {
                        self?.suppressedUntilRelease = true
                        self?.cancelPending()
                    }
                }
            }
            if monitor == nil {
                send("status", "Could not watch the global shortcut. Re-enable Input Monitoring and restart Polymath.")
                return
            }
            send("status", accessibility
                ? "Ready · hold ⌘ + ⌥ in any app"
                : "Shortcut ready · allow Accessibility for automatic paste")
        }
    }

    private func modifierChanged(_ event: NSEvent) {
        // macOS combines the left and right variants in modifierFlags. Accept
        // either side, including a mixed pair. The short hold distinguishes
        // dictation from ordinary Command-Option keyboard shortcuts.
        modifiersHeld = event.modifierFlags.contains([.command, .option])
        if modifiersHeld && !held && !pending && !suppressedUntilRelease {
            pending = true
            pendingGeneration += 1
            let generation = pendingGeneration
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.18) { [weak self] in
                guard let self, self.pending, self.modifiersHeld,
                      self.pendingGeneration == generation else { return }
                self.pending = false
                self.held = true
                self.beginRecording()
            }
        } else if !modifiersHeld && held {
            held = false
            finishRecording()
        } else if !modifiersHeld {
            cancelPending()
            suppressedUntilRelease = false
        }
    }

    private func cancelPending() {
        pending = false
        pendingGeneration += 1
    }

    private func focusedPID() -> pid_t? {
        let system = AXUIElementCreateSystemWide()
        var focused: CFTypeRef?
        if AXUIElementCopyAttributeValue(system, kAXFocusedUIElementAttribute as CFString, &focused) == .success,
           let focused,
           CFGetTypeID(focused) == AXUIElementGetTypeID() {
            let element = focused as! AXUIElement
            var pid: pid_t = 0
            if AXUIElementGetPid(element, &pid) == .success && pid > 0 {
                return pid
            }
        }
        return NSWorkspace.shared.frontmostApplication?.processIdentifier
    }

    private func beginRecording() {
        if hotkeyTest {
            send("status", "TEST_START")
            return
        }
        guard !busy else { return }
        let macOS = URL(fileURLWithPath: CommandLine.arguments[0]).deletingLastPathComponent()
        let helper = macOS.appendingPathComponent("PolymathRecorder")
        guard FileManager.default.isExecutableFile(atPath: helper.path) else {
            send("status", "Polymath's microphone recorder is missing.")
            showPill("Recorder unavailable", color: .systemRed, hideAfter: 3)
            return
        }
        let audio = FileManager.default.temporaryDirectory.appendingPathComponent(
            "polymath-dictation-\(getpid())-\(UUID().uuidString).wav")
        // Remember the editor before macOS can show a microphone permission
        // prompt, which may briefly become the frontmost application.
        let destination = focusedPID()
        let process = Process()
        let input = Pipe()
        let errorOutput = Pipe()
        process.executableURL = helper
        process.arguments = [audio.path]
        process.standardInput = input
        process.standardOutput = FileHandle.nullDevice
        process.standardError = errorOutput
        do {
            try process.run()
        } catch {
            send("status", "Could not start microphone: \(error.localizedDescription)")
            showPill("Microphone unavailable", color: .systemRed, hideAfter: 3)
            return
        }
        targetPID = destination
        recorder = process
        recorderInput = input
        recorderError = errorOutput
        recordingURL = audio
        busy = true
        send("status", "Listening · release ⌘ or ⌥ to insert")
        showPill("●  Listening · release to insert", color: .systemRed)
    }

    private func finishRecording() {
        if hotkeyTest {
            send("status", "TEST_STOP")
            return
        }
        guard let process = recorder, let input = recorderInput, let audio = recordingURL else { return }
        let errorOutput = recorderError
        recorder = nil
        recorderInput = nil
        recorderError = nil
        recordingURL = nil
        let destination = targetPID
        targetPID = nil
        input.fileHandleForWriting.write(Data("\n".utf8))
        try? input.fileHandleForWriting.close()
        send("status", "Transcribing locally…")
        showPill("Transcribing locally…", color: .systemBlue)
        let selectedLanguage = language
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            process.waitUntilExit()
            guard let self else { return }
            if process.terminationStatus != 0 || !FileManager.default.fileExists(atPath: audio.path) {
                let detail = errorOutput.flatMap {
                    String(data: $0.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)
                }?.trimmingCharacters(in: .whitespacesAndNewlines)
                try? FileManager.default.removeItem(at: audio)
                let message = detail.flatMap { $0.isEmpty ? nil : $0 } ?? "No recording was captured."
                DispatchQueue.main.async { self.complete(message, success: false) }
                return
            }
            let result = self.transcribe(audio, language: selectedLanguage)
            try? FileManager.default.removeItem(at: audio)
            DispatchQueue.main.async {
                switch result {
                case .success(let text):
                    let pasted = self.paste(text, into: destination)
                    self.send("transcript", text, extra: ["pasted": pasted])
                    self.complete(pasted ? "Inserted in the focused app" : "Transcript copied to clipboard", success: pasted)
                case .failure(let error):
                    self.complete(error.localizedDescription, success: false)
                }
            }
        }
    }

    private func transcribe(_ audio: URL, language: String) -> Result<String, Error> {
        let macOS = URL(fileURLWithPath: CommandLine.arguments[0]).deletingLastPathComponent()
        let whisper = macOS.appendingPathComponent("PolymathWhisper")
        let model = macOS.deletingLastPathComponent()
            .appendingPathComponent("Resources/models/ggml-small-q5_1.bin")
        guard FileManager.default.isExecutableFile(atPath: whisper.path),
              FileManager.default.fileExists(atPath: model.path) else {
            return .failure(NSError(domain: "Polymath", code: 1, userInfo: [NSLocalizedDescriptionKey: "The local Whisper model is missing."]))
        }
        let prefix = audio.deletingPathExtension().appendingPathExtension("transcript")
        let transcript = URL(fileURLWithPath: prefix.path + ".txt")
        defer { try? FileManager.default.removeItem(at: transcript) }
        let process = Process()
        process.executableURL = whisper
        process.arguments = ["-m", model.path, "-f", audio.path, "-l", language,
                             "-nt", "-np", "-otxt", "-of", prefix.path]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        do {
            try process.run()
            process.waitUntilExit()
            guard process.terminationStatus == 0 else {
                throw NSError(domain: "Polymath", code: 2, userInfo: [NSLocalizedDescriptionKey: "Local transcription failed."])
            }
            let text = try String(contentsOf: transcript, encoding: .utf8)
                .trimmingCharacters(in: .whitespacesAndNewlines)
            guard !text.isEmpty else {
                throw NSError(domain: "Polymath", code: 3, userInfo: [NSLocalizedDescriptionKey: "No speech was recognized."])
            }
            return .success(text)
        } catch {
            return .failure(error)
        }
    }

    private struct ClipboardItem {
        let data: [(NSPasteboard.PasteboardType, Data)]
    }

    private func copyTranscript(_ text: String) {
        let board = NSPasteboard.general
        board.clearContents()
        _ = board.setString(text, forType: .string)
    }

    private func paste(_ text: String, into pid: pid_t?) -> Bool {
        guard AXIsProcessTrusted(), let pid,
              let target = NSRunningApplication(processIdentifier: pid) else {
            copyTranscript(text)
            return false
        }
        if NSWorkspace.shared.frontmostApplication?.processIdentifier != pid {
            _ = target.activate()
            Thread.sleep(forTimeInterval: 0.12)
        }
        guard NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else {
            copyTranscript(text)
            return false
        }
        let board = NSPasteboard.general
        let saved = (board.pasteboardItems ?? []).map { item in
            ClipboardItem(data: item.types.compactMap { type in
                item.data(forType: type).map { (type, $0) }
            })
        }
        board.clearContents()
        guard board.setString(text, forType: .string) else { return false }
        let stagedChange = board.changeCount
        guard let down = CGEvent(keyboardEventSource: nil, virtualKey: 9, keyDown: true),
              let up = CGEvent(keyboardEventSource: nil, virtualKey: 9, keyDown: false) else {
            return false
        }
        down.flags = .maskCommand
        up.flags = .maskCommand
        down.post(tap: .cghidEventTap)
        up.post(tap: .cghidEventTap)
        Thread.sleep(forTimeInterval: 0.4)
        if board.changeCount == stagedChange {
            board.clearContents()
            if !saved.isEmpty {
                let restored: [NSPasteboardItem] = saved.map { source in
                    let item = NSPasteboardItem()
                    for (type, data) in source.data { item.setData(data, forType: type) }
                    return item
                }
                _ = board.writeObjects(restored)
            }
        }
        return true
    }

    private func complete(_ message: String, success: Bool) {
        busy = false
        send("status", message)
        showPill(message, color: success ? .systemGreen : .systemOrange, hideAfter: 2.5)
    }

    private func makePanel() {
        let panel = NSPanel(contentRect: NSRect(x: 0, y: 0, width: 315, height: 54),
                            styleMask: [.borderless, .nonactivatingPanel], backing: .buffered,
                            defer: false)
        panel.level = .statusBar
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.ignoresMouseEvents = true
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        let material = NSVisualEffectView(frame: panel.contentView!.bounds)
        material.material = .hudWindow
        material.state = .active
        material.wantsLayer = true
        material.layer?.cornerRadius = 15
        material.layer?.masksToBounds = true
        let label = NSTextField(labelWithString: "")
        label.font = .systemFont(ofSize: 14, weight: .medium)
        label.alignment = .center
        label.frame = NSRect(x: 12, y: 14, width: 291, height: 25)
        material.addSubview(label)
        panel.contentView = material
        self.panel = panel
        self.panelLabel = label
    }

    private func showPill(_ text: String, color: NSColor, hideAfter delay: TimeInterval? = nil) {
        guard let panel, let label = panelLabel else { return }
        hideGeneration += 1
        let generation = hideGeneration
        label.stringValue = text
        label.textColor = color
        if let screen = NSScreen.main {
            panel.setFrameOrigin(NSPoint(x: screen.visibleFrame.midX - panel.frame.width / 2,
                                         y: screen.visibleFrame.maxY - 76))
        }
        panel.orderFrontRegardless()
        if let delay {
            DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in
                guard let self, self.hideGeneration == generation else { return }
                panel.orderOut(nil)
            }
        }
    }

    private func installInputCommands() {
        DispatchQueue.global(qos: .utility).async { [weak self] in
            while let line = readLine() {
                guard let bytes = line.data(using: .utf8),
                      let command = (try? JSONSerialization.jsonObject(with: bytes)) as? [String: Any] else { continue }
                DispatchQueue.main.async {
                    guard let self else { return }
                    if command["quit"] as? Bool == true { self.shutdown() }
                    if let language = command["language"] as? String,
                       Self.validLanguage(language) { self.language = language }
                }
            }
            DispatchQueue.main.async { self?.shutdown() }
        }
    }

    private func shutdown() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        recorder?.terminate()
        if let recordingURL { try? FileManager.default.removeItem(at: recordingURL) }
        NSApplication.shared.stop(nil)
        exit(0)
    }

    func testPaste(_ text: String) {
        let destination = focusedPID()
        let pasted = paste(text, into: destination)
        send("pasteTest", pasted ? "inserted" : "copied", extra: ["pasted": pasted])
    }
}

@main
private struct PolymathHelperMain {
    static func main() {
        let args = CommandLine.arguments
        if args.contains("--clipboard-self-test") {
            print(testPolymathClipboard() ? "PASS: clipboard persistence and pinning" : "FAIL: clipboard history")
        } else if args.contains("--self-test") {
            // The helper must be able to run without either privacy permission.
            print("Polymath global dictation helper ready")
        } else if let index = args.firstIndex(of: "--test-paste"),
                  args.indices.contains(index + 1) {
            DictationBridge(language: "auto", parentPID: 0).testPaste(args[index + 1])
        } else if args.count > 1 && args[1] == "--clipboard" {
            let parent = args.count > 2 ? pid_t(args[2]) ?? 0 : 0
            let shortcut = args.count > 3 ? args[3] : "cmd+d"
            runPolymathClipboard(parentPID: parent, shortcut: shortcut)
        } else {
            let language = args.count > 1 ? args[1] : "auto"
            let parent = args.count > 2 ? pid_t(args[2]) ?? 0 : 0
            DictationBridge(language: language, parentPID: parent,
                            hotkeyTest: args.contains("--test-hotkey")).run()
        }
    }
}
