#!/usr/bin/env python3
"""Bounded real Cargo browser-input and cartridge-isolation measurement.

Edits one browser boss text, checks packed WASM, and restores/rebuilds in finally.
No provider calls. Output is exclusive; keep prior failed samples.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import time
import zlib

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / 'cockpit/assets/dungeon/bosses/grinwick.boss'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    with output.open('x') as file:
        file.write('{}\n')
    tc = Path(os.environ.get('RUSTUP_HOME', str(Path.home() / '.rustup'))) / 'toolchains/1.95.0-x86_64-unknown-linux-gnu/bin'
    env = dict(os.environ)
    for key in ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER',
                'ANGEL_DEV_SKIP_WEB_BUNDLE', 'CARGO_TARGET_DIR'):
        env.pop(key, None)
    env.update(RUSTC=str(tc / 'rustc'), RUSTDOC=str(tc / 'rustdoc'))
    command = [str(tc / 'cargo'), 'build', '--manifest-path', str(ROOT / 'cockpit/Cargo.toml'),
               '--locked', '--profile', 'fast', '-p', 'angel-web-bundle', '-vv']
    report = {'schema': 'angel.browser-invalidation/v1', 'steps': [], 'scope': 'actual nested WASM and packed bundle; no app/model turns'}

    def save():
        output.write_text(json.dumps(report, indent=2) + '\n')

    def run(label, overrides=None):
        log = output.with_name(output.stem + '-' + label + '.log')
        started = time.perf_counter()
        with log.open('x') as file:
            child = subprocess.Popen(command, cwd=ROOT, env=dict(env, **(overrides or {})),
                                     stdout=file, stderr=subprocess.STDOUT, start_new_session=True)
            try:
                child.wait(timeout=300)
            except BaseException:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
                raise
        row = {'label': label, 'seconds': time.perf_counter() - started,
               'exit_code': child.returncode, 'log': log.name}
        report['steps'].append(row)
        save()
        print(row, flush=True)
        if child.returncode:
            raise RuntimeError('build failed: ' + str(log))

    def bundle():
        generated = max((ROOT / 'cockpit/target/fast/build').glob('angel-web-bundle-*/out/delve_web.rs'),
                        key=lambda path: path.stat().st_mtime_ns)
        code = generated.read_text()
        packed = re.search(r'\("delve.wasm", include_bytes!\("([^"]+)"\)', code)
        if not packed:
            raise RuntimeError('normal bundle has no WASM')
        path = Path(packed[1])
        stored = path.read_bytes()
        wasm = zlib.decompress(stored) if path.suffix == '.zlib' else stored
        return {'generated_mtime_ns': generated.stat().st_mtime_ns,
                'wasm_sha256': hashlib.sha256(wasm).hexdigest()}, wasm

    original = SOURCE.read_bytes()
    needle = b'The timetable says never. The whistle says now.'
    marker = b'BROWSER_FRESHNESS_PROBE_20261028'
    if original.count(needle) != 1:
        raise RuntimeError('asset edit must be unique')
    changed = original.replace(needle, marker)
    try:
        run('prime')
        baseline, wasm = bundle()
        assert marker not in wasm
        report['baseline'] = baseline
        run('cartridge-environment', {'ANGEL_CARTRIDGES': str(ROOT / 'not-a-cartridge')})
        isolated, _ = bundle()
        report['cartridge_environment_did_not_rebuild_browser'] = isolated == baseline
        assert isolated == baseline
        if SOURCE.read_bytes() != original:
            raise RuntimeError('concurrent asset edit; refusing overwrite')
        SOURCE.write_bytes(changed)
        run('asset-edit')
        edited, wasm = bundle()
        report['asset_edit'] = edited
        report['edited_marker_in_packed_wasm'] = marker in wasm
        assert marker in wasm and edited['wasm_sha256'] != baseline['wasm_sha256']
    finally:
        if SOURCE.read_bytes() == changed:
            SOURCE.write_bytes(original)
        elif SOURCE.read_bytes() != original:
            raise RuntimeError('concurrent asset edit; refusing restoration')
        run('restore')
        restored, wasm = bundle()
        report['restored'] = restored
        report['source_restored'] = SOURCE.read_bytes() == original
        report['restored_wasm_verified'] = marker not in wasm
        save()
    assert report['source_restored'] and report['restored_wasm_verified']
    print('BROWSER_ASSET_EDIT_AND_RESTORE_VERIFIED', flush=True)


if __name__ == '__main__':
    main()
