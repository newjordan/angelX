#!/usr/bin/env python3
"""decide_table.py out/decide-*.jsonl > decide.json — per decision point, model
and arm: how often the next move was the route's action (with a Wilson 95%
interval), what the next moves were, and the request's tokens and time."""
import argparse
import collections
import importlib.util
import json
import math
from pathlib import Path
import statistics
import sys


spec = importlib.util.spec_from_file_location("decision_receipt_usage", Path(__file__).resolve().parents[1] / "receipt_usage.py")
usage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(usage)

SCENARIOS = ("batch", "batch2", "untested", "finish")
ARMS = ("prebook", "nocue", "bare", "every", "intro")
MODELS = ("glm-flash", "deepseek-flash", "muse", "grok")


def wilson(k, n, z=1.96):
    if not n:
        return None, None
    p = k / n
    centre = (p + z * z / (2 * n)) / (1 + z * z / n)
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / (1 + z * z / n)
    return max(0.0, centre - half), min(1.0, centre + half)


def med(values):
    values = [v for value in values if (v := usage.number(value)) is not None]
    return statistics.median(values) if values else None


def read_rows(paths):
    rows = []
    for path in paths:
        with Path(path).open() as source:
            for line_no, line in enumerate(source, 1):
                if not line.strip():
                    continue
                try:
                    row = json.loads(line)
                except ValueError as error:
                    raise ValueError(f"invalid decision JSON: {path}:{line_no}") from error
                if not isinstance(row, dict):
                    raise ValueError(f"decision row must be an object: {path}:{line_no}")
                rows.append(row)
    return rows


def metric_summary(rows, fields):
    observations = {field: [usage.number(row.get(field)) for row in rows] for field in fields}
    totals, subtotals, coverage = usage.history.receipt_fields(observations)
    reported_medians = {field: med(values) for field, values in observations.items()}
    medians = {field: reported_medians[field] if coverage[field]["complete"] else None for field in fields}
    return medians, reported_medians, subtotals, coverage


def reply_observed(row):
    calls, args = row.get("calls"), row.get("args")
    return (not row.get("error") and isinstance(calls, list) and isinstance(args, list)
            and len(calls) == len(args) and all(isinstance(c, str) and c for c in calls)
            and all(isinstance(arg, str) for arg in args))


def move(r):
    """The reply's next move: answer, or its calls with a ledger read named as one."""
    if not r["calls"]:
        return "answer"
    names = ["read ledger" if "ledger://" in a else c for c, a in zip(r["calls"], r["args"] + [""] * len(r["calls"]))]
    return " + ".join(f"{n} ×{names.count(n)}" if names.count(n) > 1 else n for n in sorted(set(names)))


def aggregate(rows):
    cells = collections.defaultdict(list)
    for row in rows:
        key = tuple(value if isinstance(value := row.get(field), str) and value else "unknown"
                    for field in ("scenario", "model", "arm"))
        cells[key].append(row)
    known = [(scenario, model, arm) for scenario in SCENARIOS for model in MODELS for arm in ARMS]
    # Unknown cohorts remain visible after the historical ordering.
    order = [key for key in known if key in cells] + [key for key in cells if key not in known]
    out = []
    for scenario, model, arm in order:
        rs = cells[(scenario, model, arm)]
        replies = [row for row in rs if reply_observed(row)]
        scored = [row for row in replies if isinstance(row.get("match"), bool)]
        k = sum(row["match"] for row in scored)
        lo, hi = wilson(k, len(scored))
        moves = collections.Counter(move(row) for row in replies)
        medians, reported, subtotals, coverage = metric_summary(rs, ("prompt", "completion", "reasoning", "secs"))
        out.append({"scenario": scenario, "model": model, "arm": arm, "n": len(scored),
                    "attempts": len(rs), "replies": len(replies), "no_reply": len(rs) - len(replies),
                    "unscored_replies": len(replies) - len(scored), "errors": sum(bool(row.get("error")) for row in rs),
                    "match": k, "decoded": sum("ledger://" in "".join(row["args"]) for row in replies),
                    "rate": k / len(scored) if scored else None, "lo": lo, "hi": hi, "moves": moves.most_common(),
                    **medians, "reported_medians": reported, "reported_subtotals": subtotals,
                    "metric_coverage": coverage})
    return out


def summary(cell):
    c = cell
    interval = f"[{c['lo']:.2f},{c['hi']:.2f}]" if c["lo"] is not None else "[—,—]"
    text = (f"{c['scenario']:8} {c['model']:14} {c['arm']:7} {c['match']:2}/{c['n']:<2} dec={c['decoded']} "
            f"{interval} prompt={c['prompt']} out={c['completion']} {c['secs']}s {c['moves'][:3]}")
    if c["n"] != c["attempts"] or not all(value["complete"] for value in c["metric_coverage"].values()):
        fields = ", ".join(f"{key} {value['reported']}/{value['attempts']} reported median={c['reported_medians'][key]}"
                           for key, value in c["metric_coverage"].items() if not value["complete"])
        text += f" [attempts={c['attempts']}; replies={c['replies']}; errors={c['errors']}; no_reply={c['no_reply']}; {fields}]"
    return text


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="*", type=Path)
    args = parser.parse_args(argv)
    try:
        out = aggregate(read_rows(args.paths))
    except (OSError, ValueError) as error:
        parser.exit(2, f"{error}\n")
    json.dump(out, sys.stdout, indent=1, ensure_ascii=False, allow_nan=False)
    for cell in out:
        print(summary(cell), file=sys.stderr)


if __name__ == "__main__":
    main()
