"""The knights of Moonfall: a tall closed helm over a tabard in the seat's
house colour, a plume and a cloak of it, steel at the shoulders and shins.
The three masks change the helm: bare (plain steel), Swift (pale blue steel,
white wings), Stout (bronze, heavy and brow-barred).

The house colour is drawn in marker characters (`#` shadow, `&` base, `*` lit)
and the stage swaps them for the seat's ramp when it builds its bank, so one
set of frames serves all six seats. Same canvas and rig as the villagers."""
from rig import *

HOUSE = ["#", "&", "*"]
STEEL = ["g", "J", "i", "H"]
SWIFT = ["u", "v", "V", "W"]
STOUT = ["p", "R", "O", "t"]
MASKS = {"bare": STEEL, "swift": SWIFT, "stout": STOUT}


def helm(sp, c, d, kind, ph, bob, x):
    ramp = MASKS[c.mask]
    hy = c.head_y + bob
    stout = c.mask == "stout"
    rx = 4.3 if stout else 4.0
    if d == "left":
        hx = CX - 0.5
        sp.paint(ell(hx + 0.5, hy - 0.2, rx - 0.1, 4.2), ramp, rim=True, fall=False)
        # the visor: a dark slit with an eye in it
        sp.fill(rect(int(hx - 4), int(hy + 0.2), 4, 1), "k")
        px(sp, hx - 3, hy + 0.2, "w")
        sp.fill(rect(int(hx - 3), int(hy + 1.2), 3, 1), ramp[0])
        # plume streaming back
        for i, (dx, dy) in enumerate([(1, -5), (2, -6), (3, -6), (4, -6), (5, -5), (5, -4)]):
            px(sp, hx + dx, hy + dy, HOUSE[2] if i < 3 else HOUSE[1])
        px(sp, hx + 2, hy - 5, HOUSE[1])
        if c.mask == "swift":
            for dx, dy in [(1, -1), (2, -2), (3, -2), (2, -1)]:
                px(sp, hx + dx, hy + dy, "W")
        if stout:
            sp.fill(rect(int(hx - 4), int(hy - 1.4), 5, 1), ramp[0])
    elif d == "up":
        sp.paint(ell(CX, hy - 0.2, rx, 4.2), ramp, rim=True, fall=False)
        sp.fill(rect(int(CX), int(hy - 3), 1, 7), ramp[0])
        for dx, dy, ch in [(0, -5, HOUSE[2]), (1, -6, HOUSE[2]), (1, -5, HOUSE[1]), (-1, -5, HOUSE[1]), (2, -6, HOUSE[1])]:
            px(sp, CX + dx, hy + dy, ch)
        if c.mask == "swift":
            for dy in (-1, 0):
                px(sp, CX - 5, hy + dy, "W"); px(sp, CX + 5, hy + dy, "W")
            px(sp, CX - 6, hy - 1, "W"); px(sp, CX + 6, hy - 1, "W")
    else:
        sp.paint(ell(CX, hy - 0.2, rx, 4.2), ramp, rim=True, fall=False)
        sp.fill(rect(int(CX - 3), int(hy + 0.2), 7, 1), "k")
        px(sp, CX - 2, hy + 0.2, "w"); px(sp, CX + 1, hy + 0.2, "w")
        sp.fill(rect(int(CX), int(hy - 3.5), 1, 3), ramp[0])
        sp.fill(rect(int(CX - 2), int(hy + 1.2), 5, 1), ramp[0])
        # the plume, a crest of the house's colour over the crown
        for dx, dy, ch in [(0, -5, HOUSE[2]), (1, -6, HOUSE[2]), (2, -6, HOUSE[1]), (3, -5, HOUSE[1]), (-1, -5, HOUSE[1]), (0, -6, HOUSE[1])]:
            px(sp, CX + dx, hy + dy, ch)
        if c.mask == "swift":
            for dy in (-1, 0):
                px(sp, CX - 5, hy + dy, "W"); px(sp, CX + 5, hy + dy, "W")
            px(sp, CX - 6, hy - 1, "W"); px(sp, CX + 6, hy - 1, "W")
            px(sp, CX - 6, hy - 2, "V"); px(sp, CX + 6, hy - 2, "V")
        if stout:
            sp.fill(rect(int(CX - 4), int(hy - 1.4), 9, 1), ramp[0])
            px(sp, CX - 4, hy - 0.4, ramp[2]); px(sp, CX + 4, hy - 0.4, ramp[2])


