#!/usr/bin/env python3
"""A tiny pixel-art workbench for Lanternward's painted slots.

Parts are masks painted back to front in the realm's inks (`PAL` is read from
render/overworld/ink.rs, so a character here is the same colour the stage
draws). A part is lit from the upper left: its up-left rim takes the lit ink,
its down-right rim the shadow ink, and the middle a dithered fall between.
`Sprite.finish` outlines the whole silhouette in the darkest ink.
"""
import math
import re
from pathlib import Path

INK_RS = Path(__file__).resolve().parents[3] / "src/render/overworld/ink.rs"


def load_palette():
    pal = {}
    text = INK_RS.read_text()
    body = text[text.index("pub fn ink(ch: char)"):]
    body = body[: body.index("\n}\n")]
    for m in re.finditer(r"'(.)' => 0x([0-9a-f]{6})", body):
        v = int(m.group(2), 16)
        pal[m.group(1)] = ((v >> 16) & 255, (v >> 8) & 255, v & 255)
    return pal


PAL = load_palette()

BAYER = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]


def bayer(x, y):
    return (BAYER[y & 3][x & 3] + 0.5) / 16.0


# ---- masks -----------------------------------------------------------------

def ell(cx, cy, rx, ry):
    out = set()
    for y in range(int(cy - ry - 1), int(cy + ry + 2)):
        for x in range(int(cx - rx - 1), int(cx + rx + 2)):
            if ((x + 0.5 - cx) / (rx + 0.0001)) ** 2 + ((y + 0.5 - cy) / (ry + 0.0001)) ** 2 <= 1.0:
                out.add((x, y))
    return out


def rect(x, y, w, h):
    return {(i, j) for j in range(y, y + h) for i in range(x, x + w)}


def poly(pts):
    """Even-odd scanline fill of a polygon given in pixel coordinates."""
    out = set()
    ys = [p[1] for p in pts]
    for y in range(int(math.floor(min(ys))), int(math.ceil(max(ys))) + 1):
        yc = y + 0.5
        xs = []
        n = len(pts)
        for i in range(n):
            (x0, y0), (x1, y1) = pts[i], pts[(i + 1) % n]
            if (y0 <= yc < y1) or (y1 <= yc < y0):
                xs.append(x0 + (yc - y0) * (x1 - x0) / (y1 - y0))
        xs.sort()
        for a, b in zip(xs[0::2], xs[1::2]):
            for x in range(int(math.floor(a + 0.5)), int(math.floor(b + 0.5))):
                out.add((x, y))
    return out


def cap(x0, y0, x1, y1, r0, r1=None):
    """A tapered capsule: a thick line from (x0, y0) to (x1, y1)."""
    r1 = r0 if r1 is None else r1
    out = set()
    steps = max(2, int(math.hypot(x1 - x0, y1 - y0) * 2))
    for i in range(steps + 1):
        t = i / steps
        out |= ell(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, r0 + (r1 - r0) * t, r0 + (r1 - r0) * t)
    return out


def line(x0, y0, x1, y1):
    out = set()
    steps = max(1, int(max(abs(x1 - x0), abs(y1 - y0)) * 2))
    for i in range(steps + 1):
        t = i / steps
        out.add((int(math.floor(x0 + (x1 - x0) * t + 0.5)), int(math.floor(y0 + (y1 - y0) * t + 0.5))))
    return out


def shift(mask, dx, dy):
    return {(x + dx, y + dy) for x, y in mask}


def mirror(mask, w):
    return {(w - 1 - x, y) for x, y in mask}


# ---- the canvas ------------------------------------------------------------

