"""A small three-view rig for Moonfall's people: front (down), back (up) and
side (left; right is the mirror). 18 by 28 canvas, bottom-centre anchor, feet
on row 26, the figure centred on pixel column 9. Each person is a `Cfg`:
colours for the skin, tunic, sleeves, legs and boots, a body shape, and three
hooks that draw what makes them themselves (`back`, `hat`, `front`) for each
view and pose.

A pose is (kind, ph): `idle` phases 0..3, `walk` phases 0..3. Bob and limb
swing come from the phase; `extra` carries what a particular frame needs."""
from engine import *

W, H = 18, 28
CX = 9.5
FOOT = 26


class Cfg:
    def __init__(self, **kw):
        self.skin = ["$", "c", "9"]
        self.hair = ["b", "I", "p"]
        self.tunic = ["X", "S", "u"]
        self.sleeve = None
        self.legs = ["Z", "n", "I"]
        self.boot = ["k", "n", "b"]
        self.sh = 4.2               # shoulder half-width (front)
        self.hm = 4.4               # hem half-width (front)
        self.side_w = 3.2           # torso half-width (side)
        self.top = 12               # shoulder line
        self.hem = 21               # where the tunic ends
        self.head_r = (3.9, 3.8)
        self.head_y = 9.0
        self.head_dx = 0.0          # head leans forward in the side view
        self.stoop = 0
        self.belt = None
        self.eye = "k"
        self.eye_dy = 0.3
        self.leg_w = 3
        self.gait = 2
        self.hat = None
        self.back = None
        self.front = None
        self.__dict__.update(kw)
        if self.sleeve is None:
            self.sleeve = self.tunic


def px(sp, x, y, ch):
    sp.put(int(x), int(y), ch)


def legs_front(sp, c, kind, ph):
    lift_l = lift_r = 0
    if kind == "walk":
        lift_l = [0, 1, 0, 0][ph % 4]
        lift_r = [0, 0, 0, 1][ph % 4]
    lw = c.leg_w
    top = c.hem - 1
    for x0, lift in ((9 - lw, lift_l), (10, lift_r)):
        sp.paint(rect(x0, top, lw, FOOT - 1 - top - lift), c.legs, rim=True, fall=False)
        sp.paint(rect(x0 - (1 if x0 < 9 else 0), FOOT - 2 - lift, lw + 1, 2), c.boot, rim=True, fall=False)


def legs_side(sp, c, kind, ph):
    s = 0
    up = 0
    if kind == "walk":
        s = [c.gait, 0, -c.gait, 0][ph % 4]
        up = [0, 1, 0, 1][ph % 4]
    lw = c.leg_w
    top = c.hem - 1
    x_far = 9 - lw // 2 - s
    x_near = 9 - lw // 2 + s
    # far leg, in shadow
    sp.paint(rect(x_far, top, lw, FOOT - 1 - top - (up if s == 0 else 0)), [c.legs[0], c.legs[0], c.legs[1]], rim=False, fall=False)
    sp.paint(rect(x_far - 1, FOOT - 2 - (up if s == 0 else 0), lw + 1, 2), [c.boot[0], c.boot[0], c.boot[1]], rim=False, fall=False)
    sp.paint(rect(x_near, top, lw, FOOT - 1 - top), c.legs, rim=True, fall=False)
    sp.paint(rect(x_near - 1, FOOT - 2, lw + 1, 2), c.boot, rim=True, fall=False)


def torso_front(sp, c, bob):
    top = c.top + bob + c.stoop
    body = poly([(CX - c.sh, top), (CX + c.sh, top), (CX + c.hm, c.hem), (CX - c.hm, c.hem)])
    sp.paint(body, c.tunic, rim=True, fall=False)
    # a fold of cloth down the right of the chest, a belt, and the hem
    for y in range(top + 2, c.hem - 1):
        q = (int(CX + c.sh * 0.35), y)
        if q in body and y % 3 != 2:
            sp.px[q] = c.tunic[0]
    if c.belt:
        by = top + 7
        for x in range(int(CX - c.hm) + 1, int(CX + c.hm)):
            if (x, by) in body:
                sp.px[(x, by)] = c.belt[0]
        px(sp, CX - 0.5, by, c.belt[1])
        px(sp, CX + 0.5, by, c.belt[1]) if False else None
    for x in range(int(CX - c.hm) + 1, int(CX + c.hm)):
        if (x, c.hem - 1) in body:
            sp.px[(x, c.hem - 1)] = c.tunic[0]
    return body


def torso_side(sp, c, bob):
    top = c.top + bob + c.stoop
    body = poly([(CX - c.side_w, top), (CX + c.side_w, top), (CX + c.side_w + 0.6, c.hem), (CX - c.side_w - 0.2, c.hem)])
    sp.paint(body, c.tunic, rim=True, fall=False)
    if c.belt:
        by = top + 7
        for x in range(int(CX - c.side_w) - 1, int(CX + c.side_w) + 2):
            if (x, by) in body:
                sp.px[(x, by)] = c.belt[0]
        px(sp, CX - c.side_w + 0.2, by, c.belt[1])
    for x in range(int(CX - c.side_w) - 1, int(CX + c.side_w) + 2):
        if (x, c.hem - 1) in body:
            sp.px[(x, c.hem - 1)] = c.tunic[0]
    return body


def arm_swing(kind, ph):
    if kind == "walk":
        return [(0, 0), (1, -1), (0, 0), (-1, 1)][ph % 4]
    return (0, 0)


