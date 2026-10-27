#!/usr/bin/env python3
"""Check the signed app's offline Indian-language speech bundle with synthetic audio."""
from pathlib import Path
import hashlib
import plistlib
import subprocess

ROOT = Path(__file__).resolve().parent.parent
APP = ROOT / "dist/Polymath.app"
BIN = APP / "Contents/MacOS"
MODEL = APP / "Contents/Resources/models/ggml-small-q5_1.bin"
QA = ROOT / "target/native-qa/voice-smoke"
EXPECTED_MODEL_SHA256 = "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb"


def main() -> None:
    QA.mkdir(parents=True, exist_ok=True)
    plist = plistlib.loads((APP / "Contents/Info.plist").read_bytes())
    assert plist["CFBundleShortVersionString"] == "0.4.0"
    assert plist["NSMicrophoneUsageDescription"]
    assert (BIN / "PolymathRecorder").is_file()
    assert (BIN / "PolymathWhisper").is_file()
    digest = hashlib.sha256()
    with MODEL.open("rb") as model_file:
        for chunk in iter(lambda: model_file.read(1024 * 1024), b""):
            digest.update(chunk)
    assert digest.hexdigest() == EXPECTED_MODEL_SHA256

    voices = subprocess.check_output(["/usr/bin/say", "-v", "?"], text=True)
    cases = [
        ("Hindi", "hi", "Lekha", "hi_IN", "नमस्ते, मेरा नाम लेखा है। आज हम किताब पढ़ेंगे।", "\u0900", "\u097f"),
        ("Tamil", "ta", "Vani", "ta_IN", "வணக்கம். இன்று நாம் ஒரு புத்தகம் படிக்கலாம்.", "\u0b80", "\u0bff"),
    ]
    for name, code, voice, locale, sample, first, last in cases:
        assert voice in voices and locale in voices
        aiff = QA / f"{code}.aiff"
        wav = QA / f"{code}.wav"
        output = QA / f"{code}-transcript"
        for path in (aiff, wav, output.with_suffix(".txt")):
            path.unlink(missing_ok=True)
        subprocess.run(
            ["/usr/bin/say", "-v", voice, "-o", str(aiff), sample],
            check=True,
            timeout=30,
        )
        subprocess.run(
            ["/usr/bin/afconvert", "-f", "WAVE", "-d", "LEI16@16000", "-c", "1", str(aiff), str(wav)],
            check=True,
            timeout=30,
        )
        result = subprocess.run(
            [str(BIN / "PolymathWhisper"), "-m", str(MODEL), "-f", str(wav), "-l", code, "-nt", "-np", "-otxt", "-of", str(output)],
            text=True,
            capture_output=True,
            timeout=120,
        )
        assert result.returncode == 0, result.stderr[-2000:]
        transcript = output.with_suffix(".txt").read_text(encoding="utf-8").strip()
        assert any(first <= char <= last for char in transcript), transcript
        print(f"PASS: bundled {name} transcription: {transcript}")
    print("PASS: local macOS voices, microphone metadata, and model checksum")


if __name__ == "__main__":
    main()
