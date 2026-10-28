#!/usr/bin/env python3
"""Exercise the bundled shortcut helper lifecycle without recording audio."""

import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent
BINARY = Path(os.environ.get("POLYMATH_QA_BINARY", ROOT.parent / "reader/target/debug/polymath"))
HELPER = Path(os.environ.get("POLYMATH_QA_GLOBAL_HELPER", ROOT / "target/PolymathGlobalDictation"))
spec = importlib.util.spec_from_file_location("bridge", ROOT / "packaging/agent_bridge.py")
assert spec and spec.loader
bridge = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge)

with tempfile.TemporaryDirectory(prefix="polymath-global-dictation-", dir=ROOT / "target") as scratch:
    folder = Path(scratch)
    endpoint = folder / "control.json"
    env = os.environ.copy()
    env["POLYMATH_NO_PREFS"] = "1"
    env["POLYMATH_GLOBAL_DICTATION_HELPER"] = str(HELPER.resolve())
    with (folder / "app.log").open("w") as log:
        app = subprocess.Popen([str(BINARY), "--control", str(endpoint)],
                               stdout=log, stderr=log, env=env)
        try:
            deadline = time.monotonic() + 30
            while not endpoint.is_file():
                assert app.poll() is None and time.monotonic() < deadline
                time.sleep(0.1)

            def state(enabled=None):
                params = {} if enabled is None else {"enabled": enabled}
                return bridge.request(endpoint, "app.dictation.global", params)

            assert state()["enabled"] is False
            assert state(True)["enabled"] is True
            deadline = time.monotonic() + 10
            while True:
                current = state()
                if current["status"] != "Starting global dictation…":
                    break
                assert time.monotonic() < deadline
                time.sleep(0.2)
            assert any(word in current["status"] for word in
                       ("Ready", "Allow Input Monitoring", "Shortcut ready")), current
            assert state(False) == {
                "enabled": False,
                "status": "Off",
                "shortcut": "right Command + right Option",
            }
            print("PASS: global shortcut starts, reports permission state, and stops")
        finally:
            app.terminate()
            app.wait(timeout=10)
