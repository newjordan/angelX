"""What a knight carries over their head: five small things (12 by 12)."""
from engine import *

W, H = 12, 12


def ink():
    sp = Sprite(W, H)
    sp.paint(ell(6, 7.5, 3.8, 3.4), ["d", "(", ")"], rim=True, fall=False)
    sp.fill(rect(4, 3, 4, 2), "q")
    sp.put(5, 3, "z")
    sp.fill(rect(5, 2, 2, 2), "o")
    # a quill standing in it
    for i in range(5):
        sp.put(8 + i // 3, 5 - i, "W" if i > 1 else "V")
    return sp.finish()


def reeds():
    sp = Sprite(W, H)
    for x, h in [(3, 9), (5, 10), (7, 9), (9, 8)]:
        sp.fill(rect(x, 10 - h + 1, 1, h), "M" if x % 2 else "y")
    sp.fill(rect(2, 9, 8, 1), "o")
    for x, y in [(3, 1), (5, 0), (7, 1), (9, 2)]:
        sp.fill(rect(x - 1 if x > 3 else x, y, 2, 2), "I")
    return sp.finish()


def pass_():
    sp = Sprite(W, H)
    sp.paint(rect(2, 3, 8, 6), ["$", "T", "9"], rim=True, fall=False)
    for y in (4, 6):
        sp.fill(rect(3, y, 5, 1), "O")
    sp.paint(ell(8.5, 8.5, 1.6, 1.6), ["!", "7", "R"], rim=False, fall=False)
    return sp.finish()


def scroll():
    sp = Sprite(W, H)
    sp.paint(rect(1, 4, 10, 4), ["$", "T", "9"], rim=True, fall=False)
    sp.fill(rect(1, 3, 1, 6), "O"); sp.fill(rect(10, 3, 1, 6), "O")
    sp.fill(rect(5, 4, 2, 4), "7")
    return sp.finish()


def shoe():
    sp = Sprite(W, H)
    ring = ell(6, 6.2, 4.2, 4.4) - ell(6, 5.0, 2.0, 2.4) - rect(4, 0, 4, 5)
    sp.paint(ring, ["g", "j", "h", "i"], rim=True, fall=False)
    for x, y in [(2, 5), (3, 8), (9, 8), (10, 5)]:
        sp.put(x, y, "k")
    return sp.finish()


def all_items():
    return {"item-ink": ink(), "item-reeds": reeds(), "item-pass": pass_(), "item-scroll": scroll(), "item-shoe": shoe()}


if __name__ == "__main__":
    fr = all_items()
    sheet([s.rows() for s in fr.values()], scale=10, cols=5, bg=(60, 58, 52)).save("/work/tmp/mf/items.png")
