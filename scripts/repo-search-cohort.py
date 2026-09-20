#!/usr/bin/env python3
"""T01 repository search / file discovery recall@k cohort (model-free)."""
from __future__ import annotations

from trace_schema import receipt_header_markdown
from receipt_provenance import attach_producer_header
import argparse
import hashlib
import importlib.util
import json
import os
import random
import shutil
import statistics
import subprocess
import tempfile
import time
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
TMP = REPO / ".tmp" / "repo-search-cohort"
DEFAULT_BINARY = REPO / "cockpit/target/release/angel"
if not DEFAULT_BINARY.is_file():
    DEFAULT_BINARY = REPO / "cockpit" / "target" / "release" / "angel"
DEFAULT_OUT = REPO / "docs" / "audits" / "evidence" / "2026-09-08-straight-a" / "T01"
SCHEMA = "angel-t01-repo-search/v2"
SEED = 20260908
TOOLS = ("find_files", "file_search", "grep", "list_dir")
OPTIONAL_TOOLS = ("repo_search",)
CLASSES = ("large", "ignored", "symlink")


def load_stress():
    spec = importlib.util.spec_from_file_location(
        "harness_stress", Path(__file__).with_name("harness-stress.py")
    )
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def utc():
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat()


def work_temp(prefix: str = "t01-") -> Path:
    TMP.mkdir(parents=True, exist_ok=True)
    return Path(tempfile.mkdtemp(prefix=prefix, dir=TMP))


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()


def posix(path) -> str:
    return str(path).replace("\\", "/")


def token(rng: random.Random, prefix: str, n: int = 8) -> str:
    alphabet = "abcdefghijkmnpqrstuvwxyz23456789"
    return prefix + "_" + "".join(rng.choice(alphabet) for _ in range(n))


def write_file(root: Path, rel: str, text: str) -> Path:
    path = root / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    return path


def target_rec(
    tid: str,
    rel: str,
    kind: str,
    repo_class: str,
    *,
    identifier: str,
    ignored: bool = False,
    symlink: str | None = None,
    escaping: bool = False,
    near_dup_of: str | None = None,
    content_only: bool = False,
    renamed_from: str | None = None,
) -> dict:
    return {
        "id": tid,
        "path": posix(rel),
        "kind": kind,
        "repo_class": repo_class,
        "identifier": identifier,
        "ignored": ignored,
        "symlink": symlink,
        "escaping": escaping,
        "near_dup_of": near_dup_of,
        "content_only": content_only,
        "renamed_from": renamed_from,
    }


