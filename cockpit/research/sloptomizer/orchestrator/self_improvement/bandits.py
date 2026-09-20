"""Multi-armed bandit policies for source-mix optimization.

Three policies, each takes ``SourceStats`` rows and emits a score per
source — higher score = "spend more iterations here." The score is
NOT a probability; convert with :func:`scores_to_weights`.

Policies:
  - **UCB1** (Auer/Cesa-Bianchi/Fischer 2002): deterministic, optimism
    in face of uncertainty. Best when you have a moderate number of
    pulls per arm and want zero hyperparameters.
  - **Thompson Sampling (Beta-Bernoulli)**: probabilistic posterior
    sampling. Best for binary reward (PASS vs not-PASS) — the natural
    fit for verdict outcomes. Listed in program.md as a future direction.
  - **Epsilon-greedy**: simple exploration-exploitation tradeoff.
    Useful as a baseline / when you want a tunable exploration rate.

Math reference: each ``SourceStats`` carries ``n_judged`` (= total
pulls of that arm) and ``n_pass`` (= successes). The reward signal is
``PASS = 1, FAIL/INCONCLUSIVE = 0`` by default; configurable via
``reward_map``.

Pure stdlib + math + random. No numpy.
"""

from __future__ import annotations

import math
import random
from dataclasses import dataclass
from typing import Mapping

from .stats import SourceStats


# Per-policy reward map. Default treats PASS as the only success.
DEFAULT_REWARD_MAP: dict[str, float] = {
    "PASS": 1.0,
    "INCONCLUSIVE": 0.0,
    "FAIL": 0.0,
}


@dataclass
class ArmScore:
    source: str
    score: float
    n_pulls: int          # n_judged for the arm
    n_successes: float    # accumulated reward (sum of reward_map values)
    note: str = ""        # one-line rationale (UCB term, posterior mean, etc.)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _success_count(s: SourceStats, reward_map: Mapping[str, float]) -> float:
    """Compute total reward for an arm under the reward map."""
    return (
        reward_map.get("PASS", 0.0) * s.n_pass
        + reward_map.get("FAIL", 0.0) * s.n_fail
        + reward_map.get("INCONCLUSIVE", 0.0) * s.n_inconclusive
    )


def _total_pulls(stats: Mapping[str, SourceStats]) -> int:
    return sum(s.n_judged for s in stats.values())


# ---------------------------------------------------------------------------
# UCB1
# ---------------------------------------------------------------------------


def ucb1_scores(
    stats: Mapping[str, SourceStats],
    *,
    reward_map: Mapping[str, float] = DEFAULT_REWARD_MAP,
    c: float = math.sqrt(2),
    cold_start_score: float = float("inf"),
) -> list[ArmScore]:
    """UCB1: score = mean_reward + c * sqrt(2 * ln(N) / n_i).

    ``c`` is the exploration coefficient (sqrt(2) is the Auer/CB/F
    canonical default). Unpulled arms receive ``cold_start_score`` so
    they're tried first.
    """
    N = _total_pulls(stats)
    out: list[ArmScore] = []
    for src, s in stats.items():
        successes = _success_count(s, reward_map)
        if s.n_judged == 0:
            out.append(ArmScore(
                source=src, score=cold_start_score,
                n_pulls=0, n_successes=0.0,
                note="cold-start: never pulled",
            ))
            continue
        if N == 0:
            mean = successes / s.n_judged
            out.append(ArmScore(
                source=src, score=mean,
                n_pulls=s.n_judged, n_successes=successes,
                note=f"mean-only (N=0): {mean:.3f}",
            ))
            continue
        mean = successes / s.n_judged
        bonus = c * math.sqrt(math.log(N) / s.n_judged)
        out.append(ArmScore(
            source=src, score=mean + bonus,
            n_pulls=s.n_judged, n_successes=successes,
            note=f"mean={mean:.3f} + bonus={bonus:.3f}",
        ))
    out.sort(key=lambda a: a.score, reverse=True)
    return out


# ---------------------------------------------------------------------------
# Thompson sampling (Beta-Bernoulli)
# ---------------------------------------------------------------------------


def thompson_scores(
    stats: Mapping[str, SourceStats],
    *,
    reward_map: Mapping[str, float] = DEFAULT_REWARD_MAP,
    alpha_prior: float = 1.0,
    beta_prior: float = 1.0,
    n_samples: int = 1000,
    seed: int | None = None,
) -> list[ArmScore]:
    """Beta-Bernoulli Thompson sampling.

    For each arm we maintain a Beta(α + successes, β + failures)
    posterior. The score is the mean posterior over ``n_samples`` draws
    (using stdlib :func:`random.betavariate`). The uniform prior
    Beta(1, 1) is the standard non-informative choice.

    Returns the AVERAGE of ``n_samples`` posterior draws so the result
    is deterministic given ``seed`` — for actual arm selection on each
    pull, use :func:`thompson_select` instead.
    """
    rng = random.Random(seed)
    out: list[ArmScore] = []
    for src, s in stats.items():
        successes = _success_count(s, reward_map)
        failures = s.n_judged - successes
        # Beta requires positive params; priors handle the n_judged=0 case.
        a = alpha_prior + successes
        b = beta_prior + max(0.0, failures)
        draws = [rng.betavariate(a, b) for _ in range(n_samples)]
        score = sum(draws) / len(draws)
        out.append(ArmScore(
            source=src, score=score,
            n_pulls=s.n_judged, n_successes=successes,
            note=f"Beta(α={a:.1f}, β={b:.1f}) posterior mean={score:.3f}",
        ))
    out.sort(key=lambda a: a.score, reverse=True)
    return out


