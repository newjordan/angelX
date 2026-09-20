"""Owner-only local store writers; shared write-time credential redaction."""

from contextlib import contextmanager
import os
from pathlib import Path
import secrets
import stat
import runpy

_REDACTOR = None


def redact_text(text):
    """Apply the scanner rule; preserve clean bytes and reject sealed changes."""
    global _REDACTOR
    if _REDACTOR is None:
        _REDACTOR = runpy.run_path(str(Path(__file__).with_name("trajectory-redact.py")))
    values = _REDACTOR["known_values"]([Path(__file__).resolve().parent.parent / ".angel.env"])
    # JSONL needs per-record inspection so integrity bindings cannot be lost.
    import json
    try:
        json.loads(text)
    except ValueError:
        lines = text.splitlines(keepends=True)
        if lines and all(not line.strip() or _is_json(line) for line in lines):
            clean, _ = _REDACTOR["redact_document"](text, values, jsonl=True)
        else:
            clean, _ = _REDACTOR["redact_text"](text, values)
    else:
        clean, _ = _REDACTOR["redact_document"](text, values, jsonl=False)
    if any(_REDACTOR["scan_text"](clean, values)):
        raise ValueError("private write rejected by final credential scan")
    return clean


def _is_json(text):
    import json
    try:
        json.loads(text)
        return True
    except ValueError:
        return False


def restrict_process_creation():
    """Set a permanent private creation mask at dedicated process startup.

    Never temporarily change a shared worker process's umask around one job.
    Call this before starting any threads; preserve a stricter inherited mask.
    Explicit per-file checks still own existing-file migration and authority.
    """
    import threading

    if threading.current_thread() is not threading.main_thread() or threading.active_count() != 1:
        raise RuntimeError("private process policy must be set before starting threads")
    # First tighten every bit, so even a stricter inherited mask is never
    # transiently weakened while retrieving its value. No workers exist yet.
    previous = os.umask(0o777)
    restricted = previous | 0o077
    os.umask(restricted)
    return restricted


@contextmanager
def private_directory(path):
    """Hold a checked directory; only missing components are created at 0700."""
    path = Path(path)
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
    directory = os.open("/" if path.is_absolute() else ".", flags)
    try:
        for part in path.parts:
            if part in ("/", "."):
                continue
            try:
                child = os.open(part, flags, dir_fd=directory)
            except FileNotFoundError:
                try:
                    os.mkdir(part, 0o700, dir_fd=directory)
                except FileExistsError:
                    pass
                child = os.open(part, flags, dir_fd=directory)
            os.close(directory)
            directory = child
        if os.fstat(directory).st_uid != os.geteuid():
            raise PermissionError("private store directory belongs to another owner")
        yield directory
    finally:
        os.close(directory)


@contextmanager
def private_parent(path):
    """Hold a nofollow parent without changing unrelated ancestor modes."""
    path = Path(path)
    if path.name in ("", ".", ".."):
        raise ValueError("private store path needs a file name")
    with private_directory(path.parent) as directory:
        yield directory, path.name


def ensure_private_directory(path):
    """Migrate only an explicitly declared private directory through its FD."""
    with private_directory(path) as directory:
        if stat.S_IMODE(os.fstat(directory).st_mode) & 0o077:
            os.fchmod(directory, 0o700)


def _matches(directory, name, descriptor):
    named = os.stat(name, dir_fd=directory, follow_symlinks=False)
    opened = os.fstat(descriptor)
    if (
        (named.st_dev, named.st_ino) != (opened.st_dev, opened.st_ino)
        or not stat.S_ISREG(named.st_mode)
        or named.st_uid != os.geteuid()
        or named.st_nlink != 1
        or stat.S_IMODE(named.st_mode) != 0o600
    ):
        raise OSError("private store name no longer identifies its checked file")


def _checked(directory, name, flags):
    descriptor = os.open(
        name, flags | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC,
        0o600, dir_fd=directory,
    )
    try:
        metadata = os.fstat(descriptor)
        if (not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != os.geteuid()
                or metadata.st_nlink != 1):
            raise OSError("private store entry must be a singly linked owner file")
        # Existing owner files are migrated through the checked FD before bytes
        # are written. Unrelated parent modes and rejected targets stay intact.
        os.fchmod(descriptor, 0o600)
        _matches(directory, name, descriptor)
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


@contextmanager
def private_append(path):
    """Retain the checked parent and append FD for caller-owned locking."""
    with private_parent(path) as (directory, name):
        descriptor = _checked(directory, name, os.O_WRONLY | os.O_APPEND | os.O_CREAT)
        try:
            yield descriptor
        finally:
            os.close(descriptor)


