"""Drive a Doll through a Clip: springs prewarmed, smear/hit events, squash & stretch, fx."""
import numpy as np
from . import draw as Dr

ZZZ = [["kkk", "..k", ".k.", "kkk"], ["kk", ".k", "kk"]]


def _fx(doll, out, fx, t, X_head_px):
    if fx == "zzz":
        w = doll.idx("w")
        for i, glyph in enumerate(ZZZ * 2):
            ph = (t * 0.25 + i * 0.5) % 2.0
            if ph > 1.4:
                continue
            x = int(X_head_px[0] + 4 + ph * 6 + i)
            y = int(X_head_px[1] - 6 - ph * 9)
            g = np.array([[c != "." for c in r] for r in glyph])
            h, wd = g.shape
            if 0 <= y and y + h < out.shape[0] and 0 <= x and x + wd < out.shape[1]:
                out[y:y + h, x:x + wd][g] = w


def frames(doll, clip, yaw=0.0, pitch=20.0, facing=90.0, size=None, pad=None, squash=None,
           smear=True, fx=None, loops=1, prewarm=12, travel=False):
    """-> list of canvases. squash=None uses clip.meta['squash'] (default off).
    With size and pad unset, the canvas grows (per clip) until nothing touches the edge,
    so jumps, spins and long weapons never get clipped."""
    if size is None and pad is None:
        p = int(round(doll.h * 0.55))
        for _ in range(4):
            fr = _frames(doll, clip, yaw, pitch, facing, None, p, squash, smear, fx, loops, prewarm, travel)
            if not any(f[0].any() or f[-1].any() or f[:, 0].any() or f[:, -1].any() for f in fr):
                break
            p = int(p * 1.3) + 2
        return fr
    return _frames(doll, clip, yaw, pitch, facing, size, pad, squash, smear, fx, loops, prewarm, travel)


def _frames(doll, clip, yaw, pitch, facing, size, pad, squash, smear, fx, loops, prewarm, travel):
    T = len(clip)
    doll.reset()
    dt = 1.0 / clip.fps
    sm = getattr(clip, "smear", set())
    sq_on = squash if squash is not None else bool(getattr(clip, "meta", {}).get("squash"))
    fx = fx or getattr(clip, "meta", {}).get("fx")
    hips = clip.hips
    kw = dict(yaw=yaw, pitch=pitch, facing=facing, size=size, pad=pad, dt=dt)
    for _ in range(prewarm):          # settle capes/plumes at the first pose
        doll.frame(clip.rot[0], hips[0] * [0, 1, 0] if not travel else hips[0], **kw)
    out = []
    for k in range(T * loops):
        t = k % T
        h = hips[t].copy()
        if not travel:
            pass
        s = 1.0
        if sq_on:
            v = (hips[(t + 1) % T, 1] - hips[t - 1, 1]) * clip.fps / 2
            s = float(np.clip(1 + 0.6 * v, 0.82, 1.18))
        c = doll.frame(clip.rot[t], h, smear=smear and t in sm, squash=s, **kw)
        if fx:
            _fx(doll, c, fx, k, doll.last_head)
            Dr.cleanup(c)
        out.append(c)
    return out
