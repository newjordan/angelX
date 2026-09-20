#!/usr/bin/env python3
"""Offline harness stress bench: drive the real `angel --task-json` binary with
a scripted OpenAI-compatible SSE model on localhost and measure the HARNESS,
not the model.

Every scenario is a Python policy: given (hop, request_json) it returns either
a list of tool calls or a final text. The server timestamps each request and
response so the per-hop "harness gap" (response sent -> next request received,
= tool execution + context assembly + trajectory/rollout writes) is measured
independently of the receipt's own `angel-task-timing/v1` block. RSS is polled
from /proc while the run lasts.

Config is printed first; a number without its config is noise.

Usage:
  python3 scripts/harness-stress.py [--angel-bin BIN] [--scenario NAME|all]
                                    [--hops N] [--out DIR] [--json]
Scenarios: hops, fanout, bigout, errors, editstorm, readstorm, timeout, shellcost,
           flaky500, ratelimit, cutstream, badmodel, deadline, compact, slowmodel
           (--concurrent N runs N copies of one scenario sharing a single HOME)
Exit 0 = every scenario ran to its expected stop; 1 = a scenario diverged.
--gate additionally enforces the speed floors in GATE_FLOORS (regression gate for
harness changes): python3 scripts/harness-stress.py --gate --hops 30
"""

from __future__ import annotations

from trace_schema import attach_receipt_header
import argparse
import json
import os
import platform
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]


# --------------------------------------------------------------------------
# SSE encoding
# --------------------------------------------------------------------------
def scripted_usage(prompt_tokens: int, completion_tokens: int) -> dict:
    """Synthetic inclusive counts, never a measurement of a model tokenizer."""
    return dict(prompt_tokens=prompt_tokens, completion_tokens=completion_tokens,
                total_tokens=prompt_tokens + completion_tokens,
                prompt_tokens_details=dict(cached_tokens=0, cache_write_tokens=0),
                completion_tokens_details=dict(reasoning_tokens=0))


