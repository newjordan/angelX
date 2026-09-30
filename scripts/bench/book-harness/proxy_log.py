#!/usr/bin/env python3
"""Pass-through logger for an OpenAI-compatible endpoint (a real model run).

  PROXY_UPSTREAM=https://host/v1 PROXY_KEY=... PROXY_LOG=out/label.jsonl \
  PROXY_PORT=18100 python3 proxy_log.py

Point angel at http://127.0.0.1:$PROXY_PORT/v1 (ANGEL_DRIVER=local,
ANGEL_LOCAL_URL=...). Every chat request is forwarded unchanged; its body,
arrival time, finish time and the provider's usage are appended to PROXY_LOG
in the stub_model.py format ({"t", "bytes", "body", ...}), so gaps.py,
phases.py, harness_time.py and tokens/analyze.mjs read real-model runs too.
"""
import json
import http.client
import os
import subprocess
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

UPSTREAM = os.environ["PROXY_UPSTREAM"].rstrip("/")
KEY = os.environ.get("PROXY_KEY", "")
# PROXY_KEY_CMD prints the upstream bearer (an OAuth token that refreshes on
# disk); it is re-run at most once a minute so a refreshed token is picked up.
KEY_CMD = os.environ.get("PROXY_KEY_CMD", "")
_key_cache = {"at": 0.0, "value": ""}


def upstream_key():
    if not KEY_CMD:
        return KEY
    if time.time() - _key_cache["at"] > 60:
        _key_cache["value"] = subprocess.run(KEY_CMD, shell=True, capture_output=True,
                                             text=True, check=True).stdout.strip()
        _key_cache["at"] = time.time()
    return _key_cache["value"]
LOG = os.environ["PROXY_LOG"]
PORT = int(os.environ.get("PROXY_PORT", "18100"))


def usage_in(event):
    # Chat Completions carries usage at the top; the Responses API nests it in
    # the response object (the final `response.completed` event, or the body).
    if not isinstance(event, dict):
        return None
    response = event.get("response")
    usage = event.get("usage") or (response.get("usage") if isinstance(response, dict) else None)
    return usage if isinstance(usage, dict) else None


def usage_of(text):
    try:
        body = json.loads(text)
        message = body.get("message") if isinstance(body, dict) else None
        return usage_in(body) or usage_in(message)
    except ValueError:
        pass
    usage = None
    for line in text.splitlines():
        if line.startswith("data:") and "usage" in line:
            try:
                event = json.loads(line[5:].strip())
            except ValueError:
                continue
            # Messages: message_start carries the input side, message_delta
            # the output side; merge them.
            message = event.get("message") if isinstance(event, dict) else None
            found = usage_in(event) or usage_in(message)
            if found:
                usage = {**(usage or {}), **found}
    return usage


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *args):
        pass

    def forward(self, method, body=None):
        path = self.path[3:] if self.path.startswith("/v1") else self.path
        request = urllib.request.Request(UPSTREAM + path, data=body, method=method)
        request.add_header("Content-Type", "application/json")
        # The Messages seat authenticates itself; pass its headers through.
        for name in ("x-api-key", "anthropic-version", "anthropic-beta"):
            if self.headers.get(name):
                request.add_header(name, self.headers[name])
        key = upstream_key()
        if key and not self.headers.get("x-api-key"):
            request.add_header("Authorization", "Bearer " + key)
        try:
            return urllib.request.urlopen(request, timeout=900)
        except urllib.error.HTTPError as error:
            return error

    def relay(self, response):
        status = getattr(response, "status", None) or response.code
        self.send_response(status)
        self.send_header("Content-Type", response.headers.get("Content-Type", "application/json"))
        self.send_header("Connection", "close")
        self.end_headers()
        chunks = []
        while True:
            chunk = response.read1(65536) if hasattr(response, "read1") else response.read(65536)
            if not chunk:
                # HTTPResponse.read1 (and bounded read) accepts an early EOF
                # without raising when Content-Length still promises bytes.
                # Already forwarded headers/body cannot be recalled; refuse
                # the completion receipt rather than claiming full usage.
                remaining = getattr(response, "length", None)
                if isinstance(remaining, int) and remaining > 0:
                    raise http.client.IncompleteRead(b"", remaining)
                break
            chunks.append(chunk)
            self.wfile.write(chunk)
            self.wfile.flush()
        self.close_connection = True
        return b"".join(chunks).decode("utf-8", "replace")

    def proxy_failure(self, error, record):
        # Keep the attempted call even when no response/usage was observed.
        # Exception strings can contain URLs or credential command output.
        record["proxy_error"] = type(error).__name__
        self.close_connection = True
        if record["status"] is None:
            try:
                self.send_error(502, "Upstream request failed")
            except OSError:
                pass

    def do_GET(self):
        # Probes (/models, /props) go to <log>.get, so the request log keeps
        # only chat bodies.
        arrived = time.time()
        record = {"t": arrived, "path": self.path, "status": None}
        try:
            with self.forward("GET") as response:
                record["status"] = getattr(response, "status", None) or response.code
                self.relay(response)
        except (OSError, http.client.HTTPException, subprocess.SubprocessError) as error:
            self.proxy_failure(error, record)
        finally:
            record["done"] = time.time()
            with open(LOG + ".get", "a") as f:
                f.write(json.dumps(record) + "\n")

    def do_POST(self):
        raw = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        arrived = time.time()
        try:
            body = json.loads(raw)
        except ValueError:
            body = None
        record = {"t": arrived, "bytes": len(raw), "body": body, "status": None, "usage": None}
        try:
            with self.forward("POST", raw) as response:
                record["status"] = getattr(response, "status", None) or response.code
                text = self.relay(response)
                record["usage"] = usage_of(text)
        except (OSError, http.client.HTTPException, subprocess.SubprocessError) as error:
            self.proxy_failure(error, record)
        finally:
            record["done"] = time.time()
            with open(LOG, "a") as f:
                f.write(json.dumps(record) + "\n")


if __name__ == "__main__":
    server = ThreadingHTTPServer(("127.0.0.1", PORT), Handler)
    ready = os.environ.get("PROXY_READY")
    if ready:
        with open(ready + ".tmp", "w") as receipt:
            receipt.write(str(server.server_address[1]) + "\n")
        os.replace(ready + ".tmp", ready)
    server.serve_forever()
