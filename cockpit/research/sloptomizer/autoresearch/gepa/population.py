"""Pareto-front population store for the GEPA proposer.

Persisted to ``autoresearch/gepa_population.jsonl`` — one JSON object per
line, append-friendly and human-readable (same shape as
``verdicts.jsonl``). The file is part of project state; do NOT add it to
.gitignore.

Public surface:
  - ``Population``       — in-memory mirror of the JSONL file.
  - ``compute_novelty``  — stdlib Jaccard-distance novelty scorer.

Each individual is a dict with the following shape (see NOTES.md §3 for
the rationale on each field):

    {
      "id": "ind_<utc_ts>_<short_hash>",
      "idea_text": str,
      "born_iter": int,
      "verdict_history": [
        {"idea_id": str, "decision": "PASS|FAIL|INCONCLUSIVE",
         "gate0_reject": bool, "primary_delta": float|None,
         "timestamp": "YYYY-MM-DDTHH:MM:SSZ"},
        ...
      ],
      "parent_id": str | None,
      "mutation_rationale": str | None,
      "fitness": {
        "accuracy_delta": float,    # avg primary_delta of PASS verdicts; 0 if no PASS
        "novelty": float,           # 1.0 - max-Jaccard-similarity to other individuals
        "evaluation_count": int     # len(verdict_history)
      }
    }
"""

from __future__ import annotations

import hashlib
import json
import re
import time
from pathlib import Path
from typing import Any, Iterable


# ---------------------------------------------------------------------------
# Token-level novelty (stdlib; no embeddings)
# ---------------------------------------------------------------------------


# Split on whitespace and ASCII punctuation. Keeps the scorer dependency-free
# while still tokenising natural language well enough for short idea strings.
_TOKEN_SPLIT_RE = re.compile(r"[^a-z0-9]+")


def _tokens(text: str) -> set[str]:
    """Lowercase + split on non-alphanumeric. Empty tokens dropped."""
    if not text:
        return set()
    return {t for t in _TOKEN_SPLIT_RE.split(text.lower()) if t}


def _jaccard_similarity(a: set[str], b: set[str]) -> float:
    """Standard Jaccard: |A∩B| / |A∪B|. Both empty -> 0 (no overlap signal)."""
    if not a and not b:
        return 0.0
    inter = len(a & b)
    union = len(a | b)
    if union == 0:
        return 0.0
    return inter / union


def compute_novelty(text: str, others: Iterable[dict[str, Any]]) -> float:
    """Novelty score in ``[0, 1]``: ``1.0 - max(Jaccard(text, other.idea_text))``.

    Args:
      text:    The individual's ``idea_text``.
      others:  Iterable of OTHER individuals (caller is responsible for
               excluding the one being scored; including self always returns
               novelty=0 which would skew the Pareto front).

    Returns:
      1.0 if there are no others (singleton population → maximal novelty).
      Otherwise 1.0 minus the highest Jaccard similarity to any other
      individual's idea_text. Always clamped to ``[0.0, 1.0]``.
    """
    self_tokens = _tokens(text)
    max_sim = 0.0
    saw_any = False
    for other in others:
        saw_any = True
        other_text = other.get("idea_text") if isinstance(other, dict) else None
        if not isinstance(other_text, str):
            continue
        sim = _jaccard_similarity(self_tokens, _tokens(other_text))
        if sim > max_sim:
            max_sim = sim
    if not saw_any:
        return 1.0
    novelty = 1.0 - max_sim
    # Clamp defensively in case of float drift.
    if novelty < 0.0:
        return 0.0
    if novelty > 1.0:
        return 1.0
    return novelty


# ---------------------------------------------------------------------------
# Fitness computation
# ---------------------------------------------------------------------------


def _accuracy_delta_from_history(verdict_history: list[dict[str, Any]]) -> float:
    """Average ``primary_delta`` over PASS verdicts. Returns 0.0 if never PASSED.

    FAIL / INCONCLUSIVE verdicts contribute nothing (their primary_delta is
    typically null or undefined). This matches NOTES.md §3: strict positive
    credit only.
    """
    deltas: list[float] = []
    for v in verdict_history or ():
        if not isinstance(v, dict):
            continue
        if v.get("decision") != "PASS":
            continue
        d = v.get("primary_delta")
        if isinstance(d, (int, float)):
            deltas.append(float(d))
    if not deltas:
        return 0.0
    return sum(deltas) / len(deltas)


def _recompute_fitness(
    individual: dict[str, Any], others: list[dict[str, Any]]
) -> None:
    """Mutate ``individual['fitness']`` in place from its history + others.

    Splits responsibility cleanly: history-derived fields (accuracy_delta,
    evaluation_count) only need the individual; novelty needs the rest of
    the pool.
    """
    history = individual.get("verdict_history") or []
    fit = individual.setdefault("fitness", {})
    fit["accuracy_delta"] = _accuracy_delta_from_history(history)
    fit["novelty"] = compute_novelty(
        individual.get("idea_text", ""),
        (o for o in others if o.get("id") != individual.get("id")),
    )
    fit["evaluation_count"] = len(history)


# ---------------------------------------------------------------------------
# ID generation
# ---------------------------------------------------------------------------


def new_individual_id(idea_text: str) -> str:
    """Generate a stable individual ID: ``ind_<utc_ts>_<short_hash>``.

    Hash is derived from idea_text so two individuals seeded with identical
    text at the same UTC second still collide (intentional — duplicates are
    not added; see ``Population.add``).
    """
    ts = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime())
    h = hashlib.sha256((idea_text or "").encode("utf-8")).hexdigest()[:6]
    return f"ind_{ts}_{h}"


