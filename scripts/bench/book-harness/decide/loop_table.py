#!/usr/bin/env python3
"""loop_table.py <replay.jsonl>... — per cue: loops broken, the move, and the
median prompt-token cost over raw at the same point (same task, depth, sample)."""
import json, sys, collections

rows = [json.loads(line) for path in sys.argv[1:] for line in open(path)]
rows = [r for r in rows if "error" not in r]
base = {(r["task"], r.get("loop_at", 5), r["i"]): r["prompt_tokens"] for r in rows if r["cue"] == "raw"}
for depth in sorted({r.get("loop_at", 5) for r in rows}):
    print(f"\nafter repeat {depth}")
    print(f"{'cue':<13}{'broke':>9}{'edit':>6}{'other':>7}{'answer':>8}{'cost (tok)':>12}")
    cues = []
    for r in rows:
        if r.get("loop_at", 5) == depth and r["cue"] not in cues:
            cues.append(r["cue"])
    for cue in cues:
        rs = [r for r in rows if r["cue"] == cue and r.get("loop_at", 5) == depth]
        moves = collections.Counter(r["move"] for r in rs)
        cost = sorted(r["prompt_tokens"] - base[(r["task"], depth, r["i"])]
                      for r in rs if (r["task"], depth, r["i"]) in base)
        print(f"{cue:<13}{sum(r['broke'] for r in rs):>5}/{len(rs):<3}{moves['edit']:>6}{moves['other_call']:>7}"
              f"{moves['answer']:>8}{(cost[len(cost) // 2] if cost else '—'):>12}")