class Sprite:
    def __init__(self, w, h):
        self.w, self.h = w, h
        self.px = {}

    def clip(self, mask):
        return {(x, y) for x, y in mask if 0 <= x < self.w and 0 <= y < self.h}

    def fill(self, mask, ch):
        for p in self.clip(mask):
            self.px[p] = ch

    def put(self, x, y, ch):
        if 0 <= x < self.w and 0 <= y < self.h:
            self.px[(x, y)] = ch

    def paint(self, mask, ramp, rim=True, fall=True, seed=0, soft=0.0):
        """Paint `mask` lit from the upper left. `ramp` runs dark to light:
        [shadow, base, lit] or [shadow, base, lit, high]."""
        mask = self.clip(mask)
        if not mask:
            return
        xs = [p[0] for p in mask]
        ys = [p[1] for p in mask]
        x0, x1, y0, y1 = min(xs), max(xs), min(ys), max(ys)
        sh, ba, li = ramp[0], ramp[1], ramp[2]
        hi = ramp[3] if len(ramp) > 3 else ramp[2]
        for (x, y) in mask:
            ul = ((x - 1, y) not in mask) or ((x, y - 1) not in mask)
            ul2 = (x - 1, y - 1) not in mask
            dr = ((x + 1, y) not in mask) or ((x, y + 1) not in mask)
            dr2 = (x + 1, y + 1) not in mask
            # how far down and across the part this pixel sits, 0..1
            ty = (y - y0 + 0.5) / (y1 - y0 + 1)
            tx = (x - x0 + 0.5) / (x1 - x0 + 1)
            v = 0.5 + 0.0 * tx  # 0 dark .. 1 light
            if fall:
                v = 0.78 - 0.55 * (0.62 * ty + 0.38 * tx)
            b = bayer(x + seed, y)
            if rim and dr and not ul:
                ch = sh if (dr2 or b < 0.7) else ba
            elif rim and ul and not dr:
                ch = hi if (ul2 or b < 0.5) else li
            else:
                if v > 0.55:
                    ch = li if b < (v - 0.55) * 3.2 + soft else ba
                elif v < 0.34:
                    ch = sh if b < (0.34 - v) * 3.2 + soft else ba
                else:
                    ch = ba
            self.px[(x, y)] = ch

    def edge(self, mask, ch, where="dr"):
        """Ink the mask's own rim (down-right, or all round) in `ch`."""
        mask = self.clip(mask)
        for (x, y) in mask:
            if where == "all":
                if any(q not in mask for q in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1))):
                    self.px[(x, y)] = ch
            elif where == "dr":
                if (x + 1, y) not in mask or (x, y + 1) not in mask:
                    self.px[(x, y)] = ch
            elif where == "ul":
                if (x - 1, y) not in mask or (x, y - 1) not in mask:
                    self.px[(x, y)] = ch

    def finish(self, outline="k", under="K"):
        """Outline the silhouette; a darker ink under the feet of it."""
        solid = set(self.px)
        for (x, y) in solid:
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                q = (x + dx, y + dy)
                if q not in solid and 0 <= q[0] < self.w and 0 <= q[1] < self.h:
                    self.px[q] = outline
        return self

    def rows(self):
        return ["".join(self.px.get((x, y), ".") for x in range(self.w)) for y in range(self.h)]

    def copy(self):
        s = Sprite(self.w, self.h)
        s.px = dict(self.px)
        return s

    def stamp(self, other, dx=0, dy=0):
        for (x, y), c in other.px.items():
            self.put(x + dx, y + dy, c)


# ---- looking ---------------------------------------------------------------

def png(rows, scale=1, bg=(11, 10, 8)):
    from PIL import Image
    h, w = len(rows), max(len(r) for r in rows)
    im = Image.new("RGB", (w * scale, h * scale), bg)
    for y, r in enumerate(rows):
        for x, c in enumerate(r):
            if c in PAL:
                for j in range(scale):
                    for i in range(scale):
                        im.putpixel((x * scale + i, y * scale + j), PAL[c])
    return im


def sheet(frames, scale=6, cols=8, bg=(21, 29, 40), gap=2):
    """Contact sheet of frames (lists of rows), on a dusky floor."""
    from PIL import Image
    h, w = len(frames[0]), len(frames[0][0])
    rows = (len(frames) + cols - 1) // cols
    im = Image.new("RGB", (cols * (w * scale + gap) + gap, rows * (h * scale + gap) + gap), bg)
    for i, f in enumerate(frames):
        t = png(f, scale, bg)
        im.paste(t, (gap + (i % cols) * (w * scale + gap), gap + (i // cols) * (h * scale + gap)))
    return im
