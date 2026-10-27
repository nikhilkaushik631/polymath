#!/usr/bin/env python3
"""End-to-end checks on anonymized copies of supported files from Downloads.

No original file is opened by Polymath, changed, or named in the report.
"""
from contextlib import contextmanager
from pathlib import Path
import hashlib
import json
import os
import shutil
import socket
import subprocess
import sys
import time
import zipfile

ROOT = Path(__file__).resolve().parent.parent
QA = ROOT / "target/native-qa/downloads-e2e"
DOWNLOADS = Path.home() / "Downloads"
APP = Path(os.environ.get("POLYMATH_TEST_BINARY", ROOT / "dist/Polymath.app/Contents/MacOS/Polymath"))
CLI = ROOT.parent / "reader/target/release/iris-reader-cli"
QA.mkdir(parents=True, exist_ok=True)


def valid(path: Path) -> bool:
    try:
        size = path.stat().st_size
        if not 100 < size <= 100_000_000:
            return False
        ext = path.suffix.lower()
        if ext in {".docx", ".xlsx", ".pptx"}:
            entry = {".docx": "word/document.xml", ".xlsx": "xl/workbook.xml", ".pptx": "ppt/presentation.xml"}[ext]
            with zipfile.ZipFile(path) as archive:
                return entry in archive.namelist()
        with path.open("rb") as source:
            magic = source.read(12)
        if ext == ".pdf":
            return magic.startswith(b"%PDF-") and subprocess.run(
                [str(CLI), "info", str(path)], capture_output=True, timeout=20
            ).returncode == 0
        if ext == ".png":
            return magic.startswith(b"\x89PNG\r\n\x1a\n")
        if ext in {".jpg", ".jpeg"}:
            return magic.startswith(b"\xff\xd8\xff")
        if ext == ".webp":
            return magic.startswith(b"RIFF") and magic[8:12] == b"WEBP"
        if ext in {".md", ".csv"}:
            path.read_text(encoding="utf-8")
            return True
    except (OSError, ValueError, UnicodeError, zipfile.BadZipFile, subprocess.TimeoutExpired):
        return False
    return False


def samples() -> dict[str, Path]:
    listing = subprocess.run(
        ["rg", "--files", str(DOWNLOADS)], capture_output=True, text=True, check=True
    ).stdout.splitlines()
    by_type: dict[str, list[Path]] = {}
    for name in listing:
        path = Path(name)
        if path.suffix.lower() in {".pdf", ".docx", ".md", ".xlsx", ".csv", ".pptx", ".png", ".jpg", ".webp"}:
            by_type.setdefault(path.suffix.lower(), []).append(path)
    selected = {}
    for ext, paths in by_type.items():
        for path in sorted(paths, key=lambda p: p.stat().st_size):
            if valid(path):
                selected[ext] = path
                break
    return selected


def copy_sample(source: Path, label: str) -> Path:
    target = QA / f"{label}{source.suffix.lower()}"
    shutil.copyfile(source, target)
    return target


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


@contextmanager
def app_session(label: str):
    endpoint = QA / f"control-{label}-{os.getpid()}.json"
    env = os.environ.copy()
    env["POLYMATH_NO_PREFS"] = "1"
    with (QA / f"app-{label}.log").open("w") as log:
        app = subprocess.Popen(
            [str(APP), "--control", str(endpoint)], stdout=log, stderr=log, env=env
        )
        try:
            deadline = time.monotonic() + 60
            while not endpoint.exists():
                if app.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError("Polymath failed to start")
                time.sleep(0.2)

            def call(method: str, params: dict | None = None):
                auth = json.loads(endpoint.read_text())
                with socket.create_connection(("127.0.0.1", auth["port"]), timeout=10) as conn:
                    conn.settimeout(180)
                    conn.sendall((json.dumps({"id": 1, "token": auth["token"], "method": method, "params": params or {}}) + "\n").encode())
                    raw = conn.makefile("rb").readline(32 * 1024 * 1024)
                if not raw:
                    raise RuntimeError("Polymath closed the control connection")
                reply = json.loads(raw)
                if not reply.get("ok"):
                    raise RuntimeError(reply.get("error", "Polymath command failed"))
                return reply.get("result")

            yield call
            quitting = call("app.quit")
            assert not quitting.get("dirtyWorkspaces"), quitting
            app.wait(timeout=15)
            assert app.returncode == 0, app.returncode
        finally:
            if app.poll() is None:
                app.terminate()
                app.wait(timeout=15)
            endpoint.unlink(missing_ok=True)


