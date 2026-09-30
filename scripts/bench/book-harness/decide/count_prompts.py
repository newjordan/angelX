#!/usr/bin/env python3
"""count_prompts.py <label>... — send every request (or COUNT_AT's) of each captured session
(out/<label>.jsonl) to DeepSeek flash with a one-token cap and record the
prompt tokens it counts: out/tokcount-<label>.jsonl, one row per request."""
import json, os, sys, time, urllib.error, urllib.request
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
KEY = os.environ["DEEPSEEK_API_KEY"]


def count(body):
    messages = json.loads(json.dumps(body["messages"]))
    for m in messages:
        if m.get("role") == "assistant":
            m.setdefault("reasoning_content", "")
    payload = {"model": "deepseek-flash", "messages": messages, "tools": body.get("tools", []),
               "max_tokens": 1, "stream": False}
    req = urllib.request.Request("https://api.deepseek.com/v1/chat/completions", data=json.dumps(payload).encode(),
                                 method="POST", headers={"Content-Type": "application/json", "Authorization": "Bearer " + KEY})
    for attempt in range(5):
        try:
            usage = json.load(urllib.request.urlopen(req, timeout=300)).get("usage", {})
            return usage.get("prompt_tokens"), usage.get("prompt_cache_hit_tokens"), None
        except urllib.error.HTTPError as e:
            err = f"{e.code} {e.read()[:200].decode('utf-8', 'replace')}"
            if e.code not in (429, 500, 502, 503, 504):
                return None, None, err
        except Exception as e:
            err = repr(e)[:200]
        time.sleep(3 * (attempt + 1))
    return None, None, err


for label in sys.argv[1:]:
    rows = [json.loads(line) for line in open(os.path.join(ROOT, "out", f"{label}.jsonl"))]
    # COUNT_AT=9,51,213: only those request numbers (default: every request).
    at = [int(n) for n in os.environ.get("COUNT_AT", "").split(",") if n]
    numbers = at or list(range(1, len(rows) + 1))
    rows = [rows[n - 1] for n in numbers]
    with ThreadPoolExecutor(8) as pool:
        counted = list(pool.map(lambda row: count(row["body"]), rows))
    with open(os.path.join(ROOT, "out", f"tokcount-{label}.jsonl"), "w") as out:
        for i, (prompt, cached, err), row in zip(numbers, counted, rows):
            out.write(json.dumps({"label": label, "i": i, "prompt": prompt, "cached": cached,
                                  "bytes": row["bytes"], "error": err}) + "\n")
    errors = sum(1 for c in counted if c[2])
    print(f"{label}: {len(rows)} requests, final {counted[-1][0]} prompt tokens, errors {errors}", flush=True)
