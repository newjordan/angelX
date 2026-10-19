#!/usr/bin/env python3
"""Prepare the beacon delivery for the unchanged importer: one tower, seven fires.

The delivered sheet draws the tower again in every cell and its stone texture
drifts from cell to cell (mean channel difference about 17/255 between cells,
position within two source pixels). Imported as it is, the tower would
shimmer between frames. This recipe locks the tower from the first cell (cold:
tower, bowl, ash and coals) and lays each other cell's own fire over it. Every
pixel of a prepared cell is a pixel of cell 1 or of that cell's own fire; no
pixel is painted or filtered. The fire is the cell's warm saturated pixels, the
dark edge about them, and nothing else that differs from the tower.

    python3 -I prepare.py --output DIR [--check]

DIR gets source-sheet.png, source-sheet.json (measured crops of the prepared
sheet: seven identical windows so one scale and one pixel grid register the
tower the same in every frame) and notes.md. The original delivery is never
written; its SHA-256 is checked first. Prepared sheets stay outside git.
"""
import argparse, hashlib, io, json
from pathlib import Path
from PIL import Image, ImageChops, ImageFilter

SOURCE = Path('/work/artifacts/art-prod-rt/slots/lanternward.beacon/v01')
SOURCE_SHA256 = '8fcf7a94f0a98977638482d6de3140adb2d4c2289f835213d2327252cb420fda'
CANVAS = (320, 520)        # tower space: cell 1 pasted with its base centre at (160, 500)
PIVOT = (160, 500)
SEARCH = 3                 # alignment search, source pixels either way
DILATE = 3                 # reach of the fire's own dark edge, source pixels
DIFFER = 90                # a non-fire pixel joins the fire only if it differs from the tower this much
NAMES = ['cold-0', 'kindling-0', 'kindling-1', 'lit-0', 'lit-1', 'lit-2', 'lit-3']


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def is_fire(c):
    r, g, b, a = c
    return a >= 128 and r >= 205 and b <= 130 and g >= 60 and r - b >= 110


