#!/usr/bin/env python3
"""Compare Polyglot attempts across proxy, Angel and Codex seats offline.

--audit-json retains body-free attempt provenance and measurement coverage.
--json writes the legacy chart arrays only for complete, unambiguous catalogs
with observed scores, agent durations and input tokens. Input includes cache.
"""
from __future__ import annotations

import argparse
from collections import Counter
import importlib.util
import json
from pathlib import Path
import statistics


RR = Path("/home/frosty40/angel_tests/angelX-bench/polyglot-20260921")
spec = importlib.util.spec_from_file_location(
    "polyglot_history", Path(__file__).resolve().parents[1] / "analyze-polyglot-history.py")
history = importlib.util.module_from_spec(spec)
spec.loader.exec_module(history)
TOKEN_FIELDS = ("input", "cached_input", "output", "reasoning")


def usage_of(trace):
    """Separate complete per-attempt fields from aggregated reported subtotals."""
    info = trace.get("info") or {}
    if "codex_usage" in info:
        usage = info.get("codex_usage") or {}
        turns = usage.get("turns")
        valid_turns = isinstance(turns, int) and not isinstance(turns, bool) and turns > 0
        reports = usage.get("reported_attempts") or {}
        coherent = not (usage.get("untracked_sources") or usage.get("overflowed")
                        or usage.get("inconsistent_attempts"))
        fields, subtotals, coverage = {}, {}, {}
        for key, wire in (("input", "input_tokens"), ("cached_input", "cached_input_tokens"),
                          ("output", "output_tokens"), ("reasoning", "reasoning_output_tokens")):
            value = history.number(usage.get(wire))
            value = value if value is not None and value >= 0 else None
            # The adapter sums turn.completed counters. One turn proves a
            # present field; multiple turns need explicit per-field coverage.
            reported = reports.get(wire, 1 if turns == 1 and valid_turns and value is not None else None)
            reported = reported if isinstance(reported, int) and not isinstance(reported, bool) and reported >= 0 else None
            complete = valid_turns and reported == turns and coherent and value is not None
            fields[key] = value if complete else None
            subtotals[key] = value
            coverage[key] = {"reported": reported, "attempts": turns if valid_turns else None,
                             "complete": complete}
        source = "codex"
    else:
        calls = trace.get("calls") or []
        angel = (info.get("angel") or {}).get("usage") or {}
        fields, subtotals, coverage = history.usage_fields(calls, angel)
        source = "proxy" if calls else "angel" if angel else "none"
    return {"source": source, "fields": {key: fields[key] for key in TOKEN_FIELDS},
            "reported_subtotals": {key: subtotals[key] for key in TOKEN_FIELDS},
            "metric_coverage": {key: coverage[key] for key in TOKEN_FIELDS}}


def task_name(task):
    name = ((task or {}).get("data") or {}).get("name")
    return name if isinstance(name, str) and name else None


def load(run):
    """Keep all attempts, including repeats and records without any trace."""
    rows, empty = [], []
    with (Path(run) / "traces.jsonl").open() as stream:
        for line_no, line in enumerate(stream, 1):
            if not line.strip():
                continue
            record = json.loads(line)
            name = task_name(record.get("task"))
            traces = record.get("traces") or []
            if not traces:
                empty.append({"line": line_no, "task": name})
            for attempt, trace in enumerate(traces, 1):
                trace_name = task_name(trace.get("task"))
                conflict = name is not None and trace_name is not None and name != trace_name
                score = history.number(((trace.get("rewards") or {}).get("technical_outcome") or {}).get("score"))
                rows.append({"task": None if conflict else name or trace_name,
                             "identity_conflict": {"record": name, "trace": trace_name} if conflict else None,
                             "line": line_no, "attempt_index": attempt, "score": score,
                             "solved": score == 1,
                             "agent_s": history.elapsed((trace.get("timing") or {}).get("agent") or {}),
                             "usage": usage_of(trace)})
    return {"rows": rows, "records_without_traces": empty}


def aggregate(cell, order):
    rows = cell["rows"]
    _, duplicates, unnamed = history.index_rows(rows)
    actual = {row["task"] for row in rows if row["task"] is not None}
    expected = set(order)
    metrics = {}
    for key in ("agent_s", *TOKEN_FIELDS):
        values = [row["agent_s"] if key == "agent_s" else row["usage"]["fields"][key] for row in rows]
        known = [value for value in values if value is not None]
        subtotals = known if key == "agent_s" else [row["usage"]["reported_subtotals"][key]
                                                   for row in rows if row["usage"]["reported_subtotals"][key] is not None]
        metrics[key] = {"reported": len(known), "attempts": len(rows),
                        "total": sum(known) if known else None,
                        "median": statistics.median(known) if known else None,
                        "reported_subtotal": sum(subtotals) if subtotals else None,
                        "subtotal_attempts": len(subtotals)}
    cache_rows = [row for row in rows if all(row["usage"]["fields"][key] is not None
                                            for key in ("input", "cached_input"))]
    valid_cache = [row for row in cache_rows if row["usage"]["fields"]["cached_input"] <= row["usage"]["fields"]["input"]]
    cache_input = sum(row["usage"]["fields"]["input"] for row in valid_cache)
    return {"attempts": len(rows), "unique_tasks": len(actual), "duplicate_tasks": duplicates,
            "unnamed_attempts": unnamed, "identity_conflicts": sum(bool(row["identity_conflict"]) for row in rows),
            "solved": sum(row["solved"] for row in rows), "unscored": sum(row["score"] is None for row in rows),
            "missing_tasks": sorted(expected - actual), "unexpected_tasks": sorted(actual - expected),
            "complete_catalog": actual == expected and len(rows) == len(order) and not unnamed
                                and not cell["records_without_traces"],
            "usage_complete_attempts": sum(all(row["usage"]["fields"][key] is not None
                                               for key in ("input", "output")) for row in rows),
            "usage_sources": dict(sorted(Counter(row["usage"]["source"] for row in rows).items())),
            "cache_ratio": sum(row["usage"]["fields"]["cached_input"] for row in valid_cache) / cache_input if cache_input else None,
            "cache_reported_attempts": len(valid_cache), "cache_invalid_attempts": len(cache_rows) - len(valid_cache),
            "metrics": metrics}


