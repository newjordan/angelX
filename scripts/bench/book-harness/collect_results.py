#!/usr/bin/env python3
"""collect_results.py <out-dir>... > results.json — every run's metrics, for
the report's charts. Reads the proxy logs (tokens, cache, requests), the run
lines (solved, wall) and the task receipts (timing split) under each out/ dir;
a later dir wins for a label present in both (sparky's copy after atlas's)."""
import glob
import json
import os
import re
import sys


def usage_totals(rows):
    prompt = cached = completion = reasoning = 0
    for row in rows:
        u = row.get("usage") or {}
        if "prompt_tokens" in u:
            p = u.get("prompt_tokens") or 0
            c = ((u.get("prompt_tokens_details") or {}).get("cached_tokens")
                 or u.get("prompt_cache_hit_tokens") or 0)
        else:  # Responses or Messages
            read = u.get("cache_read_input_tokens") or 0
            p = (u.get("input_tokens") or 0) + read + (u.get("cache_creation_input_tokens") or 0)
            c = read or (u.get("input_tokens_details") or {}).get("cached_tokens") or 0
        prompt += p
        cached += c
        completion += u.get("completion_tokens") or u.get("output_tokens") or 0
        reasoning += ((u.get("completion_tokens_details") or u.get("output_tokens_details") or {})
                      .get("reasoning_tokens") or 0)
    return prompt, cached, completion, reasoning


def per_request(rows):
    out = []
    for row in rows:
        u = row.get("usage") or {}
        if "prompt_tokens" in u:
            p = u.get("prompt_tokens") or 0
            c = ((u.get("prompt_tokens_details") or {}).get("cached_tokens")
                 or u.get("prompt_cache_hit_tokens") or 0)
        else:
            read = u.get("cache_read_input_tokens") or 0
            p = (u.get("input_tokens") or 0) + read + (u.get("cache_creation_input_tokens") or 0)
            c = read
        out.append({"t": round(row.get("t", 0), 3), "prompt": p, "cached": c,
                    "messages": len(row["body"].get("messages") or row["body"].get("input") or [])})
    return out


runs = {}
for root in sys.argv[1:]:
    for log in glob.glob(os.path.join(root, "*.jsonl")):
        label = os.path.basename(log)[:-6]
        if label.startswith("e2-count") or label.startswith("e2-") or "pilot" in label:
            continue
        rows = []
        for line in open(log):
            try:
                row = json.loads(line)
            except ValueError:
                continue
            if "body" in row:
                rows.append(row)
        if not rows:
            continue
        prompt, cached, completion, reasoning = usage_totals(rows)
        run = {"label": label, "requests": len(rows), "prompt": prompt, "cached": cached,
               "fresh": prompt - cached, "completion": completion, "reasoning": reasoning,
               "model": rows[0]["body"].get("model"), "host": "sparky" if "sparky" in root else "atlas"}
        runline = os.path.join(root, label + ".run")
        if os.path.exists(runline):
            text = open(runline).read()
            run["solved"] = "solved=yes" in text if "solved=" in text else None
            wall = re.search(r"wall=([\d.]+)", text)
            run["wall_s"] = float(wall.group(1)) if wall else None
            status = re.search(r"status=(\S+)", text)
            run["status"] = status.group(1) if status else None
        receipt = os.path.join(root, label + ".result.json")
        if os.path.exists(receipt):
            try:
                timing = json.load(open(receipt)).get("timing") or {}
                run["timing"] = {k: timing.get(k) for k in (
                    "wall_ms", "model_ms", "tool_ms", "tool_overhead_ms", "residual_ms",
                    "other_ms", "startup_ms", "shutdown_ms", "model_calls", "tool_calls")}
            except ValueError:
                pass
        if label.startswith(("e3-", "dsfinal-", "dsloop-", "twoturn-")):
            run["per_request"] = per_request(rows)
        runs[label] = run

e2 = []
for root in sys.argv[1:]:
    for path in glob.glob(os.path.join(root, "e2-count-*.jsonl")):
        for line in open(path):
            try:
                d = json.loads(line)
            except ValueError:
                continue
            if d.get("prompt_tokens"):
                e2.append({"route": d["route"], "label": d["label"], "prompt_tokens": d["prompt_tokens"]})

json.dump({"runs": runs, "e2": e2}, sys.stdout)
