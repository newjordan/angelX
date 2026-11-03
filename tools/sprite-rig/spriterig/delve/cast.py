"""The angelX delve cast, as rig-ready cutouts.

Every character is built from the game's own ink rows (see source.py). A cutout is a
list of layers; each layer belongs to one bone and has its own ink grid, so where the
game draws one drawing over another (the hero's shield over the harness) the covered
pixels survive and show when the part moves. Characters cut from a single drawing get
the hole behind a moving part filled with the neighbouring body ink.

Bones (side view, facing right): root, torso, head, arm_b (far arm / shield / staff),
arm_f (near arm / weapon / tool), legs. Legs keep the game's own authored stride
frames and are swapped by the motion's gait phase rather than bent: at sixteen pixels
a rotated two-row leg reads worse than delve's hand-drawn step.
"""
import numpy as np
from . import source

BONES = ("root", "torso", "head", "arm_b", "arm_f", "legs")


def grid(rows, w=None, h=None, at=(0, 0)):
    h = h or len(rows) + at[1]
    w = w or max(len(r) for r in rows) + at[0]
    g = np.full((h, w), ".", dtype="<U1")
    for j, r in enumerate(rows):
        for i, c in enumerate(r):
            if c != "." and 0 <= j + at[1] < h and 0 <= i + at[0] < w:
                g[j + at[1], i + at[0]] = c
    return g


def recolor(g, swaps):
    out = g.copy()
    for a, b in swaps:
        out[g == a] = b
    return out


class Character:
    def __init__(self, name, w, h, layers, pivots, legs, order, facing="right", view="side",
                 source_ref="", note="", amp=None):
        self.name, self.w, self.h = name, w, h
        self.layers = layers          # [(bone, grid HxW)] in draw order
        self.pivots = pivots          # joint -> (x, y) in pixel-edge coords
        self.legs = legs              # [stand, step_a, step_b] full-size grids (bone 'legs')
        self.order = order
        self.facing, self.view, self.source_ref, self.note = facing, view, source_ref, note
        self.amp = amp or {}
        self.alts = {}               # (bone, snapped degrees) -> authored grid

    def rest(self):
        g = np.full((self.h, self.w), ".", dtype="<U1")
        for bone, lg in [("legs", self.legs[0])] + self.layers:
            m = lg != "."
            g[m] = lg[m]
        return g


def _split(base, rects, fill_under=("arm_b", "arm_f")):
    """Cut one drawing into bone layers. rects: [(bone, x0, y0, x1, y1)] (inclusive,
    later wins); unclaimed pixels go to torso. Pixels taken by a moving part are
    filled in the torso layer with the nearest torso ink (so no hole opens)."""
    h, w = base.shape
    lab = np.full((h, w), "torso", dtype="<U6")
    for bone, x0, y0, x1, y1 in rects:
        lab[y0:y1 + 1, x0:x1 + 1] = bone
    lab[base == "."] = ""
    layers = {}
    for bone in set(lab[lab != ""].tolist()):
        g = np.where(lab == bone, base, ".")
        layers[bone] = g
    tor = layers.setdefault("torso", np.full((h, w), ".", dtype="<U1"))
    ty, tx = np.nonzero(tor != ".")
    if len(ty):
        for bone in fill_under:
            if bone not in layers:
                continue
            for y, x in zip(*np.nonzero(layers[bone] != ".")):
                row = tx[ty == y]
                if len(row) and row.min() < x < row.max():
                    k = np.argmin((tx - x) ** 2 + (ty - y) ** 2 * 2)
                    tor[y, x] = tor[ty[k], tx[k]]
    return layers


# ------------------------------------------------------------------ the hero
def hero(seat="1", root=source.DEFAULT_ROOT, held="sword"):
    """kit/hero.rs ACROSS: the realm's knight, sixteen pixels, facing right."""
    p = source.hero_parts(root)
    # realm_colours: seats 1-3 are seat_colours; the gold seat takes its rim to bronze first
    swaps = source.seat_colours(root).get(seat) if seat in "123" else \
        source.seat_colours(root, "realm_colours")["_"]
    W = H = 16
    mk = lambda rows, at: recolor(grid(rows, W, H, at), swaps)
    legs = [mk(s, (0, 14)) for s in p["steel"]]
    hl = p[held][0]
    layers = [("torso", mk(p["harness"], (0, 8))), ("head", mk(p["helm"], (0, 0))),
              ("arm_b", mk(p["shield"]["rows"], tuple(p["shield"]["at"]))),
              ("arm_f", mk(hl["rows"], tuple(hl["at"])))]
    names = {"1": "red", "2": "blue", "3": "green"}
    ch = Character(
        f"hero_{names.get(seat, 'gold')}", W, H, layers,
        pivots={"hip": (8.0, 14.0), "neck": (8.5, 8.0), "shoulder_b": (5.5, 9.5),
                "shoulder_f": (10.5, 9.5)},
        legs=legs, order=["legs", "torso", "head", "arm_b", "arm_f"],
        source_ref=f"render/overworld/kit/hero.rs ACROSS + arena/knights.rs seat_colours({seat})",
        note="the knight every player starts as; plume/tabard/shield take the seat's colours",
        amp={"head": 0.6, "torso": 0.7})
    # the game's own strike drawing (held levelled) stands in for the arm thrust forward
    st = p[held][1]
    ch.alts[("arm_f", -90)] = mk(st["rows"], tuple(st["at"]))
    return ch


