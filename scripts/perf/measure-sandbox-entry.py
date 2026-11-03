#!/usr/bin/env python3
"""Bounded alternating sandbox-entry startup probe; no target program executes.

Both entrypoints must reject the missing internal policy before exec. This
measures binary load/early dispatch only, not confinement or complete tool time.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--runs', type=int, default=40)
    args = parser.parse_args()
    assert 1 <= args.runs <= 100
    args.output.parent.mkdir(parents=True, exist_ok=True)
    paths = {name: ROOT / 'cockpit/target/fast' / file for name, file in [('cockpit', 'angel'), ('helper', 'angel-sandbox')]}
    env = dict(os.environ)
    for key in list(env):
        if key.startswith('ANGEL_INTERNAL_SANDBOX_'):
            env.pop(key)
    report = {'schema': 'angel.helper-entry-rejection/v1', 'scope': __doc__, 'probe_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'images': {name: {'path': str(path.relative_to(ROOT)), 'bytes': path.stat().st_size,
                               'sha256': hashlib.sha256(path.read_bytes()).hexdigest()} for name, path in paths.items()}, 'samples': []}
    with args.output.open('x') as file:
        file.write('{}\n')
    def run(name, warm):
        start = time.perf_counter()
        proc = subprocess.run([str(paths[name]), '--sandbox-exec', '--', '/usr/bin/true'], env=env,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10)
        elapsed = (time.perf_counter() - start) * 1000
        assert proc.returncode != 0 and b'missing sandbox helper policy' in proc.stderr, (proc.returncode, proc.stderr[-1000:])
        assert not proc.stdout, proc.stdout
        if not warm:
            report['samples'].append({'entry': name, 'ms': elapsed, 'exit_code': proc.returncode})
    try:
        for name in paths:
            for _ in range(3): run(name, True)
        for index in range(args.runs):
            for name in (['cockpit', 'helper'] if index % 2 == 0 else ['helper', 'cockpit']): run(name, False)
        report['medians_ms'] = {name: statistics.median(row['ms'] for row in report['samples'] if row['entry'] == name) for name in paths}
        print(json.dumps(report['medians_ms']), flush=True)
    finally:
        args.output.write_text(json.dumps(report, indent=2) + '\n')

if __name__ == '__main__':
    main()
