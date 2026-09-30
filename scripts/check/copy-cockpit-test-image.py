#!/usr/bin/env python3
"""Copy Cargo's development test image and siblings into a gate-owned directory."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import sys


def messages(path):
    result = []
    for line in Path(path).read_text().splitlines():
        if not line.strip():
            continue
        message = json.loads(line)
        if message.get("reason") == "compiler-message":
            rendered = message.get("message", {}).get("rendered")
            if rendered:
                print(rendered, end="", file=sys.stderr)
        result.append(message)
    return result


def executable(records, name, is_test):
    candidates = [Path(record["executable"]) for record in records
                  if record.get("reason") == "compiler-artifact"
                  and record.get("target", {}).get("name") == name
                  and record.get("profile", {}).get("test") is is_test
                  and record.get("executable")]
    if len(candidates) != 1:
        raise ValueError(f"expected one Cargo executable for {name} (test={is_test})")
    return candidates[0]


def copy_executable(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    digest = hashlib.sha256()
    # Open first: Cargo may unlink/replace the shared pathname while the gate
    # copies. The descriptor retains the selected executable's original bytes.
    descriptor = os.open(source, os.O_RDONLY | getattr(os, "O_NONBLOCK", 0))
    with os.fdopen(descriptor, "rb") as reader:
        before = os.fstat(reader.fileno())
        if not stat.S_ISREG(before.st_mode) or not before.st_mode & 0o111:
            raise ValueError("Cargo test image requires regular executable files")
        with destination.open("xb") as writer:
            while chunk := reader.read(1024 * 1024):
                digest.update(chunk)
                writer.write(chunk)
        after = os.fstat(reader.fileno())
        if (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
            raise ValueError("build artifact changed in place while copying the test image")
    destination.chmod(0o700)
    return digest.hexdigest()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--diagnostics", type=Path)
    parser.add_argument("--build", type=Path)
    parser.add_argument("--tests", type=Path)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--helper", type=Path)
    args = parser.parse_args(argv)
    try:
        if args.diagnostics is not None:
            messages(args.diagnostics)
            return 0
        if args.build is None or args.tests is None or args.out is None:
            parser.error("copy requires --build, --tests, and --out")
        build, tests = messages(args.build), messages(args.tests)
        sources = {
            "debug/deps/angel-tests": executable(tests, "angel", True),
            "debug/angel": executable(build, "angel", False),
            "debug/angel-sandbox": args.helper or executable(build, "angel-sandbox", False),
        }
        hashes = {name: copy_executable(source, args.out / name) for name, source in sources.items()}
        (args.out / "image.json").write_text(json.dumps({"sha256": hashes}, indent=2) + "\n")
        return 0
    except (OSError, ValueError, TypeError, AttributeError) as error:
        print(f"could not prepare development test image: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