def with_scripted_usage(content_type: str, body: bytes, request_bytes: int,
                        usage_constant: bool = False) -> tuple[bytes, dict]:
    """Complete ordinary fixtures; raw/error/cut-stream fault policies bypass this.

    Default counts are deterministic byte quarters (at least one), overriding
    fixture counts. The constant escape restores 500/20 for legacy tests.
    Reply bytes are measured before usage injection; the terminal frame is cumulative.
    """
    if content_type == "application/json":
        frames = [json.loads(body)]
    else:
        frames = [json.loads(line[6:]) for line in body.decode().splitlines()
                  if line.startswith("data: ") and line != "data: [DONE]"]
    usage = (scripted_usage(500, 20) if usage_constant else
             scripted_usage(max(1, (request_bytes + 3) // 4),
                            max(1, (len(body) + 3) // 4)))
    # One complete observation avoids accidental double-counting by test clients.
    for frame in frames:
        frame.pop("usage", None)
    if content_type == "application/json":
        frames[0]["usage"] = usage
        return json.dumps(frames[0]).encode(), usage
    frames.append(dict(choices=[], usage=usage))
    return ("".join("data: " + json.dumps(f) + "\n\n" for f in frames)
            + "data: [DONE]\n\n").encode(), usage


def sse_calls(calls: list[tuple[str, str, dict]]) -> bytes:
    """calls = [(id, name, args), ...] emitted as ONE delta chunk (parallel batch)."""
    payload = {
        "choices": [
            {
                "delta": {
                    "tool_calls": [
                        {
                            "index": i,
                            "id": cid,
                            "type": "function",
                            "function": {"name": name, "arguments": json.dumps(args)},
                        }
                        for i, (cid, name, args) in enumerate(calls)
                    ]
                }
            }
        ]
    }
    finish = {"choices": [{"delta": {}, "finish_reason": "tool_calls"}]}
    return (
        f"data: {json.dumps(payload)}\n\ndata: {json.dumps(finish)}\n\n" "data: [DONE]\n\n"
    ).encode()


def sse_text(content: str) -> bytes:
    payload = {"choices": [{"delta": {"content": content}}]}
    finish = {"choices": [{"delta": {}, "finish_reason": "stop"}]}
    return (
        f"data: {json.dumps(payload)}\n\ndata: {json.dumps(finish)}\n\n" "data: [DONE]\n\n"
    ).encode()


# --------------------------------------------------------------------------
# Scenarios: policy(hop, request) -> ("calls", [...]) | ("text", str)
# Each scenario also declares the expected stop (status/stop_reason) and a
# workspace seeder.
# --------------------------------------------------------------------------
def seed_basic(ws: Path, hops: int) -> None:
    (ws / "src").mkdir(parents=True, exist_ok=True)
    lines = [f"pub fn f{i}() -> u32 {{ {i} }}  // line {i}" for i in range(400)]
    (ws / "src" / "lib.rs").write_text("\n".join(lines) + "\n")
    (ws / "README.md").write_text("# stress fixture\n")
    for i in range(hops + 2):
        (ws / "src" / f"m{i}.rs").write_text(f"pub const M{i}: u32 = {i};\n")


def make_scenarios(hops: int) -> dict:
    S: dict[str, dict] = {}

    # 1. hops: N sequential trivial tool calls, then answer. Pure per-hop overhead.
    def hops_policy(h, _req):
        if h < hops:
            return ("calls", [(f"h{h}", "read_file", {"path": f"src/m{h}.rs"})])
        return ("text", "Done: inspected every module.")

    S["hops"] = dict(policy=hops_policy, seed=seed_basic, expect=("completed", None))

    # 2. fanout: N hops each with 12 parallel reads. Batch dispatch parallelism.
    def fanout_policy(h, _req):
        if h < hops:
            return (
                "calls",
                [(f"f{h}-{k}", "read_file", {"path": f"src/m{(h + k) % (hops + 2)}.rs"}) for k in range(12)],
            )
        return ("text", "Done: fan-out inspection complete.")

    S["fanout"] = dict(policy=fanout_policy, seed=seed_basic, expect=("completed", None))

    # 3. bigout: shell producing large outputs each hop (1 MB, 4 MB, 16 MB
    #    cycling). Truncation cost + request-size growth.
    def bigout_policy(h, _req):
        if h < hops:
            mb = [1, 4, 16][h % 3]
            return (
                "calls",
                [(f"b{h}", "shell", {"command": f"echo hop {h}; yes xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx | head -c {mb * 1024 * 1024}"})],
            )
        return ("text", "Done: big outputs handled.")

    S["bigout"] = dict(policy=bigout_policy, seed=seed_basic, expect=("completed", None))

    # 4. errors: every call fails (missing file / bad anchor / bad regex / exit 1).
    def errors_policy(h, _req):
        if h < hops:
            kind = h % 4
            if kind == 0:
                call = (f"e{h}", "read_file", {"path": f"src/nope{h}.rs"})
            elif kind == 1:
                call = (f"e{h}", "str_replace", {"path": "src/lib.rs", "old": f"NOT-THERE-{h}", "new": "x"})
            elif kind == 2:
                call = (f"e{h}", "grep", {"pattern": "(unclosed", "path": "src"})
            else:
                call = (f"e{h}", "shell", {"command": f"echo failing hop {h} >&2; exit 1"})
            return ("calls", [call])
        return ("text", "Done: reported every error.")

    S["errors"] = dict(policy=errors_policy, seed=seed_basic, expect=("stopped", "error_stop"))

    # 5. editstorm: alternating write_file / str_replace on the 400-line file.
    def editstorm_policy(h, _req):
        if h < hops:
            if h % 2 == 0:
                return ("calls", [(f"w{h}", "str_replace", {"path": "src/lib.rs", "old": f"{{ {h} }}  // line {h}", "new": f"{{ {h} }}  // edited {h}"})])
            return ("calls", [(f"w{h}", "write_file", {"path": f"src/gen{h}.rs", "content": f"pub const G{h}: u32 = {h};\n" * 50})])
        return ("text", "Done: edits applied.")

    S["editstorm"] = dict(policy=editstorm_policy, seed=seed_basic, expect=("completed", None))

    # 6. readstorm: read the same 400-line file + grep + list_dir each hop
    #    (result-aging / dedupe pressure).
    def readstorm_policy(h, _req):
        if h < hops:
            return (
                "calls",
                [
                    (f"r{h}a", "read_file", {"path": "src/lib.rs"}),
                    (f"r{h}b", "grep", {"pattern": "pub fn", "path": "src"}),
                    (f"r{h}c", "list_dir", {"path": "src"}),
                ],
            )
        return ("text", "Done: repeated reads.")

    S["readstorm"] = dict(policy=readstorm_policy, seed=seed_basic, expect=("stopped", "spin"))

    # 7. timeout: one shell call sleeping past ANGEL_TOOL_TIMEOUT (set to 3 s
    #    for this scenario), then answer. Measures kill latency + honesty.
    def timeout_policy(h, _req):
        if h == 0:
            return ("calls", [("t0", "shell", {"command": "sleep 30; echo survived"})])
        return ("text", "Done: the sleep was killed, reported it.")

    S["timeout"] = dict(
        policy=timeout_policy,
        seed=seed_basic,
        expect=("completed", None),
        env={"ANGEL_TOOL_TIMEOUT": "3", "ANGEL_TOOL_HARD_TIMEOUT": "5"},
        yolo=False,  # --yolo returns None from tool_timeout(); only the 30 s idle floor would apply
    )
    # 7b. idlefloor: a legitimate but silent sleep-wait (35 s) under the default
    #     30 s idle floor and a generous tool timeout — documents the kill.
    def idlefloor_policy(h, _req):
        if h == 0:
            return ("calls", [("i0", "shell", {"command": "sleep 35; echo waited-ok"})])
        return ("text", "Done: reported the idle-floor result.")

    S["idlefloor"] = dict(policy=idlefloor_policy, seed=seed_basic, expect=("completed", None), yolo=False,
                          env={"ANGEL_TOOL_TIMEOUT": "120"})

    # 8. shellcost: trivial shell each hop -> per-call sandbox/exec cost.
    def shellcost_policy(h, _req):
        if h < hops:
            return ("calls", [(f"s{h}", "shell", {"command": f"echo hop {h}"})])
        return ("text", "Done: shells ran.")

    S["shellcost"] = dict(policy=shellcost_policy, seed=seed_basic, expect=("completed", None))

    # 9. flaky500: every 3rd model request fails once with HTTP 500 (retryable).
    #    Measures recovery wall (backoff) and that no hop is lost.
    failed_once: set = set()

    def flaky500_policy(h, _req):
        if h < hops:
            if h % 3 == 2 and h not in failed_once:
                failed_once.add(h)
                return ("http", 500)
            return ("calls", [(f"k{h}", "read_file", {"path": f"src/m{h}.rs"})])
        return ("text", "Done despite provider hiccups.")

    S["flaky500"] = dict(policy=flaky500_policy, seed=seed_basic, expect=("completed", None), hop_reuse=True)

    # 10. ratelimit: every 4th request answers 429 once.
    limited_once: set = set()

    def ratelimit_policy(h, _req):
        if h < hops:
            if h % 4 == 3 and h not in limited_once:
                limited_once.add(h)
                return ("http", 429)
            return ("calls", [(f"l{h}", "read_file", {"path": f"src/m{h}.rs"})])
        return ("text", "Done despite rate limits.")

    S["ratelimit"] = dict(policy=ratelimit_policy, seed=seed_basic, expect=("completed", None), hop_reuse=True)

    # 11. cutstream: every 4th response is cut mid-stream (no finish, no [DONE]).
    cut_once: set = set()

    def cutstream_policy(h, _req):
        if h < hops:
            if h % 4 == 1 and h not in cut_once:
                cut_once.add(h)
                return ("cut", [(f"c{h}", "read_file", {"path": f"src/m{h}.rs"})])
            return ("calls", [(f"c{h}", "read_file", {"path": f"src/m{h}.rs"})])
        return ("text", "Done despite dropped streams.")

    S["cutstream"] = dict(policy=cutstream_policy, seed=seed_basic, expect=("completed", None), hop_reuse=True)

    # 12. badmodel: malformed model output — invalid JSON args, unknown tool,
    #     empty reply, finish_reason=length, then a real answer.
    empty_once: set = set()

    def badmodel_policy(h, _req):
        if h == 2 and h in empty_once:
            # the empty reply happens once; the retry gets a real call
            return ("calls", [("ok2", "read_file", {"path": "src/m1.rs"})])
        if h == 2:
            empty_once.add(h)
        seq = [
            ("raw", sse_calls([("bad-json", "read_file", {})]).replace(b'"{}"', b'"{\\"path\\": "')),
            ("calls", [("no-such", "frobnicate", {"x": 1})]),
            ("text", ""),
            ("length", "I will now read the fi"),
            ("calls", [("ok", "read_file", {"path": "src/m0.rs"})]),
        ]
        if h < len(seq):
            return seq[h]
        return ("text", "Done after malformed turns.")

    S["badmodel"] = dict(policy=badmodel_policy, seed=seed_basic, expect=("completed", None), hop_reuse=True)

    # 13. deadline: 6 s turn deadline, no explicit tool timeout, tool sleeps 60 s.
    #     b3777c1d derives the tool ceiling from the deadline -> kill near 6 s.
    def deadline_policy(h, _req):
        if h == 0:
            return ("calls", [("d0", "shell", {"command": "sleep 60; echo survived"})])
        return ("text", "Done: reported the deadline kill.")

    S["deadline"] = dict(policy=deadline_policy, seed=seed_basic, expect=(None, None), yolo=False,
                         deadline_secs=6, env={"ANGEL_TOOL_IDLE_FLOOR_SECS": "0"})

    # Exercise the production post-write seam (unit run_turn disables The Cut).
    def postwrite_policy(h, _req):
        if h == 0:
            return ("calls", [("edit", "write_file", {"path": "src/cancel.rs", "content": "pub const CHECK: u32 = 1;\n"})])
        return ("text", "Verifier was interrupted; correctness remains unverified.")

    def postwrite_checks(ws, envelope):
        checks = {"edit_applied": (ws / "src/cancel.rs").is_file(),
                  "turn_deadline": envelope.get("deadline_reached") is True}
        for filename in ("verifier.pid", "child.pid"):
            path = ws / filename
            pid = path.read_text().strip() if path.is_file() else ""
            stat = Path(f"/proc/{pid}/stat")
            checks[filename + "_reaped"] = bool(pid) and (
                not stat.exists() or stat.read_text().rsplit(") ", 1)[-1].startswith("Z"))
        rows = [json.loads(line) for path in (ws / ".cut").glob("*.jsonl")
                for line in path.read_text().splitlines()]
        checks["cancelled_not_failed"] = any(
            (row.get("machine") or {}).get("skipped") == "cancelled" for row in rows)
        return checks

    S["postwrite-deadline"] = dict(policy=postwrite_policy, seed=seed_basic,
        expect=(None, "deadline"), deadline_secs=6, yolo=True, validate=postwrite_checks,
        env={"ANGEL_CUT": "1", "ANGEL_CUT_DIR": ".cut",
             "ANGEL_CUT_VERIFY": "sleep 30 & echo $! > child.pid; echo $$ > verifier.pid; wait",
             "ANGEL_VERIFY_BEFORE_DONE": "0", "ANGEL_TOOL_IDLE_FLOOR_SECS": "0"})

    # 14. compact: small context budget + 3 KB tool outputs each hop -> forces
    #     compaction mid-run; tool-less summary requests are answered by the server.
    def compact_policy(h, _req):
        if h < hops:
            return ("calls", [(f"p{h}", "shell", {"command": f"echo hop {h}; yes 'ctx filler line {h}' | head -c 3000"})])
        return ("text", "Done: survived compaction.")

    S["compact"] = dict(policy=compact_policy, seed=seed_basic, expect=("completed", None),
                        env={"ANGEL_CONTEXT_BUDGET_TOKENS": "6000", "ANGEL_COMPACT_LOCAL": "0"})

    # 15. slowmodel: 150 ms model latency per hop -> shows whether the harness
    #     overlaps anything with model wait (it should not need to) and gives the
    #     concurrent runner realistic overlap.
    def slowmodel_policy(h, _req):
        if h < hops:
            return ("delay", (0.15, "calls", [(f"w{h}", "read_file", {"path": f"src/m{h}.rs"})]))
        return ("text", "Done slowly.")

    S["slowmodel"] = dict(policy=slowmodel_policy, seed=seed_basic, expect=("completed", None))
    return S


# --------------------------------------------------------------------------
# Scripted server with timing
# --------------------------------------------------------------------------
class PolicyServer:
    def __init__(self, policy, hop_reuse=False, completion=None, usage_constant=False):
        self.usage_constant = usage_constant
        self.completion = completion
        self.policy = policy
        self.hop_reuse = hop_reuse
        self.log: list[dict] = []
        self.hops = 0
        self.lock = threading.Lock()
        owner = self

        class Handler(BaseHTTPRequestHandler):
            protocol_version = "HTTP/1.1"

            def setup(self):
                super().setup()
                # Headers and body go out as two small segments; without
                # NODELAY Nagle + the client's delayed ACK add a flat ~40 ms
                # per response that would be misread as harness latency.
                self.connection.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)

            def do_POST(self):  # noqa: N802
                t_recv = time.perf_counter()
                length = int(self.headers.get("Content-Length", "0"))
                raw = self.rfile.read(length)
                try:
                    req = json.loads(raw)
                except Exception:
                    req = {}
                if owner.completion is not None:
                    # Optional complete JSON/SSE wire fixture for formation seats.
                    content_type, body = owner.completion(req)
                    body, usage = with_scripted_usage(content_type, body, len(raw), owner.usage_constant)
                    with owner.lock:
                        owner.log.append({"stream": bool(req.get("stream")),
                                          "model": req.get("model"), "usage": usage,
                                          "req_bytes": len(raw), "response_bytes": len(body)})
                    self.send_response(200)
                    self.send_header("Content-Type", content_type)
                    self.send_header("Content-Length", str(len(body)))
                    self.end_headers()
                    self.wfile.write(body)
                    self.wfile.flush()
                    return
                msgs = req.get("messages") or []
                tools_offered = len(req.get("tools") or [])
                with owner.lock:
                    index = len(owner.log)
                    if tools_offered == 0:
                        # A tool-less request is the harness asking for a compaction
                        # summary (or a side channel); answer with a marker so the
                        # policy hop count stays honest.
                        kind, value = ("text", "SUMMARY: earlier hops read modules and applied edits; nothing pending.")
                        hop = -1
                    elif owner.hop_reuse:
                        hop = sum(1 for m in msgs if m.get("role") == "assistant")
                        kind, value = owner.policy(hop, req)
                    else:
                        hop = owner.hops
                        owner.hops += 1
                        kind, value = owner.policy(hop, req)
                    tool_sizes = [len(m.get("content") or "") for m in msgs if m.get("role") == "tool"]
                    import hashlib
                    digests = [hashlib.blake2b(json.dumps(m, sort_keys=True).encode(), digest_size=8).hexdigest() for m in msgs]
                    head_digest = hashlib.blake2b(json.dumps(req.get("tools") or [], sort_keys=True).encode(), digest_size=8).hexdigest()
                    entry = {
                        "index": index, "hop": hop, "t_recv": t_recv, "req_bytes": len(raw),
                        "model": req.get("model"), "stream": bool(req.get("stream")),
                        "tool_bytes": sum(tool_sizes), "max_tool_bytes": max(tool_sizes, default=0),
                        "digests": digests, "tools_digest": head_digest,
                        "messages": len(msgs), "tool_msgs": sum(1 for m in msgs if m.get("role") == "tool"),
                        "tools_offered": tools_offered, "kind": kind,
                        "n_calls": len(value) if kind in ("calls", "cut", "calls_then_die") else 0,
                    }
                    # C02 prompt-cache instrumentation: persist the exact request
                    # body so offline cohort runs can diff byte-level prefix
                    # stability between consecutive hops. Opt-in via env var so
                    # ordinary stress receipts do not balloon.
                    if os.environ.get("ANGEL_STRESS_KEEP_BODIES") == "1":
                        entry["body"] = raw.decode("utf-8", "replace")
                    owner.log.append(entry)
                delay = 0.0
                if kind == "delay":
                    delay, kind, value = value
                    entry["kind"] = kind
                if delay:
                    time.sleep(delay)
                if kind == "http":
                    extra_headers = {}
                    if isinstance(value, dict):
                        extra_headers = dict(value.get("headers") or {})
                        code = int(value.get("code") or value.get("status") or 500)
                        if "body" in value:
                            raw_body = value["body"]
                            body = raw_body if isinstance(raw_body, bytes) else (
                                raw_body.encode() if isinstance(raw_body, str)
                                else json.dumps(raw_body).encode()
                            )
                        else:
                            body = json.dumps({
                                "error": {"message": f"injected {code}", "type": "stress"}
                            }).encode()
                    else:
                        code = int(value)
                        body = json.dumps({
                            "error": {"message": f"injected {code}", "type": "stress"}
                        }).encode()
                    self.send_response(code)
                    self.send_header("Content-Type", extra_headers.pop(
                        "Content-Type", "application/json"))
                    for hk, hv in extra_headers.items():
                        self.send_header(str(hk), str(hv))
                    self.send_header("Content-Length", str(len(body)))
                    self.end_headers()
                    self.wfile.write(body)
                    self.wfile.flush()
                    entry["t_sent"] = time.perf_counter()
                    return
                if kind == "reset":
                    partial = b'data: {"choices":[{"delta":{"content":"partial"}}]}\n\n'
                    self.send_response(200)
                    self.send_header("Content-Type", "text/event-stream")
                    self.send_header("Connection", "close")
                    self.end_headers()
                    try:
                        self.wfile.write(partial)
                        self.wfile.flush()
                    except BrokenPipeError:
                        pass
                    linger = __import__("struct").pack("ii", 1, 0)
                    try:
                        self.connection.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, linger)
                    except OSError:
                        pass
                    self.close_connection = True
                    try:
                        self.connection.close()
                    except OSError:
                        pass
                    entry["t_sent"] = time.perf_counter()
                    return
                if kind == "stall":
                    stall_s = float(value) if not isinstance(value, (list, tuple)) else float(value[0])
                    self.send_response(200)
                    self.send_header("Content-Type", "text/event-stream")
                    self.send_header("Connection", "keep-alive")
                    self.end_headers()
                    time.sleep(stall_s)
                    try:
                        body, entry["usage"] = with_scripted_usage(
                            "text/event-stream", sse_text("stalled-then-late"), len(raw), owner.usage_constant)
                        self.wfile.write(body)
                        self.wfile.flush()
                    except (BrokenPipeError, ConnectionResetError, TimeoutError, OSError):
                        pass
                    entry["t_sent"] = time.perf_counter()
                    return
                if kind == "cut":
                    # Deltas without finish/[DONE], then close: a dropped provider stream.
                    body = sse_calls(value).split(b"\n\ndata: {\"choices\": [{\"delta\": {}, \"finish_reason\"")[0] + b"\n\n"
                    self.close_connection = True
                    self.send_response(200)
                    self.send_header("Content-Type", "text/event-stream")
                    self.send_header("Connection", "close")
                    self.end_headers()
                    self.wfile.write(body)
                    self.wfile.flush()
                    entry["t_sent"] = time.perf_counter()
                    return
                if kind == "length":
                    payload = {"choices": [{"delta": {"content": value}}]}
                    finish = {"choices": [{"delta": {}, "finish_reason": "length"}]}
                    body = (f"data: {json.dumps(payload)}\n\ndata: {json.dumps(finish)}\n\ndata: [DONE]\n\n").encode()
                elif kind == "raw":
                    body = value if isinstance(value, bytes) else value.encode()
                elif kind in ("calls", "calls_then_die"):
                    body = sse_calls(value)
                else:
                    body = sse_text(value)
                content_type = "text/event-stream"
                if kind != "raw":
                    # Omitted stream retains the historical SSE fixture default.
                    if req.get("stream") is False:
                        frames = [json.loads(line[6:]) for line in body.decode().splitlines()
                                  if line.startswith("data: ") and line != "data: [DONE]"]
                        message = dict(role="assistant", **frames[0]["choices"][0]["delta"])
                        for call in message.get("tool_calls", []):
                            call.pop("index", None)
                        body = json.dumps(dict(choices=[dict(message=message,
                            finish_reason=frames[-1]["choices"][0]["finish_reason"])])).encode()
                        content_type = "application/json"
                    body, entry["usage"] = with_scripted_usage(content_type, body, len(raw), owner.usage_constant)
                if kind == "calls_then_die":
                    # Close admission before delivering hop zero. Also close
                    # this HTTP/1.1 connection after its complete response:
                    # shutdown() alone leaves persistent handlers alive.
                    self.server.socket.close()
                    self.close_connection = True
                    entry["listener_closed_before_reply"] = True
                self.send_response(200)
                self.send_header("Content-Type", content_type)
                self.send_header("Content-Length", str(len(body)))
                if kind == "calls_then_die":
                    self.send_header("Connection", "close")
                self.end_headers()
                self.wfile.write(body)
                self.wfile.flush()
                entry["t_sent"] = time.perf_counter()

            def log_message(self, _f, *_a):
                return

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    def __enter__(self):
        self.thread.start()
        host, port = self.server.server_address
        self.url = f"http://{host}:{port}/v1/chat/completions"
        return self

    def __exit__(self, *_):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)


def poll_rss(pid: int, out: dict, stop: threading.Event) -> None:
    peak = 0
    while not stop.is_set():
        try:
            with open(f"/proc/{pid}/status") as fh:
                for line in fh:
                    if line.startswith("VmHWM:"):
                        peak = max(peak, int(line.split()[1]))
                        break
        except (FileNotFoundError, ProcessLookupError):
            break
        stop.wait(0.05)
    out["peak_rss_kb"] = peak


# --------------------------------------------------------------------------
# Runner
# --------------------------------------------------------------------------
def run_scenario(name: str, spec: dict, hops: int, angel_bin: str, out_dir: Path, keep: bool, home: Path | None = None, tag: str = "", template: str | None = None, usage_constant: bool = False) -> dict:
    ws = Path(tempfile.mkdtemp(prefix=f"angel-stress-{name}-"))
    if template:
        # Real-sized tree: a detached worktree of the template repo's HEAD.
        ws.rmdir()
        subprocess.run(["git", "-C", template, "worktree", "add", "-q", "--detach", str(ws), "HEAD"], check=True, capture_output=True)
        spec["seed"](ws, hops)
    else:
        spec["seed"](ws, hops)
        subprocess.run(["git", "-C", str(ws), "init", "-q"], check=False, capture_output=True)
    env = {
        **os.environ,
        "PATH": f"{Path.home()}/.cargo/bin:{Path.home()}/.local/bin:" + os.environ.get("PATH", ""),
        "HOME": str(home or (ws / ".home")),  # isolate ~/.angel state unless a shared home is given
        "ANGEL_DRIVER": "openrouter",
        "ANGEL_OPENROUTER_KEY": "offline-stress-key",
        "OPENROUTER_API_KEY": "offline-stress-key",
        "ANGEL_OPENROUTER_MODEL": "offline-stress-model",
        "ANGEL_API_CLUBS": "openrouter",
        "ANGEL_FIRST_WRITE_CALLS": "0",
        "ANGEL_PROJECT_DOC": "0",
        "ANGEL_TASK_RECON": "0",
        "ANGEL_SKILL_HINT": "0",
        "ANGEL_ADVISOR": "0",
        "ANGEL_TASK_STRICT_EXIT": "0",
        "CODEX_HOME": "/nonexistent-codex",
        **spec.get("env", {}),
    }
    yolo = spec.get("yolo", True)
    if yolo:
        env["ANGEL_YOLO"] = "1"
    if home is None:
        (ws / ".home").mkdir()
    receipt: dict = {"scenario": name + tag, "hops_requested": hops, "workspace": str(ws), "shared_home": home is not None}
    with PolicyServer(spec["policy"], spec.get("hop_reuse", False), usage_constant=usage_constant) as srv:
        env["ANGEL_OPENROUTER_URL"] = srv.url
        cmd = [
            angel_bin, *(["--yolo"] if yolo else []), "--task-json",
            "--workspace", str(ws),
            "--max-hops", str(hops * 3 + 8),
            "--deadline-secs", str(spec.get("deadline_secs", 1200)),
            "--tool-profile", "essential",
            "--rollout", "off",
            "--task-id", f"stress-{name}",
            "--run-id", f"stress-{name}-{int(time.time())}",
            f"Stress scenario {name}: follow the tool script exactly.",
        ]
        t0 = time.perf_counter()
        proc = subprocess.Popen(cmd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, cwd=str(ws))
        rss: dict = {}
        stop = threading.Event()
        th = threading.Thread(target=poll_rss, args=(proc.pid, rss, stop), daemon=True)
        th.start()
        try:
            stdout, stderr = proc.communicate(timeout=1500)
        except subprocess.TimeoutExpired:
            proc.kill()
            stdout, stderr = proc.communicate()
            receipt["killed"] = True
        wall = time.perf_counter() - t0
        stop.set()
        th.join(timeout=2)
        log = list(srv.log)

    envelope: dict = {}
    try:
        envelope = json.loads(stdout) if stdout.strip() else {}
    except json.JSONDecodeError:
        envelope = {"stdout_tail": stdout[-600:]}

    # Per-hop harness gap: response i sent -> request i+1 received.
    gaps = [log[i + 1]["t_recv"] - log[i]["t_sent"] for i in range(len(log) - 1) if "t_sent" in log[i]]
    gaps_ms = sorted(g * 1000 for g in gaps)

    def pct(p):
        if not gaps_ms:
            return None
        return round(gaps_ms[min(len(gaps_ms) - 1, int(p * len(gaps_ms)))], 1)

    # Prompt-cache prefix stability: for consecutive requests, how many leading
    # messages are byte-identical? A break before the previous tail means a
    # provider prefix cache would miss from that point.
    prefix_breaks = 0
    broken_at = []
    tools_changes = 0
    for i in range(1, len(log)):
        a, b = log[i - 1]["digests"], log[i]["digests"]
        common = 0
        for x, y in zip(a, b):
            if x != y:
                break
            common += 1
        if common < len(a):
            prefix_breaks += 1
            broken_at.append((log[i]["hop"], common, len(a)))
        if log[i]["tools_digest"] != log[i - 1]["tools_digest"]:
            tools_changes += 1
    timing = envelope.get("timing") or {}
    receipt.update(
        {
            "exit_code": proc.returncode,
            "status": envelope.get("status"),
            "stop_reason": envelope.get("stop_reason"),
            "hops": envelope.get("hops"),
            "wall_s": round(wall, 2),
            "startup_to_first_request_ms": round((log[0]["t_recv"] - t0) * 1000, 1) if log else None,
            "model_requests": len(log),
            "harness_gap_ms": {
                "p50": pct(0.5), "p90": pct(0.9), "max": pct(1.0),
                "sum": round(sum(gaps_ms), 1),
                "first": round(gaps_ms[0], 1) if gaps_ms else None,
            },
            "req_bytes": {
                "first": log[0]["req_bytes"] if log else None,
                "last": log[-1]["req_bytes"] if log else None,
                "max": max((e["req_bytes"] for e in log), default=None),
                "sum_mb": round(sum(e["req_bytes"] for e in log) / 1e6, 2),
            },
            "messages_last": log[-1]["messages"] if log else None,
            "cache_prefix": {"breaks": prefix_breaks, "tools_changes": tools_changes, "first_breaks": broken_at[:5]},
            "tool_bytes_last": log[-1]["tool_bytes"] if log else None,
            "max_tool_bytes": max((e["max_tool_bytes"] for e in log), default=None),
            "tools_offered": log[0]["tools_offered"] if log else None,
            "peak_rss_mb": round(rss.get("peak_rss_kb", 0) / 1024, 1),
            "timing": {
                k: timing.get(k)
                for k in ("model_ms", "model_calls", "tool_ms", "tool_calls", "tool_errors", "tool_max_ms", "tool_max_name", "other_ms")
            },
            "stderr_tail": stderr[-1500:],
        }
    )
    exp_status, exp_reason = spec["expect"]
    receipt["ok"] = (exp_status is None or receipt["status"] == exp_status) and (exp_reason is None or receipt["stop_reason"] == exp_reason)
    if validate := spec.get("validate"):
        receipt["checks"] = validate(ws, envelope)
        receipt["ok"] = receipt["ok"] and all(receipt["checks"].values())
    receipt["summary_requests"] = sum(1 for e in log if e["hop"] == -1)
    receipt["injected_failures"] = sum(1 for e in log if e["kind"] in ("http", "cut", "length"))
    receipt["hop_log"] = [{k: v for k, v in e.items() if k != "digests"} for e in log]
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / f"{name}{tag}.json").write_text(json.dumps(receipt, indent=1, default=str))
    if not keep:
        if template:
            subprocess.run(["git", "-C", template, "worktree", "remove", "--force", str(ws)], check=False, capture_output=True)
        shutil.rmtree(ws, ignore_errors=True)
    return receipt


