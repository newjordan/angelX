import numpy as np
from ..anim import ANIMS

SS = 4


def _rot(a, px, py):
    c, s = np.cos(a), np.sin(a)
    return np.array([[c, -s, px - c * px + s * py], [s, c, py - s * px - c * py], [0, 0, 1.0]])


def fold(pose, ch):
    g = lambda k: pose.get(k, 0.0)
    amp = ch.amp
    out = {
        "root": g("root"),
        "torso": g("torso") * amp.get("torso", 1.0),
        "head": g("head") * amp.get("head", 1.0),
        "arm_f": (g("upper_arm_r") + 0.5 * g("lower_arm_r")) * amp.get("arm_f", 1.0),
        "arm_b": (g("upper_arm_l") + 0.5 * g("lower_arm_l")) * amp.get("arm_b", 1.0),
    }
    ul, ur = g("upper_leg_l"), g("upper_leg_r")
    d = np.degrees(ur - ul)
    out["leg_frame"] = 0 if abs(d) < 12 else (1 if d < 0 else 2)
    mean = 0.5 * (ul + ur)
    out["legs"] = mean * 0.5 if abs(np.degrees(mean)) > 20 else 0.0
    out["off"] = pose.get("root_off", (0.0, 0.0))
    return out

SNAP = {"root": 90, "torso": 90, "head": 90, "arm_b": 45, "arm_f": 45, "legs": 90}


def _snap(a, step):
    d = np.degrees(a)
    return np.radians(step * np.round(d / step)) if abs(d) >= step * 0.6 else 0.0


def _joint(a, piv, centre, step, scale):
    if step is None:
        return _rot(a, *piv)
    c, s = np.cos(a), np.sin(a)
    cx, cy = centre
    px, py = piv
    nx = px + c * (cx - px) - s * (cy - py)
    ny = py + s * (cx - px) + c * (cy - py)
    tx, ty = np.round(nx - cx), np.round(ny - cy)
    if scale > 1:
        tx, ty = np.round((nx - cx) / scale) * scale, np.round((ny - cy) / scale) * scale
    return np.array([[1, 0, tx], [0, 1, ty], [0, 0, 1.0]]) @ _rot(_snap(a, step), cx, cy)


def matrices(ch, b, centres=None, snap=None, scale=1, alts=None):
    P = {k: (v[0] * scale, v[1] * scale) for k, v in ch.pivots.items()}
    centres = centres or {}
    snap = snap or {}
    dx, dy = b["off"]
    dx, dy = np.round(dx * scale), np.round(dy * scale)
    alts = alts or {}

    def J(bone, piv):
        if bone in alts:
            return np.eye(3)
        return _joint(b[bone], P[piv], centres.get(bone, P[piv]), snap.get(bone), scale)
    root = np.array([[1, 0, dx], [0, 1, dy], [0, 0, 1.0]]) @ J("root", "hip")
    torso = root @ J("torso", "hip")
    return {"root": root, "torso": torso, "head": torso @ J("head", "neck"),
            "arm_b": torso @ J("arm_b", "shoulder_b"),
            "arm_f": torso @ J("arm_f", "shoulder_f"),
            "legs": root @ J("legs", "hip")}


class Renderer:
    def __init__(self, ch, pad=None, scale=1, mode="pixel"):
        self.ch, self.scale, self.mode = ch, scale, mode
        self.snap = SNAP if mode == "pixel" else {}
        self.pad = pad if pad is not None else max(ch.w, ch.h) // 2 + 2
        inks = sorted({c for _, g in ch.layers for c in g.ravel()} |
                      {c for g in ch.legs for c in g.ravel()} |
                      {c for g in getattr(ch, "alts", {}).values() for c in g.ravel()})
        self.inks = ["."] + [c for c in inks if c != "."]
        lut = {c: i for i, c in enumerate(self.inks)}
        enc = np.vectorize(lambda c: lut[c], otypes=[np.uint8])
        up = lambda a: np.repeat(np.repeat(a, scale, 0), scale, 1)
        self.layers = [(b, up(enc(g))) for b, g in ch.layers]
        self.legs = [up(enc(g)) for g in ch.legs]
        self.alts = {k: up(enc(g)) for k, g in getattr(ch, "alts", {}).items()}
        self.sw, self.sh = ch.w * scale, ch.h * scale
        self.centres = {}
        for b, g in self.layers + [("legs", self.legs[0])]:
            ys, xs = np.nonzero(g)
            if len(xs):
                self.centres[b] = (np.round(xs.mean() / scale) * scale, np.round(ys.mean() / scale) * scale)
        for k in ("root", "torso"):
            self.centres[k] = tuple(v * scale for v in ch.pivots["hip"])
        self.pad *= scale
        self.W, self.H = self.sw + 2 * self.pad, self.sh + 2 * self.pad
        o = (np.arange(SS) + 0.5) / SS
        ys = (np.arange(self.H)[:, None] + o[None]).ravel() - self.pad
        xs = (np.arange(self.W)[:, None] + o[None]).ravel() - self.pad
        X, Y = np.meshgrid(xs, ys)
        self.pts = np.stack([X.ravel(), Y.ravel(), np.ones(X.size)])

    def frame(self, pose):
        ch = self.ch
        b = fold(pose, ch)
        parts = dict(self.layers)
        alts = {}
        for (bone, deg), g in self.alts.items():
            if self.snap and round(np.degrees(_snap(b[bone], self.snap[bone]))) == deg:
                alts[bone] = parts[bone] = g
        M = matrices(ch, b, self.centres, self.snap, self.scale, alts)
        acc = np.zeros(self.pts.shape[1], np.uint8)
        for bone in ch.order:
            src = self.legs[b["leg_frame"]] if bone == "legs" else parts.get(bone)
            if src is None:
                continue
            inv = np.linalg.inv(M[bone])
            u, v, _ = inv @ self.pts
            iu, iv = np.floor(u).astype(np.int32), np.floor(v).astype(np.int32)
            ok = (iu >= 0) & (iv >= 0) & (iu < self.sw) & (iv < self.sh)
            val = np.zeros_like(acc)
            val[ok] = src[iv[ok], iu[ok]]
            hit = val > 0
            acc[hit] = val[hit]
        idx = self._resolve(acc)
        ys = np.nonzero(idx.any(1))[0]
        ground = self.pad + self.sh - 1
        if len(ys) and ys[-1] > ground:
            idx = np.roll(idx, ground - ys[-1], 0)
        if any(isinstance(v, float) and abs(v) > 1e-3 for k, v in b.items() if k != "leg_frame"):
            from ..doll.draw import cleanup
            cleanup(idx)
        return idx

    def _resolve(self, acc):
        k = SS * SS
        blk = acc.reshape(self.H, SS, self.W, SS).transpose(0, 2, 1, 3).reshape(self.H, self.W, k)
        n = len(self.inks)
        counts = (blk[..., None] == np.arange(n)).sum(2)
        empty = counts[..., 0].copy()
        counts[..., 0] = -1
        best = counts.argmax(-1).astype(np.uint8)
        best[empty > k * 3 // 4] = 0
        return best

    def to_rows(self, idx):
        lut = np.array(self.inks)
        return ["".join(r) for r in lut[idx]]


def render_anim(r, name, n=None):
    fn, dn, fps = ANIMS[name]
    n = n or dn
    h = r.ch.h * r.scale
    return [r.frame(fn(i / n, h)) for i in range(n)], fps, bool(getattr(fn, "loop", False))
