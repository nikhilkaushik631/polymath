// Explicit setup utility, never called by the app or installer automatically.
// Uses only extensions declared by the installed Polymath bundle. Before changing
// defaults, writes the previous handlers to Application Support for recovery.
// HTML belongs to the default browser: Polymath still offers Open With for it.
// Usage: default-documents --check | --status /Applications/Polymath.app | --set /Applications/Polymath.app
import AppKit
import CoreServices
import UniformTypeIdentifiers

let setting = CommandLine.arguments.count == 3 && CommandLine.arguments[1] == "--set"
let checking = CommandLine.arguments.count == 2 && CommandLine.arguments[1] == "--check"
let reporting = CommandLine.arguments.count == 3 && CommandLine.arguments[1] == "--status"
if !setting && !checking && !reporting {
    fputs("Usage: default-documents --check | --status /Applications/Polymath.app | --set /Applications/Polymath.app\n", stderr)
    exit(2)
}
let application = (setting || reporting) ? URL(fileURLWithPath: CommandLine.arguments[2]) : nil
let expected = application ?? URL(fileURLWithPath: "/Applications/Polymath.app")
guard let bundle = Bundle(url: expected),
      bundle.bundleIdentifier == "local.polymath.app",
      let build = bundle.infoDictionary?["CFBundleVersion"] as? String,
      let buildNumber = Int(build), buildNumber >= 14,
      let documentTypes = bundle.infoDictionary?["CFBundleDocumentTypes"] as? [[String: Any]] else {
    fputs("Expected Polymath 0.4.7 or newer at \(expected.path).\n", stderr)
    exit(1)
}
// .xls is intentionally absent: Sheets does not yet import legacy binary XLS.
let extensions = Array(Set(documentTypes.flatMap { $0["CFBundleTypeExtensions"] as? [String] ?? [] }
    .map { $0.lowercased() }.filter { $0 != "html" && $0 != "htm" })).sorted()
