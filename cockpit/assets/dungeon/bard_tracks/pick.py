#!/usr/bin/env python3
"""Bard tracks, step 3: the excerpt of each song the game plays.

An excerpt is EXCERPT_BARS bars, starting where a drop lands (the full band
comes back after a gap) so the first bars carry the lead's hook; the lead
(tier 1) must be clearly there from the first bar. Reads `analyze.py`'s
JSON; writes `OUT_DIR/<id>.excerpt.json`.

    python3 pick.py ANALYSIS_DIR [id ...]
"""
import json
import pathlib
import sys

import numpy as np

EXCERPT_BARS = 24
MIN_FULL = 0.8
STRICT_FULL = 0.9


def pick(an):
    bar_db = np.array(an["bar_db"])
    n = len(bar_db)
    ref = np.percentile(bar_db, 90, axis=0)
    rel = bar_db - ref  # per layer, dB under the song's own loud level
    full = np.all(rel > np.array([-14.0, -14.0, -12.0]), axis=1)
    best = None
    for s in range(0, n - EXCERPT_BARS + 1):
        win = slice(s, s + EXCERPT_BARS)
        frac = full[win].mean()
        lead3 = rel[s:s + 3, 0].min()
        lead_all = rel[win, 0].mean()
        bass3 = rel[s:s + 3, 1].min()
        onset = s == 0 or not full[s - 1]
        onset_loud = rel[s, 2] > -6  # the drums are in on the first bar (the kick returns)
        if frac < MIN_FULL or lead3 < -8.0 or not full[s]:
            continue
        score = 6 * (frac >= STRICT_FULL) + 3 * frac + 0.15 * lead3 + 0.1 * lead_all + (0.6 if onset else 0.0) + (0.3 if onset_loud else 0.0)
        if best is None or score > best[0]:
            best = (score, s, frac, lead3, onset)
    if best is None:
        return None
    score, s, frac, lead3, onset = best
    return {"id": an["id"], "bpm": an["bpm"], "period_s": an["period_s"], "start_bar": int(s), "bars": EXCERPT_BARS,
            "start_s": an["bar0_s"] + s * 4 * an["period_s"], "full_frac": round(float(frac), 2),
            "lead_first3_rel_db": round(float(lead3), 1), "at_onset": bool(onset), "score": round(float(score), 2),
            "labels": "".join({"LBD": "#", "LB": "b", "L": "l", "BD": "k", "D": "d", "B": "_", "LD": "x", "-": "."}.get(l, "?")
                              for l in an["bar_labels"][s:s + EXCERPT_BARS])}


if __name__ == "__main__":
    d = pathlib.Path(sys.argv[1])
    ids = sys.argv[2:] or sorted(p.name.split(".")[0] for p in d.glob("*.json") if "excerpt" not in p.name)
    for sid in ids:
        an = json.loads((d / f"{sid}.json").read_text())
        ex = pick(an)
        if not ex:
            print(sid, "no excerpt")
            continue
        (d / f"{sid}.excerpt.json").write_text(json.dumps(ex, indent=1))
        print(sid, an["bpm"], "bpm  bar", ex["start_bar"], ex["labels"], "full", ex["full_frac"], "lead3", ex["lead_first3_rel_db"], "onset", ex["at_onset"], "score", ex["score"])
