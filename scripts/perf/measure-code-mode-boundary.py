#!/usr/bin/env python3
"""Measure a real V8 implementation edit/test/restore across the crate boundary.

Uses an explicit immutable pre-extraction source snapshot and the installed
candidate; never git checkout/revert. Serial, offline, no provider calls. Every
successful sample must execute exactly one test and observe the implementation's
new error text. All swapped files are guarded and restored in finally. Do not
run while editing the named files. This measures targeted Rust tests, not startup
or a cockpit relink after a dependency edit.
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


def interrupted(signum, _frame):
    # proc_stop sends SIGTERM, not KeyboardInterrupt. Route it through the
    # subprocess cleanup and guarded source-restoration finally blocks.
    raise KeyboardInterrupt(f'interrupted by signal {signum}')

FILES = ('cockpit/Cargo.toml', 'cockpit/Cargo.lock',
         'cockpit/src/agent/code_mode.rs', 'tests/cockpit/app/code_mode__tests.rs')
RUNTIME = 'tests/cockpit/code_mode_runtime.rs'
CORE = 'cockpit/code-mode/src/lib.rs'
TEST = r'''
#[test]
fn v8_boundary_compile_marker() {
    let error = run("return ((( ;", &[], &mock, &parallel_all,
        Duration::from_secs(5), 128).unwrap_err();
    let expected = std::env::var("ANGEL_V8_BOUNDARY_EXPECT").unwrap();
    assert!(error.contains(&expected), "expected {expected:?}, got {error:?}");
}
'''
NEEDLE = b'"code_mode: compile error: {}"'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--baseline-sources', type=Path, required=True)
    parser.add_argument('--rounds', type=int, default=2)
    args = parser.parse_args()
    signal.signal(signal.SIGTERM, interrupted)
    if not 1 <= args.rounds <= 3:
        parser.error('--rounds must be 1..3')
    baseline = {name: (args.baseline_sources / name).read_bytes() for name in FILES}
    hashes = json.loads((args.baseline_sources / 'baseline-hashes.json').read_text())
    for name, data in baseline.items():
        if sha(data) != hashes[name]:
            raise RuntimeError(f'baseline snapshot mismatch: {name}')
    paths = (*FILES, RUNTIME, CORE)
    candidate = {name: (ROOT / name).read_bytes() for name in paths}
    owned = dict(candidate)
    report = {'schema': 'angel.code-mode-boundary/v1', 'scope': __doc__,
              'probe_sha256': sha(Path(__file__).read_bytes()),
              'baseline_sources': str(args.baseline_sources), 'baseline_hashes': hashes,
              'candidate_hashes': {name: sha(data) for name, data in candidate.items()},
              'samples': [], 'sources_restored': False}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open('x') as file:
        file.write(json.dumps(report, indent=2) + '\n')

    def save():
        args.output.write_text(json.dumps(report, indent=2) + '\n')

    def write(name, data):
        path = ROOT / name
        if path.read_bytes() != owned[name]:
            raise RuntimeError(f'concurrent editor; refusing to overwrite {name}')
        if data != owned[name]:
            path.write_bytes(data)
            owned[name] = data

    cargo = subprocess.check_output(['rustup', 'which', 'cargo'], text=True).strip()
    env = dict(os.environ, ANGEL_BUILD_RUSTC=subprocess.check_output(
        ['rustup', 'which', 'rustc'], text=True).strip(), ANGEL_BUILD_PROFILE='fast')
    for key in ('ANGEL_YOLO', 'ANGEL_YOLO_SMART', 'ANGEL_CODE_MODE_EFFECTS',
                'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER'):
        env.pop(key, None)

    def run(label, layout, expected):
        env['ANGEL_V8_BOUNDARY_EXPECT'] = expected
        command = [cargo, 'test', '--manifest-path', 'cockpit/Cargo.toml',
                   '--offline', '--locked', '--profile', 'fast', '-p',
                   'angelX-cockpit' if layout == 'baseline' else 'angel-code-mode',
                   '--bin' if layout == 'baseline' else '--lib']
        if layout == 'baseline':
            command.append('angel')
        command += ['v8_boundary_compile_marker', '--', '--test-threads=1']
        start = time.perf_counter()
        process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=subprocess.PIPE,
                                   stderr=subprocess.STDOUT, start_new_session=True)
        try:
            stdout, _ = process.communicate(timeout=180)
        except BaseException:
            os.killpg(process.pid, signal.SIGKILL)
            process.communicate()
            raise
        elapsed = time.perf_counter() - start
        log = args.output.with_name(args.output.stem + '-' + label + '.log')
        with log.open('xb') as file:
            file.write(stdout)
        passed = (process.returncode == 0 and b'v8_boundary_compile_marker ... ok' in stdout
                  and b'test result: ok. 1 passed; 0 failed;' in stdout)
        report['samples'].append({'name': label, 'layout': layout, 'seconds': elapsed,
                                  'command': command, 'exit_code': process.returncode,
                                  'expected_error_marker': expected, 'verified': passed,
                                  'implementation_sha256': sha(owned[FILES[2] if layout == 'baseline' else CORE]),
                                  'log': log.name})
        save()
        print(label, round(elapsed, 4), 'verified', passed, flush=True)
        if not passed:
            raise RuntimeError(f'{label}: no verified semantic test result; inspect {log}')

    try:
        for round_index in range(args.rounds):
            # Alternate order, but always prime before measured edit/restore.
            layouts = ('baseline', 'candidate') if round_index % 2 == 0 else ('candidate', 'baseline')
            for layout in layouts:
                selected = baseline if layout == 'baseline' else candidate
                for name in FILES:
                    write(name, selected[name])
                test_path = FILES[3] if layout == 'baseline' else RUNTIME
                test_original = baseline[test_path] if layout == 'baseline' else candidate[test_path]
                write(test_path, test_original + TEST.encode())
                source = FILES[2] if layout == 'baseline' else CORE
                original = baseline[source] if layout == 'baseline' else candidate[source]
                if original.count(NEEDLE) != 1:
                    raise RuntimeError('implementation edit must be unique')
                prefix = f'r{round_index}-{layout}'
                run(prefix + '-prime', layout, 'code_mode: compile error:')
                marker = f'code_mode: compile error: {prefix}-implementation-edit'
                write(source, original.replace(NEEDLE, json.dumps(marker + ' {}').encode()))
                try:
                    run(prefix + '-edit', layout, marker)
                finally:
                    write(source, original)
                run(prefix + '-restore', layout, 'code_mode: compile error:')
                write(test_path, test_original)
    finally:
        conflicts = []
        for name, data in candidate.items():
            try:
                write(name, data)
            except RuntimeError as error:
                conflicts.append(str(error))
        report['sources_restored'] = not conflicts and all(
            (ROOT / name).read_bytes() == data for name, data in candidate.items())
        report['restore_conflicts'] = conflicts
        save()
        if conflicts:
            raise RuntimeError('; '.join(conflicts))
    print('TARGETED_IMPLEMENTATION_EDIT_TEST_RESTORE_VERIFIED', flush=True)


if __name__ == '__main__':
    main()
