"""2D fast track: camera-facing sprites driven by the shared Mixamo skeleton.

A Billboard holds one or more authored *views* (delve has TOWARD / ACROSS / AWAY
for the hero; folk are drawn ACROSS only). Each view is a stack of pixel-art parts
pinned to Mixamo bones at a pivot pixel. Per frame:
  1. FK the clip pose in 3D (skeleton.fk), turn it to the character's facing.
  2. Camera (yaw, pitch, any angle) -> the character's heading relative to the
     camera picks the view (nearest authored angle; mirrored art fills the rest).
  3. Project pose and rest joints to the screen; each part moves by its joint's
     projected delta and turns by its bone's projected angle delta, snapped in
     pixel mode (45° arms / 90° body) so the art stays crisp and on-palette.
  4. Parts are drawn back-to-front by camera depth; legs pick the authored stride
     frame from the feet's fore/aft split; nothing sinks below the floor."""
import numpy as np
from . import skeleton as S
from ..delve import source
from ..delve.cast import grid, recolor

DEG = np.pi / 180


class Camera:
    """Orthographic; yaw about +Y (0 = looking at the character's front when it
    faces +Z), pitch looks down (deg). Camera space: x right, y up, +z toward viewer."""

    def __init__(self, yaw=0.0, pitch=0.0):
        self.yaw, self.pitch = yaw, pitch
        self.M = S.rx(pitch * DEG) @ S.ry(-yaw * DEG)


class Part:
    def __init__(self, key, bone, end, ink, pivot, snap=90, amp=1.0, limit=180):
        self.key, self.bone, self.end = key, S.INDEX[bone], S.INDEX[end]
        self.amp, self.limit = amp, limit * DEG
        self.ink, self.pivot, self.snap = ink, np.array(pivot, float), snap


class View:
    def __init__(self, name, heading, w, h, parts, legs, hip, alts=None):
        self.name, self.heading, self.w, self.h = name, heading, w, h
        self.parts, self.legs, self.hip = parts, legs, np.array(hip, float)
        self.alts = alts or {}


