#!/usr/bin/env python3
"""Native cleanup and retained nearest candidates for the pit/debt trio."""
import argparse
import json
import runpy
import subprocess
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
A = runpy.run_path(str(ROOT / 'scripts/boss-art.py'))
NAMES = ('red-pick-chief', 'the-foreman', 'late-fee-leviathan')
OUT = Path('/work/artifacts/boss-art-20261015/boss-4b')
PALS = [
    dict(s='#3f5a2a', S='#5f7f34', U='#86a24a', h='#b0c46a',
         o='#6e1f1c', O='#a8322a', t='#d0503a', T='#b83a2e', H='#8a96a0', n='#6b4a2a'),
    dict(s='#3f5a2a', S='#728e42', U='#9fb35d', h='#cbc2a5',
         o='#6a4426', O='#8a4a24', t='#6f8796', T='#93a9b6', H='#e8c040', n='#b83a2e'),
    dict(s='#265859', S='#387e76', U='#50a390', h='#cbc2a5',
         o='#694936', O='#83baa1', t='#cbbf98', T='#bb965b', H='#c8c040', n='#2a1418'),
]
EFFECT_PALS = [
    dict(s='#56554e', S='#726e64', U='#8a8478', h='#b4aa94',
         o='#80362a', O='#b83a2e', t='#c8642c', T='#e0a040', H='#8a96a0', n='#6b4a2a'),
    dict(s='#344f62', S='#577c90', U='#83a6b4', h='#a5bdc4',
         o='#344f62', O='#577c90', t='#83a6b4', T='#a5bdc4', H='#577c90', n='#344f62'),
    dict(s='#2b6f66', S='#3f9c8c', U='#62b39d', h='#83baa1',
         o='#694936', O='#9b6b40', t='#bb965b', T='#cbbf98', H='#83baa1', n='#265859'),
]


def put(im, x, y, k):
    if 0 <= x < im.width and 0 <= y < im.height:
        im.putpixel((x, y), ord(k))


def rect(im, box, k):
    ImageDraw.Draw(im).rectangle(box, fill=ord(k))


def line(im, points, k, width=1):
    ImageDraw.Draw(im).line(points, fill=ord(k), width=width)


def poly(im, points, k):
    ImageDraw.Draw(im).polygon(points, fill=ord(k))


def motif(im, x, y, rows):
    for dy, row in enumerate(rows):
        for dx, k in enumerate(row):
            if k != '.':
                put(im, x + dx, y + dy, k)


def original(i):
    p = f'cockpit/assets/dungeon/bosses/{NAMES[i]}.boss'
    return subprocess.check_output(['git', 'show', f'a13e2138:{p}'], cwd=ROOT, text=True).split('\nart\n')[1].splitlines()


def mask(i, scale=1):
    rows = original(i)
    im = Image.new('L', (len(rows[0]), len(rows)))
    for y, row in enumerate(rows):
        for x, k in enumerate(row):
            if k != '.':
                im.putpixel((x, y), 255)
    return im.resize((im.width * scale, im.height * scale), Image.Resampling.NEAREST)


def finish(im, i, alpha=None, details=()):
    rows = [''.join(chr(im.getpixel((x, y))) if (alpha.getpixel((x, y)) if alpha else im.getpixel((x, y)) != ord('.')) else '.'
                    for x in range(im.width)) for y in range(im.height)]
    keep = set(details)
    for y, row in enumerate(rows):
        for x, k in enumerate(row):
            if k != '.' and all(not (0 <= nx < im.width and 0 <= ny < im.height) or rows[ny][nx] == '.' for nx, ny in A['neighbours'](x, y)):
                keep.add((x, y))  # Inherited disconnected silhouette pixels.
    while True:
        clean = A['despeckle'](rows, keep)
        if clean == rows:
            break
        rows = clean
    return dict(rows=rows, pigments=PALS[i], hand_pixels=[list(q) for q in sorted(keep)])


