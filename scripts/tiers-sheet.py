#!/usr/bin/env python3
"""Lay out the shots of `delve_tiers` (docs/WORLD-TIERS.md): `contact.png`, a
row a room and a column a rung, labelled with the rung's dials, and `detail.png`,
one patch of every room at every rung at the shots' own size, for the grain.

    python3 scripts/tiers-sheet.py <dir> [--cell 520] [--detail x,y,w,h]
"""
import argparse
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

RUNGS = [
    ('r0', 'R0 flat pixel', 'tilt 0, half the grain (192 x 112)'),
    ('r1', 'R1 tilted cards', 'tilt 0.3, 384 x 224, the art as it is'),
    ('r2', 'R2 first light', 'blocks, a low sun and shadows, 4 colours'),
    ('r3', 'R3 the monolith', '8 colours, lamps, the ember ramp'),
    ('r4', 'R4 image-like', '768 x 448, 16 colours, the finest weave'),
    ('r5', 'R5 painted plate', 'past the climb: where a pack has the plate'),
]
# Rooms in the order the doc takes them; any other after.
ORDER = ['crypt-fight', 'crypt-passage', 'crypt-bone-chapel', 'crypt-open-floor']

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('directory', type=Path)
parser.add_argument('--cell', type=int, default=520, help='a shot as wide as this on the contact sheet')
parser.add_argument('--detail', default='600,380,480,300', help='the patch of each shot `detail.png` keeps, in the shots\' pixels')
args = parser.parse_args()
try:
    font = ImageFont.truetype('/usr/share/fonts/TTF/DejaVuSans.ttf', 13)
    bold = ImageFont.truetype('/usr/share/fonts/TTF/DejaVuSans-Bold.ttf', 14)
except OSError:
    font = bold = ImageFont.load_default()
ground, ink, dim = (12, 12, 14), (226, 226, 232), (150, 150, 162)

found = {p.name.rsplit('-r', 1)[0] for p in args.directory.glob('*-r[0-9].png')}
cases = sorted(found, key=lambda c: (ORDER.index(c) if c in ORDER else len(ORDER), c))
rungs = [r for r in RUNGS if any((args.directory / f'{c}-{r[0]}.png').exists() for c in cases)]
if not cases or not rungs:
    raise SystemExit(f'{args.directory}: no <case>-r0.png to r4.png shots')


def sheet(tile, label, cell_w):
    """Every case a row, every rung a column, each tile made by `tile`."""
    gap, head = 8, 42
    rows = []
    for case in cases:
        row = []
        for r in rungs:
            path = args.directory / f'{case}-{r[0]}.png'
            row.append(tile(Image.open(path).convert('RGB')) if path.exists() else None)
        rows.append((case, row))
    cell_h = max(t.height for _, row in rows for t in row if t)
    out = Image.new('RGB', (gap + len(rungs) * (cell_w + gap), head + len(rows) * (cell_h + 26 + gap)), ground)
    d = ImageDraw.Draw(out)
    for i, (_, name, dials) in enumerate(rungs):
        x = gap + i * (cell_w + gap)
        d.text((x, 6), name, font=bold, fill=ink)
        d.text((x, 24), dials, font=font, fill=dim)
    for j, (case, row) in enumerate(rows):
        y = head + j * (cell_h + 26 + gap)
        for i, im in enumerate(row):
            if im:
                out.paste(im, (gap + i * (cell_w + gap), y))
        d.text((gap, y + cell_h + 5), label(case), font=font, fill=dim)
    return out


contact = sheet(
    lambda im: im.resize((args.cell, round(im.height * args.cell / im.width)), Image.LANCZOS),
    lambda case: case,
    args.cell,
)
contact.save(args.directory / 'contact.png')
print(args.directory / 'contact.png', contact.size)

x, y, w, h = (int(v) for v in args.detail.split(','))
detail = sheet(lambda im: im.crop((x, y, x + w, y + h)), lambda case: f'{case}: {w} x {h} of the shot, as drawn', w)
detail.save(args.directory / 'detail.png')
print(args.directory / 'detail.png', detail.size)
