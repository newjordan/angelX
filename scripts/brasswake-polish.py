#!/usr/bin/env python3
"""Flatten the original character ramps and finish the larger keeper at native grain.

python scripts/brasswake-polish.py KEEPER_SOURCE --review REVIEW_DIRECTORY
The original twelve poses come from bdebb771; the keeper source is a 4x3 sheet.
No interpolation, runtime resizing, palette expansion or gameplay edits.
"""
from pathlib import Path
from collections import Counter
from io import BytesIO
import argparse
import importlib.util
import json
import subprocess
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / 'crates/delve/assets/brasswake'


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + '.py'))
    out = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(out)
    return out


boss = module('boss-art')
brass = module('brasswake-art')
INK = boss.palette()
# Each material has exactly four authored levels, cool contour to warm light.
# Use the realm's actual pigment names rather than approximate duplicated RGBs.
REVERSE = {v: k for k, v in INK.items()}
RAMPS = {name: ''.join(REVERSE[c] for c in colours) for name, colours in {
    'copper': [(69, 39, 18), (120, 70, 32), (169, 118, 57), (216, 169, 94)],
    'slate': [(34, 48, 58), (47, 74, 82), (110, 138, 142), (143, 163, 168)],
    'steel': [(34, 48, 58), (69, 96, 112), (110, 138, 142), (179, 194, 196)],
    'cloth': [(69, 39, 18), (101, 44, 16), (168, 52, 31), (169, 118, 57)],
}.items()}


def old(name):
    path = 'crates/delve/assets/brasswake/' + name
    return Image.open(BytesIO(subprocess.check_output(['git', 'show', 'bdebb771:' + path], cwd=ROOT))).convert('RGBA')


def finish(im, ramps):
    inks = {k: INK[k] for ramp in ramps for k in ramp}
    _, rows = boss.snap(im, inks)
    while True:
        clean = boss.despeckle(rows)
        if clean == rows:
            break
        rows = clean
    boss.gate(rows, inks)
    return boss.render(rows, inks)


def flatten(im, ramps):
    """Merge 1–3 px tone islands while preserving the native alpha silhouette."""
    inks = {k: INK[k] for ramp in ramps for k in ramp}
    _, rows = boss.snap(im, inks)
    cells = [list(row) for row in rows]
    while True:
        small = []
        for k in inks:
            points = {(x, y) for y, row in enumerate(cells) for x, c in enumerate(row) if c == k}
            for part in boss.components(points):
                if len(part) <= 3:
                    small.append((k, part))
        changed = False
        for k, part in small:
            adjacent = Counter(cells[y][x] for px, py in part for x, y in boss.neighbours(px, py)
                               if 0 <= x < im.width and 0 <= y < im.height and (x, y) not in part
                               and cells[y][x] not in ('.', k))
            if adjacent:
                ramp = next(r for r in ramps if k in r)
                same = {c: n for c, n in adjacent.items() if c in ramp}
                choices = same or adjacent
                colour = max(choices, key=lambda c: (choices[c], -abs(boss.lightness(INK[c]) - boss.lightness(INK[k]))))
                for x, y in part:
                    cells[y][x] = colour
                changed = True
        if not changed:
            break
    rows = boss.contour([''.join(row) for row in cells], ramps)
    while True:
        clean = boss.despeckle(rows)
        if clean == rows:
            break
        rows = clean
    boss.gate(rows, inks)
    return boss.render(rows, inks)


def lenses(im, centres, dim=False, cracked=False):
    """Two 3x2 curved lights with a one-pixel falloff, placed after cleanup."""
    lights = []
    for i, (cx, cy) in enumerate(centres):
        core = {(cx, cy), (cx + 1, cy), (cx - 1, cy + 1), (cx, cy + 1), (cx + 1, cy + 1)}
        if cracked and i == 0:
            core -= {(cx, cy), (cx, cy + 1), (cx - 1, cy + 1)}
        fall = {p for q in core for p in boss.neighbours(*q)} - core
        for x, y in {(x, y) for x in range(cx - 2, cx + 3) for y in range(cy - 1, cy + 3)}:
            if im.getpixel((x, y))[3]:
                im.putpixel((x, y), INK[RAMPS['slate'][0]] + (255,))
        for points, k in [(fall, RAMPS['slate'][1]), (core, RAMPS['slate'][2 if dim else 3])]:
            for x, y in points:
                if im.getpixel((x, y))[3]:
                    im.putpixel((x, y), INK[k] + (255,))
        if cracked and i == 0:
            for dy in [-1, 0, 1, 2]:
                im.putpixel((cx, cy + dy), INK[RAMPS['slate'][0]] + (255,))
        lights.append({'name': 'left lens' if i == 0 else 'right lens',
                       'pixels': [[x, y, RAMPS['slate'][2 if dim else 3]] for x, y in sorted(core)],
                       'falloff': [[x, y, RAMPS['slate'][1]] for x, y in sorted(fall)],
                       'hues': {k: '#%02x%02x%02x' % INK[k] for k in RAMPS['slate']}})
    return lights


