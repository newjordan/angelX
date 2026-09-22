#!/usr/bin/env python3
"""Build website/js/bench-data.js from the polyglot 136-task benchmark results."""

from __future__ import annotations

import json
import statistics
import os
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parent
RESULTS_FILE = Path(os.environ.get("BENCH_RESULTS_FILE", REPO_ROOT / "benchmarks" / "results-polyglot-full.json"))
SITE_JS = Path(os.environ.get("BENCH_SITE_JS", REPO_ROOT / "website" / "js" / "bench-data.js"))

HARNESS_ORDER = ["angelx", "omp", "opencode"]
HARNESS_LABEL = {"angelx": "angelX", "omp": "omp", "opencode": "OpenCode"}
MODEL_ORDER = ["deepseek", "glm"]
MODEL_LABEL = {
    "deepseek": {"name": "DeepSeek V4.1 Flash", "id": "deepseek-flash", "thinking": "off"},
    "glm": {"name": "GLM-5.3-Flash", "id": "glm-5.3-flash", "thinking": "low (minimum; off is rejected by the provider)"},
}


def quantiles(values: list[float]) -> dict:
    if not values:
        return {"min": 0, "q1": 0, "median": 0, "q3": 0, "max": 0, "mean": 0, "outliers": []}
    v = sorted(values)
    if len(v) >= 4:
        q1, med, q3 = statistics.quantiles(v, n=4, method="inclusive")
    else:
        q1 = med = q3 = statistics.median(v)
    iqr = q3 - q1
    lo_fence, hi_fence = q1 - 1.5 * iqr, q3 + 1.5 * iqr
    inside = [x for x in v if lo_fence <= x <= hi_fence]
    return {
        "min": min(inside) if inside else min(v),
        "q1": q1,
        "median": med,
        "q3": q3,
        "max": max(inside) if inside else max(v),
        "mean": statistics.mean(v),
        "outliers": [x for x in v if x < lo_fence or x > hi_fence],
    }


def main() -> None:
    if not RESULTS_FILE.is_file():
        raise SystemExit(f"Results file not found: {RESULTS_FILE}")

    with open(RESULTS_FILE) as f:
        data = json.load(f)
    rows = data.get("rows", [])

    cells = []
    for model in MODEL_ORDER:
        for harness in HARNESS_ORDER:
            rs = [r for r in rows if r["model"] == model and r["harness"] == harness]
            if not rs:
                continue
            rs.sort(key=lambda r: (r.get("run", ""), r.get("task_index", 0)))
            runs = sorted({r.get("run", "") for r in rs})
            attempts = [
                {
                    "task": r["task"],
                    "run": (runs.index(r["run"]) + 1) if runs else 1,
                    "solved": r["reward"] == 1,
                    "wall_s": round(r["agent_wall_s"], 2) if r.get("agent_wall_s") is not None else 0.0,
                    "calls": r.get("calls", 0),
                    "uncached_in": r.get("prompt_tokens", 0),
                    "cached_in": r.get("cached_tokens", 0),
                    "out": r.get("completion_tokens", 0),
                    "reasoning": r.get("reasoning_tokens", 0),
                }
                for r in rs
            ]
            n = len(attempts)
            if n == 0:
                continue

            solved_count = sum(1 for a in attempts if a["solved"])
            walls = [a["wall_s"] for a in attempts]
            total_uncached = sum(a["uncached_in"] for a in attempts)
            total_cached = sum(a["cached_in"] for a in attempts)
            total_in = total_uncached + total_cached
            total_out = sum(a["out"] for a in attempts)
            total_reasoning = sum(a["reasoning"] for a in attempts)
            calls_mean = sum(a["calls"] for a in attempts) / n
            cache_hit = total_cached / total_in if total_in > 0 else 0.0

            cells.append(
                {
                    "model": model,
                    "harness": harness,
                    "harness_label": HARNESS_LABEL[harness],
                    "attempts": attempts,
                    "summary": {
                        "attempts": n,
                        "solved": solved_count,
                        "wall": quantiles(walls),
                        "calls_mean": calls_mean,
                        "uncached_in_per_task": total_uncached / n,
                        "cached_in_per_task": total_cached / n,
                        "in_per_task": total_in / n,
                        "out_per_task": total_out / n,
                        "reasoning_per_task": total_reasoning / n,
                        "cache_hit": cache_hit,
                        "call_errors": sum(len(r.get("call_errors", [])) for r in rs),
                        "trace_errors": sum(len(r.get("trace_errors", [])) for r in rs),
                        "not_isolated": 0,
                    },
                }
            )

    # Get angel build info
    angel_bin = Path(os.environ.get("BENCH_ANGEL_BIN", REPO_ROOT / "cockpit" / "target" / "release" / "angel"))
    build_info = json.loads(subprocess.check_output([str(angel_bin), "--build-info", "--json"]))

    commit = subprocess.check_output(["git", "rev-parse", "--short", "HEAD"], cwd=REPO_ROOT, text=True).strip()

    data = {
        "schema": "angelx-bench-site/v1",
        "suite": "polyglot-v1 · 136 repository-repair tasks (JS, Python, Rust, C++)",
        "models": MODEL_LABEL,
        "harness_order": HARNESS_ORDER,
        "angelx": {
            "commit": commit,
            "cockpit_source_sha256": build_info.get("cockpit_source_sha256", ""),
            "executable_sha256": build_info.get("executable_sha256", ""),
        },
        "peers": {"omp": "oh-my-pi 18.2.4", "opencode": "opencode 1.18.31"},
        "cells": cells,
    }

    out_content = "window.BENCH = " + json.dumps(data, separators=(",", ":")) + ";\n"
    SITE_JS.write_text(out_content)
    print(f"Successfully generated {SITE_JS} ({len(out_content):,} bytes, {len(cells)} cells).")


if __name__ == "__main__":
    main()
