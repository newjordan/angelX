"""Tiny text clip DSL: author a new emote/attack in minutes, on the shared Mixamo skeleton.

    clip dab                      # starts a clip (many per file)
    fps 12                        # default 12
    loop no                       # yes|no (default yes)
    tags emote joke               # free-form
    # frame  [ease=in|out|inout|linear|snap]  [hit] [smear] [rest]  bone=x,y,z ...
    0   rest
    4   ease=out  rarm=135,0,30 rfore=20 larm=60,0,80 head=35,0,0 spine=15
    10  hold                      # same pose as previous key
    14  =0                        # copy key 0

Joint angles are degrees, friendly and mirrored per side so both sides read the same:
  arms   larm/rarm   x = raise forward, y = twist, z = raise outward (sideways)
  elbows lfore/rfore x = bend (+ = flex)       knees lknee/rknee x = bend (+ = flex)
  legs   lleg/rleg   x = raise forward, z = spread outward
  feet   lfoot/rfoot x = toes down;  hands lhand/rhand x = flex fwd, y = twist, z = tilt out
  body   spine/chest/neck/head  x = bend forward, y = turn to own left, z = lean to own left
  root   root=x,y,z  whole-body rotation on Hips (y = spin)
  hips   hips=dx,dy,dz  metres (dy<0 crouch, >0 jump); 'bob=dy' shortcut
Bones not named keep their value from the previous key ("carry"); `rest` clears all first.
Keys are interpolated with the key's easing curve (applies to the segment arriving at it).
Markers: `hit` = contact frame, `smear` = draw weapon trails arriving at this key.
"""
import re
import numpy as np
from ..skel import skeleton as S
from ..skel.clips import Clip

D = np.pi / 180
ALIAS = {
    "larm": ("LeftArm", "limb", +1), "rarm": ("RightArm", "limb", -1),
    "lfore": ("LeftForeArm", "elbow", +1), "rfore": ("RightForeArm", "elbow", -1),
    "lhand": ("LeftHand", "hand", +1), "rhand": ("RightHand", "hand", -1),
    "lsh": ("LeftShoulder", "limb", +1), "rsh": ("RightShoulder", "limb", -1),
    "lleg": ("LeftUpLeg", "limb", +1), "rleg": ("RightUpLeg", "limb", -1),
    "lknee": ("LeftLeg", "knee", +1), "rknee": ("RightLeg", "knee", -1),
    "lfoot": ("LeftFoot", "foot", +1), "rfoot": ("RightFoot", "foot", -1),
    "spine": ("Spine", "body", 1), "chest": ("Spine2", "body", 1), "mid": ("Spine1", "body", 1),
    "neck": ("Neck", "body", 1), "head": ("Head", "body", 1), "root": ("Hips", "body", 1),
}
EASE = {
    "linear": lambda u: u,
    "in": lambda u: u * u * u,
    "out": lambda u: 1 - (1 - u) ** 3,
    "inout": lambda u: 3 * u * u - 2 * u * u * u,
    "snap": lambda u: 1.0 if u >= 1 else 0.0,
    "back": lambda u: 1 + 2.70158 * (u - 1) ** 3 + 1.70158 * (u - 1) ** 2,   # overshoot (follow-through)
}


def expmap(v):
    """Rotation vector (rad) -> matrix (Rodrigues)."""
    th = float(np.linalg.norm(v))
    if th < 1e-9:
        return np.eye(3)
    k = np.asarray(v, float) / th
    Kx = np.array([[0, -k[2], k[1]], [k[2], 0, -k[0]], [-k[1], k[0], 0]])
    return np.eye(3) + np.sin(th) * Kx + (1 - np.cos(th)) * Kx @ Kx


def bone_matrix(kind, side, x, y, z):
    """Friendly degrees -> local rotation. Swing (x, z) is one rotation about the combined
    axis, so x and z mix intuitively (x=90,z=90 = diagonal forward-and-out); y twists about
    the bone itself first (limbs) or turns (body)."""
    if kind in ("limb", "elbow", "hand"):
        return expmap(np.array([-x, 0, side * z]) * D) @ S.ry(side * y * D)
    if kind == "knee":
        return S.rx(x * D)
    if kind == "foot":
        return expmap(np.array([x, 0, side * z]) * D)
    return expmap(np.array([x, y, -z]) * D)   # body


