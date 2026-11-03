#!/usr/bin/env python3
"""Controlled exact-byte reuse experiment: candidate/baseline/baseline/candidate.

Run without another editor/build touching the browser. Uses current source with
only the packing loop switched; records frozen variants. Shell comments are
edited/restored after every sample. Each build is real locked/offline Cargo with
all browser assets. No build skipping or timestamp freshness shortcut.
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
BUILD = ROOT / "cockpit/web-bundle/build.rs"
CSS = ROOT / "crates/web/shell/shell.css"
NEW = '''        let packed_path = packed_dir.join(format!("{index}.zlib"));
        let original_len = pack_asset::pack_asset(&plain, &packed_path)
            .expect("pack and verify browser asset");
        let stored_path = if let Some(original_len) = original_len {
'''
OLD = '''        let (stored, original_len) = bundle_bytes::pack(&plain);
        let stored_path = if let Some(original_len) = original_len {
            assert_eq!(bundle_bytes::unpack(&stored, Some(original_len)).expect("verify packed browser asset"), plain);
            let packed_path = packed_dir.join(format!("{index}.zlib"));
            if fs::read(&packed_path).ok().as_deref() != Some(stored.as_slice()) {
                fs::write(&packed_path, &stored).expect("write packed browser asset");
            }
'''


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--pairs", type=int, default=3, choices=range(1, 7))
    parser.add_argument("--launcher-edits", action="store_true",
                        help="measure full source-launch CSS edits, including host link")
    parser.add_argument("--baseline-first", action="store_true", help="reverse the four-phase order")
    args = parser.parse_args()
    output = args.output.resolve()
    with output.open("x") as file:
        file.write("{}\n")
    candidate = BUILD.read_bytes()
    if candidate.count(NEW.encode()) != 1 or candidate.count(b"mod pack_asset;\n") != 1:
        raise RuntimeError("candidate shape changed; refusing to invent baseline")
    baseline = candidate.replace(NEW.encode(), OLD.encode()).replace(b"mod pack_asset;\n", b"")
    original_css = CSS.read_bytes()
    current_build, current_css = candidate, original_css
    tc = Path(os.environ.get("RUSTUP_HOME", str(Path.home() / ".rustup"))) / "toolchains/1.95.0-x86_64-unknown-linux-gnu/bin"
    env = dict(os.environ)
    for key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
                "ANGEL_DEV_SKIP_WEB_BUNDLE", "CARGO_TARGET_DIR", "CARGO_BUILD_TARGET", "CARGO_LOG",
                "ANGEL_NO_BUILD", "ANGEL_LAUNCH_BINARY", "ANGEL_LAUNCH_BINARY_SHA256"):
        env.pop(key, None)
    env.update(RUSTC=str(tc / "rustc"), RUSTDOC=str(tc / "rustdoc"),
               ANGEL_BUILD_BACKEND="cargo", ANGEL_BIND_SOURCE="0")
    command = [str(tc / "cargo"), "build", "--offline", "--locked", "--profile", "fast",
               "--manifest-path", str(ROOT / "cockpit/Cargo.toml"), "-p", "angel-web-bundle"]
    report = {"schema": "angel.web-pack-reuse/v1", "command": command, "steps": [],
              "scope": "full source-launch CSS edit wall" if args.launcher_edits else "warm browser-package edits; separate unchanged full-source launcher timing; no UI-ready timing",
              "probe_sha256": sha(Path(__file__).read_bytes()),
              "css_sha256": sha(original_css), "helper_sha256": sha((BUILD.parent / "pack_asset.rs").read_bytes()),
              "restored": False}
    for name, data in (("baseline", baseline), ("candidate", candidate)):
        path = output.with_name(output.stem + "-" + name + ".rs")
        with path.open("xb") as file:
            file.write(data)
        report[name + "_sha256"] = sha(data)
    with output.with_name(output.stem + "-helper.rs").open("xb") as file:
        file.write((BUILD.parent / "pack_asset.rs").read_bytes())

    def save():
        output.write_text(json.dumps(report, indent=2) + "\n")

    def select(build, css):
        nonlocal current_build, current_css
        if BUILD.read_bytes() != current_build or CSS.read_bytes() != current_css:
            raise RuntimeError("concurrent source edit; refusing overwrite")
        if build != current_build:
            BUILD.write_bytes(build)
        if css != current_css:
            CSS.write_bytes(css)
        current_build, current_css = build, css

    def bundle():
        path = max((ROOT / "cockpit/target/fast/build").glob("angel-web-bundle-*/out/delve_web.rs"),
                   key=lambda path: path.stat().st_mtime_ns)
        code = path.read_text()
        assets = {}
        for name, source in re.findall(r'\("([^"\n]+)", include_bytes!\("([^"\n]+)"\)\)', code):
            stored = Path(source).read_bytes()
            plain = zlib.decompress(stored) if source.endswith(".zlib") else stored
            if name in assets:
                raise RuntimeError("duplicate asset: " + name)
            assets[name] = sha(plain)
        assert assets["shell.css"] == sha(current_css)
        return assets

    def run(label, mode, config, expected=None):
        log = output.with_name(output.stem + "-" + label + ".log")
        launches = mode == "startup" or (mode == "edit" and args.launcher_edits)
        cmd = [str(ROOT / "bin/angelX"), "--version"] if launches else command
        start = time.perf_counter()
        with log.open("x") as file:
            child = subprocess.Popen(cmd, cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=file,
                                     start_new_session=True)
            try:
                stdout, _ = child.communicate(timeout=300)
                file.write(stdout.decode("utf-8", errors="replace"))
            except BaseException:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
                raise
        row = {"label": label, "mode": mode, "configuration": config,
               "seconds": time.perf_counter() - start, "exit_code": child.returncode, "log": log.name}
        report["steps"].append(row)
        save()
        if child.returncode:
            raise RuntimeError("build failed: " + str(log))
        assets = bundle()
        row["assets"] = assets
        if expected is not None:
            assert assets == expected, "asset inventory/decoded content mismatch"
        if launches:
            assert "angelX-cockpit 0.2.6" in log.read_text()
        save()
        print({key: value for key, value in row.items() if key != "assets"}, flush=True)
        return assets

    save()
    try:
        order = ("baseline", "candidate", "candidate", "baseline") if args.baseline_first else ("candidate", "baseline", "baseline", "candidate")
        for phase, name in enumerate(order):
            assert sha((BUILD.parent / "pack_asset.rs").read_bytes()) == report["helper_sha256"], "concurrent packing-helper edit"
            config = candidate if name == "candidate" else baseline
            select(config, original_css)
            reference = run(f"p{phase}-prime", "prime", name)
            # Full launcher build primes host relinking outside warm observations.
            run(f"p{phase}-launch-prime", "startup", name, reference)
            report["steps"][-1]["mode"] = "startup-prime"
            save()
            for n in range(4):
                run(f"p{phase}-launch-{n}", "startup", name, reference)
            for n in range(args.pairs):
                changed = original_css + f"\n/* ANGEL_PACK_REUSE_PHASE_{phase}_PAIR_{n} */\n".encode()
                select(config, changed)
                edited = run(f"p{phase}-edit-{n}", "edit", name)
                assert {k: v for k, v in edited.items() if k != "shell.css"} == {k: v for k, v in reference.items() if k != "shell.css"}
                select(config, original_css)
                run(f"p{phase}-restore-{n}", "edit", name, reference)
    finally:
        select(candidate, original_css)
        run("final-restore", "startup", "candidate")
        report["steps"][-1]["mode"] = "restore"
        report["restored"] = BUILD.read_bytes() == candidate and CSS.read_bytes() == original_css
        save()
    assert report["restored"]
    print("WEB_PACK_REUSE_CONTROLLED_AND_RESTORED", flush=True)


if __name__ == "__main__":
    main()