def lights(spec, points, name):
    rows, pal = spec['rows'], spec['pigments']
    collar = {}
    pixels = [[x, y, rows[y][x]] for x, y in points]
    for x, y, k in pixels:
        neighbours = [(nx, ny) for nx, ny in A['neighbours'](x, y)
                      if 0 <= nx < len(rows[0]) and 0 <= ny < len(rows) and rows[ny][nx] != '.' and (nx, ny) not in points
                      and A['lightness'](bytes.fromhex(pal[rows[ny][nx]][1:])) <= A['lightness'](bytes.fromhex(pal[k][1:])) - 7.5]
        if not neighbours:
            continue
        q = neighbours[0]
        collar[q] = rows[q[1]][q[0]]
    hues = {k: pal[k] for k in sorted({p[2] for p in pixels} | set(collar.values()))}
    spec.setdefault('signature_lights', []).append(dict(name=name, pixels=pixels, falloff=[[x, y, k] for (x, y), k in collar.items()], hues=hues))
    spec['hand_pixels'] += [list(q) for q in points] + [list(q) for q in collar]


def red():
    im = Image.new('L', mask(0).size, ord('n'))
    # Steel spans the inherited pick head; the red grip stays inside its mask.
    line(im, [(15, 0), (23, 0), (25, 1)], 'H', 2)
    line(im, [(21, 2), (21, 20)], 'T', 2)
    rect(im, (20, 9, 21, 19), 'T')
    poly(im, [(10,0),(13,0),(16,2),(8,2)], 'O')
    rect(im, (8, 1, 16, 2), 'O')
    line(im, [(10, 1), (12, 1)], 't')
    line(im, [(8, 3), (16, 3)], 'o')
    rect(im, (8, 4, 16, 8), 'S')
    poly(im, [(9, 5), (12, 4), (15, 5), (15, 7), (12, 8), (9, 7)], 'U')
    line(im, [(9, 4), (11, 4)], 's'); line(im, [(14, 4), (16, 4)], 's')
    put(im, 10, 4, 't'); put(im, 15, 4, 't')
    line(im, [(10, 7), (10, 8)], 'h'); line(im, [(15, 7), (15, 8)], 'h')
    rect(im, (7, 9, 19, 19), 'n')
    poly(im, [(9, 9), (11, 9), (11, 13), (9, 15)], 'U')
    for points in [[(1, 10), (3, 9), (5, 12), (5, 18), (1, 18)],
                   [(18, 10), (21, 10), (23, 13), (23, 18), (19, 18)]]:
        poly(im, points, 'U')
    line(im, [(2, 11), (3, 12), (3, 15)], 'U', 2)
    line(im, [(22, 12), (22, 15)], 'U', 2)
    # Red remains a single diagonal colour block, with restrained cap-red rims.
    line(im, [(7, 9), (17, 19)], 't', 6)
    line(im, [(6, 10), (15, 19)], 'O', 1)
    rect(im, (0, 16, 4, 20), 'n')
    line(im, [(0, 18), (4, 18)], 'T')
    line(im, [(1, 16), (3, 16)], 'H')
    rect(im, (20, 9, 22, 20), 'T')
    line(im, [(21, 3), (21, 9)], 'T', 2)
    rect(im, (7, 20, 17, 20), 'n')
    rect(im, (7, 21, 10, 23), 'o'); rect(im, (14, 21, 17, 23), 'o')
    line(im, [(6, 22), (9, 22)], 'n'); line(im, [(14, 22), (17, 22)], 'n')
    return im


