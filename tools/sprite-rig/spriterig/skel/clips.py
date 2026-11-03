"""Clips in the shared skeleton: per frame, a local rotation per Mixamo bone plus
the Hips translation. Sources:
  * procedural: the 15 sprite-rig motions, lifted into 3D (sagittal-plane turns)
  * BVH (Mixamo 'BVH' download, or any mocap BVH with Mixamo-ish names)
  * glTF/GLB animation channels (node names matched to Mixamo names)
  * FBX: not parsed here (no pure-python FBX reader worth trusting); see DESIGN.md.
Imports are retargeted onto our rest pose, so a T-pose source drives an I-pose rig."""
import glob, json, os, re, struct
import numpy as np
from . import skeleton as S
from .. import anim as A

B = len(S.NAMES)


class Clip:
    def __init__(self, name, rot, hips, fps=30, loop=True, source="procedural"):
        self.name, self.rot, self.hips = name, rot, hips   # [T,B,3,3], [T,3] (m, delta from rest)
        self.fps, self.loop, self.source = fps, loop, source

    def __len__(self):
        return len(self.rot)

    def sample(self, t):
        """t in seconds -> (local [B,3,3], hips [3]); nearest frame (cheap, stable for sprites)."""
        n = len(self)
        i = int(round(t * self.fps))
        i = i % n if self.loop else min(max(i, 0), n - 1)
        return self.rot[i], self.hips[i]

    def save(self, path):
        np.savez_compressed(path, rot=self.rot.astype(np.float32), hips=self.hips.astype(np.float32),
                            meta=json.dumps({"name": self.name, "fps": self.fps, "loop": self.loop,
                                             "source": self.source, "bones": S.NAMES}))

    @staticmethod
    def load(path):
        z = np.load(path)
        m = json.loads(str(z["meta"]))
        return Clip(m["name"], z["rot"].astype(float), z["hips"].astype(float), m["fps"], m["loop"], m["source"])


# ------------------------------------------------------------------ procedural
# 2D sprite-rig poses are sagittal: an angle in the side view. Lifted to 3D they are
# turns about the character's +X (its left-right axis); signs line up because a
# positive 2D angle swings a hanging limb backward and Rx(+a) swings -Y toward -Z.
LIFT = {"torso": ["Spine", "Spine1"], "head": ["Head"],
        "upper_arm_r": ["RightArm"], "lower_arm_r": ["RightForeArm"],
        "upper_arm_l": ["LeftArm"], "lower_arm_l": ["LeftForeArm"],
        "upper_leg_r": ["RightUpLeg"], "lower_leg_r": ["RightLeg"],
        "upper_leg_l": ["LeftUpLeg"], "lower_leg_l": ["LeftLeg"], "root": ["Hips"]}
SPRITE_H = 64.0  # the procedural motions are authored against a sprite height


ONESHOT = {"jump", "attack", "attack_right", "attack_left", "cast", "hurt", "death", "kick"}


def procedural(name, frames=None, fps=None):
    f, n0, fps0 = A.ANIMS[name]
    loop = name not in ONESHOT
    n, fps = frames or n0, fps or fps0
    hgt = S.height()
    rot = np.tile(np.eye(3), (n, B, 1, 1))
    hips = np.zeros((n, 3))
    for k in range(n):
        p = f(k / n, SPRITE_H)
        for key, bones in LIFT.items():
            a = float(p.get(key, 0.0))
            if key == "torso":
                a = a / 2  # shared by two spine bones
            for b in bones:
                rot[k, S.INDEX[b]] = S.rx(a)
        dx, dy = p.get("root_off", (0.0, 0.0))
        hips[k] = (0, -dy / SPRITE_H * hgt, dx / SPRITE_H * hgt)
    return Clip(name, rot, hips, fps, loop, "procedural:" + name)


def procedural_set(names=None):
    return {n: procedural(n) for n in (names or A.DEFAULT_SET.split(","))}


