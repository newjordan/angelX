#!/usr/bin/env python3
"""Summarize Polyglot cell traces without exposing prompts, answers or wire data.

Missing telemetry stays null. Sum/median fields state how many tasks reported
the metric. Two or more cells also produce per-task paired deltas against the
first cell; these are descriptive pilot results, not confidence claims.
"""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import math
from pathlib import Path
import statistics


def numeric(value):
    return (value if isinstance(value, (int, float)) and not isinstance(value, bool)
            and math.isfinite(value) else None)


def duration(value):
    start, end = numeric(value.get("start")), numeric(value.get("end"))
    return end - start if start is not None and end is not None and end >= start else None


def milliseconds(value):
    value = numeric(value)
    return value / 1000 if value is not None and value >= 0 else None


def task_counts(rows):
    return Counter(row["task"] for row in rows if isinstance(row.get("task"), str) and row["task"])


TOKEN_FIELDS = {"input_tokens": "input", "cached_input_tokens": "cache_read",
                "uncached_input_tokens": "uncached_input", "output_tokens": "output",
                "reasoning_tokens": "reasoning"}


def measured_usage(usage):
    values, subtotals, coverage = {}, {}, {}
    attempts = usage.get("attempts")
    reports = usage.get("reported_attempts") or {}
    valid_attempts = isinstance(attempts, int) and not isinstance(attempts, bool) and attempts > 0
    coherent = not (usage.get("untracked_sources") or usage.get("overflowed") or usage.get("inconsistent_attempts"))
    for key, field in TOKEN_FIELDS.items():
        value = numeric(usage.get(field))
        value = value if value is not None and value >= 0 else None
        count = reports.get(field)
        count = count if isinstance(count, int) and not isinstance(count, bool) and count >= 0 else None
        # Earlier envelopes have no per-field coverage. Preserve their reported
        # counters while keeping the absence of a completeness claim visible.
        complete = valid_attempts and count == attempts and coherent and value is not None
        values[key] = value if complete or ("attempts" not in usage and not reports and coherent) else None
        subtotals[key] = value
        coverage[key] = {"reported": count, "attempts": attempts if valid_attempts else None, "complete": complete}
    return values, subtotals, coverage


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
    token_fields, usage_subtotals, usage_coverage = measured_usage(usage)
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
        **token_fields,
        "usage_reported_subtotals": usage_subtotals, "usage_metric_coverage": usage_coverage,
        "usage_complete": (usage.get("core_complete") is True and token_fields["input_tokens"] is not None
                           and token_fields["output_tokens"] is not None),
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
    cache_reported = [row for row in rows if row["input_tokens"] is not None
                      and row["cached_input_tokens"] is not None]
    cached_rows = [row for row in cache_reported if 0 <= row["cached_input_tokens"] <= row["input_tokens"]]
    total_input = sum(row["input_tokens"] for row in cached_rows)
    counts = task_counts(rows)
    return {
        "tasks": len(rows), "scored": sum(row["score"] is not None for row in rows),
        "unique_tasks": len(counts), "unscored": sum(row["score"] is None for row in rows),
        "unnamed_attempts": len(rows) - sum(counts.values()),
        "duplicate_tasks": {task: count for task, count in sorted(counts.items()) if count > 1},
        "solved": sum(row["solved"] for row in rows),
        "usage_complete_tasks": sum(row["usage_complete"] for row in rows),
        "usage_partial_tasks_by_metric": {key: sum(row[key] is None and
            (row.get("usage_reported_subtotals") or {}).get(key) is not None for row in rows)
            for key in TOKEN_FIELDS},
        "cache_hit_ratio": (sum(row["cached_input_tokens"] for row in cached_rows) / total_input
                            if total_input else None),
        "cache_hit_reported_tasks": len(cached_rows),
        "cache_hit_invalid_tasks": len(cache_reported) - len(cached_rows), "metrics": stats,
    }


