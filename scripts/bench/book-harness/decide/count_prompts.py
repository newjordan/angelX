#!/usr/bin/env python3
"""count_prompts.py <label>... — send every request (or COUNT_AT's) of each captured session
(out/<label>.jsonl) to DeepSeek flash with a one-token cap and record the
prompt tokens it counts: out/tokcount-<label>.jsonl, one row per request."""
import argparse
import copy
import importlib.util
import json
import os
from pathlib import Path
import sys
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor


ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("prompt_count_receipt", ROOT / "receipt_usage.py")
receipt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(receipt)
RETRY_STATUSES = (429, 500, 502, 503, 504)


def selection(value):
    """COUNT_AT names actual, distinct positive request numbers, in given order."""
    if not value:
        return None
    parts = value.split(",")
    if any(not part.strip() for part in parts):
        raise ValueError("COUNT_AT must contain positive request numbers")
    try:
        numbers = [int(part) for part in parts]
    except ValueError as error:
        raise ValueError("COUNT_AT must contain positive request numbers") from error
    if any(number <= 0 for number in numbers) or len(set(numbers)) != len(numbers):
        raise ValueError("COUNT_AT must contain distinct positive request numbers")
    return numbers


def payload_for(body):
    if not isinstance(body, dict) or not isinstance(body.get("messages"), list):
        raise ValueError("capture must contain messages")
    messages = copy.deepcopy(body["messages"])
    for message in messages:
        if not isinstance(message, dict):
            raise ValueError("captured messages must be objects")
        if message.get("role") == "assistant":
            message.setdefault("reasoning_content", "")
    return {"model": "deepseek-flash", "messages": messages, "tools": copy.deepcopy(body.get("tools", [])),
            "max_tokens": 1, "stream": False}


def request_count(payload, key):
    req = urllib.request.Request("https://api.deepseek.com/v1/chat/completions", data=json.dumps(payload).encode(),
                                 method="POST", headers={"Content-Type": "application/json", "Authorization": "Bearer " + key})
    attempts = []
    measured, error_class, status = None, None, None
    for attempt in range(5):
        measured, error_class, status = None, None, None
        retry, usage_invalid = False, False
        try:
            with urllib.request.urlopen(req, timeout=300) as response:
                observed = getattr(response, "status", None)
                status = observed if isinstance(observed, int) and not isinstance(observed, bool) else None
                reply = json.load(response)
                if not isinstance(reply, dict):
                    raise ValueError("provider reply must be an object")
                measured = reply.get("usage", {})
                if not isinstance(measured, dict):
                    raise ValueError("provider usage must be an object")
                try:
                    json.dumps(measured, allow_nan=False)
                except ValueError as error:
                    # json.load accepts NaN, which the old counter did not
                    # retry. Keep independent finite fields; flag the bad
                    # receipt without creating extra provider requests.
                    error_class, usage_invalid = type(error).__name__, True
        except urllib.error.HTTPError as error:
            status, error_class = error.code, type(error).__name__
            retry = status in RETRY_STATUSES
            try:
                error.close()
            except Exception:
                pass
        except Exception as error:
            # Preserve the existing policy: all non-HTTP exceptions retry.
            error_class, retry = type(error).__name__, True
        if error_class is not None and not usage_invalid:
            measured = None
        outcome = {"attempt": attempt + 1, "status": status, "error_class": error_class,
                   "usage": None if usage_invalid else measured}
        if usage_invalid:
            outcome["usage_invalid"] = True
        attempts.append(outcome)
        if not retry:
            break
        # Include the old final exhausted backoff as well as its five tries.
        time.sleep(3 * (attempt + 1))
    details = measured.get("prompt_tokens_details") if measured is not None else None
    cached = (receipt.number(details.get("cached_tokens")) if isinstance(details, dict) and "cached_tokens" in details
              else receipt.number(measured.get("prompt_cache_hit_tokens")) if measured is not None else None)
    return {"prompt": receipt.number(measured.get("prompt_tokens")) if measured is not None else None,
            "cached": cached, "error": error_class, "error_class": error_class, "status": status, "attempts": attempts}


