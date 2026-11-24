"""Metering proxy: the agent talks to this, and this talks to the model provider.

One proxy runs per task, on a free local port. It
  * holds the real API key. The agent gets a placeholder, so no harness can read
    or leak the key, and every harness is billed through the same door;
  * imposes the run's sampling (temperature, max_tokens, thinking...) on every
    generation request, the way the published runs' interception proxy did, so
    two harnesses on one config ask the model for the same thing;
  * records each call: latency, time to first byte, HTTP status, and token usage
    (OpenAI chat/completions, the Responses API and Anthropic Messages, streamed
    or not).

Paths pass through unchanged. With upstream https://api.deepseek.com/v1 the agent's
base URL is http://127.0.0.1:<port>/v1, and /v1/chat/completions goes to
https://api.deepseek.com/v1/chat/completions.
"""

from __future__ import annotations

import http.client
import json
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlsplit

GENERATION_SUFFIXES = ("/chat/completions", "/completions", "/responses", "/messages")
HOP_BY_HOP = {
    "connection", "keep-alive", "proxy-authenticate", "proxy-authorization", "te",
    "trailers", "transfer-encoding", "upgrade", "content-length", "content-encoding", "host",
}
AUTH_HEADERS = {"authorization", "x-api-key", "api-key", "x-goog-api-key"}


def _num(value) -> int:
    return int(value) if isinstance(value, (int, float)) else 0


def usage_of(event: dict) -> dict | None:
    if not isinstance(event, dict):
        return None
    for holder in (event, event.get("message"), event.get("response")):
        if isinstance(holder, dict) and isinstance(holder.get("usage"), dict):
            return holder["usage"]
    return None


def normalize(usage: dict) -> dict:
    """One shape for every API: input = all prompt tokens (cached included),
    cached = the part served from the provider's prompt cache."""
    details_in = usage.get("prompt_tokens_details") or usage.get("input_tokens_details") or {}
    details_out = usage.get("completion_tokens_details") or usage.get("output_tokens_details") or {}
    anthropic = "cache_read_input_tokens" in usage or "cache_creation_input_tokens" in usage
    if anthropic:
        cached = _num(usage.get("cache_read_input_tokens"))
        written = _num(usage.get("cache_creation_input_tokens"))
        total_in = _num(usage.get("input_tokens")) + cached + written
    else:
        total_in = _num(usage.get("prompt_tokens") or usage.get("input_tokens"))
        cached = _num(
            details_in.get("cached_tokens")
            or usage.get("prompt_cache_hit_tokens")
            or usage.get("cached_tokens")
        )
    return {
        "input": total_in,
        "cached": cached,
        "output": _num(usage.get("completion_tokens") or usage.get("output_tokens")),
        "reasoning": _num(details_out.get("reasoning_tokens")),
    }


def merge(acc: dict, usage: dict | None) -> None:
    """Streams report usage in pieces (Anthropic: input at message_start, output at
    message_delta) or once at the end; keep the largest value seen per field."""
    if usage:
        for key, value in normalize(usage).items():
            acc[key] = max(acc.get(key, 0), value)


