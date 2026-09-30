#!/usr/bin/env python3
"""decide_table.py out/decide-*.jsonl > decide.json — per decision point, model
and arm: how often the next move was the route's action (with a Wilson 95%
interval), what the next moves were, and the request's tokens and time."""
import collections, json, math, statistics, sys

SCENARIOS = ("batch", "batch2", "untested", "finish")
ARMS = ("prebook", "nocue", "bare", "every", "intro")
MODELS = ("glm-flash", "deepseek-flash", "muse", "grok")


def wilson(k, n, z=1.96):
    if not n:
        return None, None
    p = k / n
    centre = (p + z * z / (2 * n)) / (1 + z * z / n)
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / (1 + z * z / n)
    return max(0.0, centre - half), min(1.0, centre + half)


def med(values):
    values = [v for v in values if v is not None]
    return statistics.median(values) if values else None


def move(r):
    """The reply's next move: answer, or its calls with a ledger read named as one."""
    if not r["calls"]:
        return "answer"
    names = ["read ledger" if "ledger://" in a else c for c, a in zip(r["calls"], r["args"] + [""] * len(r["calls"]))]
    return " + ".join(f"{n} ×{names.count(n)}" if names.count(n) > 1 else n for n in sorted(set(names)))


rows = [json.loads(line) for path in sys.argv[1:] for line in open(path)]
cells = collections.defaultdict(list)
for r in rows:
    cells[(r["scenario"], r["model"], r["arm"])].append(r)
out = []
for scenario in SCENARIOS:
    for model in MODELS:
        for arm in ARMS:
            rs = cells.get((scenario, model, arm))
            if not rs:
                continue
            ok = [r for r in rs if not r["error"]]
            k = sum(bool(r["match"]) for r in ok)
            lo, hi = wilson(k, len(ok))
            moves = collections.Counter(move(r) for r in ok)
            decoded = sum("ledger://" in "".join(r["args"]) for r in ok)
            out.append({"scenario": scenario, "model": model, "arm": arm, "n": len(ok), "errors": len(rs) - len(ok),
                        "match": k, "decoded": decoded, "rate": k / len(ok) if ok else None, "lo": lo, "hi": hi,
                        "moves": moves.most_common(),
                        "prompt": med([r["prompt"] for r in ok]), "completion": med([r["completion"] for r in ok]),
                        "reasoning": med([r["reasoning"] for r in ok]), "secs": med([r["secs"] for r in ok])})
json.dump(out, sys.stdout, indent=1, ensure_ascii=False)
for c in out:
    print(f"{c['scenario']:8} {c['model']:14} {c['arm']:7} {c['match']:2}/{c['n']:<2} dec={c['decoded']} "
          f"[{c['lo']:.2f},{c['hi']:.2f}] prompt={c['prompt']} out={c['completion']} {c['secs']}s "
          f"{c['moves'][:3]}", file=sys.stderr)
