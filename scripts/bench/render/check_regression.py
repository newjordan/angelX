#!/usr/bin/env python3
"""Regression gate for the render scorecard (docs/RENDER_SCORECARD.md).

Compares glass medians in a fresh run directory against the committed
v2 baseline (baseline-v2.json, same directory as this script). A metric
regresses when its fresh median exceeds baseline * (1 + tol) + floor_ms.
Kitty columns, when present, are reported but never fail the gate: the
gate guards glass against its own past, not against Kitty's present.

Usage:
    check_regression.py RUN_DIR [--tol 0.25] [--floor-ms 0.25]

RUN_DIR holds the same file shapes run_v2.py / run_scorecard.sh write:
glass "<mode>...series.json" files with "frames_ms", keyframe text
files with "raw_ms=[...]". Missing metrics are reported as SKIP, not
failures, so a partial re-run still gates what it measured.
Exit status: 0 pass, 1 regression, 2 usage/no metrics found.
"""
import glob
import json
import os
import re
import sys

BASELINE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "baseline-v2.json")


def median(vals):
    vals = sorted(vals)
    if not vals:
        return None
    return vals[len(vals) // 2]


def glass_medians(run_dir):
    """Map metric name -> median frame ms from a run directory."""
    out = {}

    def series(pattern):
        vals = []
        for path in sorted(glob.glob(os.path.join(run_dir, pattern))):
            vals += json.load(open(path))["frames_ms"]
        return median(vals)

    kf = []
    for path in sorted(glob.glob(os.path.join(run_dir, "keyframe-glass*.txt"))):
        m = re.search(r"raw_ms=\[([^\]]+)\]", open(path).read())
        if m:
            kf += [float(x) for x in m.group(1).split(",")]
    if kf:
        out["M1-keyframe"] = median(kf)
    for metric, pattern in (
        ("M2-blast4", "glass-blast4-immediate*.series.json"),
        ("M3-stream", "glass-big-immediate*.series.json"),
        ("M4-placements", "glass-placements-immediate*.series.json"),
        ("M5-text", "glass-text-immediate*.series.json"),
        ("M6-churn", "glass-churn-immediate*.series.json"),
    ):
        got = series(pattern)
        if got is not None:
            out[metric] = got
    return out


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 2
    run_dir = argv[1]
    tol = 0.25
    floor_ms = 0.25
    rest = argv[2:]
    while rest:
        if rest[0] == "--tol":
            tol = float(rest[1])
            rest = rest[2:]
        elif rest[0] == "--floor-ms":
            floor_ms = float(rest[1])
            rest = rest[2:]
        else:
            print(f"unknown argument: {rest[0]}")
            return 2
    baseline = json.load(open(BASELINE))["glass_median_ms"]
    fresh = glass_medians(run_dir)
    if not fresh:
        print(f"no glass metrics found in {run_dir}")
        return 2
    failed = []
    for metric in sorted(baseline):
        base = baseline[metric]
        got = fresh.get(metric)
        if got is None:
            print(f"SKIP {metric}: not in run (baseline {base:.3f} ms)")
            continue
        limit = base * (1 + tol) + floor_ms
        status = "PASS" if got <= limit else "FAIL"
        if status == "FAIL":
            failed.append(metric)
        print(f"{status} {metric}: {got:.3f} ms (baseline {base:.3f}, limit {limit:.3f})")
    if failed:
        print(f"regressed: {', '.join(failed)}")
        return 1
    print("no regressions")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
