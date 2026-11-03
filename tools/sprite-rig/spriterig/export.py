"""Sprite sheets, PNG frames and polished GIFs (shared canvas, ground shadow, labels)."""
import json, os
import numpy as np
from PIL import Image, ImageDraw


def union_bbox(frame_lists, pad=4):
    a = None
    for fr in frame_lists:
        for f in fr:
            m = f[..., 3] > 0
            a = m if a is None else (a | m)
    ys, xs = np.nonzero(a)
    H, W = a.shape
    return (max(xs.min() - pad, 0), max(ys.min() - pad, 0),
            min(xs.max() + 1 + pad, W), min(ys.max() + 1 + pad, H))


def crop_all(frames, box):
    x0, y0, x1, y1 = box
    return [f[y0:y1, x0:x1] for f in frames]


def save_sheet(frames, path_png, fps, loop):
    fh, fw = frames[0].shape[:2]
    Image.fromarray(np.concatenate(frames, 1)).save(path_png)
    meta = {"image": os.path.basename(path_png), "fps": fps, "frame_w": fw, "frame_h": fh,
            "loop": loop, "frames": [{"x": i * fw, "y": 0, "w": fw, "h": fh}
                                     for i in range(len(frames))]}
    with open(path_png[:-4] + ".json", "w") as f:
        json.dump(meta, f, indent=1)


def _backdrop(w, h, ground_y, bg=(34, 32, 44)):
    im = Image.new("RGB", (w, h), bg)
    d = ImageDraw.Draw(im)
    d.rectangle([0, ground_y, w, h], fill=tuple(max(c - 10, 0) for c in bg))
    return im


def compose(frame, ground_y, label=None, bg=(34, 32, 44), shadow_x=None):
    h, w = frame.shape[:2]
    im = _backdrop(w, h, ground_y, bg)
    sh = Image.new("L", (w, h), 0)
    cx = shadow_x if shadow_x is not None else w / 2
    rw = w * 0.22
    ImageDraw.Draw(sh).ellipse([cx - rw, ground_y - rw * 0.12, cx + rw, ground_y + rw * 0.12], fill=90)
    im.paste((10, 8, 14), (0, 0), sh)
    fr = Image.fromarray(frame, "RGBA")
    im.paste(fr, (0, 0), fr)
    if label:
        ImageDraw.Draw(im).text((4, 3), label, fill=(235, 225, 200))
    return im


def label(im, text):
    im = im.copy()
    ImageDraw.Draw(im).text((4, 3), text, fill=(235, 225, 200))
    return im


def save_gif(images, path, fps):
    # one shared adaptive palette for stable colours across frames
    strip = Image.new("RGB", (images[0].width, images[0].height * len(images)))
    for i, im in enumerate(images):
        strip.paste(im, (0, i * images[0].height))
    if strip.height > 4096:  # palette from a subsample of frames is plenty
        step = -(-len(images) // (4096 // images[0].height or 1))
        return save_gif_pal(images, path, fps, images[::step])
    pal = strip.quantize(255, method=Image.FASTOCTREE, dither=Image.NONE)
    q = [im.quantize(palette=pal, dither=Image.NONE) for im in images]
    q[0].save(path, save_all=True, append_images=q[1:], duration=int(1000 / fps), loop=0,
              optimize=False, disposal=1)


def save_gif_pal(images, path, fps, sample):
    strip = Image.new("RGB", (images[0].width, images[0].height * len(sample)))
    for i, im in enumerate(sample):
        strip.paste(im, (0, i * images[0].height))
    pal = strip.quantize(255, method=Image.FASTOCTREE, dither=Image.NONE)
    q = [im.quantize(palette=pal, dither=Image.NONE) for im in images]
    q[0].save(path, save_all=True, append_images=q[1:], duration=int(1000 / fps), loop=0,
              optimize=False, disposal=1)
