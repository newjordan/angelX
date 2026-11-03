"""Bones, grid-triangulated meshes per part and smooth LBS weights."""
import numpy as np
from .segment import PARTS

# bone: (name, parent, joint/pivot key)
BONES = [
    ("root", None, "root"), ("torso", "root", "root"), ("head", "torso", "neck"),
    ("upper_arm_b", "torso", "shoulder_b"), ("lower_arm_b", "upper_arm_b", "elbow_b"),
    ("upper_arm_f", "torso", "shoulder_f"), ("lower_arm_f", "upper_arm_f", "elbow_f"),
    ("upper_leg_b", "root", "hip_b"), ("lower_leg_b", "upper_leg_b", "knee_b"),
    ("upper_leg_f", "root", "hip_f"), ("lower_leg_f", "upper_leg_f", "knee_f"),
]
BONE_IDX = {b[0]: i for i, b in enumerate(BONES)}
# back-to-front draw order
DRAW_ORDER = ["upper_arm_b", "lower_arm_b", "upper_leg_b", "lower_leg_b", "upper_leg_f",
              "lower_leg_f", "torso", "head", "upper_arm_f", "lower_arm_f"]


class Part:
    pass


def _grid_mesh(mask, cell):
    """Regular grid triangulation covering mask pixels (no Delaunay needed)."""
    H, W = mask.shape
    gh, gw = -(-H // cell), -(-W // cell)
    pad = np.zeros((gh * cell, gw * cell), bool)
    pad[:H, :W] = mask
    occ = pad.reshape(gh, cell, gw, cell).any((1, 3))
    cy, cx = np.nonzero(occ)
    # vertex ids on (gh+1)x(gw+1) lattice
    vid = lambda y, x: y * (gw + 1) + x
    quads = np.stack([vid(cy, cx), vid(cy, cx + 1), vid(cy + 1, cx + 1), vid(cy + 1, cx)], 1)
    tris = np.concatenate([quads[:, [0, 1, 2]], quads[:, [0, 2, 3]]])
    used, inv = np.unique(tris, return_inverse=True)
    tris = inv.reshape(tris.shape)
    vy, vx = np.divmod(used, gw + 1)
    verts = np.stack([vx * cell, vy * cell], 1).astype(np.float64)
    return verts, tris


def _dilate(m, r=1):
    o = m.copy()
    for _ in range(r):
        n = o.copy()
        n[1:] |= o[:-1]; n[:-1] |= o[1:]; n[:, 1:] |= o[:, :-1]; n[:, :-1] |= o[:, 1:]
        o = n
    return o


def _blur(x, r):
    """Separable box blur (edge-clamped), radius r."""
    for ax in (0, 1):
        p = np.pad(x, [(r, r) if i == ax else (0, 0) for i in range(x.ndim)], mode="edge")
        c = np.cumsum(p, axis=ax, dtype=np.float64)
        c = np.concatenate([np.zeros_like(c.take([0], axis=ax)), c], axis=ax)
        n = x.shape[ax]
        x = (c.take(range(2 * r + 1, 2 * r + 1 + n), axis=ax) - c.take(range(n), axis=ax)) / (2 * r + 1)
    return x.astype(np.float32)


def _diffuse_fill(rgba, known, want, radius=None):
    """Paint `want & ~known` pixels with a smooth local average of the body colour.

    Normalised convolution over the known pixels, skipping near-black outline pixels so
    the fill reads as cloth rather than streaks of ink; three widening passes guarantee
    every hole gets a colour. Darkened slightly, as cloth in the limb's shadow."""
    h, w = known.shape
    rgb = rgba[..., :3].astype(np.float32)
    src = known & (rgb.max(-1) > 45)
    if not src.any():
        src = known
    out = rgba.copy()
    fill = want & ~known
    col = np.zeros((h, w, 3), np.float32); got = np.zeros((h, w), bool)
    r0 = radius or max(4, h // 24)
    for r in (r0, r0 * 2, r0 * 4, max(h, w)):
        k = _blur(src.astype(np.float32), r)
        c = np.stack([_blur(rgb[..., i] * src, r) for i in range(3)], -1)
        new = fill & ~got & (k > (0.3 if r < max(h, w) else 1e-6))
        col[new] = c[new] / k[new, None]; got |= new
        if got[fill].all():
            break
    sm = np.stack([_blur(col[..., i], r0) for i in range(3)], -1) / np.maximum(_blur(fill.astype(np.float32), r0), 1e-6)[..., None]
    col[fill] = sm[fill]
    out[fill, :3] = np.clip(col[fill] * 0.8, 0, 255).astype(np.uint8)
    out[fill, 3] = 255
    return out


def _row_span(mask):
    """Pixels between the leftmost and rightmost mask pixel of each row."""
    H, W = mask.shape
    any_ = mask.any(1)
    l = np.where(any_, mask.argmax(1), W)
    r = np.where(any_, W - 1 - mask[:, ::-1].argmax(1), -1)
    X = np.arange(W)[None]
    return (X >= l[:, None]) & (X <= r[:, None])


def build_rig(rgba, labels, joints, cell=6, blend=None, side_f="r", facing="right",
              amp=None, backfill=True, seam=2):
    """side_f: which image side ('r'/'l') holds the front (drawn-on-top) limbs.
    facing: 'right' or 'left' - motions are authored facing right and mirrored if left.
    amp: {bone: multiplier} scales animation angles per bone (e.g. stiff robe legs)."""
    H = labels.shape[0]
    blend = blend or max(4.0, H * 0.05)
    alpha = rgba[..., 3] > 0
    parts = []
    tor = labels == PARTS.index("torso")
    hole = np.zeros_like(tor)
    if backfill and tor.any():
        # pixels of the front arm that sit over the torso: give the torso a plausible
        # painted-in colour there so moving the arm doesn't punch a hole.
        front = np.isin(labels, [PARTS.index("upper_arm_f"), PARTS.index("lower_arm_f")])
        rows = tor.any(1)[:, None]
        hole = front & rows
        if isinstance(backfill, (list, tuple)):      # [x0, y0, x1, y1]: limit the paint-in
            x0, y0, x1, y1 = backfill
            box = np.zeros_like(hole)
            box[y0:y1, x0:x1] = True
            hole &= box
    torso_tex = _diffuse_fill(rgba, tor, tor | hole) if hole.any() else rgba
    for name in DRAW_ORDER:
        pid = PARTS.index(name)
        mask = labels == pid
        if not mask.any():
            continue
        p = Part()
        p.name = name
        src = rgba
        if name == "torso" and hole.any():
            mask = mask | hole; src = torso_tex
        elif name != "torso" and seam:
            # overlap into neighbouring opaque pixels so joints never open a crack
            mask = mask | (_dilate(mask, seam) & alpha & (labels != PARTS.index("head")))
        tex = src.copy()
        tex[..., 3] = np.where(mask, src[..., 3], 0)
        p.tex = tex
        p.verts, p.tris = _grid_mesh(mask, cell)
        b = BONE_IDX[name]
        par = BONES[b][1]
        jx, jy = joints[BONES[b][2]]
        d = np.hypot(p.verts[:, 0] - jx, p.verts[:, 1] - jy)
        s = np.clip(d / blend, 0, 1)
        s = s * s * (3 - 2 * s)  # smoothstep: near the joint follow parent half-way
        w = np.zeros((len(p.verts), len(BONES)))
        w[:, b] = 0.5 + 0.5 * s
        w[:, BONE_IDX[par]] += 0.5 - 0.5 * s
        p.weights = w
        parts.append(p)
    return {"parts": parts, "joints": joints, "shape": labels.shape,
            "map_pose": make_pose_mapper(side_f, facing, amp or {})}


def make_pose_mapper(side_f="r", facing="right", amp=None):
    """Motions use image-side keys (upper_arm_r / upper_arm_l, ...) authored for a
    right-facing character. Map them onto the rig's back/front bones, mirror for
    left-facing sprites and apply per-bone amplitude multipliers."""
    amp = amp or {}
    mirror = facing == "left"
    sm = {"r": "f", "l": "b"} if side_f == "r" else {"r": "b", "l": "f"}

    def f(pose):
        out = {}
        for k, v in pose.items():
            if k == "root_off":
                out[k] = (-v[0], v[1]) if mirror else v
                continue
            if k[-2:] in ("_r", "_l"):
                side = k[-1]
                if mirror:
                    side = "l" if side == "r" else "r"
                k = k[:-1] + sm[side]
            v = -v if mirror else v
            out[k] = out.get(k, 0.0) + v * amp.get(k, 1.0)
        return out
    return f


def bone_matrices(joints, pose):
    """pose: dict bone -> angle (rad); 'root_off': (dx,dy). Returns (B,2,3) world affines."""
    M = np.zeros((len(BONES), 3, 3))
    for i, (n, par, j) in enumerate(BONES):
        jx, jy = joints[j]
        a = pose.get(n, 0.0)
        c, s = np.cos(a), np.sin(a)
        L = np.array([[c, -s, jx - c * jx + s * jy], [s, c, jy - s * jx - c * jy], [0, 0, 1]])
        if par is None:
            dx, dy = pose.get("root_off", (0.0, 0.0))
            L[0, 2] += dx; L[1, 2] += dy
            M[i] = L
        else:
            M[i] = M[BONE_IDX[par]] @ L
    return M[:, :2, :]


def skin(part, M):
    """Linear blend skinning, vectorized."""
    vh = np.concatenate([part.verts, np.ones((len(part.verts), 1))], 1)  # V,3
    per = np.einsum("bij,vj->vbi", M, vh)  # V,B,2
    return np.einsum("vb,vbi->vi", part.weights, per)
