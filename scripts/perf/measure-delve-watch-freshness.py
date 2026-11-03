#!/usr/bin/env python3
"""Verify real Delve Cargo invalidation after recursive watch minimization.

Temporarily changes an approved mapping and adds/removes a nested sprite slot.
Checks actual generated Rust rather than a synthetic watcher. No game/provider
runs. Run without a concurrent source editor. Receipts are exclusive; restoration
refuses to overwrite unexpected input changes. Owned children are killed/reaped.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
SLOTS = ROOT / 'crates/delve/assets/art-slots'
ADDED = SLOTS / 'dev-watch-probe'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    with output.open('x') as f:
        f.write('{}\n')
    if ADDED.exists() or ADDED.is_symlink():
        raise RuntimeError('probe slot already exists')
    source = None
    for path in sorted(SLOTS.glob('*/*.map.json')):
        mapping = json.loads(path.read_bytes())
        if mapping.get('runtime') is not True:
            continue
        for frame in mapping.get('pieces', []):
            png = path.parent / 'v1' / (frame['name'] + '.png')
            if frame.get('runtime') is True and frame.get('approved') is True and png.is_file():
                source, name, pixels = path, frame['name'], png.read_bytes()
                break
        if source:
            break
    if source is None:
        raise RuntimeError('no approved runtime piece')
    original = source.read_bytes()
    changed_map = json.loads(original)
    for frame in changed_map['pieces']:
        if frame['name'] == name:
            frame['approved'] = False
    changed = (json.dumps(changed_map, indent=2) + '\n').encode()
    slot = source.parent.name
    existing_entry = f'({json.dumps(slot)}, {json.dumps(name)}) => Some('
    added_entry = '("dev-watch-probe", "frame") => Some('
    added_map = json.dumps({'runtime': True, 'pieces': [{'name': 'frame', 'runtime': True, 'approved': True}]}).encode()
    tc = Path(os.environ.get('RUSTUP_HOME', str(Path.home() / '.rustup'))) / 'toolchains/1.95.0-x86_64-unknown-linux-gnu/bin'
    env = dict(os.environ, RUSTC=str(tc / 'rustc'), RUSTDOC=str(tc / 'rustdoc'))
    for key in ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'CARGO_TARGET_DIR'):
        env.pop(key, None)
    command = [str(tc / 'cargo'), 'build', '--locked', '--offline', '--profile', 'fast',
               '--manifest-path', str(ROOT / 'cockpit/Cargo.toml'), '-p', 'angel-delve', '-vv']
    report = {'schema': 'angel.delve-watch-freshness/v1', 'command': command, 'steps': [],
              'source': str(source.relative_to(ROOT)), 'original_sha256': sha(original),
              'probe_sha256': sha(Path(__file__).read_bytes())}

    def save():
        output.write_text(json.dumps(report, indent=2) + '\n')

    def run(label, existing, added):
        log = output.with_name(output.stem + '-' + label + '.log')
        started = time.perf_counter()
        with log.open('x') as file:
            child = subprocess.Popen(command, cwd=ROOT, env=env, stdout=file, stderr=subprocess.STDOUT, start_new_session=True)
            try:
                child.wait(timeout=180)
            except BaseException:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
                raise
        row = {'label': label, 'seconds': time.perf_counter() - started, 'exit': child.returncode, 'log': log.name}
        report['steps'].append(row)
        save()
        if child.returncode:
            raise RuntimeError('build failed: ' + str(log))
        generated = max((ROOT / 'cockpit/target/fast/build').glob('angel-delve-*/out/slot_art.rs'), key=lambda p: p.stat().st_mtime_ns)
        code = generated.read_text()
        row.update(generated_sha256=sha(code.encode()), generated_mtime_ns=generated.stat().st_mtime_ns,
                   existing_matches=(existing_entry in code) == existing, added_matches=(added_entry in code) == added)
        save()
        assert row['existing_matches'] and row['added_matches'], row
        print(row, flush=True)
        return row

    owned = False
    def remove_added():
        nonlocal owned
        if owned:
            expected = {ADDED / 'dev-watch-probe.map.json': added_map, ADDED / 'v1/frame.png': pixels}
            actual = {p for p in ADDED.rglob('*') if not p.is_dir()}
            if actual != set(expected) or any(p.is_symlink() or p.read_bytes() != data for p, data in expected.items()):
                raise RuntimeError('concurrent probe edit; refusing removal')
            for p in expected:
                p.unlink()
            (ADDED / 'v1').rmdir()
            ADDED.rmdir()
            owned = False

    save()
    try:
        baseline = run('prime', True, False)
        generated = max((ROOT / 'cockpit/target/fast/build').glob('angel-delve-*/out/slot_art.rs'), key=lambda p: p.stat().st_mtime_ns)
        watches = [l.split('=', 1)[1] for l in (generated.parent.parent / 'output').read_text().splitlines() if l.startswith('cargo:rerun-if-changed=')]
        report.update(root_watched='assets/art-slots' in watches, map_explicit_watch_removed=str(source.relative_to(ROOT / 'crates/delve')) not in watches)
        assert report['root_watched'] and report['map_explicit_watch_removed']
        if source.read_bytes() != original:
            raise RuntimeError('concurrent source edit')
        source.write_bytes(changed)
        edited = run('covered-map-edit', False, False)
        assert edited['generated_sha256'] != baseline['generated_sha256']
        ADDED.mkdir()
        owned = True
        (ADDED / 'v1').mkdir()
        (ADDED / 'dev-watch-probe.map.json').write_bytes(added_map)
        (ADDED / 'v1/frame.png').write_bytes(pixels)
        run('nested-slot-added', False, True)
        remove_added()
        run('nested-slot-removed', False, False)
    finally:
        if source.read_bytes() == changed:
            source.write_bytes(original)
        elif source.read_bytes() != original:
            raise RuntimeError('concurrent source edit; refusing restoration')
        remove_added()
        restored = run('restore', True, False)
        report.update(source_restored=source.read_bytes() == original, added_slot_removed=not ADDED.exists(),
                      generated_restored=restored['generated_sha256'] == report['steps'][0].get('generated_sha256'))
        save()
    assert report['source_restored'] and report['added_slot_removed'] and report['generated_restored']
    print('DELVE_COVERED_MAP_EDIT_ADD_REMOVE_RESTORE_VERIFIED', flush=True)


if __name__ == '__main__':
    main()
