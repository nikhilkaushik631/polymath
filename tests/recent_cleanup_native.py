#!/usr/bin/env python3
"""Check that moved Recent files vanish from the UI and saved preferences."""

import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent
BINARY = Path(
    os.environ.get(
        "POLYMATH_QA_BINARY",
        ROOT.parent / "reader/target/debug/polymath",
    )
)
spec = importlib.util.spec_from_file_location("bridge", ROOT / "packaging/agent_bridge.py")
assert spec and spec.loader
bridge = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge)

with tempfile.TemporaryDirectory(prefix="polymath-recent-", dir=ROOT / "target") as scratch:
    folder = Path(scratch)
    existing = folder / "existing.txt"
    existing.write_text("still here", encoding="utf-8")
    missing = folder / "moved.txt"
    prefs = folder / "preferences.json"
    prefs.write_text(json.dumps({"recent": [str(missing), str(existing)]}), encoding="utf-8")
    endpoint = folder / "control.json"
    env = os.environ.copy()
    env.pop("POLYMATH_NO_PREFS", None)
    env.pop("IRIS_NO_PREFS", None)
    env["POLYMATH_PREFS_PATH"] = str(prefs)
    with (folder / "app.log").open("w") as log:
        process = subprocess.Popen(
            [str(BINARY), "--control", str(endpoint)], stdout=log, stderr=log, env=env
        )
        try:
            deadline = time.monotonic() + 30
            while not endpoint.is_file():
                assert process.poll() is None and time.monotonic() < deadline
                time.sleep(0.1)

            def recent():
                return bridge.request(endpoint, "app.inspect")["recent"]

            assert recent() == [str(existing)], recent()
            assert json.loads(prefs.read_text())["recent"] == [str(existing)]

            existing.unlink()
            deadline = time.monotonic() + 10
            while recent() and time.monotonic() < deadline:
                time.sleep(0.2)
            assert recent() == []
            assert json.loads(prefs.read_text())["recent"] == []
            print("PASS: moved Recent files disappear on launch and during use, then stay removed")
        finally:
            process.terminate()
            process.wait(timeout=10)