class Billboard:
    def __init__(self, name, views, inks, height_px, pad=6):
        self.name, self.views, self.inks, self.pad = name, views, inks, pad
        self.ppm = height_px / S.height()
        self._rest = {}

    # ------------------------------------------------------------ view choice
    def pick(self, facing, cam):
        """Heading of the character as seen by this camera: 0 = toward camera,
        +90 = walking screen-right, 180 = away. Returns (view, mirrored)."""
        f = cam.M @ S.ry(facing * DEG) @ np.array([0, 0, 1.0])
        hd = np.degrees(np.arctan2(f[0], f[2]))
        best = None
        for v in self.views:
            for m in (False, True):
                a = -v.heading if m else v.heading
                d = abs((hd - a + 180) % 360 - 180)
                if best is None or d < best[0] - 1e-6:
                    best = (d, v, m)
        return best[1], best[2]

    # ------------------------------------------------------------ projection
    def _proj(self, local, hips, facing, cam, mirror):
        R, X = S.fk(local, hips)
        P = (cam.M @ S.ry(facing * DEG) @ X.T).T
        if mirror:
            P[:, 0] *= -1
        return P  # x right, y up, z toward viewer

    def frame(self, local, hips, facing=90.0, cam=None, mode="pixel"):
        cam = cam or Camera()
        view, mirror = self.pick(facing, cam)
        key = (view.name, facing, cam.yaw, cam.pitch, mirror)
        if key not in self._rest:
            self._rest[key] = self._proj(np.tile(np.eye(3), (len(S.NAMES), 1, 1)), (0, 0, 0), facing, cam, mirror)
        P0 = self._rest[key]
        P = self._proj(local, hips, facing, cam, mirror)
        pad = self.pad
        H, W = view.h + 2 * pad, view.w + 2 * pad
        out = np.zeros((H, W), np.uint8)
        # legs: authored stride frame from the feet's fore/aft split (character space)
        _, Xc = S.fk(local, hips)
        dz = Xc[S.INDEX["LeftFoot"], 2] - Xc[S.INDEX["RightFoot"], 2]
        li = 0 if abs(dz) < 0.08 or len(view.legs) < 2 else (1 if dz > 0 else 2) if len(view.legs) > 2 else 1
        hipd = self._delta(P, P0, S.INDEX["Hips"])
        items = [(-1e9, 0, view.legs[li], np.zeros(2), 0.0, hipd)]
        for k, pt in enumerate(view.parts):
            d = (P[pt.bone] + P[pt.end]) / 2
            ang = float(np.clip(self._turn(P, P0, pt) * pt.amp, -pt.limit, pt.limit))
            if mode == "pixel":
                ang = _snap(ang, pt.snap)
            ink = view.alts.get((pt.key, int(round(np.degrees(ang)))))
            if ink is not None:
                ang = 0.0
            items.append((d[2] if pt.key.startswith("arm") else -1e6 + k, k + 1,
                          ink if ink is not None else pt.ink, pt.pivot, ang, self._delta(P, P0, pt.bone)))
        items.sort(key=lambda t: (t[0], t[1]))
        for _, _, ink, piv, ang, mv in items:
            _stamp(out, ink, piv, ang, mv + pad, mode)
        ys = np.nonzero(out.any(1))[0]
        floor = pad + view.h - 1
        if len(ys) and ys[-1] > floor:
            out = np.roll(out, floor - ys[-1], 0)
        if mode == "pixel":
            from ..doll.draw import cleanup
            cleanup(out)       # seams between stamped parts, stray rotation pixels
        return out[:, ::-1] if mirror else out, view.name, mirror

    def _delta(self, P, P0, b):
        d = (P[b, :2] - P0[b, :2]) * self.ppm
        return np.array([d[0], -d[1]])  # screen px, y down

    def _turn(self, P, P0, pt):
        v = P[pt.end, :2] - P[pt.bone, :2]
        v0 = P0[pt.end, :2] - P0[pt.bone, :2]
        L = np.linalg.norm(P0[pt.end] - P0[pt.bone])
        if np.linalg.norm(v) < 0.3 * L or np.linalg.norm(v0) < 0.3 * L:
            return 0.0  # pointing at the camera: no reliable screen angle
        a = np.arctan2(-v[1], v[0]) - np.arctan2(-v0[1], v0[0])
        return (a + np.pi) % (2 * np.pi) - np.pi

    def rgba(self, idx, pal):
        lut = np.zeros((len(self.inks) + 1, 4), np.uint8)
        for i, c in enumerate(self.inks):
            lut[i + 1, :3] = pal[c][:3]
            lut[i + 1, 3] = 255
        return lut[idx]


def _snap(a, step_deg):
    s = step_deg * DEG
    q = np.round(a / s) * s
    if abs(a) < 0.6 * s:
        return 0.0
    return q


def _stamp(out, ink, pivot, ang, move, mode, ss=2):
    """Paste ink [h,w] (0 = clear) turned by ang about pivot (canvas px), moved by
    move px. Inverse map: 90° multiples are exact; others sample ss x ss sub-pixels,
    draw where ≥ 1/4 covered, colour from the nearest covered sample (stays on palette)."""
    h, w = ink.shape
    mv = np.round(move) if mode == "pixel" else move
    c, s = np.cos(ang), np.sin(ang)
    pc = pivot + mv  # pivot on canvas after the move
    # bounding box of the turned part
    corners = np.array([[0, 0], [w, 0], [0, h], [w, h]], float) - (pivot - 0)
    rc = np.stack([corners[:, 0] * c - corners[:, 1] * s, corners[:, 0] * s + corners[:, 1] * c], 1) + pc
    x0, y0 = np.floor(rc.min(0)).astype(int) - 1
    x1, y1 = np.ceil(rc.max(0)).astype(int) + 1
    x0, y0 = max(x0, 0), max(y0, 0)
    x1, y1 = min(x1, out.shape[1]), min(y1, out.shape[0])
    if x1 <= x0 or y1 <= y0:
        return
    o = (np.arange(ss) + 0.5) / ss
    gy, gx = np.mgrid[y0:y1, x0:x1]
    sx = gx[..., None, None] + o[None, None, None, :] - pc[0]
    sy = gy[..., None, None] + o[None, None, :, None] - pc[1]
    u = c * sx + s * sy + pivot[0]   # rotate back by -ang
    v = -s * sx + c * sy + pivot[1]
    ui, vi = np.floor(u).astype(int), np.floor(v).astype(int)
    ok = (ui >= 0) & (ui < w) & (vi >= 0) & (vi < h)
    smp = np.where(ok, ink[np.clip(vi, 0, h - 1), np.clip(ui, 0, w - 1)], 0).reshape(*gx.shape, ss * ss)
    cov = (smp > 0).mean(-1)
    # colour: the most central covered sample
    order = np.argsort(np.abs(np.arange(ss * ss) - (ss * ss - 1) / 2))
    pick = smp[..., order]
    first = np.argmax(pick > 0, -1)
    col = np.take_along_axis(pick, first[..., None], -1)[..., 0]
    m = cov >= 0.25
    reg = out[y0:y1, x0:x1]
    reg[m] = col[m]


