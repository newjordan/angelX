"""The snuffers: hooded wisps with two pale eyes, a smoke tail and a glowing wick.
Facing right; the tail trails left. Canvas 20 x 20, feet on the last row."""
from engine import parse, shift, over, outline, rows, render, recolor

W, H = 20, 20

HOOD = [
    "......xx......",
    ".....xSSx.....",
    "....xSuuSx....",
    "...xSuuuuSx...",
    "..xSuuuuuuSx..",
    ".xxxxxxxxxxxx.",
    ".xSSWWSSSWWSx.",
    ".xSSWkSSSWkSx.",
    ".xSSSSSSSSSSx.",
    "..xSSSuuSSSx..",
    "...xSSSSSSx...",
]

def snuffer(step=0, eyes_dx=1):
    body = parse(HOOD, 4, 4)
    # the eyes look the way it walks: move the pupils
    body = dict(body)
    for (x, y), c in list(body.items()):
        if c == "W" and y in (10, 11):
            pass
    # a smoke tail trailing back and its wick stub, glowing at the end
    t = step % 2
    tail = parse([
        "..hjj",
        ".hJjg",
        "hjgg.",
    ], 0, 12 + t)
    wick = {(0, 14 + t): "O", (1, 13 + t): "6", (0, 13 + t): "4"} if t == 0 else {(0, 13 + t): "O", (1, 12 + t): "6", (0, 12 + t): "4"}
    # two stubby feet that shuffle
    feet = parse(["xx..xx"], 7 + (1 if t else 0), 15) if True else {}
    feet = {(x, 15): "x" for x in (7, 8, 12, 13)} if t == 0 else {(x, 15): "x" for x in (8, 9, 11, 12)}
    return over(tail, wick, body, feet)

def snuffer_flat():
    """Stomped flat: a pancake with spiral eyes."""
    s = parse([
        "xSSSSSSSSSSSSx",
        "xSWkSSSSSkWSSx",
        "xxxxxxxxxxxxxx",
    ], 4, 13)
    return s

CAP = [
    "..uUUUu..",
    ".uUhhhUu.",
    "uUhhhhhUu",
    "TuuuuuuuT",
]

def capped(step=0):
    s = dict(snuffer(step))
    cap = parse(CAP, 7, 4)
    # the cap replaces the point of the hood
    for (x, y) in list(s):
        if y <= 6:
            s.pop((x, y))
    return over(s, shift(cap, 0, 3))

def cap_item(frame=0):
    """The dented iron bowl left behind, 14 x 9."""
    return parse([
        "..UUUUUU..",
        ".UhhhiiUu.",
        "UhhhhhhhUu",
        "uuuuuuuuuu",
        "jTj.jj.jTj"[:10],
    ], 3, 11)

def wisp(step=0):
    """The floating wisp, a pale flame with two dark eyes. 12 x 14."""
    flick = step % 2
    s = parse([
        "....WW....",
        "...WVVW...",
        "..WVVVVW..",
        ".WVVVVVVW.",
        ".VVkVVkVV.",
        ".VVkVVkVV.",
        ".zVVVVVVz.",
        "..zVVVVz..",
        "...zQQz...",
    ], 5, 4)
    tail = {(9 + flick, 14): "O", (10 - flick, 15): "6"}
    return over(s, tail)

def loaf():
    return parse([
        "..tttt..",
        ".tOOOOt.",
        "tOOttOOo",
        "tOtOOtOo",
        "ooooooor",
        ".rrrrrr.",
    ], 6, 14)

def frames():
    f = {
        "snuffer": [outline(snuffer(0), "k"), outline(snuffer(1), "k")],
        "snuffer_flat": [outline(snuffer_flat(), "k")],
        "capped": [outline(capped(0), "k"), outline(capped(1), "k")],
        "cap": [outline(cap_item(), "k")],
        "wisp": [outline(wisp(0), "k"), outline(wisp(1), "k")],
        "loaf": [outline(loaf(), "k")],
    }
    return f

if __name__ == "__main__":
    f = frames()
    allf = [s for k in f for s in f[k]]
    render(allf, W, H, zoom=12, cols=8).save("/work/tmp/bp/art/foes.png")
