#!/usr/bin/env python3
"""README-style polyglot table for the Luna cell.

Codex posts to chatgpt.com directly, so proxy call usage is usually empty.
Token and cache numbers come from angel.usage on each trace.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import statistics
from pathlib import Path


spec = importlib.util.spec_from_file_location(
    "polyglot_history", Path(__file__).with_name("analyze-polyglot-history.py"))
history = importlib.util.module_from_spec(spec)
spec.loader.exec_module(history)
TOKEN_FIELDS = {"input_tokens": "input", "cached_tokens": "cached_input",
                "output_tokens": "output", "reasoning_tokens": "reasoning"}


def newest() -> Path:
    root = Path("/home/frosty40/angel_tests/angelX-bench/polyglot-20260921/runs/luna/angelx")
    runs = [p for p in root.glob("seed0-*") if (p / "traces.jsonl").is_file()]
    if not runs:
        raise SystemExit(f"no luna runs under {root}")
    return max(runs, key=lambda p: p.stat().st_mtime)


def num(value):
    value = history.number(value)
    return value if value is not None and value >= 0 else None


def load_rows(run: Path, *, prefer_proxy_metadata=False):
    rows = []
    path = run / "traces.jsonl"
    if not path.is_file():
        return rows
    with path.open() as stream:
        for line_no, line in enumerate(stream, 1):
            if not line.strip():
                continue
            rec = json.loads(line)
            task = (rec.get("task") or {}).get("data") or {}
            name = task.get("name")
            name = name if isinstance(name, str) and name else None
            for attempt, trace in enumerate(rec.get("traces") or [], 1):
                metrics = trace.get("metrics") or {}
                calls = trace.get("calls") or []
                first_call = calls[0] if calls else {}
                info = trace.get("info") or {}
                angel = info.get("angel") or {}
                timed_out = (info.get("agent_exit") or {}).get("timed_out") is True
                usage = angel.get("usage") or {}
                fields, subtotals, coverage = history.usage_fields(calls, usage)
                score = history.number(((trace.get("rewards") or {}).get("technical_outcome") or {}).get("score"))
                # Keep the table's recorded task wall metric when observed.
                # A missing metric can use the evaluator's validated clock;
                # an explicitly invalid metric remains unknown.
                wall = history.milliseconds(metrics.get("wall_ms")) if "wall_ms" in metrics else history.elapsed(
                    (trace.get("timing") or {}).get("agent") or {})
                wall_source = "task-metric" if "wall_ms" in metrics else "agent-clock"
                hops = num(metrics.get("hops")) if "hops" in metrics else len(calls) if calls else num(angel.get("hops"))
                # A killed adapter never emitted task metrics: the evaluator
                # leaves zero placeholders despite a measured timeout/calls.
                if timed_out and wall == 0:
                    wall = history.elapsed((trace.get("timing") or {}).get("agent") or {})
                    wall_source = "agent-clock-timeout"
                if timed_out and hops == 0 and calls:
                    hops = len(calls)
                rows.append({"idx": task.get("idx"), "task": name, "lang": (name or "?").split("-")[0],
                             "line": line_no, "attempt_index": attempt, "score": score, "solved": score == 1,
                             "wall_s": wall, "wall_source": wall_source,
                             "hops": hops, **{key: fields[source] for key, source in TOKEN_FIELDS.items()},
                             "usage_reported_subtotals": {key: subtotals[source] for key, source in TOKEN_FIELDS.items()},
                             "usage_metric_coverage": {key: coverage[source] for key, source in TOKEN_FIELDS.items()},
                             "core": usage.get("core_complete") is True,
                             "model": first_call.get("model") if prefer_proxy_metadata else angel.get("model") or first_call.get("model"),
                             "effort": (first_call.get("sampling") or {}).get("reasoning_effort") if prefer_proxy_metadata else angel.get("reasoning_effort"),
                             "stop": angel.get("stop_reason") or angel.get("status")})
    return rows


def aggregate(rows):
    metrics = {}
    for key in ("wall_s", "hops", *TOKEN_FIELDS):
        known = [row[key] for row in rows if row[key] is not None]
        reported = [row["usage_reported_subtotals"][key] for row in rows
                    if row["usage_reported_subtotals"][key] is not None] if key in TOKEN_FIELDS else known
        metrics[key] = {"reported": len(known), "attempts": len(rows),
                        "total": sum(known) if known else None,
                        "median": statistics.median(known) if known else None,
                        "mean": statistics.mean(known) if known else None,
                        "reported_subtotal": sum(reported) if reported else None}
    _, duplicate, unnamed = history.index_rows(rows)
    measured_cache = [row for row in rows if row["input_tokens"] is not None and row["cached_tokens"] is not None]
    valid_cache = [row for row in measured_cache if row["cached_tokens"] <= row["input_tokens"]]
    inputs = sum(row["input_tokens"] for row in valid_cache)
    return {"attempts": len(rows), "unique_tasks": len({row["task"] for row in rows if row["task"]}),
            "duplicate_tasks": duplicate, "unnamed_attempts": unnamed,
            "solved": sum(row["solved"] for row in rows), "unscored": sum(row["score"] is None for row in rows),
            "usage_complete_attempts": sum(row["input_tokens"] is not None and row["output_tokens"] is not None for row in rows),
            "cache_ratio": sum(row["cached_tokens"] for row in valid_cache) / inputs if inputs else None,
            "cache_reported_attempts": len(valid_cache), "cache_invalid_attempts": len(measured_cache) - len(valid_cache),
            "metrics": metrics}


def display(metric, stat="total", digits=0, suffix=""):
    value = metric[stat]
    text = "—" if value is None else f"{value:.{digits}f}{suffix}"
    return text + (f" ({metric['reported']}/{metric['attempts']})" if metric["reported"] != metric["attempts"] else "")


def print_table(run, rows):
    summary = aggregate(rows)
    print(f"run={run}")
    print(f"attempts={summary['attempts']} scored={summary['attempts'] - summary['unscored']} "
          f"unscored={summary['unscored']} unique_tasks={summary['unique_tasks']} "
          f"repeated_tasks={len(summary['duplicate_tasks'])} unnamed_attempts={summary['unnamed_attempts']}")
    if not rows:
        return
    metrics = summary["metrics"]
    hit = "—" if summary["cache_ratio"] is None else f"{100 * summary['cache_ratio']:.0f}%"
    if summary["cache_reported_attempts"] != len(rows):
        hit += f" ({summary['cache_reported_attempts']}/{len(rows)})"
    print(f"model={rows[0]['model']} effort={rows[0]['effort']} usage_tasks={summary['usage_complete_attempts']}/{len(rows)} "
          f"invalid_cache_attempts={summary['cache_invalid_attempts']}")
    print("\nToken totals use complete fields; parentheses show measurement coverage when incomplete.\n")
    print("| model | harness | solved attempts | wall (median) | hops / attempt | input tokens | cache hit | output tokens | reasoning tokens |")
    print("|---|---|---:|---:|---:|---:|---:|---:|---:|")
    print(f"| {rows[0]['model']}, reasoning {rows[0]['effort']} | angelX | {summary['solved']} / {len(rows)} | "
          f"{display(metrics['wall_s'], 'median', 1, ' s')} | {display(metrics['hops'], 'mean', 1)} | "
          f"{display(metrics['input_tokens'])} | {hit} | {display(metrics['output_tokens'])} | {display(metrics['reasoning_tokens'])} |")
    print()
    print("| language | solved | n | median wall |")
    print("|---|---:|---:|---:|")
    by = {}
    for r in rows:
        by.setdefault(r["lang"], []).append(r)
    for lang in ("js", "py", "rust", "cpp"):
        group = by.get(lang) or []
        if not group:
            continue
        print(f"| {lang} | {sum(1 for r in group if r['solved'])} | {len(group)} | "
              f"{display(aggregate(group)['metrics']['wall_s'], 'median', 1, ' s')} |")
    failed = [r for r in rows if not r["solved"]]
    if failed:
        print()
        print("unsolved or unscored attempts:")
        for r in failed:
            wall = "—" if r["wall_s"] is None else f"{r['wall_s']:.1f}s"
            print(f"  {r['idx']} {r['task']} score={r['score']} wall={wall} stop={r['stop']}")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("run", type=Path, nargs="?", help="run directory; defaults to newest Luna seed0")
    args = parser.parse_args(argv)
    run = args.run if args.run is not None else newest()
    print_table(run, load_rows(run))


if __name__ == "__main__":
    main()