def arms_front(sp, c, bob, kind, ph, skip=()):
    """Both arms hanging; returns the hand points (left, right)."""
    top = c.top + bob + c.stoop
    sw = arm_swing(kind, ph)
    hands = {}
    for side, off in ((-1, sw[0]), (1, sw[1])):
        sx = CX + side * (c.sh + 0.2)
        hx = CX + side * (c.sh + 0.7)
        hy = top + 6.5 + off
        hands[side] = (hx, hy)
        if side in skip:
            continue
        arm = cap(sx, top + 1.4, hx, hy - 1.0, 1.1)
        sp.paint(arm, c.sleeve, rim=True, fall=False)
        for (ax, ay) in arm:
            if (ax - side, ay) not in arm and ay > top + 1:
                sp.px[(ax, ay)] = c.sleeve[0]
        sp.fill(rect(int(hx - 0.9), int(hy - 1), 2, 2), c.skin[1])
        sp.put(int(hx - 0.9), int(hy - 1), c.skin[2])
    return hands


def arms_side(sp, c, bob, kind, ph, skip_near=False):
    top = c.top + bob + c.stoop
    sw = 0
    if kind == "walk":
        sw = [2, 0, -2, 0][ph % 4]
    fx = CX + 0.8
    sp.paint(cap(fx, top + 1.5, fx - sw * 0.5, top + 6.5, 1.0), [c.sleeve[0], c.sleeve[0], c.sleeve[1]], rim=False, fall=False)
    nx = CX - 0.5
    hx, hy = nx - sw * 0.6, top + 6.5
    if not skip_near:
        sp.paint(cap(nx, top + 1.5, hx, hy, 1.15), c.sleeve, rim=True, fall=False)
        sp.fill(rect(int(hx - 0.9), int(hy), 2, 2), c.skin[1])
    return (hx, hy)


def head_front(sp, c, bob, blink=False):
    hy = c.head_y + bob + c.stoop
    rx, ry = c.head_r
    head = ell(CX, hy, rx, ry)
    sp.paint(head, c.skin, rim=True, fall=True, soft=0.1)
    ey = int(hy + c.eye_dy)
    if blink:
        px(sp, CX - 2, ey, c.skin[0]); px(sp, CX + 1, ey, c.skin[0])
    else:
        px(sp, CX - 2, ey, c.eye); px(sp, CX + 1, ey, c.eye)
    return head


def head_back(sp, c, bob):
    hy = c.head_y + bob + c.stoop
    rx, ry = c.head_r
    head = ell(CX, hy, rx, ry)
    sp.paint(head, c.hair, rim=True, fall=True)
    return head


def head_side(sp, c, bob, blink=False):
    hy = c.head_y + bob + c.stoop
    rx, ry = c.head_r
    hx = CX - 0.5 - c.head_dx
    head = ell(hx, hy, rx - 0.2, ry)
    sp.paint(head, c.skin, rim=True, fall=True, soft=0.1)
    back = {(x, y) for (x, y) in head if x >= int(hx + 1.3)}
    sp.paint(back, c.hair, rim=False, fall=False)
    ex = int(hx - rx * 0.55)
    ey = int(hy + c.eye_dy)
    px(sp, ex, ey, c.skin[0] if blink else c.eye)
    px(sp, hx - rx + 0.3, ey + 1, c.skin[2])
    return head


def person(c, d, kind="idle", ph=0, bob=None, x=None):
    """One frame of person `c` facing `d` ('down', 'up', 'left')."""
    sp = Sprite(W, H)
    x = x or {}
    if bob is None:
        bob = [0, -1, 0, -1][ph % 4] if kind == "walk" else x.get("bob", 0)
    blink = x.get("blink", False)
    if d == "left":
        if c.back: c.back(sp, c, d, kind, ph, bob, x)
        legs_side(sp, c, kind, ph)
        torso_side(sp, c, bob)
        hand = arms_side(sp, c, bob, kind, ph, skip_near=x.get("skip_near", False))
        head_side(sp, c, bob, blink)
        if c.hat: c.hat(sp, c, d, kind, ph, bob, x)
        if c.front and not x.get("bare"): c.front(sp, c, d, kind, ph, bob, x, {"near": hand})
    elif d == "up":
        if c.back: c.back(sp, c, d, kind, ph, bob, x)
        legs_front(sp, c, kind, ph)
        torso_front(sp, c, bob)
        hands = arms_front(sp, c, bob, kind, ph, skip=x.get("skip", ()))
        head_back(sp, c, bob)
        if c.hat: c.hat(sp, c, d, kind, ph, bob, x)
        if c.front and not x.get("bare"): c.front(sp, c, d, kind, ph, bob, x, hands)
    else:
        if c.back: c.back(sp, c, d, kind, ph, bob, x)
        legs_front(sp, c, kind, ph)
        torso_front(sp, c, bob)
        hands = arms_front(sp, c, bob, kind, ph, skip=x.get("skip", ()))
        head_front(sp, c, bob, blink)
        if c.hat: c.hat(sp, c, d, kind, ph, bob, x)
        if c.front and not x.get("bare"): c.front(sp, c, d, kind, ph, bob, x, hands)
    return sp


def mirrored(sp):
    out = Sprite(sp.w, sp.h)
    for (x, y), ch in sp.px.items():
        out.px[(sp.w - 1 - x, y)] = ch
    return out


def arm_to(sp, c, side, bob, tx, ty, hand=None):
    """An arm from the `side` shoulder to a hand at (tx, ty)."""
    top = c.top + bob + c.stoop
    sx = CX + side * (c.sh + 0.2)
    arm = cap(sx, top + 1.4, tx, ty, 1.1)
    sp.paint(arm, c.sleeve, rim=True, fall=False)
    sp.fill(rect(int(tx - 0.9), int(ty - 0.9), 2, 2), hand or c.skin[1])