def foreman():
    im = Image.new('L', mask(1).size, ord('o'))
    poly(im, [(10, 0), (17, 0), (19, 3), (8, 3)], 't')
    poly(im, [(11, 0), (15, 0), (17, 2), (10, 2)], 'T')
    line(im, [(8, 3), (19, 3)], 't')
    rect(im, (5, 4, 23, 12), 'S')
    poly(im, [(7, 5), (19, 5), (22, 8), (19, 11), (8, 11), (5, 8)], 'S')
    poly(im, [(10, 5), (16, 5), (18, 8), (16, 9), (11, 9)], 'U')
    line(im, [(8, 6), (10, 6)], 's'); line(im, [(17, 6), (19, 6)], 's')
    line(im, [(8, 7), (9, 7)], 'n'); line(im, [(18, 7), (19, 7)], 'n')
    line(im, [(8, 10), (19, 10)], 'o', 2)
    motif(im, 9, 10, ['hh.hh..hh', '.h..h..h.'])
    rect(im, (6, 13, 21, 18), 'O')
    rect(im, (10, 13, 17, 16), 'S')
    line(im, [(7, 13), (8, 18)], 'O', 3); line(im, [(20, 13), (19, 18)], 'O', 3)
    line(im, [(8, 17), (19, 17)], 'o', 2)
    motif(im, 12, 16, ['TTT', 'ToT', 'TTT'])
    line(im, [(3, 16), (5, 17)], 'S', 3); line(im, [(22, 16), (24, 17)], 'S', 3)
    line(im, [(4, 17), (1, 19), (0, 20)], 'o', 2)
    line(im, [(24, 17), (25, 19), (27, 21)], 'o', 2)
    rect(im, (7, 19, 8, 21), 'O'); rect(im, (18, 19, 19, 21), 'O')
    rect(im, (6, 21, 8, 22), 'o'); rect(im, (18, 21, 20, 22), 'o')
    # Five pixels in a 3x2 lamp, with a cut upper corner instead of a rectangle.
    motif(im, 20, 3, ['tHH', 'HHH'])
    return im


def leviathan():
    im = Image.new('L', mask(2).size, ord('o'))
    # Rounded book-cover plates round the wide maw; no regular texture noise.
    rect(im, (2, 2, 24, 12), 'T')
    poly(im, [(7, 2), (18, 2), (23, 4), (22, 6), (4, 6), (4, 4)], 'T')
    line(im, [(6, 3), (19, 3)], 'T')
    line(im, [(3, 4), (3, 10)], 't', 2); line(im, [(23, 4), (23, 10)], 'o', 2)
    rect(im, (5, 5, 9, 6), 'n'); rect(im, (17, 5, 21, 6), 'n')
    motif(im, 6, 5, ['HT', 'Tn']); motif(im, 18, 5, ['HT', 'Tn'])
    rect(im, (5, 7, 23, 10), 'n')
    for x in [6, 10, 15, 20]:
        line(im, [(x, 7), (x, 8)], 'T')
    for x in [8, 13, 18, 22]:
        line(im, [(x, 9), (x, 10)], 'T')
    line(im, [(4, 11), (23, 11)], 'T')
    # Belly and arm curls read as one wet animal under the dry book shell.
    poly(im, [(8, 12), (20, 12), (23, 16), (20, 22), (8, 22), (5, 17)], 'S')
    poly(im, [(10, 13), (17, 13), (20, 15), (19, 18), (15, 20), (10, 18)], 'O')
    poly(im, [(12, 14), (15, 14), (16, 16), (14, 17), (12, 16)], 'U')
    line(im, [(8, 14), (7, 17), (9, 19)], 's', 2)
    poly(im, [(1, 12), (5, 12), (6, 16), (3, 19), (1, 18)], 'S')
    line(im, [(1, 13), (3, 14), (3, 16), (2, 17)], 'U', 2)
    poly(im, [(21, 12), (24, 11), (26, 14), (26, 17), (23, 18)], 'S')
    line(im, [(24, 13), (25, 14), (25, 16), (24, 16)], 'U', 2)
    for points in [[(8, 20), (7, 23), (6, 23)], [(18, 20), (19, 23), (21, 23)],
                   [(2, 24), (4, 23), (6, 25)], [(10, 25), (12, 23), (15, 25)], [(17, 24), (20, 24), (22, 25)]]:
        line(im, points, 'S', 3); line(im, points, 'U')
    # Four uninterrupted dark native rows retain the maw in actual cells;
    # upper fangs and three lower teeth sit outside its two dark cell windows.
    rect(im, (5, 7, 23, 12), 'n')
    for x in (6, 10, 15, 20): line(im, [(x, 7), (x, 8)], 'T')
    for x in (7, 15, 22): line(im, [(x, 11), (x, 12)], 'T')
    line(im, [(4, 13), (23, 13)], 'T')
    # Small dorsal curl; broad 1px native signature is shaped in the study.
    motif(im, 12, 0, ['SUUS', 'SUUS', 'SSSS'])
    # Right-hand retrieval anchor preserves the original loop and all prongs.
    line(im, [(29, 9), (29, 23)], 'T', 2)
    line(im, [(29, 12), (29, 22)], 'T')
    line(im, [(25, 12), (31, 12)], 't')
    line(im, [(26, 19), (26, 22), (29, 24), (31, 22), (31, 20)], 't', 2)
    put(im, 30, 9, 'T')
    line(im, [(28, 10), (28, 11)], 'T')
    for x, y in [(29, 9), (30, 10), (29, 11), (28, 10)]:
        put(im, x, y, 'T')
    # Eyes above a cream page stripe, then long ivory fangs into a dark gullet.
    rect(im, (4, 4, 23, 6), 'T')
    line(im, [(5, 6), (22, 6)], 't')
    line(im, [(3, 5), (3, 12)], 'o', 2)
    rect(im, (5, 7, 23, 14), 'n')
    motif(im, 6, 3, ['HHHH', 'HHH.']); motif(im, 18, 3, ['HHHH', '.HHH'])
    for x in (6, 21):
        motif(im, x-1, 7, ['hhh', 'hhh', '.h.', '.h.'])
    for x in (8, 15, 22):
        motif(im, x-1, 13, ['.h.', 'hhh'])
    line(im, [(4, 15), (23, 15)], 't')
    line(im, [(5, 11), (23, 11)], 'o')
    return im



