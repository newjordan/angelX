#!/usr/bin/env python3
"""Head-to-head XY plots: glass micro-wins + kitty encoder floor.
Fresh numbers 2026-10-26, host load ~25-30 (ratios hold, absolutes noisy).
Glass: rustc -O black_box (h2h rig). Kitty: kitty_gfx_bench.py pixel formula.
Glass render p50s are standing Board A (docs/RENDER_SCORECARD.md).
"""
from PIL import Image, ImageDraw

W, H = 1600, 1000
BG = (13, 17, 23); PANEL = (22, 27, 34); GRID = (48, 54, 63)
TXT = (230, 237, 245); DIM = (139, 148, 158)
SIP = (248, 81, 73); FX = (63, 185, 80)
B64 = (248, 81, 73); PNGC = (210, 153, 34); GLASS = (63, 185, 80)

img = Image.new("RGB", (W, H), BG)
d = ImageDraw.Draw(img)

def text(xy, s, fill=TXT, anchor="la"):
    d.text(xy, s, fill=fill, anchor=anchor)

# title
text((40, 24), "glass vs kitty — head-to-head, fresh 2026-10-26 (load ~25-30, ratios hold)", fill=TXT)
text((40, 48), "left: symbol_hash ns/op by symbol bytes (SipHash vs Fx)  |  right: frame pixels vs ms (kitty encoder cost vs glass presented render)", fill=DIM)

def panel(x0, y0, x1, y1, title, sub):
    d.rectangle([x0, y0, x1, y1], fill=PANEL, outline=GRID)
    text((x0 + 16, y0 + 12), title)
    text((x0 + 16, y0 + 30), sub, fill=DIM)

def xmap(x, xmin, xmax, px0, px1):
    return px0 + (x - xmin) / (xmax - xmin) * (px1 - px0)

def ymap(y, ymin, ymax, py0, py1):
    return py1 - (y - ymin) / (ymax - ymin) * (py1 - py0)

import math
def lxmap(x, lmin, lmax, px0, px1):
    lx, a, b = math.log10(x), math.log10(lmin), math.log10(lmax)
    return px0 + (lx - a) / (b - a) * (px1 - px0)
def lymap(y, lmin, lmax, py0, py1):
    ly, a, b = math.log10(max(y, 1e-9)), math.log10(lmin), math.log10(lmax)
    return py1 - (ly - a) / (b - a) * (py1 - py0)

# ---- LEFT: hash ns vs bytes (linear x, linear y) ----
LX0, LY0, LX1, LY1 = 40, 110, 770, 920
PX0, PY0, PX1, PY1 = LX0 + 70, LY0 + 60, LX1 - 30, LY1 - 70
panel(LX0, LY0, LX1, LY1, "symbol_hash: ns/op vs symbol bytes", "lower is better · Fx (green) beats SipHash (red) on every symbol")
hash_rows = [("e", 1, 4.9, 0.7), ("ab", 2, 6.4, 0.9), ("─", 3, 5.3, 1.1),
             ("│", 3, 5.3, 1.1), ("😀", 4, 4.8, 1.4), ("hello", 5, 4.9, 1.7),
             ("全15B", 15, 6.6, 4.9)]
xmin, xmax, ymin, ymax = 0, 16, 0, 7.5
for gv in [0, 1, 2, 3, 4, 5, 6, 7]:
    y = ymap(gv, ymin, ymax, PY0, PY1)
    d.line([PX0, y, PX1, y], fill=GRID)
    text((PX0 - 8, y), f"{gv}", fill=DIM, anchor="rm")
for xv in [0, 2, 4, 6, 8, 10, 12, 14, 16]:
    x = xmap(xv, xmin, xmax, PX0, PX1)
    d.line([x, PY0, x, PY1], fill=GRID)
    text((x, PY1 + 8), f"{xv}B", fill=DIM, anchor="ma")
