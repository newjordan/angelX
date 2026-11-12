"""The Warden: a hooded iron giant with a caged lamp in its belly (48 by 56,
anchored at the bottom centre). Parametric, so its fifty-odd frames are one
body in different poses. The lamp-heart is drawn by the stage over the
`HEART` point; the frames carry only the iron the cage hangs from."""
import math
from engine import *

W, H = 48, 56
HEART = (24, 37)

CLOTH = ["s", "x", "S", "u"]
CLOTH_D = ["K", "s", "x", "S"]
IRON = ["X", "g", "j", "G"]
IRON_L = ["g", "j", "G", "J"]
IRON_D = ["k", "K", "X", "g"]
BRASS = ["a", "R", "@", "6"]


def deg(a):
    return math.radians(a)


def arm(sp, side, shoulder, a1, a2, grip=False, raise_hand=None):
    """One arm from `shoulder`: upper arm at a1 degrees from straight down
    (outward positive), forearm a2 from the upper arm's line."""
    sx, sy = shoulder
    sgn = -1 if side == "L" else 1
    l1, l2 = 9, 8
    ex = sx + sgn * math.sin(deg(a1)) * l1
    ey = sy + math.cos(deg(a1)) * l1
    a = a1 + a2
    hx = ex + sgn * math.sin(deg(a)) * l2
    hy = ey + math.cos(deg(a)) * l2
    sp.paint(cap(sx, sy, ex, ey, 3.4, 3.0), CLOTH)
    sp.paint(cap(ex, ey, hx, hy, 3.4, 3.0), IRON_L)
    # a vambrace band and the elbow cop
    sp.paint(ell(ex, ey, 3.6, 3.6), IRON_L)
    bx, by = ex + (hx - ex) * 0.55, ey + (hy - ey) * 0.55
    sp.fill(cap(bx, by, bx, by, 3.4), "X")
    sp.put(int(bx), int(by), "G")
    # the gauntlet
    sp.paint(ell(hx, hy + 0.5, 3.6, 3.4), IRON)
    for k in (-1.5, 0.0, 1.5):
        fx, fy = hx + k + sgn * 0.0, hy + 2.6
        sp.put(int(round(fx)), int(round(fy)), "k")
    return hx, hy


def lantern(sp, x, y, glow=1):
    """A small hanging lantern: cap, glass, base. Light is the accent."""
    sp.fill(rect(x - 1, y - 1, 3, 1), "g")
    sp.fill(rect(x - 2, y, 5, 4), "g")
    sp.fill(rect(x - 1, y + 1, 3, 2), "6" if glow else "R")
    sp.put(x, y + 1, "w" if glow == 2 else "5")
    sp.fill(rect(x - 1, y + 4, 3, 1), "X")


