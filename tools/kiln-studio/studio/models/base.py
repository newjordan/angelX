"""One interface for the owner's models.

    adapter.ask(prompt, images=(), workdir=..., budget=seconds) -> Answer

An Answer is the model's last text, the files it wrote in workdir, and what
the call cost: seconds, and the tokens, dollars and turns the model reports.
Every call is appended to the ledger a caller passes, so a job can account
for its time and calls.
"""
import json
import os
import subprocess
import threading
import time
from dataclasses import asdict, dataclass, field
from pathlib import Path


class ModelError(Exception):
    """A model could not be called: down, blocked by auth or permission, timed out."""


@dataclass
class Answer:
    model: str
    text: str = ""
    files: list = field(default_factory=list)
    ok: bool = True
    error: str = ""
    seconds: float = 0
    cost_usd: float = 0
    tokens: int = 0
    turns: int = 0
    session: str = ""
    log: str = ""
    denials: list = field(default_factory=list)
    data: dict = field(default_factory=dict)

    def as_dict(self):
        d = asdict(self)
        d["text"] = d["text"][:4000]
        return d


class Ledger:
    """The calls a job made, one JSON line each, in calls.jsonl."""

    def __init__(self, path=None):
        self.path = Path(path) if path else None
        self.lock = threading.Lock()
        self.calls = []

    def add(self, stage, answer, **extra):
        row = {"t": round(time.time(), 1), "stage": stage, "model": answer.model, "ok": answer.ok,
               "seconds": round(answer.seconds, 1), "cost_usd": round(answer.cost_usd, 4), "tokens": answer.tokens,
               "turns": answer.turns, **({"error": answer.error[:300]} if answer.error else {}), **extra}
        with self.lock:
            self.calls.append(row)
            if self.path:
                with open(self.path, "a") as f:
                    f.write(json.dumps(row) + "\n")
        return row


class Adapter:
    name = ""
    vision = False  # can look at images
    agentic = False  # runs tools (kiln's MCP) in a workdir

    def available(self):
        """(True, what) when callable, else (False, why)."""
        raise NotImplementedError

    def ask(self, prompt, images=(), workdir=None, budget=600, **opts):
        raise NotImplementedError


def files_in(workdir, since):
    """Files in workdir changed at or after `since`, relative, logs left out."""
    if not workdir:
        return []
    root = Path(workdir)
    found = []
    for p in root.rglob("*"):
        if p.is_file() and p.stat().st_mtime >= since and not p.name.endswith((".log", ".jsonl")):
            found.append(str(p.relative_to(root)))
    return sorted(found)[:200]


def run_logged(cmd, log_path, stdin_text=None, cwd=None, env=None, timeout=600, alive=None, stall=None):
    """Run a model CLI with its output in log_path; (exit code, seconds). The
    code is None on a timeout, and "stalled" when `alive` (a test on the
    log's text) has not passed `stall` seconds in. A killed run's process
    group is killed."""
    t0 = time.time()
    seen = alive is None
    with open(log_path, "w") as log:
        p = subprocess.Popen(cmd, cwd=cwd, env=env, stdin=subprocess.PIPE if stdin_text is not None else subprocess.DEVNULL,
                             stdout=log, stderr=subprocess.STDOUT, text=True, start_new_session=True)
        if stdin_text is not None:
            p.stdin.write(stdin_text)
            p.stdin.close()
        code = None
        while True:
            try:
                code = p.wait(timeout=2)
                break
            except subprocess.TimeoutExpired:
                pass
            now = time.time()
            if not seen and stall and now - t0 > stall:
                seen = alive(Path(log_path).read_text(errors="replace"))
                if not seen:
                    code = "stalled"
            if now - t0 >= timeout or code == "stalled":
                try:
                    os.killpg(p.pid, 15)
                    p.wait(timeout=10)
                except Exception:
                    os.killpg(p.pid, 9)
                break
    return code, time.time() - t0
