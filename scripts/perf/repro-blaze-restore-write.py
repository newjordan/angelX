#!/usr/bin/env python3
"""Bounded stdlib reduction of pinned Blaze Kept::restore's file replay.

No Blaze install edits or daemon execution. Compare exact bytes, execute bit,
missing/changed files and unchanged-file mtime. Retain source + proposed patch.
"""
import argparse
import difflib
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "cockpit/target/tools/blaze-src/crates/ra-jit/src/cargo/script.rs"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--fixture", choices=("unchanged", "changed", "small"), default="unchanged")
    args = parser.parse_args()
    output = args.output.resolve()
    with output.open("x") as f:
        f.write("{}\n")
    source = SOURCE.read_text()
    start = source.index("    pub(super) fn restore(")
    end = source.index("\n    pub(super) fn bytes(", start)
    original = source[start:end]
    write = '            std::fs::write(&path, bytes).with_context(|| format!("writing {}", path.display()))?;'
    assert original.count(write) == 1
    replacement = '''            // This is a proposed replay optimization, not an upstream patch.
            // Reject symlinks/non-files and bound reading to the expected size.
            let unchanged = std::fs::symlink_metadata(&path).is_ok_and(|m| {
                m.is_file() && m.len() == bytes.len() as u64
            }) && (|| -> std::io::Result<bool> {
                use std::io::Read as _;
                let mut old = Vec::new();
                std::fs::File::open(&path)?.take(bytes.len() as u64 + 1).read_to_end(&mut old)?;
                Ok(old == *bytes)
            })().unwrap_or(false);
            if !unchanged {
''' + write + '''
            }'''
    candidate = original.replace(write, replacement)
    for suffix, content in [("-original.rs", original), ("-candidate.rs", candidate),
                            ("-candidate.patch", "".join(difflib.unified_diff(
                                original.splitlines(True), candidate.splitlines(True),
                                fromfile="a/crates/ra-jit/src/cargo/script.rs",
                                tofile="b/crates/ra-jit/src/cargo/script.rs")))]:
        output.with_name(output.stem + suffix).write_text(content)

    def reduced(text, name):
        return text.replace("pub(super) fn restore", "fn " + name).replace(
            "anyhow::Result<()>", "std::io::Result<()>").replace(
            '.with_context(|| format!("writing {}", path.display()))', "")

    harness = r'''
use std::{fs, path::Path, time::{Duration, Instant, SystemTime}};
fn make_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}
struct Kept { files: Vec<(String, bool, Vec<u8>)> }
impl Kept { METHODS }
fn check(kept: &Kept, dir: &Path) {
    use std::os::unix::fs::PermissionsExt as _;
    for (name, executable, bytes) in &kept.files {
        assert_eq!(fs::read(dir.join(name)).unwrap(), *bytes);
        if *executable { assert_ne!(fs::metadata(dir.join(name)).unwrap().permissions().mode() & 0o111, 0); }
    }
}
fn main() {
    let root = std::env::args_os().nth(1).unwrap();
    let root = Path::new(&root);
    let kept = Kept { files: vec![("nested/large.bin".into(), false, vec![42; 12*1024*1024]),
                                 ("tool".into(), true, b"executable".to_vec()),
                                 ("empty".into(), false, vec![])] };
    for candidate in [false, true] {
        let dir = root.join(if candidate {"candidate"} else {"original"});
        let restore = if candidate {Kept::candidate} else {Kept::original};
        restore(&kept, &dir).unwrap(); check(&kept, &dir); // missing outputs
        let path = dir.join("nested/large.bin");
        fs::write(&path, b"changed").unwrap();
        restore(&kept, &dir).unwrap(); check(&kept, &dir);
        // Same-size semantic change, removal, executable permission repair.
        fs::write(&path, vec![43; 12*1024*1024]).unwrap();
        fs::remove_file(dir.join("empty")).unwrap();
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(dir.join("tool"), fs::Permissions::from_mode(0o644)).unwrap();
        restore(&kept, &dir).unwrap(); check(&kept, &dir);
        let before = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        fs::File::options().write(true).open(&path).unwrap().set_times(fs::FileTimes::new().set_modified(before)).unwrap();
        restore(&kept, &dir).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap() == before, candidate);
        // Errors still propagate; do not turn directory slots into cache hits.
        fs::remove_file(dir.join("empty")).unwrap(); fs::create_dir(dir.join("empty")).unwrap();
        assert!(restore(&kept, &dir).is_err()); fs::remove_dir(dir.join("empty")).unwrap();
        restore(&kept, &dir).unwrap();
    }
    for round in 0..6 {
        let order = if round % 2 == 0 {[false, true]} else {[true, false]};
        for candidate in order {
            let dir = root.join(if candidate {"candidate"} else {"original"});
            let restore = if candidate {Kept::candidate} else {Kept::original};
            let start = Instant::now();
            for _ in 0..8 { restore(&kept, &dir).unwrap(); }
            println!("sample {round} {candidate} {}", start.elapsed().as_nanos());
            check(&kept, &dir);
        }
    }
    println!("RESTORE_BYTES_MODES_ERRORS_AND_UNCHANGED_MTIME_VERIFIED");
}
'''.replace("METHODS", reduced(original, "original") + reduced(candidate, "candidate"))
    if args.fixture == "changed":
        harness = harness.replace(
            'for _ in 0..8 { restore(&kept, &dir).unwrap(); }',
            'for _ in 0..8 { fs::write(dir.join("nested/large.bin"), vec![43; 12*1024*1024]).unwrap(); restore(&kept, &dir).unwrap(); }')
    elif args.fixture == "small":
        harness = harness.replace('12*1024*1024', '1024').replace('for _ in 0..8', 'for _ in 0..128')
    src = output.with_suffix(".rs")
    src.write_text(harness)
    exe = output.with_suffix(".bin")
    tc = Path(os.environ.get("RUSTUP_HOME", str(Path.home() / ".rustup"))) / "toolchains/1.95.0-x86_64-unknown-linux-gnu/bin"
    compile_result = subprocess.run([str(tc / "rustc"), "--edition=2024", "-O", str(src), "-o", str(exe)], capture_output=True, text=True)
    report = {"schema": "angel.blaze-restore-reduction/v1", "source": str(SOURCE.relative_to(ROOT)),
              "fixture": args.fixture,
              "method_sha256": hashlib.sha256(original.encode()).hexdigest(),
              "scope": "extracted method; warm local file replay, not whole-Blaze or angelX latency",
              "compile_exit": compile_result.returncode, "compile_stderr": compile_result.stderr}
    try:
        assert compile_result.returncode == 0, compile_result.stderr
        work = output.with_suffix(".work")
        work.mkdir()
        result = subprocess.run([str(exe), str(work)], capture_output=True, text=True, timeout=30)
        report.update(exit_code=result.returncode, stdout=result.stdout, stderr=result.stderr)
        assert result.returncode == 0, result.stderr
    finally:
        output.write_text(json.dumps(report, indent=2) + "\n")
    print(report["stdout"], end="")


if __name__ == "__main__":
    main()
