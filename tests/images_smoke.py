#!/usr/bin/env python3
"""Exercise the embedded Images workspace with a synthetic local PNG."""
from pathlib import Path
import importlib.util
import os
import struct
import subprocess
import time
import zlib

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("bridge", ROOT / "packaging/agent_bridge.py")
bridge = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge)


def chunk(kind: bytes, payload: bytes) -> bytes:
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(
        ">I", zlib.crc32(kind + payload) & 0xFFFFFFFF
    )


def fixture(path: Path) -> None:
    width, height = 32, 24
    rows = b"".join(
        b"\0" + b"".join(bytes((x * 7, y * 10, 110, 255)) for x in range(width))
        for y in range(height)
    )
    path.write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(rows))
        + chunk(b"IEND", b"")
    )


def main() -> None:
    qa = ROOT / "target/native-qa"
    qa.mkdir(parents=True, exist_ok=True)
    source = qa / "synthetic-image.png"
    exported = qa / "edited-image.png"
    endpoint = qa / f"images-control-{os.getpid()}.json"
    fixture(source)
    binary = ROOT / "dist/Polymath.app/Contents/MacOS/Polymath"
    if not binary.exists():
        binary = ROOT.parent / "reader/target/release/polymath"
    env = os.environ.copy()
    env["POLYMATH_NO_PREFS"] = "1"
    with (qa / "images-app.log").open("w") as log:
        app = subprocess.Popen(
            [str(binary), "--control", str(endpoint)],
            stdout=log,
            stderr=log,
            env=env,
        )
        try:
            deadline = time.monotonic() + 60
            while not endpoint.exists():
                if app.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError("Polymath failed to start; see target/native-qa/images-app.log")
                time.sleep(0.2)

            def call(method: str, params: dict | None = None):
                return bridge.request(endpoint, method, params)

            assert call("app.open", {"path": str(source)})["workspace"] == "images"
            deadline = time.monotonic() + 30
            while not call("document.inspect", {"workspace": "images"})["photo"]:
                if time.monotonic() > deadline:
                    raise RuntimeError("The image did not finish importing")
                time.sleep(0.2)
            original = source.read_bytes()
            call(
                "engine.execute",
                {
                    "workspace": "images",
                    "command": "develop.set",
                    "params": {"control": "light.exposure", "value": 0.5},
                },
            )
            result = call(
                "engine.execute",
                {
                    "workspace": "images",
                    "command": "app.export",
                    "params": {"path": str(exported), "format": "png"},
                },
            )
            assert exported.is_file() and exported.stat().st_size > 0, result
            assert source.read_bytes() == original, "Image import must preserve the original"
            assert exported.read_bytes() != original, "The edited output must differ"
            call("app.workspace", {"workspace": "home"})
            assert call("app.inspect")["editors"]["images"] >= 1
            call("app.quit")
            app.wait(timeout=15)
            assert app.returncode == 0, app.returncode
            print("PASS: image routing, import, non-destructive exposure edit, PNG export, workspace return")
        finally:
            if app.poll() is None:
                app.terminate()
                app.wait(timeout=15)
            endpoint.unlink(missing_ok=True)


if __name__ == "__main__":
    main()