def body(
    bob=0, lean=0, la=(14, 5), ra=(14, 5), eyes=2, hood_dy=0, hood_dx=0, lan=0.0,
    hem=0, crouch=0, peak=0, sag=0, lamp_glow=1, face_dark=False, eye_ink=None,
):
    sp = Sprite(W, H)
    by = bob + crouch
    lx = lean
    # ---- the hem: a ragged cloth skirt, wide at the floor
    base_y = 55
    top_y = 36 + by
    pts = [(14 + lx, top_y), (34 + lx, top_y), (43 + hem, base_y), (5 - hem, base_y)]
    sp.paint(poly(pts), CLOTH_D, fall=True)
    # tatters along the bottom edge
    for x in range(5 - hem, 44 + hem):
        h = 2 + ((x * 5 + 3 * (x // 4)) % 4)
        if (x + hem) % 3 == 0:
            sp.px.pop((x, base_y), None)
            if h > 3:
                sp.px.pop((x, base_y - 1), None)
    # folds
    for fx in (13, 20, 28, 36):
        for y in range(top_y + 5, base_y - 1):
            if (y + fx) % 3:
                sp.put(fx + (y - top_y) // 4 * (1 if fx > 24 else -1) + hem // 2, y, "K")
    # two iron toes under the hem
    sp.paint(ell(15, 54, 4, 2), IRON_D, rim=False)
    sp.paint(ell(33, 54, 4, 2), IRON_D, rim=False)
    # ---- cloak behind the shoulders
    sp.paint(poly([(9 + lx, 17 + by), (39 + lx, 17 + by), (43 + lx, 31 + by), (5 + lx, 31 + by)]), CLOTH_D)
    # ---- torso: breastplate over a belly plate
    torso = poly([(12 + lx, 17 + by), (36 + lx, 17 + by), (34 + lx, 30 + by), (31 + lx, 43 + by + sag), (17 + lx, 43 + by + sag), (14 + lx, 30 + by)])
    sp.paint(torso, IRON)
    # plate lines
    for y, a in ((24, 1), (31, 0), (39, 1)):
        yy = y + by
        for x in range(15 + lx + a, 34 + lx - a):
            if (x, yy) in sp.px and sp.px[(x, yy)] != "k":
                sp.px[(x, yy)] = "K"
        for x in range(15 + lx + a, 34 + lx - a):
            if (x, yy + 1) in sp.px and (x * 2 + y) % 5 == 0:
                sp.px[(x, yy + 1)] = "G"
    # a breastbone ridge and rivets
    for y in range(18 + by, 30 + by):
        if (24 + lx, y) in sp.px:
            sp.px[(24 + lx, y)] = "G" if y % 2 else "j"
    for rx, ry in ((16, 20), (32, 20), (18, 28), (30, 28), (19, 38), (29, 38)):
        sp.put(rx + lx, ry + by, "J")
    # belt and the buckle the cage hangs from
    sp.fill(rect(15 + lx, 41 + by + sag, 19, 2), "X")
    sp.fill(rect(15 + lx, 41 + by + sag, 19, 1), "j")
    sp.fill(rect(22 + lx, 40 + by + sag, 5, 4), "g")
    sp.put(24 + lx, 41 + by + sag, "6")
    # ---- two small lanterns on short chains from the belt
    sw = int(round(math.sin(lan) * 2))
    for side, x0 in ((-1, 12), (1, 36)):
        cx = x0 + lx
        for k in range(4):
            sp.put(cx + (sw if k > 1 else 0) * (k - 1) // 2, 42 + by + sag + k, "h" if k % 2 else "J")
        lantern(sp, cx + sw, 46 + by + sag, lamp_glow)
    # ---- pauldrons, over the arms' roots
    for side, cx in ((-1, 10), (1, 38)):
        sp.paint(ell(cx + lx, 20 + by, 7.2, 6.2), IRON_L)
        sp.paint(ell(cx + lx, 19 + by, 4.6, 3.6), IRON_L)
        sp.fill(rect(cx + lx - 5, 24 + by, 11, 1), "X")
        # a short horn on each shoulder, tilted out
        sp.paint(poly([(cx + lx + side * 1, 15 + by), (cx + lx + side * 4, 8 + by), (cx + lx + side * 5, 15 + by)]), IRON_L)
    # ---- arms (drawn after the pauldrons' base so they hang from under them)
    arm(sp, "L", (10 + lx, 23 + by), la[0], la[1])
    arm(sp, "R", (38 + lx, 23 + by), ra[0], ra[1])
    # ---- the hood
    hx, hy = 24 + lx + hood_dx, 11 + by + hood_dy
    # the cowl falling over the shoulders
    sp.paint(poly([(hx - 12, hy + 5), (hx + 12, hy + 5), (hx + 14, hy + 14), (hx - 14, hy + 14)]), CLOTH)
    sp.paint(ell(hx, hy, 10.2, 10.6), CLOTH)
    sp.paint(poly([(hx - 7, hy - 6), (hx, hy - 15 - peak), (hx + 7, hy - 6)]), CLOTH)
    # the face-hole: black, a cold rim
    sp.fill(ell(hx, hy + 2, 6.4, 6.9), "K" if face_dark else "k")
    sp.fill(ell(hx, hy + 2, 5.2, 5.8), "k")
    # eyes
    ink = eye_ink or {0: "S", 1: "2", 2: "3", 3: "w", 4: "6", 5: "w"}[eyes]
    ey = hy + 1
    if eyes == 0:
        sp.fill(rect(hx - 4, ey + 1, 3, 1), "S")
        sp.fill(rect(hx + 2, ey + 1, 3, 1), "S")
    elif eyes == 1:
        sp.put(hx - 3, ey, ink); sp.put(hx + 3, ey, ink)
    else:
        sp.fill(rect(hx - 4, ey, 3, 2), ink)
        sp.fill(rect(hx + 2, ey, 3, 2), ink)
        if eyes >= 3:
            sp.fill(rect(hx - 5, ey, 1, 2), "3"); sp.fill(rect(hx + 5, ey, 1, 2), "3")
            sp.put(hx - 3, ey - 1, "3"); sp.put(hx + 3, ey - 1, "3")
    # the hood's lower hem over the brow: a fold line
    for x in range(int(hx - 8), int(hx + 9)):
        if (x, hy - 5) in sp.px and sp.px[(x, hy - 5)] != "k":
            sp.px[(x, hy - 5)] = "s"
    return sp.finish()


def rows_of(sp):
    return sp.rows()


if __name__ == "__main__":
    s = body()
    png(s.rows(), 10).save("/work/tmp/lw/warden_idle.png")


# ---- the frames ------------------------------------------------------------

def hand_of(side, shoulder_x, a1, a2, lx=0, by=0):
    """Where `arm` puts a hand (kept in step with `arm`)."""
    sgn = -1 if side == "L" else 1
    sx, sy = shoulder_x + lx, 23 + by
    ex = sx + sgn * math.sin(deg(a1)) * 9
    ey = sy + math.cos(deg(a1)) * 9
    a = a1 + a2
    return ex + sgn * math.sin(deg(a)) * 8, ey + math.cos(deg(a)) * 8


def orb(sp, x, y, r, big=False):
    """A ball of lamp light: white core, gold body, a dithered amber rim."""
    x, y = int(round(x)), int(round(y))
    sp.fill(ell(x, y, r + 1.5, r + 1.5), "@")
    for (px, py) in ell(x, y, r + 2.2, r + 2.2):
        if (px, py) not in sp.px and bayer(px, py) < 0.5:
            sp.put(px, py, "4")
    sp.fill(ell(x, y, r, r), "6")
    sp.fill(ell(x, y, max(1, r - 1.5), max(1, r - 1.5)), "w")


def lerp(a, b, t):
    return a + (b - a) * t


def frames():
    out = {}
    B = body

    # sleep: slumped, head bowed, eyes shut, the lanterns burning low
    out["sleep"] = [B(bob=1, crouch=1, hood_dy=3, eyes=0, la=(18, 6), ra=(18, 6), lamp_glow=0, lean=0)]
    # idle: a slow breath, the lanterns swinging a little
    out["idle"] = [B(bob=0, lan=0.0, eyes=2), B(bob=1, lan=math.pi, eyes=2, hood_dy=0)]
    # walk: heavy steps, the hem and the lanterns swaying
    out["walk"] = [
        B(bob=0, lean=-1, hem=1, lan=0.0, la=(10, 5), ra=(18, 5)),
        B(bob=1, lean=0, hem=0, lan=1.6, la=(14, 5), ra=(14, 5)),
        B(bob=0, lean=1, hem=-1, lan=3.1, la=(18, 5), ra=(10, 5)),
        B(bob=1, lean=0, hem=0, lan=4.7, la=(14, 5), ra=(14, 5)),
    ]

    # beam-tell: the right hand rises beside the hood, light gathers in the palm
    tell = []
    for i in range(7):
        t = i / 6
        a1 = lerp(14, 28, t)
        a2 = lerp(5, 150, t)
        ink = ["3", "3", "5", "5", "6", "6", "w"][i]
        sp = B(bob=i % 2, lean=-int(t * 1.5), ra=(a1, a2), la=(lerp(14, 20, t), 8), eyes=4 if i > 1 else 2, eye_ink=ink, lamp_glow=2 if i > 3 else 1, peak=int(t * 2), lan=i * 0.7)
        hx, hy = hand_of("R", 38, a1, a2, -int(t * 1.5))
        if i >= 2:
            orb(sp, hx - 2, hy - 5, [0, 0, 1, 1, 2, 2, 3][i])
        tell.append(sp)
    out["beam-tell"] = tell

    # beam: the palm blazing, a recoil, the eyes white
    beam = []
    for i in range(6):
        sp = B(bob=0, lean=-2 + (i % 2), ra=(28, 150 + (i % 2) * 4), la=(22, 8), eyes=5, lamp_glow=2, peak=2, lan=i)
        hx, hy = hand_of("R", 38, 28, 150, -2)
        orb(sp, hx - 2, hy - 6, 3 + (i % 2))
        for k, (dx, dy) in enumerate([(5, -3), (6, 1), (-4, -4), (0, -8)]):
            if (i + k) % 2:
                sp.put(int(hx - 2 + dx), int(hy - 6 + dy), "w")
        beam.append(sp)
    out["beam"] = beam

    # slam-tell: both fists rise over the head, the body leans back
    st = []
    for i in range(7):
        t = i / 6
        a = lerp(14, 166, t)
        a2 = lerp(5, 2, t)
        ink = ["3", "3", "5", "5", "6", "6", "w"][i]
        sp = B(bob=0, lean=-int(t * 1.4), crouch=-int(t * 1), la=(a, a2), ra=(a, a2), eyes=4 if i > 1 else 2, eye_ink=ink, hood_dy=int(t * 1), peak=int(t * 2), lamp_glow=2 if i > 3 else 1, lan=i * 0.8)
        st.append(sp)
    out["slam-tell"] = st

    # slam: the fists crash down, the body folds over them, the floor jumps
    sl = []
    for i, (a, c, hd) in enumerate([(120, 0, 0), (62, 2, 1), (24, 4, 3), (20, 3, 3), (16, 2, 2)]):
        sp = B(bob=0, lean=0, crouch=c, la=(a, 10), ra=(a, 10), eyes=5, hood_dy=hd, lamp_glow=2, lan=i * 1.2, hem=2 if i in (2, 3) else 0)
        if i >= 1:
            # rubble thrown up either side of the fists, bigger and higher as it lands
            chunks = [(-14, 0, 2), (-9, -4, 2), (-18, -7, 1), (14, -1, 2), (10, -5, 2), (19, -8, 1), (0, -9, 1), (-4, -12, 1), (5, -11, 1)]
            for k, (dx, dy, sz) in enumerate(chunks):
                rise = [0, 2, 4, 3, 1][i]
                if (k + i) % 2 == 0 or i in (2, 3):
                    cx_, cy_ = 24 + dx * (1 + (i - 1) // 3 * 0), 49 + dy - rise
                    sp.fill(rect(cx_, cy_, sz, sz), "J")
                    sp.put(cx_, cy_ + sz - 1 + 0, "G")
        sl.append(sp)
    out["slam"] = sl

    # summon: arms flung wide and up, the hem lifting, the lanterns flaring
    out["summon"] = [
        B(crouch=-1, peak=2, la=(30, 110 + k * 3), ra=(30, 110 + k * 3), eyes=4, hem=k % 3, lamp_glow=2, eye_ink=("6" if k % 2 else "w"), lan=k * 1.5)
        for k in range(4)
    ]

    # hit: knocked back a step, the head jerking
    out["hit"] = [
        B(lean=-2, hood_dy=1, hood_dx=-1, eyes=1, la=(30, 12), ra=(36, 12), bob=1),
        B(lean=-1, hood_dy=0, hood_dx=-1, eyes=2, la=(24, 10), ra=(26, 10), bob=0),
    ]

    # rear: it draws itself up to its full height with a roar
    rear = []
    for i in range(6):
        t = i / 5
        sp = B(bob=0, crouch=-int(t * 2), lean=-int(t * 2), la=(lerp(14, 150, t), lerp(5, 6, t)), ra=(lerp(14, 150, t), lerp(5, 6, t)),
               eyes=5 if i > 2 else 4, hood_dy=-int(t * 2), peak=int(t * 3), lamp_glow=2, hem=int(t * 2), lan=i * 1.3, eye_ink=("w" if i > 2 else "6"))
        rear.append(sp)
    out["rear"] = rear

    # stagger: reeling, the head lolling, the lamp-heart bare to the light
    out["stagger"] = [
        B(bob=1, crouch=2, lean=-1, hood_dy=3, hood_dx=-2, eyes=1, la=(24, 14), ra=(30, 12), lan=0.0, sag=1),
        B(bob=2, crouch=3, lean=0, hood_dy=4, hood_dx=0, eyes=0, la=(28, 14), ra=(24, 12), lan=2.0, sag=1),
        B(bob=1, crouch=2, lean=1, hood_dy=3, hood_dx=2, eyes=1, la=(30, 12), ra=(24, 14), lan=4.0, sag=1),
    ]

    # die: it folds, crumbles and leaves a heap of iron and one low light
    die = []
    base = B(bob=1, crouch=2, hood_dy=3, eyes=1, la=(30, 14), ra=(30, 14), sag=1)
    for i in range(8):
        k = i / 7
        sp = crumble(base, k, i)
        die.append(sp)
    out["die"] = die

    # the lamp-heart overlays: a socket's worth of cage, bright or dark
    out["heart-shut"] = [heart_frame(open_=0, i=0)]
    out["heart-open"] = [heart_frame(open_=1, i=0), heart_frame(open_=1, i=1)]
    return out


def crumble(sp, k, i):
    """The body folding down on itself: squashed from the top, the rim of it
    eaten away, a pile at the foot."""
    out = Sprite(W, H)
    keep = 1.0 - 0.70 * (k ** 0.8)
    for (x, y), c in sp.px.items():
        # squash toward the floor
        ny = H - 1 - int(round((H - 1 - y) * keep))
        # eat the pixels thinly toward the end
        if k > 0.15:
            h = (x * 37 + y * 91 + i * 17) % 100
            if h < k * 40 and y < 46:
                continue
        out.px[(x, ny)] = c
    # spread the heap
    spread = int(k * 3)
    heap = Sprite(W, H)
    for (x, y), c in out.px.items():
        if k > 0.45:
            heap.px[(24 + int((x - 24) * (1 + k * 0.25)), y)] = c
        else:
            heap.px[(x, y)] = c
    out = heap
    # rolling lanterns and a last spark at the end
    if k > 0.55:
        for dx in (-15, 13):
            lantern(out, 24 + dx + int(k * 3) * (1 if dx > 0 else -1), 51, 1 if k < 0.9 else 0)
    if k > 0.55:
        # the lamp-heart, fallen out of its cage, burning low in the iron
        r = 2 if k < 0.9 else 1
        out.fill(ell(24, 49, r + 0.5, r), "@" if i % 2 else "4")
        out.put(24, 49, "w" if k < 0.9 else "6")
    out.finish()
    return out


def heart_frame(open_, i):
    """The caged lamp that hangs over HEART: iron bars, glass, the flame."""
    sp = Sprite(W, H)
    cx, cy = HEART
    sp.fill(ell(cx, cy, 8.2, 8.2), "k")
    sp.fill(ell(cx, cy, 7, 7), "g")
    if open_:
        sp.fill(ell(cx, cy, 5.8, 5.8), "R" if i else "7")
        sp.fill(ell(cx, cy, 4.6, 4.6), "@")
        sp.fill(ell(cx, cy, 3.2, 3.2), "6")
        sp.fill(ell(cx, cy, 1.8, 1.8), "w")
    else:
        sp.fill(ell(cx, cy, 5.8, 5.8), "s")
        sp.fill(ell(cx, cy, 4.6, 4.6), "S")
        sp.fill(ell(cx, cy, 3.0, 3.0), "u")
        sp.put(cx - 1, cy - 1, "3")
    # the cage: a ring and four bars
    for (x, y) in ell(cx, cy, 7.4, 7.4) - ell(cx, cy, 6.2, 6.2):
        sp.put(x, y, "j")
    for dx in (-5, -2, 2, 5):
        for y in range(cy - 6, cy + 7):
            if (cx + dx, y) in sp.px:
                sp.px[(cx + dx, y)] = "h" if dy_ok(y, cy) else sp.px[(cx + dx, y)]
    sp.fill(rect(cx - 2, cy - 9, 5, 2), "G")
    sp.fill(rect(cx - 1, cy - 11, 3, 2), "j")
    return sp


def dy_ok(y, cy):
    return True
