#!/usr/bin/env python3
"""Exercise Polymath's signed Video workspace with synthetic local media."""
from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import struct
import subprocess
import time

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / 'target' / 'video-editor-qa'
OUT.mkdir(parents=True, exist_ok=True)
spec = importlib.util.spec_from_file_location('bridge', ROOT / 'packaging' / 'agent_bridge.py')
assert spec and spec.loader
bridge = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge)


def ffmpeg(*args: str) -> bytes:
    result = subprocess.run(
        ['/opt/homebrew/bin/ffmpeg', '-hide_banner', '-loglevel', 'error', *args],
        capture_output=True, check=True, timeout=60,
    )
    return result.stdout


def probe(path: Path) -> dict:
    result = subprocess.run(
        ['/opt/homebrew/bin/ffprobe', '-v', 'error', '-show_streams', '-show_format', '-of', 'json', str(path)],
        capture_output=True, check=True, timeout=20,
    )
    return json.loads(result.stdout)


def rgb(path: Path, second: float) -> tuple[int, int, int]:
    pixel = ffmpeg('-ss', str(second), '-i', str(path), '-frames:v', '1', '-vf', 'scale=1:1',
                   '-pix_fmt', 'rgb24', '-f', 'rawvideo', 'pipe:1')
    assert len(pixel) == 3, len(pixel)
    return tuple(pixel)