def build_large_tree(
    root: Path,
    *,
    seed: int = SEED,
    n_files: int = 5000,
    n_dirs: int = 300,
    n_targets: int = 24,
) -> list[dict]:
    rng = random.Random(seed)
    root = Path(root)
    root.mkdir(parents=True, exist_ok=True)
    dirs = [Path(f"d{i:03d}/s{j:02d}") for i in range((n_dirs + 4) // 5) for j in range(5)]
    dirs = dirs[:n_dirs]
    for d in dirs:
        (root / d).mkdir(parents=True, exist_ok=True)
    filler = n_files - n_targets
    for i in range(max(0, filler)):
        d = dirs[i % len(dirs)]
        write_file(root, posix(d / f"f{i:05d}.txt"), f"filler {i}\n")
    targets = []
    for i in range(n_targets):
        d = dirs[(i * 7) % len(dirs)]
        ident = token(rng, "uniq")
        kind = ("unique", "near_dup", "content_only", "renamed")[i % 4]
        if kind == "unique":
            rel = posix(d / f"{ident}.py")
            write_file(root, rel, f"def {ident}():\n    return {i}\n")
            targets.append(target_rec(f"L{i:02d}", rel, kind, "large", identifier=ident))
        elif kind == "near_dup":
            base = token(rng, "near")
            rel_a = posix(d / f"{base}.rs")
            rel_b = posix(d / f"{base}_copy.rs")
            write_file(root, rel_a, f"fn {base}() {{ {i} }}\n")
            write_file(root, rel_b, f"fn {base}_copy() {{ {i} }}\n")
            targets.append(
                target_rec(f"L{i:02d}a", rel_a, kind, "large", identifier=base, near_dup_of=rel_b)
            )
            targets.append(
                target_rec(f"L{i:02d}b", rel_b, kind, "large", identifier=base + "_copy", near_dup_of=rel_a)
            )
        elif kind == "content_only":
            rel = posix(d / f"blob_{i:03d}.md")
            ident = token(rng, "body")
            write_file(root, rel, f"# notes\nsecret token {ident} lives only in content\n")
            targets.append(
                target_rec(f"L{i:02d}", rel, kind, "large", identifier=ident, content_only=True)
            )
        else:
            ident = token(rng, "ren")
            old = posix(d / f"old_{ident}.js")
            new = posix(d / f"new_{ident}.js")
            body = f"export const {ident} = {i};\n"
            write_file(root, old, body)
            write_file(root, new, body)
            targets.append(
                target_rec(f"L{i:02d}", new, kind, "large", identifier=ident, renamed_from=old)
            )
    return targets


def build_ignored_tree(root: Path, *, seed: int = SEED, n_targets: int = 20) -> list[dict]:
    rng = random.Random(seed + 1)
    root = Path(root)
    write_file(root, ".gitignore", "node_modules/\nbuild/\n*.log\n")
    write_file(root, "src/app.py", "print('visible')\n")
    write_file(root, "README.md", "ignored-tree fixture\n")
    targets = []
    for i in range(n_targets):
        ident = token(rng, "ign")
        if i % 2 == 0:
            rel = f"node_modules/pkg{i}/{ident}.js"
            write_file(root, rel, f"module.exports = '{ident}';\n")
        else:
            rel = f"build/out{i}/{ident}.o.txt"
            write_file(root, rel, f"OBJ {ident}\n")
        targets.append(
            target_rec(f"I{i:02d}", rel, "ignored", "ignored", identifier=ident, ignored=True)
        )
    vis = token(rng, "vis")
    rel = f"src/{vis}.py"
    write_file(root, rel, f"{vis} = 1\n")
    targets.append(target_rec("Ivis", rel, "unique", "ignored", identifier=vis, ignored=False))
    return targets


def build_symlink_tree(root: Path, *, seed: int = SEED, n_targets: int = 16) -> list[dict]:
    rng = random.Random(seed + 2)
    root = Path(root)
    root.mkdir(parents=True, exist_ok=True)
    outside = root.parent / (root.name + ".escape")
    outside.mkdir(parents=True, exist_ok=True)
    secret = token(rng, "esc")
    escape_file = outside / f"{secret}.txt"
    escape_file.write_text(f"ESCAPING {secret}\n", encoding="utf-8")
    link_escape = root / "vendor" / "escape-link"
    link_escape.parent.mkdir(parents=True, exist_ok=True)
    if link_escape.exists() or link_escape.is_symlink():
        link_escape.unlink()
    link_escape.symlink_to(escape_file)
    targets = [
        target_rec(
            "Sesc",
            "vendor/escape-link",
            "escaping_symlink",
            "symlink",
            identifier=secret,
            symlink="escaping",
            escaping=True,
        )
    ]
    vendored = []
    for i in range(n_targets):
        ident = token(rng, "sym")
        rel = f"src/mod{i}/{ident}.py"
        write_file(root, rel, f"{ident} = {i}\n")
        copy = f"vendor/copy_{ident}.py"
        write_file(root, copy, f"{ident} = {i}\n")
        vendored.append(copy)
        in_tree = root / "links" / f"to_{ident}"
        in_tree.parent.mkdir(parents=True, exist_ok=True)
        if in_tree.exists() or in_tree.is_symlink():
            in_tree.unlink()
        in_tree.symlink_to(Path("..") / rel)
        targets.append(
            target_rec(
                f"S{i:02d}",
                rel,
                "unique",
                "symlink",
                identifier=ident,
                symlink="in-tree",
            )
        )
        targets.append(
            target_rec(
                f"Sv{i:02d}",
                copy,
                "vendored",
                "symlink",
                identifier=ident,
            )
        )
    return targets


def build_all_repos(base: Path, *, seed: int = SEED, large_files: int = 5000) -> dict:
    base = Path(base)
    large = base / "large"
    ignored = base / "ignored"
    symlink = base / "symlink"
    if large.exists():
        shutil.rmtree(large)
    if ignored.exists():
        shutil.rmtree(ignored)
    if symlink.exists():
        shutil.rmtree(symlink)
    t_large = build_large_tree(large, seed=seed, n_files=large_files)
    t_ign = build_ignored_tree(ignored, seed=seed)
    t_sym = build_symlink_tree(symlink, seed=seed)
    return {
        "large": {"root": large, "targets": t_large},
        "ignored": {"root": ignored, "targets": t_ign},
        "symlink": {"root": symlink, "targets": t_sym},
    }


def count_tree(root: Path) -> tuple[int, int]:
    n_files = 0
    n_dirs = 0
    for dirpath, dirnames, filenames in os.walk(root, followlinks=False):
        n_dirs += 1
        n_files += len(filenames)
    return n_files, n_dirs


def extract_paths(content: str) -> list[str]:
    if not content:
        return []
    out = []
    seen = set()
    for raw in content.splitlines():
        line = raw.strip().strip("`").strip('"').strip("'")
        if not line or line.startswith("{") or line.startswith("[") or line.lower().startswith("tool error:"):
            continue
        low = line.lower()
        if low.startswith("no files match") or "no files fuzzy" in low or "no matches" in low or " is empty" in low:
            continue
        candidate = line
        if ":" in line:
            head = line.split(":", 1)[0]
            if "/" in head or head.endswith((".py", ".rs", ".js", ".md", ".txt", ".o")):
                candidate = head
        candidate = candidate.split()[0] if candidate.split() else candidate
        candidate = posix(candidate)
        while candidate.startswith("./"):
            candidate = candidate[2:]
        if candidate and candidate not in seen and not candidate.startswith("error"):
            seen.add(candidate)
            out.append(candidate)
    return out


def find_files_args(target: dict) -> dict:
    """Schema: find_files.pattern is a glob vs workspace-relative paths (not `glob`)."""
    name = Path(target["path"]).name
    return {"pattern": f"**/{name}"}


def file_search_args(target: dict) -> dict:
    return {"query": Path(target["path"]).name, "limit": 20}


def grep_args(target: dict, *, explicit_path: bool = False) -> dict:
    args = {"pattern": target["identifier"]}
    if explicit_path:
        args["path"] = target["path"]
    return args


def list_dir_args(target: dict) -> dict:
    parent = str(Path(target["path"]).parent)
    args = {"path": parent if parent and parent != "." else "."}
    if target.get("list_dir_hint"):
        args["hint"] = Path(target["path"]).name
    return args


def classify_miss(gold: str, content: str, ranked: list[str], args: dict, tool: str) -> str | None:
    gold = posix(gold)
    def hit(p):
        p = posix(p)
        return p == gold
    if any(hit(p) for p in ranked):
        return None
    blob = content or ""
    stripped = blob.strip()
    low = stripped.lower()
    if low.startswith("no files match") or "no files fuzzy" in low or "no matches" in low or low.startswith("error") or "missing '" in low:
        if tool == "find_files" and "glob" in args and "pattern" not in args:
            return "query_shape"
        if stripped.lower().startswith("error") or "missing '" in low:
            return "query_shape"
        return "tool_empty"
    if gold in posix(blob) or Path(gold).name in blob:
        return "extractor_miss"
    if tool == "find_files" and "glob" in args and "pattern" not in args:
        return "query_shape"
    pat = str(args.get("pattern") or "")
    if tool == "find_files" and "pattern" in args and "/" not in pat and "**" not in pat:
        return "query_shape"
    if tool == "file_search" and "query" not in args:
        return "query_shape"
    if tool == "grep" and "pattern" not in args:
        return "query_shape"
    stripped = blob.strip()
    if stripped.lower().startswith("error") or "missing '" in stripped.lower():
        return "query_shape"
    return "tool_empty"


def recall_at_k(ranked: list[str], gold: str, k: int) -> float:
    gold = posix(gold)
    for path in ranked[:k]:
        p = posix(path)
        if p == gold:
            return 1.0
    return 0.0


def hops_to_first_hit(tool_results: list[dict], gold: str, identifier: str) -> int | None:
    gold = posix(gold)
    for i, item in enumerate(tool_results, 1):
        if gold in ranked_paths(item):
            return i
    return None


def percentile(xs: list[float], p: float) -> float:
    if not xs:
        return 0.0
    ys = sorted(xs)
    if len(ys) == 1:
        return float(ys[0])
    k = (len(ys) - 1) * p
    lo = int(k)
    hi = min(lo + 1, len(ys) - 1)
    frac = k - lo
    return float(ys[lo] * (1 - frac) + ys[hi] * frac)


def classify_ignore(default_paths: list[str], explicit_paths: list[str], gold: str, ignored: bool) -> str:
    gold = posix(gold)
    def hit(paths):
        return any(gold in posix(p) or posix(p).endswith(gold) for p in paths)

    d, e = hit(default_paths), hit(explicit_paths)
    if ignored:
        if not d and e:
            return "ignored_found_only_when_asked"
        if d:
            return "ignored_found_by_default"
        return "ignored_not_found"
    if d or e:
        return "visible_found"
    return "visible_miss"


def classify_symlink(content: str, escaping: bool) -> str:
    low = (content or "").lower()
    refused = any(
        tok in low
        for tok in ("refused", "escape", "outside", "denied", "off-limits", "not allowed", "blocked")
    )
    if escaping:
        return "escaping_refused" if refused else "escaping_not_refused"
    return "in_tree_ok"


def results_for_target(tool_results: list[dict], target: dict, tool: str) -> list[dict]:
    """Associate by the query, never by a coincidental hit in another response.

    Repeated identical directory requests are one ranking, not concatenated
    copies of the same alphabetical listing. Explicit grep is a separate probe.
    """
    builders = {"find_files": find_files_args, "file_search": file_search_args,
                "grep": grep_args, "list_dir": list_dir_args}
    expected = builders[tool](target)
    for item in tool_results:
        args = item.get("args") or {}
        if (item.get("name") == tool
                and all(args.get(key) == value for key, value in expected.items())
                and (tool != "grep" or "path" not in args)):
            return [item]
    return []


def ranked_paths(item: dict) -> list[str]:
    paths = extract_paths(item.get("content") or "")
    if item.get("name") == "list_dir":
        parent = (item.get("args") or {}).get("path") or "."
        paths = [posix(Path(parent) / path) for path in paths]
    return list(dict.fromkeys(paths))


def score_target(target: dict, by_tool: dict[str, list[str]], tool_results: list[dict]) -> dict:
    gold = target["path"]
    ident = target["identifier"]
    rec = {
        "id": target["id"],
        "path": gold,
        "repo_class": target["repo_class"],
        "kind": target["kind"],
        "ignored": target["ignored"],
        "escaping": target["escaping"],
        "recall": {},
        "miss_class": {},
        "hops_to_hit": hops_to_first_hit(tool_results, gold, ident),
    }
    for tool in TOOLS:
        items = results_for_target(tool_results, target, tool)
        ranked = ranked_paths(items[0]) if items else []
        rec["recall"][tool] = {
            "at1": recall_at_k(ranked, gold, 1),
            "at3": recall_at_k(ranked, gold, 3),
            "at5": recall_at_k(ranked, gold, 5),
            "at10": recall_at_k(ranked, gold, 10),
            "at_all": recall_at_k(ranked, gold, len(ranked)),
        }
        rec["miss_class"][tool] = classify_miss(
            gold,
            "\n".join(item.get("content") or "" for item in items),
            ranked,
            (items[0].get("args") if items else {}) or {},
            tool,
        )
    default, explicit = [], []
    for item in tool_results:
        if item.get("name") not in TOOLS:
            continue
        args = item.get("args") or {}
        destination = explicit if args.get("no_ignore") else default
        destination.extend(ranked_paths(item))
    rec["ignore_class"] = classify_ignore(default, explicit, gold, target["ignored"])
    escape_reads = [item.get("content") or "" for item in tool_results
                    if item.get("name") == "read_file"
                    and (item.get("args") or {}).get("path") == gold]
    rec["symlink_class"] = classify_symlink("\n".join(escape_reads), target["escaping"])
    return rec


def identity(binary: Path) -> dict:
    info = {}
    try:
        proc = subprocess.run(
            [str(binary), "--build-info", "--json"], capture_output=True, text=True, timeout=30
        )
        if proc.returncode == 0 and proc.stdout.strip():
            info = json.loads(proc.stdout)
    except (OSError, json.JSONDecodeError, subprocess.TimeoutExpired):
        info = {}
    return {"path": str(binary), "sha256": sha256_file(binary) if binary.is_file() else "", "build_info": info}


def angel_env(home: Path, url: str, extra: dict | None = None) -> dict:
    env = {k: os.environ[k] for k in ("PATH", "LANG", "LC_ALL", "HOME") if k in os.environ}
    env.update(
        {
            "HOME": str(home),
            "ANGEL_DRIVER": "openrouter",
            "ANGEL_API_CLUBS": "openrouter",
            "ANGEL_OPENROUTER_URL": url,
            "ANGEL_OPENROUTER_MODEL": "offline-repo-search",
            "ANGEL_OPENROUTER_KEY": "offline-stress-key",
            "OPENROUTER_API_KEY": "offline-stress-key",
            "ANGEL_YOLO": "1",
            "ANGEL_FIRST_WRITE_CALLS": "0",
            "ANGEL_PROJECT_DOC": "0",
            "ANGEL_TASK_RECON": "0",
            "ANGEL_SKILL_HINT": "0",
            "ANGEL_ADVISOR": "0",
            "ANGEL_TASK_STRICT_EXIT": "0",
            "ANGEL_REPO_TOOL": "1",
            "ANGEL_TRAJECTORY_LOG": "1",
            "ANGEL_TRAJECTORY_DIR": str(home / "trajectories"),
            "CODEX_HOME": "/nonexistent-codex",
        }
    )
    if extra:
        env.update(extra)
    (home / "trajectories").mkdir(parents=True, exist_ok=True)
    return env


def make_policy(plan: list, sink: list):
    def policy(hop, req):
        sink.append(req)
        if hop < len(plan):
            return plan[hop]
        return ("text", "repo-search cohort: plan exhausted")

    return policy


def index_tool_results(messages: list) -> list[dict]:
    pending = {}
    results = []
    for message in messages:
        if message.get("role") == "assistant":
            for call in message.get("tool_calls") or []:
                fn = call.get("function") or {}
                args = fn.get("arguments")
                if isinstance(args, str):
                    try:
                        args = json.loads(args) if args else {}
                    except json.JSONDecodeError:
                        args = {}
                if not isinstance(args, dict):
                    args = {}
                pending[call.get("id")] = {"name": fn.get("name"), "args": args}
        elif message.get("role") == "tool":
            tid = message.get("tool_call_id")
            meta = pending.get(tid) or {}
            results.append(
                {
                    "id": tid,
                    "name": meta.get("name"),
                    "args": meta.get("args") or {},
                    "content": message.get("content") or "",
                }
            )
    return results


def load_trajectory(traj_dir: Path) -> dict | None:
    files = sorted(Path(traj_dir).glob("session-*.jsonl"))
    if not files:
        files = sorted(Path(traj_dir).rglob("*.jsonl"))
    best = None
    for path in files:
        try:
            lines = [ln for ln in path.read_text(encoding="utf-8").splitlines() if ln.strip()]
        except OSError:
            continue
        for line in lines:
            try:
                rec = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(rec, dict) and (rec.get("tools") or rec.get("messages")):
                best = rec
    return best


def discovery_plan(targets: list[dict], repo_class: str, *, no_ignore: bool = False, hidden: bool = False) -> list:
    plan = []
    batch = 6
    for i in range(0, len(targets), batch):
        chunk = targets[i : i + batch]
        calls = []
        for j, t in enumerate(chunk):
            ident = t["identifier"]
            path = t["path"]
            calls.append((f"ff{i}_{j}", "find_files", find_files_args(t)))
            calls.append((f"fs{i}_{j}", "file_search", file_search_args(t)))
            calls.append((f"g{i}_{j}", "grep", grep_args(t)))
            calls.append((f"ld{i}_{j}", "list_dir", list_dir_args(t)))
            if t.get("ignored"):
                calls.append((f"gx{i}_{j}", "grep", grep_args(t, explicit_path=True)))
            if t.get("escaping"):
                calls.append((f"re{i}_{j}", "read_file", {"path": path}))
        # Keep every query: ignored/escaping probes can push a batch past 24.
        plan.extend(("calls", calls[j : j + 24]) for j in range(0, len(calls), 24))
    if repo_class == "large" and targets:
        plan.append(
            (
                "calls",
                [("rs0", "repo_search", {"query": targets[0]["identifier"]})],
            )
        )
    plan.append(("text", "repo-search cohort complete"))
    for kind, payload in plan:
        if kind == "calls":
            for _, name, args in payload:
                if name in TOOLS:
                    if no_ignore:
                        args["no_ignore"] = True
                    if hidden:
                        args["hidden"] = True
    return plan


def run_angel(binary: Path, workspace: Path, home: Path, plan: list, hops: int, deadline: int) -> dict:
    stress = load_stress()
    requests: list = []
    home.mkdir(parents=True, exist_ok=True)
    cmd = [
        str(binary),
        "--yolo",
        "--task-json",
        "--workspace",
        str(workspace),
        "--max-hops",
        str(hops),
        "--deadline-secs",
        str(deadline),
        "--tool-profile",
        "full",
        "--rollout",
        "off",
        "--task-id",
        "t01-repo-search",
        "Discover planted files with find_files, file_search, grep, and list_dir. Do not edit.",
    ]
    t0 = time.perf_counter()
    with stress.PolicyServer(make_policy(plan, requests)) as srv:
        env = angel_env(home, srv.url)
        proc = subprocess.run(cmd, cwd=str(workspace), env=env, capture_output=True, text=True)
    wall_ms = (time.perf_counter() - t0) * 1000
    envelope = {}
    try:
        text = (proc.stdout or "").strip()
        if text:
            envelope = json.loads(text.splitlines()[-1])
    except json.JSONDecodeError:
        envelope = {"stdout_tail": (proc.stdout or "")[-800:]}
    traj = load_trajectory(home / "trajectories")
    messages = []
    if requests:
        messages = max(requests, key=lambda r: len(r.get("messages") or [])).get("messages") or []
    if traj and len(traj.get("messages") or []) > len(messages):
        messages = traj["messages"]
    return {
        "returncode": proc.returncode,
        "envelope": envelope,
        "stderr_tail": (proc.stderr or "")[-2000:],
        "wall_ms": wall_ms,
        "messages": messages,
        "tool_results": index_tool_results(messages),
        "ledger_tools": (traj or {}).get("tools") or [],
        "hops": len(requests),
    }


def paths_by_tool(tool_results: list[dict]) -> dict[str, list[str]]:
    by = defaultdict(list)
    for item in tool_results:
        by[item.get("name") or ""].extend(ranked_paths(item))
    return by


def probe_one_per_class(binary: Path, trees: dict, *, no_ignore: bool = False, hidden: bool = False) -> list[dict]:
    """One known target per repo class: raw first 400 bytes + miss classification."""
    probes = []
    for cls, pack in trees.items():
        targets = pack["targets"]
        t = next((x for x in targets if not x.get("escaping") and not x.get("content_only")), targets[0])
        home = work_temp(f"t01-probe-{cls}-")
        calls = [
            ("ff", "find_files", find_files_args(t)),
            ("fs", "file_search", file_search_args(t)),
            ("g", "grep", grep_args(t)),
            ("ld", "list_dir", list_dir_args(t)),
        ]
        for _, _, args in calls:
            if no_ignore:
                args["no_ignore"] = True
            if hidden:
                args["hidden"] = True
        run = run_angel(binary, pack["root"], home, [("calls", calls), ("text", "probe done")], hops=8, deadline=60)
        by = paths_by_tool(run["tool_results"])
        tools = {}
        for item in run["tool_results"]:
            name = item.get("name") or ""
            if name not in TOOLS:
                continue
            raw = item.get("content") or ""
            miss = classify_miss(t["path"], raw, by.get(name) or [], item.get("args") or {}, name)
            tools[name] = {
                "args": item.get("args") or {},
                "raw_400": raw[:400],
                "extracted": (by.get(name) or [])[:10],
                "miss_class": miss,
            }
        probes.append({"repo_class": cls, "target": t["path"], "identifier": t["identifier"], "tools": tools})
    return probes


def summarize(rows: list[dict], walls: list[float], binary_meta: dict, trees: dict) -> dict:
    by_class: dict[str, list] = defaultdict(list)
    by_tool_scores: dict[str, list] = defaultdict(list)
    for row in rows:
        by_class[row["repo_class"]].append(row)
        for tool, rec in (row.get("recall") or {}).items():
            by_tool_scores[tool].append(rec)
    def mean_at(items, k):
        if not items:
            return 0.0
        key = {1: "at1", 3: "at3", 5: "at5", 10: "at10"}[k]
        return sum(x[key] for x in items) / len(items)

    hops = [r["hops_to_hit"] for r in rows if r.get("hops_to_hit") is not None]
    ignore_counts = defaultdict(int)
    symlink_counts = defaultdict(int)
    for r in rows:
        ignore_counts[r.get("ignore_class") or "n/a"] += 1
        symlink_counts[r.get("symlink_class") or "n/a"] += 1
    per_class = {}
    for cls, items in by_class.items():
        recs = []
        for it in items:
            recs.extend(it.get("recall", {}).values())
        per_class[cls] = {
            "n": len(items),
            "recall@1": mean_at(recs, 1),
            "recall@5": mean_at(recs, 5),
            "recall@10": mean_at(recs, 10),
        }
    per_tool = {
        tool: {
            "recall@1": mean_at(vals, 1),
            "recall@3": mean_at(vals, 3),
            "recall@5": mean_at(vals, 5),
            "recall@10": mean_at(vals, 10),
            "membership_recall": sum(v.get("at_all", v["at10"]) for v in vals) / len(vals),
        }
        for tool, vals in by_tool_scores.items()
    }
    tree_stats = {}
    for name, pack in trees.items():
        nf, nd = count_tree(pack["root"])
        tree_stats[name] = {"files": nf, "dirs": nd, "targets": len(pack["targets"])}
    miss_counts = defaultdict(int)
    for r in rows:
        for tool, cls_name in (r.get("miss_class") or {}).items():
            miss_counts[f"{tool}:{cls_name or 'hit'}"] += 1
    miss_counts = dict(miss_counts)
    return {
        "schema": SCHEMA,
        "generated_at": utc(),
        "binary": binary_meta,
        "trees": tree_stats,
        "n_targets": len(rows),
        "per_tool": per_tool,
        "per_class": per_class,
        "hops_to_first_hit": {
            "n": len(hops),
            "p50": percentile([float(h) for h in hops], 0.50) if hops else None,
            "p95": percentile([float(h) for h in hops], 0.95) if hops else None,
            "mean": statistics.mean(hops) if hops else None,
        },
        "wall_ms": {
            "n": len(walls),
            "p50": percentile(walls, 0.50),
            "p95": percentile(walls, 0.95),
        },
        "ignore_behaviour": dict(ignore_counts),
        "symlink_behaviour": dict(symlink_counts),
        "miss_class": miss_counts,
        "rows": rows,
    }


def render_md(summary: dict) -> str:
    b = summary.get("binary") or {}
    lines = [
        "# T01 repository search / file discovery",
        "",
        f"- schema: `{summary.get('schema')}`",
        f"- generated_at: {summary.get('generated_at')}",
        f"- binary: `{b.get('path')}`",
        f"- binary sha256: `{b.get('sha256')}`",
        f"- n_targets: {summary.get('n_targets')}",
        "",
        "## Trees",
        "",
        "| class | files | dirs | targets |",
        "|---|---:|---:|---:|",
    ]
    for name, st in (summary.get("trees") or {}).items():
        lines.append(f"| {name} | {st.get('files')} | {st.get('dirs')} | {st.get('targets')} |")
    lines += [
        "",
        "## Recall@k per tool",
        "",
        "Ranks use one matching query response; membership_recall measures the complete returned listing. "
        + ("list_dir uses an exact basename hint (changed query protocol)." if summary.get("list_dir_hint")
           else "list_dir uses only the parent directory (original unhinted query protocol)."),
        "",
        "| tool | recall@1 | recall@3 | recall@5 | recall@10 | membership_recall |",
        "|---|---:|---:|---:|---:|---:|",
    ]
    for tool, rec in (summary.get("per_tool") or {}).items():
        lines.append(
            f"| {tool} | {rec.get('recall@1', 0):.3f} | {rec.get('recall@3', 0):.3f} | {rec.get('recall@5', 0):.3f} | {rec.get('recall@10', 0):.3f} | {rec.get('membership_recall', 0):.3f} |"
        )
    lines += [
        "",
        "## Recall@k per repository class",
        "",
        "| class | n | recall@1 | recall@5 | recall@10 |",
        "|---|---:|---:|---:|---:|",
    ]
    for cls, rec in (summary.get("per_class") or {}).items():
        lines.append(
            f"| {cls} | {rec.get('n')} | {rec.get('recall@1', 0):.3f} | {rec.get('recall@5', 0):.3f} | {rec.get('recall@10', 0):.3f} |"
        )
    hops = summary.get("hops_to_first_hit") or {}
    wall = summary.get("wall_ms") or {}
    lines += [
        "",
        f"hops_to_first_hit p50={hops.get('p50')} p95={hops.get('p95')} mean={hops.get('mean')}",
        f"wall_ms p50={wall.get('p50')} p95={wall.get('p95')}",
        "",
        f"ignore_behaviour: {json.dumps(summary.get('ignore_behaviour') or {})}",
        f"symlink_behaviour: {json.dumps(summary.get('symlink_behaviour') or {})}",
          f"miss_class: {json.dumps(summary.get('miss_class') or {})}",
        "",
        "## Schema probes (one target per class, raw 400)",
        "",
    ]
    for probe in summary.get("probes") or []:
        lines.append(f"### {probe.get('repo_class')} `{probe.get('target')}`")
        for tool, info in (probe.get("tools") or {}).items():
            raw = (info.get("raw_400") or "").replace("\n", " / ")
            lines.append(f"- {tool} args={json.dumps(info.get('args'))} miss={info.get('miss_class')} raw={raw[:400]}")
        lines.append("")
    lines.append("")
    return "\n".join(lines)


def emit(summary: dict, out_dir: Path, meta: dict | None = None, dataset=None) -> tuple[Path, Path]:
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "rows.jsonl").write_text(
        "".join(json.dumps({k: v for k, v in row.items() if k != "content"}) + "\n"
                for row in (summary.get("rows") or [])))
    identity = dict(meta or (summary.get("binary") or {}))
    if identity.get("build_info") is None:
        identity = {}
    from receipt_provenance import binary_metadata
    if identity:
        try:
            identity.update(binary_metadata(identity["path"]))
        except Exception:
            pass
    if dataset is None:
        dataset = {"id": "t01/repo-search", "sha256": None}
    attach_producer_header(summary, metadata=identity, dataset=dataset,
        evaluator_path=out_dir / "rows.jsonl",
        declarations={'wall_ms': 'ms', 'hops_to_first_hit': 'hops', 'recall': 'fraction',
                      'precision': 'fraction', 'n': 'count', 'seed': 'seed',
                      'recall@1': 'fraction', 'recall@3': 'fraction', 'recall@5': 'fraction',
                      'recall@10': 'fraction', 'membership_recall': 'fraction',
                      'first_hit_rate': 'fraction', 'hit_rate': 'fraction'})
    slim = dict(summary)
    slim["rows"] = [
        {k: v for k, v in row.items() if k != "content"} for row in (summary.get("rows") or [])
    ]
    jp = out_dir / "repo-search.json"
    mp = out_dir / "repo-search.md"
    encoded = json.dumps(slim, indent=2)
    if len(encoded.encode("utf-8")) > 900_000:
        raise ValueError("summary exceeds receipt size limit")
    jp.write_text(encoded, encoding="utf-8")
    mp.write_text(render_md(slim) + receipt_header_markdown(summary), encoding="utf-8")
    return jp, mp


def run_cohort(
    binary: Path | None = None,
    out_dir: Path | None = None,
    *,
    large_files: int = 5000,
    hops: int = 40,
    deadline: int = 180,
    no_ignore: bool = False,
    hidden: bool = False,
    list_dir_hint: bool = False,
) -> dict:
    binary = Path(binary or DEFAULT_BINARY).resolve()
    out_dir = Path(out_dir or DEFAULT_OUT)
    base = work_temp("t01-ws-")
    trees = build_all_repos(base, large_files=large_files)
    if list_dir_hint:
        for pack in trees.values():
            for target in pack["targets"]:
                target["list_dir_hint"] = True
    meta = identity(binary)
    rows = []
    walls = []
    out_dir.mkdir(parents=True, exist_ok=True)
    probes = probe_one_per_class(binary, trees, no_ignore=no_ignore, hidden=hidden)
    for cls, pack in trees.items():
        home = work_temp(f"t01-home-{cls}-")
        plan = discovery_plan(pack["targets"], cls, no_ignore=no_ignore, hidden=hidden)
        run = run_angel(binary, pack["root"], home, plan, hops=hops, deadline=deadline)
        walls.append(run["wall_ms"])
        by = paths_by_tool(run["tool_results"])
        trace = {"generated_at": utc(), "binary": meta,
                 "returncode": run["returncode"], "hops": run["hops"],
                 "queries": []}
        for t in pack["targets"]:
            for tool in TOOLS:
                items = results_for_target(run["tool_results"], t, tool)
                ranked = ranked_paths(items[0]) if items else []
                trace["queries"].append({"target": t, "tool": tool, "results": items,
                                         "ranked": ranked,
                                         "rank": ranked.index(t["path"]) + 1 if t["path"] in ranked else None})
        (out_dir / f"queries-{cls}.json").write_text(json.dumps(trace, indent=2), encoding="utf-8")
        for t in pack["targets"]:
            rows.append(score_target(t, by, run["tool_results"]))
    summary = summarize(rows, walls, meta, trees)
    summary["probes"] = probes
    summary["list_dir_hint"] = list_dir_hint
    summary["discovery_options"] = {"no_ignore": no_ignore, "hidden": hidden}
    from receipt_provenance import declared_dataset
    emit(summary, out_dir, meta=meta,
         dataset=declared_dataset("t01/repo-search", {
             "targets": len(rows),
             "discovery": {"no_ignore": no_ignore, "hidden": hidden,
                           "list_dir_hint": list_dir_hint}}) if rows else None)
    return summary


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("--list-dir-hint", action="store_true",
                    help="separate hinted protocol: rank by known basename; default preserves unhinted calls")
    ap.add_argument("--no-ignore", action="store_true")
    ap.add_argument("--hidden", action="store_true")
    ap.add_argument("--fixtures-only", action="store_true", help="materialise deterministic trees without running a binary or server")
    ap.add_argument("--fixture-dir", type=Path, help="fixture destination (required with --fixtures-only)")
    ap.add_argument("--angel-bin", default=str(DEFAULT_BINARY))
    ap.add_argument("--out", default=str(DEFAULT_OUT))
    ap.add_argument("--large-files", type=int, default=5000)
    ap.add_argument("--hops", type=int, default=40)
    ap.add_argument("--deadline-secs", type=int, default=180)
    args = ap.parse_args(argv)
    if args.fixtures_only:
        if args.fixture_dir is None:
            ap.error("--fixtures-only requires --fixture-dir")
        trees = build_all_repos(args.fixture_dir.resolve(), large_files=args.large_files)
        manifest = {
            cls: {"root": str(pack["root"]), "targets": pack["targets"],
                  "file_count": count_tree(pack["root"])[0]}
            for cls, pack in trees.items()
        }
        args.fixture_dir.mkdir(parents=True, exist_ok=True)
        (args.fixture_dir / "manifest.json").write_text(json.dumps(manifest, indent=2))
        print(json.dumps({cls: pack["file_count"] for cls, pack in manifest.items()}))
        return 0
    summary = run_cohort(
        Path(args.angel_bin),
        Path(args.out),
        large_files=args.large_files,
        hops=args.hops,
        deadline=args.deadline_secs,
        no_ignore=args.no_ignore,
        hidden=args.hidden,
        list_dir_hint=args.list_dir_hint,
    )
    print(json.dumps({k: summary[k] for k in summary if k != "rows"}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
