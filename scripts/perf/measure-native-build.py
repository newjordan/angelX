#!/usr/bin/env python3
"""Paired native-build benchmark of the production bounded compile runner.

Owns at most four compiler processes, no Cargo build, network, or model calls.
Uses the exact native source/flags in cockpit/native/build.rs, checks object byte parity
and retains sample order, hashes, compiler identity and failures. A warm native
compile microbenchmark is not a cold start or end-to-end application measurement.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import statistics
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
SOURCES = [ROOT / 'cockpit/native' / (name + '.c') for name in
           ('skill_router', 'activity_classifier', 'verify_shell', 'moa_router')]
FLAGS = ['-std=c11', '-O2', '-fPIC', '-fno-fast-math', '-ffp-contract=off', '-frounding-math', '-c']


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--pairs', type=int, default=7, choices=range(1, 21))
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open('x') as file:
        file.write('{}\n')
    # Resolve rustup outside the temporary cwd. No global toolchain mutation.
    rustc = subprocess.check_output(['rustup', 'which', 'rustc'], cwd=ROOT, text=True).strip()
    cc = shutil.which('cc')
    if not cc:
        raise RuntimeError('cc is required')
    report = {
        'schema': 'angel.native-build-bench/v1', 'scope': __doc__,
        'runner_sha256': digest(ROOT / 'cockpit/native/build_parallel.rs'),
        'build_script_sha256': digest(ROOT / 'cockpit/native/build.rs'),
        'sources': {str(path.relative_to(ROOT)): digest(path) for path in SOURCES},
        'flags': FLAGS, 'compiler': cc,
        'compiler_version': subprocess.check_output([cc, '--version'], text=True).splitlines()[0],
        'samples': [], 'all_object_hashes_match': True,
    }

    def save():
        args.output.write_text(json.dumps(report, indent=2) + '\n')

    save()
    with tempfile.TemporaryDirectory(prefix='angel-native-bench-') as temp:
        temp = Path(temp)
        driver = temp / 'driver.rs'
        driver.write_text('''#[path = %s] mod build_parallel;
use std::process::Command;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let slots: usize = args[1].parse().unwrap();
    let mut remaining = slots - 1;
    let commands = args[4..].iter().map(|source| {
        let mut cc = Command::new(&args[2]);
        cc.args(%s).arg(source).arg("-o");
        cc.arg(std::path::Path::new(&args[3]).join(std::path::Path::new(source).file_stem().unwrap()).with_extension("o"));
        cc
    }).collect();
    build_parallel::run_commands(commands, || {
        if remaining == 0 { None } else { remaining -= 1; Some(()) }
    }).unwrap();
}
''' % (json.dumps(str(ROOT / 'cockpit/native/build_parallel.rs')), '&' + json.dumps(FLAGS)))
        binary = temp / 'driver'
        subprocess.run([rustc, '--edition=2021', '-O', str(driver), '-o', str(binary)], check=True)
        # Includes a serial warmup, explicitly excluded from samples.
        warm = temp / 'warm'
        warm.mkdir()
        subprocess.run([str(binary), '1', cc, str(warm), *map(str, SOURCES)], check=True)
        expected = {path.name: digest(path) for path in warm.glob('*.o')}
        slots = min(4, os.cpu_count() or 1)
        report['parallel_slots'] = slots
        for pair in range(args.pairs):
            order = [('serial', 1), ('parallel', slots)]
            if pair % 2:
                order.reverse()
            for mode, count in order:
                out = temp / f'{pair}-{mode}'
                out.mkdir()
                start = time.perf_counter()
                process = subprocess.run([str(binary), str(count), cc, str(out), *map(str, SOURCES)],
                                         text=True, capture_output=True)
                seconds = time.perf_counter() - start
                hashes = {path.name: digest(path) for path in out.glob('*.o')}
                report['samples'].append(dict(pair=pair, mode=mode, seconds=seconds,
                                              exit_code=process.returncode, stderr=process.stderr,
                                              object_sha256=hashes))
                report['all_object_hashes_match'] &= hashes == expected
                save()
                print(pair, mode, round(seconds, 4), 'exit', process.returncode, flush=True)
                if process.returncode or hashes != expected:
                    raise RuntimeError('native compiler failed or object bytes diverged; receipt retained')
        report['median_seconds'] = {
            mode: statistics.median(row['seconds'] for row in report['samples'] if row['mode'] == mode)
            for mode in ('serial', 'parallel')}
        report['median_speedup'] = report['median_seconds']['serial'] / report['median_seconds']['parallel']
        save()
        print(json.dumps({key: report[key] for key in ('median_seconds', 'median_speedup', 'all_object_hashes_match')}))


if __name__ == '__main__':
    main()