def count(body, key=None):
    try:
        payload = payload_for(body)
        json.dumps(payload, allow_nan=False)
        key = os.environ["DEEPSEEK_API_KEY"] if key is None else key
        return request_count(payload, key)
    except Exception as error:
        return {"prompt": None, "cached": None, "error": type(error).__name__, "error_class": type(error).__name__,
                "status": None, "attempts": [], "setup_error": True}


def capture_plans(labels, out_dir, at):
    if len(set(labels)) != len(labels):
        raise ValueError("labels must be distinct to preserve each output")
    out_dir = Path(out_dir)
    sources = []
    for label in labels:
        if not label or label in (".", "..") or Path(label).name != label:
            raise ValueError("label must name a capture inside the output directory")
        sources.append(out_dir / (label + ".jsonl"))
    # Every destination must be distinct from every selected input, including
    # later labels: tokcount-<label> may itself name a selected capture.
    try:
        resolved_sources = [source.resolve() for source in sources]
        for label in labels:
            destination = out_dir / f"tokcount-{label}.jsonl"
            resolved_destination = destination.resolve()
            for source, resolved_source in zip(sources, resolved_sources):
                if resolved_destination == resolved_source or (
                        destination.exists() and source.exists() and os.path.samefile(destination, source)):
                    raise ValueError(f"prompt count output for {label} must not alias selected capture {source.name}")
    except (OSError, RuntimeError) as error:
        raise ValueError("could not validate prompt count input/output paths") from error
    plans = []
    for label, source_path in zip(labels, sources):
        captured, capture_error = [], None
        try:
            with source_path.open() as source:
                for line in source:
                    try:
                        captured.append((json.loads(line), None))
                    except ValueError as error:
                        captured.append((None, type(error).__name__))
        except (OSError, UnicodeError) as error:
            capture_error = type(error).__name__
        if capture_error is not None:
            jobs = [(label, number, None, capture_error) for number in at or [None]]
        else:
            if at and any(number > len(captured) for number in at):
                raise ValueError(f"COUNT_AT exceeds the {len(captured)} captured requests for {label}")
            numbers = at or list(range(1, len(captured) + 1))
            jobs = [(label, number, *captured[number - 1]) for number in numbers]
            if not jobs:
                jobs = [(label, None, None, "ValueError")]
        plans.append((label, jobs))
    return plans


def count_request(job):
    label, number, captured, capture_error = job
    row = {"label": label, "i": number, "prompt": None, "cached": None,
           "bytes": receipt.number(captured.get("bytes")) if isinstance(captured, dict) else None, "error": None}
    if capture_error:
        row.update(error=capture_error, error_class=capture_error, status=None, attempts=[], setup_error=True)
    else:
        body = captured.get("body") if isinstance(captured, dict) else None
        row.update(count(body))
    return row


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("labels", nargs="*")
    parser.add_argument("--out-dir", type=Path, default=ROOT / "out")
    args = parser.parse_args(argv)
    try:
        plans = capture_plans(args.labels, args.out_dir, selection(os.environ.get("COUNT_AT", "")))
    except ValueError as error:
        parser.error(str(error))
    if plans:
        args.out_dir.mkdir(parents=True, exist_ok=True)
    failed = False
    for label, jobs in plans:
        counted = []
        with ThreadPoolExecutor(8) as pool, (args.out_dir / f"tokcount-{label}.jsonl").open("w") as out:
            for row in pool.map(count_request, jobs):
                out.write(json.dumps(row, allow_nan=False) + "\n")
                out.flush()
                counted.append(row)
        errors = sum(bool(row["error"]) for row in counted)
        requests = sum(row["i"] is not None for row in counted)
        print(f"{label}: {requests} requests, final {counted[-1]['prompt']} prompt tokens, errors {errors}", flush=True)
        failed = failed or bool(errors)
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