def cloak(sp, c, d, kind, ph, bob, x):
    top = c.top + bob
    fl = x.get("flare", 0)
    if d == "down":
        # a cloak edge showing past each shoulder
        sp.paint(poly([(CX - c.sh - 1.2 - fl, top + 1), (CX + c.sh + 1.2 + fl, top + 1), (CX + c.sh + 2.0 + fl, c.hem + 1), (CX - c.sh - 2.0 - fl, c.hem + 1)]), HOUSE, rim=False, fall=False)
    elif d == "up":
        sp.paint(poly([(CX - c.sh - 0.6, top), (CX + c.sh + 0.6, top), (CX + c.sh + 1.4 + fl, c.hem + 3), (CX - c.sh - 1.4 - fl, c.hem + 3)]), HOUSE, rim=True, fall=False)
    else:
        sp.paint(poly([(CX + 1, top), (CX + c.side_w + 1.2, top + 1), (CX + c.side_w + 3.0 + fl, c.hem + 2), (CX + 0.5, c.hem + 3)]), HOUSE, rim=True, fall=False)


def kit(sp, c, d, kind, ph, bob, x, hands):
    top = c.top + bob
    ramp = MASKS[c.mask]
    if d == "down":
        # the house colour is the tabard's front; a crest and a belt
        sp.paint(poly([(CX - 2.6, top + 1), (CX + 2.6, top + 1), (CX + 3.4, c.hem), (CX - 3.4, c.hem)]), HOUSE, rim=True, fall=False)
        sp.fill(rect(int(CX - 3), c.hem - 1, 7, 1), HOUSE[0])
        px(sp, CX - 0.5, top + 4, "6"); px(sp, CX + 0.5, top + 4, "6") if False else None
        sp.fill(rect(int(CX - 1), top + 3, 2, 3), "6")
        # steel at the shoulders
        for side in (-1, 1):
            sp.paint(rect(int(CX + side * (c.sh + 0.1) - (1 if side < 0 else 0)) - 1, top - 0, 3, 2), ramp, rim=True, fall=False)
    elif d == "up":
        for side in (-1, 1):
            sp.paint(rect(int(CX + side * (c.sh + 0.1) - (1 if side < 0 else 0)) - 1, top - 0, 3, 2), ramp, rim=True, fall=False)
    else:
        sp.paint(poly([(CX - c.side_w, top + 1), (CX + 0.8, top + 1), (CX + 0.8, c.hem), (CX - c.side_w - 0.2, c.hem)]), HOUSE, rim=True, fall=False)
        sp.paint(rect(int(CX) - 2, top - 0, 4, 2), ramp, rim=True, fall=False)
        sp.fill(rect(int(CX - 3), top + 3, 1, 3), "6")


def knight(mask):
    return Cfg(
        skin=["j", "G", "h"], hair=MASKS[mask],
        tunic=["K", "g", "j"], sleeve=["K", "g", "j"], belt=["I", "O"],
        legs=["K", "g", "j"], boot=["k", "g", "j"],
        sh=4.0 if mask != "stout" else 4.6, hm=4.2 if mask != "stout" else 4.8, side_w=3.2 if mask != "stout" else 3.8,
        head_r=(3.9, 3.9), head_y=9.4, hat=helm, back=cloak, front=kit, mask=mask,
        leg_w=3,
    )


KNIGHTS = {m: knight(m) for m in MASKS}


def with_head_painted(sp, c, d, bob):
    return sp


def shifted(sp, dx, dy):
    out = Sprite(sp.w, sp.h)
    for (x, y), ch in sp.px.items():
        out.put(x + dx, y + dy, ch)
    return out


