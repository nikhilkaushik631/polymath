#!/usr/bin/env python3
"""Check the signed clipboard helper without reading the user's clipboard."""

import json
import os
import select
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
HELPER = ROOT / "dist/Polymath.app/Contents/MacOS/PolymathGlobalDictation"


def main() -> None:
    result = subprocess.run(
        [str(HELPER), "--clipboard-self-test"],
        check=True,
        capture_output=True,
        text=True,
        timeout=15,
    )
    assert "PASS: clipboard persistence and pinning" in result.stdout

    with tempfile.TemporaryDirectory(prefix="polymath-clipboard-qa-") as directory:
        environment = os.environ.copy()
        environment["POLYMATH_CLIPBOARD_DIR"] = directory
        process = subprocess.Popen(
            [str(HELPER), "--clipboard", str(os.getpid()), "cmd+w"],
            env=environment,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        )
        try:
            assert process.stdout is not None
            ready, _, _ = select.select([process.stdout], [], [], 8)
            assert ready, "Clipboard helper did not report hotkey state"
            status = json.loads(process.stdout.readline())
            assert status["kind"] == "status"
            assert status["text"].startswith("Ready"), status["text"]
            print("Signed clipboard helper: self-test and Command+W registration passed")
        finally:
            if process.stdin is not None:
                process.stdin.write('{"quit":true}\n')
                process.stdin.flush()
            process.wait(timeout=5)


if __name__ == "__main__":
    main()
