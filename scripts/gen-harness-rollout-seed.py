#!/usr/bin/env python3
"""Generate a seed harness-rollout corpus through the real cockpit mechanics.

Runs offline task episodes against a scripted model (the same pattern the
contract tests use), with `--rollout local` capture, then exports the audited
corpus with `angel --export-harness-rollout --all`. The policy is scripted, so
this is a PLUMBING seed — real-mechanic reward/verifier/eligibility handling,
not real policy data — labeled as such in the export directory receipt.

Usage:
  python3 scripts/gen-harness-rollout-seed.py --angel-bin cockpit/target/debug/angel
  python3 scripts/gen-harness-rollout-seed.py --out .angel/harness-rollout-seed/corpus.json --episodes green,stop

Exit 0 = corpus exported; 1 = an episode or the export failed.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

GREEN_PATCH = """*** Begin Patch
*** Update File: src/lib.rs
@@
-pub fn accepted() -> bool { false }
+pub fn accepted() -> bool { true }
*** End Patch"""


def _scripted_fixture(workspace: Path) -> None:
    """The operator owns the test; the scripted policy only repairs the source."""
    (workspace / "src").mkdir(exist_ok=True)
    (workspace / "tests").mkdir(exist_ok=True)
    (workspace / "Cargo.toml").write_text(
        '[package]\nname = "rollout-seed"\nversion = "0.1.0"\nedition = "2021"\n'
    )
    (workspace / "src/lib.rs").write_text("pub fn accepted() -> bool { false }\n")
    (workspace / "tests/acceptance.rs").write_text(
        "#[test]\nfn seed_is_green() { assert!(rollout_seed::accepted()); }\n"
    )



def _sse_tool(call_id: str, name: str, arguments: dict) -> bytes:
    payload = {
        "choices": [
            {
                "delta": {
                    "tool_calls": [
                        {
                            "index": 0,
                            "id": call_id,
                            "function": {
                                "name": name,
                                "arguments": json.dumps(arguments),
                            },
                        }
                    ]
                }
            }
        ]
    }
    finish = {
        "choices": [{"delta": {}, "finish_reason": "tool_calls"}],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 1,
            "total_tokens": 11,
            "prompt_tokens_details": {"cached_tokens": 0},
        },
    }
    return (
        f"data: {json.dumps(payload)}\n\ndata: {json.dumps(finish)}\n\n"
        "data: [DONE]\n\n"
    ).encode()


def _sse_text(content: str) -> bytes:
    payload = {"choices": [{"delta": {"content": content}}]}
    finish = {
        "choices": [{"delta": {}, "finish_reason": "stop"}],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 1,
            "total_tokens": 11,
            "prompt_tokens_details": {"cached_tokens": 0},
        },
    }
    return (
        f"data: {json.dumps(payload)}\n\ndata: {json.dumps(finish)}\n\n"
        "data: [DONE]\n\n"
    ).encode()


DIALOGUES = {
    # A completed repair: patch a green crate, typed verification, answer.
    "green": [
        _sse_tool("seed-patch", "apply_patch", {"diff": GREEN_PATCH}),
        _sse_tool("seed-verify", "run_tests", {}),
        _sse_text("Implemented and verified the repair."),
    ],
    # A stopped horizon: read-only inspection until max_hops (four reads
    # covers the max_hops=4 budget plus any nudge-driven re-request).
    "stop": [
        _sse_tool("seed-read-1", "read_file", {"path": "missing.rs"}),
        _sse_tool("seed-read-2", "read_file", {"path": "missing.rs"}),
        _sse_tool("seed-read-3", "read_file", {"path": "missing.rs"}),
        _sse_tool("seed-read-4", "read_file", {"path": "missing.rs"}),
    ],
}


class ScriptedServer:
    def __init__(self, responses: list[bytes]):
        self.responses = responses
        self.requests: list[dict] = []
        owner = self

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):  # noqa: N802
                length = int(self.headers.get("Content-Length", "0"))
                owner.requests.append(json.loads(self.rfile.read(length)))
                index = len(owner.requests) - 1
                if index >= len(owner.responses):
                    self.send_error(500, "script exhausted")
                    return
                body = owner.responses[index]
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Content-Length", str(len(body)))
                self.send_header("Connection", "close")
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, _format, *_args):
                return

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    def __enter__(self):
        self.thread.start()
        host, port = self.server.server_address
        self.url = f"http://{host}:{port}/v1/chat/completions"
        return self

    def __exit__(self, *_exc):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)


class _NullServer:
    """Stand-in for real-provider episodes (no scripted server)."""

    url = None

    def __enter__(self):
        return self

    def __exit__(self, *_exc):
        return False


def _nullserver() -> _NullServer:
    return _NullServer()


def _reset_workspace(workspace: Path, fixture: Path) -> None:
    """Clear the workspace except .angel (the rollout store) and copy the
    fixture in, so consecutive episodes share one repo_key."""
    if not fixture.is_dir():
        raise ValueError(f"fixture not found: {fixture}")
    for child in workspace.iterdir():
        if child.name == ".angel0":
            continue
        if child.is_dir() and not child.is_symlink():
            for entry in child.rglob("*"):
                if entry.is_file() or entry.is_symlink():
                    entry.unlink()
            for entry in sorted(child.rglob("*"), reverse=True):
                if entry.is_dir() and not entry.is_symlink():
                    entry.rmdir()
            child.rmdir()
        else:
            child.unlink()
    for entry in fixture.rglob("*"):
        relative = entry.relative_to(fixture)
        destination = workspace / relative
        if entry.is_dir():
            destination.mkdir(parents=True, exist_ok=True)
        elif entry.is_file() or entry.is_symlink():
            destination.parent.mkdir(parents=True, exist_ok=True)
            if not destination.exists():
                import shutil

                shutil.copy2(entry, destination)


def _apply_setup_patch(workspace: Path, patch_path: str, name: str) -> None:
    """git-init the workspace, commit the fixture, and apply the task's
    setup patch so 'inspect the existing git diff' prompts see real state."""
    patch = Path(patch_path)
    if not patch.is_file():
        raise ValueError(f"setup patch not found for {name}: {patch_path}")
    for command in [
        ["git", "-C", str(workspace), "init", "-q"],
        ["git", "-C", str(workspace), "config", "user.email", "rollout-seed@angel.invalid"],
        ["git", "-C", str(workspace), "config", "user.name", "Rollout Seed"],
        ["git", "-C", str(workspace), "add", "."],
        ["git", "-C", str(workspace), "commit", "-q", "-m", "frozen fixture"],
    ]:
        completed = subprocess.run(command, capture_output=True, text=True, check=False)
        if completed.returncode != 0:
            raise ValueError(f"setup failed for {name}: {' '.join(command)}: {completed.stderr[-200:]}")
    applied = subprocess.run(
        ["git", "-C", str(workspace), "apply", str(patch)],
        capture_output=True,
        text=True,
        check=False,
    )
    if applied.returncode != 0:
        raise ValueError(f"setup patch failed for {name}: {applied.stderr[-300:]}")


def run_episode(
    angel_bin: str,
    workspace: Path,
    url: str | None,
    dialogue: list[bytes] | None,
    *,
    task_id: str,
    prompt: str = "repair the fixture and verify it",
    real: bool = False,
) -> dict:
    env = {
        **os.environ,
        # The cockpit's verifier tools shell out to cargo; keep the toolchain
        # findable regardless of the caller's PATH.
        "PATH": f"{Path.home()}/.cargo/bin:{Path.home()}/.local/bin:"
        + os.environ.get("PATH", ""),
        "ANGEL_MAX_HOPS": "12" if real else "4",
        "ANGEL_FINAL_MILE_HOPS": "6" if real else "3",
        "ANGEL_FIRST_WRITE_CALLS": "0",
        "ANGEL_PROJECT_DOC": "0",
        "ANGEL_TOOL_SCHEMA_PROFILE": "essential",
        "ANGEL_TASK_STRICT_EXIT": "0",
        "ANGEL_HTTP_RETRIES": "0",
        "ANGEL_PROVIDER_RETRIES": "0",
        "ANGEL_HARNESS_ROLLOUT_DIR": str(workspace / ".angel0" / "harness-rollouts"),
    }
    if real:
        # Free preturn repo map + project doc: paid hops stay for the work.
        env["ANGEL_TASK_RECON"] = "repo"
        env["ANGEL_PROJECT_DOC"] = "1"
    if not real:
        assert url is not None
        # Private operator state is a sibling of the task workspace, so the
        # verifier's trusted Cargo home is outside the task's writable tree.
        # The typed verifier refuses any private Cargo-home parent inside a
        # task-writable root, and /tmp counts as task-writable even when the
        # episode workspace itself is a tempdir, so the operator home must
        # sit under the real $HOME (mkdtemp keeps each run isolated).
        operator_home = Path.home() / ".angel0" / "rollout-seed-operator"
        operator_home.mkdir(parents=True, exist_ok=True)
        # Resolve the installed toolchain before redirecting HOME. No download.
        cargo = subprocess.run(
            ["rustup", "which", "cargo"], capture_output=True, text=True, check=True,
            env={**os.environ, "RUSTUP_AUTO_INSTALL": "0"},
        ).stdout.strip()
        env.update({
            "HOME": str(operator_home),
            "RUSTUP_HOME": str(operator_home / ".rustup"),
            "CARGO": cargo,
            "CARGO_NET_OFFLINE": "true",
            "RUSTUP_AUTO_INSTALL": "0",
            "ANGEL_YOLO": "1",
            "ANGEL_TASK_RECON": "0",
            "ANGEL_SKILL_HINT": "0",
            "ANGEL_ADVISOR": "0",
            "ANGEL_MCP_CONFIG": str(operator_home / "no-mcp.json"),
        })
        env.update(
            {
                "ANGEL_DRIVER": "openrouter",
                # The club gate: without it the openrouter club is "busted" and
                # every episode stops with driver_unavailable (2026-09-08).
                "ANGEL_API_CLUBS": "openrouter",
                "ANGEL_OPENROUTER_URL": url,
                "ANGEL_OPENROUTER_KEY": "offline-seed-key",
                "OPENROUTER_API_KEY": "offline-seed-key",
                "ANGEL_OPENROUTER_MODEL": "offline-seed-model",
            }
        )
    completed = subprocess.run(
        [
            angel_bin,
            "--task-json",
            "--workspace",
            str(workspace),
            "--rollout",
            "local",
            "--task-id",
            task_id,
            "--run-id",
            task_id,
            prompt,
        ],
        env=env,
        capture_output=True,
        text=True,
        timeout=1500 if real else 120,
        check=False,
    )
    envelope = {}
    if completed.stdout.strip():
        try:
            envelope = json.loads(completed.stdout)
        except json.JSONDecodeError:
            envelope = {"stdout_tail": completed.stdout[-400:]}
    return {
        "task_id": task_id,
        "exit_code": completed.returncode,
        "status": envelope.get("status"),
        "stop_reason": envelope.get("stop_reason"),
        "hops": envelope.get("hops"),
        "rollout_id": envelope.get("rollout_id"),
        "reward_binding": envelope.get("reward_binding"),
        "tools": envelope.get("tools", []),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--angel-bin",
        default=str(Path(__file__).resolve().parents[1] / "cockpit/target/debug/angel"),
    )
    parser.add_argument(
        "--episodes",
        default="green,stop",
        help="comma-separated dialogue names from DIALOGUES",
    )
    parser.add_argument("--out", default=None, help="corpus.json output path")
    parser.add_argument(
        "--real",
        action="store_true",
        help="run REAL provider episodes against the daily-driver fixtures "
             "instead of scripted dialogues (no scripted server)",
    )
    parser.add_argument(
        "--fixture-root",
        default=str(Path(__file__).resolve().parents[1] / "benchmarks/action-agent"),
        help="root containing fixtures/ and daily-driver-tasks.json",
    )
    args = parser.parse_args(argv)
    angel_bin = str(Path(args.angel_bin).expanduser())
    if args.real:
        fixture_root = Path(args.fixture_root)
        tasks_path = fixture_root / "daily-driver-tasks.json"
        tasks = json.loads(tasks_path.read_text())
        requested = {name.strip() for name in args.episodes.split(",") if name.strip()}
        episodes = [
            {
                "name": task["name"],
                "prompt": task["prompt"],
                "fixture": fixture_root / task["fixture"],
                "setup_patch": (
                    fixture_root / task["setup_patch"] if task.get("setup_patch") else None
                ),
            }
            for task in tasks
            if not requested or task["name"] in requested
            or task.get("category") in requested
        ]
        if not episodes:
            parser.error(f"no daily-driver tasks match {requested or 'all'}")
    else:
        episodes = [name.strip() for name in args.episodes.split(",") if name.strip()]
        for name in episodes:
            if name not in DIALOGUES:
                parser.error(f"unknown episode {name!r}")
        episodes = [{"name": name, "prompt": None, "fixture": None} for name in episodes]

    workspace = Path(tempfile.mkdtemp(prefix="angel-rollout-seed-"))
    results = []
    for index, episode in enumerate(episodes, start=1):
        if not args.real:
            _scripted_fixture(workspace)
        if args.real:
            _reset_workspace(workspace, episode["fixture"])
            if episode.get("setup_patch"):
                _apply_setup_patch(workspace, episode["setup_patch"], episode["name"])
        with ScriptedServer(DIALOGUES[episode["name"]]) if not args.real else _nullserver() as server:
            result = run_episode(
                angel_bin,
                workspace,
                server.url if not args.real else None,
                DIALOGUES[episode["name"]] if not args.real else None,
                task_id=f"seed-{index}-{episode['name']}",
                prompt=episode["prompt"] or "repair the fixture and verify it",
                real=args.real,
            )
            result["name"] = episode["name"]
            results.append(result)
            print(f"[episode] {json.dumps(result)}", flush=True)

    out = args.out or str(workspace / "harness-rollout-corpus.json")
    # The export must resolve the SAME store the episodes captured into.
    export_env = {
        **os.environ,
        "ANGEL_HARNESS_ROLLOUT_DIR": str(workspace / ".angel0" / "harness-rollouts"),
    }
    exported = subprocess.run(
        [angel_bin, "--export-harness-rollout", "--workspace", str(workspace),
         "--all", out],
        env=export_env,
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
    )
    if exported.returncode != 0:
        print(f"export failed: {exported.stderr[-1500:]}", file=sys.stderr)
        return 1
    # The corpus is written to the output file; stdout carries the operator
    # confirmation, not the payload.
    try:
        corpus = json.loads(Path(out).read_text())
    except (OSError, json.JSONDecodeError) as error:
        print(f"corpus read failed: {error}", file=sys.stderr)
        return 1
    records = corpus.get("records", [])
    summary = {
        "workspace": str(workspace),
        "episodes": results,
        "corpus": out,
        "schema": corpus.get("schema"),
        "audit": corpus.get("audit"),
        "exported": len(records),
        "eligible": sum(
            1
            for rec in records
            if isinstance(rec.get("source_rollout"), dict)
            and rec["source_rollout"].get("eligibility", {}).get("status") == "eligible"
        ),
        "by_reward_owner": sorted({rec.get("reward_owner") for rec in records}),
    }
    print(json.dumps(summary, indent=2))
    return 0 if records else 1


if __name__ == "__main__":
    sys.exit(main())
