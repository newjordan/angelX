#!/usr/bin/env python3
"""Find the composer's white block cursor in a grim shot, the same hunt
keyframe does (lower fifth, left half, all channels >= 245), and print
it as KF_CELL=x,y,w,h. Exits 1 while the box is implausibly large —
the startup intro's white blocks, not a cursor — so a caller can wait.
"""
import sys

from PIL import Image

im = Image.open(sys.argv[1]).convert("RGB")
w, h = im.size
pts = [(x, y) for y in range(h * 4 // 5, h) for x in range(0, w // 2)
       if min(im.getpixel((x, y))) >= 245]
if not pts:
    sys.exit(1)
# The densest cell-sized window, not the whole white set: the mouse
# pointer is white too and would stretch a bounding box across it.
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
# A block cursor is mostly solid (measured fill 0.57 with its glyph
# cutout); the mouse pointer is a hollow outline (0.11) in the same
# hunt region and once won the density race while the real cursor
# was still starting up (kf-imm, 2026-10-17).
if len(best) < 0.35 * cw * ch:
    sys.exit(1)
print(f"{x0},{y0},{cw},{ch}")