# ------------------------------------------------------------------ retarget
def _arc(a, b):
    """Shortest rotation taking unit vector a to b."""
    a = a / (np.linalg.norm(a) + 1e-12)
    b = b / (np.linalg.norm(b) + 1e-12)
    v, c = np.cross(a, b), float(a @ b)
    if c < -0.9999:
        ax = np.cross(a, [1, 0, 0])
        if np.linalg.norm(ax) < 1e-6:
            ax = np.cross(a, [0, 1, 0])
        ax /= np.linalg.norm(ax)
        return 2 * np.outer(ax, ax) - np.eye(3)
    K = np.array([[0, -v[2], v[1]], [v[2], 0, -v[0]], [-v[1], v[0], 0]])
    return np.eye(3) + K + K @ K / (1 + c)


def retarget(src_names, src_parent, src_offset, src_rest_rot, frames_world_rot, hips_pos, fps, name, src="bvh"):
    """Source world rotations per frame -> our locals. For each bone we keep the
    source's posed direction: G_ours = G_src · Rrest_src⁻¹ · A, with A taking our rest
    bone direction onto the source's rest direction (absorbs T-pose vs I-pose)."""
    idx = {S.bare(n): i for i, n in enumerate(src_names)}
    # source rest world rotations + positions
    Rr = [None] * len(src_names)
    Xr = np.zeros((len(src_names), 3))
    for i in range(len(src_names)):
        p = src_parent[i]
        Rr[i] = src_rest_rot[i] if p < 0 else Rr[p] @ src_rest_rot[i]
        Xr[i] = src_offset[i] if p < 0 else Xr[p] + Rr[p] @ src_offset[i]
    _, ours = S.rest()
    T = len(frames_world_rot)
    rot = np.tile(np.eye(3), (T, B, 1, 1))
    hips = np.zeros((T, 3))
    scale = S.height() / max(np.ptp(Xr[:, 1]), 1e-6)
    A_ = {}
    for b in range(B):
        nm = S.NAMES[b]
        c = S.CHILD.get(b)
        if nm not in idx or c is None or S.NAMES[c] not in idx:
            continue
        si, sc = idx[nm], idx[S.NAMES[c]]
        A_[b] = Rr[si].T @ _arc(ours[c] - ours[b], Xr[sc] - Xr[si])
    hip_i = idx.get("Hips", 0)
    for t in range(T):
        G = np.tile(np.eye(3), (B, 1, 1))
        for b in range(B):
            p = S.PARENT[b]
            if b in A_:
                G[b] = frames_world_rot[t][idx[S.NAMES[b]]] @ A_[b]
            else:
                G[b] = G[p] if p >= 0 else np.eye(3)
            rot[t, b] = G[b] if p < 0 else G[p].T @ G[b]
        hips[t] = (hips_pos[t] - hips_pos[0]) * scale
        hips[t, 1] = (hips_pos[t][1] - Xr[hip_i][1]) * scale
    return Clip(name, rot, hips, fps, True, src)


# ------------------------------------------------------------------ BVH
def load_bvh(path, name=None):
    txt = open(path).read()
    head, motion = txt.split("MOTION", 1)
    tok = head.replace("{", " { ").replace("}", " } ").split()
    names, parent, offset, chans, stack = [], [], [], [], []
    i = 0
    while i < len(tok):
        t = tok[i]
        if t in ("ROOT", "JOINT"):
            names.append(tok[i + 1]); parent.append(stack[-1] if stack else -1); i += 2
        elif t == "End":
            names.append((names[stack[-1]] if stack else "") + "_End"); parent.append(stack[-1]); chans.append([]); i += 2
        elif t == "{":
            stack.append(len(names) - 1); i += 1
        elif t == "}":
            stack.pop(); i += 1
        elif t == "OFFSET":
            offset.append([float(x) for x in tok[i + 1:i + 4]]); i += 4
        elif t == "CHANNELS":
            n = int(tok[i + 1]); chans.append(tok[i + 2:i + 2 + n]); i += 2 + n
        else:
            i += 1
    lines = motion.strip().splitlines()
    ft = float(lines[1].split(":")[1])
    data = np.array([[float(x) for x in l.split()] for l in lines[2:] if l.strip()])
    J = len(names)
    off = np.array(offset)
    world = []
    hp = []
    for row in data:
        c = 0
        loc = [np.eye(3)] * J
        pos = off[0].copy()
        for j in range(J):
            ch = chans[j]
            vals = row[c:c + len(ch)]; c += len(ch)
            rots = [(n[0], v) for n, v in zip(ch, vals) if n.endswith("rotation")]
            ps = {n[0]: v for n, v in zip(ch, vals) if n.endswith("position")}
            if rots:
                loc[j] = S.euler("".join(a for a, _ in rots), [v for _, v in rots])
            if j == 0 and ps:
                pos = np.array([ps.get("X", 0), ps.get("Y", 0), ps.get("Z", 0)])
        G = [None] * J
        for j in range(J):
            G[j] = loc[j] if parent[j] < 0 else G[parent[j]] @ loc[j]
        world.append(G); hp.append(pos)
    return retarget(names, parent, off, [np.eye(3)] * J, world, hp, round(1 / ft),
                    name or os.path.splitext(os.path.basename(path))[0], "bvh:" + os.path.basename(path))