# Regression floors for --gate (ms, p50 harness gap unless noted). Set from the
# reference baseline with headroom; a breach is a harness performance signal.
# Compare matched binaries and host load before
# attributing a breach to a code change rather than the environment.
GATE_FLOORS = {
    "hops": ("harness_gap_p50_ms", 5.0),
    "fanout": ("harness_gap_p50_ms", 10.0),
    "editstorm": ("harness_gap_p50_ms", 10.0),
    "shellcost": ("harness_gap_p50_ms", 12.0),   # 32 ms before angel-sandbox, 2.5 ms after
    "slowmodel": ("harness_gap_p50_ms", 5.0),
    "timeout": ("wall_s", 6.0),                 # 3 s tool timeout + startup, never the 30 s floor
    "deadline": ("wall_s", 9.0),                # 6 s deadline-derived kill
    "postwrite-deadline": ("wall_s", 9.0),      # same deadline through post-write verification
    "startup_any": ("startup_to_first_request_ms", 400.0),
}


def gate(results: list) -> list:
    """Return human-readable breaches; empty means the gate passed."""
    breaches = []
    for r in results:
        if not r["ok"]:
            breaches.append(f"{r['scenario']}: diverged ({r['status']}/{r['stop_reason']})")
        name = r["scenario"].split("-c")[0]
        checks = [GATE_FLOORS.get(name), GATE_FLOORS["startup_any"]]
        for check in checks:
            if not check:
                continue
            metric, floor = check
            if metric == "harness_gap_p50_ms":
                value = r["harness_gap_ms"]["p50"]
            elif metric == "wall_s":
                value = r["wall_s"]
            else:
                value = r.get(metric)
            if value is not None and value > floor:
                breaches.append(f"{r['scenario']}: {metric}={value} > floor {floor}")
    return breaches


