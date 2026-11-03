#!/usr/bin/env python3
"""The page's frames, cut from the Delve's own HUD chrome, built to skin.css.

    python3 crates/web/shell/art/frames.py      (needs Pillow)

Every panel, plate and slot the page draws around its words is the realm's
reviewed HUD kit (crates/delve/assets/art-slots/dgn.hud.chrome/v1): the
minimap's studded iron frame for windows, the status gem on a window's
crown, the toast's capped plate for lines and buttons, the action plates
for keys and slots, wells and buttons. Nothing is redrawn: corners and caps are the kit's
own pixels, a window's rails are the kit's rail repeated, and its face is
the plates' own deep teal, darkened for a page of words.

skin.css holds each as a data URI in a custom property on :root, with the
nine-slice widths beside it; shell.css lays them as border images at the
picture's own whole-number scale (`--px`), nearest-sampled.
"""

import base64
import io
import os
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
KIT = os.path.join(HERE, '..', '..', '..', 'delve', 'assets', 'art-slots', 'dgn.hud.chrome', 'v1')

# The kit's face, and the deeper face a window of words sits on.
FACE = (0x13, 0x27, 0x24, 255)
DEEP = (0x0b, 0x15, 0x14, 255)
WELL = (0x07, 0x0d, 0x0c, 255)
CLEAR = (0, 0, 0, 0)


def kit(name):
    return Image.open(os.path.join(KIT, name + '.png')).convert('RGBA')


def uri(im):
    buf = io.BytesIO()
    im.save(buf, 'PNG', optimize=True)
    return 'url("data:image/png;base64,' + base64.b64encode(buf.getvalue()).decode() + '")'


def recolour(im, old, new):
    out = im.copy()
    px = out.load()
    for y in range(out.height):
        for x in range(out.width):
            if px[x, y] == old:
                px[x, y] = new
    return out


def window():
    """The minimap's frame as a window: its four studded corners, its rails
    run between them, the face filled in. 12 corner + 4 rail + 12 corner."""
    src = kit('minimap')
    c, mid, n = 12, 4, src.width  # 38
    size = 2 * c + mid
    out = Image.new('RGBA', (size, size), CLEAR)
    # Corners.
    out.paste(src.crop((0, 0, c, c)), (0, 0))
    out.paste(src.crop((n - c, 0, n, c)), (c + mid, 0))
    out.paste(src.crop((0, n - c, c, n)), (0, c + mid))
    out.paste(src.crop((n - c, n - c, n, n)), (c + mid, c + mid))
    # Rails: a plain column and row of the kit's, clear of its ornaments.
    top = src.crop((16, 0, 17, c))
    bottom = src.crop((16, n - c, 17, n))
    left = src.crop((0, 16, c, 17))
    right = src.crop((n - c, 16, n, 17))
    for i in range(mid):
        out.paste(top, (c + i, 0))
        out.paste(bottom, (c + i, c + mid))
        out.paste(left, (0, c + i))
        out.paste(right, (c + mid, c + i))
    # The face: every clear pixel inside the rails.
    px = out.load()
    inside = lambda x, y: 4 <= x < size - 4 and 4 <= y < size - 4
    for y in range(size):
        for x in range(size):
            if px[x, y][3] == 0 and inside(x, y) and is_enclosed(out, x, y):
                px[x, y] = DEEP
    return out, c


def is_enclosed(im, x, y):
    """Whether a clear pixel is inside the frame: ink to its left and right
    and above and below along its row and column."""
    px = im.load()
    w, h = im.size
    hit = lambda pts: any(px[a, b][3] for a, b in pts)
    return (hit((i, y) for i in range(0, x)) and hit((i, y) for i in range(x + 1, w))
            and hit((x, j) for j in range(0, y)) and hit((x, j) for j in range(y + 1, h)))


def even(im, sx, sy, cx, cy, mid=4):
    """`im` with its middle span made even: its corners (`sx` wide, `sy`
    tall) kept, column `cx` and row `cy` run `mid` pixels between them, so
    a nine-slice stretches nothing but a plain rail and the face."""
    w, h = im.size
    out = Image.new('RGBA', (2 * sx + mid, 2 * sy + mid), CLEAR)
    W, H = out.size
    out.paste(im.crop((0, 0, sx, sy)), (0, 0))
    out.paste(im.crop((w - sx, 0, w, sy)), (W - sx, 0))
    out.paste(im.crop((0, h - sy, sx, h)), (0, H - sy))
    out.paste(im.crop((w - sx, h - sy, w, h)), (W - sx, H - sy))
    for i in range(mid):
        out.paste(im.crop((cx, 0, cx + 1, sy)), (sx + i, 0))
        out.paste(im.crop((cx, h - sy, cx + 1, h)), (sx + i, H - sy))
        out.paste(im.crop((0, cy, sx, cy + 1)), (0, sy + i))
        out.paste(im.crop((w - sx, cy, w, cy + 1)), (W - sx, sy + i))
        for j in range(mid):
            out.putpixel((sx + i, sy + j), im.getpixel((cx, cy)))
    return out


def plate(name='toast'):
    """The toast's capped plate: lines and buttons. Caps 9 wide, rails 4."""
    return even(kit(name), 9, 4, 12, 6)


def slot(name):
    """An action plate: keys and slots. Corners 5."""
    return even(kit(name), 5, 5, 8, 8)


def main():
    win, cut = window()
    props = {
        '--art-window': uri(win),
        '--art-plate': uri(plate()),
        '--art-plate-lit': uri(recolour(plate(), FACE, (0x1b, 0x36, 0x31, 255))),
        '--art-slot': uri(slot('ready')),
        '--art-slot-cold': uri(slot('cooldown')),
        '--art-slot-lit': uri(slot('pressed')),
        '--art-well': uri(recolour(slot('cooldown'), FACE, WELL)),
        '--art-well-lit': uri(recolour(slot('ready'), FACE, WELL)),
        '--art-gem': uri(kit('status')),
    }
    lines = [
        '/* Generated by art/frames.py from the Delve\'s HUD kit',
        '   (crates/delve/assets/art-slots/dgn.hud.chrome/v1): do not edit. */',
        ':root {',
    ]
    for k, v in props.items():
        lines.append(f'  {k}: {v};')
    lines.append('}')
    out = os.path.join(HERE, '..', 'skin.css')
    with open(out, 'w') as f:
        f.write('\n'.join(lines) + '\n')
    print(out, os.path.getsize(out), 'bytes')


if __name__ == '__main__':
    main()