guard !extensions.isEmpty else {
    fputs("Polymath declares no document extensions.\n", stderr)
    exit(1)
}
if setting, let application {
    // public.html can also change the http/https handler on macOS. Keep the
    // browser selected in System Settings when setting document defaults.
    for scheme in ["http", "https"] {
        guard let address = URL(string: "\(scheme)://example.org/") else { continue }
        let handler = NSWorkspace.shared.urlForApplication(toOpen: address)
        if handler.flatMap({ Bundle(url: $0)?.bundleIdentifier }) == "local.polymath.app" {
            fputs("Polymath currently handles \(scheme) links. Restore your browser first.\n", stderr)
            exit(1)
        }
    }
    // LaunchServices can prefer the build-tree copy when two Polymath bundles
    // share an identifier. Remove only that copy's registration, then register
    // the requested installation before choosing its document types.
    let register = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
    let buildCopy = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("dist/Polymath.app")
    func runRegistration(_ args: [String]) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: register)
        process.arguments = args
        try process.run()
        process.waitUntilExit()
        if process.terminationStatus != 0 {
            throw NSError(domain: "PolymathLaunchServices", code: Int(process.terminationStatus))
        }
    }
    do {
        let buildCopyIsDefault = extensions.compactMap { UTType(filenameExtension: $0) }
            .contains { NSWorkspace.shared.urlForApplication(toOpen: $0)?.standardizedFileURL == buildCopy.standardizedFileURL }
        if buildCopy.standardizedFileURL != application.standardizedFileURL,
           buildCopyIsDefault {
            // A stale registration may already have been removed by a previous run.
            try? runRegistration(["-u", buildCopy.path])
        }
        try runRegistration(["-f", application.path])
    } catch {
        fputs("Could not register Polymath at \(application.path): \(error)\n", stderr)
        exit(1)
    }
}
var failed = false
var registeredTypes: [(String, UTType)] = []
for ext in extensions {
    guard let type = UTType(filenameExtension: ext) else {
        fputs("No registered content type for .\(ext)\n", stderr)
        failed = true
        continue
    }
    registeredTypes.append((ext, type))
}
if reporting {
    let assigned = registeredTypes.filter { _, type in
        NSWorkspace.shared.urlForApplication(toOpen: type)?.standardizedFileURL == expected.standardizedFileURL
    }.count
    let report: [String: Any] = [
        "eligible": extensions.count,
        "registered": registeredTypes.count,
        "assigned": assigned,
        "unresolved": extensions.count - registeredTypes.count
    ]
    if let data = try? JSONSerialization.data(withJSONObject: report, options: [.sortedKeys]),
       let text = String(data: data, encoding: .utf8) {
        print(text)
        exit(0)
    }
    exit(1)
}
if setting {
    let directory = URL(fileURLWithPath: NSHomeDirectory())
        .appendingPathComponent("Library/Application Support/Polymath", isDirectory: true)
    let formatter = DateFormatter()
    formatter.dateFormat = "yyyyMMdd-HHmmss"
    let backup = directory.appendingPathComponent("default-handlers-before-\(formatter.string(from: Date())).json")
    let previous: [[String: String]] = registeredTypes.map { ext, type in
        let prior = NSWorkspace.shared.urlForApplication(toOpen: type)
        return ["extension": ext, "contentType": type.identifier,
                "application": prior?.path ?? "",
                "bundleIdentifier": prior.flatMap { Bundle(url: $0)?.bundleIdentifier } ?? ""]
    }
    do {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let data = try JSONSerialization.data(withJSONObject: previous, options: [.prettyPrinted, .sortedKeys])
        try data.write(to: backup, options: [.atomic])
        print("Previous defaults: \(backup.path)")
    } catch {
        fputs("Could not back up current handlers: \(error)\n", stderr)
        exit(1)
    }
}
if setting {
    guard let application else { exit(2) }
    // This API is asynchronous. Keep the process alive for its completions;
    // otherwise the last few associations can retain their previous handler.
    let needed = registeredTypes.filter { _, type in
        NSWorkspace.shared.urlForApplication(toOpen: type)?.standardizedFileURL != application.standardizedFileURL
    }
    if needed.isEmpty {
        print("All \(registeredTypes.count) document extensions already resolve to Polymath.")
        exit(failed ? 1 : 0)
    }
    // Launch Services may request consent. NSApplication's event loop is needed
    // for both the consent sheet and the completion callbacks.
    let host = NSApplication.shared
    host.setActivationPolicy(.regular)
    host.finishLaunching()
    host.activate(ignoringOtherApps: true)
    let lock = NSLock()
    var errors: [String] = []
    var remaining = needed.count
    var finished = false
    func finish(_ timedOut: Bool) {
        lock.lock()
        if finished {
            lock.unlock()
            return
        }
        finished = true
        let reported = errors
        lock.unlock()
        if timedOut {
            fputs("Timed out waiting for macOS to apply file defaults.\n", stderr)
        }
        for error in reported {
            fputs("\(error)\n", stderr)
        }
        print("Requested defaults for \(needed.count) document extensions. Run --check in a new process to verify Finder's resolved handlers.")
        exit(failed || timedOut || !reported.isEmpty ? 1 : 0)
    }
    for (ext, type) in needed {
        NSWorkspace.shared.setDefaultApplication(at: application, toOpen: type) { error in
            lock.lock()
            if let error {
                errors.append(".\(ext): \(error)")
            }
            remaining -= 1
            let done = remaining == 0
            lock.unlock()
            if done {
                DispatchQueue.main.async { finish(false) }
            }
        }
    }
    DispatchQueue.main.asyncAfter(deadline: .now() + 45) { finish(true) }
    host.run()
    exit(1)
}
for (ext, type) in registeredTypes {
    let current = NSWorkspace.shared.urlForApplication(toOpen: type)
    print(".\(ext) (\(type.identifier)): \(current?.path ?? "No default")")
    if current?.standardizedFileURL != expected.standardizedFileURL {
        failed = true
    }
}
for scheme in ["http", "https"] {
    guard let address = URL(string: "\(scheme)://example.org/") else { continue }
    let handler = NSWorkspace.shared.urlForApplication(toOpen: address)
    if handler.flatMap({ Bundle(url: $0)?.bundleIdentifier }) == "local.polymath.app" {
        fputs("Polymath must not handle \(scheme) links.\n", stderr)
        failed = true
    }
}
exit(failed ? 1 : 0)