class MeterProxy:
    def __init__(self, upstream: str, key: str = "", headers: dict | None = None,
                 sampling: dict | None = None, stream_usage: bool = True,
                 auth: str = "auto", timeout: float = 900.0):
        parts = urlsplit(upstream.rstrip("/"))
        if parts.scheme not in ("http", "https"):
            raise ValueError(f"upstream must be http(s): {upstream}")
        self.scheme = parts.scheme
        self.netloc = parts.netloc
        self.base_path = parts.path
        self.key = key
        self.headers = dict(headers or {})
        self.sampling = dict(sampling or {})
        self.stream_usage = stream_usage
        self.auth = auth
        self.timeout = timeout
        self.calls: list[dict] = []
        self._lock = threading.Lock()
        self._server: ThreadingHTTPServer | None = None

    @property
    def base_url(self) -> str:
        assert self._server is not None
        return f"http://127.0.0.1:{self._server.server_address[1]}{self.base_path}"

    def start(self) -> "MeterProxy":
        proxy = self

        class Handler(BaseHTTPRequestHandler):
            protocol_version = "HTTP/1.1"

            def log_message(self, *args):
                pass

            def do_GET(self):
                proxy.forward(self, None)

            def do_POST(self):
                length = int(self.headers.get("Content-Length") or 0)
                proxy.forward(self, self.rfile.read(length) if length else b"")

        self._server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self._server.daemon_threads = True
        threading.Thread(target=self._server.serve_forever, daemon=True).start()
        return self

    def stop(self) -> list[dict]:
        if self._server:
            self._server.shutdown()
            self._server.server_close()
        with self._lock:
            return list(self.calls)

    # -- request shaping -------------------------------------------------

    def shape(self, path: str, body: bytes | None) -> tuple[bytes | None, bool]:
        """Impose sampling on generation requests. Returns (body, streamed)."""
        if not body or not path.split("?")[0].endswith(GENERATION_SUFFIXES):
            return body, False
        try:
            payload = json.loads(body)
        except (json.JSONDecodeError, UnicodeDecodeError):
            return body, False
        if not isinstance(payload, dict):
            return body, False
        for key, value in self.sampling.items():
            if value is None:
                payload.pop(key, None)
            elif key == "max_tokens" and "max_completion_tokens" in payload:
                payload["max_completion_tokens"] = value
            elif key == "max_tokens" and path.split("?")[0].endswith("/responses"):
                payload["max_output_tokens"] = value
            else:
                payload[key] = value
        streamed = bool(payload.get("stream"))
        if (streamed and self.stream_usage and path.split("?")[0].endswith("/chat/completions")
                and "stream_options" not in payload):
            payload["stream_options"] = {"include_usage": True}
        return json.dumps(payload).encode(), streamed

    def upstream_headers(self, incoming) -> dict:
        out = {}
        sent_x_api_key = False
        for name, value in incoming.items():
            low = name.lower()
            if low in AUTH_HEADERS:
                sent_x_api_key |= low == "x-api-key"
                continue
            if low in HOP_BY_HOP or low == "accept-encoding":
                continue
            out[name] = value
        out.update(self.headers)
        out["Accept-Encoding"] = "identity"
        if self.key:
            style = self.auth if self.auth != "auto" else ("x-api-key" if sent_x_api_key else "bearer")
            if style == "x-api-key":
                out["x-api-key"] = self.key
            else:
                out["Authorization"] = f"Bearer {self.key}"
        return out

    # -- forwarding ------------------------------------------------------

    def forward(self, handler: BaseHTTPRequestHandler, body: bytes | None) -> None:
        started = time.time()
        clock = time.monotonic()
        body, streamed = self.shape(handler.path, body)
        record = {"t": round(started, 3), "path": handler.path.split("?")[0], "stream": streamed}
        usage: dict = {}
        conn_cls = http.client.HTTPSConnection if self.scheme == "https" else http.client.HTTPConnection
        conn = conn_cls(self.netloc, timeout=self.timeout)
        try:
            conn.request(handler.command, handler.path, body=body, headers=self.upstream_headers(handler.headers))
            resp = conn.getresponse()
            record["ttfb_s"] = round(time.monotonic() - clock, 3)
            record["status"] = resp.status
            ctype = resp.getheader("Content-Type", "")
            handler.send_response(resp.status)
            for name, value in resp.getheaders():
                if name.lower() not in HOP_BY_HOP:
                    handler.send_header(name, value)
            if "text/event-stream" in ctype:
                handler.send_header("Transfer-Encoding", "chunked")
                handler.end_headers()
                pending = b""
                while True:
                    chunk = resp.read1(65536)
                    if not chunk:
                        break
                    handler.wfile.write(b"%x\r\n%s\r\n" % (len(chunk), chunk))
                    handler.wfile.flush()
                    pending += chunk
                    *lines, pending = pending.split(b"\n")
                    for line in lines:
                        line = line.strip()
                        if line.startswith(b"data:") and line[5:].strip() not in (b"", b"[DONE]"):
                            try:
                                merge(usage, usage_of(json.loads(line[5:])))
                            except (json.JSONDecodeError, UnicodeDecodeError):
                                pass
                handler.wfile.write(b"0\r\n\r\n")
                handler.wfile.flush()
            else:
                data = resp.read()
                handler.send_header("Content-Length", str(len(data)))
                handler.end_headers()
                handler.wfile.write(data)
                handler.wfile.flush()
                if resp.status >= 400:
                    record["error"] = data[:300].decode("utf-8", "replace")
                elif "json" in ctype:
                    try:
                        merge(usage, usage_of(json.loads(data)))
                    except (json.JSONDecodeError, UnicodeDecodeError):
                        pass
        except (BrokenPipeError, ConnectionResetError):
            record["error"] = "client disconnected"
        except (OSError, http.client.HTTPException) as err:
            record["status"] = record.get("status", 502)
            record["error"] = f"upstream: {err}"
            handler.close_connection = True
            if "ttfb_s" not in record:  # nothing sent to the agent yet: answer with a 502
                try:
                    msg = json.dumps({"error": {"message": f"bench proxy: {err}"}}).encode()
                    handler.send_response(502)
                    handler.send_header("Content-Type", "application/json")
                    handler.send_header("Content-Length", str(len(msg)))
                    handler.end_headers()
                    handler.wfile.write(msg)
                except OSError:
                    pass
        finally:
            conn.close()
        record["latency_s"] = round(time.monotonic() - clock, 3)
        record.update(usage)
        if record["path"].endswith(GENERATION_SUFFIXES) or usage:
            with self._lock:
                self.calls.append(record)