def rms(path: Path, second: float) -> float:
    pcm = ffmpeg('-ss', str(second), '-i', str(path), '-t', '0.2', '-map', '0:a:0',
                 '-ac', '1', '-ar', '16000', '-f', 's16le', 'pipe:1')
    samples = struct.unpack('<' + 'h' * (len(pcm) // 2), pcm)
    return (sum(sample * sample for sample in samples) / len(samples)) ** .5


def main() -> None:
    binary = ROOT / 'dist' / 'Polymath.app' / 'Contents' / 'MacOS' / 'Polymath'
    assert binary.is_file(), 'Build and bundle Polymath before running this test'
    red = OUT / 'red-source.mp4'
    blue = OUT / 'blue-source.mp4'
    soundtrack = OUT / 'soundtrack.wav'
    output = OUT / 'ordered-trimmed-muted-small.mp4'
    mixed = OUT / 'ordered-trimmed-mixed.mp4'
    for path in (red, blue, soundtrack, output, mixed):
        path.unlink(missing_ok=True)
    for color, frequency, path in [('red', '220', red), ('blue', '330', blue)]:
        ffmpeg('-f', 'lavfi', '-i', f'color=c={color}:s=960x540:r=24',
               '-f', 'lavfi', '-i', f'sine=frequency={frequency}:sample_rate=48000',
               '-t', '2', '-c:v', 'libx264', '-preset', 'ultrafast', '-pix_fmt', 'yuv420p',
               '-c:a', 'aac', '-y', str(path))
    ffmpeg('-f', 'lavfi', '-i', 'sine=frequency=880:sample_rate=48000',
           '-t', '2', '-c:a', 'pcm_s16le', '-y', str(soundtrack))
    originals = {path: hashlib.sha256(path.read_bytes()).hexdigest()
                 for path in (red, blue, soundtrack)}

    endpoint = OUT / f'control-{os.getpid()}.json'
    endpoint.unlink(missing_ok=True)
    env = os.environ.copy()
    env.update(POLYMATH_NO_PREFS='1', PDFCRAFT_SYSTEM_FONTS='0')
    with (OUT / 'app.log').open('w') as log:
        app = subprocess.Popen([str(binary), '--control', str(endpoint)],
                               stdout=log, stderr=log, env=env)
        try:
            deadline = time.monotonic() + 60
            while not endpoint.is_file():
                assert app.poll() is None and time.monotonic() < deadline, 'Polymath did not start'
                time.sleep(.2)

            def call(method: str, params: dict | None = None):
                return bridge.request(endpoint, method, params)

            assert call('app.open', {'path': str(red)})['workspace'] == 'reader'
            assert call('app.open', {'path': str(red), 'edit': True})['workspace'] == 'video'
            assert call('app.video.add', {'path': str(blue)})['workspace'] == 'video'
            deadline = time.monotonic() + 30
            while True:
                state = call('app.video.inspect')
                if all(clip['ready'] for clip in state['clips']):
                    break
                assert time.monotonic() < deadline, state
                time.sleep(.2)
            assert [Path(clip['path']) for clip in state['clips']] == [red, blue]
            call('app.video.trim', {'index': 0, 'start': .25, 'end': 1.75})
            call('app.video.trim', {'index': 1, 'start': .5, 'end': 2.0})
            state = call('app.video.move', {'from': 1, 'to': 0})
            assert [Path(clip['path']) for clip in state['clips']] == [blue, red]
            assert abs(state['timelineSeconds'] - 3) < .02, state
            call('app.video.audio', {'path': str(soundtrack)})
            deadline = time.monotonic() + 20
            while not call('app.video.inspect')['audio']['ready']:
                assert time.monotonic() < deadline, call('app.video.inspect')
                time.sleep(.2)
            call('app.video.audio.options', {'start': .5, 'gain': .8})
            call('app.video.mute', {'original': True})
            call('app.video.quality', {'preset': 'small'})
            call('app.video.output', {'path': str(output)})
            assert call('app.video.inspect')['audioMode'] == 'strip original'
            time.sleep(.8)
            call('ui.screenshot', {'path': str(OUT / 'timeline.png')})
            call('app.video.export')
            deadline = time.monotonic() + 120
            while True:
                state = call('app.video.inspect')
                if state['phase'] == 'Export complete':
                    break
                assert state['phase'] != 'Export failed', state
                assert time.monotonic() < deadline, state
                time.sleep(.3)
            assert output.is_file(), state
            deadline = time.monotonic() + 5
            while str(output) not in call('app.inspect')['recent']:
                assert time.monotonic() < deadline, 'Export not added to Home Recent'
                time.sleep(.2)
            info = probe(output)
            assert abs(float(info['format']['duration']) - 3) < .15, info['format']
            video = next(stream for stream in info['streams'] if stream['codec_type'] == 'video')
            assert video['codec_name'] == 'h264'
            assert video['width'] <= 854 and video['height'] <= 854, video
            first, second = rgb(output, .4), rgb(output, 2.0)
            assert first[2] > first[0] + 80, first
            assert second[0] > second[2] + 80, second
            assert rms(output, .15) < 100, 'Original clip audio was not stripped'
            assert rms(output, .9) > 300, 'Added soundtrack did not play'
            assert all(hashlib.sha256(path.read_bytes()).hexdigest() == digest
                       for path, digest in originals.items()), 'A source file changed'
            call('app.video.mute', {'original': False})
            call('app.video.output', {'path': str(mixed)})
            call('app.video.export')
            deadline = time.monotonic() + 120
            while True:
                state = call('app.video.inspect')
                if state['phase'] == 'Export complete' and not state['exporting']:
                    break
                assert state['phase'] != 'Export failed', state
                assert time.monotonic() < deadline, state
                time.sleep(.3)
            assert mixed.is_file()
            assert abs(float(probe(mixed)['format']['duration']) - 3) < .15
            assert rms(mixed, .15) > 300, 'Original clip sound was not preserved in mix mode'
            assert all(hashlib.sha256(path.read_bytes()).hexdigest() == digest
                       for path, digest in originals.items()), 'A source file changed'
            print('PASS: trim, reorder, merge, strip or mix audio, delayed soundtrack, small MP4, Recent and unchanged sources')
        finally:
            if app.poll() is None:
                try:
                    bridge.request(endpoint, 'app.quit')
                    app.wait(timeout=8)
                except (OSError, RuntimeError, subprocess.TimeoutExpired):
                    app.terminate()
                    app.wait(timeout=8)


if __name__ == '__main__':
    main()
