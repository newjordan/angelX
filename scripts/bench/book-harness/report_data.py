#!/usr/bin/env python3
"""report_data.py results.json > report-data.json — the report's aggregates:
medians and ranges per model / arm / build, and per-request series for the
long loops. Every figure traces to runs listed in results.json."""
import argparse
import importlib.util
import json
from pathlib import Path
import os
import re
import statistics
import sys

MODELS = [("glm-flash", "GLM-5.3 flash"), ("deepseek-flash", "DeepSeek flash"),
          ("muse", "Muse Spark 1.3 contributor"), ("grok", "Grok 4.7 low")]
ARMS = [("prebook", "Pre-book English"), ("bare", "Braille only"), ("intro", "Braille + introduction")]

spec = importlib.util.spec_from_file_location("book_report_receipt_usage", Path(__file__).with_name("receipt_usage.py"))
receipt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(receipt)


def require_number(value, context):
    measured = receipt.number(value)
    if measured is None:
        raise ValueError(f"unsafe report export: {context} is missing or invalid")
    return measured


def require_usage(run, keys, context, attempts=None):
    """Report renderers do arithmetic on these values and coerce null to zero.

    template.html subtracts prompt/cached and divides for cache-share charts;
    require complete receipts before any report aggregation or JSON output.
    """
    expected = run.get("requests") if attempts is None else attempts
    coverage = run.get("usage_metric_coverage") or {}
    for key in keys:
        field = coverage.get(key) or {}
        reported, counted = field.get("reported"), field.get("attempts")
        valid_count = isinstance(expected, int) and not isinstance(expected, bool) and expected > 0
        valid_coverage = all(isinstance(value, int) and not isinstance(value, bool) for value in (reported, counted))
        if (not valid_count or not valid_coverage or field.get("complete") is not True
                or reported != expected or counted != expected or receipt.number(run.get(key)) is None):
            raise ValueError(f"unsafe report export: {context} {key} coverage={reported}/{counted} "
                             f"requests={expected}; complete receipt accounting is required")
    if any(key in ("cached", "fresh") for key in keys) and run.get("cache_invalid_receipts"):
        raise ValueError(f"unsafe report export: {context} has cache counts above prompt counts")


def stats(values):
    values = [v for v in values if v is not None]
    if not values:
        return None
    return {"median": statistics.median(values), "min": min(values), "max": max(values), "n": len(values)}


def require_clocks(runs, key, context, timing=False):
    """Every selected run must contribute to the rendered clock statistic."""
    values = []
    for run in runs:
        source = run.get("timing") if timing else run
        values.append(receipt.number(source.get(key)) if isinstance(source, dict) else None)
    reported = sum(value is not None for value in values)
    if reported != len(runs):
        missing = [run.get("label", "unknown") for run, value in zip(runs, values) if value is None]
        raise ValueError(f"unsafe report export: {context} {key} clock coverage={reported}/{len(runs)}; "
                         f"missing or invalid in {', '.join(missing)}")
    return values



