import AppKit
import Foundation

// A temporary native text field for the global paste QA. It writes only its
// current text to the caller's scratch file, never a user's document.
final class Target: NSObject, NSApplicationDelegate, NSTextViewDelegate {
    private let output: URL
    private var window: NSWindow?
    private var editor: NSTextView?

    init(output: URL) {
        self.output = output
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        let menu = NSMenu()
        let edit = NSMenuItem(title: "Edit", action: nil, keyEquivalent: "")
        let editMenu = NSMenu(title: "Edit")
        editMenu.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        edit.submenu = editMenu
        menu.addItem(edit)
        NSApplication.shared.mainMenu = menu
        let window = NSWindow(contentRect: NSRect(x: 160, y: 180, width: 500, height: 230),
                              styleMask: [.titled, .closable, .resizable],
                              backing: .buffered, defer: false)
        window.title = "Polymath dictation QA"
        let editor = NSTextView(frame: window.contentView!.bounds)
        editor.isEditable = true
        editor.delegate = self
        editor.autoresizingMask = [.width, .height]
        window.contentView = editor
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(editor)
        NSApplication.shared.activate()
        self.window = window
        self.editor = editor
        try? "".write(to: output, atomically: true, encoding: .utf8)
    }

    func textDidChange(_ notification: Notification) {
        try? editor?.string.write(to: output, atomically: true, encoding: .utf8)
    }
}

guard CommandLine.arguments.count == 2 else { exit(2) }
let app = NSApplication.shared
app.setActivationPolicy(.regular)
let target = Target(output: URL(fileURLWithPath: CommandLine.arguments[1]))
app.delegate = target
app.run()