class ClipSpec:
    def __init__(self, name):
        self.name, self.fps, self.loop, self.tags = name, 12, True, []
        self.keys = []   # (frame, ease, flags, {bone: (x,y,z)}, hips, mode)
        self.meta = {}

    def build(self):
        """-> Clip with .events {frame: [hit|smear]} and .meta."""
        keys = sorted(self.keys, key=lambda k: k[0])
        # resolve carry / copy into absolute per-key poses
        poses, hipss = [], []
        cur, hp = {}, np.zeros(3)
        byframe = {}
        for f, ease, flags, bones, hips, mode in keys:
            if mode and mode.startswith("="):
                c, h = byframe[int(mode[1:])]
                cur, hp = dict(c), h.copy()
            elif "rest" in flags:
                cur, hp = {}, np.zeros(3)
            for b, v in bones.items():
                cur[b] = v
            if hips is not None:
                hp = hips.copy() if hips.shape == (3,) else hp
            byframe[f] = (dict(cur), hp.copy())
            poses.append(dict(cur)); hipss.append(hp.copy())
        closed = keys[-1][5] == f"={keys[0][0]}" and not keys[-1][3]
        T = keys[-1][0] + (0 if (self.loop and closed) else 1)
        T = max(T, 1)
        names = sorted({b for p in poses for b in p})
        vals = np.zeros((len(keys), len(names), 3))
        for i, p in enumerate(poses):
            for j, b in enumerate(names):
                vals[i, j] = p.get(b, (0, 0, 0))
        hk = np.array(hipss)
        fr = np.array([k[0] for k in keys], float)
        rot = np.tile(np.eye(3), (T, len(S.NAMES), 1, 1))
        hips = np.zeros((T, 3))
        events = {}
        for i, k in enumerate(keys):
            for fl in k[2]:
                if fl in ("hit", "smear"):
                    events.setdefault(k[0], []).append(fl)
        smear = set()
        for i, k in enumerate(keys):
            if "smear" in k[2] and i > 0:
                smear.update(range(keys[i - 1][0] + 1, k[0] + 1))
        for t in range(T):
            if self.loop and t > fr[-1]:   # wrap segment last -> first
                i0, i1, u = len(keys) - 1, 0, (t - fr[-1]) / max(T - fr[-1], 1)
            else:
                i1 = int(np.searchsorted(fr, t, side="left"))
                i1 = min(i1, len(keys) - 1)
                i0 = max(i1 - 1, 0)
                u = 1.0 if fr[i1] == fr[i0] else (t - fr[i0]) / (fr[i1] - fr[i0])
            e = EASE[keys[i1][1]](float(np.clip(u, 0, 1)))
            v = vals[i0] + (vals[i1] - vals[i0]) * e
            hips[t] = hk[i0] + (hk[i1] - hk[i0]) * e
            for j, b in enumerate(names):
                bone, kind, side = ALIAS[b]
                rot[t, S.INDEX[bone]] = bone_matrix(kind, side, *v[j])
        c = Clip(self.name, rot, hips, self.fps, self.loop, "dsl")
        c.events, c.smear, c.tags, c.meta = events, smear, self.tags, self.meta
        return c


def _vec(s):
    v = [float(x) for x in s.split(",")]
    return tuple((v + [0, 0, 0])[:3])


def parse(text):
    """-> {name: Clip}"""
    specs, cur = {}, None
    for raw in text.splitlines():
        line = raw.split("#")[0].strip()
        if not line:
            continue
        tok = line.split()
        if tok[0] == "clip":
            cur = specs[tok[1]] = ClipSpec(tok[1]); continue
        if cur is None:
            raise ValueError(f"no 'clip NAME' before: {raw}")
        if tok[0] == "fps":
            cur.fps = int(tok[1]); continue
        if tok[0] == "loop":
            cur.loop = tok[1] in ("yes", "true", "1"); continue
        if tok[0] == "tags":
            cur.tags = tok[1:]; continue
        if tok[0] == "weapon":
            cur.meta["weapon"] = tok[1:]; continue
        if tok[0] in ("trail", "squash", "fx"):
            cur.meta[tok[0]] = (tok[1] if len(tok) > 1 else "yes") not in ("no", "off", "0") and (tok[1] if len(tok) > 1 else True); continue
        if not re.match(r"^\d+$", tok[0]):
            raise ValueError(f"{cur.name}: bad line: {raw}")
        f, ease, flags, bones, hips, mode = int(tok[0]), "inout", set(), {}, None, None
        for t in tok[1:]:
            t = t.rstrip(":")
            if t.startswith("ease="):
                ease = t[5:]
                if ease not in EASE:
                    raise ValueError(f"{cur.name}@{f}: unknown ease {ease}")
            elif t in ("hit", "smear", "rest", "hold"):
                flags.add(t)
            elif t.startswith("=") and t[1:].isdigit():
                mode = t
            elif "=" in t:
                k, v = t.split("=", 1)
                if k == "hips":
                    hips = np.array(_vec(v))
                elif k == "bob":
                    hips = np.array([0, float(v), 0])
                elif k in ALIAS:
                    bones[k] = _vec(v)
                else:
                    raise ValueError(f"{cur.name}@{f}: unknown bone '{k}' (know: {', '.join(ALIAS)})")
            # bare words are labels (anticipation, contact, ...)
        cur.keys.append((f, ease, flags, bones, hips, mode))
    return {n: s.build() for n, s in specs.items()}


