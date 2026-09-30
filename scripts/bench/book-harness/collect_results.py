#!/usr/bin/env python3
"""collect_results.py <out-dir>... > results.json — checked receipts for reports.

Every attempted proxy call remains in the request count. Missing usage has
per-field coverage and reported subtotals; it never becomes a measured zero.
A later directory wins for a label present in both.
"""
import argparse
import glob
import importlib.util
import json
import os
from pathlib import Path
import re
import sys


spec = importlib.util.spec_from_file_location("book_receipt_usage", Path(__file__).with_name("receipt_usage.py"))
receipt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(receipt)


def usage_totals(rows):
    totals = receipt.usage_summary(rows)
    return tuple(totals[key] for key in ("prompt", "cached", "completion", "reasoning"))


per_request = receipt.per_request


def collect(roots):
    roots = tuple(map(str, roots))
    runs = {}
    for root in roots:
        for log in glob.glob(os.path.join(root, "*.jsonl")):
            label = os.path.basename(log)[:-6]
            if label.startswith("e2-count") or label.startswith("e2-") or "pilot" in label:
                continue
            rows = receipt.read_receipts(log)
            if not rows:
                continue
            body = rows[0].get("body") if isinstance(rows[0].get("body"), dict) else {}
            run = {"label": label, "requests": len(rows), **receipt.usage_summary(rows),
                   "model": body.get("model"), "host": "sparky" if "sparky" in root else "atlas"}
            runline = os.path.join(root, label + ".run")
            if os.path.exists(runline):
                text = Path(runline).read_text()
                run["solved"] = "solved=yes" in text if "solved=" in text else None
                wall = re.search(r"wall=([\d.]+)", text)
                run["wall_s"] = receipt.number(float(wall.group(1))) if wall else None
                status = re.search(r"status=(\S+)", text)
                run["status"] = status.group(1) if status else None
            receipt_path = os.path.join(root, label + ".result.json")
            if os.path.exists(receipt_path):
                try:
                    timing = json.loads(Path(receipt_path).read_text()).get("timing") or {}
                    run["timing"] = {k: timing.get(k) for k in (
                        "wall_ms", "model_ms", "tool_ms", "tool_overhead_ms", "residual_ms",
                        "other_ms", "startup_ms", "shutdown_ms", "model_calls", "tool_calls")}
                except ValueError:
                    pass
            if label.startswith(("e3-", "dsfinal-", "dsloop-", "twoturn-")):
                run["per_request"] = per_request(rows)
            runs[label] = run

    e2 = []
    for root in roots:
        for path in glob.glob(os.path.join(root, "e2-count-*.jsonl")):
            for line_no, line in enumerate(Path(path).read_text().splitlines(), 1):
                if not line.strip():
                    continue
                try:
                    row = json.loads(line)
                except ValueError as error:
                    raise ValueError(f"invalid count JSON: {path}:{line_no}") from error
                if isinstance(row, dict) and "route" in row and "label" in row:
                    e2.append({"route": row["route"], "label": row["label"],
                               "prompt_tokens": receipt.number(row.get("prompt_tokens")),
                               **{key: row[key] for key in ("status", "error_class", "attempts", "setup_error")
                                  if key in row}})
    return {"runs": runs, "e2": e2}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("roots", nargs="*", type=Path)
    args = parser.parse_args(argv)
    try:
        result = collect(args.roots)
    except (ValueError, OSError) as error:
        parser.error(str(error))
    json.dump(result, sys.stdout, allow_nan=False)


if __name__ == "__main__":
    main()
