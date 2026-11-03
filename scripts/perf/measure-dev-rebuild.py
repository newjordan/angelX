#!/usr/bin/env python3
"""Measure the real source launcher, including a verified Rust edit and restore.

No provider calls, no no-build shortcut, no feature reduction. Run with no other
editor/build on runtime_paths.rs. Output names are exclusive (do not erase prior
measurements). A failed build remains a failed sample, not a cached-image launch.
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
SOURCE = ROOT / "cockpit/src/platform/runtime_paths.rs"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="new JSON receipt; adjacent logs are retained")
    parser.add_argument("--backend", choices=("blaze", "cargo"), default="cargo")
    parser.add_argument("--compiler-phases", action="store_true",
                        help="Cargo only: record rustc -Ztime-passes for the host, not dependencies")
    parser.add_argument("--case", choices=("build-info", "version"), default="build-info",
                        help="semantic edit in a leaf module or the crate root")
    args = parser.parse_args()
    if args.compiler_phases and args.backend != "cargo":
        parser.error("--compiler-phases is a Cargo-only diagnostic")
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    # Reserve the result before doing any compilation or changing source.
    with output.open("x") as file:
        file.write("{}\n")
    env = dict(os.environ)
    for name in ("ANGEL_NO_BUILD", "ANGEL_LAUNCH_BINARY", "ANGEL_LAUNCH_BINARY_SHA256",
                 "ANGEL_BUILD_SOURCE_SHA256", "ANGEL_DEV_SKIP_WEB_BUNDLE", "RUSTFLAGS",
                 "CARGO_ENCODED_RUSTFLAGS", "RUSTC", "RUSTDOC", "RUSTC_WRAPPER",
                 "RUSTC_WORKSPACE_WRAPPER", "RUSTUP_TOOLCHAIN", "CARGO_TARGET_DIR"):
        env.pop(name, None)
    env.update(ANGEL_BUILD_BACKEND=args.backend, ANGEL_BLAZE_MODE="wrapper",
               ANGEL_BIND_SOURCE="0", ANGEL_FAST="1", ANGEL_DEV="0", ANGEL_VIDEO="1",
               ANGEL_GLASS="0", ANGEL_BACKGROUND_BUILD="0", ANGEL_SETUP="0")
    # A normal (not workspace) wrapper does not create a new Cargo artifact namespace.
    # Only the host gets this diagnostic; optimization/link flags stay unchanged.
    if args.compiler_phases:
        wrapper = output.with_name(output.stem + "-rustc-wrapper.py")
        with wrapper.open("x") as file:
            file.write('''#!/usr/bin/env python3
import os, sys
compiler, *args = sys.argv[1:]
if any(args[i:i+2] == ["--crate-name", "angel"] for i in range(len(args))):
    os.environ["RUSTC_BOOTSTRAP"] = "1"
    args.append("-Ztime-passes")
os.execv(compiler, [compiler, *args])
''')
        wrapper.chmod(0o755)
        env["RUSTC_WRAPPER"] = str(wrapper)
    source = SOURCE if args.case == "build-info" else ROOT / "cockpit/src/main.rs"
    original = source.read_bytes()
    marker = output.stem
    if args.case == "build-info":
        needle = b'serde_json::json!({"sha256": bundle_identity(), "available": required.iter().all(|path| root.join(path).is_file())})'
        replacement = needle.replace(b'{"sha256":',
            b'{"_dev_rebuild_probe": ' + json.dumps(marker).encode() + b', "sha256":')
        cli = ("--build-info", "--json")
    else:
        needle = b'println!("angelX-cockpit {}", env!("CARGO_PKG_VERSION"));'
        replacement = b'println!("angelX-cockpit {} {}", env!("CARGO_PKG_VERSION"), ' + json.dumps(marker).encode() + b');'
        cli = ("--version",)
    changed = original.replace(needle, replacement)
    def has_marker(stdout):
        if args.case == "build-info":
            return json.loads(stdout)["resources"].get("_dev_rebuild_probe") == marker
        return marker in stdout

    report = {
        "schema": "angel.dev-rebuild/v1", "backend": args.backend,
        "scope": "full-feature source launcher; fast; browser retained; no model calls",
        "head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "cargo_toml_sha256": hashlib.sha256((ROOT / "cockpit/Cargo.toml").read_bytes()).hexdigest(),
        "compiler_phase_diagnostic": args.compiler_phases,
        "case": args.case, "source": str(source.relative_to(ROOT)),
        "probe_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "source_sha256": hashlib.sha256(original).hexdigest(), "steps": [],
    }

    def save():
        output.write_text(json.dumps(report, indent=2) + "\n")

    def run(name, *cli):
        start = time.perf_counter()
        log = output.with_name(output.stem + "-" + name + ".log")
        with log.open("x") as errors:
            process = subprocess.Popen([str(ROOT / "bin/angelX"), *cli], cwd=ROOT, env=env,
                                       text=True, stdout=subprocess.PIPE, stderr=errors,
                                       start_new_session=True)
            try:
                stdout, _ = process.communicate(timeout=360)
            except BaseException:
                # Do not leave Cargo/rustc writing after restoring the source.
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.communicate()
                raise
        row = dict(name=name, seconds=time.perf_counter() - start,
                   exit_code=process.returncode, stdout=stdout, log=log.name)
        report["steps"].append(row)
        save()
        print(name, round(row["seconds"], 4), "exit", process.returncode, flush=True)
        if process.returncode:
            raise RuntimeError(f"{name} failed; see {log}")
        return stdout

    save()
    if original.count(needle) != 1:
        raise RuntimeError("semantic edit location must be unique")
    try:
        run("prime", "--version")
        for index in range(3):
            run(f"unchanged-{index}", "--version")
        # Do not overwrite a concurrent editor's changes.
        if source.read_bytes() != original:
            raise RuntimeError("source changed before probe")
        source.write_bytes(changed)
        stdout = run("semantic-edit", *cli)
        report["semantic_edit_verified"] = has_marker(stdout)
        save()
        if not report["semantic_edit_verified"]:
            raise RuntimeError("executed image did not reflect the semantic edit")
    finally:
        if source.read_bytes() == changed:
            source.write_bytes(original)
            report["source_restored"] = True
            save()
            stdout = run("restore", *cli)
            report["restored_image_verified"] = ("_dev_rebuild_probe" not in json.loads(stdout)["resources"]
                if args.case == "build-info" else marker not in stdout)
            save()
            if not report["restored_image_verified"]:
                raise RuntimeError("restored image still carries probe")
        elif source.read_bytes() != original:
            report["source_restore_conflict"] = True
            save()
            raise RuntimeError("concurrent source change; refusing to overwrite it")
    print("SEMANTIC_EDIT_AND_RESTORE_VERIFIED", flush=True)


if __name__ == "__main__":
    main()
