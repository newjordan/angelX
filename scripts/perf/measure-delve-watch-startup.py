#!/usr/bin/env python3
"""Controlled candidate/baseline/baseline/candidate source-launch watch measurement.

Temporarily uses the two HEAD build scripts, saving the exact current bytes first.
Each transition refuses to overwrite concurrent edits. A finally block restores
the exact candidate bytes and compiles them. No other file, cache or feature changes.
Run without any concurrent source editor/build. All receipts are exclusive.
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
SOURCES = [ROOT / 'crates/delve/build.rs', ROOT / 'crates/delve/arcade_gate.rs']


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    with output.open('x') as file:
        file.write('{}\n')
    candidate = {path: path.read_bytes() for path in SOURCES}
    baseline = {path: subprocess.check_output(['git', 'show', 'HEAD:' + str(path.relative_to(ROOT))], cwd=ROOT) for path in SOURCES}
    env = dict(os.environ)
    for name in ('CARGO_LOG', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER',
                 'CARGO_TARGET_DIR', 'ANGEL_NO_BUILD', 'ANGEL_LAUNCH_BINARY', 'ANGEL_LAUNCH_BINARY_SHA256'):
        env.pop(name, None)
    env.update(ANGEL_BUILD_BACKEND='cargo', ANGEL_BIND_SOURCE='0')
    report = {'schema': 'angel.delve-watch-startup/v1', 'scope': 'warm full-feature source-launch --version; not UI-ready timing',
              'source_hashes': {str(path.relative_to(ROOT)): {'baseline': sha(baseline[path]), 'candidate': sha(candidate[path])} for path in SOURCES},
              'phases': [], 'restored': False}
    current = candidate

    def save():
        output.write_text(json.dumps(report, indent=2) + '\n')

    def select(target):
        nonlocal current
        if any(path.read_bytes() != current[path] for path in SOURCES):
            raise RuntimeError('concurrent source edit; refusing overwrite')
        for path in SOURCES:
            if target[path] != current[path]:
                path.write_bytes(target[path])
        current = target

    def run(label):
        log = output.with_name(output.stem + '-' + label + '.log')
        started = time.perf_counter()
        with log.open('x') as file:
            child = subprocess.Popen(['bin/angelX', '--version'], cwd=ROOT, env=env,
                                     stdout=subprocess.PIPE, stderr=file, start_new_session=True)
            try:
                stdout, _ = child.communicate(timeout=300)
            except BaseException:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
                raise
        row = {'seconds': time.perf_counter() - started, 'exit': child.returncode, 'stdout': stdout.decode(), 'log': log.name}
        if child.returncode != 0 or stdout != b'angelX-cockpit 0.2.6\n':
            raise RuntimeError('source launch failed: ' + str(row))
        return row

    try:
        for index, (name, target) in enumerate([('candidate', candidate), ('baseline', baseline), ('baseline', baseline), ('candidate', candidate)]):
            select(target)
            phase = {'index': index, 'name': name, 'prime': run(f'{index}-prime'), 'samples': []}
            report['phases'].append(phase)
            for sample in range(8):
                phase['samples'].append(run(f'{index}-{sample}'))
                save()
            print(index, name, [round(s['seconds'] * 1000, 2) for s in phase['samples']], flush=True)
    finally:
        select(candidate)
        report['final_launch'] = run('final-restored')
        report['restored'] = all(path.read_bytes() == candidate[path] for path in SOURCES)
        save()
    assert report['restored']
    print('DELVE_WATCH_STARTUP_COMPARISON_RESTORED', flush=True)


if __name__ == '__main__':
    main()
