#!/usr/bin/env python3
"""Measure the real native Cargo fingerprint, with compiled ABI edit/restore proof.

Uses the host lock/target/profile and a logging CC at a path containing spaces.
Restores edited source and host build environment before returning. This is a
bounded local measurement, not a replacement for independent regression tests.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / 'cockpit/native/skill_router.c'


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--cargo', default=shutil.which('cargo'))
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open('x') as stream:
        stream.write('{}\n')
    original = SOURCE.read_bytes()
    needle = b'angel_skill_route_abi_version(void) { return 1; }'
    if original.count(needle) != 1:
        raise RuntimeError('native semantic probe location must be unique')
    changed = original.replace(needle, needle.replace(b'return 1', b'return 7'))
    env = dict(os.environ)
    # A compiler wrapper is deliberately used for independent call counting;
    # do not let an inherited target-specific CC bypass it.
    for key in ('CC', 'TARGET_CC', 'CC_x86_64_unknown_linux_gnu',
                'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER'):
        env.pop(key, None)
    env['RUSTUP_AUTO_INSTALL'] = '0'
    env['ANGEL_BIND_SOURCE'] = '0'
    report = {'schema': 'angel.native-fingerprint/v1',
              'scope': __doc__, 'measurement_version': 2, 'probe_sha256': sha(Path(__file__)),
              'native_script_sha256': sha(ROOT / 'cockpit/native/build.rs'),
              'steps': []}

    def save():
        args.output.write_text(json.dumps(report, indent=2) + '\n')

    with tempfile.TemporaryDirectory(prefix='angel-native-fingerprint-') as folder:
        work = Path(folder)
        calls = work / 'cc.jsonl'
        compiler = shutil.which('cc')
        wrapper = work / 'compiler with spaces'
        wrapper.write_text('#!/usr/bin/env python3\nimport json,os,sys\n'
                           f'fd=os.open({str(calls)!r},os.O_WRONLY|os.O_CREAT|os.O_APPEND,0o600)\n'
                           'os.write(fd,(json.dumps(sys.argv[1:])+"\\n").encode());os.close(fd)\n'
                           f'os.execv({compiler!r},[{compiler!r},*sys.argv[1:]])\n')
        wrapper.chmod(0o700)
        measured_env = dict(env, CC=str(wrapper))
        abi = work / 'abi.c'
        abi.write_text('#include <stdio.h>\n#include <stdint.h>\n'
                       'uint32_t angel_skill_route_abi_version(void);\n'
                       'uint32_t angel_activity_classifier_abi_version(void);\n'
                       'uint32_t angel_verify_shell_abi_version(void);\n'
                       'uint32_t angel_moa_route_abi_version(void);\n'
                       'int main(void) {printf("%u %u %u %u\\n",'
                       'angel_skill_route_abi_version(),angel_activity_classifier_abi_version(),'
                       'angel_verify_shell_abi_version(),angel_moa_route_abi_version());}\n')
        command = [args.cargo, 'build', '--manifest-path', str(ROOT / 'cockpit/Cargo.toml'),
                   '--locked', '--offline', '--profile', 'fast', '--jobs', '1',
                   '--features', 'glass']

        selected_archive = [None]

        def run(name, expected_calls, *, host=False, overrides=None, expected_abi='1 1 1 1', bare=False):
            calls.write_text('')
            current = dict(env if bare else measured_env)
            current.update(overrides or {})
            cmd = command if host else command[:-2] + ['-p', 'angel-native']
            log = args.output.with_name(args.output.stem + '-' + name + '.log')
            start = time.perf_counter()
            with log.open('x') as stream:
                result = subprocess.run(cmd, cwd=ROOT, env=current, stdout=stream,
                                        stderr=subprocess.STDOUT, timeout=300)
            rows = [json.loads(line) for line in calls.read_text().splitlines()]
            all_rows = rows
            native_sources = {str(ROOT / 'cockpit/native' / f'{stem}.c') for stem in
                              ['skill_router', 'activity_classifier', 'verify_shell', 'moa_router']}
            rows = [argv for argv in all_rows if native_sources.intersection(argv)]
            if rows:
                argv = rows[0]
                selected_archive[0] = Path(argv[argv.index('-o') + 1]).parent / 'libangel_native_modules.a'
            archive = selected_archive[0]
            if archive is None or not archive.is_file():
                raise RuntimeError('no archive from the measured native compiler invocations')
            row = {'name': name, 'seconds': time.perf_counter() - start,
                   'exit_code': result.returncode, 'compiler_calls': len(rows),
                   'expected_calls': expected_calls, 'compiler_argv': rows, 'all_compiler_argv': all_rows,
                   'source_sha256': sha(SOURCE), 'log': log.name,
                   'archive': str(archive.relative_to(ROOT)), 'archive_sha256': sha(archive),
                   'object_mtime_ns': {path.name: path.stat().st_mtime_ns for path in archive.parent.glob('*.o')}}
            report['steps'].append(row)
            save()
            if result.returncode or len(rows) != expected_calls:
                raise RuntimeError(f'{name}: build/count failed, see {log}')
            executable = work / 'abi'
            subprocess.run([compiler, str(abi), str(archive), '-lm', '-o', str(executable)], check=True)
            output = subprocess.check_output([str(executable)], text=True).strip()
            row.update(abi_stdout=output, expected_abi=expected_abi)
            if output != expected_abi:
                save()
                raise RuntimeError(f'{name}: linked native ABI is stale: {output!r}')
            if host:
                binary = ROOT / 'cockpit/target/fast/angel'
                row['host_stdout'] = subprocess.check_output([str(binary), '--version'], env=current, text=True).strip()
                row['host_sha256'] = sha(binary)
                info = json.loads(subprocess.check_output([str(binary), '--build-info', '--json'], env=current, text=True))
                row['host_abi_versions'] = info['builtin_native_abi_versions']
                save()
                if row['host_abi_versions'] != [int(n) for n in expected_abi.split()]:
                    raise RuntimeError('native edit did not propagate into the actual host')
            save()
            print(name, round(row['seconds'], 4), 'cc', len(rows), 'ABI', output, flush=True)

        try:
            run('prime-host', 4, host=True)
            run('unchanged-host', 0, host=True)
            # The same default cartridge root, but a changed watched environment
            # value. The host script must rerun; the native package must not.
            roots = env.get('ANGEL_CARTRIDGES', str(Path(env['HOME']) / '.angelX/cartridges'))
            roots += os.pathsep + str(work / 'nonexistent-cartridge-root')
            run('cartridge-env-host', 0, host=True, overrides={'ANGEL_CARTRIDGES': roots})
            if SOURCE.read_bytes() != original:
                raise RuntimeError('source changed concurrently before semantic probe')
            SOURCE.write_bytes(changed)
            run('native-edit', 4, expected_abi='7 1 1 1', host=True)
            SOURCE.write_bytes(original)
            run('native-restore', 4, host=True)
            run('native-cflags', 4, overrides={'CFLAGS': env.get('CFLAGS', '') + ' -DANGEL_NATIVE_INVALIDATION_PROBE=1'})
        finally:
            if SOURCE.read_bytes() == changed:
                SOURCE.write_bytes(original)
            elif SOURCE.read_bytes() != original:
                raise RuntimeError('concurrent native edit: refusing to overwrite it')
            # Restore default CC/CFLAGS and cartridge environment on the real
            # host. This intentionally compiles native once, without our logger.
            run('restored-host', 0, host=True, bare=True)
        report['all_expectations_met'] = True
        save()


if __name__ == '__main__':
    main()
