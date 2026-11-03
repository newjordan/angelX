"""Raster primitives on an ink-index canvas (uint8, 0 = clear). Pure numpy, tiny canvases."""
import numpy as np

LIGHT = np.array([0.55, 0.83])  # screen dir light travels (down-right): pixels offset this way are shaded


_GRID = {}


def _grid(h, w):
    g = _GRID.get((h, w))
    if g is None:
        gy, gx = np.mgrid[0:h, 0:w].astype(np.float32) + 0.5
        g = _GRID[(h, w)] = (gx, gy)
    return g


def capsule(out, a, b, r, ink, shade=0, mask=None):
    """Thick segment a->b (screen px, float) radius r. shade ink on the side away from the light."""
    ax, ay, bx, by = float(a[0]), float(a[1]), float(b[0]), float(b[1])
    H, W = out.shape
    x0 = max(int(min(ax, bx) - r - 1), 0); x1 = min(int(max(ax, bx) + r + 2), W)
    y0 = max(int(min(ay, by) - r - 1), 0); y1 = min(int(max(ay, by) + r + 2), H)
    if x1 <= x0 or y1 <= y0:
        return
    gx, gy = _grid(H, W)
    px, py = gx[y0:y1, x0:x1], gy[y0:y1, x0:x1]
    dx, dy = bx - ax, by - ay
    L2 = dx * dx + dy * dy or 1e-9
    t = ((px - ax) * dx + (py - ay) * dy) * (1.0 / L2)
    np.clip(t, 0, 1, out=t)
    ox = px - (ax + t * dx)
    oy = py - (ay + t * dy)
    m = ox * ox + oy * oy <= r * r + 0.15
    reg = out[y0:y1, x0:x1]
    if shade and r >= 1.4:
        sh = m & (ox * LIGHT[0] + oy * LIGHT[1] > 0.3 * r)
        reg[m] = ink
        reg[sh] = shade
    else:
        reg[m] = ink
    if mask is not None:
        mask[y0:y1, x0:x1] |= m


def disc(out, c, r, ink, shade=0, mask=None):
    capsule(out, c, c + 1e-3, r, ink, shade, mask)


def hull(pts):
    pts = sorted(map(tuple, pts))
    if len(pts) < 3:
        return np.array(pts)
    def half(P):
        h = []
        for p in P:
            while len(h) >= 2 and (h[-1][0] - h[-2][0]) * (p[1] - h[-2][1]) - (h[-1][1] - h[-2][1]) * (p[0] - h[-2][0]) <= 0:
                h.pop()
            h.append(p)
        return h
    lo, up = half(pts), half(pts[::-1])
    return np.array(lo[:-1] + up[:-1])


def poly(out, pts, ink, r=0.0, shade=0, mask=None):
    """Filled convex hull of pts, dilated by r (round)."""
    H = hull(pts)
    if len(H) < 3:
        if len(H) == 2:
            capsule(out, H[0], H[1], max(r, 0.5), ink, shade, mask)
        elif len(H) == 1:
            disc(out, H[0], max(r, 0.5), ink, shade, mask)
        return
    lo = np.floor(H.min(0) - r - 1).astype(int)
    hi = np.ceil(H.max(0) + r + 1).astype(int)
    x0, y0 = max(lo[0], 0), max(lo[1], 0)
    x1, y1 = min(hi[0], out.shape[1]), min(hi[1], out.shape[0])
    if x1 <= x0 or y1 <= y0:
        return
    gx, gy = _grid(*out.shape)
    px, py = gx[y0:y1, x0:x1], gy[y0:y1, x0:x1]
    inside = np.ones(px.shape, bool)
    n = len(H)
    area = sum(H[i][0] * H[(i + 1) % n][1] - H[(i + 1) % n][0] * H[i][1] for i in range(n))
    sgn = 1 if area > 0 else -1
    for i in range(n):
        a, b = H[i], H[(i + 1) % n]
        e = b - a
        L = np.hypot(*e) or 1e-9
        dist = sgn * (e[0] * (py - a[1]) - e[1] * (px - a[0])) / L   # >0 inside
        inside &= dist >= -r
    reg = out[y0:y1, x0:x1]
    reg[inside] = ink
    if mask is not None:
        mask[y0:y1, x0:x1] |= inside
    if r > 0:  # round the dilated corners
        for p in H:
            pass


