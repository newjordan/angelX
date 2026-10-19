#!/usr/bin/env python3
"""Scorecard v2 driver: paired, order-randomized repetitions of the
pre-registered races, then bootstrap verdicts (analyze.py).

  SCORECARD_RUN=/work/tmp/render-scorecard/v2-1 \
    python3 scripts/bench/render/run_v2.py [reps] [angel-binary]

Randomization is seeded (recorded in the run dir), so the schedule
itself is reproducible. Kitty runs steelmanned (kitty-steel.conf via
run_scorecard.sh); glass numbers are presented-frame durations.
"""
import glob
import json
import os
import random
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
RUNNER = os.path.join(HERE, "run_scorecard.sh")
sys.path.insert(0, HERE)
from analyze import summarize  # noqa: E402

REPS = int(sys.argv[1]) if len(sys.argv) > 1 else 5
ANGEL = sys.argv[2] if len(sys.argv) > 2 else ""
RUN = os.environ["SCORECARD_RUN"]
os.makedirs(RUN, exist_ok=True)
SEED = 20261017
rng = random.Random(SEED)
with open(os.path.join(RUN, "schedule.txt"), "a") as f:
    f.write(f"run_v2 reps={REPS} seed={SEED}\n")


def run(tag, *args, env=None):
    full = dict(os.environ)
    full["SCORECARD_TAG"] = tag
    if env:
        full.update(env)
    print(f"== {tag} {' '.join(args)}", flush=True)
    subprocess.run([RUNNER, *args], env=full, check=False,
                   stdout=subprocess.DEVNULL)


def kitty_times(pattern):
    out = []
    for path in sorted(glob.glob(os.path.join(RUN, pattern))):
        out += json.load(open(path))["times_ms"]
    return out


def glass_frames(pattern):
    out = []
    for path in sorted(glob.glob(os.path.join(RUN, pattern))):
        out += json.load(open(path))["frames_ms"]
    return out


# (metric, kitty-args, glass-args, kitty-glob, glass-glob)
RACES = [
    ("M2-blast4", ["kitty", "blast4", "--transport", "shm"],
     ["glass", "blast4"], "kitty-blast4-*-r*.json", "glass-blast4-immediate-r*.series.json"),
    ("M3-1080p", ["kitty", "big", "--w", "1920", "--h", "1080", "--transport", "shm", "--frames", "120"],
     ["glass", "big", "STREAM_BENCH_W=1920", "STREAM_BENCH_H=1080"],
     "kitty-big-w-1920-*-r*.json", "glass-big-immediate-r*.series.json"),
    ("M4-placements-1000", ["kitty", "placements", "--n", "1000", "--frames", "120"],
     ["glass", "placements", "STREAM_BENCH_N=1000", "STREAM_BENCH_SECONDS=10"],
     "kitty-placements-n-1000-*-r*.json", "glass-placements-immediate-r*.series.json"),
    ("M6-churn-256", ["kitty", "churn", "--n", "256", "--frames", "120"],
     ["glass", "churn", "STREAM_BENCH_N=256", "STREAM_BENCH_SECONDS=10"],
     "kitty-churn-*-r*.json", "glass-churn-immediate-r*.series.json"),
]

for rep in range(1, REPS + 1):
    for name, kargs, gargs, _kg, _gg in RACES:
        order = ["kitty", "glass"]
        rng.shuffle(order)
        with open(os.path.join(RUN, "schedule.txt"), "a") as f:
            f.write(f"{name} rep{rep} order={order} load={os.getloadavg()}\n")
        for side in order:
            run(f"-r{rep}", *(kargs if side == "kitty" else gargs))

# M1: keyframe, alternating start side, up to 3 reps.
if ANGEL:
    for rep in range(1, 4):
        order = "kitty glass" if rep % 2 else "glass kitty"
        run(f"-r{rep}", "keyframe", ANGEL, env={"KF_ORDER": order})

verdicts = []
for name, _k, _g, kg, gg in RACES:
    verdicts.append(summarize(name, glass_frames(gg), kitty_times(kg)))

if ANGEL:
    def kf_series(side):
        vals = []
        for path in sorted(glob.glob(os.path.join(RUN, f"keyframe-{side}-r*.txt"))):
            text = open(path).read()
            if "raw_ms=[" in text:
                vals += [float(x) for x in
                         text.split("raw_ms=[", 1)[1].split("]", 1)[0].split(",")]
        return vals
    verdicts.insert(0, summarize("M1-keyframe", kf_series("glass"), kf_series("kitty")))

with open(os.path.join(RUN, "verdicts.json"), "w") as f:
    json.dump(verdicts, f, indent=1)
for v in verdicts:
    print(f"{v['metric']}: glass={v['glass_median']} kitty={v['kitty_median']} "
          f"diff={v['diff_median']} CI95={v['ci95']} -> {v['verdict']}")
