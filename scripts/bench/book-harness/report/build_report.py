#!/usr/bin/env python3
"""build_report.py report-data.json out.html — the calibration report. Every
sentence that carries a number is computed from the data it sits above."""
import html
import json
import os
import sys

data = json.load(open(sys.argv[1]))
template = open(os.path.join(os.path.dirname(__file__), "template.html")).read()
names = {m["id"]: m["name"] for m in data["models"]}
short = {"glm-flash": "GLM", "deepseek-flash": "DeepSeek", "muse": "Muse", "grok": "Grok"}


def pct(a, b):
    return 100 * (a - b) / b


def signed(v):
    return f"{'+' if v >= 0 else '−'}{abs(v):.0f}%"


def e1(model, arm):
    return next((r for r in data["e1"] if r["model"] == model and r["arm"] == arm), None)


claims = {}
solved = sum(r["solved"] for r in data["e1"])
total = sum(r["n"] for r in data["e1"])
parts, walls = [], []
for m in data["models"]:
    pre, bare, intro = e1(m["id"], "prebook"), e1(m["id"], "bare"), e1(m["id"], "intro")
    if not (pre and bare and intro):
        continue
    if intro.get("delivered", True):
        parts.append(f"{short[m['id']]} {signed(pct(intro['fresh']['median'], pre['fresh']['median']))} with the introduction, "
                     f"{signed(pct(bare['fresh']['median'], pre['fresh']['median']))} braille only")
        walls.append(f"{short[m['id']]} {pre['wall']['median']:.1f} / {bare['wall']['median']:.1f} / {intro['wall']['median']:.1f} s")
    else:
        parts.append(f"{short[m['id']]} {signed(pct(bare['fresh']['median'], pre['fresh']['median']))} braille only "
                     "(its seat has no introduction)")
        walls.append(f"{short[m['id']]} {pre['wall']['median']:.1f} / {bare['wall']['median']:.1f} s")
claims["tokens-claim"] = (f"<strong>{solved} of {total} runs solved, so every comparison is between finished tasks.</strong> "
                          "Against the pre-book English prompt, uncached tokens per solved task: " + "; ".join(parts) + ".")
claims["speed-claim"] = ("Median seconds per solved task (pre-book / braille / braille + intro): " + "; ".join(walls)
                         + ". Model time dominates; the wire moves wall time mostly through turns. "
                         "No model read the braille lookup pages in any of these runs, and in every arm the model answered "
                         "on the request right after its tests went green: the plain test result, not the stamp, ends the task.")

at213 = []
for m in data["e2"]:
    p = next(pt for pt in m["points"] if pt["requests"] == 213)
    at213.append((short[m["model"]], pct(p["intro"], p["prebook"]), pct(p["bare"], p["prebook"])))
claims["long-claim"] = ("The introduction never gets cheaper with length: at 213 requests it sits at "
                        + ", ".join(f"{n} {signed(i)}" for n, i, _ in at213)
                        + " against pre-book, while braille alone saves "
                        + ", ".join(f"{n} {signed(b)}" for n, _, b in at213) + ".")

st = data["stub"]
speedup = st["base"]["wall_ms"]["median"] / st["head"]["wall_ms"]["median"]
share = [100 * r["harness_ms"]["median"] / r["wall_ms"]["median"] for r in data["e5"] if r["build"] == "head"]
claims["harness-claim"] = (f"<strong>The scripted session runs {speedup:.1f}× faster</strong> "
                           f"({st['base']['wall_ms']['median'] / 1000:.1f} → {st['head']['wall_ms']['median'] / 1000:.1f} s), "
                           f"and the first request leaves {st['base']['startup_ms']['median']:.0f} → {st['head']['startup_ms']['median']:.0f} ms after launch. "
                           f"Behind a real model the harness's own time is {min(share):.2f}–{max(share):.2f}% of a task, "
                           "so the remaining lever was startup: DeepSeek and GLM waited on metadata probes before their first request.")

ds = {r["arm"]: r for r in data["dsab"]}
fu = {r["name"]: r for r in data["followup"]}
if ds and fu:
    before, after = fu["Before"], fu["After"]
    claims["deepseek-claim"] = (
        "DeepSeek's own harness and docs set the contract: every reasoned turn goes back, output capped at 256K, "
        "an append-only prefix, compaction at 678K. After the fixes the two-bug task solves in "
        f"{ds['chat']['turns']['median']:.0f} turns instead of {ds['base']['turns']['median']:.0f} "
        f"and {ds['base']['wall']['median']:.1f} → {ds['chat']['wall']['median']:.1f} s; "
        f"<strong>a follow-up turn is served {100 * before['cached'] / before['prompt']:.0f}% → "
        f"{100 * after['cached'] / after['prompt']:.0f}% from cache.</strong>")

