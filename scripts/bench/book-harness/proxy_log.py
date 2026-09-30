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
    return event.get("usage") or (event.get("response") or {}).get("usage")


def usage_of(text):
    try:
        body = json.loads(text)
        return usage_in(body) or (body.get("message") or {}).get("usage")
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
            found = usage_in(event) or (event.get("message") or {}).get("usage")
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

    def relay(self, response, raw=None, arrived=None):
        status = getattr(response, "status", None) or response.code
        self.send_response(status)
        self.send_header("Content-Type", response.headers.get("Content-Type", "application/json"))
        self.send_header("Connection", "close")
        self.end_headers()
        chunks = []
        while True:
            chunk = response.read1(65536) if hasattr(response, "read1") else response.read(65536)
            if not chunk:
                break
            chunks.append(chunk)
            self.wfile.write(chunk)
            self.wfile.flush()
        self.close_connection = True
        if raw is not None:
            text = b"".join(chunks).decode("utf-8", "replace")
            with open(LOG, "a") as f:
                f.write(json.dumps({"t": arrived, "done": time.time(), "bytes": len(raw),
                                    "status": status, "body": json.loads(raw),
                                    "usage": usage_of(text)}) + "\n")

    def do_GET(self):
        # Probes (/models, /props) go to <log>.get, so the request log keeps
        # only chat bodies.
        arrived = time.time()
        response = self.forward("GET")
        self.relay(response)
        with open(LOG + ".get", "a") as f:
            f.write(json.dumps({"t": arrived, "done": time.time(), "path": self.path,
                                "status": getattr(response, "status", None) or response.code}) + "\n")

    def do_POST(self):
        raw = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        arrived = time.time()
        self.relay(self.forward("POST", raw), raw if raw else None, arrived)


if __name__ == "__main__":
    ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
