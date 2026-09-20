#!/usr/bin/env python3
"""Scan or redact secrets in cockpit trajectory stores before they ship.

The cockpit redacts at write time (cockpit/src/secrets.rs); this script is the
same rule applied to what is already on disk and to the feed staging copy, so
nothing recorded before the write-time redaction landed can leave the box.

    trajectory-redact.py --scan DIR...            # report, exit 1 on any hit
    trajectory-redact.py --scan --json DIR...     # machine-readable summary
    trajectory-redact.py --redact SRC DST         # redacted copies into DST

Known values come from the process environment plus `.angel.env` next to the
repo (or `--env-file`), keeping the Rust rule: a variable whose name contains
KEY, TOKEN, SECRET, AUTH or PASSWORD (case-insensitive) and whose value is at
least 12 bytes. Values are never printed; only names and counts are.
"""

import argparse
import json
import os
import re
import sys
import stat
import runpy
from pathlib import Path

MIN_SECRET_LEN = 12
SECRET_WORDS = ("KEY", "TOKEN", "SECRET", "AUTH", "PASSWORD")
SHAPES = {
    "sk-prefix": re.compile(r"\bsk-[A-Za-z0-9_-]{16,}"),
    "zai-hex32.suffix": re.compile(r"\b[0-9a-f]{32}\.[A-Za-z0-9]{12,}\b"),
    "github-token": re.compile(r"\bgh[pousr]_[A-Za-z0-9]{30,}\b"),
    "github-pat": re.compile(r"\bgithub_pat_[A-Za-z0-9_]{20,}\b"),
    "huggingface-token": re.compile(r"\bhf_[A-Za-z0-9]{20,}\b"),
    "google-api-key": re.compile(r"\bAIza[A-Za-z0-9_-]{30,}\b"),
    "slack": re.compile(r"\bxox[abpr]-[A-Za-z0-9-]{10,}"),
    "aws-akia": re.compile(r"\bAKIA[0-9A-Z]{16}\b"),
    "jwt": re.compile(r"\beyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}(?:\.[A-Za-z0-9_-]{10,})?"),
    "pem-private-key": re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----"),
}
BEARER = re.compile(r"(?i)(bearer\s+)[A-Za-z0-9._~+/=-]{20,}")
REDACTED = "«redacted»"
ASSIGNMENT = re.compile(r'''(?i)(\b(?:[a-z0-9]+_)*(?:api[_-]?key|access_token|refresh_token|client_secret|token|password|passwd|secret)\s*[=:]\s*)(?:"[^"\r\n]+"|'[^'\r\n]+'|[^\s,;\}\]]+)''')


def credential_field(name):
    name = name.lower().replace("-", "_")
    return name in {"token", "secret", "password", "passwd", "authorization", "api_key", "apikey", "auth", "access_token", "refresh_token", "client_secret"} or name.endswith(("_api_key", "_access_token", "_refresh_token", "_client_secret", "_password"))


def is_secret_name(name: str) -> bool:
    up = name.upper()
    return any(w in up for w in SECRET_WORDS)


def load_env_file(path: Path) -> dict:
    out = {}
    if not path.is_file():
        return out
    try:
        body = safe_read(path)
    except FileNotFoundError:
        return out
    for raw in body.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("export "):
            line = line[7:].strip()
        if "=" not in line:
            continue
        name, value = line.split("=", 1)
        name = name.strip()
        value = value.strip()
        if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
            value = value[1:-1]
        if name and value:
            out[name] = value
    return out


def known_values(env_files):
    # Retain both current environment and saved values when a name was rotated.
    pairs = set(os.environ.items())
    for f in env_files:
        pairs.update(load_env_file(Path(f)).items())
    pairs = [
        (n, v)
        for n, v in pairs
        if is_secret_name(n)
        and len(v.encode("utf-8")) >= MIN_SECRET_LEN
        and not v.startswith("«redacted")
        and not v.startswith(("/", "~", "."))  # *_KEY_FILE holds a path, not a key
    ]
    # Longest first so a value that embeds another is replaced whole.
    pairs.sort(key=lambda nv: (-len(nv[1]), nv[0]))
    return pairs


MAX_SCAN_BYTES = 32 * 1024 * 1024


