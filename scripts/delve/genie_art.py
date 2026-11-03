#!/usr/bin/env python3
"""Cut the art team's genie into the Delve's embedded assets.

The painted sheets (transparent PNGs, any size) go in; the frames the wish
scroll draws come out, already shrunk to the size drawn on a 768-wide seat
screen (the game halves them itself for a 384-wide one):

    genie-idle.png, genie-talk.png, genie-cheer.png  4-frame strips
    genie-free.png                                   one frame
    lamp.png                                         one frame (optional)

Each strip is split into four equal frames, cropped to the box all four
share (so he stays put from frame to frame), and shrunk to HEIGHT rows;
the four are laid side by side again. Alpha is made hard (the game's
pictures have no half-clear pixels).

    python3 scripts/delve/genie_art.py [SOURCE_DIR]

SOURCE_DIR defaults to $TMPDIR/delve-play/genie-art (/tmp when unset). Then rebuild:
the assets are `include_bytes!`'d by crates/delve/src/render/genie.rs.
"""

import os
import sys
import tempfile
from pathlib import Path

from PIL import Image

HEIGHT = 132          # a strip frame's rows on a 768-wide screen
FREE_HEIGHT = 168     # the freeing's picture
LAMP_HEIGHT = 28      # the lamp icon beside the gauge
FRAMES = 4

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "crates/delve/assets/genie"


def box(images):
    """The box every image's opaque pixels fit in together."""
    boxes = [im.getchannel("A").point(lambda a: 255 if a > 24 else 0).getbbox() for im in images]
    boxes = [b for b in boxes if b]
    return (min(b[0] for b in boxes), min(b[1] for b in boxes),
            max(b[2] for b in boxes), max(b[3] for b in boxes))


def shrink(im, height):
    """`im` at `height` rows, smoothed then made hard-edged."""
    width = max(1, round(im.width * height / im.height))
    small = im.resize((width, height), Image.LANCZOS, reducing_gap=3.0)
    alpha = small.getchannel("A").point(lambda a: 255 if a >= 128 else 0)
    small.putalpha(alpha)
    return small


def strip(src, name):
    sheet = Image.open(src / f"{name}.png").convert("RGBA")
    w = sheet.width // FRAMES
    frames = [sheet.crop((i * w, 0, (i + 1) * w, sheet.height)) for i in range(FRAMES)]
    shared = box(frames)
    frames = [shrink(f.crop(shared), HEIGHT) for f in frames]
    out = Image.new("RGBA", (frames[0].width * FRAMES, HEIGHT), (0, 0, 0, 0))
    for i, f in enumerate(frames):
        out.paste(f, (i * f.width, 0))
    out.save(OUT / f"{name}.png", optimize=True)
    print(f"{name}: {FRAMES} frames of {frames[0].width}x{HEIGHT}")


def single(src, name, height, required=True):
    path = src / f"{name}.png"
    if not path.exists():
        if required:
            sys.exit(f"missing {path}")
        print(f"{name}: not painted yet (the game draws its own)")
        return
    im = Image.open(path).convert("RGBA")
    im = shrink(im.crop(box([im])), height)
    im.save(OUT / f"{name}.png", optimize=True)
    print(f"{name}: {im.width}x{im.height}")


def main():
    src = Path(sys.argv[1] if len(sys.argv) > 1 else os.path.join(tempfile.gettempdir(), "delve-play", "genie-art"))
    OUT.mkdir(parents=True, exist_ok=True)
    for name in ("genie-idle", "genie-talk", "genie-cheer"):
        strip(src, name)
    single(src, "genie-free", FREE_HEIGHT)
    single(src, "lamp", LAMP_HEIGHT, required=False)


if __name__ == "__main__":
    main()
