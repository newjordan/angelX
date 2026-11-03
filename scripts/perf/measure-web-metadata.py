#!/usr/bin/env python3
"""Compare frozen web-bundle boundary variants with real source launches.

Run with no other source editor/build in this checkout. Baseline source files
must already exist as PREFIX-baseline-{build,lib}.rs. Keep packaging policies,
features and Cargo freshness intact. Restore the installed candidate and CSS
in finally, checking ownership before each write. Variant-switch rebuild costs
are recorded separately from warm edit/restore and no-touch observations.
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
SOURCES = {"build": ROOT / "cockpit/web-bundle/build.rs",
           "lib": ROOT / "cockpit/web-bundle/lib.rs"}
CSS = ROOT / "crates/web/shell/shell.css"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--baseline-prefix", type=Path, required=True)
    parser.add_argument("--pairs", type=int, default=2, choices=range(1, 5))
    parser.add_argument("--candidate-first", action="store_true")
    args = parser.parse_args()
    output = args.output.resolve()
    # All artifacts are exclusive: never overwrite an earlier result.
    with output.open("x") as file:
        file.write("{}\n")
    variants = {
        "baseline": {key: Path(str(args.baseline_prefix) + f"-baseline-{key}.rs").read_bytes()
                     for key in SOURCES},
        "candidate": {key: path.read_bytes() for key, path in SOURCES.items()},
    }
    for variant, files in variants.items():
        for key, data in files.items():
            with output.with_name(f"{output.stem}-{variant}-{key}.rs").open("xb") as file:
                file.write(data)
    original_css = CSS.read_bytes()
    owned = {**variants["candidate"], "css": original_css}
    paths = {**SOURCES, "css": CSS}
    env = dict(os.environ)
    for key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
                "ANGEL_DEV_SKIP_WEB_BUNDLE", "CARGO_TARGET_DIR", "CARGO_BUILD_TARGET", "CARGO_LOG",
                "ANGEL_NO_BUILD", "ANGEL_LAUNCH_BINARY", "ANGEL_LAUNCH_BINARY_SHA256", "ANGEL_BUILD_SOURCE_SHA256"):
        env.pop(key, None)
    env.update(ANGEL_BUILD_BACKEND="cargo", ANGEL_BIND_SOURCE="0", ANGEL_FAST="1")
    report = {"schema": "angel.web-metadata-boundary/v1", "steps": [],
              "scope": "full-feature current-source launcher; real CSS edits and host relink; no UI-ready timing",
              "command": [str(ROOT / "bin/angelX"), "--version"],
              "source_sha256": {name: {key: sha(data) for key, data in files.items()}
                                for name, files in variants.items()},
              "css_sha256": sha(original_css), "probe_sha256": sha(Path(__file__).read_bytes()),
              "invariants_sha256": {path: sha((ROOT / path).read_bytes()) for path in
                                    ("cockpit/Cargo.toml", "cockpit/web-bundle/pack_asset.rs", "bin/angelX")},
              "restored": False}

    def save():
        output.write_text(json.dumps(report, indent=2) + "\n")

    def select(name, css):
        for key, path in paths.items():
            if path.read_bytes() != owned[key]:
                raise RuntimeError(f"concurrent source edit: {path}; refusing overwrite")
        for key, data in {**variants[name], "css": css}.items():
            if data != owned[key]:
                paths[key].write_bytes(data)
                owned[key] = data

    def inventory():
        # Bind generated inputs to this library's rustc dep-info, never to the
        # newest output left behind by an unrelated feature/profile invocation.
        metadata = list((ROOT / "cockpit/target/fast/deps").glob("libangel_web_bundle-*.rmeta"))
        assert len(metadata) == 1, "multiple metadata variants: use a qualified target directory"
        rmeta = metadata[0]
        depinfo = rmeta.with_name(rmeta.stem.removeprefix("lib") + ".d")
        line = next(line for line in depinfo.read_text().splitlines()
                    if line.startswith(str(rmeta) + ": "))
        generated = re.findall(r'(/[^ \t\n]+/build/angel-web-bundle-[^/ ]+/out/delve_web\.rs)(?= |$)', line)
        assert len(generated) == 1 and "\\" not in generated[0], "ambiguous dep-info path"
        codepath = Path(generated[0])
        code = codepath.read_text()
        # include_str! index and include_bytes! table remain the complete inputs
        # of embedded(), regardless of whether they are function-local or static.
        assets = {}
        for name, source in re.findall(r'\("([^"\n]+)", include_bytes!\("([^"\n]+)"\)\)', code):
            stored = Path(source).read_bytes()
            plain = zlib.decompress(stored) if source.endswith(".zlib") else stored
            if name in assets:
                raise RuntimeError("duplicate asset " + name)
            assets[name] = sha(plain)
        assert assets["shell.css"] == sha(owned["css"])
        index = re.findall(r'include_str!\("([^"\n]+)"\)', code)
        assert len(index) == 1
        assets["<index.html>"] = sha(Path(index[0]).read_bytes())
        return assets, {"rmeta_bytes": rmeta.stat().st_size, "rmeta_sha256": sha(rmeta.read_bytes()),
                        "rlib_bytes": rmeta.with_suffix(".rlib").stat().st_size,
                        "generated_source": str(codepath.relative_to(ROOT)),
                        "generated_sha256": sha(codepath.read_bytes()), "depinfo_sha256": sha(depinfo.read_bytes())}

    def run(label, mode, variant, expected=None):
        for path, digest in report["invariants_sha256"].items():
            assert sha((ROOT / path).read_bytes()) == digest, "concurrent invariant edit: " + path
        log = output.with_name(f"{output.stem}-{label}.log")
        start = time.perf_counter()
        with log.open("x") as file:
            child = subprocess.Popen(report["command"], cwd=ROOT, env=env, stdout=subprocess.PIPE,
                                     stderr=file, start_new_session=True)
            try:
                stdout, _ = child.communicate(timeout=300)
            except BaseException:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
                raise
            file.write(stdout.decode("utf-8", errors="replace"))
        row = {"label": label, "mode": mode, "variant": variant,
               "seconds": time.perf_counter() - start, "exit_code": child.returncode, "log": log.name}
        report["steps"].append(row)
        save()
        assert child.returncode == 0 and b"angelX-cockpit 0.2.6" in stdout, str(log)
        assets, sizes = inventory()
        row.update(sizes)
        row["assets"] = assets
        if expected is not None:
            assert assets == expected, "generated asset content changed unexpectedly"
        save()
        print({key: value for key, value in row.items() if key != "assets"}, flush=True)
        return assets

    save()
    try:
        order = ("candidate", "baseline", "baseline", "candidate") if args.candidate_first else ("baseline", "candidate", "candidate", "baseline")
        reference = None
        for phase, variant in enumerate(order):
            select(variant, original_css)
            reference = run(f"p{phase}-prime", "prime", variant, reference)
            for index in range(4):
                run(f"p{phase}-untouched-{index}", "untouched", variant, reference)
            for index in range(args.pairs):
                select(variant, original_css + f"\n/* ANGEL_METADATA_PHASE_{phase}_PAIR_{index} */\n".encode())
                edited = run(f"p{phase}-edit-{index}", "edit", variant)
                assert {key: value for key, value in edited.items() if key != "shell.css"} == {key: value for key, value in reference.items() if key != "shell.css"}
                select(variant, original_css)
                run(f"p{phase}-restore-{index}", "edit", variant, reference)
    finally:
        select("candidate", original_css)
        run("final-restore", "restore", "candidate", reference)
        report["restored"] = all(paths[key].read_bytes() == data for key, data in
                                 {**variants["candidate"], "css": original_css}.items())
        save()
    assert report["restored"]
    print("WEB_METADATA_MEASURED_AND_RESTORED", flush=True)


if __name__ == "__main__":
    main()
