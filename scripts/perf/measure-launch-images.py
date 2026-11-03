#!/usr/bin/env python3
"""Compare two existing trusted cockpit images, alternating order (no model calls).

Measures warm process startup only: --version and rejected sandbox entry, not
complete sandboxed tool execution, terminal readiness, compilation or cold I/O.
Each image must produce identical version output and reject a missing policy.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import time


def sha256(path):
    digest = hashlib.sha256()
    with path.open('rb') as file:
        for block in iter(lambda: file.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--runs', type=int, default=40)
    args = parser.parse_args()
    if not 1 <= args.runs <= 200:
        parser.error('--runs must be between 1 and 200')
    paths = {key: path.resolve(strict=True) for key, path in
             [('baseline', args.baseline), ('candidate', args.candidate)]}
    env = dict(os.environ)
    for key in list(env):
        if key.startswith('ANGEL_INTERNAL_SANDBOX_'):
            env.pop(key)
    images = {key: {'path': str(path), 'bytes': path.stat().st_size,
                    'sha256': sha256(path)} for key, path in paths.items()}
    report = {'schema': 'angel.launch-images/v1', 'scope': __doc__,
              'probe_sha256': sha256(Path(__file__)), 'images': images,
              'samples': [], 'complete': False}
    # Reserve the result without clobbering an earlier experiment.
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open('x') as file:
        file.write('{}\n')
    version = None

    def run(name, case, warm=False):
        nonlocal version
        command = ['--version'] if case == 'version' else [
            '--sandbox-exec', '--', '/usr/bin/true']
        started = time.perf_counter()
        result = subprocess.run([str(paths[name]), *command], env=env,
                                capture_output=True, timeout=15)
        elapsed_ms = (time.perf_counter() - started) * 1000
        if case == 'version':
            if result.returncode != 0 or not result.stdout.startswith(b'angelX-cockpit '):
                raise RuntimeError(f'{name}: invalid version response (exit {result.returncode})')
            if version is None:
                version = result.stdout
            if version != result.stdout:
                raise RuntimeError('images report different versions')
        elif (result.returncode == 0 or result.stdout or
              b'missing sandbox helper policy' not in result.stderr):
            raise RuntimeError(f'{name}: sandbox entry did not reject missing policy')
        if not warm:
            report['samples'].append({'image': name, 'case': case,
                                      'ms': elapsed_ms, 'exit_code': result.returncode})

    try:
        for case in ('version', 'sandbox-rejection'):
            for name in paths:
                for _ in range(3):
                    run(name, case, warm=True)
            for index in range(args.runs):
                order = ('baseline', 'candidate') if index % 2 == 0 else ('candidate', 'baseline')
                for name in order:
                    run(name, case)
        # A changed target path invalidates the measurement, even if it ran.
        for name, path in paths.items():
            if sha256(path) != images[name]['sha256']:
                raise RuntimeError(f'{name}: image changed during measurement')
        report['medians_ms'] = {
            case: {name: statistics.median(row['ms'] for row in report['samples']
                                          if row['image'] == name and row['case'] == case)
                   for name in paths}
            for case in ('version', 'sandbox-rejection')}
        report['complete'] = True
        print(json.dumps(report['medians_ms']), flush=True)
    finally:
        args.output.write_text(json.dumps(report, indent=2) + '\n')


if __name__ == '__main__':
    main()
