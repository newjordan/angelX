#!/usr/bin/env python3
"""A scripted OpenAI-compatible model for wire benchmarks.

Every request body is appended to LOG (one JSON object per line) with the
wall time it arrived. The reply follows a fixed coding script, so two harness
builds see the same model behaviour and only the harness's own bytes differ.

MODE=plain   never decodes braille.
MODE=reader  before each scripted step, reads every braille run it has not
             read yet with read_file ledger://<cells> (a diligent model that
             decodes each route once).
"""
import json
import os
import re
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

LOG = os.environ["STUB_LOG"]
MODE = os.environ.get("STUB_MODE", "plain")
PORT = int(os.environ.get("STUB_PORT", "18080"))

SCRIPT = [
    ("list_dir", {"path": "."}),
    ("read_file", {"path": "calc.py"}),
    ("run_tests", {"runtime": "python", "entrypoint": "unittest"}),
    ("read_file", {"path": "test_calc.py"}),
    ("str_replace", {"path": "calc.py", "old": "return a - b", "new": "return a + b"}),
    ("run_tests", {"runtime": "python", "entrypoint": "unittest"}),
    None,  # final answer
]
FINAL = "Fixed `add` in calc.py (it subtracted); the unittest suite now passes."

# STUB_CYCLES=N: a long session. Each cycle reads, runs the red suite, and
# swaps one wrong operator for another (the workspace changes, the tests stay
# red); then the real fix, a green run, and the answer. 3N+3 tool turns.
CYCLES = int(os.environ.get("STUB_CYCLES", "0"))
if CYCLES:
    SCRIPT = []
    wrong = ["-", "*"]
    for i in range(CYCLES):
        old, new = wrong[i % 2], wrong[(i + 1) % 2]
        SCRIPT += [
            ("read_file", {"path": "calc.py"}),
            ("run_tests", {"runtime": "python", "entrypoint": "unittest"}),
            ("str_replace", {"path": "calc.py", "old": f"return a {old} b", "new": f"return a {new} b"}),
        ]
    last = wrong[CYCLES % 2]
    SCRIPT += [
        ("str_replace", {"path": "calc.py", "old": f"return a {last} b", "new": "return a + b"}),
        ("run_tests", {"runtime": "python", "entrypoint": "unittest"}),
        None,
    ]
# STUB_SCRIPT=<file.json>: a scenario's own steps, a list of [tool, args] with
# null for the answer (decision-point captures, decide_capture.sh); or a list
# of such lists, one per user turn (decide_turns.sh).
TURNS = None
PROMPTS = [line.strip() for line in open(os.environ["STUB_PROMPTS"])] if os.environ.get("STUB_PROMPTS") else []
if os.environ.get("STUB_SCRIPT"):
    loaded = json.load(open(os.environ["STUB_SCRIPT"]))
    steps = lambda script: [tuple(step) if step else None for step in script]
    if loaded and isinstance(loaded[0], list) and loaded[0] and not isinstance(loaded[0][0], str):
        TURNS = [steps(turn) for turn in loaded]
    else:
        SCRIPT = steps(loaded)
BRAILLE = re.compile(r"[⠀-⣿]+")

state = {"read": set(), "calls": 0}


def text_of(message):
    content = message.get("content")
    if isinstance(content, list):
        return "".join(part.get("text", "") for part in content if isinstance(part, dict))
    return content or ""


def scripted_steps_done(messages):
    """How many scripted (non-ledger) tool calls the conversation already holds."""
    done = 0
    for message in messages:
        for call in message.get("tool_calls") or []:
            args = call.get("function", {}).get("arguments", "")
            if "ledger://" not in args:
                done += 1
    return done


def unread_routes(messages):
    """Braille the harness sent (prompt, notes, tool-result tails), not the
    addresses inside a ledger read's own decoded reply."""
    ledger_ids = set()
    for message in messages:
        for call in message.get("tool_calls") or []:
            if "ledger://" in call.get("function", {}).get("arguments", ""):
                ledger_ids.add(call.get("id"))
    runs = []
    for message in messages:
        if message.get("role") == "assistant":
            continue
        if message.get("role") == "tool" and message.get("tool_call_id") in ledger_ids:
            continue
        for run in BRAILLE.findall(text_of(message)):
            if run not in state["read"] and run not in runs:
                runs.append(run)
    return runs