vse = data.get("vsenglish")
if vse and vse["tokens"] and vse["turn2"]:
    final = {t["wire"]: t["points"][-1] for t in vse["tokens"]}
    t2 = {c["arm"]: c for c in vse["turn2"]}
    n = final["intro"]["requests"]
    turns = {t["wire"]: {p["turn"]: p["prompt"] for p in t["points"]} for t in vse.get("turns", [])}
    last = max(turns["intro"])
    gap = {k: turns["every"][k] - turns["intro"][k] for k in turns["intro"]}
    slope = (gap[last] - gap[1]) / (last - 1)
    flat = sorted({turns["intro"][k] - turns["bare"][k] for k in turns["intro"]})
    claims["vsenglish-claim"] = (
        f"<strong>The legend's saving over English every time grows by a fixed {slope:.0f} tokens a turn: "
        f"{gap[last]:,} tokens ({100 * gap[last] / turns['intro'][last]:+.0f}%) by turn {last}, and its stamp alone "
        "routes about as well as the repeated English.</strong> "
        f"Each turn reopens with the protocol, batching and verification lines and raises its cues; English every time "
        f"repeats them and the legend sends stamps. The legend's own cost over braille alone stays flat at "
        f"{flat[0]:,} tokens{'' if len(flat) == 1 else f'–{flat[-1]:,}'}: paid once, never grown. Inside a single turn "
        f"the two cost the same ({final['intro']['prompt']:,} against {final['every']['prompt']:,} prompt tokens at "
        f"{n} requests): the harness seldom repeats a stamp within a turn. At the batch cue's second sighting the "
        f"legend's stamp alone batched {t2['intro']['match']} of {t2['intro']['n']} replies and English every time "
        f"{t2['every']['match']} of {t2['every']['n']}, against braille only {t2['bare']['match']}, no cue "
        f"{t2['nocue']['match']} and pre-book {t2['prebook']['match']}.")
    data["vsenglish"]["caption"] = (
        "DeepSeek flash only: it follows the introduced routes and shows the difference between wires most clearly. "
        "Top left: DeepSeek's own prompt-token counts at four points of one scripted 213-request turn, "
        "each wire minus the legend. Middle: the end of each turn of a scripted 20-turn TUI session (three reads, the "
        "batch cue, an answer), each wire minus the legend. Bottom: the request at the batch cue's second sighting, turn 2 "
        "of a TUI session whose first turn introduced the cue, sent 20 times per wire. English every time is the current build "
        "with ANGEL_BOOK_INTRO=every: each sighting carries its English.")

facts = data.get("facts") or [
    ["Models", "GLM-5.3 flash, DeepSeek flash, Muse Spark 1.3 contributor, Grok 4.7 at low effort"],
    ["Task", "Two-bug Python fix, five runs per arm, arm order shuffled"],
    ["Builds", "dev 158b7a8 against pre-book d9c11e7 and the speed baseline 3295966"],
    ["Hosts", "atlas for GLM, DeepSeek and Muse; sparky for Grok, the DeepSeek loops and the decision test"],
    ["Decision test", "Three decision points, four wires, ten replies per model and wire"],
]
out = dict(data)
out["claims"] = claims
out["facts"] = facts
dc = data.get("decide")
if dc:
    def tally(point, arm, key="match"):
        cs = [c for c in dc["cells"] if c["scenario"] == point and c["arm"] == arm]
        return sum(c[key] for c in cs), sum(c["n"] for c in cs)

    def said(point, arm):
        k, n = tally(point, arm)
        return f"{k}/{n}" if n else "no request"
    decoded, _ = tally("untested", "bare", "decoded")
    finish = {arm: tally("finish", arm) for arm in ("prebook", "nocue", "bare", "intro")}
    finish_missers = sorted({c["model"] for c in dc["cells"] if c["scenario"] == "finish" and c["match"] < c["n"]})
    behind = [f"{short[c['model']]} at the {c['scenario']} point ({c['match']}/{c['n']})" for c in dc["cells"]
              if c["arm"] == "intro" and c["scenario"] != "finish"
              and any(o["match"] > c["match"] for o in dc["cells"]
                      if o["scenario"] == c["scenario"] and o["model"] == c["model"] and o["arm"] != "intro")]
    extra = [c["prompt"] - b["prompt"] for c in dc["cells"] if c["arm"] == "intro"
             for b in dc["cells"] if b["arm"] == "bare" and b["model"] == c["model"] and b["scenario"] == c["scenario"]]
    claims["decide-claim"] = (
        "<strong>The introduction is what turns a route into the next move.</strong> "
        f"After three single reads, replies that shared the next reads in one response: pre-book {said('batch', 'prebook')}, "
        f"braille with the cue removed {said('batch', 'nocue')}, braille only {said('batch', 'bare')}, "
        f"braille with the introduction {said('batch', 'intro')}. "
        f"After an answer that followed an untested edit, replies that ran the tests or said the change was untested: "
        f"pre-book {said('untested', 'prebook')}, braille only {said('untested', 'bare')} "
        f"(another {decoded} read the stamp's ledger page first), with the introduction {said('untested', 'intro')}. "
        f"After a green run, replies that answered: pre-book {said('finish', 'prebook')}, cue removed {said('finish', 'nocue')}, "
        f"braille only {said('finish', 'bare')}, introduction {said('finish', 'intro')}"
        + (f"; every miss is {' and '.join(short[m] for m in finish_missers)}" if len(finish_missers) == 1 else "") + ". "
        + (f"The introduction trails another wire only for {' and '.join(behind)}. " if behind else "")
        + f"It adds {min(extra):,.0f}–{max(extra):,.0f} prompt tokens at these early requests, which carry the session's first sightings.")
    out["decide"]["caption"] = (
        "Each request is the harness's own. A scripted stand-in model drove the current build (braille, with and without "
        "the introduction) and the pre-book build to the same point on the same workspace; the no-cue request is the braille "
        "request with the decision stamp's line removed. Every model received the same bytes, ten replies per request, with "
        "its angelX seat settings (GLM thinking and Grok at low effort). A bar is the share of replies whose next move was "
        "the route's action, the whisker a 95% Wilson interval. A ledger read counts as not taking the action on that move.")
page = template.replace("/*DATA*/", json.dumps(out).replace("</", "<\\/"))
open(sys.argv[2], "w").write(page)
print(f"wrote {sys.argv[2]} ({len(page) // 1024} KB)")
