#!/usr/bin/env python3
import itertools
import random
import statistics

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
    assert point != point and lo != lo and hi != hi


def test_two_observations_match_exact_bootstrap():
    a, b = [1.0, 5.0], [2.0, 6.0]
    diffs = sorted(
        statistics.median(x) - statistics.median(y)
        for x in itertools.product(a, repeat=len(a))
        for y in itertools.product(b, repeat=len(b))
    )
    expected = (diffs[int(0.025 * len(diffs))], diffs[int(0.975 * len(diffs))])
    assert expected == (-5.0, 3.0), expected
    point, lo, hi = bootstrap_median_diff(a, b)
    assert (point, lo, hi) == (-1.0, *expected), (point, lo, hi)
    for lower_is_better in (True, False):
        assert summarize("x", a, b, lower_is_better)["verdict"] == "inconclusive"


def test_power_of_two_samples_retain_uncertainty():
    for n in (2, 4, 8, 16, 32):
        a = list(range(n))
        b = [x + 0.25 for x in a]
        point, lo, hi = bootstrap_median_diff(a, b, resamples=2000)
        assert point == -0.25, (n, point)
        assert lo < 0 < hi, (n, lo, hi)


def test_constant_samples_keep_exact_results_and_polarities():
    assert bootstrap_median_diff([1.0], [3.0]) == (-2.0, -2.0, -2.0)
    assert bootstrap_median_diff([2.0] * 4, [2.0] * 8) == (0.0, 0.0, 0.0)
    assert summarize("x", [1.0], [3.0], lower_is_better=False)["verdict"] == "kitty"
    assert summarize("x", [3.0], [1.0], lower_is_better=False)["verdict"] == "glass"


def test_seed_is_local_and_series_are_unchanged():
    a, b = [1.0, 5.0, 7.0, 9.0], [2.0, 4.0, 6.0, 8.0]
    before_a, before_b = list(a), list(b)
    state = random.getstate()
    first = bootstrap_median_diff(a, b, resamples=200, seed=123)
    assert first == bootstrap_median_diff(a, b, resamples=200, seed=123)
    assert random.getstate() == state
    assert a == before_a and b == before_b


if __name__ == "__main__":
    test_clear_separation_resolves()
    test_identical_distributions_straddle_zero()
    test_reproducible_to_the_digit()
    test_empty_is_nan_not_a_crash()
    test_two_observations_match_exact_bootstrap()
    test_power_of_two_samples_retain_uncertainty()
    test_constant_samples_keep_exact_results_and_polarities()
    test_seed_is_local_and_series_are_unchanged()
    print("test_analyze: ok")