def append_lines(path, lines, *, sync=True):
    with private_parent(path) as (directory, name):
        descriptor = _checked(directory, name, os.O_WRONLY | os.O_APPEND | os.O_CREAT)
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="") as stream:
            for line in lines:
                stream.write(redact_text(line) + "\n")
            stream.flush()
            if sync:
                os.fsync(stream.fileno())


def read_text(path, *, max_bytes=32 * 1024 * 1024):
    """Read a bounded private state file through a checked, nonblocking FD."""
    if not isinstance(max_bytes, int) or isinstance(max_bytes, bool) or max_bytes < 0:
        raise ValueError("private read bound must be a nonnegative integer")
    with private_parent(path) as (directory, name):
        descriptor = _checked(directory, name, os.O_RDONLY)
        with os.fdopen(descriptor, "rb") as stream:
            body = stream.read(max_bytes + 1)
        if len(body) > max_bytes:
            raise ValueError("private state exceeds read bound")
        return body.decode("utf-8")


def replace_text(path, text):
    """Publish redacted text; preserve the old file on validation/write failure."""
    _replace(path, redact_text(text))


def replace_bytes(path, body):
    """Preserve binary assets exactly; refuse any detected credential payload."""
    # Initialize the canonical scanner without serializing or altering bytes.
    redact_text("")
    values = _REDACTOR["known_values"]([Path(__file__).resolve().parent.parent / ".angel.env"])
    if any(_REDACTOR["scan_text"](body.decode("utf-8", errors="surrogateescape"), values)):
        raise ValueError("binary private write contains credentials")
    _replace(path, body)


def _replace(path, text):
    """Publish a complete 0600 file, preserving the old file on write failure."""
    with private_parent(path) as (directory, name):
        previous = temporary_fd = None
        temporary = None
        try:
            try:
                previous = _checked(directory, name, os.O_RDONLY)
            except FileNotFoundError:
                pass
            temporary = f".{name}.{os.getpid()}.{secrets.token_hex(8)}.tmp"
            temporary_fd = _checked(
                directory, temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL,
            )
            options = {} if isinstance(text, bytes) else {"encoding": "utf-8", "newline": ""}
            with os.fdopen(temporary_fd, "wb" if isinstance(text, bytes) else "w", closefd=False, **options) as stream:
                stream.write(text)
                stream.flush()
                os.fsync(temporary_fd)
            _matches(directory, temporary, temporary_fd)
            if previous is not None:
                _matches(directory, name, previous)
            else:
                try:
                    os.stat(name, dir_fd=directory, follow_symlinks=False)
                except FileNotFoundError:
                    pass
                else:
                    raise FileExistsError("private store destination appeared while staging")
            os.replace(temporary, name, src_dir_fd=directory, dst_dir_fd=directory)
            temporary = None
            os.fsync(directory)
        finally:
            if temporary is not None and temporary_fd is not None:
                try:
                    _matches(directory, temporary, temporary_fd)
                    os.unlink(temporary, dir_fd=directory)
                except OSError:
                    pass
            if temporary_fd is not None:
                os.close(temporary_fd)
            if previous is not None:
                os.close(previous)


@contextmanager
def text_writer(path, mode="w", *, encoding="utf-8", errors=None, newline=None):
    """Serialize text before publication; exceptions never publish partial data."""
    import io
    if mode not in ("w", "a") or encoding not in (None, "utf-8", "utf8"):
        raise ValueError("private text writer requires UTF-8 append or replace")
    with io.StringIO(newline=newline) as stream:
        yield stream
        clean = redact_text(stream.getvalue())
    if mode == "w":
        replace_text(path, clean)
    else:
        body = clean.encode("utf-8")
        with private_append(path) as descriptor:
            offset = 0
            while offset < len(body):
                written = os.write(descriptor, body[offset:])
                if written <= 0:
                    raise OSError("short private write")
                offset += written
            os.fsync(descriptor)


def replace_gzip_json(path, value):
    """Redact the decoded document before compression, never after sealing it."""
    import gzip
    import json
    clean = redact_text(json.dumps(value, ensure_ascii=False))
    _replace(path, gzip.compress(clean.encode("utf-8"), mtime=0))

def redact_records(records):
    """Per-record cache hygiene: redact secret-bearing string fields; DROP a record
    that is integrity-bound (carries sha256-style bindings) and secret-bearing, since
    rewriting it would break its digest authority. Returns (clean_records, dropped)."""
    global _REDACTOR
    if _REDACTOR is None:
        _REDACTOR = runpy.run_path(str(Path(__file__).with_name("trajectory-redact.py")))
    values = _REDACTOR["known_values"]([Path(__file__).resolve().parent.parent / ".angel.env"])
    clean, dropped = [], 0
    for record in records:
        item, count = _REDACTOR["redact_record"](record, values)
        if count and _REDACTOR["integrity_bound"](record):
            dropped += 1
            continue
        clean.append(item)
    return clean, dropped