def save_bvh(clip, path):
    """Writes a clip as a Mixamo-named BVH (cm, ZXY) — round-trip tests and hand-off."""
    ch = {i: [c for c in range(B) if S.PARENT[c] == i] for i in range(B)}
    out = ["HIERARCHY"]

    def emit(i, d):
        ind = "  " * d
        kw = "ROOT" if d == 0 else "JOINT"
        out.append(f"{ind}{kw} {S.P}{S.NAMES[i]}")
        out.append(ind + "{")
        o = S.OFFSET[i] * 100
        out.append(f"{ind}  OFFSET {o[0]:.4f} {o[1]:.4f} {o[2]:.4f}")
        out.append(f"{ind}  CHANNELS " + ("6 Xposition Yposition Zposition " if d == 0 else "3 ") + "Zrotation Xrotation Yrotation")
        for c in ch[i]:
            emit(c, d + 1)
        if not ch[i]:
            out.append(f"{ind}  End Site\n{ind}  {{\n{ind}    OFFSET 0 0 0\n{ind}  }}")
        out.append(ind + "}")
    emit(0, 0)
    order = []

    def walk(i):
        order.append(i)
        for c in ch[i]:
            walk(c)
    walk(0)
    out += ["MOTION", f"Frames: {len(clip)}", f"Frame Time: {1 / clip.fps:.6f}"]
    for t in range(len(clip)):
        vals = list((S.OFFSET[0] + clip.hips[t]) * 100)
        for i in order:
            vals += list(zxy_degrees(clip.rot[t, i]))
        out.append(" ".join(f"{v:.4f}" for v in vals))
    open(path, "w").write("\n".join(out) + "\n")


def zxy_degrees(m):
    """Inverse of euler('ZXY'): m = Rz(z) Rx(x) Ry(y)."""
    x = np.arcsin(np.clip(m[2, 1], -1, 1))
    z = np.arctan2(-m[0, 1], m[1, 1])
    y = np.arctan2(-m[2, 0], m[2, 2])
    return np.degrees([z, x, y])


# ------------------------------------------------------------------ glTF / GLB
def read_glb(path):
    f = open(path, "rb").read()
    n = struct.unpack("<I", f[12:16])[0]
    js = json.loads(f[20:20 + n])
    binc = f[20 + n + 8:]
    return js, binc


def accessor(js, binc, i):
    a = js["accessors"][i]
    bv = js["bufferViews"][a["bufferView"]]
    comp = {5126: np.float32, 5125: np.uint32, 5123: np.uint16, 5121: np.uint8, 5122: np.int16, 5120: np.int8}[a["componentType"]]
    k = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}[a["type"]]
    off = bv.get("byteOffset", 0) + a.get("byteOffset", 0)
    stride = bv.get("byteStride", 0)
    if stride and stride != k * np.dtype(comp).itemsize:
        raw = np.frombuffer(binc, np.uint8, a["count"] * stride, off).reshape(-1, stride)
        return raw[:, :k * np.dtype(comp).itemsize].copy().view(comp).reshape(a["count"], k)
    return np.frombuffer(binc, comp, a["count"] * k, off).reshape(a["count"], k) if k > 1 else \
        np.frombuffer(binc, comp, a["count"], off)


