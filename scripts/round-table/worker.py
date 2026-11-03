#!/usr/bin/env python3
"""Idle without model calls; run one native CLI turn for each queued room task."""
import argparse
import fcntl
import json
import os
import re
from pathlib import Path
import signal
import subprocess
import sys
import time
import tomllib
from datetime import datetime
from zoneinfo import ZoneInfo

sys.path.insert(0, str(Path(__file__).resolve().parent))
from table import DEFAULT_STATE, Table

SCHEMA = {"type": "object", "properties": {
    "reply": {"type": "string"},
    "pings": {"type": "array", "maxItems": 2, "items": {"type": "object", "properties": {
        "team": {"type": "string", "enum": ["v", "grok", "muse"]},
        "question": {"type": "string"}}, "required": ["team", "question"], "additionalProperties": False}}},
    "required": ["reply", "pings"], "additionalProperties": False}


def prompt_for(task, name):
    messages = []
    budget = 16000
    for message in reversed(task["transcript"]):
        line = f"{message['name']}: {message['text']}"
        if len(line) > budget:
            if not messages:
                messages.append(line[-budget:])
            break
        messages.append(line)
        budget -= len(line)
    return f"""You are {name}, a local worker in the user's Round Table conversation.
You are separate from their existing cloud agent chats. You know only the room
context below. Do not claim to remember another agent's private conversations,
grant links or documents that are absent here. Ask for missing material directly.
This is a discussion turn: do not use tools, run commands, edit files, contact
people, or make commitments. Give a useful, concise answer (usually 100–250 words).
Distinguish evidence from assumptions. For funding discussion, do not invent
grant eligibility, investor interest, deadlines, or facts about the user's work.
The human wants the best balance of responsiveness and token use.
Return ONLY JSON matching {{"reply":"your message","pings":[]}}.
A ping calls another local worker and uses tokens. Use zero pings for ordinary
answers. If a peer's distinct view is necessary, request at most one or two
specific questions for teams v (Codex), grok, or muse. Never ping yourself.
{'This is already a peer ping: pings MUST be empty.' if task['peer_turn'] else 'Avoid redundant peer calls; everyone already selected for this discussion will reply.'}
Human question: {task['question'][:12000]}
Specific peer request, if any: {task['peer_request']}
Turn instruction: {task['instruction']}
Recent room context (may omit older messages):
{chr(10).join(reversed(messages))}
"""


def extract_reply(text):
    def visit(value, depth=0):
        if depth > 12:
            return None
        if isinstance(value, dict):
            if isinstance(value.get("reply"), str) and isinstance(value.get("pings"), list):
                return value
            for child in reversed(list(value.values())):
                found = visit(child, depth + 1)
                if found:
                    return found
        elif isinstance(value, list):
            for child in reversed(value):
                found = visit(child, depth + 1)
                if found:
                    return found
        elif isinstance(value, str):
            clean = value.strip()
            if clean.startswith("```"):
                clean = "\n".join(clean.splitlines()[1:-1])
            try:
                return visit(json.loads(clean), depth + 1)
            except (ValueError, RecursionError):
                pass
        return None
    for candidate in [text, *reversed(text.splitlines())]:
        found = visit(candidate)
        if found and found["reply"].strip():
            return found
    raise ValueError("The native client did not return a valid room response.")


def command_for(provider, job):
    runtime = str(Path.home() / ".local/bin" / provider)
    if provider == "codex":
        config = Path.home() / ".codex/config.toml"
        model = tomllib.loads(config.read_text()).get("model") if config.exists() else None
        cmd = [runtime, "exec", "--ignore-user-config", "--ephemeral", "--skip-git-repo-check",
               "--sandbox", "read-only", "--disable", "shell_tool", "--disable", "multi_agent",
               "-c", 'web_search="disabled"', "-c", 'model_reasoning_effort="low"',
               "--output-schema", str(job / "schema.json"), "--output-last-message", str(job / "answer.json"),
               "--json", "-"]
        if model:
            cmd[2:2] = ["--model", model]
        return cmd
    if provider == "grok":
        return [runtime, "--tools", "", "--no-subagents", "--disable-web-search", "--max-turns", "1",
                "--reasoning-effort", "low", "--output-format", "json", "--json-schema", json.dumps(SCHEMA),
                "--prompt-file", str(job / "prompt.txt")]
    return [runtime, "exec", "--preset", "empty", "--disable-shell", "--disable-write",
            "--disable-web-tools", "--disable-reminders", "--disable-request-user-input",
            "--no-foreign-personal-context", "--reasoning-effort", "low", "--max-model-steps", "1",
            "--output-schema", str(job / "schema.json"), "--json", "--prompt-file", str(job / "prompt.txt")]