# ---------------------------------------------------------------------------
# Population store
# ---------------------------------------------------------------------------


class Population:
    """In-memory mirror of ``autoresearch/gepa_population.jsonl``.

    The file is append-only on disk (one JSON object per line, no header).
    In-memory operations may mutate individuals; call ``save()`` to flush
    the whole file. Concurrent writers are not supported — the autoresearch
    loop is strictly serial.
    """

    def __init__(self, path: Path | str) -> None:
        self._path = Path(path)
        self._individuals: list[dict[str, Any]] = []

    # -- I/O --

    @property
    def path(self) -> Path:
        return self._path

    def load(self) -> None:
        """Read the JSONL file into memory. Skips malformed lines silently.

        No-op if the file doesn't exist (fresh-population case). Always
        clears the in-memory list first so repeated load() calls are
        idempotent.
        """
        self._individuals = []
        if not self._path.is_file():
            return
        text = self._path.read_text(encoding="utf-8")
        for raw in text.splitlines():
            raw = raw.strip()
            if not raw:
                continue
            try:
                obj = json.loads(raw)
            except json.JSONDecodeError:
                continue
            if isinstance(obj, dict) and isinstance(obj.get("id"), str):
                self._individuals.append(obj)

    def save(self) -> None:
        """Atomic rewrite of the JSONL file from the current in-memory list."""
        self._path.parent.mkdir(parents=True, exist_ok=True)
        tmp = self._path.with_suffix(self._path.suffix + ".tmp")
        with tmp.open("w", encoding="utf-8") as fh:
            for ind in self._individuals:
                fh.write(json.dumps(ind, sort_keys=True) + "\n")
        tmp.replace(self._path)

    # -- Access --

    def all(self) -> list[dict[str, Any]]:
        """Return a shallow copy of the in-memory individual list."""
        return list(self._individuals)

    def get(self, ind_id: str) -> dict[str, Any] | None:
        for ind in self._individuals:
            if ind.get("id") == ind_id:
                return ind
        return None

    # -- Mutation --

    def add(self, individual: dict[str, Any]) -> None:
        """Append a new individual to the pool.

        Recomputes the new individual's fitness against the rest of the
        pool (so its novelty is correct on first read) AND recomputes
        every existing individual's novelty (the new arrival changed the
        denominator). Caller is responsible for calling ``save()`` afterward
        if persistence is desired — ``add`` is in-memory only.

        Silently no-ops if an individual with the same ``id`` is already
        present (prevents duplicate appends across re-evaluation passes).
        """
        ind_id = individual.get("id")
        if not isinstance(ind_id, str) or not ind_id:
            raise ValueError("individual must have a non-empty string 'id'")
        if any(x.get("id") == ind_id for x in self._individuals):
            return  # already present; treat as no-op
        # Ensure required fields exist before we trust the rest of the API.
        individual.setdefault("verdict_history", [])
        individual.setdefault("parent_id", None)
        individual.setdefault("mutation_rationale", None)
        individual.setdefault("fitness", {})
        self._individuals.append(individual)
        # Recompute every individual's fitness now that the pool changed.
        for ind in self._individuals:
            _recompute_fitness(ind, self._individuals)

    def record_verdict(self, ind_id: str, verdict_dict: dict[str, Any]) -> None:
        """Append a verdict entry to ``ind_id`` and recompute its fitness.

        Silently no-ops if ``ind_id`` is not found (defensive — the loop
        should never call with an unknown ID, but we don't want a typo to
        crash mid-iteration).
        """
        ind = self.get(ind_id)
        if ind is None:
            return
        history = ind.setdefault("verdict_history", [])
        history.append(dict(verdict_dict))
        # Only this individual's accuracy_delta changes; novelty is
        # text-derived and unaffected by verdict updates.
        _recompute_fitness(ind, self._individuals)

    # -- Pareto front --

    def pareto_front(self) -> list[dict[str, Any]]:
        """Return the non-dominated set across ``(accuracy_delta, novelty)``.

        An individual A dominates B iff:
            A.accuracy_delta >= B.accuracy_delta AND A.novelty >= B.novelty
        AND
            A.accuracy_delta >  B.accuracy_delta OR  A.novelty >  B.novelty
        (i.e. weakly better on both axes and strictly better on at least one).

        Tiebreak: when two individuals have IDENTICAL axis values, only the
        one with the higher ``evaluation_count`` survives (prefer
        better-tested individuals — exploitation bias matches NOTES.md §3).
        """
        inds = self._individuals
        if not inds:
            return []

        def fit(ind: dict[str, Any]) -> tuple[float, float, int]:
            f = ind.get("fitness") or {}
            return (
                float(f.get("accuracy_delta", 0.0) or 0.0),
                float(f.get("novelty", 0.0) or 0.0),
                int(f.get("evaluation_count", 0) or 0),
            )

        front: list[dict[str, Any]] = []
        for cand in inds:
            ca, cn, ce = fit(cand)
            dominated = False
            for other in inds:
                if other is cand:
                    continue
                oa, on, oe = fit(other)
                # Strict Pareto: ``other`` weakly-better on both, strictly
                # better on at least one — ``cand`` is dominated.
                if oa >= ca and on >= cn and (oa > ca or on > cn):
                    dominated = True
                    break
                # Identical axes tiebreak — prefer the better-tested one.
                if oa == ca and on == cn and oe > ce:
                    dominated = True
                    break
            if not dominated:
                front.append(cand)
        return front
