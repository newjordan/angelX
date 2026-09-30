#!/usr/bin/env python3
"""e2_count.py <route> <label>... — E2: send each stub session's final request
to the provider (Chat Completions, smallest output cap) and print the prompt
tokens it reports. Routes: glm-flash, deepseek-flash, muse (keys from env)."""
import argparse
import copy
import http.client
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import urllib.request
import urllib.error


HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("e2_receipt_usage", HERE / "receipt_usage.py")
receipt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(receipt)

ROUTES = {
    "glm-flash": ("https://api.z.ai/api/coding/paas/v4", "ZAI_API_KEY", "glm-5.3-flash", {"thinking": {"type": "disabled"}}),
    "deepseek-flash": ("https://api.deepseek.com/v1", "DEEPSEEK_API_KEY", "deepseek-flash", {}),
    "muse": ("https://api.meta.ai/v1", "ANGEL_META_KEY", "muse-spark-1.3-contributor", {}),
    "grok": ("https://api.x.ai/v1", None, "grok-4.7", {"reasoning_effort": "low"}),
}
RETRY_STATUSES = (429, 500, 502, 503)


def final_request(label, out_dir=Path("out")):
    if not label or label in (".", "..") or Path(label).name != label:
        raise ValueError("label must name a capture inside the output directory")
    last = None
    with (Path(out_dir) / (label + ".jsonl")).open() as source:
        for line in source:
            if line.strip():
                last = json.loads(line)
    body = last.get("body") if isinstance(last, dict) else None
    if not isinstance(body, dict) or not isinstance(body.get("messages"), list):
        raise ValueError("capture has no final request with messages")
    return body


def payload_for(route, last):
    _, _, model, extra = ROUTES[route]
    messages = copy.deepcopy(last["messages"])
    if route == "deepseek-flash":  # thinking mode wants each assistant turn's reasoning back
        for m in messages:
            if m.get("role") == "assistant":
                m.setdefault("reasoning_content", "")
    return {"model": model, "messages": messages, "tools": copy.deepcopy(last.get("tools", [])),
            "max_tokens": 16, "stream": False, **extra}


def credential_for(route):
    key_env = ROUTES[route][1]
    return os.environ[key_env] if key_env else subprocess.run(
        [sys.executable, str(HERE / "grok_bearer.py")], capture_output=True, text=True, check=True).stdout.strip()


def request_usage(route, body, key):
    url = ROUTES[route][0]
    req = urllib.request.Request(url + "/chat/completions", data=json.dumps(body).encode(), method="POST",
                                 headers={"Content-Type": "application/json", "Authorization": "Bearer " + key})
    attempts = []
    usage, terminal_error, observed_status = None, None, None
    for attempt in range(3):
        status, error_class, measured = None, None, None
        retry = False
        try:
            with urllib.request.urlopen(req, timeout=300) as response:
                observed = getattr(response, "status", None)
                status = observed if isinstance(observed, int) and not isinstance(observed, bool) else None
                reply = json.load(response)
                if not isinstance(reply, dict):
                    raise ValueError("provider reply must be an object")
                measured = reply.get("usage")
                if not isinstance(measured, dict):
                    measured = None
                # Invalid JSON numbers cannot discard the whole receipt when
                # the caller serializes its result with allow_nan=False.
                json.dumps(measured, allow_nan=False)
        except urllib.error.HTTPError as error:
            status, error_class = error.code, type(error).__name__
            retry = status in RETRY_STATUSES
            try:
                error.close()
            except (OSError, ValueError):
                pass  # Retain the observed HTTP failure even if close fails.
        except (OSError, ValueError, http.client.HTTPException) as error:
            error_class = type(error).__name__
        if error_class is not None:
            measured = None
        attempts.append({"attempt": attempt + 1, "status": status,
                         "error_class": error_class, "usage": measured})
        usage, terminal_error, observed_status = measured, error_class, status
        if retry:
            # Retain the original three attempts and five-second backoff,
            # including its final exhausted backoff; no new retry conditions.
            time.sleep(5)
            continue
        break
    return {"prompt_tokens": receipt.number(usage.get("prompt_tokens")) if usage is not None else None,
            "usage": usage, "status": observed_status, "error_class": terminal_error, "attempts": attempts}


def count_label(route, label, out_dir=Path("out")):
    row = {"route": route, "label": label, "turns": None, "prompt_tokens": None,
           "usage": None, "status": None, "error_class": None, "attempts": []}
    try:
        last = final_request(label, out_dir)
        row["turns"] = len(last["messages"])
        body = payload_for(route, last)
        json.dumps(body, allow_nan=False)
        key = credential_for(route)
        result = request_usage(route, body, key)
    except (OSError, ValueError, KeyError, TypeError, AttributeError, subprocess.CalledProcessError) as error:
        row["error_class"] = type(error).__name__
        row["setup_error"] = True
        return row
    row.update(result)
    return row


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("route", choices=ROUTES)
    parser.add_argument("labels", nargs="*")
    parser.add_argument("--out-dir", type=Path, default=Path("out"))
    args = parser.parse_args(argv)
    failed = False
    for label in args.labels:
        row = count_label(args.route, label, args.out_dir)
        print(json.dumps(row, allow_nan=False), flush=True)
        failed = failed or row["error_class"] is not None
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
