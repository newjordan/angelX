#!/usr/bin/env python3
"""Compare exact standalone native reload builders, using real production C sources.

Runs isolated output roots, not Cargo. No live providers or changes to application
sources. Alternates baseline/candidate, executes all four ABI exports, and checks
library hash parity. An isolated module benchmark is not application startup.
"""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import shutil
import statistics
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
MODULES = {'skill_router': 'angel_skill_route_abi_version',
           'activity_classifier': 'angel_activity_classifier_abi_version',
           'verify_shell': 'angel_verify_shell_abi_version',
           'moa_router': 'angel_moa_route_abi_version'}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--baseline', required=True, type=Path)
    parser.add_argument('--pairs', type=int, default=7, choices=range(1, 16))
    args = parser.parse_args()
    candidate = ROOT / 'scripts/runtime/build-native-modules.py'
    baseline = args.baseline.resolve()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open('x') as stream:
        stream.write('{}\n')
    env = dict(os.environ, CC=shutil.which('cc'), CFLAGS='', ANGEL_NATIVE_BUILD_JOBS='4')
    for key in ('CARGO_MAKEFLAGS', 'MAKEFLAGS', 'MFLAGS', 'ANGEL_NO_BUILD'):
        env.pop(key, None)
    report = {'schema': 'angel.native-reload-bench/v1', 'scope': __doc__,
              'builders': {str(p): sha(p) for p in (baseline, candidate)},
              'sources': {name: sha(ROOT / 'cockpit/native' / (name + '.c')) for name in MODULES},
              'compiler': subprocess.check_output([env['CC'], '--version'], text=True).splitlines()[0],
              'samples': []}

    def save():
        args.output.write_text(json.dumps(report, indent=2) + '\n')

    expected = None
    with tempfile.TemporaryDirectory(prefix='angel-native-reload-bench-') as folder:
        work = Path(folder)
        for pair in range(args.pairs):
            order = [('baseline', baseline), ('candidate', candidate)]
            if pair % 2:
                order.reverse()
            for mode, builder in order:
                root = work / f'{pair}-{mode}'
                source = root / 'cockpit/native'
                source.mkdir(parents=True)
                for name in MODULES:
                    shutil.copyfile(ROOT / 'cockpit/native' / (name + '.c'), source / (name + '.c'))
                start = time.perf_counter()
                result = subprocess.run([sys.executable, str(builder), str(root)], env=env,
                                        capture_output=True, text=True, timeout=60)
                row = dict(pair=pair, mode=mode, seconds=time.perf_counter() - start,
                           exit_code=result.returncode, stderr=result.stderr)
                report['samples'].append(row)
                save()
                if result.returncode:
                    raise RuntimeError('compile failed; receipt retained')
                images = {name: root / 'cockpit/target/native' / (name + '.so') for name in MODULES}
                row['library_sha256'] = {name: sha(path) for name, path in images.items()}
                row['executed_abi'] = {name: getattr(ctypes.CDLL(str(path)), MODULES[name])()
                                       for name, path in images.items()}
                expected = expected or row['library_sha256']
                row['byte_parity'] = row['library_sha256'] == expected
                # Verify exact cache reuse separately; it is not a compile sample.
                before = {name: (path.stat().st_ino, path.stat().st_mtime_ns) for name, path in images.items()}
                start = time.perf_counter()
                reuse = subprocess.run([sys.executable, str(builder), str(root)], env=env,
                                       capture_output=True, text=True, timeout=60)
                row['reuse_seconds'] = time.perf_counter() - start
                row['reuse_exit_code'] = reuse.returncode
                row['reuse_preserved'] = before == {name: (path.stat().st_ino, path.stat().st_mtime_ns) for name, path in images.items()}
                save()
                if not (row['byte_parity'] and all(v == 1 for v in row['executed_abi'].values())
                        and reuse.returncode == 0 and row['reuse_preserved']):
                    raise RuntimeError('ABI/byte/cache verification failed; receipt retained')
                print(pair, mode, round(row['seconds'], 4), 'ABI', row['executed_abi'], flush=True)
    report['median_seconds'] = {mode: statistics.median(r['seconds'] for r in report['samples'] if r['mode'] == mode)
                                for mode in ('baseline', 'candidate')}
    report['median_reuse_seconds'] = {mode: statistics.median(r['reuse_seconds'] for r in report['samples'] if r['mode'] == mode)
                                      for mode in ('baseline', 'candidate')}
    report['ratio_of_medians'] = report['median_seconds']['baseline'] / report['median_seconds']['candidate']
    save()
    print(json.dumps({key: report[key] for key in ('median_seconds', 'median_reuse_seconds', 'ratio_of_medians')}))


if __name__ == '__main__':
    main()
