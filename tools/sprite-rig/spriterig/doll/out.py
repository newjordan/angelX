"""Canvas -> RGBA / sheets / GIFs / JSON for the doll animator."""
import json
import numpy as np
from PIL import Image, ImageDraw
from ..delve.source import palette

_LUT = None


def lut():
    global _LUT
    if _LUT is None:
        p = palette()
        _LUT = np.zeros((len(p) + 1, 4), np.uint8)
        for i, k in enumerate(sorted(p)):
            _LUT[i + 1] = (*p[k], 255)
    return _LUT


def rgba(canvas):
    return lut()[canvas]


def bbox(frames, pad=2):
    m = np.zeros_like(frames[0], bool)
    for f in frames:
        m |= f > 0
    ys, xs = np.nonzero(m)
    if not len(ys):
        return 0, frames[0].shape[0], 0, frames[0].shape[1]
    return (max(ys.min() - pad, 0), min(ys.max() + pad + 1, m.shape[0]),
            max(xs.min() - pad, 0), min(xs.max() + pad + 1, m.shape[1]))


def tile(frames, scale=3, bg=(34, 32, 44), labels=None, cols=None, floor=None):
    """Frames (same size) -> one PIL image grid, scaled nearest, on a backdrop."""
    h, w = frames[0].shape
    cols = cols or len(frames)
    rows = (len(frames) + cols - 1) // cols
    lh = 10 if labels else 0
    im = Image.new("RGB", (cols * w * scale, rows * (h * scale + lh)), bg)
    for i, f in enumerate(frames):
        c = Image.fromarray(rgba(f), "RGBA").resize((w * scale, h * scale), Image.NEAREST)
        x, y = (i % cols) * w * scale, (i // cols) * (h * scale + lh)
        if floor is not None:
            ImageDraw.Draw(im).rectangle([x, y + int(floor * scale), x + w * scale - 1, y + int(floor * scale) + 1], fill=(52, 48, 62))
        im.paste(c, (x, y + lh), c)
        if labels:
            ImageDraw.Draw(im).text((x + 3, y), labels[i], fill=(220, 220, 220))
    return im


def save_gif(images, path, fps):
    images[0].save(path, save_all=True, append_images=images[1:], duration=int(round(1000 / fps)),
                   loop=0, disposal=1, optimize=True)


def save_sheet(frames, path_png, meta):
    """Horizontal strip PNG (transparent) + sibling JSON with per-frame events."""
    h, w = frames[0].shape
    im = Image.new("RGBA", (w * len(frames), h), (0, 0, 0, 0))
    for i, f in enumerate(frames):
        im.paste(Image.fromarray(rgba(f), "RGBA"), (i * w, 0))
    im.save(path_png)
    meta = dict(meta, frame_w=w, frame_h=h, frames=len(frames), image=path_png.split("/")[-1])
    with open(path_png[:-4] + ".json", "w") as f:
        json.dump(meta, f, indent=1)
    return meta
