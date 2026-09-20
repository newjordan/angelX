"""Selection from the Pareto front for the GEPA proposer.

Exposes a single ``select_next`` function that returns the next individual
the caller should act on, plus a flag indicating whether that individual
should be mutated (to produce a child) or simply re-evaluated (to tighten
its fitness estimate).

Design (see NOTES.md §3 + §7):

  - With probability ``exploration_rate`` (default 0.2), pick a random
    parent from the Pareto front and signal that it should be MUTATED.
    Borrows from upstream's ``epsilon_greedy`` candidate-selector spirit.
  - With probability ``1 - exploration_rate``, pick the Pareto-best
    individual (scored by ``0.7 * accuracy_delta + 0.3 * novelty``, ties
    broken by ``evaluation_count``) and signal RE-EVALUATION. Re-evaluating
    the current best gathers more verdict samples on its fitness without
    perturbing the textual parameter — analogous to upstream's
    ``current_best`` selector.

Empty-population case: the caller (``proposer.propose_gepa``) is expected
to seed via the composer proposer. ``select_next`` returns a sentinel
``{"parent": None, ...}`` for clarity rather than raising.

Agent seed (Phase 15): when the scoring layer is enabled in program.md's
``## Agent (bandit + scoring)`` block, the ``_scored_best`` score
formula is replaced with a blend of the hand-rolled novelty signal and
the ``LinearScorer``'s predicted reward. When scoring is disabled, the
original formula is used unchanged.

Public surface:
  - ``select_next(population, exploration_rate=0.2) -> dict``
    Returns ``{"parent": dict | None, "should_mutate": bool, "selection_reason": str}``.
"""

from __future__ import annotations

import random
from typing import Any

from .population import Population


# Default weight blend used to pick the Pareto-best individual for re-evaluation.
# Tilted toward accuracy_delta so the loop spends its re-evaluation budget on
# already-promising individuals; novelty still matters as a tiebreaker against
# many-times-evaluated micro-edits clustering near each other.
_ACCURACY_WEIGHT = 0.7

# Warmup threshold: while population is smaller than this, every selection
# forces a mutation (ignoring the configured exploration_rate). Re-evaluating
# a tiny population produces no new signal — it just re-runs the same idea
# against the same cached workspace. Spend early iterations on growth instead.
WARMUP_SIZE = 5
_NOVELTY_WEIGHT = 0.3


def _scored_best(
    front: list[dict[str, Any]],
    *,
    scorer_blend: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Return the highest-scoring individual on the Pareto front.

    Default score = ``_ACCURACY_WEIGHT * accuracy_delta + _NOVELTY_WEIGHT * novelty``;
    ties broken by ``evaluation_count`` (better-tested first), then by
    ``id`` (lexicographic) for determinism.

    When ``scorer_blend`` is provided (callers wire this when
    ``agent.scoring.enabled: true`` in program.md), the score formula
    becomes:

        score = novelty_weight   * novelty
              + predicted_weight * scorer.predict_score(features)

    ``scorer_blend`` must contain keys ``scorer`` (callable: ind -> float),
    ``novelty_weight`` (float), ``predicted_weight`` (float). The two
    weights are expected to sum to ~1.0 but the function does NOT
    enforce or renormalize this — that's the caller's job.
    """
    if scorer_blend is not None:
        scorer_fn = scorer_blend["scorer"]
        nw = float(scorer_blend["novelty_weight"])
        pw = float(scorer_blend["predicted_weight"])

        def key(ind: dict[str, Any]) -> tuple[float, int, str]:
            f = ind.get("fitness") or {}
            nv = float(f.get("novelty", 0.0) or 0.0)
            ec = int(f.get("evaluation_count", 0) or 0)
            try:
                pred = float(scorer_fn(ind))
            except Exception:  # noqa: BLE001 — scorer failures must never crash selection
                pred = 0.0
            return (
                nw * nv + pw * pred,
                ec,
                ind.get("id") or "",
            )

        return max(front, key=key)

    def key(ind: dict[str, Any]) -> tuple[float, int, str]:
        f = ind.get("fitness") or {}
        ad = float(f.get("accuracy_delta", 0.0) or 0.0)
        nv = float(f.get("novelty", 0.0) or 0.0)
        ec = int(f.get("evaluation_count", 0) or 0)
        return (
            _ACCURACY_WEIGHT * ad + _NOVELTY_WEIGHT * nv,
            ec,
            ind.get("id") or "",
        )

    # ``max`` with the composite key gives us the best by score then
    # ``evaluation_count`` then ``id``.
    return max(front, key=key)


def select_next(
    population: Population,
    exploration_rate: float = 0.2,
    *,
    rng: random.Random | None = None,
    scorer_blend: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Decide what the GEPA proposer should do next.

    Args:
      population:        The loaded Population store.
      exploration_rate:  Probability in ``[0, 1]`` of picking a random
                         Pareto-front parent for mutation. ``1 - rate`` is
                         the probability of re-evaluating the current best.
                         Clamped to ``[0, 1]`` defensively.
      rng:               Optional ``random.Random`` for deterministic tests;
                         defaults to the module-level ``random``.

    Returns:
      ``{"parent": dict | None, "should_mutate": bool, "selection_reason": str}``.

      - Empty population: ``parent=None``, ``should_mutate=True``,
        ``selection_reason="empty_population_seed_required"``. Caller seeds
        via the composer proposer.
      - Empty Pareto front despite non-empty population (defensive — should
        never happen if fitness was computed correctly): falls back to the
        whole pool for selection so the loop never deadlocks.
    """
    all_inds = population.all()
    if not all_inds:
        return {
            "parent": None,
            "should_mutate": True,
            "selection_reason": "empty_population_seed_required",
        }

    rate = max(0.0, min(1.0, float(exploration_rate)))
    rng = rng or random

    front = population.pareto_front()
    pool = front if front else all_inds  # defensive fallback

    # WARMUP: while population is below WARMUP_SIZE, always mutate (force
    # exploration). Re-evaluating a 1-individual population just runs the
    # same idea against the same cached workspace — pure waste. Burn early
    # iterations on growing the population instead.
    pop_size = len(all_inds)
    if pop_size < WARMUP_SIZE:
        parent = rng.choice(pool)
        return {
            "parent": parent,
            "should_mutate": True,
            "selection_reason": (
                f"warmup (population size {pop_size} < {WARMUP_SIZE}); "
                f"forced mutation of parent {parent.get('id')!r}"
            ),
        }

    if rng.random() < rate:
        parent = rng.choice(pool)
        return {
            "parent": parent,
            "should_mutate": True,
            "selection_reason": (
                f"exploration (rate={rate:.2f}); picked random Pareto-front parent "
                f"{parent.get('id')!r}"
            ),
        }

    best = _scored_best(pool, scorer_blend=scorer_blend)
    return {
        "parent": best,
        "should_mutate": False,
        "selection_reason": (
            f"exploitation (1-rate={1.0 - rate:.2f}); re-evaluating Pareto-best "
            f"{best.get('id')!r} to tighten fitness estimate"
        ),
    }
