"""Load a sprite, key out background, split into humanoid parts by silhouette heuristics."""
import json
import numpy as np
from PIL import Image

PARTS = ["head", "torso", "upper_arm_b", "lower_arm_b", "upper_arm_f", "lower_arm_f",
         "upper_leg_b", "lower_leg_b", "upper_leg_f", "lower_leg_f"]


def load_sprite(path, key_tol=24):
    im = np.asarray(Image.open(path).convert("RGBA")).copy()
    a = im[..., 3]
    if a.min() == 255:  # opaque: key out the corner colour
        bg = im[0, 0, :3].astype(int)
        d = np.abs(im[..., :3].astype(int) - bg).sum(-1)
        im[..., 3] = np.where(d <= key_tol, 0, 255).astype(np.uint8)
    return im


def segment(rgba, override=None):
    """Return (labels HxW int8 with -1 = empty, joints dict name->(x,y))."""
    m = rgba[..., 3] > 16
    ys, xs = np.nonzero(m)
    y0, y1 = ys.min(), ys.max() + 1
    H = y1 - y0
    cx = float(np.median(xs))
    o = override or {}
    fr = o.get("fractions", {})
    neck = y0 + H * fr.get("neck", 0.24)
    hip = y0 + H * fr.get("hip", 0.56)
    knee = y0 + H * fr.get("knee", 0.78)
    # torso half width: median half-width of rows in the middle torso band
    band = m[int(y0 + H * 0.32):int(y0 + H * 0.5)]
    hw = []
    for row in band:
        xx = np.nonzero(row)[0]
        if len(xx):
            # contiguous run containing cx
            c = int(round(cx))
            l = c
            while l > 0 and row[l - 1]: l -= 1
            r = c
            while r < len(row) - 1 and row[r + 1]: r += 1
            hw.append(max(c - l, r - c))
    thw = o.get("torso_half_width", float(np.median(hw)) if hw else H * 0.12)
    shoulder_y = neck + H * 0.04
    elbow_y = neck + (hip - neck) * 0.55 + H * 0.02

    Y, X = np.mgrid[0:m.shape[0], 0:m.shape[1]]
    lab = np.full(m.shape, -1, np.int8)
    P = {n: i for i, n in enumerate(PARTS)}
    lab[m & (Y < neck)] = P["head"]
    body = m & (Y >= neck) & (Y < hip)
    side = X < cx  # image-left = "back" limb (drawn behind)
    arm = body & (np.abs(X - cx) > thw + 0.5)
    lab[body & ~arm] = P["torso"]
    lab[arm & side & (Y < elbow_y)] = P["upper_arm_b"]
    lab[arm & side & (Y >= elbow_y)] = P["lower_arm_b"]
    lab[arm & ~side & (Y < elbow_y)] = P["upper_arm_f"]
    lab[arm & ~side & (Y >= elbow_y)] = P["lower_arm_f"]
    # anything below the hip that is far out sideways is still arm (hands hanging low)
    low = m & (Y >= hip)
    lowarm = low & (np.abs(X - cx) > thw + 0.5)
    lab[lowarm & side] = P["lower_arm_b"]
    lab[lowarm & ~side] = P["lower_arm_f"]
    leg = low & ~lowarm
    lab[leg & side & (Y < knee)] = P["upper_leg_b"]
    lab[leg & side & (Y >= knee)] = P["lower_leg_b"]
    lab[leg & ~side & (Y < knee)] = P["upper_leg_f"]
    lab[leg & ~side & (Y >= knee)] = P["lower_leg_f"]

    def armx(s):
        sel = arm & (side if s < 0 else ~side)
        return float(X[sel].mean()) if sel.any() else cx + s * (thw + 4)

    def legx(s):
        sel = leg & (side if s < 0 else ~side)
        return float(X[sel].mean()) if sel.any() else cx + s * thw * 0.5

    J = {
        "root": (cx, hip), "neck": (cx, neck), "head_top": (cx, float(y0)),
        "shoulder_b": (cx - thw, shoulder_y), "shoulder_f": (cx + thw, shoulder_y),
        "elbow_b": (armx(-1), elbow_y), "elbow_f": (armx(1), elbow_y),
        "hand_b": (armx(-1), hip), "hand_f": (armx(1), hip),
        "hip_b": (legx(-1), hip), "hip_f": (legx(1), hip),
        "knee_b": (legx(-1), knee), "knee_f": (legx(1), knee),
        "foot_b": (legx(-1), float(y1)), "foot_f": (legx(1), float(y1)),
    }
    J.update({k: tuple(v) for k, v in o.get("joints", {}).items()})
    # optional explicit part boxes: {"head": [x0,y0,x1,y1], ...} applied in order
    for name, (bx0, by0, bx1, by1) in o.get("parts", {}).items():
        sel = m & (X >= bx0) & (X < bx1) & (Y >= by0) & (Y < by1)
        lab[sel] = P[name]
    # optional polygons: {"lower_arm_f": [[x,y], ...], ...} painted in order (last wins)
    polys = o.get("polys", {})
    items = polys.items() if isinstance(polys, dict) else polys
    if items:
        from PIL import Image as _I, ImageDraw as _D
        for name, pts in items:
            im = _I.new("L", (m.shape[1], m.shape[0]), 0)
            _D.Draw(im).polygon([tuple(p) for p in pts], fill=1)
            lab[m & (np.asarray(im) > 0)] = P[name]
    return lab, J


def load_override(path):
    with open(path) as f:
        return json.load(f)
