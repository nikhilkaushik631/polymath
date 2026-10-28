#!/usr/bin/env python3
"""Capture the native Reader surfaces most prone to layout regressions."""

import importlib.util
import os
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "target/reader-visual-qa"
OUT.mkdir(parents=True, exist_ok=True)
ENDPOINT = OUT / f"control-{os.getpid()}.json"
BINARY = Path(
    os.environ.get(
        "POLYMATH_QA_BINARY", str(ROOT / "dist/Polymath.app/Contents/MacOS/Polymath")
    )
)
spec = importlib.util.spec_from_file_location("bridge", ROOT / "packaging/agent_bridge.py")
bridge = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge)
env = os.environ.copy()
env["POLYMATH_NO_PREFS"] = "1"
with (OUT / "app.log").open("w") as log:
    process = subprocess.Popen(
        [str(BINARY), "--control", str(ENDPOINT)], stdout=log, stderr=log, env=env
    )

    def call(method, params=None):
        return bridge.request(ENDPOINT, method, params)

    def screenshot(name):
        for attempt in range(3):
            subprocess.run(
                [
                    "swift",
                    "-e",
                    f"import AppKit; NSRunningApplication(processIdentifier: {process.pid})?.activate(options: [])",
                ],
                capture_output=True,
                timeout=15,
                check=False,
            )
            time.sleep(0.7 + attempt * 0.5)
            try:
                call("ui.screenshot", {"path": str(OUT / f"{name}.png")})
                return
            except RuntimeError as error:
                if "Screenshot timed out" not in str(error) or attempt == 2:
                    raise

    try:
        deadline = time.monotonic() + 60
        while not ENDPOINT.exists():
            if process.poll() is not None or time.monotonic() > deadline:
                raise RuntimeError("Polymath did not start; see target/reader-visual-qa/app.log")
            time.sleep(0.2)
        call("app.workspace", {"workspace": "reader"})
        screenshot("reader-empty")
        call("app.theme", {"dark": True})
        screenshot("reader-empty-dark")
        call("app.theme", {"dark": False})
        first = OUT / "march9_2026_addip_source_with_a_very_long_name.sh"
        second = OUT / "visual-audit-component.jsx"
        third = OUT / "wiredleap_id_card_vertical (1).svg"
        first.write_text("#!/bin/sh\nprintf 'Polymath'\n", encoding="utf-8")
        second.write_text(
            "export const Preview = () => <section>Polymath</section>;\n"
            + "\n".join(f"export const row{i} = {i};" for i in range(2, 76))
            + "\n",
            encoding="utf-8",
        )
        third.write_text(
            "\n".join(
                [
                    '<svg xmlns="http://www.w3.org/2000/svg" width="404" height="638" viewBox="0 0 404 638">',
                    '<rect width="404" height="638" rx="28" fill="#e9f4fa"/>',
                    '<circle cx="202" cy="230" r="90" fill="#73bcd7"/>',
                    *[f"<!-- source line {i + 4} -->" for i in range(80)],
                    "</svg>",
                ]
            ),
            encoding="utf-8",
        )
        for path in (first, second, third):
            assert call("app.open", {"path": str(path)})["workspace"] == "reader"
        assert len(call("app.reader.tabs")["tabs"]) == 3
        screenshot("reader-tabs-and-source")
        call("app.theme", {"dark": True})
        screenshot("reader-tabs-and-source-dark")
        call("app.theme", {"dark": False})
        call("app.reader.select", {"path": str(second)})
        assert call("app.source.goto", {"line": 75})["line"] == 75
        screenshot("reader-source-last-line")
        call("app.reader.select", {"path": str(third)})
        call("app.reader.favorite", {"value": True, "path": str(second)})
        for path in (first, second):
            call("app.reader.close", {"path": str(path)})
        call("app.theme", {"dark": True})
        screenshot("reader-single-source-dark")
        call("app.theme", {"dark": False})
        call("app.reader.close", {"path": str(third)})
        screenshot("reader-recent-and-favorite")
        call("app.theme", {"dark": True})
        screenshot("reader-recent-and-favorite-dark")
        call("app.theme", {"dark": False})

        video = OUT / "visual-audit-video-16x10.mp4"
        if not video.exists():
            subprocess.run(
                [
                    "ffmpeg", "-hide_banner", "-loglevel", "error", "-y",
                    "-f", "lavfi", "-i", "testsrc2=size=1280x800:rate=24",
                    "-t", "12", "-c:v", "libx264", "-pix_fmt", "yuv420p", str(video),
                ],
                check=True,
                timeout=60,
            )
        assert call("app.open", {"path": str(video)})["workspace"] == "reader"
        deadline = time.monotonic() + 30
        while call("app.media.inspect")["state"] == "loading":
            if time.monotonic() > deadline:
                raise RuntimeError("Video preview did not become ready")
            time.sleep(0.2)
        screenshot("reader-video")
        call("app.media.play")
        time.sleep(0.7)
        screenshot("reader-video-playing")
        call("app.theme", {"dark": True})
        screenshot("reader-video-playing-dark")
        call("app.theme", {"dark": False})
        assert call("app.media.fullscreen", {"enabled": True})["queue"]["fullscreen"]
        time.sleep(1.2)
        screenshot("reader-video-fullscreen")
        call("app.media.fullscreen", {"enabled": False})
        print(f"PASS: Reader layout screenshots in {OUT}", flush=True)
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=15)
        ENDPOINT.unlink(missing_ok=True)