def summarize(r: dict) -> str:
    g = r["harness_gap_ms"]
    t = r["timing"]
    return (
        f"{r['scenario']:<10} ok={str(r['ok']):<5} status={r['status']}/{r['stop_reason']} "
        f"hops={r['hops']} wall={r['wall_s']}s startup={r['startup_to_first_request_ms']}ms "
        f"gap p50/p90/max={g['p50']}/{g['p90']}/{g['max']}ms sum={g['sum']}ms "
        f"req last={r['req_bytes']['last']}B (tool {r['tool_bytes_last']}B, maxmsg {r['max_tool_bytes']}B) sum={r['req_bytes']['sum_mb']}MB "
        f"rss={r['peak_rss_mb']}MB tool_ms={t['tool_ms']} other_ms={t['other_ms']} errs={t['tool_errors']} "
        f"inj={r.get('injected_failures')} summaries={r.get('summary_requests')} "
        f"prefix_breaks={r['cache_prefix']['breaks']} tools_changes={r['cache_prefix']['tools_changes']}"
    )


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--angel-bin", default=str(REPO / "cockpit/target/release/angel"))
    ap.add_argument("--usage-constant", action="store_true", help="report legacy synthetic 500 prompt / 20 completion tokens")
    ap.add_argument("--scenario", default="all")
    ap.add_argument("--hops", type=int, default=40)
    ap.add_argument("--out", default=str(REPO / ".angel0/harness-stress"))
    ap.add_argument("--keep", action="store_true", help="keep scenario workspaces")
    ap.add_argument("--json", action="store_true", help="print the full receipts as JSON")
    ap.add_argument("--env", action="append", default=[], help="KEY=VAL applied to every scenario (repeatable)")
    ap.add_argument("--workspace-template", default=None, help="git repo whose HEAD is checked out (detached worktree) as each scenario workspace, to measure startup/turn snapshots on a real-sized tree")
    ap.add_argument("--gate", action="store_true", help="regression gate: exit 1 if any scenario diverges or a speed floor is exceeded (see GATE_FLOORS)")
    ap.add_argument("--concurrent", type=int, default=1, help="run N copies of each scenario at once sharing ONE $HOME (ledger/lock contention)")
    args = ap.parse_args(argv)

    angel_bin = str(Path(args.angel_bin).expanduser())
    extra_env = dict(kv.split("=", 1) for kv in args.env)
    build = subprocess.run([angel_bin, "--build-info", "--json"], capture_output=True, text=True, check=False)
    git_sha = subprocess.run(["git", "-C", str(REPO), "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip()
    stamp = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime())
    out_dir = Path(args.out) / stamp
    config = {
        "schema": "angel-harness-stress/v1",
        "stamp": stamp,
        "git_head": git_sha,
        "angel_bin": angel_bin,
        "build_info": (build.stdout.strip()[:400] if build.returncode == 0 else build.stderr[-200:]),
        "host": platform.node(),
        "hops": args.hops,
        "usage_constant": args.usage_constant,
        "scenarios": args.scenario,
        "extra_env": extra_env,
    }
    print("CONFIG " + json.dumps(config), file=sys.stderr)

    scenarios = make_scenarios(args.hops)
    for spec in scenarios.values():
        spec["env"] = {**spec.get("env", {}), **extra_env}
    names = list(scenarios) if args.scenario == "all" else [s.strip() for s in args.scenario.split(",")]
    results = []
    for name in names:
        if name not in scenarios:
            print(f"unknown scenario {name}", file=sys.stderr)
            return 2
        if args.concurrent > 1:
            home = Path(tempfile.mkdtemp(prefix="angel-stress-home-"))
            batch: list = [None] * args.concurrent
            def one(i):
                # Each policy owns state, but configuration must match the
                # serial lane and the CONFIG receipt (including --env).
                spec = make_scenarios(args.hops)[name]
                spec["env"] = {**spec.get("env", {}), **extra_env}
                batch[i] = run_scenario(name, spec, args.hops, angel_bin, out_dir, args.keep, home=home, tag=f"-c{i}", template=args.workspace_template, usage_constant=args.usage_constant)

            t0 = time.perf_counter()
            threads = [threading.Thread(target=one, args=(i,)) for i in range(args.concurrent)]
            for t in threads:
                t.start()
            for t in threads:
                t.join()
            batch_wall = time.perf_counter() - t0
            for r in batch:
                results.append(r)
                print(summarize(r), file=sys.stderr)
                if not r["ok"]:
                    print("  stderr: " + r["stderr_tail"][-600:].replace("\n", "\n          "), file=sys.stderr)
            print(f"{name:<10} CONCURRENT x{args.concurrent}: batch wall={batch_wall:.2f}s  max single={max(r['wall_s'] for r in batch)}s  "
                  f"mean gap p50={sum((r['harness_gap_ms']['p50'] or 0) for r in batch)/len(batch):.1f}ms  all ok={all(r['ok'] for r in batch)}", file=sys.stderr)
            if not args.keep:
                shutil.rmtree(home, ignore_errors=True)
            continue
        r = run_scenario(name, scenarios[name], args.hops, angel_bin, out_dir, args.keep, template=args.workspace_template, usage_constant=args.usage_constant)
        results.append(r)
        print(summarize(r), file=sys.stderr)
        if not r["ok"]:
            print("  stderr: " + r["stderr_tail"][-600:].replace("\n", "\n          "), file=sys.stderr)
    (out_dir / "summary.json").write_text(json.dumps(attach_receipt_header({"config": config, "results": [{k: v for k, v in r.items() if k != "hop_log"} for r in results]}), indent=1, default=str))
    print(f"receipts: {out_dir}", file=sys.stderr)
    if args.json:
        print(json.dumps({"config": config, "results": [{k: v for k, v in r.items() if k not in ("hop_log", "stderr_tail")} for r in results]}, indent=1, default=str))
    if args.gate:
        breaches = gate(results)
        if breaches:
            print("GATE FAILED:\n  " + "\n  ".join(breaches), file=sys.stderr)
            return 1
        print(f"GATE PASSED: {len(results)} scenario(s) within floors", file=sys.stderr)
        return 0
    return 0 if all(r["ok"] for r in results) else 1


if __name__ == "__main__":
    sys.exit(main())
