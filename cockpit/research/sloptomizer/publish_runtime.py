"""Publish one coherent Sloptomizer source generation for the native host.

Usage: python3 publish_runtime.py DIRECTORY
The directory must already contain the complete runtime dependency set. Save
edits first; the manifest is atomically replaced only after two matching reads.
"""

import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile

FILES = (
    "relations.py", "runner.py", "UPSTREAM.json",
    "autoresearch/__init__.py", "autoresearch/gepa/__init__.py",
    "autoresearch/gepa/population.py", "autoresearch/gepa/select.py",
    "orchestrator/__init__.py", "orchestrator/self_improvement/__init__.py",
    "orchestrator/self_improvement/bandits.py", "orchestrator/self_improvement/stats.py",
    "orchestrator/micro_llm/__init__.py", "orchestrator/micro_llm/core.py",
    "orchestrator/caseops_codec.py", "orchestrator/caseops_crystal_codec.py",
    "orchestrator/caseops_taxonomy_codec.py", "orchestrator/caseops_vocab_analogy_codec.py",
    "orchestrator/caseops_vocab_tensor_codec.py",
)


def snapshot(root):
    result = {}
    total = 0
    for name in FILES:
        path = root / name
        if path.is_symlink() or not path.is_file():
            raise ValueError(f"not a regular file: {name}")
        with path.open("rb") as source:
            data = source.read(2 * 1024 * 1024 + 1)
        if len(data) > 2 * 1024 * 1024:
            raise ValueError(f"file exceeds 2 MiB: {name}")
        total += len(data)
        if total > 8 * 1024 * 1024:
            raise ValueError("generation exceeds 8 MiB")
        result[name] = hashlib.sha256(data).hexdigest()
    return result


def publish(root):
    files = snapshot(root)
    if snapshot(root) != files:
        raise ValueError("runtime files changed while publishing; retry after saving")
    manifest = {"schema": "angel.sloptomizer-generation/v1", "files": files}
    fd, temporary = tempfile.mkstemp(prefix=".generation-", dir=root)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as output:
            json.dump(manifest, output, sort_keys=True, indent=2)
            output.write("\n")
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, root / "generation.json")
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: publish_runtime.py DIRECTORY")
    try:
        publish(Path(sys.argv[1]))
    except (OSError, ValueError) as error:
        raise SystemExit(f"research generation not published: {error}")