def next_action(body):
    messages = body.get("messages", [])
    if MODE == "reader":
        pending = unread_routes(messages)
        if pending:
            run = pending[0]
            state["read"].add(run)
            return ("read_file", {"path": f"ledger://{run}"})
    script = SCRIPT
    if TURNS is not None:
        # The operator's prompts (STUB_PROMPTS, one per line; the harness adds
        # its own user-role context beside them) mark the turns; each turn
        # counts its own steps.
        prompts = [i for i, m in enumerate(messages)
                   if m.get("role") == "user" and text_of(m).strip() in PROMPTS]
        script = TURNS[min(len(prompts), len(TURNS)) - 1] if prompts else TURNS[0]
        messages = messages[prompts[-1]:] if prompts else messages
    step = scripted_steps_done(messages)
    if step >= len(script) - 1:
        return None
    return script[step]


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *args):
        pass

    def _send_json(self, code, payload):
        data = json.dumps(payload).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        with open(LOG + ".paths", "a") as f:
            f.write(f"GET {self.path}\n")
        if self.path.rstrip("/").endswith("/models"):
            self._send_json(200, {"object": "list", "data": [{"id": "stub", "object": "model"}]})
        else:
            self._send_json(404, {"error": "not found"})

    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length)
        with open(LOG + ".paths", "a") as f:
            f.write(f"POST {self.path}\n")
        if not self.path.rstrip("/").endswith("/chat/completions"):
            self._send_json(404, {"error": "not found"})
            return
        body = json.loads(raw)
        arrived = time.time()
        action = next_action(body)
        state["calls"] += 1
        with open(LOG, "a") as f:
            f.write(json.dumps({"t": arrived, "bytes": len(raw), "body": body, "action": action}) + "\n")
        usage = {"prompt_tokens": len(raw) // 4, "completion_tokens": 20, "total_tokens": len(raw) // 4 + 20}
        if action is None:
            message = {"role": "assistant", "content": FINAL}
            finish = "stop"
        else:
            name, args = action
            message = {
                "role": "assistant",
                "content": None,
                "tool_calls": [{
                    "id": f"call_{state['calls']}",
                    "type": "function",
                    "function": {"name": name, "arguments": json.dumps(args)},
                }],
            }
            finish = "tool_calls"
        if not body.get("stream"):
            self._send_json(200, {
                "id": f"stub-{state['calls']}", "object": "chat.completion", "model": "stub",
                "choices": [{"index": 0, "message": message, "finish_reason": finish}],
                "usage": usage,
            })
            return
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "close")
        self.end_headers()
        delta = {"role": "assistant"}
        if action is None:
            delta["content"] = FINAL
        else:
            call = message["tool_calls"][0]
            delta["tool_calls"] = [{"index": 0, **call}]
        chunks = [
            {"id": "stub", "object": "chat.completion.chunk", "model": "stub",
             "choices": [{"index": 0, "delta": delta, "finish_reason": None}]},
            {"id": "stub", "object": "chat.completion.chunk", "model": "stub",
             "choices": [{"index": 0, "delta": {}, "finish_reason": finish}], "usage": usage},
        ]
        for chunk in chunks:
            self.wfile.write(f"data: {json.dumps(chunk)}\n\n".encode())
        self.wfile.write(b"data: [DONE]\n\n")
        self.wfile.flush()
        self.close_connection = True


if __name__ == "__main__":
    server = ThreadingHTTPServer(("127.0.0.1", PORT), Handler)
    port = server.server_address[1]
    if ready := os.environ.get("STUB_READY"):
        temporary = ready + ".tmp"
        with open(temporary, "w") as output:
            output.write(f"{port}\n")
        os.replace(temporary, ready)
    print(f"stub model on :{port} mode={MODE}", file=sys.stderr, flush=True)
    server.serve_forever()