def build_report_data(data, results_path):
    runs = data["runs"]

    def cohort(prefix):
        return [run for label, run in runs.items() if label.startswith(prefix) and label[len(prefix):].isdigit()]

    out = {"models": [{"id": m, "name": n} for m, n in MODELS], "arms": [{"id": a, "name": n} for a, n in ARMS]}

    # E1: tokens, turns, wall per model and arm
    e1 = []
    for model, _ in MODELS:
        for arm, _ in ARMS:
            rs = cohort(f"e1-{model}-{arm}-")
            # Muse's first "intro" arm never received the legend (it lived on the
            # Chat Completions wire only until 95c85d2); its rerun on the fixed
            # build is the legend arm.
            rerun = model == "muse" and arm == "intro" and cohort("e1-muse-introfix-")
            if rerun:
                rs = rerun
            if not rs:
                continue
            for run in rs:
                if run.get("solved"):
                    require_usage(run, ("fresh", "prompt"), run["label"])
            solved_runs = [run for run in rs if run.get("solved")]
            walls = require_clocks(solved_runs, "wall_s", f"E1 {model}/{arm}")
            delivered = bool(rerun) or not (model == "muse" and arm == "intro")
            e1.append({"model": model, "arm": arm, "delivered": delivered,
                       "solved": sum(bool(r.get("solved")) for r in rs), "n": len(rs),
                       "fresh": stats([r["fresh"] for r in rs if r.get("solved")]),
                       "prompt": stats([r["prompt"] for r in rs if r.get("solved")]),
                       "turns": stats([r["requests"] for r in rs if r.get("solved")]),
                       "wall": stats(walls),
                       "host": rs[0]["host"]})
    out["e1"] = e1

    # E2: provider-counted prompt tokens of each stub session's final request
    lengths = {"2": 9, "16": 51, "33": 102, "70": 213}
    e2 = {}
    identities = set()
    for row in data["e2"]:
        route, label = row.get("route"), row.get("label")
        if not isinstance(route, str) or not isinstance(label, str):
            raise ValueError("unsafe report export: E2 route and label must identify a count")
        identity = (route, label)
        if identity in identities:
            raise ValueError(f"unsafe report export: duplicate E2 count identity {route}/{label}")
        identities.add(identity)
        match = re.fullmatch(r"e2-(prebook|bare|intro)-(2|16|33|70)", label)
        if not match:
            raise ValueError(f"unsafe report export: E2 count label is unsupported: {label}")
        require_number(row["prompt_tokens"], f"E2 {row['label']} prompt_tokens")
        arm, cycles = match.groups()
        e2.setdefault(route, {}).setdefault(arm, {})[lengths[cycles]] = row["prompt_tokens"]
    for model, _ in MODELS:
        if model in e2:
            for arm in ("prebook", "bare", "intro"):
                for requests in lengths.values():
                    require_number(e2[model].get(arm, {}).get(requests),
                                   f"E2 {model}/{arm}/{requests} prompt tokens")
    out["e2"] = [{"model": model, "points": [
        {"requests": n, **{arm: e2[model][arm].get(n) for arm in ("prebook", "bare", "intro")}}
        for n in sorted(lengths.values())]} for model, _ in MODELS if model in e2]

    # E5: behind a real model, speed baseline against head (intro arm, two-bug task)
    e5 = []
    for model, _ in MODELS:
        for build in ("base", "head"):
            rs = cohort(f"e5-{model}-{build}-")
            selected = [r for r in rs if r.get("solved")]
            if not selected:
                continue
            clocks = {key: require_clocks(selected, key, f"E5 {model}/{build}", timing=True)
                      for key in ("wall_ms", "model_ms", "tool_ms", "tool_overhead_ms",
                                  "residual_ms", "other_ms", "startup_ms")}
            harness = [require_number(sum(values), f"E5 {model}/{build} harness clock")
                       for values in zip(clocks["tool_overhead_ms"], clocks["residual_ms"], clocks["other_ms"])]
            e5.append({"model": model, "build": build, "solved": sum(bool(r.get("solved")) for r in rs), "n": len(rs),
                       "wall_ms": stats(clocks["wall_ms"]),
                       "model_ms": stats(clocks["model_ms"]),
                       "tool_ms": stats(clocks["tool_ms"]),
                       "harness_ms": stats(harness),
                       "startup_ms": stats(clocks["startup_ms"]),
                       "host": rs[0]["host"]})
    out["e5"] = e5

    # The scripted 213-request session and time to first request (stub model)
    def timing_stat(prefix, key):
        return stats(require_clocks(cohort(prefix), key, prefix, timing=True))
    out["stub"] = {build: {"wall_ms": timing_stat(f"{build}-", "wall_ms"),
                           "startup_ms": timing_stat(f"ttfr-{build}-", "startup_ms")} for build in ("base", "head")}

    # X6 and the DeepSeek A/B
    out["x6"] = {build: {"startup_ms": timing_stat(f"x6-{build}-", "startup_ms"),
                         "wall_ms": timing_stat(f"x6-{build}-", "wall_ms")} for build in ("h", "eng")}
    dsab = []
    for arm, name in (("base", "Before the fixes (Chat)"), ("chat", "Fixed (Chat)"), ("msgs", "Fixed (Messages API)")):
        rs = cohort(f"dsab-{arm}-")
        if rs:
            for run in rs:
                require_usage(run, ("fresh",), run["label"])
            walls = require_clocks(rs, "wall_s", f"DeepSeek {arm}")
            startup = require_clocks(rs, "startup_ms", f"DeepSeek {arm}", timing=True)
            dsab.append({"arm": arm, "name": name, "solved": sum(bool(r.get("solved")) for r in rs), "n": len(rs),
                         "fresh": stats([r["fresh"] for r in rs]), "wall": stats(walls),
                         "turns": stats([r["requests"] for r in rs]),
                         "startup_ms": stats(startup)})
    out["dsab"] = dsab

    # Long DeepSeek loops: per-request prompt and cache, cumulative uncached tokens
    loops = []
    for label, name in (("e3-deepseek-flash-bare-1", "Before the fixes (Chat)"),
                        ("dsfinal-chat-1", "Fixed (Chat)"), ("dsfinal-msgs-1", "Fixed (Messages API)")):
        run = runs.get(label)
        if not run or not run.get("per_request"):
            continue
        require_usage(run, ("prompt", "cached"), label)
        if len(run["per_request"]) != run["requests"]:
            raise ValueError(f"unsafe report export: {label} request-series coverage="
                             f"{len(run['per_request'])}/{run['requests']}")
        previous_time = None
        for index, request in enumerate(run["per_request"], 1):
            require_usage(request, ("prompt", "cached"), f"{label} request {index}", attempts=1)
            timestamp = require_number(request["t"], f"{label} request {index} timestamp")
            if previous_time is not None and timestamp < previous_time:
                raise ValueError(f"unsafe report export: {label} request timestamps are reversed")
            previous_time = timestamp
        t0 = run["per_request"][0]["t"]
        series, uncached = [], 0
        for i, req in enumerate(run["per_request"]):
            uncached += req["prompt"] - req["cached"]
            series.append({"i": i + 1, "minutes": round((req["t"] - t0) / 60, 2), "prompt": req["prompt"],
                           "cached": req["cached"], "uncached_cum": uncached})
        loops.append({"label": label, "name": name, "host": run["host"], "requests": run["requests"],
                      "prompt": run["prompt"], "cached": run["cached"], "series": series})
    out["loops"] = loops

    # The two-turn follow-up: cache hit on the first request of turn 2
    follow = []
    for label, name in (("twoturn-ds", "Before"), ("twoturn-ds3", "After")):
        run = runs.get(label)
        if run and run.get("per_request"):
            last = run["per_request"][-1]
            require_usage(last, ("prompt", "cached"), f"{label} final request", attempts=1)
            if last["prompt"] == 0:
                raise ValueError(f"{label} final request has zero prompt tokens; cache-share export is undefined")
            follow.append({"name": name, "prompt": last["prompt"], "cached": last["cached"]})
    out["followup"] = follow

    # The decision test (decide/decide_table.py beside results.json)
    decide = os.path.join(os.path.dirname(os.path.abspath(str(results_path))), "decide.json")
    if os.path.exists(decide):
        spec = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "decide", "scenarios.json")))
        cells = json.load(open(decide))
        out["decide"] = {"cells": [c for c in cells if c["scenario"] != "batch2"],
                         "points": [{"id": k, "route": v["route"], "name": v["name"], "action": v["action"]}
                                    for k, v in spec.items() if k != "batch2"]}
        # The legend against English at every sighting: DeepSeek's prompt-token
        # counts through one 213-request turn, and the batch cue's second sighting
        # (turn 2 of a TUI session).
        here = os.path.dirname(os.path.abspath(__file__))
        tokens = []
        for wire in ("intro", "every", "prebook", "bare"):
            path = os.path.join(here, "out", f"tokcount-tok-{wire}.jsonl")
            if os.path.exists(path):
                tokens.append({"wire": wire, "points": [{"requests": r["i"], "prompt": require_number(r["prompt"], f"{path} prompt tokens")}
                                                         for r in map(json.loads, open(path))]})
        # Twenty turns: DeepSeek's count at the last request of each turn.
        turns = []
        for wire in ("intro", "every", "bare"):
            counted = os.path.join(here, "out", f"tokcount-t20-{wire}.jsonl")
            if not os.path.exists(counted):
                continue
            ends = [i + 1 for i, r in enumerate(map(json.loads, open(os.path.join(here, "out", f"t20-{wire}.jsonl"))))
                    if r["action"] is None]
            by_request = {r["i"]: r["prompt"] for r in map(json.loads, open(counted))}
            turns.append({"wire": wire, "points": [{"turn": k, "requests": n, "prompt": require_number(by_request.get(n), f"{counted} request {n} prompt tokens")}
                                                   for k, n in enumerate(ends, 1)]})
        out["vsenglish"] = {"tokens": tokens, "turns": turns, "turn2": [c for c in cells if c["scenario"] == "batch2"]}

    return out


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("results", type=Path)
    args = parser.parse_args(argv)
    try:
        result = build_report_data(json.loads(args.results.read_text()), args.results)
    except (ValueError, OSError) as error:
        parser.error(str(error))
    json.dump(result, sys.stdout, indent=1, allow_nan=False)


if __name__ == "__main__":
    main()
