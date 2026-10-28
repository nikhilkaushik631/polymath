#!/usr/bin/env python3
"""Verify a global transcript reaches another native app's focused text field."""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests/GlobalDictationTarget.swift"
HELPER = Path(os.environ.get(
    "POLYMATH_QA_GLOBAL_HELPER", ROOT / "target/PolymathGlobalDictation"
))
SENTENCE = "Polymath global dictation QA"

with tempfile.TemporaryDirectory(prefix="polymath-paste-", dir=ROOT / "target") as scratch:
    folder = Path(scratch)
    target_binary = folder / "PolymathPasteTarget"
    target_text = folder / "text.txt"
    subprocess.run(["swiftc", "-O", str(SOURCE), "-o", str(target_binary)], check=True)
    original = subprocess.check_output([
        "osascript", "-e",
        'tell application "System Events" to get name of first application process whose frontmost is true',
    ], text=True).strip()
    target = subprocess.Popen([str(target_binary), str(target_text)],
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        deadline = time.monotonic() + 10
        while not target_text.exists():
            assert target.poll() is None and time.monotonic() < deadline
            time.sleep(0.1)
        subprocess.run([
            "osascript", "-e",
            'tell application "System Events" to set frontmost of process "PolymathPasteTarget" to true',
        ], check=True)
        time.sleep(0.25)
        result = subprocess.check_output([str(HELPER), "--test-paste", SENTENCE], text=True)
        assert json.loads(result)["pasted"] is True, result
        deadline = time.monotonic() + 2
        while target_text.read_text() != SENTENCE:
            assert time.monotonic() < deadline, target_text.read_text()
            time.sleep(0.05)
        print("PASS: global helper pasted into a separate native text editor")
    finally:
        target.terminate()
        target.wait(timeout=5)
        if original:
            subprocess.run([
                "osascript", "-e",
                f'tell application "System Events" to set frontmost of process {json.dumps(original)} to true',
            ], check=False, capture_output=True)
