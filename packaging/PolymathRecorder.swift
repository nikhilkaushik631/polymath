import AVFoundation
import Foundation

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8))
    exit(1)
}

let permission = DispatchSemaphore(value: 0)
var granted = false
AVCaptureDevice.requestAccess(for: .audio) { allowed in
    granted = allowed
    permission.signal()
}
guard permission.wait(timeout: .now() + 60) == .success && granted else {
    fail("Allow microphone access for Polymath in macOS Privacy & Security settings.")
}

guard let requestedPath = CommandLine.arguments.dropFirst().first,
      requestedPath.hasPrefix(NSTemporaryDirectory() + "polymath-dictation-"),
      requestedPath.hasSuffix(".wav") else {
    fail("Invalid temporary recording path.")
}
let url = URL(fileURLWithPath: requestedPath)
let settings: [String: Any] = [
    AVFormatIDKey: Int(kAudioFormatLinearPCM),
    AVSampleRateKey: 16000.0,
    AVNumberOfChannelsKey: 1,
    AVLinearPCMBitDepthKey: 16,
    AVLinearPCMIsFloatKey: false,
    AVLinearPCMIsBigEndianKey: false,
    AVLinearPCMIsNonInterleaved: false,
]
let recorder: AVAudioRecorder
do {
    recorder = try AVAudioRecorder(url: url, settings: settings)
} catch {
    fail("Could not open microphone recorder: \(error.localizedDescription)")
}
guard recorder.prepareToRecord(), recorder.record() else {
    fail("Could not start microphone recording.")
}
try? FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
// The parent sends a newline on Stop. EOF also ends this recording.
_ = readLine()
recorder.stop()
guard let size = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?.intValue,
      size > 44 else {
    try? FileManager.default.removeItem(at: url)
    fail("No audio was recorded.")
}
FileHandle.standardOutput.write(Data("recorded".utf8))
