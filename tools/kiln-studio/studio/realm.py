"""The realm's inks, and the studio's eyes on a PNG.

The eyes work on pictures, not on recipes, so they see every backend's work
the same way: which inks a picture uses and which it shouldn't, lone pixels,
where it meets the floor, how it reads at the game's grain, and the grain
ladder shot a model or a person looks at.

A picture drawn at grain 4 (64 px a tile) is brought to grains 2 and 1 here
by a reduction (each block's ink by vote, the outline kept on the edge). The
kiln backend renders a recipe at every grain directly; the reduction stands
in for it with the backends that draw at one grain only.
"""
from collections import Counter
from functools import lru_cache
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

from . import paths

TILE = 16  # game pixels a tile
CLEAR = "."


class Palette:
    """The realm's inks: key, colour and family, in realm.hex order."""

    def __init__(self, rows):
        self.keys = [k for k, _, _ in rows]
        self.rgb = {k: c for k, c, _ in rows}
        self.family = {k: f for k, _, f in rows}
        self.by_rgb = {c: k for k, c, _ in rows}

    @classmethod
    def parse(cls, text):
        rows = []
        for n, line in enumerate(text.splitlines(), 1):
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            words = line.split()
            if len(words) < 2 or len(words[0]) != 6 or len(words[1]) != 1:
                raise ValueError(f"realm.hex line {n}: want `rrggbb key family`, got {line!r}")
            rgb = tuple(int(words[0][i:i + 2], 16) for i in (0, 2, 4))
            rows.append((words[1], rgb, " ".join(words[2:])))
        return cls(rows)

    def hex(self, key):
        return "%02x%02x%02x" % self.rgb[key]

    def luma(self, key):
        r, g, b = self.rgb[key]
        return (0.299 * r + 0.587 * g + 0.114 * b) / 255

    def nearest(self, rgb):
        return min(self.keys, key=lambda k: sum((a - b) ** 2 for a, b in zip(self.rgb[k], rgb)))

    def ramp(self, keys):
        """A ramp's keys checked against the realm, dark to light."""
        bad = [k for k in keys if k not in self.rgb]
        if bad:
            raise ValueError(f"ramp {keys!r}: no realm ink {''.join(bad)!r}")
        return list(keys)


@lru_cache(maxsize=1)
def realm():
    return Palette.parse(paths.REALM_HEX.read_text())


# --- pictures as ink grids -------------------------------------------------

class Grid:
    """A picture as rows of ink keys: `.` clear, `?` a colour not in the realm."""

    def __init__(self, rows, off=None):
        self.rows = rows
        self.off = off or Counter()  # off-realm colours (hex) and their counts
        self.h = len(rows)
        self.w = len(rows[0]) if rows else 0

    def at(self, x, y):
        return self.rows[y][x] if 0 <= x < self.w and 0 <= y < self.h else CLEAR

    def crop(self, x, y, w, h):
        return Grid([row[x:x + w] for row in self.rows[y:y + h]])

    def text(self):
        return "\n".join(self.rows)


def read_grid(path, palette=None):
    """A PNG as a Grid. Alpha under 128 is clear; partial alpha above is counted."""
    pal = palette or realm()
    im = Image.open(path).convert("RGBA")
    w, h = im.size
    px = im.load()
    rows, off, partial = [], Counter(), 0
    for y in range(h):
        row = []
        for x in range(w):
            r, g, b, a = px[x, y]
            if a < 128:
                row.append(CLEAR)
                continue
            if a < 255:
                partial += 1
            k = pal.by_rgb.get((r, g, b))
            if k is None:
                off["%02x%02x%02x" % (r, g, b)] += 1
                k = "?"
            row.append(k)
        rows.append("".join(row))
    grid = Grid(rows, off)
    grid.partial = partial
    return grid


