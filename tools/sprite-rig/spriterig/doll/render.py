"""Paper-doll animator: draws a character procedurally from the projected Mixamo skeleton.

Every limb is a capsule chain; each body *group* (leg, leg, torso, arm, arm, head, cape,
weapon, ...) is drawn outline-pass-then-fill-pass, so joints inside a group never seam, and
groups are painter-sorted by camera depth, so occlusion is right from every camera angle
(no hidden "behind the limb" hole to fill). Armour rides the same capsules (material
override + grow), weapons/hats are pixel sprites hung off bone sockets, cape + plume are
Verlet spring chains. Output is a delve-ink index canvas; colours are palette-swappable
materials. ~1-2 ms per 48 px frame.
"""
import json, os
import numpy as np
from ..skel import skeleton as S
from ..skel.billboard import Camera
from . import draw as Dr

HERE = os.path.dirname(__file__)
LIB = os.path.join(HERE, "library")
SPRITE_CODE = {"M": ("metal", 0), "m": ("metal", 1), "L": ("light", 0), "G": ("gold", 0), "g": ("gold", 1),
               "W": ("wood", 0), "w": ("wood", 1), "P": ("primary", 0), "p": ("primary", 1),
               "S": ("secondary", 0), "s": ("secondary", 1), "T": ("bone", 0), "t": ("bone", 1),
               "k": ("outline", 0)}
I = S.INDEX


def _load(name):
    with open(os.path.join(LIB, name)) as f:
        return json.load(f)


def load_kit():
    eq = _load("equipment.json")
    return eq["materials"], eq["items"], _load("bodies.json")


class Loadout:
    """{"name":..,"body":"hero","colors":{"primary":["1","q"]},"equip":["sword","helm_knight",..]}"""

    def __init__(self, spec, kit=None):
        if isinstance(spec, str):
            named = os.path.join(LIB, "loadouts", spec + ".json")
            if not spec.endswith(".json") and os.path.exists(named):
                spec = named          # a library loadout name ("knight")
            spec = json.load(open(spec)) if spec.endswith(".json") else {"body": spec}
        self.spec = spec
        mats, items, bodies = kit or load_kit()
        self.name = spec.get("name", spec.get("body", "hero"))
        bad = set(spec) - {"name", "body", "equip", "colors"}
        if bad:
            raise ValueError(f"loadout {self.name}: unknown key(s) {sorted(bad)} (use name/body/equip/colors)")
        if spec.get("body", "hero") not in bodies:
            raise ValueError(f"loadout {self.name}: unknown body {spec.get('body')!r} (have {sorted(bodies)})")
        for e in spec.get("equip", []):
            if isinstance(e, str) and e not in items:
                raise ValueError(f"loadout {self.name}: unknown item {e!r} (have {sorted(items)})")
        self.body = dict(bodies[spec.get("body", "hero")])
        self.mat = {**mats, **self.body.get("materials", {}), **spec.get("colors", {})}
        self.items = []
        slots = {}
        for e in spec.get("equip", []):
            it = dict(items[e]) if isinstance(e, str) else dict(e)
            it["id"] = e if isinstance(e, str) else it.get("id", "custom")
            if "colors" in it:
                it["mat"] = {**self.mat, **it["colors"]}
            slots[it.get("slot", it["id"])] = it    # later items replace earlier in the same slot
        self.items = list(slots.values())

    def by_kind(self, *kinds):
        return [i for i in self.items if i["kind"] in kinds]

    def weapon_type(self):
        for i in self.items:
            if i["kind"] == "weapon" and i.get("slot") == "main":
                return i["type"]
        for i in self.items:
            if i["kind"] in ("weapon", "shield"):
                return i["type"]
        return "unarmed"


class Spring:
    """Verlet chain hanging from an anchor; points live in character space (metres)."""

    def __init__(self, n, seg, grav=9.8, damp=0.86, stiff=0.0):
        self.n, self.seg, self.g, self.damp, self.stiff = n, seg, grav, damp, stiff
        self.p = self.q = None

    def step(self, anchor, rest_dir, dt, push=None, iters=3):
        if self.p is None:
            self.p = anchor + np.outer(np.arange(self.n + 1), rest_dir) * self.seg
            self.q = self.p.copy()
        p, q = self.p, self.q
        v = (p - q) * self.damp
        q[:] = p
        acc = np.array([0, -self.g, 0]) * dt * dt
        p[1:] += v[1:] + acc
        if self.stiff:   # pull toward rest shape (plumes, hair)
            target = anchor + np.outer(np.arange(self.n + 1), rest_dir) * self.seg
            p[1:] += (target[1:] - p[1:]) * self.stiff
        p[0] = anchor
        for _ in range(iters):
            for i in range(self.n):
                d = p[i + 1] - p[i]
                L = np.linalg.norm(d) or 1e-9
                p[i + 1] = p[i] + d * (self.seg / L)
            if push is not None:
                push(p)
        return p