def base_centre(sheet, rect):
    """Centre of the lowest band of the cell's alpha: the tower's base."""
    x, y, w, h = rect
    a = sheet.crop((x, y, x + w, y + h)).getchannel('A')
    band = max(2, h // 12)
    xs = [xx for yy in range(h - band, h) for xx in range(w) if a.getpixel((xx, yy)) >= 128]
    return (min(xs) + max(xs) + 1) / 2


def place(sheet, rect, centre, dx=0, dy=0):
    x, y, w, h = rect
    out = Image.new('RGBA', CANVAS)
    out.paste(sheet.crop((x, y, x + w, y + h)), (round(PIVOT[0] - centre) + dx, PIVOT[1] - h + dy))
    return out


def lower(im):
    return im.crop((60, 330, 260, 500)).convert('RGB')


def mean_diff(a, b):
    stat = ImageChops.difference(a, b).convert('L')
    return sum(i * n for i, n in enumerate(stat.histogram())) / (stat.width * stat.height)


def mask_where(im, test):
    out = Image.new('L', im.size)
    px, mp = im.load(), out.load()
    for y in range(im.height):
        for x in range(im.width):
            if test(px[x, y]):
                mp[x, y] = 255
    return out


def build(sheet, rects):
    centres = [base_centre(sheet, r) for r in rects]
    tower = place(sheet, rects[0], centres[0])
    cells, shifts = [tower], [(0, 0)]
    tower_rgb = Image.new('RGBA', CANVAS, (0, 0, 0, 255))
    tower_rgb.alpha_composite(tower)
    for i in range(1, 7):
        best = min(((mean_diff(lower(tower), lower(place(sheet, rects[i], centres[i], dx, dy))), dx, dy)
                    for dx in range(-SEARCH, SEARCH + 1) for dy in range(-SEARCH, SEARCH + 1)))
        shifts.append(best[1:])
        cell = place(sheet, rects[i], centres[i], *best[1:])
        fire = mask_where(cell, is_fire)
        reach = fire.filter(ImageFilter.MaxFilter(2 * DILATE + 1))
        cell_rgb = Image.new('RGBA', CANVAS, (0, 0, 0, 255))
        cell_rgb.alpha_composite(cell)
        differs = ImageChops.difference(cell_rgb.convert('RGB'), tower_rgb.convert('RGB'))
        r, g, b = differs.split()
        far = ImageChops.lighter(ImageChops.lighter(r, g), b).point(lambda v: 255 if v > DIFFER else 0)
        keep = ImageChops.lighter(fire, ImageChops.multiply(reach, far))
        # Where the cell has no pixel at all there is nothing of its own to lay.
        keep = ImageChops.multiply(keep, cell.getchannel('A').point(lambda v: 255 if v else 0))
        over = Image.new('RGBA', CANVAS)
        over.paste(cell, (0, 0), keep)
        composite = tower.copy()
        composite.alpha_composite(over)
        cells.append(composite)
    return cells, shifts, centres


def prepare():
    sheet = Image.open(SOURCE / 'source-sheet.png').convert('RGBA')
    meta = json.loads((SOURCE / 'source-sheet.json').read_text())
    rects = [p['source_rect_xywh'] for p in sorted(meta['pieces'], key=lambda p: p['index'])]
    cells, shifts, centres = build(sheet, rects)
    boxes = [c.getchannel('A').point(lambda a: 255 if a >= 128 else 0).getbbox() for c in cells]
    left, top = min(b[0] for b in boxes), min(b[1] for b in boxes)
    right, bottom = max(b[2] for b in boxes), max(b[3] for b in boxes)
    wide, high = right - left, bottom - top
    gutter = 24
    out = Image.new('RGBA', (4 * (wide + gutter) + gutter, 2 * (high + gutter) + gutter))
    pieces = []
    for i, cell in enumerate(cells):
        x = gutter + (i % 4) * (wide + gutter)
        y = gutter + (i // 4) * (high + gutter)
        out.paste(cell.crop((left, top, right, bottom)), (x, y))
        window = out.crop((x, y, x + wide, y + high)).getchannel('A')
        area = sum(v >= 128 for v in window.get_flattened_data())
        pieces.append({'index': i, 'source_rect_xywh': [x, y, wide, high], 'alpha_area': area})
    notes = ['# Prepared beacon sheet', '', 'Derived from the delivered sheet by prepare.py (tower locked to the first cell, each cell\'s own fire laid over it). Reading order:']
    notes += [f'{i + 1}. {n}' for i, n in enumerate(NAMES)]
    metadata = {'file': 'source-sheet.png', 'size_wh': list(out.size), 'alpha_threshold': 128, 'warnings': [],
                'piece_count': 7, 'prepared_from_sha256': SOURCE_SHA256, 'pivot_xy_in_window': [PIVOT[0] - left, PIVOT[1] - top],
                'alignment_shift_xy': shifts, 'pieces': pieces}
    return out, metadata, '\n'.join(notes) + '\n'


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--check', action='store_true')
    a = p.parse_args()
    assert digest(SOURCE / 'source-sheet.png') == SOURCE_SHA256, 'delivery changed; review before preparing'
    sheet, metadata, notes = prepare()
    buf = io.BytesIO(); sheet.save(buf, format='PNG')
    files = {'source-sheet.png': buf.getvalue(),
             'source-sheet.json': (json.dumps(metadata, indent=2, sort_keys=True) + '\n').encode(),
             'notes.md': notes.encode()}
    for name, data in files.items():
        path = a.output / name
        if a.check:
            assert path.read_bytes() == data, f'stale: {path}'
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
    print('prepared sha256', hashlib.sha256(files['source-sheet.png']).hexdigest())


if __name__ == '__main__':
    main()