text((PX0 - 52, (PY0 + PY1) // 2), "ns/op", fill=DIM, anchor="mm")
text(((PX0 + PX1) // 2, PY1 + 28), "symbol bytes", fill=DIM, anchor="mm")
# lines
pts_s = [(xmap(b, xmin, xmax, PX0, PX1), ymap(s, ymin, ymax, PY0, PY1)) for _, b, s, _ in hash_rows]
pts_f = [(xmap(b, xmin, xmax, PX0, PX1), ymap(f, ymin, ymax, PY0, PY1)) for _, b, _, f in hash_rows]
d.line(pts_s, fill=SIP, width=3); d.line(pts_f, fill=FX, width=3)
for (sym, b, s, f), ps, pf in zip(hash_rows, pts_s, pts_f):
    for p, c in ((ps, SIP), (pf, FX)):
        d.ellipse([p[0]-5, p[1]-5, p[0]+5, p[1]+5], fill=c)
    # speedup label near fx point
    text((pf[0] + 8, pf[1] - 10), f"{s/f:.1f}x", fill=FX)
text((PX0 + 12, PY0 + 10), "● SipHash (before)", fill=SIP)
text((PX0 + 12, PY0 + 28), "● FxHash (glass, in-tree)", fill=FX)

# ---- RIGHT: frame pixels vs ms (log-log) ----
RX0, RY0, RX1, RY1 = 790, 110, 1560, 920
QX0, QY0, QX1, QY1 = RX0 + 70, RY0 + 60, RX1 - 30, RY1 - 70
panel(RX0, RY0, RX1, RY1, "frame cost: ms vs pixels (log-log)", "kitty pays encoder cost before parse; glass ends at presented frame")
kitty_b64 = [(25600, 0.05), (30720, 0.06), (64000, 0.16), (67200, 0.13),
             (2073600, 11.6), (8294400, 19.2)]
kitty_png = [(25600, 1.25), (30720, 1.34), (64000, 2.08), (67200, 1.42)]
glass_render = [(2073600, 1.00), (8294400, 2.03)]
lmin, lmax, mmin, mmax = 20000, 10000000, 0.03, 40.0
for exp in [4, 5, 6, 7]:
    v = 10 ** exp
    if lmin <= v <= lmax:
        x = lxmap(v, lmin, lmax, QX0, QX1)
        d.line([x, QY0, x, QY1], fill=GRID)
        lbl = f"{v//1000}K" if v < 10**6 else f"{v//10**6}M"
        text((x, QY1 + 8), lbl, fill=DIM, anchor="ma")
for mv in [0.05, 0.1, 0.5, 1, 5, 10, 20]:
    y = lymap(mv, mmin, mmax, QY0, QY1)
    d.line([QX0, y, QX1, y], fill=GRID)
    text((QX0 - 8, y), f"{mv:g}", fill=DIM, anchor="rm")
text((QX0 - 52, (QY0 + QY1) // 2), "ms", fill=DIM, anchor="mm")
text(((QX0 + QX1) // 2, QY1 + 28), "pixels (log)", fill=DIM, anchor="mm")
def dot(pt, c):
    d.ellipse([pt[0]-6, pt[1]-6, pt[0]+6, pt[1]+6], fill=c)
pb = [(lxmap(x, lmin, lmax, QX0, QX1), lymap(y, mmin, mmax, QY0, QY1)) for x, y in kitty_b64]
pp = [(lxmap(x, lmin, lmax, QX0, QX1), lymap(y, mmin, mmax, QY0, QY1)) for x, y in kitty_png]
pg = [(lxmap(x, lmin, lmax, QX0, QX1), lymap(y, mmin, mmax, QY0, QY1)) for x, y in glass_render]
d.line(pb, fill=B64, width=3); d.line(pp, fill=PNGC, width=2); d.line(pg, fill=GLASS, width=3)
for p in pb: dot(p, B64)
for p in pp: dot(p, PNGC)
for p in pg:
    d.ellipse([p[0]-8, p[1]-8, p[0]+8, p[1]+8], outline=GLASS, width=3)
text((pb[-1][0] - 10, pb[-1][1] - 22), "kitty b64 19.2ms @4K", fill=B64, anchor="rb")
text((pg[-1][0] - 10, pg[-1][1] + 18), "glass 2.0ms @4K", fill=GLASS, anchor="rt")
text((pb[-2][0] - 10, pb[-2][1] - 20), "kitty b64 11.6ms @1080p", fill=B64, anchor="rb")
text((pg[-2][0] + 12, pg[-2][1] + 4), "glass 1.0ms @1080p", fill=GLASS)
text((QX0 + 12, QY0 + 10), "● kitty b64 encode (fresh today)", fill=B64)
text((QX0 + 12, QY0 + 28), "● kitty PNG encode (fresh today)", fill=PNGC)
text((QX0 + 12, QY0 + 46), "○ glass presented render p50 (Board A)", fill=GLASS)

# footer
text((40, 936), "glass wins: hash 1.4-6.9x · width-ascii 13x · bg-key 2.9x · presented 1080p ~6x / 4K ~10x vs kitty b64 encode alone  |  nulls: palette, LUT tie", fill=DIM)
text((1560, 936), "angel-glass 120/120 green", fill=DIM, anchor="ra")

out = "reports/glass-vs-kitty-h2h.png"
img.save(out)
print(f"wrote {out} {W}x{H}")
