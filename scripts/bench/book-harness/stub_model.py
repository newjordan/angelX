#!/usr/bin/env python3
import json
import os
import re
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

LOG = os.environ["STUB_LOG"]
MODE = os.environ.get("STUB_MODE", "plain")
PORT = int(os.environ.get("STUB_PORT", "18080"))
KEEPALIVE = os.environ.get("STUB_KEEPALIVE") == "1"
NODELAY = os.environ.get("STUB_NODELAY", "1") != "0"
IDLE_CLOSE = float(os.environ["STUB_IDLE_CLOSE_SECS"]) if os.environ.get("STUB_IDLE_CLOSE_SECS") else None
IDLE_RST = os.environ.get("STUB_IDLE_CLOSE_MODE", "fin") == "rst"
ERROR_FROM = int(os.environ["STUB_ERROR_FROM"]) if os.environ.get("STUB_ERROR_FROM") else None
ERROR_STATUS = int(os.environ.get("STUB_ERROR_STATUS", "503"))
ERROR_SHOULD_RETRY = os.environ.get("STUB_ERROR_SHOULD_RETRY")
ROUTING_HEADERS = ("x-grok-conv-id", "session_id", "x-session-id")

SCRIPT = [
    ("list_dir", {"path": "."}),
    ("read_file", {"path": "calc.py"}),
    ("run_tests", {"runtime": "python", "entrypoint": "unittest"}),
    ("read_file", {"path": "test_calc.py"}),
    ("str_replace", {"path": "calc.py", "old": "return a - b", "new": "return a + b"}),
    ("run_tests", {"runtime": "python", "entrypoint": "unittest"}),
    None,
]
FINAL = "Fixed `add` in calc.py (it subtracted); the unittest suite now passes."

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
    done = 0
    for message in messages:
        for call in message.get("tool_calls") or []:
            args = call.get("function", {}).get("arguments", "")
            if "ledger://" not in args:
                done += 1
    return done


def unread_routes(messages):
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


def advertised_workspace(messages):
    for message in messages:
        content = text_of(message)
        if content.startswith("[workspace-context/v1]"):
            try:
                payload, _ = json.JSONDecoder().raw_decode(content.split("\n", 1)[1])
                return payload.get("workspace")
            except (IndexError, ValueError, AttributeError):
                return None
    return None


def prefixed(action, messages):
    if action is None:
        return None
    workspace = advertised_workspace(messages)
    if not workspace:
        return action
    name, args = action
    args = dict(args)
    if "path" in args:
        args["path"] = workspace.rstrip("/") + "/" + args["path"]
    if name == "run_tests":
        args["dir"] = workspace
    return (name, args)


def next_action(body):
    action = scripted_action(body)
    if MODE == "prefix":
        return prefixed(action, body.get("messages", []))
    return action


def scripted_action(body):
    messages = body.get("messages", [])
    if MODE == "reader":
        pending = unread_routes(messages)
        if pending:
            run = pending[0]
            state["read"].add(run)
            return ("read_file", {"path": f"ledger://{run}"})
    script = SCRIPT
    if TURNS is not None:
        prompts = [i for i, m in enumerate(messages)
                   if m.get("role") == "user" and text_of(m).strip() in PROMPTS]
        script = TURNS[min(len(prompts), len(TURNS)) - 1] if prompts else TURNS[0]
        messages = messages[prompts[-1]:] if prompts else messages
    step = scripted_steps_done(messages)
    if step >= len(script) - 1:
        return None
    return script[step]