def editor_case(ext: str, source: Path) -> dict:
    workspace = {".pdf": "reader", ".docx": "writer", ".md": "writer", ".xlsx": "sheets", ".csv": "sheets", ".pptx": "slides"}[ext]
    label = ext[1:]
    original_before = digest(source)
    local = copy_sample(source, f"input-{label}")
    before = digest(local)
    with app_session(label) as call:
        opened = call("app.open", {"path": str(local)})
        assert opened["workspace"] == "reader", opened
        if ext != ".pdf":
            assert call("app.open", {"path": str(local), "edit": True})["workspace"] == workspace
        inspected = call("document.inspect", {"workspace": workspace})
        assert inspected, "Empty document inspection"
        def execute(command: str, params: dict | None = None):
            return call("engine.execute", {"workspace": workspace, "command": command, "params": params or {}})
        if ext == ".pdf":
            execute("page.rotate")
            execute("file.save")
            assert digest(local) != before, "PDF save did not write the rotation"
            call("app.open", {"path": str(local)})
            assert call("document.inspect", {"workspace": workspace})
        elif ext in {".docx", ".md"}:
            execute("text.insert", {"text": "\nPolymath end-to-end check.\n"})
            output = QA / f"edited-{label}.docx"
            output.unlink(missing_ok=True)
            execute("file.saveAs", {"path": str(output)})
            assert output.is_file() and output.stat().st_size > 0
            call("app.open", {"path": str(output), "edit": True})
            assert "Polymath end-to-end check." in json.dumps(call("document.inspect", {"workspace": workspace}))
        elif ext in {".xlsx", ".csv"}:
            execute("cell.set", {"cell": "A500", "input": "72849"})
            output = QA / f"edited-{label}.xlsx"
            output.unlink(missing_ok=True)
            execute("file.saveAs", {"path": str(output)})
            assert output.is_file() and output.stat().st_size > 0
            call("app.open", {"path": str(output), "edit": True})
            assert execute("cell.get", {"cell": "A500"})["value"] == 72849
        else:
            count = len(inspected["slides"])
            execute("slide.new")
            output = QA / "edited-pptx.pptx"
            output.unlink(missing_ok=True)
            execute("file.saveAs", {"path": str(output)})
            assert output.is_file() and output.stat().st_size > 0
            call("app.open", {"path": str(output), "edit": True})
            assert len(call("document.inspect", {"workspace": workspace})["slides"]) == count + 1
    assert digest(source) == original_before, "Original changed"
    return {"workspace": workspace, "input_bytes": local.stat().st_size, "opened": True, "saved_and_reopened": True}


def image_case(ext: str, source: Path) -> dict:
    label = ext[1:]
    local = copy_sample(source, f"input-{label}")
    before = digest(local)
    output = QA / f"edited-{label}.png"
    output.unlink(missing_ok=True)
    with app_session(label) as call:
        assert call("app.open", {"path": str(local)})["workspace"] == "reader"
        assert call("app.open", {"path": str(local), "edit": True})["workspace"] == "images"
        deadline = time.monotonic() + 60
        while not call("document.inspect", {"workspace": "images"})["photo"]:
            if time.monotonic() > deadline:
                raise RuntimeError("Image import did not finish")
            time.sleep(0.2)
        call("engine.execute", {"workspace": "images", "command": "develop.set", "params": {"control": "light.exposure", "value": 0.5}})
        call("engine.execute", {"workspace": "images", "command": "app.export", "params": {"path": str(output), "format": "png"}})
        assert output.is_file() and output.stat().st_size > 0
    assert digest(local) == before and digest(source) == before, "Original image changed"
    return {"workspace": "images", "input_bytes": local.stat().st_size, "opened": True, "exported": True}


