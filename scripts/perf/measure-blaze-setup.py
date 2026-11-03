#!/usr/bin/env python3
"""Local-only repeat-setup measurements and actual Cargo config-isolation control."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import statistics
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
INSTALLER = ROOT / "scripts/runtime/install-blaze.py"


def sha(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    report = {"schema": "angel.blaze-setup-reuse/v1", "scope": __doc__,
              "probe_sha256": sha(Path(__file__)), "installer_sha256": sha(INSTALLER), "steps": []}
    with args.output.open("x") as source:
        source.write(json.dumps(report, indent=2) + "\n")

    def save():
        args.output.write_text(json.dumps(report, indent=2) + "\n")

    def run(label, command, env, cwd=ROOT, expected=0):
        started = time.perf_counter()
        result = subprocess.run(command, cwd=cwd, env=env, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=120)
        log = args.output.with_name(args.output.stem + "-" + label + ".log")
        with log.open("x") as file:
            file.write(result.stdout)
        row = {"name": label, "command": [str(a) for a in command],
               "seconds": time.perf_counter() - started, "exit_code": result.returncode,
               "expected_exit_code": expected, "log": log.name, "log_sha256": sha(log)}
        report["steps"].append(row)
        save()
        if result.returncode != expected:
            raise RuntimeError(f"{label}: expected {expected}, got {result.returncode}; see {log}")
        return row, result.stdout

    spec = importlib.util.spec_from_file_location("install_blaze", INSTALLER)
    installer = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(installer)
    version = installer.DESTINATION / installer.REVISION
    receipt = installer.validate_install(version)
    report["install_receipt"] = receipt
    selector = installer.DESTINATION / "selected"

    def identities():
        return {path.name: {"sha256": sha(path), "inode": path.stat().st_ino,
                            "mtime_ns": path.stat().st_mtime_ns, "size": path.stat().st_size}
                for path in [selector, version / "install.json", *(version / name for name in installer.IMAGES)]}

    report["before"] = identities()
    env = dict(os.environ, RUSTUP_AUTO_INSTALL="0", RUSTUP_DIST_SERVER="http://127.0.0.1:9",
               RUSTUP_UPDATE_ROOT="http://127.0.0.1:9", CARGO_NET_OFFLINE="true")
    # Reuse the actual installer logic but audit its child-command requests.
    # A regression asking for fetch/install/build fails before executing it.
    guard = '''import importlib.util,json,sys
s=importlib.util.spec_from_file_location("installer",sys.argv.pop(1))
m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
original=m.run; calls=[]
def checked(*a,**kw):
    command=list(map(str,a));calls.append(command)
    if any(word in command for word in ("fetch","install","build","update","clone")):
        raise RuntimeError("repeat setup requested expensive/network command: "+repr(command))
    return original(*a,**kw)
m.run=checked
m.main()
print("AUDIT_COMMANDS "+json.dumps(calls))
'''
    for round_number in range(5):
        for offline in ([False, True] if round_number % 2 == 0 else [True, False]):
            label = f"reuse-{round_number}-{'offline' if offline else 'default'}"
            row, output = run(label, ["python3", "-c", guard, str(INSTALLER),
                                      *(["--offline"] if offline else [])], env)
            audit = next(line.removeprefix("AUDIT_COMMANDS ") for line in output.splitlines()
                         if line.startswith("AUDIT_COMMANDS "))
            row.update(offline=offline, commands=json.loads(audit), reused="verified and reused" in output)
            if not row["reused"]:
                raise RuntimeError(f"{label} did not reuse installation")
            save()
    report["after"] = identities()
    report["installed_files_and_selector_unchanged"] = report["before"] == report["after"]
    if not report["installed_files_and_selector_unchanged"]:
        raise RuntimeError("repeated setup changed installation or selector")
    report["median_ms"] = {mode: statistics.median(row["seconds"] for row in report["steps"]
                              if row["name"].endswith(mode)) * 1000 for mode in ("default", "offline")}
    save()

    # No full compiler rebuild: a stdlib-only temporary crate exercises the
    # actual Cargo precedence of the installer's wrapper/target/flags overrides.
    cargo, rustc = installer.local_toolchain()
    with tempfile.TemporaryDirectory(prefix="angel-blaze-setup-control-") as temporary:
        work = Path(temporary)
        (work / "src").mkdir()
        (work / ".cargo").mkdir()
        (work / "Cargo.toml").write_text('[package]\nname="ra-jit"\nversion="0.1.0"\nedition="2021"\n')
        (work / "src/main.rs").write_text('fn main() { println!("setup-control-original"); }\n')
        wrapper = work / "refuse-wrapper"
        wrapper.write_text('#!/bin/sh\necho invoked >> "' + str(work / "wrapper-invoked") + '"\nexit 91\n')
        wrapper.chmod(0o755)
        host = subprocess.check_output([rustc, "--print", "host-tuple"], text=True).strip()
        config = {'build': {'rustc-wrapper': str(wrapper), 'rustc-workspace-wrapper': str(wrapper),
                            'target-dir': str(work / 'wrong-config-target')},
                  'target': {host: {'rustflags': ['--invalid-config-rustflag']}}}
        # These values are JSON-compatible TOML strings/arrays.
        (work / ".cargo/config.toml").write_text(
            '[build]\n' + '\n'.join(f'{key} = {json.dumps(value)}' for key, value in config['build'].items()) +
            f'\n[target.{host}]\nrustflags = ["--invalid-config-rustflag"]\n')
        polluted = dict(env, RUSTC=rustc, RUSTUP_TOOLCHAIN=installer.TOOLCHAIN,
                        CARGO_BUILD_RUSTC_WRAPPER=str(wrapper), CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER=str(wrapper),
                        CARGO_BUILD_TARGET_DIR=str(work / 'wrong-env-target'), CARGO_BUILD_TARGET='wasm32-unknown-unknown')
        for name in ('RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS',
                     'CARGO_TARGET_DIR'):
            polluted.pop(name, None)
        run('cargo-config-negative', [cargo, 'build', '--offline'], polluted, work, expected=101)
        if not (work / 'wrapper-invoked').exists():
            raise RuntimeError('negative control did not invoke configured wrapper')
        (work / 'wrapper-invoked').unlink()
        installer.SOURCE = work
        original_env = dict(os.environ)
        try:
            os.environ.clear()
            os.environ.update(polluted)
            command, clean, artifacts = installer.build_settings(cargo, rustc, offline=True)
        finally:
            os.environ.clear()
            os.environ.update(original_env)
        # --locked also applies to this actual Cargo control.
        run('cargo-lockfile', [cargo, 'generate-lockfile', '--offline'], clean, work)
        for label, marker in [('original', 'original'), ('edit', 'edited'), ('restore', 'original')]:
            source = work / 'src/main.rs'
            source.write_text(f'fn main() {{ println!("setup-control-{marker}"); }}\n')
            run('cargo-config-' + label, command, clean, work)
            binary = artifacts / 'ra-jit'
            row, output = run('execute-' + label, [str(binary)], clean, work)
            row.update(source_sha256=sha(source), binary_sha256=sha(binary),
                       observed=output.strip(), expected='setup-control-' + marker)
            if row['observed'] != row['expected']:
                raise RuntimeError('edited/restored behavior did not execute')
            save()
        report['config_isolation'] = {'wrapper_not_invoked': not (work / 'wrapper-invoked').exists(),
                                      'wrong_config_target_absent': not (work / 'wrong-config-target').exists(),
                                      'wrong_env_target_absent': not (work / 'wrong-env-target').exists()}
        if not all(report['config_isolation'].values()):
            raise RuntimeError('Cargo overrides did not isolate configuration')
        save()
    print(json.dumps({'median_ms': report['median_ms'], 'config_isolation': report['config_isolation'],
                      'installed_files_and_selector_unchanged': report['installed_files_and_selector_unchanged']}))


if __name__ == '__main__':
    main()
