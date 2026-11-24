"""Shared paths and helpers for the polyglot kit.

The kit's own files live beside this module. Everything it builds or downloads
(the upstream exercises, fixtures, the Python venv, node_modules) goes under the
work directory: $POLYGLOT_WORK, else bench/polyglot/work.
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path

KIT = Path(__file__).resolve().parent
KIT_VERSION = "polyglot-kit/1"

UPSTREAM_URL = "https://github.com/Aider-AI/polyglot-benchmark"
UPSTREAM_COMMIT = "7e0611e77b54e2dea774cdc0aa00cf9f7ed6144f"

LANG_ORDER = ("javascript", "python", "rust", "cpp")
LANG_SHORT = {"javascript": "js", "python": "py", "rust": "rust", "cpp": "cpp"}


def work_dir() -> Path:
    raw = os.environ.get("POLYGLOT_WORK", "").strip()
    return Path(raw).expanduser().resolve() if raw else KIT / "work"


def toolbin() -> Path:
    return work_dir() / "toolbin"


def tool_path() -> str:
    """PATH for agents and the grader: the kit's toolbin first, then the host's."""
    return f"{toolbin()}:{os.environ.get('PATH', '/usr/local/bin:/usr/bin:/bin')}"


def load_tasks() -> list[dict]:
    path = work_dir() / "tasks.json"
    if not path.exists():
        raise SystemExit(f"no task catalog at {path}: run `python3 {KIT}/polyglot.py build` first")
    return json.loads(path.read_text())


def load_grading() -> dict:
    return json.loads((work_dir() / "grading.json").read_text())


def catalog_digest(tasks: list[dict]) -> str:
    """SHA-256 over what an agent sees and what grading checks, in catalog order.
    Equal digests mean two machines built the identical benchmark."""
    rows = [
        [t["name"], t["prompt"], t["solution_paths"], t["protected_paths"], t["test_cmd"]]
        for t in tasks
    ]
    return hashlib.sha256(json.dumps(rows, sort_keys=True).encode()).hexdigest()


def select_tasks(tasks: list[dict], spec: str | None) -> list[dict]:
    """--tasks: comma list of task names, language prefixes (js, py, rust, cpp)
    or @file (a JSON list of names, or one name per line)."""
    if not spec:
        return tasks
    wanted: list[str] = []
    for part in spec.split(","):
        part = part.strip()
        if not part:
            continue
        if part.startswith("@"):
            text = Path(part[1:]).read_text()
            try:
                wanted += [str(x) for x in json.loads(text)]
            except json.JSONDecodeError:
                wanted += [line.strip() for line in text.splitlines() if line.strip()]
        else:
            wanted.append(part)
    by_name = {t["name"]: t for t in tasks}
    picked: list[dict] = []
    for want in wanted:
        if want in by_name:
            picked.append(by_name[want])
        elif want in LANG_SHORT.values():
            picked += [t for t in tasks if LANG_SHORT[t["language"]] == want]
        else:
            raise SystemExit(f"unknown task or language: {want}")
    seen: set[str] = set()
    return [t for t in picked if not (t["name"] in seen or seen.add(t["name"]))]


def read_jsonl(path: Path) -> list[dict]:
    if not path.exists():
        return []
    rows = []
    for line in path.read_text().splitlines():
        line = line.strip()
        if line:
            rows.append(json.loads(line))
    return rows
