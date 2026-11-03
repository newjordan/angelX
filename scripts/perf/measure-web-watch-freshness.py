#!/usr/bin/env python3
"""Exercise directory-covered browser input edits/additions/removals with real Cargo.

Uses the production watch minimizer, no no-build or skip-web shortcut. Edits a
CSS comment and creates one CSS input, then restores both even after a failure.
Run without another editor/build on shell.css. Output names must be new.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import time
import zlib

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "crates/web/shell/shell.css"
ADDED = SOURCE.with_name("watch-freshness-probe.css")
MARKER = b"ANGEL_DIRECTORY_WATCH_PROBE_20261028"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    with output.open("x") as file:
        file.write("{}\n")
    if ADDED.exists() or ADDED.is_symlink():
        raise RuntimeError("probe input already exists; refusing to overwrite")
    original = SOURCE.read_bytes()
    changed = original + b"\n/* " + MARKER + b" */\n"
    added = b"/* " + MARKER + b" added input */\n"
    toolchain = Path(os.environ.get("RUSTUP_HOME", str(Path.home() / ".rustup"))) / "toolchains/1.95.0-x86_64-unknown-linux-gnu/bin"
    env = dict(os.environ, RUSTC=str(toolchain / "rustc"), RUSTDOC=str(toolchain / "rustdoc"))
    for key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
                "ANGEL_DEV_SKIP_WEB_BUNDLE", "CARGO_TARGET_DIR", "CARGO_BUILD_TARGET"):
        env.pop(key, None)
    command = [str(toolchain / "cargo"), "build", "--offline", "--locked", "--profile", "fast",
               "--manifest-path", str(ROOT / "cockpit/Cargo.toml"), "-p", "angel-web-bundle", "-vv"]
    report = {"schema": "angel.directory-watch-freshness/v1", "steps": [],
              "scope": "actual browser package, directory-covered CSS comment edit/add/remove; no model calls",
              "source_sha256": sha(original), "helper_sha256": sha((ROOT / "cockpit/build_fast.rs").read_bytes()),
              "probe_sha256": sha(Path(__file__).read_bytes()), "command": command}

    def save():
        output.write_text(json.dumps(report, indent=2) + "\n")

    def bundle():
        generated = max((ROOT / "cockpit/target/fast/build").glob("angel-web-bundle-*/out/delve_web.rs"),
                        key=lambda path: path.stat().st_mtime_ns)
        return generated, generated.read_text()

    def asset(code, name):
        entry = re.search(r'\("' + re.escape(name) + r'", include_bytes!\("([^\"]+)"\)\)', code)
        if entry is None:
            return None
        path = Path(entry[1])
        data = path.read_bytes()
        return zlib.decompress(data) if path.suffix == ".zlib" else data

    def run(label, expected_css, expected_added):
        log = output.with_name(output.stem + "-" + label + ".log")
        started = time.perf_counter()
        with log.open("x") as file:
            child = subprocess.Popen(command, cwd=ROOT, env=env, stdout=file, stderr=subprocess.STDOUT,
                                     start_new_session=True)
            try:
                child.wait(timeout=180)
            except BaseException:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
                raise
        row = {"label": label, "exit_code": child.returncode, "seconds": time.perf_counter() - started,
               "log": log.name}
        report["steps"].append(row)
        save()
        if child.returncode:
            raise RuntimeError("build failed: " + str(log))
        generated, code = bundle()
        css, extra = asset(code, SOURCE.name), asset(code, ADDED.name)
        row.update(css_sha256=sha(css) if css is not None else None,
                   css_matches=css == expected_css, added_matches=extra == expected_added,
                   generated_mtime_ns=generated.stat().st_mtime_ns)
        save()
        assert row["css_matches"] and row["added_matches"], row
        print(row, flush=True)
        return generated

    save()
    owns_added = False
    try:
        generated = run("prime", original, None)
        watched = [line.split("=", 1)[1] for line in (generated.parent.parent / "output").read_text().splitlines()
                   if line.startswith("cargo:rerun-if-changed=")]
        report["shell_directory_watched"] = str(SOURCE.parent) in watched
        report["css_explicit_watch_removed"] = str(SOURCE) not in watched
        assert report["shell_directory_watched"] and report["css_explicit_watch_removed"]
        if SOURCE.read_bytes() != original:
            raise RuntimeError("concurrent CSS edit; refusing overwrite")
        SOURCE.write_bytes(changed)
        run("covered-css-edit", changed, None)
        with ADDED.open("xb") as file:
            owns_added = True
            file.write(added)
        run("input-added", changed, added)
        if ADDED.read_bytes() != added:
            raise RuntimeError("concurrent added-input edit; refusing removal")
        ADDED.unlink()
        owns_added = False
        run("input-removed", changed, None)
    finally:
        if SOURCE.read_bytes() == changed:
            SOURCE.write_bytes(original)
        elif SOURCE.read_bytes() != original:
            raise RuntimeError("concurrent CSS edit; refusing restoration")
        if owns_added:
            if ADDED.read_bytes() != added:
                raise RuntimeError("concurrent added-input edit; refusing removal")
            ADDED.unlink()
        run("restore", original, None)
        report.update(source_restored=SOURCE.read_bytes() == original,
                      added_input_removed=not ADDED.exists())
        save()
    assert report["source_restored"] and report["added_input_removed"]
    print("DIRECTORY_COVERED_EDIT_ADD_REMOVE_RESTORE_VERIFIED", flush=True)


if __name__ == "__main__":
    main()
