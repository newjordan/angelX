#!/usr/bin/env python3
"""Print Grok usage field names and counts only. Never print the bearer."""
import json
import urllib.request
from pathlib import Path

auth = json.loads((Path.home() / ".grok" / "auth.json").read_text())
entry = next(iter(auth.values()))
token = entry["key"]
url = "https://api.x.ai/v1/chat/completions"
body = {
    "model": "grok-4.7",
    "messages": [{"role": "user", "content": "Reply with the single word pong."}],
    "max_tokens": 32,
    "temperature": 0,
    "reasoning_effort": "low",
    "stream": False,
}


def post(payload):
    req = urllib.request.Request(
        url,
        data=json.dumps(payload).encode(),
        headers={
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
            "Accept": "application/json",
        },
    )
    with urllib.request.urlopen(req, timeout=90) as resp:
        raw = resp.read().decode()
        return resp.status, raw


status, raw = post(body)
data = json.loads(raw)
usage = data.get("usage")
print("nostream_status", status)
print("top_keys", sorted(data.keys()))
print("usage", json.dumps(usage))
print("has_usage_substring", "usage" in raw)

stream_body = dict(body)
stream_body["stream"] = True
stream_body["stream_options"] = {"include_usage": True}
req = urllib.request.Request(
    url,
    data=json.dumps(stream_body).encode(),
    headers={
        "Authorization": f"Bearer {token}",
        "Content-Type": "application/json",
        "Accept": "text/event-stream",
    },
)
usages = []
with urllib.request.urlopen(req, timeout=90) as resp:
    print("stream_status", resp.status)
    for line in resp:
        text = line.decode(errors="replace").strip()
        if not text.startswith("data:") or text == "data: [DONE]":
            continue
        chunk = json.loads(text[5:].strip())
        if "usage" in chunk and chunk["usage"] is not None:
            usages.append(chunk["usage"])
print("stream_usage_frames", len(usages))
if usages:
    print("stream_usage_last", json.dumps(usages[-1]))
