#!/usr/bin/env python3
"""Prepare Griffinwing's eight art deliveries for scripts/slot-art-import.py.

The deliveries carry no measured crops (no source-sheet.json), and two of them
need a pick: the sky's kept sheet is source-sheet-2.png, the canyon textures
are six picked swatches from v01/picked. This writes, per slot, a folder that
holds the delivery's own bytes (the importer reads source-sheet.png and
notes.md) and the measured crops the importer wants. Nothing is repainted,
filtered or resampled: a prepared sheet is the delivery's pixels, placed.

  python3 -I prepare-art.py [--delivery DIR] [--output DIR] [slot ...]

then, per slot (the importer's status.json path is fixed, so always
--source-dir):

  python3 -I scripts/slot-art-import.py griffinwing.<slot> --source-dir <output>/griffinwing.<slot>

Crops: the sky's four bands are found by magenta detection (rows and columns
with more than half non-magenta pixels, trimmed 2 px); the sheets with
real alpha are cut by the components of their alpha >= 128 mask, with a dilation
that joins a drawing's own flecks (rows of the grid left to right, top to
bottom); the six texture swatches are pasted into one 3 x 2 sheet unscaled.
"""
import argparse
import hashlib
import json
import shutil
from collections import deque
from pathlib import Path

from PIL import Image, ImageFilter

DELIVERY = Path('/work/artifacts/art-prod-rt/slots')
OUTPUT = Path('/work/artifacts/monolith-private/art-import-gw/sources')
SLOTS = ['sky', 'canyon-tex', 'arch', 'ridge', 'shutter', 'beacon', 'gate', 'hearth-ring']
COPIES = ['notes.md', 'prompt.txt', 'generation.json', 'PRODUCER-NOTE.md']
SWATCHES = ['rock', 'stone', 'causeway', 'bone', 'lava', 'mist']


def is_magenta(c):
    return c[0] > 200 and c[2] > 200 and c[1] < 90


def runs(flags):
    out, start = [], None
    for i, on in enumerate(list(flags) + [False]):
        if on and start is None:
            start = i
        if not on and start is not None:
            out.append((start, i))
            start = None
    return out


def sky_pieces(sheet):
    """Four bands on flat magenta: rows and columns that are mostly picture."""
    rgb = sheet.convert('RGB')
    px = rgb.load()
    w, h = rgb.size
    rows = [sum(not is_magenta(px[x, y]) for x in range(w)) > w / 2 for y in range(h)]
    pieces = []
    for y0, y1 in runs(rows):
        cols = [sum(not is_magenta(px[x, y]) for y in range(y0, y1)) > (y1 - y0) / 2 for x in range(w)]
        (x0, x1), = runs(cols)
        # Trim two pixels off the magenta fringe, then check no magenta is left.
        x0, y0, x1, y1 = x0 + 2, y0 + 2, x1 - 2, y1 - 2
        for x in range(x0, x1):
            assert not is_magenta(px[x, y0]) and not is_magenta(px[x, y1 - 1]), 'magenta left on a band edge'
        for y in range(y0, y1):
            assert not is_magenta(px[x0, y]) and not is_magenta(px[x1 - 1, y]), 'magenta left on a band edge'
        pieces.append((x0, y0, x1 - x0, y1 - y0, (x1 - x0) * (y1 - y0)))
    assert len(pieces) == 4, pieces
    return pieces


def components(sheet, dilate, floor=30):
    """Boxes (x, y, w, h, area) of the alpha >= 128 mask's components, with
    flecks within `dilate` px of a drawing joined to it."""
    alpha = sheet.getchannel('A').point(lambda v: 255 if v >= 128 else 0)
    near = alpha.filter(ImageFilter.MaxFilter(dilate))
    w, h = sheet.size
    wide, solid = near.load(), alpha.load()
    seen = bytearray(w * h)
    out = []
    for y in range(h):
        for x in range(w):
            if not wide[x, y] or seen[y * w + x]:
                continue
            todo = deque([(x, y)])
            seen[y * w + x] = 1
            x0 = x1 = x
            y0 = y1 = y
            area = 0
            while todo:
                cx, cy = todo.popleft()
                if solid[cx, cy]:
                    area += 1
                    x0, x1, y0, y1 = min(x0, cx), max(x1, cx), min(y0, cy), max(y1, cy)
                for nx, ny in ((cx + 1, cy), (cx - 1, cy), (cx, cy + 1), (cx, cy - 1)):
                    if 0 <= nx < w and 0 <= ny < h and wide[nx, ny] and not seen[ny * w + nx]:
                        seen[ny * w + nx] = 1
                        todo.append((nx, ny))
            if area >= floor:
                out.append([x0, y0, x1 + 1, y1 + 1, area])
    return out


