#!/usr/bin/env python3
"""Export checked Polyglot chart arrays for Angel, OpenCode and an optional rerun.

The existing tokens-time.html renderer coerces missing values to zero. Require
one observed attempt per catalog task with valid score, time and input coverage.
"""
import argparse
import importlib.util
import json
from pathlib import Path


spec = importlib.util.spec_from_file_location(
    "seat_compare", Path(__file__).resolve().parents[1] / "seat_compare.py")
seat = importlib.util.module_from_spec(spec)
spec.loader.exec_module(seat)


def per_task(run, order):
    cell = seat.load(run)
    cell["summary"] = seat.aggregate(cell, order)
    return seat.chart_series(cell, order)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("angelx_run", type=Path)
    parser.add_argument("opencode_run", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("angelx2_run", type=Path, nargs="?")
    parser.add_argument("--catalog", type=Path, default=seat.RR / "tasks-polyglot-v1.json")
    args = parser.parse_intermixed_args(argv)
    order = [task.get("name") for task in json.loads(args.catalog.read_text())]
    if not order or any(not isinstance(name, str) or not name for name in order) or len(set(order)) != len(order):
        parser.error("catalog must contain unique, nonempty task names")
    runs = {"angelx": args.angelx_run, "opencode": args.opencode_run}
    if args.angelx2_run is not None:
        runs["angelx2"] = args.angelx2_run
    try:
        series = {label: per_task(run, order) for label, run in runs.items()}
    except ValueError as error:
        parser.error(str(error))
    args.output.write_text(json.dumps(series, allow_nan=False) + "\n")


if __name__ == "__main__":
    main()