def pdf_tools(first: Path, second: Path, image: Path | None) -> dict:
    first_before, second_before = digest(first), digest(second)
    a = copy_sample(first, "pdf-tools-a")
    b = copy_sample(second, "pdf-tools-b")
    created = {}
    steps = [
        ("edit", [str(a), "--out", str(QA / "pdf-rotated.pdf"), "--rotate", "1:90"]),
        ("combine", [str(a), str(b), "--out", str(QA / "pdf-combined.pdf")]),
        ("extract", [str(a), "--pages", "1", "--out", str(QA / "pdf-extracted.pdf")]),
        ("render", [str(a), "--page", "1", "--out", str(QA / "pdf-rendered.png")]),
    ]
    for name, args in steps:
        if name != "render":
            Path(args[args.index("--out") + 1]).unlink(missing_ok=True)
        else:
            (QA / "pdf-rendered.png").unlink(missing_ok=True)
        result = subprocess.run([str(CLI), name, *args], capture_output=True, timeout=120)
        if result.returncode:
            raise RuntimeError(f"PDF {name} failed: {result.stderr.decode(errors='replace')[:250]}")
        created[name] = True
    for name in ("pdf-rotated.pdf", "pdf-combined.pdf", "pdf-extracted.pdf"):
        assert subprocess.run([str(CLI), "info", str(QA / name)], capture_output=True, timeout=30).returncode == 0
    assert (QA / "pdf-rendered.png").read_bytes().startswith(b"\x89PNG")
    split = QA / "pdf-split"
    split.mkdir(exist_ok=True)
    assert subprocess.run([str(CLI), "split", str(a), "--every", "1", "--out-dir", str(split)], capture_output=True, timeout=120).returncode == 0
    assert list(split.glob("*.pdf")), "Split produced no PDFs"
    created["split"] = True

    def scripted(name: str, steps: list[dict], output: Path) -> None:
        output.unlink(missing_ok=True)
        script = QA / f"pdf-{name}-steps.json"
        script.write_text(json.dumps(steps))
        result = subprocess.run([str(CLI), "run", "--script", str(script)], capture_output=True, timeout=120)
        if result.returncode or not output.is_file() or output.stat().st_size == 0:
            raise RuntimeError(f"PDF {name} failed: {result.stderr.decode(errors='replace')[:250]}")
        created[name] = True

    def opened() -> dict:
        return {"tool": "doc_open", "args": {"path": str(a)}}

    optimized = QA / "pdf-optimized.pdf"
    scripted("optimize", [opened(), {"tool": "doc_optimize", "args": {"doc": 1, "path": str(optimized)}}], optimized)
    converted = QA / "pdf-to-word.docx"
    scripted("convert-to-word", [opened(), {"tool": "doc_export_office", "args": {"doc": 1, "path": str(converted)}}], converted)
    assert zipfile.is_zipfile(converted), "PDF to Word did not create DOCX"
    protected = QA / "pdf-protected.pdf"
    scripted("protect", [opened(), {"tool": "doc_protect", "args": {"doc": 1, "open_password": "PolymathQAOnly!"}}, {"tool": "doc_save", "args": {"doc": 1, "path": str(protected), "full": True}}], protected)
    assert subprocess.run([str(CLI), "info", str(protected), "--password", "PolymathQAOnly!"], capture_output=True, timeout=30).returncode == 0
    assert subprocess.run([str(CLI), "info", str(protected)], capture_output=True, timeout=30).returncode != 0
    pdfa = QA / "pdfa.pdf"
    scripted("pdfa", [opened(), {"tool": "pdfa_convert", "args": {"doc": 1, "level": "2b"}}, {"tool": "doc_save", "args": {"doc": 1, "path": str(pdfa), "full": True}}], pdfa)
    for path in (optimized, pdfa):
        assert subprocess.run([str(CLI), "info", str(path)], capture_output=True, timeout=30).returncode == 0
    if image is not None:
        scan = copy_sample(image, "pdf-ocr-image")
        ocr = QA / "pdf-ocr.pdf"
        scripted("ocr", [{"tool": "doc_create", "args": {"from": "images", "paths": [str(scan)]}}, {"tool": "ocr_recognize", "args": {"doc": 1, "pages": [1], "dpi": 72, "skip_text_pages": False}}, {"tool": "doc_save", "args": {"doc": 1, "path": str(ocr)}}], ocr)
        assert subprocess.run([str(CLI), "info", str(ocr)], capture_output=True, timeout=30).returncode == 0
    assert digest(first) == first_before and digest(second) == second_before
    return created


def main() -> int:
    selected = samples()
    report = {"source": "Downloads (copies only)", "cases": {}}
    for ext in (".pdf", ".docx", ".md", ".xlsx", ".csv", ".pptx", ".png", ".jpg", ".webp"):
        if ext not in selected:
            report["cases"][ext] = {"status": "no valid sample"}
            continue
        try:
            result = image_case(ext, selected[ext]) if ext in {".png", ".jpg", ".webp"} else editor_case(ext, selected[ext])
            report["cases"][ext] = {"status": "passed", **result}
        except Exception as error:
            report["cases"][ext] = {"status": "failed", "error": str(error)[:300]}
        print(f"{ext}: {report['cases'][ext]['status']}", flush=True)
    listing = subprocess.run(["rg", "--files", "-g", "*.pdf", str(DOWNLOADS)], capture_output=True, text=True, check=True).stdout.splitlines()
    pdfs = [p for p in sorted((Path(name) for name in listing), key=lambda p: p.stat().st_size) if valid(p)]
    if len(pdfs) >= 2:
        try:
            report["pdf_tools"] = {"status": "passed", **pdf_tools(pdfs[0], pdfs[1], selected.get(".png"))}
        except Exception as error:
            report["pdf_tools"] = {"status": "failed", "error": str(error)[:300]}
        print(f"pdf_tools: {report['pdf_tools']['status']}", flush=True)
    (QA / "report.json").write_text(json.dumps(report, indent=2))
    return 0 if all(value["status"] == "passed" for value in report["cases"].values()) and report.get("pdf_tools", {}).get("status") == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
