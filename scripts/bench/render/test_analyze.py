#!/usr/bin/env python3
"""Durable checks for the v2 bootstrap: known separations must resolve,
identical distributions must straddle zero, and the result must be
bit-reproducible.

Run: python3 scripts/bench/render/test_analyze.py
"""
from analyze import bootstrap_median_diff, summarize


def test_clear_separation_resolves():
    a = [1.0 + (i % 7) * 0.01 for i in range(400)]
    b = [3.0 + (i % 7) * 0.01 for i in range(400)]
    point, lo, hi = bootstrap_median_diff(a, b)
    assert -2.1 < point < -1.9, point
    assert hi < 0, (lo, hi)
    assert summarize("x", a, b)["verdict"] == "glass"
    assert summarize("x", b, a)["verdict"] == "kitty"


def test_identical_distributions_straddle_zero():
    a = [1.0 + (i % 13) * 0.05 for i in range(400)]
    _point, lo, hi = bootstrap_median_diff(a, list(a))
    assert lo <= 0 <= hi, (lo, hi)


def test_reproducible_to_the_digit():
    a = [float(i % 29) for i in range(200)]
    b = [float(i % 31) for i in range(200)]
    assert bootstrap_median_diff(a, b) == bootstrap_median_diff(a, b)


def test_empty_is_nan_not_a_crash():
    point, lo, hi = bootstrap_median_diff([], [1.0])
    assert point != point and lo != lo and hi != hi  # NaN


if __name__ == "__main__":
    test_clear_separation_resolves()
    test_identical_distributions_straddle_zero()
    test_reproducible_to_the_digit()
    test_empty_is_nan_not_a_crash()
    print("test_analyze: ok")