def quat_m(q):
    x, y, z, w = q
    return np.array([[1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
                     [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
                     [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)]])


def load_glb_anim(path, which=0, fps=30):
    js, binc = read_glb(path)
    if not js.get("animations"):
        raise ValueError(f"{path}: no animations")
    nodes = js["nodes"]
    names = [nd.get("name", f"n{i}") for i, nd in enumerate(nodes)]
    parent = [-1] * len(nodes)
    for i, nd in enumerate(nodes):
        for c in nd.get("children", []):
            parent[c] = i
    order = []  # parents first
    seen = set()

    def visit(i):
        if i in seen:
            return
        if parent[i] >= 0:
            visit(parent[i])
        seen.add(i); order.append(i)
    for i in range(len(nodes)):
        visit(i)
    rest_r = [quat_m(nd.get("rotation", [0, 0, 0, 1])) for nd in nodes]
    off = np.array([nd.get("translation", [0, 0, 0]) for nd in nodes], float)
    an = js["animations"][which]
    tracks = {}
    tmax = 0
    for ch in an["channels"]:
        s = an["samplers"][ch["sampler"]]
        tt = accessor(js, binc, s["input"]).astype(float)
        vv = accessor(js, binc, s["output"]).astype(float)
        tracks[(ch["target"]["node"], ch["target"]["path"])] = (tt, vv)
        tmax = max(tmax, tt[-1])
    T = max(int(tmax * fps) + 1, 1)
    world, hp = [], []
    # reindex so parents come first
    pos = {n: k for k, n in enumerate(order)}
    nm = [names[i] for i in order]
    par = [pos[parent[i]] if parent[i] >= 0 else -1 for i in order]
    for f in range(T):
        t = f / fps
        G = [None] * len(order)
        hips = None
        for k, i in enumerate(order):
            r = rest_r[i]
            if (i, "rotation") in tracks:
                tt, vv = tracks[(i, "rotation")]
                r = quat_m(vv[min(np.searchsorted(tt, t), len(tt) - 1)])
            G[k] = r if par[k] < 0 else G[par[k]] @ r
            if S.bare(names[i]) == "Hips":
                tr = tracks.get((i, "translation"))
                hips = tr[1][min(np.searchsorted(tr[0], t), len(tr[0]) - 1)] if tr else off[i]
        world.append(G); hp.append(hips if hips is not None else np.zeros(3))
    return retarget(nm, par, off[order], [rest_r[i] for i in order], world, hp, fps,
                    an.get("name", os.path.basename(path)), "glb:" + os.path.basename(path))


def load_folder(folder, in_place=False):
    """Every clip Frosty drops in a folder. BVH and GLB/glTF-binary import today;
    FBX/DAE are listed so the caller can say what to re-export."""
    clips, skipped = {}, []
    for p in sorted(glob.glob(os.path.join(folder, "*"))):
        ext = os.path.splitext(p)[1].lower()
        name = re.sub(r"\W+", "_", os.path.splitext(os.path.basename(p))[0]).lower()
        try:
            if ext == ".bvh":
                clips[name] = load_bvh(p, name)
            elif ext == ".glb":
                clips[name] = load_glb_anim(p)
            elif ext == ".npz":
                clips[name] = Clip.load(p)
            elif ext == ".fbx":
                from . import fbx
                clips[name] = fbx.load_clip(p, name)
            elif ext == ".dae":
                skipped.append((p, "Collada import is TODO: use FBX Binary or BVH"))
        except Exception as e:  # keep going; report
            skipped.append((p, repr(e)))
    if in_place:  # strip root travel (X/Z), keep the hips bob
        for c in clips.values():
            c.hips[:, [0, 2]] = 0
    return clips, skipped