def frames_of(grid, frame_w, frame_h, count):
    """A strip's frames, left to right then down."""
    per_row = max(1, grid.w // frame_w)
    return [grid.crop((i % per_row) * frame_w, (i // per_row) * frame_h, frame_w, frame_h) for i in range(count)]


def reduce(grid, n, outline="k"):
    """A grid brought down by `n` (grain 4 to 1 is n=4): each n x n block is
    drawn when at least 7/16 of it is; its ink is the block's commonest, except
    that an edge block holding the outline ink takes the outline, so the
    silhouette keeps its line."""
    if n <= 1:
        return grid
    pal = realm()
    rows = []
    need = max(1, round(n * n * 7 / 16))
    for by in range(grid.h // n):
        row = []
        for bx in range(grid.w // n):
            cells = [grid.rows[by * n + j][bx * n + i] for j in range(n) for i in range(n)]
            inked = [c for c in cells if c != CLEAR]
            if len(inked) < need:
                row.append(CLEAR)
            elif len(inked) < len(cells) and outline in inked:
                row.append(outline)
            else:
                counts = Counter(inked).most_common()
                top = counts[0][1]
                row.append(min((k for k, c in counts if c == top), key=lambda k: pal.luma(k) if k in pal.rgb else 0))
        rows.append("".join(row))
    return Grid(rows)


def to_image(grid, scale=1, palette=None):
    pal = palette or realm()
    im = Image.new("RGBA", (grid.w, grid.h), (0, 0, 0, 0))
    px = im.load()
    for y, row in enumerate(grid.rows):
        for x, k in enumerate(row):
            if k == "?":
                px[x, y] = (255, 0, 255, 255)
            elif k != CLEAR:
                px[x, y] = pal.rgb[k] + (255,)
    return im.resize((grid.w * scale, grid.h * scale), Image.NEAREST) if scale > 1 else im


# --- what the eyes measure -------------------------------------------------

def bbox(grid):
    """The drawn box: x, y, w, h, or None when nothing is drawn."""
    ys = [y for y, row in enumerate(grid.rows) if row.strip(CLEAR)]
    if not ys:
        return None
    xs = [x for row in grid.rows for x, k in enumerate(row) if k != CLEAR]
    return [min(xs), ys[0], max(xs) - min(xs) + 1, ys[-1] - ys[0] + 1]


def lone_pixels(grid):
    """Pixels whose ink none of their eight neighbours shares, not counting
    the outline ink: noise to a pixel artist unless it is a glint or an eye."""
    found = []
    for y, row in enumerate(grid.rows):
        for x, k in enumerate(row):
            if k in (CLEAR, "k"):
                continue
            if not any(grid.at(x + i, y + j) == k for i in (-1, 0, 1) for j in (-1, 0, 1) if i or j):
                found.append((x, y, k))
    return found


def pieces(grid):
    """How many 8-connected pieces the drawing falls into."""
    seen, count = set(), 0
    for y, row in enumerate(grid.rows):
        for x, k in enumerate(row):
            if k == CLEAR or (x, y) in seen:
                continue
            count += 1
            stack = [(x, y)]
            seen.add((x, y))
            while stack:
                cx, cy = stack.pop()
                for i in (-1, 0, 1):
                    for j in (-1, 0, 1):
                        p = (cx + i, cy + j)
                        if p not in seen and grid.at(*p) != CLEAR:
                            seen.add(p)
                            stack.append(p)
    return count


def silhouette(grid):
    return "\n".join("".join("#" if k != CLEAR else "." for k in row) for row in grid.rows)


def lint(grid, grain=4, outline="k", rules=None, small=None):
    """The eyes' lints on one frame, as findings {rule, level, msg}. `small` is
    the frame at grain 1 when the backend drew it; else it is a reduction."""
    rules = rules or {}
    out = []
    pal = realm()
    if grid.off:
        top = ", ".join(f"#{c} x{n}" for c, n in grid.off.most_common(6))
        out.append({"rule": "palette", "level": "error",
                    "msg": f"{sum(grid.off.values())} pixels in colours not among the realm's inks: {top}"})
    if getattr(grid, "partial", 0):
        out.append({"rule": "alpha", "level": "error", "msg": f"{grid.partial} pixels half transparent: pixels are drawn or clear"})
    if grain > 1 and (grid.w % grain or grid.h % grain):
        out.append({"rule": "grain", "level": "warn",
                    "msg": f"the canvas is {grid.w}x{grid.h}: at grain {grain} it should be a whole number of game pixels "
                           f"(a multiple of {grain}; 64x64 for one tile)"})
    box = bbox(grid)
    if box is None:
        out.append({"rule": "empty", "level": "error", "msg": "nothing is drawn"})
        return out
    lone = lone_pixels(grid)
    allowed = rules.get("lone_pixels_max", 6)
    if len(lone) > allowed:
        where = ", ".join(f"{x},{y} {k}" for x, y, k in lone[:8])
        out.append({"rule": "lone", "level": "warn",
                    "msg": f"{len(lone)} lone pixels (at most {allowed} for eyes and glints): {where}"})
    inks = Counter(k for row in grid.rows for k in row if k not in (CLEAR, "?"))
    if outline not in inks:
        out.append({"rule": "outline", "level": "warn", "msg": f"no outline ink {outline!r} anywhere"})
    else:
        edge = edge_inks(grid)
        share = edge.get(outline, 0) / max(1, sum(edge.values()))
        if share < 0.5:
            out.append({"rule": "outline", "level": "warn",
                        "msg": f"only {share:.0%} of the silhouette's edge is the outline ink {outline!r} (selout lightens some, most stay dark)"})
    if box[1] + box[3] < grid.h - 2 * grain:
        out.append({"rule": "floor", "level": "warn",
                    "msg": f"the lowest drawn row is {grid.h - box[1] - box[3]} px above the canvas foot: a figure stands on the floor"})
    small = small or (reduce(grid, grain, outline) if grain > 1 else grid)
    n = pieces(small)
    if n > rules.get("pieces_max_at_1", 2):
        out.append({"rule": "silhouette", "level": "warn",
                    "msg": f"at grain 1 the silhouette falls into {n} pieces: thin parts vanish at 16 px a tile"})
    if len(inks) > rules.get("inks_max", 20):
        out.append({"rule": "inks", "level": "warn", "msg": f"{len(inks)} inks: a figure reads best in about 12-18"})
    return out


def edge_inks(grid):
    """The inks on the silhouette's edge (a drawn pixel beside a clear one)."""
    edge = Counter()
    for y, row in enumerate(grid.rows):
        for x, k in enumerate(row):
            if k != CLEAR and any(grid.at(x + i, y + j) == CLEAR for i, j in ((1, 0), (-1, 0), (0, 1), (0, -1))):
                edge[k] += 1
    return edge


def facts(grid, grain=4, outline="k", small=None):
    """The numbers describe reports for one frame."""
    pal = realm()
    box = bbox(grid)
    inks = Counter(k for row in grid.rows for k in row if k not in (CLEAR, "?"))
    small = small or (reduce(grid, grain, outline) if grain > 1 else grid)
    sbox = bbox(small)
    return {
        "canvas": [grid.w, grid.h],
        "grain": grain,
        "box": box,
        "box_game_px": [round(v / grain, 2) for v in box] if box else None,
        "inks": {k: n for k, n in inks.most_common()},
        "families": dict(Counter(pal.family.get(k, "?") for k in inks.elements()).most_common()),
        "lone_pixels": len(lone_pixels(grid)) if box else 0,
        "pieces_at_1": pieces(small) if sbox else 0,
        "box_at_1": sbox,
        "grain_1": "drawn" if small is not None and small is not grid and getattr(small, "drawn", False) else "reduced",
    }


# --- the shot --------------------------------------------------------------

@lru_cache(maxsize=4)
def _font(size=11):
    try:
        return ImageFont.load_default(size=size)
    except TypeError:
        return ImageFont.load_default()


def floor(w, h, tile, inks=("K", "Z")):
    """A room floor: tiles of `tile` screen px in two inks, a checker."""
    pal = realm()
    a, b = (pal.rgb[k] + (255,) for k in inks)
    im = Image.new("RGBA", (w, h), a)
    d = ImageDraw.Draw(im)
    for ty in range(0, h, tile):
        for tx in range(0, w, tile):
            if (tx // tile + ty // tile) % 2:
                d.rectangle([tx, ty, tx + tile - 1, ty + tile - 1], fill=b)
    return im


def ladder(frames, out, grain=4, outline="k", floor_inks=("K", "Z"), zoom=4, title="", lower=None, key=0):
    """The grain ladder shot of a figure drawn at `grain`, written to `out`.

    Row one, the first frame: grain 1 at the game's own size (1 screen px a
    pixel), then grains 1, 2 and `grain` at one screen size (64 screen px a
    tile), each on a room floor, then the drawing zoomed. Row two, when there
    are frames: every frame at 64 px a tile, and the grain-1 frames under them.
    `lower` holds the backend's own frames at grains 1 and 2 ({1: [...], 2: [...]});
    without them those grains are reductions. Row one shows frame `key`.
    """
    lower = lower or {}

    def at(g, i):
        if lower.get(g):
            return lower[g][i]
        return reduce(frames[i], max(1, grain // g), outline) if grain >= g else frames[i]
    pal = realm()
    first = frames[key]
    label_h = 16
    pad = 8  # screen px around a panel's canvas
    S = 4  # screen px per game px for the side-by-side panels

    def panel(grid, scale, tile, label):
        img = to_image(grid, scale)
        w, h = img.size[0] + 2 * pad, img.size[1] + 2 * pad
        base = floor(w, h, tile, floor_inks)
        base.alpha_composite(img, (pad, pad))
        card = Image.new("RGBA", (w, h + label_h), pal.rgb["k"] + (255,))
        card.alpha_composite(base, (0, label_h))
        ImageDraw.Draw(card).text((2, 2), label, fill=pal.rgb["H"] + (255,), font=_font())
        return card

    g1, g2 = at(1, key), at(2, key)
    tag = "" if lower.get(1) else " (reduced)"
    which = f", frame {key}" if len(frames) > 1 else ""
    row1 = [panel(g1, 1, TILE, "game 1x"),
            panel(g1, S, TILE * S, "grain 1" + tag),
            panel(g2, S // 2 if grain >= 2 else S, TILE * S, "grain 2" + tag),
            panel(first, max(1, S // grain), TILE * S, f"grain {grain}"),
            panel(first, zoom * max(1, S // grain), TILE * S * zoom, f"grain {grain} x{zoom}{which}")]
    rows = [row1]
    if len(frames) > 1:
        rows.append([panel(f, max(1, S // grain), TILE * S, f"f{i}") for i, f in enumerate(frames)])
        rows.append([panel(at(1, i), S, TILE * S, f"f{i} g1") for i in range(len(frames))])
    gap = 6
    head = 18 if title else 0
    width = max(sum(c.size[0] for c in r) + gap * (len(r) + 1) for r in rows)
    height = head + sum(max(c.size[1] for c in r) for r in rows) + gap * (len(rows) + 1)
    sheet = Image.new("RGBA", (width, height), pal.rgb["X"] + (255,))
    if title:
        ImageDraw.Draw(sheet).text((gap, 3), title, fill=pal.rgb["W"] + (255,), font=_font(12))
    y = head + gap
    for r in rows:
        x = gap
        for c in r:
            sheet.alpha_composite(c, (x, y))
            x += c.size[0] + gap
        y += max(c.size[1] for c in r) + gap
    Path(out).parent.mkdir(parents=True, exist_ok=True)
    sheet.convert("RGB").save(out)
    return str(out)
