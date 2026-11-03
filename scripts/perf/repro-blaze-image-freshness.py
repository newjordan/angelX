#!/usr/bin/env python3
"""Offline, bounded wrapper edit/restore probe; never edits the application.

Runs tiny Cargo+Blaze fixture steps and executes every reported-success artifact.
Keeps build logs, hashes and inode/link counts.
A negative result on a tiny fixture does not clear the full-application failure.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
REV = 'b383b0bcf5da33816809b00a37e1c36a0e761efb'


def sha(path):
    with path.open('rb') as file:
        return hashlib.file_digest(file, 'sha256').hexdigest()


def run(command, cwd, env, timeout=120):
    started = time.perf_counter()
    process = subprocess.Popen(command, cwd=cwd, env=env, stdout=subprocess.PIPE,
                               stderr=subprocess.STDOUT, start_new_session=True)
    try:
        output, _ = process.communicate(timeout=timeout)
    except BaseException:
        os.killpg(process.pid, signal.SIGKILL)
        process.communicate()
        raise
    return process.returncode, output.decode(errors='replace'), time.perf_counter() - started


def parent_lock_probe(output, tool, cargo, rustc, installed):
    """Reduce V8's OUT_DIR/../../v8.fslock denial without V8 or downloads.

    Use direct Blaze so a successful rustc fallback cannot hide the denial.
    The three cases differ only in engine or the script's lock destination.
    """
    report = {'schema': 'angel.blaze-parent-lock-repro/v1', 'install': installed,
              'probe_sha256': sha(Path(__file__)), 'steps': [],
              'scope': 'offline stdlib-only Cargo script; no sandbox opt-out; direct Blaze, not wrapper'}

    def save():
        output.write_text(json.dumps(report, indent=2) + '\n')

    save()
    with tempfile.TemporaryDirectory(prefix='angel blaze parent-lock ') as directory:
        work = Path(directory)
        (work / 'src').mkdir()
        (work / 'src/main.rs').write_text('include!(concat!(env!("OUT_DIR"), "/probe.rs"));\nfn main() { println!("{MARKER}"); }\n')
        (work / 'Cargo.toml').write_text('[package]\nname="parent-lock-probe"\nversion="0.1.0"\nedition="2021"\n')
        template = r'''fn main() {
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let lock = LOCK_DIRECTORY.join("v8.fslock");
    std::fs::write(&lock, b"local probe only").unwrap();
    std::fs::write(out.join("probe.rs"), "pub const MARKER: &str = \"CASE_MARKER\";\n").unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}
'''
        env = dict(os.environ, RUSTC=rustc, RUSTDOC=str(Path(rustc).with_name('rustdoc')),
                   RUSTUP_TOOLCHAIN=installed['toolchain'], CARGO_TARGET_DIR=str(work / 'target'),
                   RA_JIT_HOME=str(work / 'daemon'), RA_JIT_CACHE=str(work / 'store'),
                   RUSTUP_AUTO_INSTALL='0', RA_JIT_REMOTE='off', RA_JIT_SANDBOX='on',
                   RA_JIT_SCRIPTS_BY_RUSTC='1', RA_JIT_PROC_MACRO_SERVER='0')
        for name in ('RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'RUSTFLAGS',
                     'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_TARGET', 'RA_JIT_HERMETIC'):
            env.pop(name, None)
        try:
            for label, engine, destination in [
                    ('cargo-parent', [cargo, 'build', '--offline'], 'out.parent().unwrap().parent().unwrap()'),
                    ('blaze-out-control', [str(tool / 'ra-jit'), 'cargo', 'build', '--offline'], 'out'),
                    ('blaze-parent', [str(tool / 'ra-jit'), 'cargo', 'build', '--offline'], 'out.parent().unwrap().parent().unwrap()')]:
                script = work / 'build.rs'
                script.write_text(template.replace('LOCK_DIRECTORY', destination).replace('CASE_MARKER', label))
                code, log, seconds = run(engine, work, env)
                log_path = output.with_name(output.stem + '-' + label + '.log')
                with log_path.open('x') as file:
                    file.write(log)
                row = {'name': label, 'command': engine, 'exit_code': code, 'seconds': seconds,
                       'script_sha256': sha(script), 'log': log_path.name,
                       'parent_lock_denied': code != 0 and 'v8.fslock' in log and
                           ('Read-only file system' in log or 'read-only' in log)}
                if code == 0:
                    binary = work / 'target/debug/parent-lock-probe'
                    row['binary_sha256'] = sha(binary)
                    executed, stdout, _ = run([str(binary)], work, env)
                    row.update(execution_exit_code=executed, stdout=stdout.strip(),
                               expected_stdout=label, fresh=executed == 0 and stdout.strip() == label)
                report['steps'].append(row)
                save()
                print(label, code, round(seconds, 3), flush=True)
        finally:
            code, log, _ = run([str(tool / 'ra-jit'), 'cargo', 'stop'], work, env, timeout=20)
            report.update(daemon_stop_exit_code=code, daemon_stop_output=log[-1000:])
            save()
    baseline, control, denied = report['steps']
    report['reproduced'] = baseline.get('fresh', False) and control.get('fresh', False) and denied['parent_lock_denied']
    save()
    if not report['reproduced']:
        raise SystemExit('PARENT_LOCK_REDUCTION_INCONCLUSIVE; inspect all case logs')
    print('PARENT_LOCK_DENIAL_REPRODUCED; sandbox remained enabled', flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    fixtures = parser.add_mutually_exclusive_group()
    fixtures.add_argument('--build-script', action='store_true',
                        help='also run a rustc-built script and include its generated output')
    fixtures.add_argument('--native-dependency', action='store_true',
                          help='edit a C static library in an independent Cargo path dependency')
    fixtures.add_argument('--embedded-dependency', choices=('static', 'promoted'),
                          help='edit generated include_bytes data across a path dependency')
    fixtures.add_argument('--feature-dependency', action='store_true',
                        help='toggle default feature unification with unchanged Rust sources')
    fixtures.add_argument('--callback-dependency', action='store_true',
                          help='edit a Rust callback bridge across an independent path dependency')
    fixtures.add_argument('--parent-lock', action='store_true',
                          help='reduce the V8-style parent-of-OUT_DIR lock denial; direct Blaze, no fallback')
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open('x') as file:
        file.write('{}\n')
    tool = ROOT / 'cockpit/target/tools/blaze' / REV
    installed = json.loads((tool / 'install.json').read_text())
    for name, expected in installed['sha256'].items():
        if sha(tool / name) != expected:
            raise RuntimeError(f'compiler identity mismatch: {name}')
    rustc = subprocess.check_output(['rustup', 'which', '--toolchain', installed['toolchain'], 'rustc'], text=True).strip()
    cargo = str(Path(rustc).with_name('cargo'))
    if args.parent_lock:
        parent_lock_probe(args.output, tool, cargo, rustc, installed)
        return
    report = {'schema': 'angel.blaze-image-repro/v1', 'install': installed,
              'probe_sha256': sha(Path(__file__)),
              'scope': __doc__, 'build_script': args.build_script,
                'native_dependency': args.native_dependency,
                'embedded_dependency': args.embedded_dependency,
                'feature_dependency': args.feature_dependency,
                'callback_dependency': args.callback_dependency,
              'steps': [], 'stale_artifact_observed': False}

    def save():
        args.output.write_text(json.dumps(report, indent=2) + '\n')

    save()
    with tempfile.TemporaryDirectory(prefix='angel-blaze-repro-') as directory:
        work = Path(directory)
        (work / 'src').mkdir()
        original = 'fn main() { println!("original"); }\n'
        if args.build_script:
            # Unlike the full host this needs no nested Cargo, native tools or
            # external crates. Isolate the wrapper's script/fallback surface.
            (work / 'build.rs').write_text(r'''fn main() {
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(out.join("suffix.rs"), "const SUFFIX: &str = \"script\";").unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}
''')
            original = 'include!(concat!(env!("OUT_DIR"), "/suffix.rs"));\nfn main() { println!("original-{SUFFIX}"); }\n'
        (work / 'Cargo.toml').write_text('[package]\nname="freshness-probe"\nversion="0.1.0"\nedition="2021"\n')
        source = work / 'src/main.rs'
        source.write_text(original)
        if args.native_dependency:
            # Rust source and build scripts remain unchanged after prime: only
            # a C input changes. Exercise Cargo link metadata across a real path
            # dependency, rather than including generated Rust in the host.
            native = work / 'native'
            native.mkdir()
            (native / 'Cargo.toml').write_text('[package]\nname="native-probe"\nversion="0.1.0"\nedition="2021"\nlinks="native_probe"\n[lib]\npath="lib.rs"\n')
            (native / 'lib.rs').write_text('extern "C" { fn native_value() -> i32; }\npub fn value() -> i32 { unsafe { native_value() } }\n')
            (native / 'build.rs').write_text(r'''use std::{env, path::PathBuf, process::Command};
fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    assert!(Command::new("cc").args(["-c", "value.c", "-o"]).arg(out.join("value.o")).status().unwrap().success());
    let archive = out.join("libnative_probe.a");
    let _ = std::fs::remove_file(&archive);
    assert!(Command::new("ar").arg("crs").arg(&archive).arg(out.join("value.o")).status().unwrap().success());
    println!("cargo:rerun-if-changed=value.c");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=native_probe");
}
''')
            (work / 'Cargo.toml').write_text('[package]\nname="freshness-probe"\nversion="0.1.0"\nedition="2021"\n[dependencies]\nnative-probe={path="native"}\n')
            source.write_text('fn main() { println!("native-{}", native_probe::value()); }\n')
            source = native / 'value.c'
            original = 'int native_value(void) { return 1; }\n'
            source.write_text(original)
        if args.embedded_dependency:
            # Match the web bundle's generated include_bytes path dependency
            # without nested Cargo, zlib, network, V8 or third-party crates.
            embedded = work / 'embedded'
            embedded.mkdir()
            (embedded / 'Cargo.toml').write_text('[package]\nname="embedded-probe"\nversion="0.1.0"\nedition="2021"\n[lib]\npath="lib.rs"\n')
            (embedded / 'lib.rs').write_text('include!(concat!(env!("OUT_DIR"), "/bundle.rs"));\n')
            body = ('static DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/asset.bin"));\n'
                    '#[inline(never)] pub fn embedded() -> &\'static [u8] { DATA }\n'
                    if args.embedded_dependency == 'static' else
                    '#[inline(never)] pub fn embedded() -> &\'static [u8] { include_bytes!(concat!(env!("OUT_DIR"), "/asset.bin")) }\n')
            (embedded / 'build.rs').write_text('fn main() {\n'
                'let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());\n'
                'std::fs::copy("asset.bin", out.join("asset.bin")).unwrap();\n'
                f'std::fs::write(out.join("bundle.rs"), {json.dumps(body)}).unwrap();\n'
                'println!("cargo:rerun-if-changed=asset.bin");\n'
                'println!("cargo:rerun-if-changed=build.rs");\n}\n')
            (work / 'Cargo.toml').write_text('[package]\nname="freshness-probe"\nversion="0.1.0"\nedition="2021"\n[dependencies]\nembedded-probe={path="embedded"}\n')
            source.write_text('fn main() { let bytes = embedded_probe::embedded();\n'
                'println!("{}-{}-{}", bytes.len(), bytes[0], bytes.iter().map(|b| *b as u64).sum::<u64>()); }\n')
            source = embedded / 'asset.bin'
            original = 'a' * (1 << 20)
            source.write_text(original)
        if args.feature_dependency:
            # Only the dependency manifest selection changes; Rust and API stay
            # fixed. Cache identity must still include the cfg/feature graph.
            dependency = work / 'features'
            dependency.mkdir()
            (dependency / 'Cargo.toml').write_text('[package]\nname="feature-probe"\nversion="0.1.0"\nedition="2021"\n'
                '[lib]\npath="lib.rs"\n[features]\ndefault=["capture"]\nplayback=[]\ncapture=["playback"]\n')
            (dependency / 'lib.rs').write_text("pub fn selection() -> &'static str {\n"
                '#[cfg(feature="capture")] { "capture" }\n'
                '#[cfg(not(feature="capture"))] { "playback" }\n}\n')
            source.write_text('fn main() { println!("{}", feature_probe::selection()); }\n')
            source = work / 'Cargo.toml'
            original = ('[package]\nname="freshness-probe"\nversion="0.1.0"\nedition="2021"\n'
                '[dependencies]\nfeature-probe={path="features",default-features=false,features=["playback"]}\n')
            source.write_text(original)
        if args.callback_dependency:
            # Reduce the V8 extraction's callback/Result/worker shape without
            # V8 or scripts. Edit the implementation, not the public API.
            dependency = work / 'callbacks'
            dependency.mkdir()
            (dependency / 'Cargo.toml').write_text('[package]\nname="callback-probe"\nversion="0.1.0"\nedition="2021"\n[lib]\npath="lib.rs"\n')
            (work / 'Cargo.toml').write_text('[package]\nname="freshness-probe"\nversion="0.1.0"\nedition="2021"\n[dependencies]\ncallback-probe={path="callbacks"}\n')
            source.write_text('fn main() { let factor = 2; let callback = |n| Ok(n * factor);\n'
                              'println!("callback-{}", callback_probe::run(&callback).unwrap()); }\n')
            source = dependency / 'lib.rs'
            original = ('pub fn run(invoke: &(dyn Fn(i32) -> Result<i32, String> + Sync)) -> Result<i32, String> {\n'
                        'std::thread::scope(|scope| scope.spawn(|| invoke(3)).join().unwrap()).map(|n| n + 1)\n}\n')
            source.write_text(original)
        env = dict(os.environ, RUSTC=rustc, RUSTUP_TOOLCHAIN=installed['toolchain'],
                   CARGO_TARGET_DIR=str(work / 'target'), RA_JIT_HOME=str(work / 'daemon'),
                   RA_JIT_CACHE=str(work / 'store'), RUSTUP_AUTO_INSTALL='0',
                   RA_JIT_REMOTE='off',
                   RUSTC_WORKSPACE_WRAPPER=str(tool / 'ra-jit-rustc'),
                   RA_JIT_SCRIPTS_BY_RUSTC='1', RA_JIT_PROC_MACRO_SERVER='0')
        for name in ('RUSTC_WRAPPER', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_TARGET'):
            env.pop(name, None)
        binary = work / 'target/debug/freshness-probe'
        command = [cargo, 'build', '--offline']
        try:
            suffix = '-script' if args.build_script else ''
            edited = original.replace('original', 'edited')
            baseline_output, edited_output = 'original' + suffix, 'edited' + suffix
            if args.native_dependency:
                edited = original.replace('return 1', 'return 7')
                baseline_output, edited_output = 'native-1', 'native-7'
            if args.embedded_dependency:
                edited = 'b' * len(original)
                baseline_output = f'{len(original)}-97-{97 * len(original)}'
                edited_output = f'{len(edited)}-98-{98 * len(edited)}'
            if args.feature_dependency:
                edited = original.replace('default-features=false', 'default-features=true')
                baseline_output, edited_output = 'playback', 'capture'
            if args.callback_dependency:
                edited = original.replace('n + 1', 'n + 7')
                baseline_output, edited_output = 'callback-7', 'callback-13'
            for step, text, expected in [('prime', original, baseline_output),
                    ('unchanged', original, baseline_output),
                    ('edit', edited, edited_output),
                    ('restore', original, baseline_output)]:
                # A true unchanged control must not rewrite/touch source.
                if source.read_text() != text:
                    source.write_text(text)
                code, log, seconds = run(command, work, env)
                log_path = args.output.with_name(args.output.stem + '-' + step + '.log')
                with log_path.open('x') as file:
                    file.write(log)
                row = {'name': step, 'exit_code': code, 'seconds': seconds,
                       'source_sha256': sha(source), 'log': log_path.name,
                       'expected_stdout': expected}
                if args.embedded_dependency:
                    row['metadata'] = [dict(name=path.name, bytes=path.stat().st_size, sha256=sha(path))
                        for path in (work / 'target/debug/deps').glob('libembedded_probe-*.rmeta')]
                report['steps'].append(row)
                if code:
                    save()
                    raise RuntimeError(f'{step}: compiler failed, not a freshness verdict')
                stat = binary.stat()
                row.update(binary_sha256=sha(binary), inode=stat.st_ino, links=stat.st_nlink)
                code, output, _ = run([str(binary)], work, env)
                row.update(execution_exit_code=code, stdout=output.strip())
                row['fresh'] = code == 0 and output.strip() == expected
                report['stale_artifact_observed'] |= not row['fresh']
                save()
                print(step, row['fresh'], round(seconds, 3), flush=True)
                # Allow the async wrapper settler to finish before the next edit.
                deadline = time.monotonic() + 20
                settling = work / 'daemon/settling'
                while settling.exists() and any(settling.iterdir()) and time.monotonic() < deadline:
                    time.sleep(.05)
        finally:
            code, output, _ = run([str(tool / 'ra-jit'), 'cargo', 'stop'], work, env, timeout=20)
            report['daemon_stop_exit_code'] = code
            report['daemon_stop_output'] = output[-1000:]
            save()
    if report['stale_artifact_observed']:
        raise SystemExit('STALE_ARTIFACT_OBSERVED')
    print('TINY_FIXTURE_FRESH; full application remains unqualified', flush=True)


if __name__ == '__main__':
    main()