def responses_messages(body):
    out = []
    if body.get("instructions"):
        out.append({"role": "system", "content": body["instructions"]})
    for item in body.get("input") or []:
        kind = item.get("type")
        if kind == "message":
            parts = item.get("content")
            text = "".join(p.get("text", "") for p in parts if isinstance(p, dict)) if isinstance(parts, list) else (parts or "")
            out.append({"role": item.get("role", "user"), "content": text})
        elif kind == "function_call":
            out.append({"role": "assistant", "content": None, "tool_calls": [
                {"id": item.get("call_id"), "type": "function",
                 "function": {"name": item.get("name"), "arguments": item.get("arguments", "")}}]})
        elif kind == "function_call_output":
            out.append({"role": "tool", "tool_call_id": item.get("call_id"), "content": item.get("output", "")})
    return out


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def setup(self):
        if IDLE_CLOSE:
            self.timeout = IDLE_CLOSE
        super().setup()
        if NODELAY:
            import socket
            self.connection.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)

    def finish(self):
        if IDLE_CLOSE and getattr(self, "_idle_closed", False):
            with open(LOG + ".closes", "a") as f:
                f.write(f"{time.time():.6f} {self.client_address[1]} {'rst' if IDLE_RST else 'fin'}\n")
            if IDLE_RST:
                import socket, struct
                self.connection.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, struct.pack("ii", 1, 0))
                self.server.rst_fds.add(self.connection.fileno())
        super().finish()

    def handle_one_request(self):
        if IDLE_CLOSE:
            try:
                self.raw_requestline = self.rfile.readline(65537)
            except TimeoutError:
                self._idle_closed = True
                self.close_connection = True
                return
            if not self.raw_requestline:
                self.close_connection = True
                return
            if not self.parse_request():
                return
            method = getattr(self, "do_" + self.command, None)
            if method is None:
                self.send_error(501)
                return
            method()
            self.wfile.flush()
            return
        super().handle_one_request()

    def _wire(self):
        headers = {name: self.headers[name] for name in ROUTING_HEADERS if self.headers.get(name)}
        return {"conn": self.client_address[1], "headers": headers}

    def _stream_open(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        if KEEPALIVE:
            self.send_header("Transfer-Encoding", "chunked")
        else:
            self.send_header("Connection", "close")
        self.end_headers()

    def _stream_write(self, data):
        if KEEPALIVE:
            self.wfile.write(f"{len(data):x}\r\n".encode() + data + b"\r\n")
        else:
            self.wfile.write(data)

    def _stream_close(self):
        if KEEPALIVE:
            self.wfile.write(b"0\r\n\r\n")
        self.wfile.flush()
        self.close_connection = not KEEPALIVE

    def _event(self, kind, payload):
        payload = {"type": kind, **payload}
        self._stream_write(f"event: {kind}\ndata: {json.dumps(payload)}\n\n".encode())
        self.wfile.flush()

    def _responses(self, body, raw):
        arrived = time.time()
        action = next_action({"messages": responses_messages(body)})
        state["calls"] += 1
        with open(LOG, "a") as f:
            f.write(json.dumps({"t": arrived, "bytes": len(raw), "body": body, "action": action, "api": "responses", **self._wire()}) + "\n")
        rid = f"resp_{state['calls']}"
        self._stream_open()
        self._event("response.created", {"response": {"id": rid, "status": "in_progress"}})
        if action is None:
            item = {"type": "message", "id": f"msg_{state['calls']}", "role": "assistant", "status": "completed",
                    "content": [{"type": "output_text", "text": FINAL}]}
            self._event("response.output_item.added", {"output_index": 0, "item": {**item, "content": []}})
            self._event("response.output_text.delta", {"output_index": 0, "content_index": 0, "delta": FINAL})
        else:
            name, args = action
            item = {"type": "function_call", "id": f"fc_{state['calls']}", "call_id": f"call_{state['calls']}",
                    "name": name, "arguments": json.dumps(args), "status": "completed"}
            self._event("response.output_item.added", {"output_index": 0, "item": {**item, "arguments": ""}})
            self._event("response.function_call_arguments.delta", {"output_index": 0, "item_id": item["id"], "delta": item["arguments"]})
            self._event("response.function_call_arguments.done", {"output_index": 0, "item_id": item["id"], "arguments": item["arguments"]})
        self._event("response.output_item.done", {"output_index": 0, "item": item})
        self._event("response.completed", {"response": {"id": rid, "status": "completed", "output": [item],
            "usage": {"input_tokens": len(raw) // 4, "output_tokens": 20,
                      "input_tokens_details": {"cached_tokens": 0}, "output_tokens_details": {"reasoning_tokens": 0}}}})
        self._stream_close()

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
        with open(LOG + ".times", "a") as f:
            f.write(f"{time.time():.6f} GET {self.path}\n")
        if self.path.rstrip("/").endswith("/models"):
            self._send_json(200, {"object": "list", "data": [{"id": "stub", "object": "model"}]})
        else:
            self._send_json(404, {"error": "not found"})

    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length)
        with open(LOG + ".paths", "a") as f:
            f.write(f"POST {self.path}\n")
        with open(LOG + ".times", "a") as f:
            f.write(f"{time.time():.6f} POST {self.path}\n")
        if ERROR_FROM is not None and state["calls"] >= ERROR_FROM:
            state["calls"] += 1
            with open(LOG, "a") as f:
                f.write(json.dumps({"t": time.time(), "bytes": len(raw), "body": json.loads(raw),
                                    "action": "error", "status": ERROR_STATUS, **self._wire()}) + "\n")
            data = json.dumps({"error": {"message": "the stub's provider failure", "type": "stub_error"}}).encode()
            self.send_response(ERROR_STATUS)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            if ERROR_SHOULD_RETRY:
                self.send_header("x-should-retry", ERROR_SHOULD_RETRY)
            self.end_headers()
            self.wfile.write(data)
            return
        if self.path.rstrip("/").endswith("/responses"):
            self._responses(json.loads(raw), raw)
            return
        if not self.path.rstrip("/").endswith("/chat/completions"):
            self._send_json(404, {"error": "not found"})
            return
        body = json.loads(raw)
        arrived = time.time()
        action = next_action(body)
        state["calls"] += 1
        with open(LOG, "a") as f:
            f.write(json.dumps({"t": arrived, "bytes": len(raw), "body": body, "action": action, **self._wire()}) + "\n")
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
        self._stream_open()
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
            self._stream_write(f"data: {json.dumps(chunk)}\n\n".encode())
        self._stream_write(b"data: [DONE]\n\n")
        self._stream_close()


class Server(ThreadingHTTPServer):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.rst_fds = set()

    def shutdown_request(self, request):
        if request.fileno() in self.rst_fds:
            self.rst_fds.discard(request.fileno())
            self.close_request(request)
            return
        super().shutdown_request(request)


if __name__ == "__main__":
    server = Server(("127.0.0.1", PORT), Handler)
    if os.environ.get("STUB_TLS_CERT"):
        import ssl
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(os.environ["STUB_TLS_CERT"], os.environ["STUB_TLS_KEY"])
        server.socket = context.wrap_socket(server.socket, server_side=True)
    port = server.server_address[1]
    if ready := os.environ.get("STUB_READY"):
        temporary = ready + ".tmp"
        with open(temporary, "w") as output:
            output.write(f"{port}\n")
        os.replace(temporary, ready)
    print(f"stub model on :{port} mode={MODE}", file=sys.stderr, flush=True)
    server.serve_forever()