def bodies(i):
    native = [red, foreman, leviathan][i]()
    specs = []
    for scale in (1, 2):
        im = native.resize((native.width * scale, native.height * scale), Image.Resampling.NEAREST)
        if i == 0:
            points = {(10*scale, 4*scale), (15*scale, 4*scale)}
        elif i == 1:
            points = {(x*scale+dx, y*scale+dy) for x,y in [(21,3),(22,3),(20,4),(21,4),(22,4)]
                      for dx in range(scale) for dy in range(scale)}
        else:
            points = {(x*scale+dx, y*scale+dy) for x,y in [(6,3),(7,3),(8,3),(9,3),(6,4),(7,4),(8,4),(18,3),(19,3),(20,3),(21,3),(19,4),(20,4),(21,4)]
                      for dx in range(scale) for dy in range(scale)}
        # Preserve the named eyes/teeth and the small page edge during cleanup.
        detail = set(points)
        for y in range(im.height):
            for x in range(im.width):
                if chr(im.getpixel((x,y))) in ('h','H'):
                    detail.add((x,y))
        spec = finish(im, i, mask(i, scale), detail)
        lights(spec, points, ['red powder-captain eyes', 'yellow hard-hat lamp', 'yellow-green eyes above the book maw'][i])
        if scale == 2:
            spec['separation'] = [
                dict(floor='#000000', head=[18,4,32,6], body=[4,22,8,28], room_size=[52,48], room_origin=[358,146], floor_sample=[400,210]),
                dict(floor='#000000', head=[20,0,32,4], body=[14,27,42,36], room_size=[58,48], room_origin=[356,146], floor_sample=[400,210]),
                dict(floor='#000000', head=[12,12,44,14], body=[14,32,18,40], room_size=[64,52], room_origin=[352,142], floor_sample=[400,210]),
            ][i]
        specs.append(spec)
    return specs


