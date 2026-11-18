#!/usr/bin/env python3
import sys

from PIL import Image

im = Image.open(sys.argv[1]).convert("RGB")
w, h = im.size
pts = [(x, y) for y in range(h * 4 // 5, h) for x in range(0, w // 2)
       if min(im.getpixel((x, y))) >= 245]
if not pts:
    sys.exit(1)
best = None
for (ox, oy) in pts[::7]:
    inside = [(x, y) for (x, y) in pts if ox <= x < ox + 20 and oy <= y < oy + 32]
    if best is None or len(inside) > len(best):
        best = inside
if not best or len(best) < 60:
    sys.exit(1)
xs = [p[0] for p in best]
ys = [p[1] for p in best]
x0, y0 = min(xs), min(ys)
cw, ch = max(xs) - x0 + 1, max(ys) - y0 + 1
if cw > 20 or ch > 32:
    sys.exit(1)
if len(best) < 0.35 * cw * ch:
    sys.exit(1)
print(f"{x0},{y0},{cw},{ch}")
