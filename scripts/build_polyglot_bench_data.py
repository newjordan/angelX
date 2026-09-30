#!/usr/bin/env python3
"""Export checked Polyglot results using recorded build metadata.

Metadata may be embedded as ``angelx`` and ``peers``, or supplied with
--provenance in a JSON object containing those fields and ``results_sha256``.
The latter binds the metadata to the exact results file, never a live binary.
--audit-json reports missing measurements without writing chart data: the site
renderer adds attempt token fields directly (website/js/bench.js:250), so null
would silently contribute zero there.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import re
import statistics
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parent
spec = importlib.util.spec_from_file_location("site_export_history", HERE / "analyze-polyglot-history.py")
history = importlib.util.module_from_spec(spec)
spec.loader.exec_module(history)

METRICS = ("agent_wall_s", "calls", "prompt_tokens", "cached_tokens", "completion_tokens", "reasoning_tokens")

HARNESS_ORDER = ["angelx", "omp", "opencode"]
HARNESS_LABEL = {"angelx": "angelX", "omp": "omp", "opencode": "OpenCode"}
MODEL_ORDER = ["deepseek", "glm"]
MODEL_LABEL = {
    "deepseek": {"name": "DeepSeek V4.1 Flash", "id": "deepseek-flash", "thinking": "off"},
    "glm": {"name": "GLM-5.3-Flash", "id": "glm-5.3-flash", "thinking": "low (minimum; off is rejected by the provider)"},
}


def quantiles(values: list[float]) -> dict:
    if not values:
        raise ValueError("wall quantiles require observed attempts")
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


def nonnegative(value):
    value = history.number(value)
    return value if value is not None and value >= 0 else None


def recorded_metadata(data, provenance=None):
    """Validate recorded identities; explicit metadata cannot replace a conflict."""
    metadata, issues = {}, []
    provenance = provenance if provenance is not None else data
    if not isinstance(provenance, dict):
        return {}, ["recorded provenance must be an object"]
    for name in ("angelx", "peers"):
        value = provenance.get(name)
        if provenance is not data and name in data and data[name] != value:
            issues.append(f"recorded {name} metadata conflicts with results")
        metadata[name] = value
    angel = metadata["angelx"]
    if not isinstance(angel, dict):
        metadata["angelx"] = None
        issues.append("missing recorded angelx build metadata")
    else:
        commit = angel.get("commit")
        if not isinstance(commit, str) or re.fullmatch(r"[0-9a-fA-F]{7,40}", commit) is None:
            issues.append("recorded angelx.commit must be a 7-40 character Git hash")
        for field in ("cockpit_source_sha256", "executable_sha256"):
            value = angel.get(field)
            if not isinstance(value, str) or re.fullmatch(r"[0-9a-fA-F]{64}", value) is None:
                issues.append(f"recorded angelx.{field} must be a SHA-256 digest")
        metadata["angelx"] = {key: angel.get(key) if isinstance(angel.get(key), str) else None for key in
                              ("commit", "cockpit_source_sha256", "executable_sha256")}
    peers = metadata["peers"]
    if not isinstance(peers, dict) or any(not isinstance(peers.get(key), str) or not peers[key].strip()
                                           for key in ("omp", "opencode")):
        metadata["peers"] = ({key: peers.get(key) if isinstance(peers.get(key), str) else None
                              for key in ("omp", "opencode")} if isinstance(peers, dict) else None)
        issues.append("missing recorded omp/opencode peer versions")
    else:
        metadata["peers"] = {key: peers[key] for key in ("omp", "opencode")}
    return metadata, issues


def audit_data(data, provenance=None):
    if not isinstance(data, dict):
        raise ValueError("results must be an object")
    raw_rows = data.get("rows")
    if not isinstance(raw_rows, list):
        raise ValueError("results.rows must be a list")
    metadata, issues = recorded_metadata(data, provenance)
    if not raw_rows:
        issues.append("results contain no observed attempts")
    rows = []
    for index, raw in enumerate(raw_rows, 1):
        row_issues = []
        raw = raw if isinstance(raw, dict) else {}
        row = {"row": index}
        for field in ("model", "harness", "task", "run"):
            value = raw.get(field)
            row[field] = value if isinstance(value, str) and value.strip() else None
            if row[field] is None:
                row_issues.append(f"missing {field}")
        if row["model"] is not None and row["model"] not in MODEL_ORDER:
            row_issues.append(f"unsupported model {row['model']!r}")
        if row["harness"] is not None and row["harness"] not in HARNESS_ORDER:
            row_issues.append(f"unsupported harness {row['harness']!r}")
        reward = history.number(raw.get("reward"))
        row["scored"] = raw.get("scored") if isinstance(raw.get("scored"), bool) else None
        row["reward"] = reward if reward in (0, 1) and raw.get("scored") is True else None
        if row["reward"] is None:
            row_issues.append("missing observed binary reward/scored flag")
        elif "solved" in raw and (not isinstance(raw["solved"], bool) or raw["solved"] != (reward == 1)):
            row_issues.append("solved flag conflicts with reward")
        for field in METRICS:
            value = nonnegative(raw.get(field))
            if field != "agent_wall_s" and value is not None and value != int(value):
                value = None
            row[field] = value
            if value is None:
                row_issues.append(f"missing/invalid {field}")
        for field in ("call_errors", "trace_errors"):
            value = raw.get(field)
            row[field] = len(value) if isinstance(value, list) else None
            if row[field] is None:
                row_issues.append(f"missing/invalid {field} list")
        isolation = raw.get("isolation")
        row["isolation"] = isolation if isinstance(isolation, str) else None
        row["not_isolated"] = int(isolation != "fresh-home") if isinstance(isolation, str) and isolation.strip() else None
        if row["not_isolated"] is None:
            row_issues.append("missing isolation measurement")
        row["issues"] = row_issues
        issues.extend(f"row {index}: {issue}" for issue in row_issues)
        rows.append(row)
    fields = (*METRICS, "reward", "call_errors", "trace_errors", "not_isolated")
    totals, subtotals, coverage = history.receipt_fields({field: [row[field] for row in rows] for field in fields})
    return {"schema": "angelx-bench-site-audit/v1", "exportable": not issues,
            "provenance": metadata, "attempts": len(rows), "metrics": totals,
            "reported_subtotals": subtotals, "metric_coverage": coverage,
            "issues": issues, "rows": rows}


def build_data(data, provenance=None):
    audit = audit_data(data, provenance)
    if not audit["exportable"]:
        raise ValueError("unsafe chart export: " + "; ".join(audit["issues"]))
    rows = audit["rows"]

    cells = []
    for model in MODEL_ORDER:
        for harness in HARNESS_ORDER:
            rs = [r for r in rows if r["model"] == model and r["harness"] == harness]
            if not rs:
                continue
            # Keep export order within a run: it is execution order, and a
            # resumed (pickup) segment restarts task_index at 0.
            rs.sort(key=lambda r: r["run"])
            runs = sorted({r["run"] for r in rs})
            attempts = [
                {
                    "task": r["task"],
                    "run": runs.index(r["run"]) + 1,
                    "solved": r["reward"] == 1,
                    "wall_s": round(r["agent_wall_s"], 2),
                    "calls": r["calls"],
                    "uncached_in": r["prompt_tokens"],
                    "cached_in": r["cached_tokens"],
                    "out": r["completion_tokens"],
                    "reasoning": r["reasoning_tokens"],
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
                        "call_errors": sum(r["call_errors"] for r in rs),
                        "trace_errors": sum(r["trace_errors"] for r in rs),
                        "not_isolated": sum(r["not_isolated"] for r in rs),
                    },
                }
            )

    return {
        "schema": "angelx-bench-site/v1",
        "suite": "polyglot-v1 · 136 repository-repair tasks (JS, Python, Rust, C++)",
        "models": MODEL_LABEL,
        "harness_order": HARNESS_ORDER,
        **audit["provenance"],
        "cells": cells,
    }
def load_inputs(results, provenance_path=None):
    payload = Path(results).read_bytes()
    data = json.loads(payload)
    provenance = None
    if provenance_path is not None:
        provenance = json.loads(Path(provenance_path).read_bytes())
        if not isinstance(provenance, dict) or provenance.get("results_sha256") != hashlib.sha256(payload).hexdigest():
            raise ValueError("recorded provenance results_sha256 does not match the results file")
    return data, provenance


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--results", type=Path, default=Path(os.environ.get(
        "BENCH_RESULTS_FILE", REPO_ROOT / "benchmarks/results-polyglot-full.json")))
    parser.add_argument("--output", type=Path, default=Path(os.environ.get(
        "BENCH_SITE_JS", REPO_ROOT / "website/js/bench-data.js")))
    parser.add_argument("--provenance", type=Path, help="recorded angelx/peers metadata with results_sha256")
    parser.add_argument("--audit-json", action="store_true", help="print audit JSON; do not write chart data")
    args = parser.parse_args(argv)
    try:
        data, provenance = load_inputs(args.results, args.provenance)
        if args.audit_json:
            print(json.dumps(audit_data(data, provenance), indent=2, allow_nan=False))
            return
        exported = build_data(data, provenance)
        out_content = "window.BENCH = " + json.dumps(exported, separators=(",", ":"), allow_nan=False) + ";\n"
    except (OSError, ValueError) as error:
        parser.exit(1, f"{error}\n")
    args.output.write_text(out_content)
    print(f"Successfully generated {args.output} ({len(out_content):,} bytes, {len(exported['cells'])} cells).")


if __name__ == "__main__":
    main()
