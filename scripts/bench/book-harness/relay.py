#!/usr/bin/env python3
"""OpenAI-compatible relay: each chat request waits for a hand-written reply.

Request N is written to DIR/N.req.json (the body the model would see, after
the optional DIRECTIVE is prepended to the system prompt); the relay blocks
until DIR/N.resp.json holds {"content": "..."} or {"tool_calls": [{"name",
"arguments"}]}, then answers in the requested (stream or plain) shape.
"""
import json
import os
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from relay_files import write_json

DIR = os.environ["RELAY_DIR"]
PORT = int(os.environ["RELAY_PORT"])
DIRECTIVE = os.environ.get("RELAY_DIRECTIVE", "")
state = {"n": 0}


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *args):
        pass

    def _json(self, code, payload):
        data = json.dumps(payload).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.path.rstrip("/").endswith("/models"):
            self._json(200, {"object": "list", "data": [{"id": "relay", "object": "model"}]})
        else:
            self._json(404, {"error": "not found"})

    def do_POST(self):
        raw = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        if not self.path.rstrip("/").endswith("/chat/completions"):
            self._json(404, {"error": "not found"})
            return
        body = json.loads(raw)
        if DIRECTIVE:
            for message in body.get("messages", []):
                if message.get("role") == "system":
                    message["content"] = DIRECTIVE + "\n" + (message.get("content") or "")
                    break
        state["n"] += 1
        n = state["n"]
        write_json(os.path.join(DIR, f"{n}.req.json"),
                   {"t": time.time(), "bytes": len(raw), "body": body})
        resp_path = os.path.join(DIR, f"{n}.resp.json")
        while not os.path.exists(resp_path):
            time.sleep(0.5)
        time.sleep(0.2)
        reply = json.load(open(resp_path))
        calls = [
            {"id": f"call_{n}_{i}", "type": "function",
             "function": {"name": c["name"], "arguments": json.dumps(c.get("arguments", {}))}}
            for i, c in enumerate(reply.get("tool_calls") or [])
        ]
        message = {"role": "assistant", "content": reply.get("content")}
        if calls:
            message["tool_calls"] = calls
        finish = "tool_calls" if calls else "stop"
        usage = {"prompt_tokens": len(raw) // 4, "completion_tokens": 20, "total_tokens": len(raw) // 4 + 20}
        if not body.get("stream"):
            self._json(200, {"id": f"relay-{n}", "object": "chat.completion", "model": "relay",
                             "choices": [{"index": 0, "message": message, "finish_reason": finish}],
                             "usage": usage})
            return
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Connection", "close")
        self.end_headers()
        delta = {"role": "assistant"}
        if message.get("content"):
            delta["content"] = message["content"]
        if calls:
            delta["tool_calls"] = [{"index": i, **c} for i, c in enumerate(calls)]
        for chunk in (
            {"id": "relay", "object": "chat.completion.chunk", "model": "relay",
             "choices": [{"index": 0, "delta": delta, "finish_reason": None}]},
            {"id": "relay", "object": "chat.completion.chunk", "model": "relay",
             "choices": [{"index": 0, "delta": {}, "finish_reason": finish}], "usage": usage},
        ):
            self.wfile.write(f"data: {json.dumps(chunk)}\n\n".encode())
        self.wfile.write(b"data: [DONE]\n\n")
        self.wfile.flush()
        self.close_connection = True


if __name__ == "__main__":
    with ThreadingHTTPServer(("127.0.0.1", PORT), Handler) as server:
        ready = os.environ.get("RELAY_READY")
        if ready:
            with open(ready, "w") as receipt:
                receipt.write(str(server.server_port) + "\n")
        server.serve_forever()
