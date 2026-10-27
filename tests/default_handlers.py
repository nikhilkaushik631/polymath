#!/usr/bin/env python3
"""Verify installed macOS defaults by opening synthetic files without `open -a`."""
from pathlib import Path
import importlib.util
import os
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "target/native-qa/default-handlers"
OUT.mkdir(parents=True, exist_ok=True)
APP = Path("/Applications/Polymath.app")
ENDPOINT = OUT / f"control-{os.getpid()}.json"
BRIDGE_SPEC = importlib.util.spec_from_file_location("bridge", ROOT / "packaging/agent_bridge.py")
BRIDGE = importlib.util.module_from_spec(BRIDGE_SPEC)
BRIDGE_SPEC.loader.exec_module(BRIDGE)

subprocess.run(
    ["swift", str(ROOT / "packaging/default-documents.swift"), "--check"],
    check=True,
    stdout=subprocess.DEVNULL,
)
word = OUT / "default-word.docx"
sheet = OUT / "default-sheet.xlsx"
template = OUT / "default-template.xltx"
shutil.copyfile(ROOT.parent / "writer/target/agent-integration/agent-demo.docx", word)
shutil.copyfile(ROOT.parent / "sheets/target/agent-integration/agent-demo.xlsx", sheet)
shutil.copyfile(sheet, template)
env = os.environ.copy()
env.update(POLYMATH_NO_PREFS="1", PDFCRAFT_SYSTEM_FONTS="0")
with (OUT / "app.log").open("w") as log:
    process = subprocess.Popen(
        [str(APP / "Contents/MacOS/Polymath"), "--control", str(ENDPOINT)],
        stdout=log,
        stderr=log,
        env=env,
    )
    try:
        deadline = time.monotonic() + 45
        while not ENDPOINT.exists():
            if process.poll() is not None or time.monotonic() > deadline:
                raise RuntimeError("Installed Polymath did not start")
            time.sleep(.2)
        for workspace, path in (("writer", word), ("sheets", sheet), ("sheets", template)):
            subprocess.run(["open", str(path)], check=True)
            deadline = time.monotonic() + 15
            while True:
                state = BRIDGE.request(ENDPOINT, "app.inspect")
                if state["workspace"] == workspace and state["recent"][:1] == [str(path)]:
                    break
                if time.monotonic() > deadline:
                    raise AssertionError(f"Finder did not deliver {path} to {workspace}: {state}")
                time.sleep(.2)
            assert BRIDGE.request(ENDPOINT, "document.inspect", {"workspace": workspace})
            print(f"Finder default: .{path.suffix[1:]} → Polymath {workspace}", flush=True)
        BRIDGE.request(ENDPOINT, "app.quit")
        process.wait(timeout=15)
        assert process.returncode == 0
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=15)
        ENDPOINT.unlink(missing_ok=True)
