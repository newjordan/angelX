"""Summarize timestamped strace -f lines with process-local descriptor lifetimes.

Successful close/dup operations update known paths. Fork/CLONE_FILES sharing,
exec-time descriptor closure and unfinished/resumed syscall decoding are not
modeled; unknown targets remain explicit rather than inferred from another PID.
"""
import argparse
import collections
import json
from pathlib import Path
import re


def norm(path):
    path = re.sub(r"/tmp/angel-bench-(ws|home)-\w+", r"<\1>", path)
    return re.sub(r"\d{6,}", "N", path)


def analyze(lines):
    counts = {name: collections.Counter() for name in ("opens", "wbytes", "wcalls", "execs", "syncs", "renames")}
    fdmap = {}
    metadata = {"lines": 0, "unmatched_lines": 0, "unfinished_or_resumed_lines": 0,
                "successful_closes": 0, "successful_duplications": 0, "unknown_write_bytes": 0,
                "unknown_write_calls": 0, "unknown_sync_calls": 0}

    def target(pid, fd):
        return fdmap.get((pid, fd), f"fd{fd} [unknown]")

    def duplicate(pid, source, destination):
        if (pid, source) in fdmap:
            fdmap[(pid, destination)] = fdmap[(pid, source)]
        else:
            fdmap.pop((pid, destination), None)
        metadata["successful_duplications"] += 1

    for line in lines:
        metadata["lines"] += 1
        match = re.match(r"(\d+)\s+\S+\s+(.*)", line)
        if not match:
            metadata["unmatched_lines"] += 1
            continue
        pid, rest = match.groups()
        if "<unfinished ...>" in rest or " resumed>" in rest:
            metadata["unfinished_or_resumed_lines"] += 1
            continue
        if rest.startswith("openat("):
            opened = re.search(r'openat\([^,]+, "([^"]*)".*= (\d+)$', rest)
            if opened:
                path, fd = opened.groups()
                fdmap[(pid, fd)] = norm(path)
                counts["opens"][norm(path)] += 1
        elif rest.startswith("close("):
            closed = re.match(r"close\((\d+)\)\s+=\s+0(?:\s|$)", rest)
            if closed:
                fdmap.pop((pid, closed.group(1)), None)
                metadata["successful_closes"] += 1
        elif rest.startswith("dup("):
            duplicated = re.match(r"dup\((\d+)\)\s+=\s+(\d+)(?:\s|$)", rest)
            if duplicated:
                duplicate(pid, *duplicated.groups())
        elif rest.startswith(("dup2(", "dup3(")):
            duplicated = re.match(r"dup[23]\((\d+),\s*(\d+)(?:,\s*[^)]*)?\)\s+=\s+(\d+)(?:\s|$)", rest)
            if duplicated:
                source, destination, returned = duplicated.groups()
                if destination == returned:
                    duplicate(pid, source, destination)
        elif rest.startswith(("fcntl(", "fcntl64(")):
            duplicated = re.match(r"fcntl(?:64)?\((\d+),\s*F_DUPFD(?:_CLOEXEC)?,.*\)\s+=\s+(\d+)(?:\s|$)", rest)
            if duplicated:
                duplicate(pid, *duplicated.groups())
        elif rest.startswith(("write(", "pwrite64(")):
            written = re.match(r"p?write(?:64)?\((\d+),.*= (\d+)$", rest)
            if written:
                fd, size = written.groups()
                size = int(size)
                path = target(pid, fd)
                counts["wbytes"][path] += size
                counts["wcalls"][path] += 1
                if (pid, fd) not in fdmap:
                    metadata["unknown_write_bytes"] += size
                    metadata["unknown_write_calls"] += 1
        elif rest.startswith("execve("):
            executed = re.search(r'execve\("([^"]*)"', rest)
            counts["execs"][executed.group(1) if executed else "?"] += 1
        elif rest.startswith(("fsync", "fdatasync")):
            synced = re.match(r"\w+\((\d+)", rest)
            if synced:
                fd = synced.group(1)
                counts["syncs"][target(pid, fd)] += 1
                if (pid, fd) not in fdmap:
                    metadata["unknown_sync_calls"] += 1
        elif rest.startswith("rename("):
            renamed = re.search(r'rename\("[^"]*", "([^"]*)"', rest)
            counts["renames"][norm(renamed.group(1)) if renamed else "?"] += 1
    return {**counts, "coverage": metadata,
            "scope": "successful process-local close and descriptor duplication; no fork/shared-table, exec-close or unfinished-syscall reconstruction"}


def render(result):
    lines = ["== bytes written by path (top 25)"]
    lines.extend(f"{size:>12,} {result['wcalls'][path]:>6} {path}" for path, size in result["wbytes"].most_common(25))
    for title, field, limit in (("opens (top 25)", "opens", 25), ("execs", "execs", 15),
                                ("fsyncs", "syncs", 10), ("renames", "renames", 10)):
        lines.append("== " + title)
        lines.extend(f"{count:>6} {path}" for path, count in result[field].most_common(limit))
    return "\n".join(lines) + "\n"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("path", type=Path)
    parser.add_argument("--audit-json", action="store_true")
    args = parser.parse_args(argv)
    try:
        with args.path.open() as source:
            result = analyze(source)
    except (OSError, UnicodeError) as error:
        parser.error(str(error))
    if args.audit_json:
        print(json.dumps(result, indent=1, allow_nan=False))
    else:
        print(render(result), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
