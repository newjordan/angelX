#!/usr/bin/env python3
"""report_data.py results.json > report-data.json — the report's aggregates:
medians and ranges per model / arm / build, and per-request series for the
long loops. Every figure traces to runs listed in results.json."""
import json
import os
import statistics
import sys

MODELS = [("glm-flash", "GLM-5.3 flash"), ("deepseek-flash", "DeepSeek flash"),
          ("muse", "Muse Spark 1.3 contributor"), ("grok", "Grok 4.7 low")]
ARMS = [("prebook", "Pre-book English"), ("bare", "Braille only"), ("intro", "Braille + introduction")]

data = json.load(open(sys.argv[1]))
runs = data["runs"]


def stats(values):
    values = [v for v in values if v is not None]
    if not values:
        return None
    return {"median": statistics.median(values), "min": min(values), "max": max(values), "n": len(values)}


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
        delivered = bool(rerun) or not (model == "muse" and arm == "intro")
        e1.append({"model": model, "arm": arm, "delivered": delivered,
                   "solved": sum(bool(r.get("solved")) for r in rs), "n": len(rs),
                   "fresh": stats([r["fresh"] for r in rs if r.get("solved")]),
                   "prompt": stats([r["prompt"] for r in rs if r.get("solved")]),
                   "turns": stats([r["requests"] for r in rs if r.get("solved")]),
                   "wall": stats([r.get("wall_s") for r in rs if r.get("solved")]),
                   "host": rs[0]["host"]})
out["e1"] = e1

# E2: provider-counted prompt tokens of each stub session's final request
lengths = {"2": 9, "16": 51, "33": 102, "70": 213}
e2 = {}
for row in data["e2"]:
    route = row["route"]
    arm, cycles = row["label"].split("-")[1], row["label"].split("-")[2]
    e2.setdefault(route, {}).setdefault(arm, {})[lengths[cycles]] = row["prompt_tokens"]
out["e2"] = [{"model": model, "points": [
    {"requests": n, **{arm: e2[model][arm].get(n) for arm in ("prebook", "bare", "intro")}}
    for n in sorted(lengths.values())]} for model, _ in MODELS if model in e2]

# E5: behind a real model, speed baseline against head (intro arm, two-bug task)
e5 = []
for model, _ in MODELS:
    for build in ("base", "head"):
        rs = cohort(f"e5-{model}-{build}-")
        timed = [r["timing"] for r in rs if r.get("timing") and r.get("solved")]
        if not timed:
            continue
        harness = [t["tool_overhead_ms"] + t["residual_ms"] + t["other_ms"] for t in timed]
        e5.append({"model": model, "build": build, "solved": sum(bool(r.get("solved")) for r in rs), "n": len(rs),
                   "wall_ms": stats([t["wall_ms"] for t in timed]),
                   "model_ms": stats([t["model_ms"] for t in timed]),
                   "tool_ms": stats([t["tool_ms"] for t in timed]),
                   "harness_ms": stats(harness),
                   "startup_ms": stats([t["startup_ms"] for t in timed]),
                   "host": rs[0]["host"]})
out["e5"] = e5

# The scripted 213-request session and time to first request (stub model)
def timing_stat(prefix, key):
    return stats([r["timing"][key] for r in cohort(prefix) if r.get("timing")])
out["stub"] = {build: {"wall_ms": timing_stat(f"{build}-", "wall_ms"),
                       "startup_ms": timing_stat(f"ttfr-{build}-", "startup_ms")} for build in ("base", "head")}

# X6 and the DeepSeek A/B
out["x6"] = {build: {"startup_ms": timing_stat(f"x6-{build}-", "startup_ms"),
                     "wall_ms": timing_stat(f"x6-{build}-", "wall_ms")} for build in ("h", "eng")}
dsab = []
for arm, name in (("base", "Before the fixes (Chat)"), ("chat", "Fixed (Chat)"), ("msgs", "Fixed (Messages API)")):
    rs = cohort(f"dsab-{arm}-")
    if rs:
        dsab.append({"arm": arm, "name": name, "solved": sum(bool(r.get("solved")) for r in rs), "n": len(rs),
                     "fresh": stats([r["fresh"] for r in rs]), "wall": stats([r.get("wall_s") for r in rs]),
                     "turns": stats([r["requests"] for r in rs]),
                     "startup_ms": stats([r["timing"]["startup_ms"] for r in rs if r.get("timing")])})
out["dsab"] = dsab

# Long DeepSeek loops: per-request prompt and cache, cumulative uncached tokens
loops = []
for label, name in (("e3-deepseek-flash-bare-1", "Before the fixes (Chat)"),
                    ("dsfinal-chat-1", "Fixed (Chat)"), ("dsfinal-msgs-1", "Fixed (Messages API)")):
    run = runs.get(label)
    if not run or not run.get("per_request"):
        continue
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
        follow.append({"name": name, "prompt": last["prompt"], "cached": last["cached"]})
out["followup"] = follow

# The decision test (decide/decide_table.py beside results.json)
decide = os.path.join(os.path.dirname(os.path.abspath(sys.argv[1])), "decide.json")
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
            tokens.append({"wire": wire, "points": [{"requests": r["i"], "prompt": r["prompt"]}
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
        turns.append({"wire": wire, "points": [{"turn": k, "requests": n, "prompt": by_request.get(n)}
                                               for k, n in enumerate(ends, 1)]})
    out["vsenglish"] = {"tokens": tokens, "turns": turns, "turn2": [c for c in cells if c["scenario"] == "batch2"]}

json.dump(out, sys.stdout, indent=1)
