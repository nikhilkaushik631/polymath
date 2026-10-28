#!/usr/bin/env python3
"""Check that a ZIP archive stays in Reader's switchable file tabs."""
from pathlib import Path
import importlib.util
import os
import subprocess
import tempfile
import time
import zipfile

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("bridge", ROOT / "packaging/agent_bridge.py")
assert SPEC and SPEC.loader
BRIDGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BRIDGE)
BINARY = Path(os.environ.get("POLYMATH_QA_BINARY", ROOT.parent / "reader/target/debug/polymath"))

with tempfile.TemporaryDirectory(prefix="polymath-archive-", dir=ROOT / "target") as scratch:
    folder = Path(scratch)
    endpoint = folder / "control.json"
    archive = folder / "notes.zip"
    with zipfile.ZipFile(archive, "w") as bundle:
        bundle.writestr("notes/first.txt", "First note")
        bundle.writestr("notes/second.txt", "Second note")
        bundle.writestr("../outside.txt", "Unsafe path")
    env = os.environ.copy()
    env.update(POLYMATH_NO_PREFS="1", PDFCRAFT_SYSTEM_FONTS="0")
    with (folder / "app.log").open("w") as log:
        app = subprocess.Popen([str(BINARY), "--control", str(endpoint)],
                               env=env, stdout=log, stderr=log)
        try:
            deadline = time.monotonic() + 40
            while not endpoint.exists():
                if app.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError("Polymath did not start")
                time.sleep(.2)

            def call(name, args=None):
                return BRIDGE.request(endpoint, name, args)

            assert call("app.open", {"path": str(archive)})["workspace"] == "reader"
            deadline = time.monotonic() + 15
            while True:
                state = call("app.inspect")["archivePreview"]
                if state["state"] != "loading":
                    break
                if time.monotonic() > deadline:
                    raise RuntimeError("Archive listing timed out")
                time.sleep(.2)
            assert state["state"] == "ready" and state["entries"] == 3, state
            tabs = call("app.reader.tabs")["tabs"]
            assert len(tabs) == 1 and tabs[0]["kind"] == "Archive", tabs
            assert call("app.reader.close", {"path": str(archive)})["closeRequested"]
            assert call("app.reader.tabs")["tabs"] == []
            print("PASS: ZIP opens, lists, and closes in a Reader tab")
        finally:
            app.terminate()
            app.wait(timeout=10)
