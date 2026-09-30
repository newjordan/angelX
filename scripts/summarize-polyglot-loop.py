#!/usr/bin/env python3
"""Summarize Polyglot cell traces without exposing prompts, answers or wire data.

Missing telemetry stays null. Sum/median fields state how many tasks reported
the metric. Two or more cells also produce per-task paired deltas against the
first cell; these are descriptive pilot results, not confidence claims.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import statistics


def numeric(value):
    return value if isinstance(value, (int, float)) and not isinstance(value, bool) else None


def duration(value):
    start, end = numeric(value.get("start")), numeric(value.get("end"))
    return end - start if start is not None and end is not None else None


def milliseconds(value):
    value = numeric(value)
    return value / 1000 if value is not None else None


def summarize_trace(record, trace):
    info = trace.get("info") or {}
    angel = info.get("angel") or {}
    usage = angel.get("usage") or {}
    timing = angel.get("timing") or {}
    task = (record.get("task") or {}).get("data") or {}
    rewards = trace.get("rewards") or {}
    score = numeric((rewards.get("technical_outcome") or {}).get("score"))
    verifier = info.get("heldout_verification") or {}
    metrics = trace.get("metrics") or {}
    task_wall = duration((trace.get("timing") or {}).get("agent") or {})
    if task_wall is None:
        task_wall = milliseconds(metrics.get("wall_ms"))
    verifier_wall = milliseconds(verifier.get("verifier_elapsed_ms"))
    return {
        "task": task.get("name"), "score": score, "solved": score == 1,
        "task_wall_s": task_wall, "verifier_wall_s": verifier_wall,
        "model_s": milliseconds(timing.get("model_ms")),
        "tool_s": milliseconds(timing.get("tool_ms")),
        "startup_shutdown_s": milliseconds(timing.get("startup_shutdown_ms")),
        "tool_overhead_s": milliseconds(timing.get("tool_overhead_ms")),
        "serial_overhead_s": milliseconds(timing.get("serial_overhead_ms")),
        "model_calls": numeric(timing.get("model_calls")),
        "tool_calls": numeric(timing.get("tool_calls")),
        "hops": numeric(angel.get("hops")),
        "input_tokens": numeric(usage.get("input")),
        "cached_input_tokens": numeric(usage.get("cache_read")),
        "uncached_input_tokens": numeric(usage.get("uncached_input")),
        "output_tokens": numeric(usage.get("output")),
        "reasoning_tokens": numeric(usage.get("reasoning")),
        "usage_complete": usage.get("core_complete") is True,
        "model": angel.get("model"), "effort": angel.get("reasoning_effort"),
        "stop_reason": angel.get("stop_reason"),
        "exit_code": (info.get("agent_exit") or {}).get("exit_code"),
        "timed_out": (info.get("agent_exit") or {}).get("timed_out"),
        "integrity_pass": verifier.get("integrity_pass"),
        "trace_error_count": len(trace.get("errors") or []),
    }


METRICS = (
    "task_wall_s", "verifier_wall_s", "model_s", "tool_s", "startup_shutdown_s",
    "tool_overhead_s", "serial_overhead_s", "model_calls", "tool_calls", "hops",
    "input_tokens", "cached_input_tokens", "uncached_input_tokens", "output_tokens",
    "reasoning_tokens",
)


def aggregate(rows):
    stats = {}
    for key in METRICS:
        values = [row[key] for row in rows if row[key] is not None]
        stats[key] = {
            "reported": len(values), "sum": sum(values) if values else None,
            "median": statistics.median(values) if values else None,
        }
    cached_rows = [row for row in rows if row["input_tokens"] is not None
                   and row["cached_input_tokens"] is not None]
    total_input = sum(row["input_tokens"] for row in cached_rows)
    return {
        "tasks": len(rows), "scored": sum(row["score"] is not None for row in rows),
        "solved": sum(row["solved"] for row in rows),
        "usage_complete_tasks": sum(row["usage_complete"] for row in rows),
        "cache_hit_ratio": (sum(row["cached_input_tokens"] for row in cached_rows) / total_input
                            if total_input else None),
        "cache_hit_reported_tasks": len(cached_rows), "metrics": stats,
    }


def load_cell(path: Path):
    traces = path / "run/traces.jsonl" if (path / "run").is_dir() else path / "traces.jsonl"
    rows = []
    if traces.is_file():
        for line in traces.read_text().splitlines():
            if line.strip():
                record = json.loads(line)
                rows.extend(summarize_trace(record, trace) for trace in record.get("traces") or [])
    manifest_path = path / "manifest.json"
    manifest = json.loads(manifest_path.read_text()) if manifest_path.exists() else {}
    contract = {key: manifest.get(key) for key in (
        "selected_rows_sha256", "catalog_sha256", "grading_sha256", "grade_script_sha256",
        "model", "effort", "wall_secs", "max_hops", "max_tokens", "attempts", "concurrency",
        "evaluator_files_sha256", "harness_env",
    )}
    for key in ("adapter_files", "fixture_files", "gold_files"):
        value = manifest.get(key)
        contract[key + "_sha256"] = (
            hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()
            if value is not None else None
        )
    build = manifest.get("binary") or {}
    contract["build_profile"] = (build.get("toolchain") or {}).get("profile")
    contract["video_decode"] = build.get("video_decode")
    return {
        "path": str(path), "expected_tasks": len(manifest.get("task_names", [])) or None,
        "manifest": contract,
        "summary": aggregate(rows), "rows": rows,
    }


def paired(baseline, candidate):
    base = {row["task"]: row for row in baseline["rows"]}
    cand = {row["task"]: row for row in candidate["rows"]}
    shared = sorted(base.keys() & cand.keys())
    deltas = []
    for task in shared:
        b, c = base[task], cand[task]
        row = {"task": task, "baseline_solved": b["solved"], "candidate_solved": c["solved"]}
        for key in METRICS:
            row[key] = c[key] - b[key] if c[key] is not None and b[key] is not None else None
        deltas.append(row)
    mismatches = [key for key, value in baseline["manifest"].items()
                  if value is not None and candidate["manifest"].get(key) != value]
    return {
        "baseline": baseline["path"], "candidate": candidate["path"],
        "contract_mismatches": mismatches, "shared_tasks": len(shared),
        "missing_from_candidate": sorted(base.keys() - cand.keys()),
        "missing_from_baseline": sorted(cand.keys() - base.keys()),
        "solve_gains": [task for task in shared if not base[task]["solved"] and cand[task]["solved"]],
        "solve_losses": [task for task in shared if base[task]["solved"] and not cand[task]["solved"]],
        "deltas_candidate_minus_baseline": deltas,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cells", nargs="+", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    cells = [load_cell(path.resolve()) for path in args.cells]
    result = {"cells": cells, "comparisons": [paired(cells[0], cell) for cell in cells[1:]]}
    rendered = json.dumps(result, indent=2) + "\n"
    if args.output:
        args.output.write_text(rendered)
    else:
        print(rendered, end="")


if __name__ == "__main__":
    main()