def load_cell(path: Path):
    traces = path / "run/traces.jsonl" if (path / "run").is_dir() else path / "traces.jsonl"
    rows, recorded_tasks, empty_records = [], set(), 0
    if traces.is_file():
        for line_no, line in enumerate(traces.read_text().splitlines(), 1):
            if line.strip():
                record = json.loads(line)
                name = ((record.get("task") or {}).get("data") or {}).get("name")
                if isinstance(name, str) and name:
                    recorded_tasks.add(name)
                attempts = record.get("traces") or []
                empty_records += not attempts
                for attempt, trace in enumerate(attempts, 1):
                    rows.append({**summarize_trace(record, trace), "line": line_no, "attempt": attempt})
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
    counts = task_counts(rows)
    expected = manifest.get("task_names")
    expected = expected if isinstance(expected, list) and expected else None
    if expected is not None and any(not isinstance(name, str) or not name for name in expected):
        raise ValueError(f"invalid task_names in {manifest_path}")
    attempts = manifest.get("attempts", 1)
    attempts = attempts if isinstance(attempts, int) and not isinstance(attempts, bool) and attempts > 0 else None
    expected_names = set(expected) if expected is not None else None
    coverage = {
        "trace_file_present": traces.is_file(), "recorded_tasks": len(recorded_tasks),
        "records_without_traces": empty_records, "traced_tasks": len(counts),
        "expected_attempts": len(expected) * attempts if expected is not None and attempts is not None else None,
        "missing_tasks": sorted(expected_names - counts.keys()) if expected_names is not None else None,
        "unexpected_tasks": sorted(counts.keys() - expected_names) if expected_names is not None else None,
        "attempt_count_mismatches": ({name: counts[name] for name in sorted(expected_names)
                                      if counts[name] != attempts}
                                     if expected_names is not None and attempts is not None else None),
        "cohort_complete": (len(expected_names) == len(expected) and set(counts) == expected_names
                            and all(counts[name] == attempts for name in expected_names)
                            and sum(counts.values()) == len(rows) and not empty_records
                            if expected_names is not None and attempts is not None else None),
    }
    return {
        "path": str(path), "expected_tasks": len(expected) if expected is not None else None,
        "manifest": contract,
        "summary": aggregate(rows), "coverage": coverage, "rows": rows,
    }


def paired(baseline, candidate):
    base_counts, cand_counts = task_counts(baseline["rows"]), task_counts(candidate["rows"])
    ambiguous = {task for task, count in (base_counts | cand_counts).items() if count > 1}
    base = {row["task"]: row for row in baseline["rows"] if isinstance(row.get("task"), str)
            and row["task"] in base_counts and row["task"] not in ambiguous}
    cand = {row["task"]: row for row in candidate["rows"] if isinstance(row.get("task"), str)
            and row["task"] in cand_counts and row["task"] not in ambiguous}
    shared = sorted(base.keys() & cand.keys())
    scored = [task for task in shared if base[task]["score"] is not None and cand[task]["score"] is not None]
    deltas = []
    for task in shared:
        b, c = base[task], cand[task]
        row = {"task": task, "baseline_solved": b["solved"], "candidate_solved": c["solved"],
               "baseline_score": b["score"], "candidate_score": c["score"]}
        for key in METRICS:
            row[key] = c[key] - b[key] if c[key] is not None and b[key] is not None else None
        deltas.append(row)
    keys = baseline["manifest"].keys() | candidate["manifest"].keys()
    mismatches = sorted(key for key in keys if baseline["manifest"].get(key) != candidate["manifest"].get(key))
    unknown_contract = sorted(key for key in keys if baseline["manifest"].get(key) is None or candidate["manifest"].get(key) is None)
    return {
        "baseline": baseline["path"], "candidate": candidate["path"],
        "contract_mismatches": mismatches, "shared_tasks": len(shared),
        "contract_unknown_fields": unknown_contract,
        "ambiguous_tasks": sorted(ambiguous), "jointly_scored_tasks": len(scored),
        "unscored_shared_tasks": sorted(set(shared) - set(scored)),
        "missing_from_candidate": sorted(base_counts.keys() - cand_counts.keys()),
        "missing_from_baseline": sorted(cand_counts.keys() - base_counts.keys()),
        "solve_gains": [task for task in scored if not base[task]["solved"] and cand[task]["solved"]],
        "solve_losses": [task for task in scored if base[task]["solved"] and not cand[task]["solved"]],
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
