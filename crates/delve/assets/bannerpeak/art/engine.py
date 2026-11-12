"""A tiny sprite engine: sprites are dicts {(x, y): ink}, composed in layers."""
from PIL import Image
from palette import PAL


def parse(rows, ox=0, oy=0):
    out = {}
    for y, r in enumerate(rows):
        for x, ch in enumerate(r):
            if ch not in ". ":
                out[(x + ox, y + oy)] = ch
    return out


def shift(s, dx, dy):
    return {(x + dx, y + dy): c for (x, y), c in s.items()}


def over(*layers):
    out = {}
    for l in layers:
        out.update(l)
    return out


def flip(s, w):
    return {(w - 1 - x, y): c for (x, y), c in s.items()}


def rot90(s, w, h, k):
    """Rotate k quarter turns clockwise on a w x h canvas (result is square-ish)."""
    for _ in range(k % 4):
        s = {(h - 1 - y, x): c for (x, y), c in s.items()}
        w, h = h, w
    return s, w, h


def rotate(s, deg, cx, cy):
    """Nearest-neighbour turn about (cx, cy) in cell units (clockwise)."""
    import math

    a = math.radians(deg)
    ca, sa = math.cos(a), math.sin(a)
    out = {}
    pts = list(s.items())
    # inverse mapping so no holes open
    xs = [p[0][0] for p in pts]
    ys = [p[0][1] for p in pts]
    r = int(max(max(xs) - min(xs), max(ys) - min(ys)) + 4)
    for y in range(int(cy) - r, int(cy) + r + 1):
        for x in range(int(cx) - r, int(cx) + r + 1):
            dx, dy = x + 0.5 - cx, y + 0.5 - cy
            sx = ca * dx + sa * dy + cx - 0.5
            sy = -sa * dx + ca * dy + cy - 0.5
            c = s.get((round(sx), round(sy)))
            if c:
                out[(x, y)] = c
    return out


def recolor(s, m):
    return {p: m.get(c, c) for p, c in s.items()}


def outline(s, ch="k", keep=None):
    """A one pixel rim round the silhouette."""
    out = dict(s)
    for (x, y) in list(s):
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            p = (x + dx, y + dy)
            if p not in s:
                out[p] = ch
    return out


def rows(s, w, h):
    out = []
    for y in range(h):
        out.append("".join(s.get((x, y), ".") for x in range(w)))
    return out


def render(sprites, w, h, zoom=8, cols=8, bg=(10, 10, 14), box=None, labels=None):
    """A contact sheet of sprites (list of dicts) for the eye."""
    n = len(sprites)
    rws = (n + cols - 1) // cols
    cw, ch = w * zoom + 8, h * zoom + 8
    img = Image.new("RGB", (cols * cw, rws * ch), bg)
    px = img.load()
    for i, s in enumerate(sprites):
        ox, oy = (i % cols) * cw + 4, (i // cols) * ch + 4
        for y in range(h):
            for x in range(w):
                base = (22, 22, 28) if (x + y) % 2 == 0 else (18, 18, 24)
                for yy in range(zoom):
                    for xx in range(zoom):
                        px[ox + x * zoom + xx, oy + y * zoom + yy] = base
        if box:
            bx, by, bw, bh = box
            for x in range(bw * zoom):
                for yy in (0, bh * zoom - 1):
                    px[ox + bx * zoom + x, oy + by * zoom + yy] = (60, 90, 60)
            for y in range(bh * zoom):
                for xx in (0, bw * zoom - 1):
                    px[ox + bx * zoom + xx, oy + by * zoom + y] = (60, 90, 60)
        for (x, y), c in s.items():
            if 0 <= x < w and 0 <= y < h and c in PAL:
                col = PAL[c]
                for yy in range(zoom):
                    for xx in range(zoom):
                        px[ox + x * zoom + xx, oy + y * zoom + yy] = col
    return img
