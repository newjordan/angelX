"""Batched triangle rasterizer with affine texture mapping (numpy only).

Modes
  nearest : point sampling, rendered directly at output scale. Keeps pixel art crisp
            (rotated pixels stay hard-edged, RotSprite-like look at scale>=2).
  aa      : supersampled (ss x ss) bilinear sampling of a *premultiplied* texture, then
            box-filtered down in premultiplied space -> clean edges, no dark fringes,
            no seams at joints.
All triangles of a part are rasterized in one vectorized batch (no per-triangle loop).
"""
import numpy as np
from .rig import bone_matrices, skin


def _batch_raster(acc, tex, src, dst, tris, bilinear, chunk=384):
    """Split triangles into size-sorted chunks so each chunk's patch grid stays tight."""
    D = dst[tris]
    ext = (D.max(1) - D.min(1)).max(1)
    order = np.argsort(ext)
    for i in range(0, len(order), chunk):
        _raster_chunk(acc, tex, src, dst, tris[order[i:i + chunk]], bilinear)


def _raster_chunk(acc, tex, src, dst, tris, bilinear):
    """acc: (H,W,4) float32 premultiplied canvas, painted in place (src-over).
    tex: (th,tw,4) float32 premultiplied. src/dst: vertex arrays in texture / canvas px."""
    H, W = acc.shape[:2]
    th, tw = tex.shape[:2]
    D = dst[tris].astype(np.float32)   # T,3,2
    S = src[tris].astype(np.float32)
    lo = np.floor(D.min(1)).astype(np.int32)
    hi = np.ceil(D.max(1)).astype(int) + 1
    lo = np.maximum(lo, 0); hi = np.minimum(hi, [W, H])
    keep = (hi > lo).all(1)
    D, S, lo, hi = D[keep], S[keep], lo[keep], hi[keep]
    if not len(D):
        return
    ax, ay = D[:, 0, 0], D[:, 0, 1]; bx, by = D[:, 1, 0], D[:, 1, 1]; cx, cy = D[:, 2, 0], D[:, 2, 1]
    den = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy)
    ok = np.abs(den) > 1e-9
    D, S, lo, hi, den = D[ok], S[ok], lo[ok], hi[ok], den[ok]
    ax, ay, bx, by, cx, cy = ax[ok], ay[ok], bx[ok], by[ok], cx[ok], cy[ok]
    pw, ph = (hi - lo).max(0)
    gy, gx = np.mgrid[0:ph, 0:pw].astype(np.float32)
    px = lo[:, 0, None, None] + gx[None] + 0.5           # T,ph,pw
    py = lo[:, 1, None, None] + gy[None] + 0.5
    inb = (px < hi[:, 0, None, None]) & (py < hi[:, 1, None, None])
    r = lambda v: v[:, None, None]
    l0 = (r(by - cy) * (px - r(cx)) + r(cx - bx) * (py - r(cy))) / r(den)
    l1 = (r(cy - ay) * (px - r(cx)) + r(ax - cx) * (py - r(cy))) / r(den)
    l2 = 1 - l0 - l1
    e = -1e-4
    ins = inb & (l0 >= e) & (l1 >= e) & (l2 >= e)
    idx = np.nonzero(ins)
    t = idx[0]
    a0, a1, a2 = l0[idx], l1[idx], l2[idx]
    sx = a0 * S[t, 0, 0] + a1 * S[t, 1, 0] + a2 * S[t, 2, 0]
    sy = a0 * S[t, 0, 1] + a1 * S[t, 1, 1] + a2 * S[t, 2, 1]
    oy = py[idx].astype(int); ox = px[idx].astype(int)
    # de-duplicate pixels covered by two triangles of the same part (shared edges)
    lin = oy * W + ox
    lin, first = np.unique(lin, return_index=True)
    sx, sy = sx[first], sy[first]
    oy, ox = np.divmod(lin, W)
    if bilinear:
        fx = np.clip(sx - 0.5, 0, tw - 1.001); fy = np.clip(sy - 0.5, 0, th - 1.001)
        x0 = fx.astype(int); y0 = fy.astype(int); wx = (fx - x0)[:, None]; wy = (fy - y0)[:, None]
        col = (tex[y0, x0] * (1 - wx) * (1 - wy) + tex[y0, x0 + 1] * wx * (1 - wy)
               + tex[y0 + 1, x0] * (1 - wx) * wy + tex[y0 + 1, x0 + 1] * wx * wy)
    else:
        col = tex[np.clip(sy.astype(int), 0, th - 1), np.clip(sx.astype(int), 0, tw - 1)]
    a = col[:, 3:4]
    acc[oy, ox] = col + acc[oy, ox] * (1 - a)


def _prep(rig):
    """Cache premultiplied float textures on each part."""
    for p in rig["parts"]:
        if not hasattr(p, "ptex"):
            t = p.tex.astype(np.float32) / 255
            t[..., :3] *= t[..., 3:4]
            p.ptex = t


def render_pose(rig, pose, canvas, offset, scale=1, mode="aa", ss=None):
    """Render one pose. Only the bbox of the skinned mesh is rasterized (the canvas can be
    large to fit jumps/falls without costing time)."""
    _prep(rig)
    ss = ss or 2
    s = ss if mode == "aa" else 1
    k = scale * s
    Wo, Ho = canvas[0] * scale, canvas[1] * scale
    M = bone_matrices(rig["joints"], rig.get("map_pose", lambda q: q)(pose))
    D = [(p, (skin(p, M) + offset) * k) for p in rig["parts"]]
    allv = np.concatenate([d for _, d in D])
    x0 = max(int(np.floor(allv[:, 0].min() / s)) - 1, 0)
    y0 = max(int(np.floor(allv[:, 1].min() / s)) - 1, 0)
    x1 = min(int(np.ceil(allv[:, 0].max() / s)) + 2, Wo)
    y1 = min(int(np.ceil(allv[:, 1].max() / s)) + 2, Ho)
    out = np.zeros((Ho, Wo, 4), np.uint8)
    if x1 <= x0 or y1 <= y0:
        return out
    acc = np.zeros(((y1 - y0) * s, (x1 - x0) * s, 4), np.float32)
    o = np.array([x0 * s, y0 * s], float)
    for p, d in D:
        _batch_raster(acc, p.ptex, p.verts, d - o, p.tris, bilinear=(mode == "aa"))
    if s > 1:
        acc = acc.reshape(y1 - y0, s, x1 - x0, s, 4).mean((1, 3))
    a = acc[..., 3:4]
    rgb = acc[..., :3] / np.maximum(a, 1e-6)
    out[y0:y1, x0:x1] = (np.clip(np.concatenate([rgb, a], -1), 0, 1) * 255 + 0.5).astype(np.uint8)
    return out


def render_frames(rig, anim_fn, n, margin=None, scale=1, mode="aa", ss=None, canvas=None, offset=None):
    h, w = rig["shape"]
    if canvas is None:
        m = margin if margin is not None else int(0.35 * h)
        canvas, offset = (w + 2 * m, h + 2 * m), (m, m)
    loop = getattr(anim_fn, "loop", True)
    ts = [i / n for i in range(n)] if loop else [i / max(n - 1, 1) for i in range(n)]
    return [render_pose(rig, anim_fn(t, h), canvas, offset, scale, mode, ss) for t in ts]
