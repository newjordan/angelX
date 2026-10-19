#!/usr/bin/env python3
"""Golden-frame contract check (scorecard M8, pinned as a regression test).

Glass composites in sRGB space — the browser convention; Kitty in
linear light. That divergence is intentional and documented in
docs/RENDER_SCORECARD.md. This check pins our side of it: given a
captured frame of stream-bench's `golden` scene (red alpha ramp +
green/blue checker), assert

  * the checker colours are exact (220 channels, ±3), and
  * the ramp composites in encoded space: red(t) = 255*t +
    bg*(1-t) within ±6, bg sampled from the frame itself.

A linear-light composite misses the mid-ramp by up to ~58/255, so
this fails loudly if the blending convention ever changes by
accident (surface format, texture view, premultiply path).

Usage: golden_check.py CAPTURE.png   (exit 0 pass, 1 fail, 2 usage)
Produce the capture on the nested seat:
  STREAM_BENCH_MODE=golden STREAM_BENCH_SECONDS=3 STREAM_BENCH_FPS=60 \
  STREAM_BENCH_CAPTURE=/tmp/golden-capture.png \
  crates/target/release/examples/stream-bench
"""
import json
import sys
from collections import Counter

from PIL import Image

SAMPLE_T = (0.25, 0.375, 0.5, 0.625, 0.75, 0.875)
CHECKER_TOL = 3
RAMP_TOL = 6


def locate_checker(px, w, h):
    xs, ys = [], []
    for y in range(0, h, 2):
        for x in range(0, w, 2):
            r, g, b = px[x, y][:3]
            if (g > 180 and r < 60 and b < 60) or (b > 180 and r < 60 and g < 60):
                xs.append(x)
                ys.append(y)
    if not xs:
        return None
    return min(xs), max(xs), min(ys), max(ys)


def background(px, w, h, box):
    x0, x1, y0, y1 = box
    candidates = []
    for y in range(0, h, 7):
        for x in range(0, w, 7):
            if x0 - (x1 - x0) - 8 <= x <= x1 + 8 and y0 - 8 <= y <= y1 + 8:
                continue
            candidates.append(px[x, y][:3])
    return Counter(candidates).most_common(1)[0][0]


def check(path):
    im = Image.open(path).convert("RGB")
    px = im.load()
    w, h = im.size
    box = locate_checker(px, w, h)
    if box is None:
        return {"ok": False, "error": "checker not found"}
    cx0, cx1, cy0, cy1 = box
    bg = background(px, w, h, box)
    ymid = (cy0 + cy1) // 2
    checker_w = cx1 - cx0
    gx0 = cx0 - checker_w

    failures = []
    # Checker cells are 16 source px; sample two cell centres.
    cell = checker_w / 10.0
    for label, frac, want in (
        ("checker_a", 0.5, (0, 0, 220)),
        ("checker_b", 1.5, (0, 220, 0)),
    ):
        x = int(cx0 + frac * cell)
        y = int(cy0 + 0.5 * (cy1 - cy0) / 12.5)  # first checker row
        got = px[x, y][:3]
        if any(abs(g - wv) > CHECKER_TOL for g, wv in zip(got, want)):
            failures.append(f"{label}: got {got}, want {want}")

    ramp = []
    for t in SAMPLE_T:
        x = int(gx0 + t * checker_w)
        got = px[x, ymid][0]
        want = 255.0 * t + bg[0] * (1.0 - t)
        ramp.append({"t": t, "got": got, "want_srgb": round(want, 1)})
        if abs(got - want) > RAMP_TOL:
            failures.append(f"ramp t={t}: red {got}, sRGB-blend want {want:.0f}")

    return {
        "ok": not failures,
        "background": list(bg),
        "checker_box": [cx0, cy0, cx1, cy1],
        "ramp": ramp,
        "failures": failures,
    }


def main(argv):
    if len(argv) != 2:
        print(__doc__)
        return 2
    result = check(argv[1])
    print(json.dumps(result, indent=1))
    return 0 if result["ok"] else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
