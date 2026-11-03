#!/usr/bin/env python3
"""Local-archive-only control of pinned Blaze install.sh; no real compiler replaced."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "cockpit/target/tools/blaze-src"
SCRIPT = SOURCE / "install.sh"
IMAGES = ("blaze", "cargo-blaze", "ra-jit", "ra-jit-rustc")


def sha(path):
    with path.open("rb") as file:
        return hashlib.file_digest(file, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    report = {"schema": "angel.blaze-install-publication/v1", "scope": __doc__,
              "upstream_revision": subprocess.check_output(["git", "-C", str(SOURCE), "rev-parse", "HEAD"], text=True).strip(),
              "install_script_sha256": sha(SCRIPT), "probe_sha256": sha(Path(__file__)), "steps": []}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x") as file:
        json.dump(report, file, indent=2)

    def save():
        args.output.write_text(json.dumps(report, indent=2) + "\n")

    with tempfile.TemporaryDirectory(prefix="blaze-install-publication-") as temporary:
        work = Path(temporary)
        archive_root = work / "blaze-local-fixture"
        archive_root.mkdir()
        for name in IMAGES:
            image = archive_root / name
            image.write_text("#!/bin/sh\necho new-generation\n")
            image.chmod(0o755)
        archive = work / "blaze-local-fixture.tar.gz"
        with tarfile.open(archive, "w:gz") as output:
            output.add(archive_root, arcname=archive_root.name)
        report["archive_sha256"] = sha(archive)
        home = work / "cargo-home"
        bin_dir = home / "bin"
        bin_dir.mkdir(parents=True)
        env = dict(os.environ, CARGO_HOME=str(home), BLAZE_ARCHIVE=str(archive),
                   BLAZE_NO_TOOLCHAIN="1", RUSTUP_AUTO_INSTALL="0")
        env.pop("BLAZE_VERSION", None)
        # A tripwire catches even attempted downloads. Actual local archive path
        # must not call curl/wget, even if the machine happens to be online.
        tripwires = work / "tripwires"
        tripwires.mkdir()
        for name in ("curl", "wget"):
            image = tripwires / name
            image.write_text(f'#!/bin/sh\necho {name} >> "{work / "network-tripwire"}"\nexit 92\n')
            image.chmod(0o755)
        env["PATH"] = str(tripwires) + os.pathsep + os.environ["PATH"]

        def identities():
            return {name: {"sha256": sha(bin_dir / name), "inode": (bin_dir / name).stat().st_ino,
                           "mtime_ns": (bin_dir / name).stat().st_mtime_ns,
                           "text": (bin_dir / name).read_text()}
                    for name in IMAGES if (bin_dir / name).exists()}

        def run(label):
            command = ["sh", str(SCRIPT), "--no-toolchain"]
            result = subprocess.run(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                    text=True, timeout=20)
            log = args.output.with_name(args.output.stem + "-" + label + ".log")
            with log.open("x") as file:
                file.write(result.stdout)
            row = {"name": label, "command": command, "exit_code": result.returncode,
                   "log": log.name, "log_sha256": sha(log), "installed": identities()}
            report["steps"].append(row)
            save()
            return row

        first = run("first")
        second = run("repeat-identical")
        if first["exit_code"] or second["exit_code"]:
            raise RuntimeError("local archive control failed; inspect logs")
        report["repeat_identical_bytes"] = all(first["installed"][name]["sha256"] == second["installed"][name]["sha256"] for name in IMAGES)
        report["repeat_metadata_changed"] = all(first["installed"][name] != second["installed"][name] for name in IMAGES)
        for name in IMAGES:
            (bin_dir / name).write_text("#!/bin/sh\necho old-generation\n")
        # Fault-inject exactly one rename failure after three earlier programs
        # publish. These are inert local scripts, never the installed compiler.
        real_mv = shutil.which("mv")
        mv = tripwires / "mv"
        mv.write_text('#!/bin/sh\ncase "$*" in *ra-jit-rustc*) echo injected-publication-failure >&2; exit 93;; esac\nexec ' + json.dumps(real_mv) + ' "$@"\n')
        mv.chmod(0o755)
        failed = run("injected-last-rename-failure")
        values = {name: state["text"].splitlines()[-1] for name, state in failed["installed"].items()}
        report["failure_left_mixed_generation"] = failed["exit_code"] != 0 and set(values.values()) == {"echo new-generation", "echo old-generation"}
        report["network_tripwire_absent"] = not (work / "network-tripwire").exists()
        save()
        if not (report["repeat_identical_bytes"] and report["repeat_metadata_changed"] and
                report["failure_left_mixed_generation"] and report["network_tripwire_absent"]):
            raise RuntimeError("upstream installation control inconclusive")
    print(json.dumps({key: report[key] for key in ("repeat_identical_bytes", "repeat_metadata_changed",
                                                  "failure_left_mixed_generation", "network_tripwire_absent")}))


if __name__ == "__main__":
    main()
