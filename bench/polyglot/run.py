"""Run an agent over the polyglot tasks and grade every attempt.

Each task gets:
  * a fresh workspace: the stub fixture, committed to a new git repo
    ("frozen fixture"); JS tasks get the shared node_modules linked in;
  * a fresh HOME: no config, plugins, memory or login from an earlier attempt.
    Toolchain homes (cargo, rustup) and download caches stay shared, so
    compile time is not penalized;
  * a minimal environment: only PATH, HOME/XDG, locale, TMPDIR, the BENCH_* vars
    and whatever --pass-env names. API keys stay with the proxy;
  * one wall clock (default 600 s), enforced from outside: the agent's whole
    process group gets SIGTERM, then SIGKILL 10 s later;
  * grading however the agent exits, by the evaluator-owned grader.

Results go to <out>/results.jsonl, one row per task, appended as tasks finish,
so --resume continues a stopped run.
"""

from __future__ import annotations

import json
import os
import platform
import shutil
import signal
import subprocess
import sys
import threading
import time
import uuid
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
from pathlib import Path

from common import (
    KIT, KIT_VERSION, catalog_digest, load_tasks, read_jsonl, select_tasks, tool_path, work_dir,
)
from grade import grade
from compare import summary_text
from proxy import MeterProxy

KILL_GRACE_SECS = 10
PROXY_PLACEHOLDER_KEY = "bench-proxy-holds-the-key"


def load_config(path: str | None) -> dict:
    if not path:
        return {}
    candidate = Path(path)
    if not candidate.exists() and (KIT / "configs" / f"{path}.json").exists():
        candidate = KIT / "configs" / f"{path}.json"
    config = json.loads(candidate.read_text())
    config["_path"] = str(candidate)
    return config


def upstream_key(config: dict) -> str:
    if config.get("key_cmd"):
        return subprocess.run(config["key_cmd"], shell=True, capture_output=True, text=True, check=True).stdout.strip()
    var = config.get("key_env")
    if var:
        value = os.environ.get(var, "").strip()
        if not value:
            raise SystemExit(f"config {config['_path']} needs ${var} set (the proxy holds it; the agent never sees it)")
        return value
    return ""


class KeySource:
    """The upstream key, re-read at most once a minute (OAuth bearers expire)."""

    def __init__(self, config: dict):
        self.config = config
        self.value = upstream_key(config) if config else ""
        self.at = time.monotonic()
        self.lock = threading.Lock()

    def get(self) -> str:
        if self.config.get("key_cmd"):
            with self.lock:
                if time.monotonic() - self.at > 60:
                    self.value = upstream_key(self.config)
                    self.at = time.monotonic()
        return self.value


def fresh_home(home: Path, template: str | None) -> None:
    if home.exists():
        shutil.rmtree(home)
    for sub in (".config", ".local/share", ".local/state"):
        (home / sub).mkdir(parents=True)
    (home / ".gitconfig").write_text(
        "[user]\n\tname = bench-agent\n\temail = bench-agent@localhost\n[init]\n\tdefaultBranch = main\n"
    )
    if template:
        shutil.copytree(Path(template).expanduser(), home, dirs_exist_ok=True, symlinks=True)


def prepare_workspace(task: dict, ws: Path, home: Path) -> None:
    if ws.exists():
        shutil.rmtree(ws)
    shutil.copytree(work_dir() / "fixtures" / task["name"], ws)
    git_env = {**os.environ, "HOME": str(home), "GIT_CONFIG_NOSYSTEM": "1"}
    for cmd in (
        ["git", "init", "-q"],
        ["git", "add", "."],
        ["git", "commit", "-q", "-m", "frozen fixture"],
    ):
        subprocess.run(cmd, cwd=ws, env=git_env, check=True, capture_output=True)
    if task["language"] == "javascript":
        (ws / "node_modules").symlink_to(work_dir() / "js" / "node_modules")