GROUPS = {"leg": ("Leg", "Foot", "ToeBase", "Toe_End"), "spine": ("Spine", "Spine1", "Spine2"),
          "neck": ("Neck", "Head"), "arm": ("ForeArm", "Hand", "HandIndex1"), "shoulder": ("Shoulder",)}
DELVE = {"leg": 0.62, "spine": 0.82, "neck": 0.55, "arm": 0.74, "shoulder": 1.0, "shoulder_width": 1.35, "hip_width": 1.45}


def proportioned_offsets(prop):
    """Bone-length scales per group -> (offset array, hips scale). Clip rotations are unchanged,
    so every Mixamo / .clip motion plays on the stout delve body as-is."""
    k = {**DELVE, **prop}
    off = S.OFFSET.copy()
    for i, n in enumerate(S.NAMES):
        b = n.replace("Left", "").replace("Right", "")
        for g, names in GROUPS.items():
            if b in names:
                off[i] *= k[g]
        if b == "UpLeg":
            off[i, 0] *= k["hip_width"]
        if b == "Arm":          # broad delve shoulders: wider yoke, arms keep their own length scale
            off[i, 0] *= k["shoulder_width"]
    off[I["Hips"], 1] *= k["leg"]
    return off, np.array([1.0, k["leg"], 1.0])


