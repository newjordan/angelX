"""Checked accounting for raw provider receipts recorded by proxy_log.py."""

import importlib.util
import json
from pathlib import Path


spec = importlib.util.spec_from_file_location(
    "book_receipt_history", Path(__file__).resolve().parents[2] / "analyze-polyglot-history.py")
history = importlib.util.module_from_spec(spec)
spec.loader.exec_module(history)
FIELDS = ("prompt", "cached", "fresh", "completion", "reasoning")


def number(value):
    value = history.number(value)
    return value if value is not None and value >= 0 else None


def detail(usage, name, field):
    value = usage.get(name)
    return number(value.get(field)) if isinstance(value, dict) else None


def receipt_fields(usage):
    """Retain independent measurements; a cache miss is not an absent field."""
    usage = usage if isinstance(usage, dict) else {}
    if "prompt_tokens" in usage:  # Chat Completions includes cached input.
        prompt = number(usage.get("prompt_tokens"))
        details = usage.get("prompt_tokens_details")
        cached = (detail(usage, "prompt_tokens_details", "cached_tokens")
                  if isinstance(details, dict) and "cached_tokens" in details
                  else number(usage.get("prompt_cache_hit_tokens")))
        completion = number(usage.get("completion_tokens"))
        reasoning = detail(usage, "completion_tokens_details", "reasoning_tokens")
    elif "cache_read_input_tokens" in usage or "cache_creation_input_tokens" in usage:
        # Messages input_tokens excludes both cache reads and cache creation.
        pieces = [number(usage.get(key)) for key in
                  ("input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens")]
        prompt = sum(pieces) if all(value is not None for value in pieces) else None
        cached = number(usage.get("cache_read_input_tokens"))
        completion = number(usage.get("output_tokens"))
        reasoning = detail(usage, "output_tokens_details", "reasoning_tokens")
    else:  # Responses input_tokens already includes its cached share.
        prompt = number(usage.get("input_tokens"))
        cached = detail(usage, "input_tokens_details", "cached_tokens")
        completion = number(usage.get("output_tokens"))
        reasoning = detail(usage, "output_tokens_details", "reasoning_tokens")
    fresh = prompt - cached if prompt is not None and cached is not None and cached <= prompt else None
    return {"prompt": prompt, "cached": cached, "fresh": fresh,
            "completion": completion, "reasoning": reasoning}


def usage_summary(rows):
    observed = [receipt_fields(row.get("usage")) for row in rows]
    fields, subtotals, coverage = history.receipt_fields(
        {key: [receipt[key] for receipt in observed] for key in FIELDS})
    return {**fields, "usage_reported_subtotals": subtotals, "usage_metric_coverage": coverage,
            "cache_invalid_receipts": sum(receipt["prompt"] is not None and receipt["cached"] is not None
                                          and receipt["cached"] > receipt["prompt"] for receipt in observed),
            "proxy_errors": sum(bool(row.get("proxy_error")) for row in rows)}


def read_receipts(path):
    rows = []
    with Path(path).open() as stream:
        for line_no, line in enumerate(stream, 1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except ValueError as error:
                raise ValueError(f"invalid receipt JSON: {path}:{line_no}") from error
            if isinstance(row, dict) and "body" in row:
                rows.append(row)
    return rows


def per_request(rows):
    out = []
    for row in rows:
        measured = usage_summary([row])
        body = row.get("body") if isinstance(row.get("body"), dict) else {}
        messages = body.get("messages") or body.get("input") or []
        timestamp = number(row.get("t"))
        out.append({"t": round(timestamp, 3) if timestamp is not None else None,
                    "prompt": measured["prompt"], "cached": measured["cached"],
                    "messages": len(messages) if isinstance(messages, list) else None,
                    "usage_reported_subtotals": measured["usage_reported_subtotals"],
                    "usage_metric_coverage": measured["usage_metric_coverage"],
                    "cache_invalid_receipts": measured["cache_invalid_receipts"],
                    "proxy_error": row.get("proxy_error")})
    return out
