#!/usr/bin/env python3
"""Check Writer's Save As page and real Word/PDF output in a private Polymath window."""
from pathlib import Path
import importlib.util
import json
import os
import subprocess
import time

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "target/native-qa/writer-save-as"
OUT.mkdir(parents=True, exist_ok=True)
ENDPOINT = OUT / f"control-{os.getpid()}.json"
SPEC = importlib.util.spec_from_file_location("bridge", ROOT / "packaging/agent_bridge.py")
BRIDGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BRIDGE)
BINARY = ROOT.parent / "reader/target/release/polymath"
LOG = (OUT / "app.log").open("w")
ENV = os.environ.copy()
ENV.update(POLYMATH_NO_PREFS="1", PDFCRAFT_SYSTEM_FONTS="0")
PROCESS = subprocess.Popen([str(BINARY), "--control", str(ENDPOINT)], stdout=LOG, stderr=LOG, env=ENV)


def call(method, params=None):
    return BRIDGE.request(ENDPOINT, method, params)


def writer(command, params=None):
    return call("engine.execute", {"workspace": "writer", "command": command, "params": params or {}})


try:
    deadline = time.monotonic() + 60
    while not ENDPOINT.exists():
        if PROCESS.poll() is not None or time.monotonic() > deadline:
            raise RuntimeError("Polymath did not start; inspect app.log")
        time.sleep(.2)
    call("app.workspace", {"workspace": "writer"})
    writer("text.insert", {"text": "Polymath Writer Save As verification."})
    call("ui.screenshot", {"path": str(OUT / "writer-toolbar-light.png")})
    assert writer("file.saveAs")["saved"] is False
    ui = call("ui.inspect", {"workspace": "writer"})["ui"]
    assert ui["backstage"] and ui["backstagePage"] == "saveAs", ui
    subprocess.run(["swift", "-e", f"import AppKit; NSRunningApplication(processIdentifier: {PROCESS.pid})?.activate(options: [])"], capture_output=True, timeout=15)
    time.sleep(.6)
    call("ui.screenshot", {"path": str(OUT / "save-as-light.png")})
    call("app.theme", {"dark": True})
    call("ui.screenshot", {"path": str(OUT / "save-as-dark.png")})

    docx = OUT / "saved-writer.docx"
    pdf = OUT / "saved-writer.pdf"
    writer("file.saveAs", {"path": str(docx)})
    assert docx.read_bytes().startswith(b"PK")
    assert call("app.inspect")["recent"][0] == str(docx), "Writer saves must appear on Polymath Home"
    call("app.workspace", {"workspace": "home"})
    call("app.theme", {"dark": False})
    call("ui.screenshot", {"path": str(OUT / "home-writer-recent-light.png")})
    call("app.theme", {"dark": True})
    call("ui.screenshot", {"path": str(OUT / "home-writer-recent-dark.png")})
    call("app.workspace", {"workspace": "writer"})
    writer("text.insert", {"text": " Later edit."})
    assert call("ui.inspect", {"workspace": "writer"})["dirty"]
    writer("file.exportPdf", {"path": str(pdf)})
    assert pdf.read_bytes().startswith(b"%PDF-")
    state = call("ui.inspect", {"workspace": "writer"})
    assert state["dirty"] and state["title"] == docx.stem, state
    writer("file.save")
    assert not call("ui.inspect", {"workspace": "writer"})["dirty"]
    call("app.quit")
    PROCESS.wait(timeout=15)
    assert PROCESS.returncode == 0, PROCESS.returncode
    print(json.dumps({"result": "PASS", "word": str(docx), "pdf": str(pdf), "screenshots": [str(OUT / "save-as-light.png"), str(OUT / "save-as-dark.png")]}))
finally:
    if PROCESS.poll() is None:
        PROCESS.terminate()
        PROCESS.wait(timeout=15)
    ENDPOINT.unlink(missing_ok=True)
    LOG.close()