def merge(boxes, cells):
    """Boxes grouped by the 3 x 3 grid cell their centre falls in, united."""
    sheet_w, sheet_h = cells
    groups = {}
    for x0, y0, x1, y1, area in boxes:
        key = (int((y0 + y1) / 2 * 3 / sheet_h), int((x0 + x1) / 2 * 3 / sheet_w))
        g = groups.setdefault(key, [x0, y0, x1, y1, 0])
        g[0], g[1], g[2], g[3] = min(g[0], x0), min(g[1], y0), max(g[2], x1), max(g[3], y1)
        g[4] += area
    return [(x0, y0, x1 - x0, y1 - y0, area) for _, (x0, y0, x1, y1, area) in sorted(groups.items())]


def alpha_pieces(slot, sheet):
    w, h = sheet.size
    if slot in ('arch', 'ridge'):
        (x0, y0, x1, y1, area), = components(sheet, 31)
        return [(x0, y0, x1 - x0, y1 - y0, area)]
    if slot in ('shutter', 'beacon'):
        boxes = sorted(components(sheet, 31))
        assert len(boxes) == 5, boxes
        return [(x0, y0, x1 - x0, y1 - y0, a) for x0, y0, x1, y1, a in boxes]
    pieces = merge(components(sheet, 15), (w, h))
    assert len(pieces) == 9, pieces
    return pieces


def canyon_sheet(delivery):
    """The six picked swatches on one sheet, unscaled, in the slot's order."""
    picked = delivery / 'picked'
    images = [Image.open(picked / f'{name}.png').convert('RGB') for name in SWATCHES]
    cell = max(max(im.size) for im in images) + 8
    sheet = Image.new('RGB', (cell * 3, cell * 2), (255, 0, 255))
    pieces = []
    for i, im in enumerate(images):
        x, y = (i % 3) * cell + 4, (i // 3) * cell + 4
        sheet.paste(im, (x, y))
        pieces.append((x, y, im.width, im.height, im.width * im.height))
    return sheet, pieces


def prepare(slot, delivery_root, output_root):
    delivery = delivery_root / f'griffinwing.{slot}' / 'v01'
    out = output_root / f'griffinwing.{slot}'
    out.mkdir(parents=True, exist_ok=True)
    for name in COPIES:
        if (delivery / name).exists():
            shutil.copyfile(delivery / name, out / name)
    if slot == 'canyon-tex':
        sheet, pieces = canyon_sheet(delivery)
        sheet.save(out / 'source-sheet.png')
    else:
        # The sky's kept sheet is the second image of its run.
        kept = delivery / ('source-sheet-2.png' if slot == 'sky' else 'source-sheet.png')
        shutil.copyfile(kept, out / 'source-sheet.png')
        sheet = Image.open(kept)
        pieces = sky_pieces(sheet) if slot == 'sky' else alpha_pieces(slot, sheet.convert('RGBA'))
    meta = {
        'alpha_threshold': 128,
        'file': str(out / 'source-sheet.png'),
        'piece_count': len(pieces),
        'pieces': [
            {'alpha_area': area, 'index': i, 'source_rect_xywh': [x, y, w, h]}
            for i, (x, y, w, h, area) in enumerate(pieces)
        ],
        'size_wh': list(sheet.size),
        'warnings': [],
    }
    (out / 'source-sheet.json').write_text(json.dumps(meta, indent=1) + '\n')
    sha = hashlib.sha256((out / 'source-sheet.png').read_bytes()).hexdigest()
    print(f'{slot}: {len(pieces)} pieces, {sheet.size}, sheet sha256 {sha[:12]}')


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--delivery', type=Path, default=DELIVERY)
    p.add_argument('--output', type=Path, default=OUTPUT)
    p.add_argument('slots', nargs='*', default=SLOTS)
    a = p.parse_args()
    for slot in a.slots:
        prepare(slot, a.delivery, a.output)


if __name__ == '__main__':
    main()