def thompson_select(
    stats: Mapping[str, SourceStats],
    *,
    reward_map: Mapping[str, float] = DEFAULT_REWARD_MAP,
    alpha_prior: float = 1.0,
    beta_prior: float = 1.0,
    seed: int | None = None,
) -> str | None:
    """Draw ONE sample per arm; return the arm with the highest sample.

    This is the canonical Thompson selection rule — use it when you
    want to PICK an arm to pull next (not just rank arms).
    """
    if not stats:
        return None
    rng = random.Random(seed)
    best_src: str | None = None
    best_draw = -1.0
    for src, s in stats.items():
        successes = _success_count(s, reward_map)
        failures = max(0.0, s.n_judged - successes)
        draw = rng.betavariate(alpha_prior + successes, beta_prior + failures)
        if draw > best_draw:
            best_draw = draw; best_src = src
    return best_src


# ---------------------------------------------------------------------------
# Epsilon-greedy
# ---------------------------------------------------------------------------


def epsilon_greedy_scores(
    stats: Mapping[str, SourceStats],
    *,
    reward_map: Mapping[str, float] = DEFAULT_REWARD_MAP,
    epsilon: float = 0.1,
    cold_start_score: float = 0.5,
) -> list[ArmScore]:
    """ε-greedy: score = (1-ε)·mean + ε·uniform_baseline.

    Uniform baseline = ``cold_start_score``. This isn't the classical
    selection rule (which is stochastic), but produces deterministic
    scores you can sort/budget on. For actual arm selection on a
    single pull, use :func:`epsilon_greedy_select`.
    """
    out: list[ArmScore] = []
    for src, s in stats.items():
        successes = _success_count(s, reward_map)
        if s.n_judged == 0:
            mean = cold_start_score
        else:
            mean = successes / s.n_judged
        score = (1.0 - epsilon) * mean + epsilon * cold_start_score
        out.append(ArmScore(
            source=src, score=score,
            n_pulls=s.n_judged, n_successes=successes,
            note=f"mean={mean:.3f}, ε-blend={score:.3f}",
        ))
    out.sort(key=lambda a: a.score, reverse=True)
    return out


def epsilon_greedy_select(
    stats: Mapping[str, SourceStats],
    *,
    reward_map: Mapping[str, float] = DEFAULT_REWARD_MAP,
    epsilon: float = 0.1,
    seed: int | None = None,
) -> str | None:
    """Classical ε-greedy single-pull selection."""
    if not stats:
        return None
    rng = random.Random(seed)
    if rng.random() < epsilon:
        return rng.choice(list(stats.keys()))
    best_src: str | None = None
    best_mean = -1.0
    for src, s in stats.items():
        if s.n_judged == 0:
            continue
        successes = _success_count(s, reward_map)
        mean = successes / s.n_judged
        if mean > best_mean:
            best_mean = mean; best_src = src
    if best_src is None:  # all cold-start
        return rng.choice(list(stats.keys()))
    return best_src


# ---------------------------------------------------------------------------
# Scores → weights
# ---------------------------------------------------------------------------


def scores_to_weights(
    scores: list[ArmScore],
    *,
    softmax_temp: float = 1.0,
    min_weight: float = 0.0,
) -> dict[str, float]:
    """Softmax over scores (with NaN/inf clamping) → probability weights.

    Infinite scores (cold-start UCB) collapse to "split mass evenly
    across infinite-score arms"; non-infinite arms get the remainder.
    """
    if not scores:
        return {}
    infs = [a for a in scores if math.isinf(a.score)]
    finite = [a for a in scores if not math.isinf(a.score)]
    if infs:
        # When at least one arm is cold-start (inf score), allocate
        # half of the budget evenly to all cold-start arms and the
        # other half by softmax over the finite arms.
        cold_share = 0.5 if finite else 1.0
        weights: dict[str, float] = {}
        if infs:
            per = cold_share / len(infs)
            for a in infs:
                weights[a.source] = per
        if finite:
            remaining = 1.0 - cold_share
            sub = scores_to_weights(finite, softmax_temp=softmax_temp,
                                    min_weight=0.0)
            for k, v in sub.items():
                weights[k] = remaining * v
    else:
        # softmax over finite scores
        scaled = [a.score / softmax_temp for a in scores]
        m = max(scaled)
        exps = [math.exp(x - m) for x in scaled]
        Z = sum(exps)
        weights = {a.source: e / Z for a, e in zip(scores, exps)}
    if min_weight > 0 and weights:
        # Strict floor: reserve K*min_weight of the budget for floors,
        # then distribute the remainder by the original softmax shares.
        # This guarantees w[k] >= min_weight for every k, provided
        # K*min_weight <= 1 (we clamp if the caller asks for an
        # infeasible floor).
        K = len(weights)
        eff_floor = min(min_weight, 1.0 / K)
        slack = 1.0 - K * eff_floor
        weights = {k: eff_floor + slack * v for k, v in weights.items()}
    return weights