def stamp(out, ink, pivot, ang, pos, sx=1.0, sy=1.0, ss=2):
    """Paste sprite ink[h,w] (0 clear): its pivot (px, in sprite) lands on pos (canvas px),
    rotated by ang (rad, clockwise on screen), scaled sx (across) / sy (along). Nearest + 1/4 coverage."""
    h, w = ink.shape
    c, s = np.cos(ang), np.sin(ang)
    corners = (np.array([[0, 0], [w, 0], [0, h], [w, h]], float) - pivot) * [sx, sy]
    rc = np.stack([corners[:, 0] * c - corners[:, 1] * s, corners[:, 0] * s + corners[:, 1] * c], 1) + pos
    x0, y0 = np.floor(rc.min(0)).astype(int) - 1
    x1, y1 = np.ceil(rc.max(0)).astype(int) + 1
    x0, y0 = max(x0, 0), max(y0, 0)
    x1, y1 = min(x1, out.shape[1]), min(y1, out.shape[0])
    if x1 <= x0 or y1 <= y0:
        return None
    o = (np.arange(ss) + 0.5) / ss
    gy, gx = np.mgrid[y0:y1, x0:x1]
    qx = gx[..., None, None] + o[None, None, None, :] - pos[0]
    qy = gy[..., None, None] + o[None, None, :, None] - pos[1]
    u = (c * qx + s * qy) / sx + pivot[0]
    v = (-s * qx + c * qy) / sy + pivot[1]
    ui, vi = np.floor(u).astype(int), np.floor(v).astype(int)
    ok = (ui >= 0) & (ui < w) & (vi >= 0) & (vi < h)
    smp = np.where(ok, ink[np.clip(vi, 0, h - 1), np.clip(ui, 0, w - 1)], 0).reshape(*gx.shape, ss * ss)
    cov = (smp > 0).mean(-1)
    order = np.argsort(np.abs(np.arange(ss * ss) - (ss * ss - 1) / 2))
    pick = smp[..., order]
    first = np.argmax(pick > 0, -1)
    col = np.take_along_axis(pick, first[..., None], -1)[..., 0]
    m = cov >= 0.25
    out[y0:y1, x0:x1][m] = col[m]
    return (y0, y1, x0, x1, m)


def outline(out, ink, src_mask=None):
    """1-px outline around every non-clear pixel (4-neighbour), drawn into clear pixels only."""
    a = out > 0 if src_mask is None else src_mask
    n = np.zeros_like(a)
    n[1:] |= a[:-1]; n[:-1] |= a[1:]; n[:, 1:] |= a[:, :-1]; n[:, :-1] |= a[:, 1:]
    out[n & (out == 0)] = ink


def cleanup(out, outline_ink=None):
    """Robustness pass: fill 1-px pinholes (all 4 neighbours set, i.e. seams between
    capsules/hulls) with the majority neighbour ink, and drop orphan pixels (no
    8-neighbours). 1-px notches (3 neighbours) are left alone: they are usually real
    gaps such as between legs or under an arm. Returns (#filled, #dropped)."""
    a = out > 0
    p = np.pad(out, 1)
    nb = np.stack([p[:-2, 1:-1], p[2:, 1:-1], p[1:-1, :-2], p[1:-1, 2:]])
    cnt = (nb > 0).sum(0)
    hole = ~a & (cnt >= 4)
    if hole.any():
        # majority non-zero neighbour (ties -> first)
        ys, xs = np.nonzero(hole)
        for y, x in zip(ys, xs):
            v = nb[:, y, x]; v = v[v > 0]
            vals, c = np.unique(v, return_counts=True)
            out[y, x] = vals[np.argmax(c)]
    pa = np.pad(out > 0, 1)
    n8 = sum(pa[1 + dy:pa.shape[0] - 1 + dy, 1 + dx:pa.shape[1] - 1 + dx]
             for dy in (-1, 0, 1) for dx in (-1, 0, 1) if dy or dx)
    orphan = (out > 0) & (n8 == 0)
    out[orphan] = 0
    return int(hole.sum()), int(orphan.sum())