def load(path):
    with open(path) as f:
        return parse(f.read())


# ---------------------------------------------------------------- blending
def _quat(R):
    w = np.sqrt(np.clip(1 + R[..., 0, 0] + R[..., 1, 1] + R[..., 2, 2], 1e-12, None)) / 2
    x = np.sqrt(np.clip(1 + R[..., 0, 0] - R[..., 1, 1] - R[..., 2, 2], 0, None)) / 2
    y = np.sqrt(np.clip(1 - R[..., 0, 0] + R[..., 1, 1] - R[..., 2, 2], 0, None)) / 2
    z = np.sqrt(np.clip(1 - R[..., 0, 0] - R[..., 1, 1] + R[..., 2, 2], 0, None)) / 2
    x = np.copysign(x, R[..., 2, 1] - R[..., 1, 2])
    y = np.copysign(y, R[..., 0, 2] - R[..., 2, 0])
    z = np.copysign(z, R[..., 1, 0] - R[..., 0, 1])
    q = np.stack([w, x, y, z], -1)
    return q / np.linalg.norm(q, axis=-1, keepdims=True)


def _mat(q):
    w, x, y, z = np.moveaxis(q, -1, 0)
    return np.stack([np.stack([1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)], -1),
                     np.stack([2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)], -1),
                     np.stack([2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)], -1)], -2)


def blend(Ra, Rb, u):
    """Per-bone nlerp of two poses [B,3,3] (u = 0..1)."""
    qa, qb = _quat(Ra), _quat(Rb)
    qb = np.where((qa * qb).sum(-1, keepdims=True) < 0, -qb, qb)
    q = qa * (1 - u) + qb * u
    return _mat(q / np.linalg.norm(q, axis=-1, keepdims=True))


def sequence(clips, fade=4, name="sequence"):
    """Chain clips with eased crossfades of `fade` frames (all resampled to the first clip's fps)."""
    fps = clips[0].fps
    rot, hips, events, smear = [], [], {}, set()
    for c in clips:
        n = max(int(round(len(c) * fps / c.fps)), 1)
        r = np.array([c.sample(i / fps)[0] for i in range(n)])
        h = np.array([c.sample(i / fps)[1] for i in range(n)])
        off = len(rot)
        if rot and fade:
            k = min(fade, n, len(rot))
            for j in range(k):
                u = EASE["inout"]((j + 1) / (k + 1))
                r[j] = blend(rot[off - k + j], r[j], u)
                h[j] = hips[off - k + j] * (1 - u) + h[j] * u
            rot, hips = rot[:off - k], hips[:off - k]
            off -= k
        for f, ev in getattr(c, "events", {}).items():
            events.setdefault(off + int(round(f * fps / c.fps)), []).extend(ev)
        smear.update(off + int(round(f * fps / c.fps)) for f in getattr(c, "smear", ()))
        rot += list(r); hips += list(h)
    out = Clip(name, np.array(rot), np.array(hips), fps, False, "sequence")
    out.events, out.smear, out.tags, out.meta = events, smear, [], dict(getattr(clips[0], "meta", {}))
    return out


def library():
    """All built-in .clip files: attacks + emotes (+ anything dropped in library/)."""
    import glob, os
    out = {}
    for p in sorted(glob.glob(os.path.join(os.path.dirname(__file__), "library", "*.clip"))):
        out.update(load(p))
    return out
