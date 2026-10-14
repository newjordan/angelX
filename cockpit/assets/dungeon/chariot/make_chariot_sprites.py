#!/usr/bin/env python3
"""SNES-floor chariot sprites: side (east), front (south) and rear (north)
sheets, two gallop frames each. West is the east sheet mirrored at render.

Ink key (stage/world_viz/overworld/ink.rs): the far/left horse wears the
fixed bay coat `B`; `R` is the house colour — near/right horse, cab and
driver — repainted per house banner by the arena. `O` is the gold harness
and trim, `K` hooves and iron, `p` the shafts, `j` dust.
Regenerate: python3 cockpit/assets/dungeon/chariot/make_chariot_sprites.py
"""
import os

OUT = os.path.dirname(os.path.abspath(__file__))


def canvas(w, h):
    return [["."] * w for _ in range(h)]


def put(g, x, y, s):
    for i, ch in enumerate(s):
        if 0 <= x + i < len(g[0]) and 0 <= y < len(g):
            g[y][x + i] = ch


def dump(g, name):
    text = "\n".join("".join(r).rstrip() for r in g) + "\n"
    open(os.path.join(OUT, name + ".ink"), "w").write(text)
    print(name, "ok")


# ── east: the side view (unchanged from the first sheet) ───────────────────

def horse_e(g, x, y, legs, coat):
    put(g, x + 10, y + 0, "BB"); put(g, x + 9, y + 1, "BoB"); put(g, x + 8, y + 2, "nBB")
    put(g, x + 2, y + 2, coat * 7); put(g, x + 1, y + 3, coat * 9); put(g, x + 2, y + 4, coat * 7)
    put(g, x + 0, y + 3, "n"); put(g, x + 8, y + 3, "O")
    legpos = [(2, -1), (4, 0), (7, 0), (9, 1)] if legs == 0 else [(2, 1), (4, 1), (7, -1), (9, -1)]
    for lx, dx in legpos:
        put(g, x + lx, y + 5, coat); put(g, x + lx + dx, y + 6, coat); put(g, x + lx + dx, y + 7, "K")


def make_east(frame):
    g = canvas(40, 22)
    put(g, 7, 11, "p" * 9); put(g, 7, 14, "p" * 9)
    horse_e(g, 15, 3, frame, "B")       # far horse
    horse_e(g, 18, 7, 1 - frame, "R")   # near horse, house colour
    put(g, 2, 8, "OOO"); put(g, 1, 9, "ORRRO"); put(g, 1, 10, "KRRRK"); put(g, 1, 11, "KKKKK")
    put(g, 3, 5, "hh"); put(g, 3, 6, "tt"); put(g, 3, 7, "RR")   # driver
    put(g, 5, 4, "t"); put(g, 6, 3, "H")                          # whip
    put(g, 2, 12, "OtO"); put(g, 2, 13, "tKt"); put(g, 2, 14, "OtO")  # wheel
    put(g, 12, 19, "jj"); put(g, 22, 20, "j"); put(g, 3, 17, "jj")    # dust
    return g


# ── south: the team at the viewer — heads, chests and forelegs ────────────

def horse_s(g, x, coat, legs):
    # ears, poll and a face that narrows to the muzzle
    put(g, x + 0, 8, coat); put(g, x + 4, 8, coat)
    put(g, x + 0, 9, coat * 5)
    put(g, x + 0, 10, coat + "o" + coat + "o" + coat)
    put(g, x + 1, 11, coat * 3)
    put(g, x + 1, 12, "n" + coat + "n")
    # chest under a gold harness strap
    put(g, x - 1, 13, "O" * 7)
    put(g, x - 1, 14, coat * 7); put(g, x - 1, 15, coat * 7)
    put(g, x + 0, 16, coat * 5)
    # forelegs: the gallop swaps which pair is down
    down, up = (0, 4) if legs == 0 else (4, 0)
    for lx in (down,):
        put(g, x + lx, 17, coat); put(g, x + lx, 18, coat); put(g, x + lx, 19, coat)
        put(g, x + lx, 20, "K"); put(g, x + lx, 21, "K")
    put(g, x + up, 17, coat); put(g, x + up + (1 if up == 0 else -1), 18, coat)
    put(g, x + up + (2 if up == 0 else -2), 18, "K")


def make_south(frame):
    g = canvas(24, 30)
    # driver and cab front, behind the team
    put(g, 11, 1, "hh"); put(g, 11, 2, "tt"); put(g, 10, 3, "RRRR")
    put(g, 15, 2, "t"); put(g, 16, 1, "H")                      # whip raised
    put(g, 7, 4, "O" * 10)
    put(g, 7, 5, "K" + "R" * 8 + "K"); put(g, 7, 6, "K" + "R" * 8 + "K")
    put(g, 7, 7, "K" * 10)
    put(g, 5, 5, "p"); put(g, 18, 5, "p")                       # shaft tops
    horse_s(g, 3, "B", frame)
    horse_s(g, 16, "R", 1 - frame)
    put(g, 1, 24, "jj"); put(g, 20, 25, "jj"); put(g, 10, 27, "j")  # dust
    return g


# ── north: away from the viewer — rumps, tails and the cab's back ─────────

def horse_n(g, x, coat, legs):
    put(g, x + 0, 1, coat * 6); put(g, x + 0, 2, coat * 6); put(g, x + 1, 3, coat * 4)
    put(g, x + 2, 3, "n"); put(g, x + 2, 4, "n"); put(g, x + 3, 5, "n"); put(g, x + 3, 6, "n")  # tail
    put(g, x + 0, 4, "O"); put(g, x + 5, 4, "O")                # harness ends
    down, up = (1, 4) if legs == 0 else (4, 1)
    put(g, x + down, 4, coat); put(g, x + down, 5, coat); put(g, x + down, 6, coat)
    put(g, x + down, 7, "K"); put(g, x + down, 8, "K")
    put(g, x + up, 4, coat); put(g, x + up, 5, coat); put(g, x + up, 5, coat)
    put(g, x + up + (1 if up == 1 else -1), 6, "K")


def make_north(frame):
    g = canvas(24, 30)
    horse_n(g, 2, "B", frame)
    horse_n(g, 16, "R", 1 - frame)
    # shafts converging on the cab
    put(g, 6, 9, "p"); put(g, 17, 9, "p"); put(g, 7, 10, "p"); put(g, 16, 10, "p")
    put(g, 8, 11, "p"); put(g, 15, 11, "p")
    # the driver's back, whip arm out to the right
    put(g, 11, 12, "hh"); put(g, 10, 13, "RRRR"); put(g, 10, 14, "RRRR")
    put(g, 14, 13, "t"); put(g, 15, 12, "H")
    # cab rear with its gold rail
    put(g, 7, 15, "O" * 10)
    put(g, 7, 16, "K" + "R" * 8 + "K"); put(g, 7, 17, "K" + "R" * 8 + "K")
    put(g, 7, 18, "K" * 10)
    # both wheels from behind
    for wx in (4, 17):
        put(g, wx, 20, "OtO"); put(g, wx, 21, "tKt"); put(g, wx, 22, "OtO")
    put(g, 2, 25, "jj"); put(g, 19, 26, "jj"); put(g, 11, 28, "j")  # dust
    return g


for name, frame in [("chariot_e_a", 0), ("chariot_e_b", 1)]:
    dump(make_east(frame), name)
for name, frame in [("chariot_s_a", 0), ("chariot_s_b", 1)]:
    dump(make_south(frame), name)
for name, frame in [("chariot_n_a", 0), ("chariot_n_b", 1)]:
    dump(make_north(frame), name)