def idle(mask, d, ph):
    c = KNIGHTS[mask]
    return person(c, d, "idle", ph, x={"bob": [0, -1, 0, 0][ph % 4]})


def walk(mask, d, ph):
    return person(KNIGHTS[mask], d, "walk", ph, x={})


def roll(mask, d, ph):
    """A sidestep: a crouch, a hop with the cloak out, a landing."""
    c = KNIGHTS[mask]
    lift = [0, 3, 1][ph]
    flare = [0, 2, 1][ph]
    sp = person(c, d, "walk", [0, 1, 3][ph], x={"flare": flare})
    return shifted(sp, 0, -lift)


def cheer(mask, ph):
    c = KNIGHTS[mask]
    b = [0, -1][ph % 2]
    sp = person(c, "down", "idle", 0, x={"skip": (-1, 1), "bob": b})
    top = c.top + b
    arm_to(sp, c, -1, b, CX - 6.2, top - 3 - (ph % 2))
    arm_to(sp, c, 1, b, CX + 6.2, top - 3 - (ph % 2))
    return shifted(sp, 0, -(ph % 2))


def down(mask, ph):
    """Snuffed out: lying where they fell, the plume and cloak spread."""
    ramp = MASKS[mask]
    sp = Sprite(W, H)
    b = ph % 2
    y0 = 19
    # a cloak spread under them, the tabard along it, the helm at one end
    sp.paint(ell(9, y0 + 3, 8.0, 3.2), HOUSE, rim=True, fall=False)
    sp.paint(ell(9.5, y0 + 2, 5.2, 2.6), ["K", "g", "j"], rim=True, fall=False)
    sp.paint(ell(9.5, y0 + 2, 3.6, 2.0 - 0 * b), HOUSE, rim=True, fall=False)
    sp.paint(ell(4.2, y0 + 1 - b, 3.0, 3.0), ramp, rim=True, fall=False)
    sp.fill(rect(2, y0 + 1 - b, 3, 1), "k")
    px(sp, 3, y0 + 1 - b, "7")
    for dx, dy in [(5, -4), (6, -5), (7, -5)]:
        px(sp, 4 + dx, y0 + dy + 4 - b * 0, HOUSE[2])
    sp.paint(rect(13, y0 + 1, 4, 3), ["K", "g", "j"], rim=True, fall=False)
    sp.paint(rect(16, y0 + 2, 2, 2), ["k", "g", "j"], rim=True, fall=False)
    return sp


def sheet_rows():
    rows = []
    for m in MASKS:
        for d in ("down", "up", "left"):
            for ph in range(2):
                rows.append(idle(m, d, ph).finish().rows())
            for ph in range(4):
                rows.append(walk(m, d, ph).finish().rows())
        for ph in range(3):
            rows.append(roll(m, "down", ph).finish().rows())
        rows.append(cheer(m, 0).finish().rows()); rows.append(cheer(m, 1).finish().rows())
        rows.append(down(m, 0).finish().rows()); rows.append(down(m, 1).finish().rows())
    return rows


def recolour(rows, ramp):
    out = []
    for r in rows:
        out.append(r.replace("#", ramp[0]).replace("&", ramp[1]).replace("*", ramp[2]))
    return out


if __name__ == "__main__":
    from engine import sheet
    rows = [recolour(r, ["!", "7", "R"]) for r in sheet_rows()]
    sheet(rows, scale=5, cols=12, bg=(60, 58, 52)).save("/work/tmp/mf/knights.png")
    rows2 = []
    for ramp in (["0", "2", "3"], ["R", "4", "@"], ["l", "C", "Y"], ["!", "7", "R"], ["1", "z", "V"], ["L", "Y", "T"]):
        rows2.append(recolour(KNIGHTS and idle("bare", "down", 0).finish().rows(), ramp))
        rows2.append(recolour(idle("swift", "down", 0).finish().rows(), ramp))
        rows2.append(recolour(idle("stout", "down", 0).finish().rows(), ramp))
    sheet(rows2, scale=6, cols=9, bg=(60, 58, 52)).save("/work/tmp/mf/knights6.png")
