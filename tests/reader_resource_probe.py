#!/usr/bin/env python3
"""Measure a signed Polymath Reader process with synthetic open tabs on macOS."""

import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "target/reader-resource-probe"
OUT.mkdir(parents=True, exist_ok=True)
ENDPOINT = OUT / f"control-{os.getpid()}.json"
BINARY = ROOT / "dist/Polymath.app/Contents/MacOS/Polymath"
PDF = ROOT / "target/native-qa/reader.pdf"
VIDEO = ROOT / "target/reader-visual-qa/visual-audit-video-16x10.mp4"
if not PDF.is_file() or not VIDEO.is_file():
    raise SystemExit("Run native_smoke.py and reader_visual_qa.py first for synthetic fixtures.")

spec = importlib.util.spec_from_file_location("bridge", ROOT / "packaging/agent_bridge.py")
bridge = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge)

env = os.environ.copy()
env["POLYMATH_NO_PREFS"] = "1"
with (OUT / "app.log").open("w") as log:
    process = subprocess.Popen(
        [str(BINARY), "--control", str(ENDPOINT)],
        stdout=log,
        stderr=log,
        env=env,
    )

    def call(method, params=None):
        return bridge.request(ENDPOINT, method, params)

    def sample(stage):
        time.sleep(3)
        line = subprocess.check_output(
            ["ps", "-p", str(process.pid), "-o", "rss=", "-o", "%cpu="], text=True
        ).strip()
        rss_kib, cpu_percent = line.split()
        vmmap = subprocess.run(
            ["vmmap", "-summary", str(process.pid)],
            capture_output=True,
            text=True,
            timeout=30,
            check=True,
        )
        footprint = re.search(r"^Physical footprint:\s+([\d.]+)([KMG])", vmmap.stdout, re.M)
        if footprint is None:
            raise RuntimeError("vmmap did not report a physical footprint")
        factor = {"K": 1 / 1024, "M": 1, "G": 1024}[footprint.group(2)]
        tabs = len(call("app.reader.tabs")["tabs"])
        result = {
            "stage": stage,
            "tabs": tabs,
            "rss_mib": round(int(rss_kib) / 1024, 1),
            "physical_footprint_mib": round(float(footprint.group(1)) * factor, 1),
            "cpu_percent_snapshot": float(cpu_percent),
        }
        samples.append(result)
        print(json.dumps(result), flush=True)

    samples = []
    try:
        deadline = time.monotonic() + 30
        while not ENDPOINT.exists():
            if process.poll() is not None or time.monotonic() > deadline:
                raise RuntimeError("Polymath did not start; see app.log")
            time.sleep(0.1)
        call("app.workspace", {"workspace": "reader"})
        sample("empty reader")

        source_paths = []
        for index in range(15):
            path = OUT / f"synthetic-source-{index:02}.jsx"
            path.write_text(
                "\n".join(f"export const item{row} = {row};" for row in range(400)),
                encoding="utf-8",
            )
            source_paths.append(path)
            call("app.open", {"path": str(path)})
        sample("15 source tabs")

        pdf_paths = []
        for index in range(15):
            path = OUT / f"synthetic-document-{index:02}.pdf"
            shutil.copyfile(PDF, path)
            pdf_paths.append(path)
            call("app.open", {"path": str(path)})
        sample("15 source + 15 PDF tabs")

        call("app.open", {"path": str(VIDEO)})
        deadline = time.monotonic() + 15
        while call("app.media.inspect")["state"] == "loading" and time.monotonic() < deadline:
            time.sleep(0.2)
        sample("30 documents + 1 paused video")

        call("app.reader.close", {"path": str(VIDEO)})
        for path in pdf_paths:
            call("app.reader.close", {"path": str(path)})
        sample("15 source tabs after closing PDFs and video")
        time.sleep(20)
        sample("15 source tabs settled idle")
        (OUT / "results.json").write_text(json.dumps(samples, indent=2) + "\n")
    finally:
        process.terminate()
        process.wait(timeout=10)
