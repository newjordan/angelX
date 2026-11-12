#!/usr/bin/env python3
"""Check angelX's plugin registry (plugins/registry.toml) the way a reviewer would.

    python3 scripts/plugins/check-registry.py                  # every entry, fetched
    python3 scripts/plugins/check-registry.py --changed BASE   # entries that differ from git BASE (a PR: origin/main)
    python3 scripts/plugins/check-registry.py --offline        # the file's own rules only

For each entry it checks the fields, then fetches the plugin's folder at the
pinned commit (shallow and partial: only that folder's files), recomputes the
plugin tree digest (cockpit/src/app/plugins/digest.rs) and reads the plugin's
cartridge.toml there: the id, label and [plugin] fields must agree with the
entry, the plugin must be config-only, and every command's skill and every
tool's program must be in the folder. Across the registry, no two plugins may
claim the same slash command or tool name. Nothing fetched is ever run.

Needs Python 3.11+ (tomllib) and git. Exit 0 when the registry passes, 1 when
it does not, 2 on a usage error.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import re
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

SCHEME = "angelx-plugin-tree/v1"
REGISTRY = "plugins/registry.toml"
ID = re.compile(r"^[a-z][a-z0-9-]{0,30}[a-z0-9]$")
VERSION = re.compile(r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?$")
COMMIT = re.compile(r"^[0-9a-f]{40}$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")
WORD = re.compile(r"^[a-z0-9-]+$")
TOOL = re.compile(r"^[a-z0-9_]+$")
FIELDS = {"label", "version", "description", "repository", "commit", "path", "sha256",
          "license", "authors", "keywords", "homepage"}
REQUIRED = FIELDS - {"path", "keywords", "homepage"}
CARTRIDGE_FIELDS = {"id", "label", "boards", "measures", "worker", "status_check",
                    "fleet_interval_secs", "fleet_command", "pages", "commands", "tools", "plugin"}
PLUGIN_FIELDS = {"version", "description", "authors", "license", "repository", "homepage",
                 "keywords", "angelx", "setup"}
RECEIPT = ".angelx-plugin.toml"


def digest(files: list[tuple[str, str, bytes]]) -> str:
    """The plugin tree digest of (path, kind, bytes) triples; kind is f, x or l."""
    h = hashlib.sha256()
    h.update(SCHEME.encode() + b"\n")
    for path, kind, data in sorted(files, key=lambda f: f[0].encode()):
        h.update(f"{kind} {path}\0{hashlib.sha256(data).hexdigest()}\n".encode())
    return h.hexdigest()


def safe_path(path: str) -> bool:
    return bool(path) and not path.startswith("/") and all(
        part not in ("", ".", "..", ".git") and "\0" not in part and "\\" not in part
        for part in path.split("/"))


def link_inside(path: str, target: str) -> bool:
    if not target or target.startswith("/"):
        return False
    depth = path.count("/")
    for part in target.split("/"):
        if part in ("", "."):
            continue
        depth += -1 if part == ".." else 1
        if depth < 0:
            return False
    return True


def git(cwd: Path, *args: str, stdin: bytes | None = None, lazy: bool = True) -> bytes:
    env = dict(os.environ, GIT_TERMINAL_PROMPT="0", LC_ALL="C")
    if not lazy:
        env["GIT_NO_LAZY_FETCH"] = "1"
    run = subprocess.run(["git", *args], cwd=cwd, input=stdin, capture_output=True, env=env)
    if run.returncode != 0:
        tail = " · ".join(run.stderr.decode(errors="replace").strip().splitlines()[-3:])
        raise RuntimeError(f"git {args[0]} failed: {tail or run.returncode}")
    return run.stdout


def fetch(url: str, commit: str, path: str, scratch: Path) -> list[tuple[str, str, bytes]]:
    """The files of `path` at `commit` of `url`, as the installer fetches them."""
    git(scratch, "init", "-q", "--bare", ".")
    for key, value in (("remote.origin.url", url), ("remote.origin.promisor", "true"),
                       ("remote.origin.partialclonefilter", "blob:none")):
        git(scratch, "config", key, value)
    try:
        git(scratch, "fetch", "-q", "--no-tags", "--depth", "1", "--filter=blob:none", "origin", commit)
    except RuntimeError:
        git(scratch, "fetch", "-q", "--no-tags", "--filter=blob:none", "origin",
            "+refs/heads/*:refs/remotes/origin/*")
    found = git(scratch, "rev-parse", "--verify", f"{commit}^{{commit}}").decode().strip()
    if found != commit:
        raise RuntimeError(f"{commit} resolves to {found}")
    tree = f"{commit}:{path}" if path else f"{commit}^{{tree}}"
    listing = git(scratch, "ls-tree", "-r", "-z", tree)
    records = []
    for record in filter(None, listing.split(b"\0")):
        meta, name = record.decode().split("\t", 1)
        mode, kind, oid = meta.split(" ")
        if not safe_path(name):
            raise RuntimeError(f"`{name}` is not a path a plugin may carry")
        if mode == "160000":
            raise RuntimeError(f"`{name}` is a git submodule")
        if kind != "blob":
            raise RuntimeError(f"`{name}` has git mode {mode} {kind}")
        records.append((name, "x" if mode == "100755" else "l" if mode == "120000" else "f", oid))
    if not records:
        raise RuntimeError(f"nothing in `{path}`")
    query = "".join(f"{oid}\n" for _, _, oid in records).encode()
    check = git(scratch, "cat-file", "--batch-check", stdin=query, lazy=False).decode()
    missing = [line.split(" ")[0] for line in check.splitlines() if line.endswith(" missing")]
    for at in range(0, len(missing), 256):
        git(scratch, "-c", "fetch.negotiationAlgorithm=noop", "fetch", "-q", "--no-tags",
            "--no-write-fetch-head", "--filter=blob:none", "origin", *missing[at:at + 256])
    out = git(scratch, "cat-file", "--batch", stdin=query)
    files, at = [], 0
    for name, kind, oid in records:
        end = out.index(b"\n", at)
        got, typ, size = out[at:end].decode().split(" ")
        if got != oid or typ != "blob":
            raise RuntimeError(f"cannot read {oid}")
        start = end + 1
        files.append((name, kind, out[start:start + int(size)]))
        at = start + int(size) + 1
    return files


def skill_names(files: dict[str, tuple[str, bytes]]) -> set[str]:
    """Skill names under skills/: <dir>/SKILL.md or <name>.md, by frontmatter name."""
    names = set()
    for path, (_, data) in files.items():
        parts = path.split("/")
        if parts[0] != "skills":
            continue
        if len(parts) == 3 and parts[2] == "SKILL.md":
            fallback = parts[1]
        elif len(parts) == 2 and parts[1].endswith(".md") and parts[1].lower() != "readme.md":
            fallback = parts[1][:-3]
        else:
            continue
        name = fallback
        text = data.decode(errors="replace")
        if text.startswith("---"):
            for line in text.split("\n")[1:]:
                if line.strip() == "---":
                    break
                if line.startswith("name:"):
                    name = line[5:].strip().strip("'\"") or fallback
        names.add(name)
    return names


def entry_problems(pid: str, entry: dict) -> list[str]:
    p = []
    if not ID.match(pid):
        p.append(f"`{pid}`: an id is 2-32 lowercase letters, digits and `-`, starting with a letter")
    unknown = set(entry) - FIELDS
    if unknown:
        p.append(f"{pid}: unknown field(s) {', '.join(sorted(unknown))}")
    for name in sorted(REQUIRED - set(entry)):
        p.append(f"{pid}: no {name}")
    if p:
        return p
    text = lambda key: isinstance(entry.get(key), str) and entry[key].strip()
    for key in ("label", "license"):
        if not text(key):
            p.append(f"{pid}: no {key}")
    if not isinstance(entry["version"], str) or not VERSION.match(entry["version"]):
        p.append(f"{pid}: version `{entry['version']}` is not MAJOR.MINOR.PATCH")
    d = entry["description"]
    if not isinstance(d, str) or not d.strip():
        p.append(f"{pid}: no description")
    elif "\n" in d or len(d) > 200:
        p.append(f"{pid}: the description is one line of at most 200 characters")
    url = entry["repository"]
    host = url.split("://", 1)[1].split("/", 1)[0] if isinstance(url, str) and "://" in url else ""
    if not isinstance(url, str) or not url.startswith("https://") or len(url) <= 8 \
            or any(c.isspace() for c in url) or url[8] == "-":
        p.append(f"{pid}: repository `{url}` is not a public https URL")
    elif "@" in host:
        p.append(f"{pid}: repository `{url}` carries credentials")
    if not isinstance(entry["commit"], str) or not COMMIT.match(entry["commit"]):
        p.append(f"{pid}: commit is not a full 40-character commit id")
    path = entry.get("path", "")
    if not isinstance(path, str) or (path and not safe_path(path)):
        p.append(f"{pid}: path `{path}` is not a folder inside the repository")
    if not isinstance(entry["sha256"], str) or not SHA256.match(entry["sha256"]):
        p.append(f"{pid}: sha256 is not 64 lowercase hex digits")
    authors = entry["authors"]
    if not isinstance(authors, list) or not any(isinstance(a, str) and a.strip() for a in authors):
        p.append(f"{pid}: no authors")
    for key in ("keywords",):
        if key in entry and not (isinstance(entry[key], list) and all(isinstance(k, str) for k in entry[key])):
            p.append(f"{pid}: {key} is a list of words")
    if "homepage" in entry and not (isinstance(entry["homepage"], str) and entry["homepage"].startswith("https://")):
        p.append(f"{pid}: homepage is an https URL")
    return p


def plugin_problems(pid: str, entry: dict, files: list[tuple[str, str, bytes]]) -> tuple[list[str], dict | None]:
    """What is wrong with the fetched plugin against its entry; and its cartridge."""
    p = []
    got = digest(files)
    if got != entry["sha256"]:
        p.append(f"{pid}: the files at {entry['commit'][:12]} digest to {got}, the entry says {entry['sha256']}")
    by_path = {path: (kind, data) for path, kind, data in files}
    if "cartridge.toml" not in by_path:
        return p + [f"{pid}: no cartridge.toml in `{entry.get('path', '')}`"], None
    try:
        cart = tomllib.loads(by_path["cartridge.toml"][1].decode())
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        return p + [f"{pid}: cartridge.toml: {error}"], None
    unknown = set(cart) - CARTRIDGE_FIELDS
    if unknown:
        p.append(f"{pid}: cartridge.toml has unknown field(s) {', '.join(sorted(unknown))}")
    if cart.get("id") != pid:
        p.append(f"{pid}: its cartridge.toml id is `{cart.get('id')}`")
    if cart.get("label") != entry["label"]:
        p.append(f"{pid}: label `{entry['label']}` differs from cartridge.toml's `{cart.get('label')}`")
    plugin = cart.get("plugin")
    if not isinstance(plugin, dict):
        p.append(f"{pid}: cartridge.toml has no [plugin] table")
        plugin = {}
    unknown = set(plugin) - PLUGIN_FIELDS
    if unknown:
        p.append(f"{pid}: [plugin] has unknown field(s) {', '.join(sorted(unknown))}")
    for key in ("version", "description", "license", "authors"):
        if plugin and plugin.get(key) != entry[key]:
            p.append(f"{pid}: {key} {entry[key]!r} differs from [plugin] {key} {plugin.get(key)!r}")
    for key in ("keywords", "homepage"):
        if plugin and plugin.get(key) != entry.get(key):
            p.append(f"{pid}: {key} {entry.get(key)!r} differs from [plugin] {key} {plugin.get(key)!r}")
    if "src/mod.rs" in by_path:
        p.append(f"{pid}: src/mod.rs makes it a source cartridge; a plugin is config-only")
    if RECEIPT in by_path:
        p.append(f"{pid}: it carries {RECEIPT}, the install receipt's name")
    for path, (kind, data) in by_path.items():
        if kind == "l" and not link_inside(path, data.decode(errors="replace")):
            p.append(f"{pid}: {path} links outside the plugin")
    skills = skill_names(by_path)
    setup = plugin.get("setup")
    for command in cart.get("commands", []):
        name = command.get("name", "")
        if not WORD.match(name):
            p.append(f"{pid}: command `{name}` is not a lowercase word")
        for skill in command.get("skills", []):
            if skill not in skills and not setup:
                p.append(f"{pid}: /{name}: skill `{skill}` is not in skills/")
    for tool in cart.get("tools", []):
        name, run = tool.get("name", ""), tool.get("run", "")
        if not TOOL.match(name):
            p.append(f"{pid}: tool `{name}` is not a lowercase name")
        if not safe_path(run):
            p.append(f"{pid}: tool {name} runs `{run}`, not a path inside the plugin")
        elif run not in by_path:
            if not setup:
                p.append(f"{pid}: tool {name}: {run} is not in the folder")
        elif by_path[run][0] == "f":
            p.append(f"{pid}: tool {name}: {run} is not executable (git mode 100755)")
    if setup is not None:
        if not isinstance(setup, str) or not safe_path(setup) or setup not in by_path:
            p.append(f"{pid}: [plugin] setup `{setup}` is not a file in the plugin")
        elif by_path[setup][0] == "f":
            p.append(f"{pid}: [plugin] setup {setup} is not executable (git mode 100755)")
    if not (cart.get("commands") or cart.get("tools") or cart.get("pages") or cart.get("boards") or cart.get("worker")):
        p.append(f"{pid}: it carries nothing")
    return p, cart


def changed_ids(base: str, current: dict) -> set[str]:
    try:
        old_text = git(Path.cwd(), "show", f"{base}:{REGISTRY}").decode()
    except RuntimeError:
        return set(current)
    old = tomllib.loads(old_text)
    return {pid for pid, entry in current.items() if old.get(pid) != entry}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("registry", nargs="?", default=REGISTRY)
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--changed", metavar="BASE", help="fetch only the entries that differ from git BASE")
    group.add_argument("--offline", action="store_true", help="check the file's own rules only")
    args = parser.parse_args()
    try:
        registry = tomllib.loads(Path(args.registry).read_text())
    except (OSError, tomllib.TOMLDecodeError) as error:
        print(f"✗ {args.registry}: {error}")
        return 1
    problems: list[str] = []
    for pid, entry in registry.items():
        if not isinstance(entry, dict):
            problems.append(f"`{pid}` is not a table")
            continue
        problems += entry_problems(pid, entry)
    seen: dict[tuple[str, str], str] = {}
    for pid, entry in registry.items():
        if isinstance(entry, dict) and isinstance(entry.get("repository"), str):
            key = (entry["repository"].rstrip("/").removesuffix(".git").lower(), entry.get("path", ""))
            if key in seen:
                problems.append(f"{pid} and {seen[key]} are the same folder of the same repository")
            seen[key] = pid
    if args.offline or problems:
        report(registry, problems, fetched=0)
        return 1 if problems else 0
    targets = changed_ids(args.changed, registry) if args.changed else set(registry)
    commands: dict[str, list[str]] = {}
    tools: dict[str, list[str]] = {}
    fetched = 0
    for pid in sorted(registry):
        entry = registry[pid]
        with tempfile.TemporaryDirectory(prefix=f"angelx-plugin-{pid}-") as scratch:
            try:
                files = fetch(entry["repository"], entry["commit"], entry.get("path", ""), Path(scratch))
            except (RuntimeError, ValueError, UnicodeDecodeError) as error:
                if pid in targets:
                    problems.append(f"{pid}: cannot fetch {entry['repository']} at {entry['commit'][:12]}: {error}")
                continue
        fetched += 1
        found, cart = plugin_problems(pid, entry, files)
        if pid in targets:
            problems += found
            print(f"{'✗' if found else '✓'} {pid} {entry['version']} · {len(files)} files · {digest(files)[:16]}")
        if cart:
            for command in cart.get("commands", []):
                commands.setdefault(command.get("name", ""), []).append(pid)
            for tool in cart.get("tools", []):
                tools.setdefault(tool.get("name", ""), []).append(pid)
    for kind, claims in (("/", commands), ("tool ", tools)):
        for name, owners in sorted(claims.items()):
            if len(owners) > 1 and targets.intersection(owners):
                problems.append(f"{kind}{name} is claimed by {', '.join(owners)}; one plugin per name")
    report(registry, problems, fetched)
    return 1 if problems else 0


def report(registry: dict, problems: list[str], fetched: int) -> None:
    for problem in problems:
        print(f"✗ {problem}")
    verdict = "fails" if problems else "passes"
    print(f"registry {verdict}: {len(registry)} plugin(s), {fetched} fetched, {len(problems)} problem(s)")


if __name__ == "__main__":
    sys.exit(main())