def keeper(source):
    tiles = []
    centres = []
    for n in range(12):
        x, y = n % 4 * source.width // 4, n // 4 * source.height // 3
        tile = brass.components(boss.extract(source.crop((x, y, x + source.width // 4, y + source.height // 3)), 'neutral', 65))
        # Source eye colours locate the face; their texture is discarded below.
        bright = {(xx, yy) for yy in range(tile.height) for xx in range(tile.width)
                  if (lambda c: c[3] and c[1] > 105 and c[0] < c[1] * .76 and c[2] > c[0] * 1.15)(tile.getpixel((xx, yy)))}
        parts = sorted(boss.components(bright), key=len, reverse=True)[:2]
        eye = sorted([(sum(x for x, _ in p) / len(p), sum(y for _, y in p) / len(p)) for p in parts])
        box = tile.getbbox()
        tiles.append(tile.crop(box))
        centres.append([(cx - box[0], cy - box[1]) for cx, cy in eye])
    factor = min(78 / max(t.width for t in tiles), 78 / max(t.height for t in tiles))
    result = []
    for n, tile in enumerate(tiles):
        tile = tile.resize((round(tile.width * factor), round(tile.height * factor)), Image.Resampling.NEAREST)
        x, y = (80 - tile.width) // 2, 80 - tile.height
        im = Image.new('RGBA', (80, 80))
        im.alpha_composite(tile, (x, y))
        for yy in range(80):
            for xx in range(80):
                r, g, b, a = im.getpixel((xx, yy))
                if a and b > r and g > r:
                    im.putpixel((xx, yy), (round(r * .45), round(g * .45), round(b * .45), a))
        im = flatten(im, [RAMPS['copper'], RAMPS['slate']])
        if n in [8, 9]:
            for yy in range(40, 65):
                for xx in range(23, 56):
                    if im.getpixel((xx, yy)) == INK[RAMPS['copper'][0]] + (255,):
                        im.putpixel((xx, yy), INK[RAMPS['slate'][0]] + (255,))
        if n in [7, 10]:
            for yy in range(40, 64):
                for xx in range(26, 53):
                    if im.getpixel((xx, yy)) == INK[RAMPS['copper'][0]] + (255,):
                        im.putpixel((xx, yy), INK[RAMPS['copper'][1]] + (255,))
        eye = [(round(cx * factor) + x, round(cy * factor) + y) for cx, cy in centres[n]]
        # Hand-shaped belly lights replace the last fragmented rivet clusters.
        d = ImageDraw.Draw(im)
        if n in [0, 1, 2, 8, 9]:
            d.polygon([(28, 43), (35, 41), (39, 42), (37, 47), (33, 49), (29, 46)], fill=INK[RAMPS['copper'][3]] + (255,))
        signature = lenses(im, eye, dim=n == 10, cracked=n == 9)
        im = finish(im, [RAMPS['copper'], RAMPS['slate']])
        for light in signature:
            light['falloff'] = [[x, y, k] for x, y, k in light['falloff'] if im.getpixel((x, y)) == INK[k] + (255,)]
        result.append((brass.STATES[n], im, eye, signature))
    return result


def porter(im, n, eye):
    im = flatten(im, [RAMPS['copper'], RAMPS['slate']])
    cx, cy = eye
    d = ImageDraw.Draw(im)
    for y in range(cy + 5, min(cy + 19, 47)):
        for x in range(max(1, cx - 9), min(47, cx + 9)):
            if im.getpixel((x, y)) == INK[RAMPS['copper'][1]] + (255,) and all(im.getpixel(p)[3] for p in boss.neighbours(x, y)):
                im.putpixel((x, y), INK[RAMPS['copper'][2]] + (255,))
    # A stout keg apron with two broad hoops and a stave light gives the porter
    # a brewery trade, rather than an indistinct orange belly beneath a hood.
    if n not in [4, 6, 10]:
        for yy, width, colour in [(cy + 9, 14, RAMPS['slate'][1]), (cy + 17, 12, RAMPS['slate'][1])]:
            for y in range(yy, yy + 2):
                for x in range(cx - width // 2, cx + width // 2):
                    if 0 <= x < 48 and 0 <= y < 48 and im.getpixel((x, y))[3]:
                        im.putpixel((x, y), INK[colour] + (255,))
        for y in range(cy + 11, min(cy + 16, 48)):
            for x in range(cx - 3, cx):
                if im.getpixel((x, y))[3]:
                    im.putpixel((x, y), INK[RAMPS['copper'][3]] + (255,))
        # Cap brim makes the face's direction explicit at native size.
        d.line([(cx - 7, cy - 4), (cx + 6, cy - 4)], fill=INK[RAMPS['slate'][2]] + (255,), width=2)
    lenses(im, [(cx - 3, cy - 1), (cx + 3, cy - 1)], dim=n == 10)
    return finish(im, [RAMPS['copper'], RAMPS['slate']])


def effects():
    for n in range(4):
        # Full-width crowns meet the 48-pixel boss slam box without changing it.
        im = Image.new('RGBA', (48, 32))
        d = ImageDraw.Draw(im)
        y = 23 + n
        d.polygon([(0, y), (2, y - 9), (10, y - 3), (15, y - 13), (24, y - 4),
                   (33, y - 13), (39, y - 3), (46, y - 9), (47, y), (40, y + 4), (8, y + 4)], fill=INK[RAMPS['copper'][1]] + (255,))
        d.polygon([(2, y), (4, y - 5), (12, y - 1), (16, y - 8), (24, y - 1),
                   (32, y - 8), (38, y - 1), (44, y - 5), (45, y), (38, y + 2), (10, y + 2)], fill=INK[RAMPS['copper'][3]] + (255,))
        im.save(ASSETS / f'effects-slam-{n}.png')
        im = Image.new('RGBA', (32, 32))
        d = ImageDraw.Draw(im)
        d.polygon([(1, 16), (10, 8 + n), (29, 6 + n), (27, 15), (30, 24 - n), (11, 23 - n)], fill=INK[RAMPS['slate'][1]] + (255,))
        d.polygon([(3, 16), (12, 12), (26, 10 + n), (23, 16), (27, 21 - n), (12, 20)], fill=INK[RAMPS['slate'][2]] + (255,))
        im.save(ASSETS / f'effects-steam-{n}.png')


def knight(im):
    im = flatten(im, [RAMPS['steel'], RAMPS['cloth']])
    steel = {INK[k] + (255,) for k in RAMPS['steel']}
    points = {(x, y) for y in range(48) for x in range(48) if im.getpixel((x, y)) in steel}
    top = min(y for x, y in points)
    head = [(x, y) for x, y in points if y < top + 7]
    cx, cy = round(sum(x for x, y in head) / len(head)), top + 2
    light = {(cx - 2, cy), (cx - 1, cy), (cx, cy), (cx - 3, cy + 1), (cx - 2, cy + 1)} & points
    for x, y in light:
        if any(p in light for p in boss.neighbours(x, y)):
            im.putpixel((x, y), INK[RAMPS['steel'][3]] + (255,))
    return finish(im, [RAMPS['steel'], RAMPS['cloth']])


def run(source, review):
    review.mkdir(parents=True, exist_ok=True)
    anchors = json.loads(subprocess.check_output(['git', 'show', 'bdebb771:crates/delve/assets/brasswake/anchors.json'], cwd=ROOT))
    records = {}
    for kind, names in [('knight', brass.KNIGHTS), ('porter', brass.STATES)]:
        items = []
        for n, name in enumerate(names):
            im = old(f'{kind}-{name}.png')
            im = porter(im, n, anchors[kind][n]) if kind == 'porter' else knight(im)
            im.save(ASSETS / f'{kind}-{name}.png')
            items.append((name, im))
        for scale in [1, 3]:
            brass.sheet(items, review / f'{kind}-{scale}x.png', scale)
    items = []
    anchors['keeper'] = []
    for name, im, eye, lights in keeper(Image.open(source)):
        im.save(ASSETS / f'keeper-{name}.png')
        anchors['keeper'].append([round(sum(p[0] for p in eye) / 2), round(sum(p[1] for p in eye) / 2)])
        records[name] = lights
        items.append((name, im))
    for scale in [1, 3]:
        brass.sheet(items, review / f'keeper-{scale}x.png', scale)
    effects()
    for scale in [1, 3]:
        brass.sheet([(f'{kind}-{n}', Image.open(ASSETS / f'effects-{kind}-{n}.png'))
                     for kind in ['contact', 'slam', 'steam'] for n in range(4)], review / f'effects-{scale}x.png', scale)
    (ASSETS / 'anchors.json').write_text(json.dumps(anchors, indent=2) + '\n')
    (review / 'signature-lights.json').write_text(json.dumps(records, indent=2) + '\n')
    brass.audit(ASSETS, review)
    print('Native ramps:', RAMPS)
    for name, lights in records.items():
        im = Image.open(ASSETS / f'keeper-{name}.png')
        _, rows = boss.snap(im, {k: INK[k] for k in RAMPS['copper'] + RAMPS['slate']})
        errors = boss.signature_gate({'rows': rows, 'signature_lights': lights}, INK)
        assert not errors, (name, errors)
    print('Keeper signature gates: curved separate lights with one-pixel falloff.')


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('source', type=Path)
    p.add_argument('--review', type=Path, required=True)
    a = p.parse_args()
    run(a.source, a.review)