class Doll:
    def __init__(self, loadout, height_px=None):
        self.lo = loadout if isinstance(loadout, Loadout) else Loadout(loadout)
        b = self.lo.body
        self.h = height_px or b["height_px"]
        # delve silhouette: every body shares one stout/chibi proportion set (short legs,
        # compact spine, big helm-sized head) so all classes read as the same family of knights
        self.offset, self.leg_k = proportioned_offsets(b.get("proportions", {}))
        self.wk = b.get("weapon_scale", 0.72)     # props shrink with the stout body
        self.lift = b.get("head_lift_m", b["head_m"] * 0.62)
        _, Xr = S.fk(np.tile(np.eye(3), (len(S.NAMES), 1, 1)), offset=self.offset)
        self.top_m = Xr[I["Head"], 1] + self.lift + b["head_m"]   # head top over the floor (rest)
        self.ppm = self.h / (self.top_m + 0.02)
        self.r = {k: v * self.ppm for k, v in b["radii_m"].items()}
        self.hr = b["head_m"] * self.ppm
        self._palette_inks = None
        self.reset()

    # ------------------------------------------------------------- materials
    def ink(self, mat, shade=0, mats=None):
        m = (mats or self.lo.mat).get(mat, mat)
        if isinstance(m, str):
            return m
        return m[min(shade, len(m) - 1)]

    def inks(self):
        """Sorted ink list; canvas index i+1 -> inks[i]. Fixed alphabet so frames can be compared."""
        from ..delve.source import palette
        if self._palette_inks is None:
            self._palette_inks = sorted(palette())
        return self._palette_inks

    def idx(self, ch):
        return self.inks().index(ch) + 1

    def mi(self, mat, mats=None):
        return self.idx(self.ink(mat, 0, mats)), self.idx(self.ink(mat, 1, mats))

    def reset(self):
        self.springs = {}
        self.prev = None
        self._sprite_cache = {}

    # ------------------------------------------------------------- sprites
    def sprite(self, item):
        key = item["id"]
        if key not in self._sprite_cache:
            mats = item.get("mat")
            rows = item["rows"]
            a = np.zeros((len(rows), len(rows[0])), np.uint8)
            for y, r in enumerate(rows):
                for x, c in enumerate(r):
                    if c == ".":
                        continue
                    if c in SPRITE_CODE:
                        m, s = SPRITE_CODE[c]
                        a[y, x] = self.idx(self.ink(m, s, mats))
                    else:
                        a[y, x] = self.idx(c)
            self._sprite_cache[key] = a
        return self._sprite_cache[key]

    # ------------------------------------------------------------- frame
    def frame(self, local, hips, yaw=0.0, pitch=20.0, facing=0.0, dt=1 / 12, smear=False,
              squash=1.0, size=None, pad=None):
        """local [B,3,3] bone rotations, hips [3] delta (m). Returns uint8 canvas (index = inks()[i-1])."""
        R, X = S.fk(local, np.asarray(hips, float) * self.leg_k, self.offset)
        M = Camera(yaw, pitch).M @ S.ry(np.radians(facing))
        lo, ppm = self.lo, self.ppm
        pad = int(round(self.h * 0.55)) if pad is None else pad
        W = size[0] if size else int(self.h + 2 * pad)
        H = size[1] if size else int(self.h + 2 * pad)
        floor = H - pad * 0.55
        cx = W / 2
        # squash & stretch about the floor point under the hips
        sq = np.array([1 / np.sqrt(squash), 1.0]) if squash != 1 else None

        def P(x):   # character space -> (screen xy, depth)
            c = np.asarray(x) @ M.T
            s = np.stack([cx + c[..., 0] * ppm, floor - c[..., 1] * ppm], -1)
            if sq is not None:
                s = np.stack([cx + (s[..., 0] - cx) * sq[0], floor + (s[..., 1] - floor) * squash], -1)
            return s, c[..., 2]

        sx, depth = P(X)
        out = np.zeros((H, W), np.uint8)
        K = self.idx(self.ink("outline"))
        groups = []        # (depth, fn)
        r = self.r
        parts = lo.body["parts"]
        limb_over = {}
        for it in lo.by_kind("limb"):
            for seg in it["segments"]:
                limb_over[seg] = it
        chest = (lo.by_kind("chest") or [None])[0]
        tabard = (lo.by_kind("tabard") or [None])[0]
        robe = (lo.by_kind("robe") or [None])[0] or ({"material": parts["top"], "trim": "gold"} if lo.body.get("robe") else None)

        def seg_spec(seg, default_mat):
            it = limb_over.get(seg)
            if it:
                return self.mi(it["material"], it.get("mat")), it.get("grow_m", 0) * ppm
            return self.mi(default_mat), 0.0

        # ---- legs
        for side in ("Left", "Right"):
            def leg(o, side=side, outline_only=False):
                segs = [(f"{side}UpLeg", f"{side}Leg", r["thigh"], "thigh", parts["legs"]),
                        (f"{side}Leg", f"{side}Foot", r["shin"], "shin", parts["legs"]),
                        (f"{side}Foot", f"{side}ToeBase", r["foot"], "foot", parts["feet"])]
                specs = [(sx[I[a]], sx[I[b]], rad, *seg_spec(sg, m)) for a, b, rad, sg, m in segs]
                for a, b, rad, (f, s), g in specs:
                    Dr.capsule(o, a, b, rad + g + 1, K)
                for a, b, rad, (f, s), g in specs:
                    Dr.capsule(o, a, b, rad + g, f, s)
            groups.append((depth[[I[f"{side}UpLeg"], I[f"{side}Leg"], I[f"{side}Foot"]]].mean(), leg))

        # ---- torso (+ neck, robe, chest armour)
        sh = [X[I["LeftArm"]] * 0.62 + X[I["Spine2"]] * 0.38, X[I["RightArm"]] * 0.62 + X[I["Spine2"]] * 0.38]
        hp = [X[I["LeftUpLeg"]] * 0.75 + X[I["Hips"]] * 0.25, X[I["RightUpLeg"]] * 0.75 + X[I["Hips"]] * 0.25]
        s2, s1 = I["Spine2"], I["Spine1"]
        chestf = [X[s2] + R[s2] @ np.array([0, 0, 0.06]), X[s2] + R[s2] @ np.array([0, 0, -0.06]),
                  X[s1] + R[s1] @ np.array([0, -0.05, 0.05]), X[s1] + R[s1] @ np.array([0, -0.05, -0.06])]
        tor3 = np.array(sh + hp + [X[I["Spine1"]]] + chestf)
        tor, tz = P(tor3)
        torso_mat = (chest["material"], chest.get("mat")) if chest else (parts["top"], None)

        def torso(o):
            f, s = self.mi(*torso_mat)
            nf, ns = self.mi("skin")
            Dr.capsule(o, sx[I["Neck"]], sx[I["Spine2"]], r["neck"] + 1, K)
            Dr.capsule(o, sx[I["Neck"]], sx[I["Spine2"]], r["neck"], nf, ns)
            if robe:
                rb = np.array([X[I["LeftUpLeg"]], X[I["RightUpLeg"]],
                               X[I["LeftLeg"]] * 0.35 + X[I["LeftFoot"]] * 0.65,
                               X[I["RightLeg"]] * 0.35 + X[I["RightFoot"]] * 0.65])
                rb[2:, 1] = np.minimum(rb[2:, 1], 0.12)      # hem near the floor whatever the knees do
                rp, _ = P(rb)
                hem = rp[2:].copy()
                spread = (hem[0] - hem[1])
                hem[0] += spread * 0.45; hem[1] -= spread * 0.45
                pts = np.vstack([rp[:2], hem])
                rf, rs = self.mi(robe["material"], robe.get("mat"))
                Dr.poly(o, pts, K, r["thigh"] * 1.3 + 1)
                Dr.poly(o, pts, rf, r["thigh"] * 1.3)
                # hem trim + centre fold shade
                tf, _ = self.mi(robe.get("trim", "gold"))
                Dr.capsule(o, hem[0], hem[1], 0.8, tf)
                mid_top, mid_bot = rp[:2].mean(0), hem.mean(0)
                Dr.capsule(o, mid_top + (mid_bot - mid_top) * 0.3, mid_bot, 0.5, rs)
            Dr.poly(o, tor, K, r["torso"] + 1)
            Dr.poly(o, tor, f, r["torso"])
            # side shade: far half of the torso (away from light)
            c = tor.mean(0)
            Dr.capsule(o, tor[1] + (c - tor[1]) * 0.1 + [r["torso"] * 0.6, 0], tor[3] + [r["torso"] * 0.6, 0], max(r["torso"] * 0.35, 0.6), s)
            if tabard:
                # surcoat in the seat colour: shoulders -> mid-thigh, hem trim, centre fold
                th = np.array([X[I["LeftUpLeg"]] * 0.45 + X[I["LeftLeg"]] * 0.55,
                               X[I["RightUpLeg"]] * 0.45 + X[I["RightLeg"]] * 0.55])
                th[:, 1] = np.maximum(th[:, 1], X[I["Hips"], 1] - 0.24)
                tp, _ = P(th)
                top = tor[:2] * 0.8 + tor[2:4] * 0.2
                tf_, ts_ = self.mi(tabard["material"], tabard.get("mat"))
                pts = np.vstack([top, tor[2:4], tp])
                Dr.poly(o, pts, K, r["torso"] * 0.55 + 1)
                Dr.poly(o, pts, tf_, r["torso"] * 0.55)
                mid_t, mid_b = top.mean(0), tp.mean(0)
                Dr.capsule(o, mid_t + (mid_b - mid_t) * 0.35, mid_b, 0.5, ts_)
                if tabard.get("trim"):
                    gf, _ = self.mi(tabard["trim"])
                    Dr.capsule(o, tp[0], tp[1], 0.7, gf)
            # belt / chest trim
            trim = chest.get("trim") if chest else "leather"
            if trim:
                bf, bs = self.mi(trim)
                Dr.capsule(o, tor[2], tor[3], max(r["torso"] * 0.28, 0.7), bf)
        tdepth = tz[:4].mean()
        if robe or tabard:
            tdepth = max(tdepth, depth[[I["LeftLeg"], I["RightLeg"]]].max() + 0.01)
        groups.append((tdepth, torso))

        # ---- arms (+ pauldrons, gauntlets, hand-held items)
        paul = (lo.by_kind("pauldron") or [None])[0]
        for side in ("Left", "Right"):
            def arm(o, side=side):
                segs = [(f"{side}Arm", f"{side}ForeArm", r["uparm"], "uparm", parts["sleeve"]),
                        (f"{side}ForeArm", f"{side}Hand", r["fore"], "fore", "skin")]
                specs = [(sx[I[a]], sx[I[b]], rad, *seg_spec(sg, m)) for a, b, rad, sg, m in segs]
                hand = sx[I[f"{side}Hand"]] * 0.55 + sx[I[f"{side}HandIndex1"]] * 0.45
                (hf, hs), hg = seg_spec("hand", "skin")
                for a, b, rad, (f, s), g in specs:
                    Dr.capsule(o, a, b, rad + g + 1, K)
                Dr.disc(o, hand, r["hand"] + hg + 1, K)
                for a, b, rad, (f, s), g in specs:
                    Dr.capsule(o, a, b, rad + g, f, s)
                Dr.disc(o, hand, r["hand"] + hg, hf, hs)
                if paul:
                    c = X[I[f"{side}Arm"]] + R[I[f"{side}Arm"]] @ np.array([0, -0.03, 0]) + np.array([0, 0.03, 0])
                    pc, _ = P(c)
                    pr = paul.get("r_m", 0.08) * ppm
                    pf, ps = self.mi(paul["material"], paul.get("mat"))
                    Dr.disc(o, pc, pr + 1, K)
                    Dr.disc(o, pc, pr, pf, ps)
                    if paul.get("trim"):
                        tf, _ = self.mi(paul["trim"])
                        Dr.capsule(o, pc + [-pr * 0.7, pr * 0.45], pc + [pr * 0.7, pr * 0.45], 0.5, tf)
            groups.append((depth[[I[f"{side}Arm"], I[f"{side}ForeArm"], I[f"{side}Hand"]]].mean(), arm))

        # ---- head
        hb = I["Head"]
        hc3 = X[hb] + R[hb] @ np.array([0, self.lift, 0.015])
        hc, hz = P(hc3)
        self.last_head = hc
        helm = (lo.by_kind("helm") or [None])[0]
        hats = lo.by_kind("headsprite")
        hide_hair = any(h.get("hides_hair") for h in hats)

        def head(o):
            hr = self.hr
            RM = M @ R[hb]                 # head axes in camera space
            fwd, up, lft = RM[:, 2], RM[:, 1], RM[:, 0]
            mask = np.zeros_like(o, bool)
            Dr.disc(o, hc, hr + 1, K)
            sf, ss = self.mi("skin")
            Dr.disc(o, hc, hr, sf, ss, mask)
            y0, y1 = int(max(hc[1] - hr - 2, 0)), int(min(hc[1] + hr + 2, o.shape[0]))
            x0, x1 = int(max(hc[0] - hr - 2, 0)), int(min(hc[0] + hr + 2, o.shape[1]))
            gy, gx = np.mgrid[y0:y1, x0:x1]
            u = (gx + 0.5 - hc[0]) / hr
            v = -(gy + 0.5 - hc[1]) / hr
            inside = mask[y0:y1, x0:x1]
            w = np.sqrt(np.clip(1 - u * u - v * v, 0, 1))
            n3 = np.stack([u, v, w], -1)      # sphere normal (camera space)
            nf = n3 @ fwd                      # facing component
            nu = n3 @ up
            reg = o[y0:y1, x0:x1]
            if helm and helm.get("shape") == "bucket":
                # delve great-helm: flat-topped barrel, dark visor slit + breaths on the face side
                mf, ms = self.mi(helm["material"], helm.get("mat"))
                ux, uy = up[0], -up[1]
                L = np.hypot(ux, uy) or 1.0
                ux, uy = ux / L, uy / L                  # screen "up" of the head
                rx_, ry_ = -uy, ux
                hw, hh = hr * 0.9, hr * 0.95
                corners = np.array([hc + sa * hw * np.array([rx_, ry_]) + sb * hh * np.array([ux, uy])
                                    for sa, sb in ((-1, -1), (1, -1), (1, 1), (-1, 1))])
                bm = np.zeros_like(o, bool)
                Dr.poly(o, corners, K, 1.3)
                Dr.poly(o, corners, mf, 0.4, 0, bm)
                b = bm[y0:y1, x0:x1]
                du = (gx + 0.5 - hc[0]) * rx_ + (gy + 0.5 - hc[1]) * ry_      # across the helm
                dv = -((gx + 0.5 - hc[0]) * ux + (gy + 0.5 - hc[1]) * uy)     # up the helm
                fx_ = fwd[0] * rx_ - fwd[1] * ry_     # face direction across the helm
                # shade band on the side away from the light / the back of the helm
                reg[b & (du * np.sign(Dr.LIGHT[0]) > hw * 0.45)] = ms
                reg[b & (dv > hh * 0.62)] = ms if helm.get("dome", True) else mf
                if helm.get("trim"):
                    tf, ts = self.mi(helm["trim"])
                    reg[b & (np.abs(dv - hh * 0.55) < 0.7)] = tf          # brow band
                if fwd[2] > -0.35:                                          # face toward camera: visor
                    side = np.sign(fx_) if abs(fx_) > 0.25 else 0.0
                    lo_u = -hw * (0.75 if side <= 0 else 0.05)
                    hi_u = hw * (0.75 if side >= 0 else 0.05)
                    if side == 0:
                        lo_u, hi_u = -hw * 0.7, hw * 0.7
                    slit = b & (np.abs(dv - hh * 0.12) < 0.75) & (du > lo_u) & (du < hi_u)
                    reg[slit] = K
                    cxu = hw * 0.55 * side
                    reg[b & (np.abs(du - cxu) < 0.6) & (dv < -hh * 0.15) & (dv > -hh * 0.7)] = ms
            elif helm:
                mf, ms = self.mi(helm["material"], helm.get("mat"))
                cover = inside & ((nu > -0.35) | (nf < 0.55))
                if helm.get("visor") == "open":
                    cover &= ~((nf > 0.55) & (nu < 0.25) & (nu > -0.6))
                shade = (u * Dr.LIGHT[0] - v * Dr.LIGHT[1]) > 0.35
                reg[cover & ~shade] = mf
                reg[cover & shade] = ms
                if helm.get("visor") == "slit":
                    reg[inside & (nf > 0.45) & (np.abs(nu - 0.02) < 0.13)] = K
                if helm.get("trim"):
                    tf, _ = self.mi(helm["trim"])
                    reg[inside & (np.abs(nu + 0.38) < 0.11) & (nf > -0.2)] = tf
            else:
                if not hide_hair:
                    hf, hs = self.mi("hair")
                    hair = inside & ((nu > 0.25) | (nf < -0.25) & (nu > -0.55))
                    reg[hair] = hf
                    reg[inside & (nu > 0.25) & (nf < -0.3) & ((u * Dr.LIGHT[0] - v * Dr.LIGHT[1]) > 0.4)] = hs
                for s3 in (+1, -1):     # eyes: drawn where they face the camera
                    e3 = RM @ np.array([0.38 * s3, 0.02, 0.92])
                    if e3[2] > 0.25:
                        ex, ey = hc[0] + e3[0] * hr, hc[1] - e3[1] * hr
                        xi, yi = int(ex), int(ey)
                        if 0 <= yi < o.shape[0] and 0 <= xi < o.shape[1]:
                            o[yi, xi] = K
                if lo.body.get("beard"):
                    bf, bs = self.mi(lo.body["beard"])
                    reg[inside & (nf > 0.2) & (nu < -0.35)] = bf
            if lo.body.get("ears"):
                for s3 in (+1, -1):
                    e = RM @ np.array([s3 * 1.0, 0.15, -0.1])
                    tip = hc + np.array([e[0], -e[1]]) * hr * 1.75
                    base = hc + np.array([e[0], -e[1]]) * hr * 0.8
                    if e[2] < 0.6:
                        ef, es = self.mi("skin")
                        Dr.capsule(o, base, tip, 1.6, K); Dr.capsule(o, base, tip, 0.7, ef)
        groups.append((hz + 0.05, head))

        # ---- hats / horns: sprites on the head-top socket
        for it in hats:
            top3 = X[hb] + R[hb] @ np.array([0, self.lift + lo.body["head_m"] * 0.75, 0.0])
            groups.append((hz + 0.08, self._socket_sprite(it, top3, R[hb] @ [0, 1, 0], R[hb] @ [1, 0, 0], P, M, it["size_m"], align="pivot")))

        # ---- plume (spring)
        for it in lo.by_kind("plume"):
            sp = self.springs.setdefault(it["id"], Spring(4, it.get("length_m", 0.28) / 4, grav=3.0, damp=0.8, stiff=it.get("stiff", 0.3)))
            anchor = X[hb] + R[hb] @ np.array([0, self.lift + lo.body["head_m"] * 0.85, -0.03])
            rest = R[hb] @ np.array(it.get("rest", [0, 0.8, -0.6]))
            rest = rest / np.linalg.norm(rest)
            pts = sp.step(anchor, rest, dt)
            pp, pz = P(pts)

            def plume(o, pp=pp, it=it):
                f, s = self.mi(it["material"], it.get("mat"))
                rad = it.get("radii_px", [3.0, 2.6, 2.0, 1.4, 0.9])
                for i in range(len(pp) - 1):
                    Dr.capsule(o, pp[i], pp[i + 1], rad[i] * self.h / 48 + 1, K)
                for i in range(len(pp) - 1):
                    Dr.capsule(o, pp[i], pp[i + 1], rad[i] * self.h / 48, f, s)
            groups.append((pz.mean() + (0.06 if pz.mean() > hz else -0.02), plume))

        # ---- cape (spring chain, collides with the back)
        for it in lo.by_kind("cape"):
            n = 5
            sp = self.springs.setdefault(it["id"], Spring(n, it.get("length_m", 0.85) / n, grav=9.8, damp=0.88))
            s2 = I["Spine2"]
            anchor = X[s2] + R[s2] @ np.array([0, 0.07, -0.10])
            back = R[s2] @ np.array([0, 0, -1.0])
            rest = R[I["Hips"]] @ np.array([0, -1, -0.15])
            rest /= np.linalg.norm(rest)

            def push(p, Rs=R[s2], Xs=X[s2], Rh=R[I["Hips"]], Xh=X[I["Hips"]]):
                for i in range(1, len(p)):     # keep behind the torso / butt planes
                    for Rb, Xb, lim in ((Rs, Xs, -0.11), (Rh, Xh, -0.12)):
                        loc = Rb.T @ (p[i] - Xb)
                        if loc[2] > lim and abs(loc[1]) < 0.45:
                            loc[2] = lim
                            p[i] = Xb + Rb @ loc
            pts = sp.step(anchor, rest, dt, push)
            side = R[s2] @ np.array([1.0, 0, 0])
            wid = np.linspace(it.get("width_m", 0.36) * 0.5, it.get("width_m", 0.36) * 0.75, n + 1)
            L3 = pts + side * wid[:, None]
            R3 = pts - side * wid[:, None]
            Lp, lz = P(L3)
            Rp, _ = P(R3)

            def cape(o, Lp=Lp, Rp=Rp, it=it):
                f, s = self.mi(it["material"], it.get("mat"))
                quads = [np.array([Lp[i], Lp[i + 1], Rp[i + 1], Rp[i]]) for i in range(n)]
                for q in quads:
                    Dr.poly(o, q, K, 1.0)
                for i, q in enumerate(quads):
                    Dr.poly(o, q, f if i % 2 == 0 or True else s, 0.0)
                # fold lines (shade) down the middle
                mids = (Lp + Rp) / 2
                for i in range(n):
                    Dr.capsule(o, (Lp[i] + mids[i]) / 2, (Lp[i + 1] + mids[i + 1]) / 2, 0.45, s)
            cz = lz.mean()
            groups.append((cz, cape))

        # ---- weapons & shields
        trails = []
        for it in lo.items:
            if it["kind"] == "weapon":
                hand = it.get("hand", "Right")
                hb_ = I[f"{hand}Hand"]
                grip = X[hb_] * 0.55 + X[I[f"{hand}HandIndex1"]] * 0.45
                axis = R[hb_] @ np.array([0, 0, 1.0])
                across = R[hb_] @ np.array([0, -1.0, 0])
                fn = self._socket_sprite(it, grip, axis, across, P, M, it["length_m"] * self.wk, other=X[I["RightHand" if hand == "Left" else "LeftHand"]])
                tip = grip + axis * it["length_m"] * self.wk
                gp, gz = P(grip)
                tp, tz_ = P(tip)
                trails.append((it["id"], gp, tp))
                groups.append(((gz + tz_) / 2 + 0.015, fn))
            elif it["kind"] == "shield":
                hand = it.get("hand", "Left")
                fa = I[f"{hand}ForeArm"]
                sgn = 1 if hand == "Left" else -1
                c3 = X[fa] * 0.4 + X[I[f"{hand}Hand"]] * 0.6 + R[fa] @ np.array([sgn * 0.07, 0, 0])
                nrm = R[fa] @ np.array([sgn * 1.0, 0, 0])
                fn = self._shield(it, c3, nrm, P, M)
                cp, cz = P(c3)
                groups.append((cz + 0.03, fn))

        # ---- smear trails (behind everything)
        if smear and self.prev is not None:
            tf, ts = self.mi("trail")
            for wid, gp, tp in trails:
                if wid in self.prev:
                    pg, pt = self.prev[wid]
                    quad = np.array([pg * 0.6 + pt * 0.4, pt, tp, gp * 0.6 + tp * 0.4])
                    Dr.poly(out, quad, ts, 0.0)
                    Dr.poly(out, np.array([pt * 0.8 + pg * 0.2, pt, tp, tp * 0.8 + gp * 0.2]), tf, 0.0)
        self.prev = {w: (g, t) for w, g, t in trails}

        for _, fn in sorted(groups, key=lambda g: g[0]):
            fn(out)
        Dr.cleanup(out)
        return out

    # ------------------------------------------------------------- sockets
    def _socket_sprite(self, it, pos3, axis3, across3, P, M, length_m, other=None, align="grip"):
        spr = self.sprite(it)
        piv = np.array(it["pivot"], float)
        p0, _ = P(pos3)
        p1, _ = P(pos3 + np.asarray(axis3) * length_m)
        pa, _ = P(pos3 + np.asarray(across3) * 0.1)
        d = p1 - p0
        L = np.hypot(*d)
        full = length_m * self.ppm
        sprite_len = piv[1] if align == "grip" else spr.shape[0]
        ref = piv[1] if align == "grip" else spr.shape[1]       # weapons: grip->tip; hats: width
        sx = full / max(ref, 1)
        if align == "grip":
            sy = max(L, 1e-3) / max(sprite_len, 1)
        else:   # hats: keep their shape, squash a little when the head tips toward the camera
            sy = sx * float(np.clip(L / max(full, 1e-3), 0.45, 1.0))
        # angle so the sprite's "up" (-y) lies along d: rotate (0,-1) to d
        ang = np.arctan2(d[0], -d[1]) if L > 0.5 else 0.0
        # flip so the sprite's +x points along the projected `across` axis
        upv = d / (L or 1)
        a = pa - p0
        flip = (upv[0] * a[1] - upv[1] * a[0]) > 0
        img = spr[:, ::-1] if flip else spr
        pv = piv.copy()
        if flip:
            pv[0] = spr.shape[1] - piv[0]
        if align == "grip" and L < 0.15 * full:   # pointing at the camera: draw a short stub instead of nothing
            sy = 0.2 * sx
        string = it.get("string")
        K = self.idx(self.ink("outline"))

        def fn(o):
            if string:   # bow string: tip-to-tip, or pulled to the drawing hand when it is near
                s = self.idx(string)
                t1 = p0 + d * (pv[1] / max(sprite_len, 1))
                t2 = p0 - d * ((spr.shape[0] - pv[1]) / max(sprite_len, 1))
                mid = None
                if other is not None:
                    oh, _ = P(other)
                    if np.linalg.norm(other - pos3) < 0.8:
                        mid = oh
                if mid is None:
                    Dr.capsule(o, t1, t2, 0.5, s)
                else:
                    Dr.capsule(o, t1, mid, 0.5, s); Dr.capsule(o, mid, t2, 0.5, s)
                    nock = p0 + (p0 - mid) * 0.25
                    Dr.capsule(o, mid, nock, 0.6, self.idx("R"))
                    Dr.capsule(o, nock, nock + (nock - mid) * 0.08, 0.9, self.idx("V"))
            res = Dr.stamp(o, img, pv, ang, p0, sx, sy)
            if res is not None:   # 1-px outline around the stamped sprite
                y0, y1, x0, x1, m = res
                n = m.copy()
                n[1:] |= m[:-1]; n[:-1] |= m[1:]; n[:, 1:] |= m[:, :-1]; n[:, :-1] |= m[:, 1:]
                o[y0:y1, x0:x1][n & ~m] = K
        return fn

    def _shield(self, it, c3, n3, P, M):
        spr = self.sprite(it)
        cp, _ = P(c3)
        nc = M @ n3
        size = it.get("size_m", 0.42) * self.wk * 1.15 * self.ppm
        s = size / spr.shape[0]
        face = abs(nc[2])
        sx = s * max(face, 0.28)
        # tilt with the projected shield "up" (world up kept)
        K = self.idx(self.ink("outline"))

        def fn(o):
            if nc[2] < -0.2:   # seen from the back: plain wood disc
                wf, ws = self.mi("wood")
                back = np.where(spr > 0, self.idx(self.ink("wood", 0)), 0).astype(np.uint8)
                back[(spr == K)] = K
                Dr.stamp(o, back, np.array(it["pivot"], float), 0.0, cp, sx, s)
            else:
                Dr.stamp(o, spr, np.array(it["pivot"], float), 0.0, cp, sx, s)
        return fn
