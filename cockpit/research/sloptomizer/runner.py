"""Offline Sloptomizer primitives; the Rust loop owns execution and evidence.

This is a stateless JSON transform. It never starts providers, shells, source
integration, mail or the historical autonomous controller. State publication
and evidence admission belong to the caller, which supplies real experiments.
"""

from __future__ import annotations

import copy
import hashlib
import json
import math
import os
from pathlib import Path
import random
import sys

# The executable launches Python with -I -S -B; only this bundled source tree
# and the standard library participate, regardless of the task's PYTHONPATH.
sys.path.insert(0, str(Path(__file__).resolve().parent))

from autoresearch.gepa.population import Population
from autoresearch.gepa.select import select_next
from orchestrator.micro_llm.core import Feedback, MicroLearner, RolloutExample
from orchestrator.self_improvement.bandits import ucb1_scores
from orchestrator.self_improvement.stats import SourceStats

SCHEMA = "angel.sloptomizer-options/v1"


def individual_id(idea):
    return "idea-" + hashlib.sha256(idea.encode()).hexdigest()


def execute(request):
    state = copy.deepcopy(request["state"] if request.get("state") is not None else {
        "schema": SCHEMA, "population": [], "observations": [], "learner": None,
    })
    if not isinstance(state, dict) or state.get("schema") != SCHEMA:
        raise ValueError("unsupported research state; original state was preserved")
    population = Population(Path("unused-stateless-population"))
    # Population.load silently skips broken JSONL. Rust supplies one validated
    # JSON state instead. Keep original fitness/selection without re-adding the
    # whole population (which would recompute all novelty scores at every read).
    population._individuals = state["population"]
    learner = (MicroLearner.from_dict(state["learner"])
               if state["learner"] is not None else MicroLearner())
    action = request.get("action", "suggest")
    observation = request.get("observation")
    changed = False
    if action == "observe":
        if not isinstance(observation, dict):
            raise ValueError("observe requires controller evidence")
        for key in ("id", "idea", "approach", "task", "receipt_sha256", "source_sha256"):
            if not isinstance(observation.get(key), str) or not observation[key]:
                raise ValueError(f"observation requires {key}")
        if type(observation.get("passed")) is not bool:
            raise ValueError("observation must carry a physical verifier outcome")
        previous = next((v for v in state["observations"]
                         if v["id"] == observation["id"]), None)
        if previous is not None and previous != observation:
            raise ValueError("conflicting observation identity; feedback not overwritten")
        if previous is None:
            idea = observation["idea"]
            ident = individual_id(idea)
            population.add({"id": ident, "idea_text": idea,
                            "born_iter": len(state["observations"]),
                            "approach": observation["approach"]})
            delta = observation.get("paired_delta")
            if delta is not None and (type(delta) not in (int, float)
                                      or not math.isfinite(delta) or not -1 <= delta <= 1):
                raise ValueError("paired_delta must be a measured verifier difference")
            decision = "PASS" if observation["passed"] else "FAIL"
            population.record_verdict(ident, {
                "idea_id": observation["id"], "decision": decision,
                # A successful single attempt is not an improvement estimate.
                "primary_delta": delta, "gate0_reject": False,
                "source_sha256": observation["source_sha256"],
                "receipt_sha256": observation["receipt_sha256"],
            })
            learner.observe(
                RolloutExample(prompt=observation["task"], response=idea,
                               metadata={"approach": observation["approach"]}),
                Feedback(reward=1.0 if observation["passed"] else -1.0,
                         notes=f"physical verifier {decision}; receipt {observation['id']}",
                         tags=(observation["approach"],)),
            )
            state["observations"].append(copy.deepcopy(observation))
            state["learner"] = learner.to_dict()
            changed = True
    elif action != "suggest":
        raise ValueError("unknown offline research operation")

    # Unmeasured ideas affect this suggestion only. They are never admitted as
    # successful outcomes and do not rewrite durable learning on a read.
    suggestions = copy.deepcopy(population.all())
    candidate_population = Population(Path("unused-suggestion-population"))
    candidate_population._individuals = suggestions
    for candidate in request.get("candidates", []):
        idea = candidate["idea"]
        candidate_population.add({"id": individual_id(idea), "idea_text": idea,
                                  "approach": candidate.get("approach", "direct")})
    selection = select_next(candidate_population, rng=random.Random(request.get("seed", 0)))
    stats = {}
    for candidate in candidate_population.all():
        approach = candidate.get("approach", "direct")
        stats.setdefault(approach, SourceStats(approach))
    for event in state["observations"]:
        row = stats.setdefault(event["approach"], SourceStats(event["approach"]))
        row.n_total += 1
        row.n_judged += 1
        if event["passed"]:
            row.n_pass += 1
        else:
            row.n_fail += 1
    scores = [{"approach": row.source, "score": row.score if math.isfinite(row.score) else None,
               "observations": row.n_pulls, "successes": row.n_successes, "reason": row.note}
              for row in ucb1_scores(stats)]
    methods = request.get("methods", ["pareto", "bandit", "memory"])
    if not isinstance(methods, list) or any(v not in {"pareto", "bandit", "memory"} for v in methods):
        raise ValueError("methods must select pareto, bandit and/or memory")
    # Use the trained slow token model and tiny MLP as well as fast retrieval.
    # These are scores for the model to inspect, never a forced selection.
    ranking = []
    if "memory" in methods:
        for candidate in candidate_population.all():
            row = learner.score_candidate(request.get("task", ""), candidate["idea_text"])
            ranking.append({"idea": row.text, "score": row.score,
                            "slow_score": row.slow_score, "fast_score": row.fast_score,
                            "mlp_score": row.mlp_score})
        ranking.sort(key=lambda row: row["score"], reverse=True)
        for row in ranking:
            for key in ("score", "slow_score", "fast_score", "mlp_score"):
                if not math.isfinite(row[key]):
                    row[key] = None  # e.g. an idea with no token features
    parent = selection["parent"]
    advice = {
        "engine": "sloptomizer", "observations": len(state["observations"]),
        "methods": methods, "advisory": True,
        "pareto": ({"idea": parent["idea_text"] if parent else None,
                     "approach": parent.get("approach") if parent else None,
                     "suggest_mutation": selection["should_mutate"],
                     "reason": selection["selection_reason"],
                     "frontier_size": len(candidate_population.pareto_front())}
                    if "pareto" in methods else None),
        "bandit": scores if "bandit" in methods else None,
        "memory": learner.retrieve_context(request.get("task", ""), limit=5)
                  if "memory" in methods else None,
        "memory_ranking": ranking[:5] if "memory" in methods else None,
        "evidence_note": "Past verifier outcomes guide exploration. Only paired experiments provide deltas; this does not validate an installed policy or train the provider model.",
    }
    return {"state": state, "advice": advice, "changed": changed}


if __name__ == "__main__":
    try:
        request = json.loads(Path(sys.argv[1]).read_text())
        encoded = json.dumps(execute(request), allow_nan=False, separators=(",", ":"))
        if len(sys.argv) == 3:
            # The owning Rust controller supplies a new file inside its private
            # spool. Large learner state must not pass through transcript caps.
            fd = os.open(sys.argv[2], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(fd, "w") as output:
                output.write(encoded)
        else:
            print(encoded)
    except Exception as error:
        print(f"Sloptomizer options: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(1)