def effect(im, i, details=()):
    spec = finish(im, i, details=details)
    spec['pigments'] = EFFECT_PALS[i]
    return spec


def projectiles(i):
    im = Image.new('L', (24, 24), ord('.'))
    if i == 0:
        # A six-by-three cartridge mass, enlarged threefold inside the same canvas.
        cartridge = Image.new('L', (6, 3), ord('.'))
        motif(cartridge, 0, 0, ['OOOOOO', 'OtnttO', 'OOOOOO'])
        im.paste(cartridge.resize((18, 9), Image.Resampling.NEAREST), (3, 8))
        line(im, [(17,7),(19,4),(21,4)], 'n', 2)
        motif(im, 20, 2, ['TT', 'TT'])
        other = im.rotate(90, resample=Image.Resampling.NEAREST)
    elif i == 1:
        line(im, [(5, 21), (9, 18), (17, 16), (20, 12), (19, 5), (15, 2), (9, 3), (5, 7), (5, 12), (9, 14), (13, 12)], 'H', 2)
        line(im, [(5, 21), (9, 18), (17, 16), (19, 12), (18, 5), (15, 3), (9, 4), (6, 7), (6, 11), (9, 12), (12, 11)], 't', 1)
        other = Image.new('L', (24, 24), ord('.'))
        line(other, [(2, 20), (6, 16), (10, 11), (16, 8), (21, 8), (21, 3), (17, 1)], 'H', 2)
        line(other, [(2, 20), (6, 16), (10, 11), (16, 8), (20, 8), (20, 3), (17, 2)], 't', 1)
        poly(other, [(1, 19), (2, 18), (4, 19), (3, 21), (1, 21)], 't')
    else:
        line(im, [(12, 4), (12, 19)], 'O', 2)
        line(im, [(12, 5), (12, 18)], 'T', 1)
        line(im, [(3, 7), (3, 14), (6, 18), (12, 20), (19, 16), (20, 9)], 'O', 2)
        line(im, [(3, 8), (4, 13), (7, 16), (12, 18), (18, 15), (19, 10)], 't', 1)
        line(im, [(7, 6), (12, 2), (17, 6)], 'U', 2)
        other = im.rotate(90, resample=Image.Resampling.NEAREST)
    frames = [effect(x, i) for x in (im, other)]
    if i == 1:
        # The travelling lash stays below the face/lamp, even in overlapping volleys.
        for frame in frames:
            frame['pigments'] = {**frame['pigments'], 'H':'#344f62', 't':'#577c90'}
    return frames