# ---------------------------------------------------------------- delve cast
def _ix(g, inks):
    lut = {c: i + 1 for i, c in enumerate(inks)}
    lut["."] = 0
    return np.vectorize(lambda ch: lut.get(ch, 0))(g).astype(np.uint8)


ARM = ("Arm", "Hand")


def hero(seat="1", root=source.DEFAULT_ROOT, held="sword"):
    """The delve hero with all three of its authored views (kit/hero.rs)."""
    pal = source.palette()
    inks = sorted(pal)
    swaps = source.seat_colours(root).get(seat) if seat in "123" else \
        source.seat_colours(root, "realm_colours")["_"]
    W = H = 16
    mk = lambda rows, at: _ix(recolor(grid(rows, W, H, at), swaps), inks)
    views = []
    # view, heading, which Mixamo side holds the shield
    for vname, heading, shield_side, sh_px, held_px in (
            ("TOWARD", 0, "Right", (4.0, 9.5), (12.0, 9.5)),
            ("ACROSS", 90, "Left", (5.5, 9.5), (10.5, 9.5)),
            ("AWAY", 180, "Left", (4.0, 9.5), (12.0, 9.5))):
        p = source.hero_parts(root, vname)
        other = "Left" if shield_side == "Right" else "Right"
        parts = [Part("torso", "Spine", "Neck", mk(p["harness"], (0, 8)), (8, 14)),
                 Part("head", "Head", "HeadTop_End", mk(p["helm"], (0, 0)), (8.5, 8)),
                 Part("arm_b", shield_side + "Arm", shield_side + "Hand", mk(p["shield"]["rows"], tuple(p["shield"]["at"])), sh_px, 45, 0.6, 45),
                 Part("arm_f", other + "Arm", other + "Hand", mk(p[held][0]["rows"], tuple(p[held][0]["at"])), held_px, 45, 0.8, 90)]
        legs = [mk(s, (0, 14)) for s in p["steel"]]
        alts = {}
        if vname == "ACROSS":
            st = p[held][1]
            alts[("arm_f", -90)] = mk(st["rows"], tuple(st["at"]))
        views.append(View(vname, heading, W, H, parts, legs, (8, 14), alts))
    names = {"1": "red", "2": "blue", "3": "green"}
    return Billboard(f"hero_{names.get(seat, 'gold')}", views, inks, H)


def from_character(ch):
    """Any sprite-rig delve Character (one side view) as a Billboard: its ACROSS art
    serves every heading, mirrored for the far side."""
    pal = source.palette()
    inks = sorted(pal)
    bone = {"torso": ("Spine", "Neck", "hip", 90), "head": ("Head", "HeadTop_End", "neck", 90),
            "arm_b": ("LeftArm", "LeftHand", "shoulder_b", 45), "arm_f": ("RightArm", "RightHand", "shoulder_f", 45)}
    amp = {"torso": (ch.amp.get("torso", 1), 90), "head": (ch.amp.get("head", 1), 90),
           "arm_b": (0.6, 45), "arm_f": (0.8, 90)}
    parts = [Part(k, bone[k][0], bone[k][1], _ix(g, inks), ch.pivots[bone[k][2]], bone[k][3], *amp[k])
             for k, g in ch.layers if k in bone]
    legs = [_ix(g, inks) for g in ch.legs]
    return Billboard(ch.name, [View("ACROSS", 90, ch.w, ch.h, parts, legs, ch.pivots["hip"])], inks, ch.h)