# ------------------------------------------------------------------ folk & foes
def _stride_legs(rows, n=2):
    """barony.rs stride(): the last two rows swapped for legs apart."""
    w, h = len(rows[0]), len(rows)
    stand = grid(rows[-n:], w, h, (0, h - n))
    apart = grid(["...n.n......", "..nn.nn....."], w, h, ((w - 12) // 2 + 1, h - 2))
    return [stand, apart, apart[:, ::-1] if False else apart]


def folk(kind, root=source.DEFAULT_ROOT):
    rows = source.const_rows(root, "arena/barony.rs", kind)
    h, w = len(rows), len(rows[0])
    base = grid(rows, w, h)
    base[h - 2:] = "."
    spec = {
        # (rects, pivots, facing/view, note)
        "MINER": ([("head", 0, 0, 7, 3), ("arm_f", 7, 0, 11, 8)],
                  {"hip": (4, 10), "neck": (4, 4), "shoulder_b": (2, 5.5), "shoulder_f": (7.5, 5.5)},
                  "right", "side", "a dwarf miner of Caer Dwfn with his pick (barony folk)"),
        "SMITH": ([("head", 0, 0, 6, 4), ("arm_f", 7, 0, 11, 7)],
                  {"hip": (4, 11), "neck": (4, 5), "shoulder_b": (2, 6.5), "shoulder_f": (7, 7.5)},
                  "right", "side", "the barony's smith, hammer up"),
        "GUARD": ([("head", 0, 0, 7, 5), ("arm_f", 8, 0, 11, 10)],
                  {"hip": (5, 11), "neck": (5, 6), "shoulder_b": (2, 7), "shoulder_f": (8.5, 7)},
                  "right", "front", "a barony guard with his spear (drawn toward the viewer)"),
        "KING": ([("head", 0, 0, 12, 8), ("arm_b", 0, 7, 3, 19), ("arm_f", 13, 9, 21, 17)],
                 {"hip": (8, 18), "neck": (7, 8), "shoulder_b": (3.5, 9), "shoulder_f": (13, 10)},
                 "right", "front", "King Brannoc Onehorn of Caer Dwfn: one horn, staff and shield"),
    }[kind]
    rects, piv, facing, view, note = spec
    layers = _split(base, rects)
    legs = _stride_legs(rows)
    order = ["legs", "arm_b", "torso", "head", "arm_f"] if kind != "KING" else ["arm_b", "legs", "torso", "head", "arm_f"]
    name = {"KING": "brannoc"}.get(kind, kind.lower())
    return Character(name, w, h, [(b, layers[b]) for b in order if b in layers and b != "legs"],
                     {k: (float(v[0]), float(v[1])) for k, v in piv.items()}, legs, order,
                     facing=facing, view=view, source_ref=f"render/overworld/arena/barony.rs {kind}",
                     note=note, amp={"head": 0.5, "torso": 0.6, "arm_f": 0.7, "arm_b": 0.7})


def goblin(root=source.DEFAULT_ROOT):
    """sprites.rs goblin_raw (no sack): the loot goblin, facing right, with his own
    standing and two running leg drawings."""
    stand = source.goblin_rows(root, False)
    h, w = len(stand), len(stand[0])
    base = grid(stand, w, h)
    base[h - 2:] = "."
    layers = _split(base, [("head", 0, 0, 13, 6), ("arm_b", 0, 8, 3, 9), ("arm_f", 9, 7, 13, 9)])
    legs = [grid(source.goblin_rows(root, r, s)[-2:], w, h, (0, h - 2))
            for r, s in ((False, True), (True, True), (True, False))]
    return Character("goblin", w, h, [(b, layers[b]) for b in ("arm_b", "torso", "head", "arm_f")],
                     {"hip": (6.5, 10.0), "neck": (6.5, 7.0), "shoulder_b": (3.5, 8.0),
                      "shoulder_f": (9.0, 8.0)}, legs, ["legs", "arm_b", "torso", "head", "arm_f"],
                     source_ref="render/overworld/arena/sprites.rs goblin_raw",
                     note="the loot goblin (no sack), green thief with long ears",
                     amp={"head": 0.5, "torso": 0.6})


def cast(root=source.DEFAULT_ROOT):
    return [hero("1", root), hero("2", root), hero("3", root), hero("_", root),
            folk("KING", root), folk("GUARD", root), folk("MINER", root), folk("SMITH", root),
            goblin(root)]