def run_task(table, route, task):
    agent, provider = route["agent"], route["provider"]
    with table.db() as db:
        name = db.execute("SELECT name FROM agents WHERE id=?", (agent,)).fetchone()[0]
    job = table.root / "worker-runs" / f"{task['id']}-{provider}-{time.time_ns()}"
    job.mkdir(parents=True, mode=0o700)
    workspace = table.root / "worker-workspaces" / provider
    workspace.mkdir(parents=True, exist_ok=True, mode=0o700)
    prompt = prompt_for(task, name)
    (job / "prompt.txt").write_text(prompt)
    (job / "schema.json").write_text(json.dumps(SCHEMA))
    table.worker_health(agent, "thinking")
    command = command_for(provider, job)
    process = None
    try:
        with (job / "events.jsonl").open("w") as output, (job / "stderr.txt").open("w") as errors:
            process = subprocess.Popen(command, cwd=workspace, stdin=subprocess.PIPE if provider == "codex" else subprocess.DEVNULL,
                                       stdout=output, stderr=errors, start_new_session=True)
            if provider == "codex":
                process.stdin.write(prompt.encode())
                process.stdin.close()
            deadline = time.monotonic() + 180
            while process.poll() is None:
                with table.db() as db:
                    state = db.execute("SELECT state FROM tasks WHERE id=?", (task["id"],)).fetchone()[0]
                if state == "cancelled":
                    raise InterruptedError("Discussion stopped")
                if time.monotonic() >= deadline:
                    raise TimeoutError("Native client took longer than 180 seconds")
                table.worker_health(agent)
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    pass
        if process.returncode:
            raise RuntimeError(f"{provider} exited with status {process.returncode}; see {job.name}")
        answer = job / "answer.json"
        text = answer.read_text() if answer.exists() and answer.stat().st_size else (job / "events.jsonl").read_text()
        result = extract_reply(text)
        table.reply(agent, task["id"], result["reply"], result["pings"])
        table.worker_health(agent, "ready")
        print(f"{provider}: delivered task {task['id']}", flush=True)
    except InterruptedError:
        table.worker_health(agent, "ready")
    except Exception as exc:
        (job / "failure.txt").write_text(str(exc))
        detail = (job / "stderr.txt").read_text() if (job / "stderr.txt").exists() else ""
        reset = re.search(r"resets at (\d{4}-\d\d-\d\dT[\d:]+Z)", detail)
        retry_after = 0
        message = f"{provider} response failed. See local run {job.name}; no automatic model retry."
        if reset and ("quota" in detail.lower() or "rate_limit" in detail.lower()):
            reset_time = datetime.fromisoformat(reset[1].replace("Z", "+00:00"))
            retry_after = reset_time.timestamp()
            local = reset_time.astimezone(ZoneInfo("America/Chicago"))
            message = f"{provider} subscription quota exhausted; resets {local.strftime('%b %d at %-I:%M %p')} Chicago time. No requests before reset."
        table.worker_failed(agent, task["id"], message, retry_after)
        print(f"{provider}: task {task['id']} failed; see {job.name}", flush=True)
    finally:
        if process and process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--state-dir", type=Path, default=DEFAULT_STATE)
    parser.add_argument("--team", choices=("v", "grok", "muse"))
    parser.add_argument("--configure", action="store_true")
    args = parser.parse_args()
    os.umask(0o077)
    table = Table(args.state_dir)
    if args.configure:
        for team, provider in (("v", "codex"), ("grok", "grok"), ("muse", "muse")):
            print(table.configure_worker(team, provider))
        with table.db() as db:
            db.execute("UPDATE tasks SET state='cancelled' WHERE state='queued' AND agent IN (SELECT a.id FROM agents a WHERE a.joined=0 AND NOT EXISTS (SELECT 1 FROM workers w WHERE w.agent=a.id))")
        return
    if not args.team:
        parser.error("--team is required")
    with table.db() as db:
        route = dict(db.execute("SELECT * FROM workers WHERE team=?", (args.team,)).fetchone())
    with (table.root / f"worker-{args.team}.lock").open("w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        if route["status"] not in ("error", "cooldown"):
            table.worker_health(route["agent"], "ready")
        next_heartbeat = 0
        while True:
            if time.monotonic() >= next_heartbeat:
                table.worker_health(route["agent"])
                next_heartbeat = time.monotonic() + 5
            with table.db() as db:
                current = dict(db.execute("SELECT * FROM workers WHERE agent=?", (route["agent"],)).fetchone())
                pending = db.execute("SELECT 1 FROM tasks t JOIN rounds r ON r.id=t.round_id WHERE t.agent=? AND r.state='active' AND t.state IN ('queued','leased') LIMIT 1", (route["agent"],)).fetchone()
            if current["retry_after"] > time.time():
                time.sleep(1)
                continue
            if current["status"] == "cooldown":
                table.worker_health(route["agent"], "ready")
            task = table.task(route["agent"]) if pending else None
            if task:
                run_task(table, route, task)
            else:
                time.sleep(1)


if __name__ == "__main__":
    main()
