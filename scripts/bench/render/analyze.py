#!/usr/bin/env python3
"""Bootstrap verdicts for scorecard v2 (docs/RENDER_SCORECARD.md).

Reads pooled raw frame series for two sides and reports medians plus
a 95% bootstrap interval on the difference of medians (glass - kitty,
so a negative interval favours glass on time metrics). Deterministic:
the resampler is a seeded LCG, no wall-clock or hash-order dependence,
so a stranger re-running this on the committed series gets the same
numbers to the last digit.
"""
import json
import math
import statistics
import sys


class Lcg:
    """A tiny seeded PRNG (Numerical Recipes constants)."""

    def __init__(self, seed=0x2545F491):
        self.state = seed & 0xFFFFFFFF

    def randrange(self, n):
        self.state = (1664525 * self.state + 1013904223) & 0xFFFFFFFF
        return self.state % n


def percentile(sorted_vals, p):
    if not sorted_vals:
        return float("nan")
    idx = min(len(sorted_vals) - 1, max(0, math.floor(p * len(sorted_vals))))
    return sorted_vals[idx]


def bootstrap_median_diff(a, b, resamples=10000, seed=0x2545F491):
    """(median(a)-median(b), lo, hi) at 95% by percentile bootstrap."""
    if not a or not b:
        return (float("nan"),) * 3
    rng = Lcg(seed)
    diffs = []
    for _ in range(resamples):
        ma = statistics.median(a[rng.randrange(len(a))] for _ in range(len(a)))
        mb = statistics.median(b[rng.randrange(len(b))] for _ in range(len(b)))
        diffs.append(ma - mb)
    diffs.sort()
    point = statistics.median(a) - statistics.median(b)
    return point, percentile(diffs, 0.025), percentile(diffs, 0.975)


def summarize(name, glass, kitty, lower_is_better=True):
    point, lo, hi = bootstrap_median_diff(glass, kitty)
    if lower_is_better:
        verdict = "glass" if hi < 0 else ("kitty" if lo > 0 else "inconclusive")
    else:
        verdict = "glass" if lo > 0 else ("kitty" if hi < 0 else "inconclusive")
    return {
        "metric": name,
        "glass_median": round(statistics.median(glass), 3) if glass else None,
        "kitty_median": round(statistics.median(kitty), 3) if kitty else None,
        "glass_p99": round(percentile(sorted(glass), 0.99), 3) if glass else None,
        "kitty_p99": round(percentile(sorted(kitty), 0.99), 3) if kitty else None,
        "diff_median": round(point, 3),
        "ci95": [round(lo, 3), round(hi, 3)],
        "n_glass": len(glass),
        "n_kitty": len(kitty),
        "verdict": verdict,
    }


def main():
    # analyze.py NAME GLASS_JSONL... -- KITTY_JSON...
    # Each glass file: a series.json ({"frames_ms": [...]}) or keyframe
    # text is handled by the caller converting first. Kitty files are
    # bench JSONs carrying "times_ms".
    args = sys.argv[1:]
    name = args[0]
    sep = args.index("--")
    glass, kitty = [], []
    for path in args[1:sep]:
        glass += json.load(open(path))["frames_ms"]
    for path in args[sep + 1:]:
        kitty += json.load(open(path))["times_ms"]
    print(json.dumps(summarize(name, glass, kitty), indent=1))


if __name__ == "__main__":
    main()
