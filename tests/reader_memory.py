#!/usr/bin/env python3
"""Restart Polymath with isolated preferences and verify Reader page/favorite memory."""
from pathlib import Path
import importlib.util, json, os, shutil, subprocess, tempfile, time

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("bridge", ROOT / "packaging/agent_bridge.py")
bridge = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge)
binary = ROOT / "dist/Polymath.app/Contents/MacOS/Polymath"
fixture = ROOT.parent / "reader/target/interface-scale/bookmarks.pdf"

with tempfile.TemporaryDirectory(prefix="polymath-reader-memory-") as temp:
    test_dir = Path(temp)
    home = test_dir / "home"
    home.mkdir()
    pdf = test_dir / "book.pdf"
    shutil.copyfile(fixture, pdf)
    prefs = test_dir / "preferences.json"
    env = os.environ.copy()
    env.pop("POLYMATH_NO_PREFS", None)
    env.update(HOME=str(home), POLYMATH_PREFS_PATH=str(prefs), PDFCRAFT_SYSTEM_FONTS="0")

    def run(index, verify):
        endpoint = test_dir / f"control-{index}.json"
        with (test_dir / f"app-{index}.log").open("w") as log:
            process = subprocess.Popen([str(binary), "--control", str(endpoint)], env=env, stdout=log, stderr=log)
            try:
                deadline = time.monotonic() + 45
                while not endpoint.exists():
                    if process.poll() is not None or time.monotonic() > deadline:
                        raise RuntimeError(f"Polymath did not start; see {log.name}")
                    time.sleep(.2)
                def call(method, params=None): return bridge.request(endpoint, method, params)
                verify(call)
                # Verify the periodic preference write also survives an unclean
                # process restart; the regular quit guard is covered by native_smoke.py.
                time.sleep(2.5)
                process.terminate()
                process.wait(timeout=15)
                assert process.returncode == -15
            finally:
                if process.poll() is None:
                    process.terminate()
                    process.wait(timeout=15)
                endpoint.unlink(missing_ok=True)

    def first(call):
        assert call("app.open", {"path": str(pdf)})["workspace"] == "reader"
        call("ui.set", {"workspace": "reader", "key": "page", "value": "3"})
        assert call("ui.state", {"workspace": "reader"})["active"]["page"] == 3
        assert call("app.reader.favorite", {"value": True})["favorite"] is True
        assert call("app.reader.favorites")["files"][0]["path"] == str(pdf)

    run(1, first)
    data = json.loads(prefs.read_text())
    assert data["reading_memory"]["positions"][str(pdf)] == 2
    assert data["reading_memory"]["favorites"] == [str(pdf)]

    def second(call):
        assert call("app.open", {"path": str(pdf)})["workspace"] == "reader"
        assert call("ui.state", {"workspace": "reader"})["active"]["page"] == 3
        assert call("app.reader.favorites")["files"][0] == {"path": str(pdf), "page": 3}
        assert call("app.reader.favorite", {"value": False})["favorite"] is False
        assert call("app.reader.favorites")["files"] == []

    run(2, second)
    assert json.loads(prefs.read_text())["reading_memory"]["favorites"] == []
    print("PASS: Reader restores page 3 after restart and Favorites persist/removal stays local.")