def agent_env(args, task: dict, ws: Path, home: Path, base_url: str, config: dict, run_id: str) -> dict:
    real_home = Path.home()
    env = {
        "PATH": tool_path(),
        "HOME": str(home),
        "USER": os.environ.get("USER", "bench"),
        "LOGNAME": os.environ.get("LOGNAME", os.environ.get("USER", "bench")),
        "LANG": "C.UTF-8",
        "TERM": "dumb",
        "TMPDIR": os.environ.get("TMPDIR", "/tmp"),
        "XDG_CONFIG_HOME": str(home / ".config"),
        "XDG_DATA_HOME": str(home / ".local/share"),
        "XDG_STATE_HOME": str(home / ".local/state"),
        "XDG_CACHE_HOME": os.environ.get("XDG_CACHE_HOME", str(real_home / ".cache")),
        "CARGO_HOME": os.environ.get("CARGO_HOME", str(real_home / ".cargo")),
        "RUSTUP_HOME": os.environ.get("RUSTUP_HOME", str(real_home / ".rustup")),
        "BENCH_TASK": task["name"],
        "BENCH_LANGUAGE": task["language"],
        "BENCH_PROMPT": task["prompt"],
        "BENCH_PROMPT_FILE": str(args.out / "prompts" / f"{task['name']}.md"),
        "BENCH_WORKSPACE": str(ws),
        "BENCH_TEST_CMD": task["test_cmd"],
        "BENCH_WALL_SECS": str(args.wall),
        "BENCH_RUN_ID": run_id,
        "BENCH_BASE_URL": base_url,
        "BENCH_API_KEY": PROXY_PLACEHOLDER_KEY if config else "",
        "BENCH_MODEL": str(config.get("model", "")),
        "BENCH_PROVIDER": str(config.get("provider", "")),
        "BENCH_EFFORT": str(config.get("effort", "") or ""),
        "BENCH_MAX_TOKENS": str((config.get("sampling") or {}).get("max_tokens", "") or ""),
    }
    for name in args.pass_env:
        if name in os.environ:
            env[name] = os.environ[name]
    return env


def run_agent(cmd: str, ws: Path, env: dict, wall: int, log_base: Path) -> dict:
    started = time.monotonic()
    with open(f"{log_base}.out", "w") as out, open(f"{log_base}.err", "w") as err:
        proc = subprocess.Popen(
            ["bash", "-c", cmd], cwd=ws, env=env, stdin=subprocess.DEVNULL,
            stdout=out, stderr=err, start_new_session=True,
        )
        timed_out = False
        try:
            proc.wait(timeout=wall)
        except subprocess.TimeoutExpired:
            timed_out = True
            try:
                os.killpg(proc.pid, signal.SIGTERM)
                proc.wait(timeout=KILL_GRACE_SECS)
            except (ProcessLookupError, subprocess.TimeoutExpired):
                pass
        agent_s = time.monotonic() - started
        # Whatever the agent left running (test servers, watchers) goes too.
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        proc.wait()
    return {"agent_s": round(agent_s, 2), "agent_exit": proc.returncode, "timed_out": timed_out}


def usage_totals(calls: list[dict]) -> dict:
    total = {"calls": len(calls), "input": 0, "cached": 0, "output": 0, "reasoning": 0, "upstream_errors": 0}
    for call in calls:
        for key in ("input", "cached", "output", "reasoning"):
            total[key] += int(call.get(key) or 0)
        if int(call.get("status") or 0) >= 400 or (call.get("error") and call.get("error") != "client disconnected"):
            total["upstream_errors"] += 1
    total["uncached"] = total["input"] - total["cached"]
    total["model_s"] = round(sum(float(c.get("latency_s") or 0) for c in calls), 2)
    return total


