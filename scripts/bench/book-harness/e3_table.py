#!/usr/bin/env python3
"""e3_table.py — E3 and E4 per model and arm, from out/e3-<route>-<arm>-<n>.*

E3: sessions, iterations reached, requests, prompt tokens (cached share), the
⡪ routes raised / judged / matched (route_action.py) and the match rate.
Heesch: the hc_verified of each session's final submission/best.heesch
(re-verified with HEESCH's venv), and whether it changed from the frontier.
E4 (intro arm): loop-iteration observations in the learner's state against
the iterations the loop record verified."""
import collections
import glob
import json
import os
import re
import subprocess

import route_action

HEESCH = os.environ.get("HEESCH", os.path.expanduser("~/comps/heesch"))
FRONTIER = open(os.path.join(HEESCH, "submission/best.heesch")).read()


def hc_of(path):
    if not os.path.exists(path):
        return None
    if open(path).read() == FRONTIER:
        return "frontier"
    run = subprocess.run([os.path.join(HEESCH, ".venv-bench/bin/python"), "-m", "heesch_verify", path],
                         capture_output=True, text=True, cwd=HEESCH)
    try:
        d = json.loads(run.stdout)
    except ValueError:
        return "rejected"
    return d.get("hc_verified", d.get("error", "?"))


def learner_observations(state_dir):
    n = 0
    for path in glob.glob(os.path.join(state_dir, "learning-*.json")):
        text = open(path).read()
        n += text.count('"loop-iteration"')
    return n


sessions = collections.defaultdict(list)
for log in sorted(glob.glob("out/e3-*-*-[0-9]*.jsonl")):
    m = re.match(r"out/e3-(.+)-(intro|bare)-(\d+)\.jsonl$", log)
    if not m or not os.path.exists(log[:-6] + ".run"):
        continue
    route, arm, n = m.groups()
    base = log[:-6]
    rows = [json.loads(line) for line in open(log)]
    prompt = cached = 0
    for r in rows:
        u = r.get("usage") or {}
        prompt += u.get("prompt_tokens") or u.get("input_tokens") or 0
        d = u.get("prompt_tokens_details") or u.get("input_tokens_details") or {}
        cached += d.get("cached_tokens") or u.get("prompt_cache_hit_tokens") or 0
    loop = {}
    for path in glob.glob(base + ".state/loops/*.json"):
        loop = json.load(open(path))
    raised, judged, matched = route_action.tally([log])
    sessions[(route, arm)].append({
        "n": n, "requests": len(rows), "prompt": prompt, "cached": cached,
        "iterations": loop.get("iteration", 0), "status": open(base + ".run").read().split()[1],
        "directions": len(loop.get("directions_tried", [])),
        "raised": raised, "judged": judged, "matched": matched,
        "hc": hc_of(base + ".state/submission/best.heesch"),
        "observed": learner_observations(base + ".state"),
    })

print("| model | arm | sessions | iterations | requests | prompt tokens (cached) | routes raised | judged | matched | match rate | final hc |")
print("|---|---|---|---|---|---|---|---|---|---|---|")
for (route, arm), ss in sorted(sessions.items()):
    raised = sum((s["raised"] for s in ss), collections.Counter())
    judged = sum(sum(s["judged"].values()) for s in ss)
    matched = sum(sum(s["matched"].values()) for s in ss)
    rate = f"{100 * matched / judged:.0f}%" if judged else "n/a"
    prompt = sum(s["prompt"] for s in ss)
    cached = sum(s["cached"] for s in ss)
    print(f"| {route} | {arm} | {len(ss)} | {', '.join(str(s['iterations']) for s in ss)} | "
          f"{sum(s['requests'] for s in ss):,} | {prompt / 1e6:.1f}M ({100 * cached / max(prompt, 1):.0f}%) | "
          f"{' '.join(f'{k}×{v}' for k, v in sorted(raised.items())) or '—'} | {judged} | {matched} | {rate} | "
          f"{', '.join(str(s['hc']) for s in ss)} |")

print("\nE4 (intro arm): learner loop-iteration observations vs loop iterations, per session")
for (route, arm), ss in sorted(sessions.items()):
    if arm == "intro":
        print(f"  {route}: " + "; ".join(f"#{s['n']} obs={s['observed']} iters={s['iterations']} dirs={s['directions']} {s['status']}" for s in ss))
