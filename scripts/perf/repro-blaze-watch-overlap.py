#!/usr/bin/env python3
"""Bounded stdlib-only reduction of Blaze's repeated rerun-if-changed traversal.

Extracts the real pinned watched() function, compares a visited-spelling candidate
in isolation, and leaves Blaze itself untouched. This measures traversal only,
not a Blaze daemon or angelX end-to-end speedup. No remote/provider calls.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "cockpit/target/tools/blaze-src/crates/ra-jit/src/cargo/script.rs"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--fixture", choices=("overlap", "tree", "files"), default="overlap",
                        help="tree/files are nonoverlapping controls for visited-set overhead")
    parser.add_argument("--directives", type=Path,
                        help="replay actual build-script stdout instead of synthetic directives")
    parser.add_argument("--cwd", type=Path,
                        help="original build-script cwd (required with --directives)")
    parser.add_argument("--algorithm", choices=("visited", "entry-types", "entry-types-only"), default="visited",
                        help="entry-types avoids opening known nondirectory children")
    args = parser.parse_args()
    if (args.directives is None) != (args.cwd is None):
        parser.error("--directives and --cwd must be supplied together")
    output = args.output.resolve()
    with output.open("x") as file:
        file.write("{}\n")
    source = SOURCE.read_text()
    start = source.index("fn watched(stdout:")
    end = source.index("\n}\n", start) + 2
    original = source[start:end]
    candidate = original.replace("fn watched(", "fn watched_unique(", 1)
    assert candidate.count("    let mut paths = Vec::new();") == 1
    assert candidate.count("        while let Some(path) = pending.pop() {") == 1
    candidate = candidate.replace("    let mut paths = Vec::new();", "    let mut seen = std::collections::HashSet::new();\n    let mut paths = Vec::new();", 1)
    candidate = candidate.replace("        while let Some(path) = pending.pop() {", "        while let Some(path) = pending.pop() {\n            if !seen.insert(path.as_os_str().to_owned()) { continue; }", 1)
    if args.algorithm in ("entry-types", "entry-types-only"):
        candidate = candidate.replace("let mut pending = vec![cwd.join(path)];", "let mut pending = vec![(cwd.join(path), true)];")
        candidate = candidate.replace("while let Some(path) = pending.pop()", "while let Some((path, may_descend)) = pending.pop()")
        candidate = candidate.replace("if let Ok(entries) = std::fs::read_dir(&path) {", "if let Ok(entries) = may_descend.then(|| std::fs::read_dir(&path)).transpose() {\n                if let Some(entries) = entries {")
        candidate = candidate.replace("pending.extend(entries.flatten().map(|it| it.path()));", "pending.extend(entries.flatten().map(|it| {\n                    let may_descend = it.file_type().map_or(true, |kind| kind.is_dir() || kind.is_symlink());\n                    (it.path(), may_descend)\n                }));\n                }")
        if args.algorithm == "entry-types-only":
            candidate = candidate.replace("    let mut seen = std::collections::HashSet::new();\n", "")
            candidate = candidate.replace("            if !seen.insert(path.as_os_str().to_owned()) { continue; }\n", "")
    harness = '''use std::path::Path;
use std::collections::BTreeSet;
use std::time::Instant;
''' + original + "\n\n" + candidate + '''
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let cwd = Path::new(&args[1]);
    let stdout = std::fs::read(&args[2]).unwrap();
    let old = watched(&stdout, cwd);
    let new = watched_unique(&stdout, cwd);
    let old_set: BTreeSet<_> = old.iter().collect();
    let new_set: BTreeSet<_> = new.iter().collect();
    assert_eq!(old_set, new_set, "every original spelling must be retained");
    assert_eq!(new.len(), new_set.len());
    println!("counts {} {} {}", old.len(), new.len(), old_set.len());
    for round in 0..6 {
        let modes = if round % 2 == 0 { [false, true] } else { [true, false] };
        for unique in modes {
            let started = Instant::now();
            for _ in 0..20 {
                let paths = if unique { watched_unique(&stdout, cwd) } else { watched(&stdout, cwd) };
                std::hint::black_box(paths);
            }
            println!("sample {} {} {}", round, unique, started.elapsed().as_nanos());
        }
    }
}
'''
    if args.algorithm == "entry-types-only":
        harness = harness.replace("    assert_eq!(new.len(), new_set.len());", "    let mut old_sorted = old.clone();\n    let mut new_sorted = new.clone();\n    old_sorted.sort();\n    new_sorted.sort();\n    assert_eq!(old_sorted, new_sorted, \"path multiplicity must also match\");")
    rust_path = output.with_suffix(".rs")
    with rust_path.open("x") as file:
        file.write(harness)
    compiler = Path(os.environ.get("RUSTUP_HOME", str(Path.home() / ".rustup"))) / "toolchains/1.95.0-x86_64-unknown-linux-gnu/bin/rustc"
    identity = subprocess.check_output([str(compiler), "-vV"], text=True)
    pin = subprocess.check_output(["git", "-C", str(SOURCE.parents[4]), "rev-parse", "HEAD"], text=True).strip()
    report = {"schema": "angel.blaze-watch-reduction/v1", "scope": "isolated exact upstream function; not a Blaze build or daemon speed claim",
              "source_pin": pin, "source_sha256": digest(SOURCE.read_bytes()), "function_sha256": digest(original.encode()),
              "harness_sha256": digest(harness.encode()), "probe_sha256": digest(Path(__file__).read_bytes()),
              "compiler": identity, "harness": rust_path.name, "algorithm": args.algorithm}
    output.write_text(json.dumps(report, indent=2) + "\n")
    with tempfile.TemporaryDirectory(prefix="blaze-watch-", dir=ROOT / "output") as temp:
        temp = Path(temp)
        # All 512 file paths are explicitly repeated after two recursive parents.
        assets = temp / "assets"
        paths = []
        for kit in range(64):
            directory = assets / str(kit)
            directory.mkdir(parents=True)
            for index in range(8):
                path = directory / f"{index}.png"
                path.write_bytes(b"fixture")
                paths.append(path)
        # Missing and lexical alias spellings must survive unchanged as well.
        aliases = [assets / "missing", assets / "0/../0/0.png"]
        if os.name == "posix":
            link = temp / "linked.png"
            link.symlink_to(paths[0])
            aliases.append(link)
            # Exercise discovered symlinks as well as explicit link roots, but
            # avoid cycles (the upstream traversal itself has no cycle guard).
            (assets / "linked-kit").symlink_to(assets / "0", target_is_directory=True)
            (assets / "dangling").symlink_to(temp / "absent")
        if args.fixture == "tree":
            directives = [f"cargo:rerun-if-changed={assets}"]
        elif args.fixture == "files":
            directives = [f"cargo:rerun-if-changed={path}" for path in paths]
        else:
            directives = [f"cargo:rerun-if-changed={assets}", f"cargo::rerun-if-changed={assets}"]
            directives.extend(f"cargo:rerun-if-changed={path}" for path in paths + aliases)
        stdout = temp / "stdout"
        if args.directives:
            raw = args.directives.read_bytes()
            stdout.write_bytes(raw)
            replay_cwd = args.cwd.resolve()
            report.update(directives_source=str(args.directives), directives_sha256=digest(raw), cwd=str(replay_cwd))
        else:
            stdout.write_text("\n".join(directives) + "\n")
            replay_cwd = temp
        executable = temp / "probe"
        command = [str(compiler), "--crate-name", "blaze_watch_reduction", "--edition=2024", "-O", str(rust_path), "-o", str(executable)]
        started = time.perf_counter()
        build = subprocess.run(command, capture_output=True, text=True, timeout=30)
        report.update(compile_exit_code=build.returncode, compile_seconds=time.perf_counter() - started, compile_stderr=build.stderr)
        output.write_text(json.dumps(report, indent=2) + "\n")
        build.check_returncode()
        ran = subprocess.run([str(executable), str(replay_cwd), str(stdout)], capture_output=True, text=True, timeout=30)
        report.update(exit_code=ran.returncode, stdout=ran.stdout, stderr=ran.stderr,
                      fixture={"mode": args.fixture, "directories": 65, "regular_files": 512, "explicit_directives": len(directives)})
        if args.directives:
            report["fixture"] = {"mode": "real-build-stdout", "explicit_directives": sum(
                line.startswith((b"cargo:rerun-if-changed=", b"cargo::rerun-if-changed=")) for line in raw.splitlines())}
        if ran.returncode == 0:
            counts = ran.stdout.splitlines()[0].split()
            report.update(original_expanded_paths=int(counts[1]), candidate_expanded_paths=int(counts[2]),
                          unique_path_spellings=int(counts[3]), equal_path_sets=True,
                          samples=[{"round": int(parts[1]), "candidate": parts[2] == "true", "nanoseconds_20_walks": int(parts[3])}
                                   for line in ran.stdout.splitlines()[1:] if (parts := line.split()) and parts[0] == "sample"])
        output.write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2))
        ran.check_returncode()


if __name__ == "__main__":
    main()