def run_task(args, task: dict, config: dict, keys: KeySource, lock: threading.Lock) -> dict:
    name = task["name"]
    out = args.out
    ws = out / "ws" / name / task["exercise"]
    home = out / "homes" / name
    fresh_home(home, args.home_template)
    prepare_workspace(task, ws, home)
    (out / "prompts" / f"{name}.md").write_text(task["prompt"])
    proxy = None
    base_url = ""
    if config:
        proxy = MeterProxy(
            config["upstream"], key=keys.get(), headers=config.get("headers"),
            sampling=config.get("sampling"), stream_usage=config.get("stream_usage", True),
            auth=config.get("auth", "auto"),
        ).start()
        base_url = proxy.base_url
    run_id = uuid.uuid4().hex
    env = agent_env(args, task, ws, home, base_url, config, run_id)
    started_at = datetime.now(timezone.utc).isoformat(timespec="seconds")
    agent = run_agent(args.agent, ws, env, args.wall, out / "logs" / name)
    calls = proxy.stop() if proxy else []
    if calls:
        with open(out / "calls" / f"{name}.jsonl", "w") as fh:
            for call in calls:
                fh.write(json.dumps(call) + "\n")
    verdict = grade(name, ws)
    keep = out / "solutions" / name
    keep.mkdir(parents=True, exist_ok=True)
    for rel in task["solution_paths"]:
        if (ws / rel).is_file():
            (keep / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ws / rel, keep / rel)
    (out / "logs" / f"{name}.grade.txt").write_text(verdict.pop("tail") + "\n")
    row = {
        "task": name,
        "language": task["language"],
        "passed": verdict["passed"],
        **agent,
        "grade_s": verdict["grade_s"],
        "summary": verdict["summary"],
        "missing_solution_files": verdict["missing_solution_files"],
        **(usage_totals(calls) if config else {}),
        "started": started_at,
    }
    if not args.keep_workspaces:
        shutil.rmtree(out / "ws" / name, ignore_errors=True)
        shutil.rmtree(home, ignore_errors=True)
    with lock:
        with open(out / "results.jsonl", "a") as fh:
            fh.write(json.dumps(row) + "\n")
    mark = "PASS" if row["passed"] else ("TIME" if row["timed_out"] else "FAIL")
    tokens = f"  {row['calls']} calls  {row['input'] / 1e3:.0f}k in" if config else ""
    print(f"{mark}  {name:32} {row['agent_s']:7.1f}s{tokens}", flush=True)
    return row


def run(args) -> int:
    tasks = load_tasks()
    picked = select_tasks(tasks, args.tasks)
    config = load_config(args.config)
    if config and not config.get("upstream"):
        raise SystemExit("config has no upstream URL")
    keys = KeySource(config)
    out: Path = args.out
    for sub in ("logs", "calls", "prompts", "solutions", "ws", "homes"):
        (out / sub).mkdir(parents=True, exist_ok=True)
    done = {r["task"] for r in read_jsonl(out / "results.jsonl")}
    if done and not args.resume:
        raise SystemExit(f"{out} already has results; pass --resume to continue it, or pick a new --out")
    todo = [t for t in picked if t["name"] not in done]
    meta_path = out / "run.json"
    meta = json.loads(meta_path.read_text()) if meta_path.exists() else {
        "kit": KIT_VERSION,
        "label": args.label,
        "catalog_sha256": catalog_digest(tasks),
        "agent": args.agent,
        "config": {k: v for k, v in config.items() if not k.startswith("_")},
        "wall_secs": args.wall,
        "jobs": args.jobs,
        "host": {"platform": platform.platform(), "cpus": os.cpu_count(), "python": sys.version.split()[0]},
        "segments": [],
    }
    segment = {"started": datetime.now(timezone.utc).isoformat(timespec="seconds"), "tasks": len(todo)}
    meta["segments"].append(segment)
    meta_path.write_text(json.dumps(meta, indent=1) + "\n")
    print(f"{len(todo)} tasks to run ({len(done)} already done) -> {out}", flush=True)
    clock = time.monotonic()
    lock = threading.Lock()
    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        list(pool.map(lambda t: run_task(args, t, config, keys, lock), todo))
    segment["finished"] = datetime.now(timezone.utc).isoformat(timespec="seconds")
    segment["elapsed_s"] = round(time.monotonic() - clock, 1)
    meta_path.write_text(json.dumps(meta, indent=1) + "\n")
    if not args.keep_workspaces:
        shutil.rmtree(out / "ws", ignore_errors=True)
        shutil.rmtree(out / "homes", ignore_errors=True)
    text = summary_text(out)
    (out / "summary.md").write_text(text)
    print("\n" + text)
    return 0