def tells(i):
    frames = []
    for stage in range(4):
        im = Image.new('L', (11, 11), ord('.'))
        if i == 0:
            # Powder fuse jaws: each stage reaches the same true 5px radius.
            for x, y in [(5, 0), (0, 5), (10, 5), (5, 10)]:
                put(im, x, y, 't')
            if stage == 1:
                poly(im, [(3, 1), (5, 1), (7, 2), (5, 3), (3, 2)], 't')
                poly(im, [(3, 8), (5, 7), (7, 8), (5, 9), (3, 9)], 't')
            if stage == 2:
                rect(im, (1, 3, 2, 7), 't')
                line(im, [(9, 3), (9, 7)], 't')
            if stage == 3:
                # Pick head and red haft inside a rounded, fuse-tipped envelope.
                line(im, [(2,3),(4,2),(7,2),(8,3)], 'H', 2)
                line(im, [(6,3),(6,8)], 'O', 2)
                line(im, [(1,5),(2,8),(5,10),(8,8),(10,5)], 't')
                motif(im, 7, 1, ['T.', 'TT'])
        elif i == 1:
            # Independent bridle / lash silhouettes rotated on real rage rays.
            if stage == 0:
                motif(im, 4, 4, ['ttt', '.t.'])
            elif stage == 1:
                line(im, [(2, 3), (5, 2), (8, 3), (8, 5), (6, 6)], 't', 2)
            elif stage == 2:
                line(im, [(1, 2), (5, 1), (9, 3), (9, 6), (6, 8), (3, 7), (2, 5)], 't', 2)
            else:
                im = Image.new('L', (15, 15), ord('.'))
                line(im, [(0, 4), (3, 1), (10, 1), (14, 5), (13, 10), (9, 14), (3, 12), (0, 8), (0, 4)], 't', 3)
                line(im, [(4, 4), (9, 4), (11, 8), (7, 11), (4, 8), (4, 4)], 'O', 2)
            if stage < 3:
                larger = Image.new('L', (15, 15), ord('.')); larger.paste(im, (2, 2)); im = larger
        else:
            # Curled sea hooks are sampled across the true annulus / laser.
            paths = [[(4, 5), (6, 5)], [(3, 3), (6, 2), (8, 4), (7, 6)],
                     [(1, 3), (4, 1), (8, 2), (9, 5), (7, 8), (4, 7)],
                     [(0, 3), (3, 0), (8, 1), (10, 5), (8, 9), (4, 10), (1, 7)]]
            line(im, paths[stage], 'U', [2, 2, 3, 3][stage])
            if stage >= 2:
                line(im, [(3, 4), (5, 3), (7, 5), (5, 7)], 'h', 2)
        # Effect dots are authored meaningful boundaries, never body texture.
        details = [(x, y) for y in range(im.height) for x in range(im.width) if im.getpixel((x, y)) != ord('.')]
        frames.append(effect(im, i, details=details))
    return frames


def impacts(i):
    frames = []
    for stage in range(5):
        im = Image.new('L', (28, 28), ord('.'))
        if i == 0:
            # Detached puff contours grow, peak, split and drift; no shared base.
            boxes = [ [(2,19,5,22),(11,17,14,22),(23,20,26,23)],
                      [(0,14,5,22),(10,8,15,21),(22,14,27,22)],
                      [(0,9,5,20),(10,2,15,18),(22,8,27,21)],
                      [(0,5,6,12),(10,0,17,8),(22,4,27,12),(3,17,8,23),(19,18,24,24)],
                      [(0,2,4,6),(12,1,16,4),(24,0,27,5),(2,22,6,25),(22,23,26,26)] ][stage]
            for box in boxes:
                ImageDraw.Draw(im).rounded_rectangle(box, radius=2, fill=ord('U'))
                x0,y0,x1,y1 = box
                line(im, [(x0+2,y0),(x1-1,y0)], 'h')
            if stage <= 2:
                poly(im, [(12,22),(13,18-stage*2),(16,20-stage),(17,23),(15,25)], 't')
        elif i == 1:
            paths = [
                [[(10,22),(8,18),(10,15),(13,16)],[(17,21),(20,18),(19,13),(16,12)]],
                [[(6,24),(4,18),(5,11),(9,7),(12,8),(12,12)],[(21,24),(24,18),(23,10),(18,5),(15,7),(16,11)]],
                [[(3,25),(1,18),(2,8),(7,2),(11,3),(12,7),(10,9)],[(24,25),(27,17),(25,6),(20,0),(16,1),(15,5),(17,8)]],
                [[(0,15),(2,7),(6,2),(9,3)],[(12,8),(12,11)],[(27,13),(25,4),(21,1),(17,2)],[(16,9),(15,12)],[(2,22),(6,25)],[(22,24),(26,20)]],
                [[(0,5),(3,1),(6,2)],[(23,0),(26,4),(27,7)],[(0,23),(3,26),(6,26)],[(21,26),(25,23)]]
            ][stage]
            for path in paths:
                line(im, path, 'S', 3)
                line(im, [(x,y-1) for x,y in path], 'U', 1)
            if stage in (1,2):
                line(im, [(4,18),(8,17),(11,19)], 'U', 2)
                line(im, [(24,16),(20,15),(17,17)], 'U', 2)
        else:
            paths = [
                [[(6,25),(4,21),(5,17),(8,15),(11,17),(10,20)],[(20,25),(23,22),(22,19),(19,18),(17,20)]],
                [[(5,25),(2,20),(3,12),(7,8),(11,9),(12,13),(10,16)],[(22,25),(25,20),(24,14),(20,11),(16,13),(16,16)]],
                [[(5,25),(1,18),(2,7),(7,2),(11,3),(13,7),(11,11)],[(22,25),(26,19),(25,9),(21,5),(17,7),(16,11),(18,14)]],
                [[(1,16),(2,7),(6,3),(10,3),(12,6),(11,9)],[(26,16),(25,8),(21,5),(17,7),(16,10)],[(3,23),(6,26)],[(21,25),(25,22)]],
                [[(0,9),(2,4),(6,2),(9,4)],[(27,11),(25,6),(22,5),(19,7)],[(2,25),(5,27)],[(23,27),(27,24)]]
            ][stage]
            for path in paths:
                line(im, path, 'S', 4)
                line(im, [(x-1,y) for x,y in path], 'U', 2)
        frames.append(effect(im, i))
    return frames