def chart_series(cell, order):
    """Reject unknowns: charts/tokens-time.html adds row[2]/60 and row[3].

    JavaScript coerces null to zero in both expressions, so null-filled arrays
    would still draw a misleading cumulative curve in that existing renderer.
    """
    summary = cell["summary"]
    if not summary["complete_catalog"]:
        raise ValueError("chart export requires exactly one named attempt per catalog task and no empty records")
    if summary["unscored"] or any(summary["metrics"][key]["reported"] != summary["attempts"]
                                  for key in ("agent_s", "input")):
        raise ValueError("chart export requires observed scores, valid agent durations and complete input token coverage")
    rows, _, _ = history.index_rows(cell["rows"])
    return [[name, rows[name]["solved"], round(rows[name]["agent_s"], 2), rows[name]["usage"]["fields"]["input"]]
            for name in order]


def display_metric(metric, scale=1, suffix="", median=False):
    value = metric["median" if median else "total"]
    text = "—" if value is None else f"{value / scale:.1f}{suffix}"
    if metric["reported"] != metric["attempts"]:
        text += f" ({metric['reported']}/{metric['attempts']})"
    return text


def markdown(cells):
    lines = ["Totals include every recorded attempt. Parentheses show measurement coverage when incomplete.", "",
             "| run | solved attempts | agent time | median / attempt | input tokens | cache hit | output | reasoning | usage |",
             "|---|---:|---:|---:|---:|---:|---:|---:|---|"]
    notes = []
    for label, cell in cells.items():
        s = cell["summary"]
        m = s["metrics"]
        cache = "—" if s["cache_ratio"] is None else f"{100 * s['cache_ratio']:.0f}%"
        cache += f" ({s['cache_reported_attempts']}/{s['attempts']})" if s["cache_reported_attempts"] != s["attempts"] else ""
        lines.append(f"| {label} | {s['solved']}/{s['attempts']} | {display_metric(m['agent_s'], 60, ' min')} | "
                     f"{display_metric(m['agent_s'], suffix=' s', median=True)} | {display_metric(m['input'], 1e6, ' M')} | "
                     f"{cache} | {display_metric(m['output'], 1e3, ' k')} | {display_metric(m['reasoning'], 1e3, ' k')} | "
                     f"{'+'.join(s['usage_sources']) or 'none'} {s['usage_complete_attempts']}/{s['attempts']} |")
        notes.append(f"{label}: {s['unique_tasks']} unique tasks; {len(s['duplicate_tasks'])} repeated task identities; "
                     f"{s['unnamed_attempts']} unnamed attempts; {s['unscored']} unscored; "
                     f"{len(s['missing_tasks'])} missing / {len(s['unexpected_tasks'])} unexpected catalog tasks; "
                     f"{len(cell['records_without_traces'])} records without traces; {s['cache_invalid_attempts']} invalid cache measurements.")
        if s["duplicate_tasks"]:
            notes.append(f"Repeated {label} tasks: " + ", ".join(f"{name} ({count})" for name, count in s["duplicate_tasks"].items()))
        failed = sorted({row["task"] for row in cell["rows"] if row["task"] and row["score"] is not None and not row["solved"]})
        if failed:
            notes.append(f"{label} unsolved attempts: {', '.join(failed)}")
    return "\n".join(lines) + "\n\n" + "\n\n".join(notes)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("runs", nargs="+", metavar="LABEL=RUN_DIR")
    parser.add_argument("--catalog", type=Path, default=RR / "tasks-polyglot-v1.json")
    parser.add_argument("--json", type=Path, help="legacy chart arrays; requires complete catalog and chart measurements")
    parser.add_argument("--audit-json", type=Path, help="body-free attempt measurements and coverage")
    args = parser.parse_intermixed_args(argv)
    order = [task.get("name") for task in json.loads(args.catalog.read_text())]
    if not order or any(not isinstance(name, str) or not name for name in order) or len(set(order)) != len(order):
        parser.error("catalog must contain unique, nonempty task names")
    cells = {}
    for arg in args.runs:
        if "=" not in arg:
            parser.error("runs must have the form LABEL=RUN_DIR")
        label, run = arg.split("=", 1)
        if not label or not run or label in cells:
            parser.error("run labels must be nonempty and unique, with a run directory")
        cell = load(run)
        cell["summary"] = aggregate(cell, order)
        cells[label] = cell
    print(markdown(cells))
    if args.audit_json:
        args.audit_json.write_text(json.dumps({"catalog": order, "runs": cells}, indent=2, allow_nan=False) + "\n")
    if args.json:
        try:
            series = {label: chart_series(cell, order) for label, cell in cells.items()}
        except ValueError as error:
            parser.error(str(error))
        args.json.write_text(json.dumps(series, allow_nan=False) + "\n")


if __name__ == "__main__":
    main()
