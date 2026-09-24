"""A loopback OpenAI-compatible provider that replies from a script.

Fixture for the rollout persistence contract test: each POST gets the next
scripted server-sent-events body, and the requests are kept for assertions.
Lifted from the operator's rollout-seed bench, which is not shipped.
"""

from __future__ import annotations

import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


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