def candidates(concepts, output):
    output.mkdir(parents=True, exist_ok=True)
    for i, name in enumerate(NAMES):
        for role, source, size in [('body', name, mask(i).size), ('impact', ['red-vfx', 'foreman-vfx', 'leviathan-vfx'][i], (28, 28))]:
            path = concepts / (source + '.png')
            if not path.exists():
                continue
            im = A['extract'](Image.open(path), 'neutral', 48)
            pal = {k: tuple(bytes.fromhex(v[1:])) for k, v in (PALS if role == 'body' else EFFECT_PALS)[i].items()}
            im, rows = A['snap'](A['fit'](im, size), pal)
            im.save(output / f'{name}-{role}.png')
            (output / f'{name}-{role}.art').write_text('\n'.join(rows) + '\n')


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--concepts', type=Path)
    p.add_argument('--write', action='store_true')
    args = p.parse_args()
    if args.concepts:
        candidates(args.concepts, args.concepts / 'nearest')
    entries = []
    for i, name in enumerate(NAMES):
        v1, v2 = bodies(i)
        text = (ROOT / 'cockpit/assets/dungeon/bosses' / (name + '.boss')).read_text().split('\ntext ')[1].split('\n')[0]
        props = [
            ['red mining pick', 'powder keg and bandolier', 'green orc face under red cap'],
            ['helmet lamp', 'pit-pony harness', 'two descending whips'],
            ['bookbound toothed maw', 'sea-green belly and curling tentacles', 'brass retrieval anchor'],
        ][i]
        entry = dict(boss=f'cockpit/assets/dungeon/bosses/{name}.boss', output=f'cockpit/assets/dungeon/bosses/{name}', inks='sSUhoOtTHn',
                     frames=dict(v1=[v1], v2=[v2], projectile=projectiles(i), tell=tells(i), impact=impacts(i)),
                     review=dict(theme='Original pit/debt character; named props checked in both terminal kits.', originality='No commercial likeness; original silhouettes and material design.',
                                 identity_props=dict(props=props, source_text=text, compared_original_1x=False, compared_original_cells=False, inspected_cells_2x=False,
                                                     evidence=[str(OUT / 'review' / f'{name}-v{k}.png') for k in (1, 2)])))
        entries.append(entry)
    target = ROOT / 'scripts/boss-art-pit-debt.json'
    if target.exists():
        previous = json.loads(target.read_text())
        for entry, old in zip(entries, previous):
            entry['review']['identity_props'] = old['review']['identity_props']
    A['write_changed'](target, (json.dumps(entries, indent=2) + '\n').encode())
    if args.write:
        A['native_recipe'](entries)


if __name__ == '__main__':
    main()