def safe_read(path, max_bytes=MAX_SCAN_BYTES):
    """Read-only, nofollow at every component; no chmod, mkdir or FIFO wait."""
    path = Path(path).absolute()
    descriptor = os.open("/", os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
    try:
        for part in path.parts[1:-1]:
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
                            dir_fd=descriptor)
            os.close(descriptor)
            descriptor = child
        child = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC,
                        dir_fd=descriptor)
        with os.fdopen(child, "rb") as stream:
            before = os.fstat(stream.fileno())
            if not stat.S_ISREG(before.st_mode):
                raise ValueError("nonregular")
            if before.st_size > max_bytes:
                raise ValueError("oversize")
            body = stream.read(max_bytes + 1)
            after = os.fstat(stream.fileno())
            if len(body) > max_bytes:
                raise ValueError("oversize")
            if (before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (after.st_size, after.st_mtime_ns, after.st_ctime_ns):
                raise ValueError("changed-during-read")
            return body.decode("utf-8", errors="surrogateescape")
    finally:
        os.close(descriptor)


def iter_files(dirs, skipped=None):
    """All extensions, with skips explicit; directory descriptors forbid races."""
    skipped = [] if skipped is None else skipped

    def reject(path, reason):
        skipped.append({"file": str(path), "reason": reason})

    def walk(path):
        # Reopen through each ancestor using nofollow. No symlinked directory
        # can redirect enumeration outside the requested store.
        descriptor = os.open("/", os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
        try:
            for part in path.absolute().parts[1:]:
                child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
                                dir_fd=descriptor)
                os.close(descriptor)
                descriptor = child
            for name in sorted(os.listdir(descriptor)):
                if name in {"off-limits", ".git"}:
                    reject(path / name, "excluded-directory")
                    continue
                child = path / name
                info = os.stat(name, dir_fd=descriptor, follow_symlinks=False)
                if stat.S_ISDIR(info.st_mode):
                    yield from walk(child)
                elif stat.S_ISREG(info.st_mode):
                    yield child
                else:
                    reject(child, "symlink" if stat.S_ISLNK(info.st_mode) else "nonregular")
        except OSError:
            reject(path, "unreadable-or-unsafe")
        finally:
            os.close(descriptor)

    for directory in dirs:
        path = Path(directory)
        try:
            mode = path.lstat().st_mode
            if stat.S_ISDIR(mode):
                yield from walk(path)
            elif stat.S_ISREG(mode):
                yield path
            else:
                reject(path, "symlink" if stat.S_ISLNK(mode) else "nonregular")
        except OSError:
            reject(path, "missing-or-unreadable")


def scan_text(text: str, values):
    # Decode JSON strings so quotes, backslashes and Unicode escapes cannot
    # hide a known value. The scanner also accepts non-JSON diagnostic text.
    texts = []
    field_hits = 0

    def visit(value, credential=False):
        nonlocal field_hits
        if isinstance(value, str):
            if value.lstrip().startswith(("{", "[", '"')):
                try:
                    nested = json.loads(value)
                except ValueError:
                    texts.append(value)
                else:
                    visit(nested)
            else:
                texts.append(value)
            if credential and value and not value.startswith("«redacted"):
                field_hits += 1
        elif isinstance(value, list):
            for item in value:
                visit(item, credential)
        elif isinstance(value, dict):
            for key, item in value.items():
                visit(key)
                visit(item, credential_field(key))

    try:
        visit(json.loads(text))
    except ValueError:
        for line in text.splitlines():
            try:
                visit(json.loads(line))
            except ValueError:
                texts.append(line)
    text = "\n".join(texts)
    literal = {}
    for name, value in values:
        n = text.count(value)
        if n:
            literal[name] = n
    shapes = {}
    for label, rx in SHAPES.items():
        n = len(rx.findall(text))
        if n:
            shapes[label] = n
    n = len(BEARER.findall(text))
    if n:
        shapes["bearer"] = n
    if field_hits:
        shapes["credential-field"] = field_hits
    n = sum(not ("«redacted" in m[0] or m[0].endswith("…")) for m in ASSIGNMENT.finditer(text))
    if n:
        shapes["credential-assignment"] = n
    return literal, shapes


def redact_text(text: str, values):
    if text.lstrip().startswith(("{", "[", '"')):
        try:
            nested = json.loads(text)
        except ValueError:
            pass
        else:
            clean, count = redact_record(nested, values)
            if count:
                return json.dumps(clean, ensure_ascii=False), count
    hits = 0
    for name, value in values:
        if value in text:
            hits += text.count(value)
            text = text.replace(value, f"«redacted:{name}»")
    for rx in SHAPES.values():
        text, n = rx.subn(REDACTED, text)
        hits += n
    text, n = BEARER.subn(lambda m: m.group(1) + REDACTED, text)
    hits += n
    def assignment(match):
        nonlocal hits
        if "«redacted" in match[0] or match[0].endswith("…"):
            return match[0]
        hits += 1
        return match[1] + REDACTED
    text = ASSIGNMENT.sub(assignment, text)
    return text, hits


def redact_record(value, values, credential=False):
    """Redact decoded string values while retaining all structural types."""
    if isinstance(value, str):
        if credential and value and not value.startswith("«redacted"):
            return REDACTED, 1
        return redact_text(value, values)
    if isinstance(value, (dict, list)):
        hits = 0
        clean = {} if isinstance(value, dict) else []
        items = value.items() if isinstance(value, dict) else enumerate(value)
        for key, item in items:
            child, n = redact_record(item, values, credential_field(key) if isinstance(value, dict) else credential)
            hits += n
            if isinstance(value, dict):
                clean_key, key_hits = redact_text(key, values)
                hits += key_hits
                if clean_key in clean:
                    raise ValueError("redaction would collide object keys")
                clean[clean_key] = child
            else:
                clean.append(child)
        return clean, hits
    return value, 0


REJECTIONS_RECEIPT = ".rejections.json"


def integrity_bound(value):
    """A sealed rollout must be rejected, never silently rewritten/resigned."""
    if isinstance(value, dict):
        bindings = {"manifest_sha256", "source_manifest_sha256", "event_sha256", "receipt_sha256", "record_sha256", "journal_head_sha256"}
        return any((key in bindings or key == "sha256" or key.endswith("_sha256")) and item for key, item in value.items()) or any(integrity_bound(item) for item in value.values())
    if isinstance(value, str) and value.lstrip().startswith(("{", "[", '"')):
        try:
            return integrity_bound(json.loads(value))
        except ValueError:
            return False
    return isinstance(value, list) and any(integrity_bound(item) for item in value)


def redact_document(text, values, *, jsonl, rejections=None):
    """Redact one JSON document or a JSONL stream.

    With ``rejections`` (a dict, one per document) a JSONL record that cannot be
    staged safely is dropped from the output and counted there instead of
    aborting the whole corpus: a line that does not parse (nothing structural
    to redact, so it cannot be certified clean) and a secret-bearing
    integrity-bound record (a sealed rollout is rejected, never rewritten or
    resigned). Without ``rejections`` the historical contract holds: either
    condition raises ValueError. One bad record must not starve the feed of
    every clean one; the feed ships the rest and the receipt names the rest.
    """
    malformed = 0
    original_records = []
    if jsonl:
        records = []
        for line in text.splitlines(keepends=True):
            if not line.strip():
                continue
            try:
                records.append(json.loads(line))
                original_records.append(line)
            except ValueError:
                if rejections is None:
                    raise
                malformed += 1
    else:
        records = [json.loads(text)]
    clean = []
    hits = 0
    sealed = 0
    clean_lines = []
    for index, record in enumerate(records):
        item, count = redact_record(record, values)
        if count and integrity_bound(record):
            if rejections is None:
                raise ValueError("secret-bearing integrity-bound record requires rejection or recapture")
            sealed += 1
            continue
        clean.append(item)
        if jsonl:
            clean_lines.append(original_records[index] if not count else json.dumps(item, ensure_ascii=False) + "\n")
        hits += count
    if rejections is not None:
        if malformed:
            rejections["malformed"] = rejections.get("malformed", 0) + malformed
        if sealed:
            rejections["sealed_secret"] = rejections.get("sealed_secret", 0) + sealed
    if not hits and not malformed and not sealed:
        return text, 0
    if jsonl:
        return "".join(clean_lines), hits
    return json.dumps(clean[0], ensure_ascii=False) + "\n", hits


def scan_summary(dirs, values):
    files = 0
    files_with_hits = 0
    by_name = {}
    by_shape = {}
    hit_files = []
    skipped = []
    for f in iter_files(dirs, skipped):
        if f.suffix.lower() in (".gz", ".zst"):
            skipped.append({"file": str(f), "reason": "compressed"})
            continue
        try:
            text = safe_read(f)
        except (OSError, ValueError) as error:
            skipped.append({"file": str(f), "reason": str(error) if isinstance(error, ValueError) else "unreadable-or-unsafe"})
            continue
        if "\0" in text[:8192]:
            skipped.append({"file": str(f), "reason": "binary"})
            continue
        files += 1
        try:
            literal, shapes = scan_text(text, values)
        except (ValueError, RecursionError):
            skipped.append({"file": str(f), "reason": "parse-depth-or-invalid-document"})
            continue
        if literal or shapes:
            files_with_hits += 1
            hit_files.append({"file": str(f), "literal": literal, "shapes": shapes})
        for k, v in literal.items():
            by_name[k] = by_name.get(k, 0) + v
        for k, v in shapes.items():
            by_shape[k] = by_shape.get(k, 0) + v
    summary = {
        "dirs": [str(d) for d in dirs],
        "files_scanned": files,
        "files_with_hits": files_with_hits,
        "known_names_loaded": len(values),
        "literal_by_name": by_name,
        "literal_total": sum(by_name.values()),
        "shapes_by_kind": by_shape,
        "shapes_total": sum(by_shape.values()),
        "files": hit_files,
    }
    summary["skipped"] = skipped
    summary["files_skipped"] = len(skipped)
    summary["complete"] = not skipped
    return summary


def do_scan(dirs, values, as_json):
    summary = scan_summary(dirs, values)
    if as_json:
        print(json.dumps(summary, indent=2))
    else:
        for key in ("files_scanned", "files_with_hits", "known_names_loaded", "literal_total", "shapes_total", "files_skipped"):
            print(f"{key}: {summary[key]}")
    return 2 if not summary["complete"] else int(bool(summary["literal_total"] or summary["shapes_total"]))

def do_redact(src, dst, values):
    src = Path(src)
    dst = Path(dst)
    if not src.is_dir():
        raise ValueError("staging source must be an existing directory")
    source_root, target_root = src.resolve(), dst.resolve()
    if source_root == target_root or source_root in target_root.parents or target_root in source_root.parents:
        raise ValueError("source and staging directories must be disjoint")
    if src.is_symlink() or dst.is_symlink():
        raise ValueError("staging roots must not be symbolic links")
    private = runpy.run_path(str(Path(__file__).with_name("private_store_io.py")))
    private["ensure_private_directory"](dst)
    wrote = 0
    skipped = 0
    hits = 0
    names = set()
    rejected = {}
    source_skips = []
    sources = list(iter_files([src], source_skips))
    if source_skips:
        raise ValueError("unsafe source enumeration")
    for f in sources:
        if f.parent != src or f.suffix != ".jsonl":
            continue
        if f.is_symlink():
            raise ValueError("staging source must not contain symbolic links")
        key = f.name
        target = dst / f.name
        if target.is_symlink():
            raise ValueError("staging target must not be a symbolic link")
        text = safe_read(f)
        per_file = {}
        clean, n = redact_document(text, values, jsonl=True, rejections=per_file)
        literal, shapes = scan_text(clean, values)
        if literal or shapes:
            raise ValueError("staged export failed final secret scan")
        if per_file:
            rejected[key] = per_file
        # Re-evaluate every source against current credentials and patterns.
        # An unchanged mtime/size cannot certify a prior staged copy as safe.
        if target.exists() and safe_read(target) == clean:
            with private["private_parent"](target) as (directory, name):
                descriptor = private["_checked"](directory, name, os.O_RDONLY)
                os.close(descriptor)
            skipped += 1
            names.add(key)
            continue
        private["replace_text"](target, clean)
        names.add(key)
        wrote += 1
        hits += n
    # Drop staged copies whose source is gone so the feed never re-ships them.
    removed = 0
    for g in dst.glob("*.jsonl"):
        if g.name not in names:
            with private["private_parent"](g) as (directory, name):
                os.unlink(name, dir_fd=directory)
            removed += 1
    # Counts-only receipt of what was dropped from the staged copy, so the
    # operator can recapture or delete the source; never the record bodies.
    receipt = dst / REJECTIONS_RECEIPT
    if receipt.is_symlink():
        raise ValueError("staging receipt must not be a symbolic link")
    if rejected:
        private["replace_text"](receipt, json.dumps(rejected, indent=2, sort_keys=True) + "\n")
    elif receipt.exists():
        with private["private_parent"](receipt) as (directory, name):
            os.unlink(name, dir_fd=directory)
    malformed = sum(v.get("malformed", 0) for v in rejected.values())
    sealed = sum(v.get("sealed_secret", 0) for v in rejected.values())
    print(
        f"redacted {wrote} files ({hits} replacements), {skipped} up to date, {removed} stale removed, "
        f"{malformed + sealed} records rejected ({malformed} malformed, {sealed} sealed secret-bearing) "
        f"in {len(rejected)} files -> {dst}"
    )
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--scan", action="store_true")
    ap.add_argument("--redact", action="store_true")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--env-file", action="append", default=[])
    ap.add_argument("--no-default-env", action="store_true", help="do not read the repo .angel.env")
    ap.add_argument("paths", nargs="+")
    a = ap.parse_args()
    env_files = list(a.env_file)
    if not a.no_default_env:
        env_files.append(str(Path(__file__).resolve().parent.parent / ".angel.env"))
    values = known_values(env_files)
    if a.scan:
        return do_scan(a.paths, values, a.json)
    if a.redact:
        if len(a.paths) != 2:
            ap.error("--redact takes SRC DST")
        try:
            return do_redact(a.paths[0], a.paths[1], values)
        except (OSError, ValueError):
            # Parser errors can include credential substrings. Never echo them.
            print("redaction failed: invalid source, unsafe staging path, or secret-bearing sealed record", file=sys.stderr)
            return 2
    ap.error("choose --scan or --redact")


if __name__ == "__main__":
    sys.exit(main())
