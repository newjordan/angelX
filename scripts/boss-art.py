#!/usr/bin/env python3
"""Convert native pixel concepts to checked boss grids and combat sprites.

Run with uv run --no-project --with Pillow scripts/boss-art.py --help.
Only nearest-neighbour sampling is used. No blur, antialiasing or dithering.
The palette is read from the game's ink function, never copied or expanded.
"""
import argparse
import io
import math
import statistics
import json
from collections import deque
from pathlib import Path
import re
import runpy
import subprocess
import sys
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]

PROMPT = ('{width} x {height} px sprite, finished pixel art AT THE REAL SPRITE SIZE, '
          '4-tone hue-shifted ramps, 1 px coloured outline, no dithering, '
          'flat neutral background. One original {subject}, front/three-quarter view. '
          'At most 10 inks, broad connected flat clusters, a large readable face, '
          'two or three body masses, a clear limb gap. No texture, speckle, '
          'antialiasing, black outline, pure white, text, floor or loose particles. '
          'One or two signature lights with locked hues. The theme must read at '
          '1x and in terminal cells; a clean generic sprite fails. No resemblance '
          'to a famous commercial character or franchise.')


def frame_inks(spec, inks):
    """Local pigments never edit the realm palette or the approved other kit."""
    colours = dict(inks)
    for k, value in spec.get('pigments', {}).items():
        colours[k] = tuple(bytes.fromhex(value.lstrip('#')))
    for light in spec.get('signature_lights', []):
        for k, value in light['hues'].items():
            locked = tuple(bytes.fromhex(value.lstrip('#')))
            assert k not in spec.get('pigments', {}) or colours[k] == locked
            colours[k] = locked
    return colours


def signature_rows(spec):
    """RULE: hand-place 1–2 named lights AFTER quantise/contour/despeckle.

    These explicit pixels and locked hues are never sampled or cleaned again.
    Theme at 1x / real cells and originality are mandatory human review gates.
    A clean but generic boss, or a familiar franchise likeness, fails.
    """
    cells = [list(row) for row in spec['rows']]
    for light in spec.get('signature_lights', []):
        assert light['name'] and light['pixels']
        for x, y, k in light['pixels'] + light.get('falloff', []):
            assert cells[y][x] != '.', 'a light must preserve the body silhouette'
            assert k in light['hues'], 'every signature ink needs a locked hue'
            cells[y][x] = k
    return [''.join(row) for row in cells]


def hand_pixels(spec):
    return spec.get('hand_pixels', []) + [p[:2] for light in spec.get('signature_lights', [])
                                        for p in light['pixels'] + light.get('falloff', [])]


def despeckle(rows, keep=()):
    """Replace singleton colour clusters with a connected neighbour's ink.

    The alpha silhouette is retained. Only explicit, reviewed eyes/highlights
    may be singleton inks; their coordinates belong in the saved recipe.
    """
    cells = [list(row) for row in rows]
    h, w = len(cells), len(cells[0])
    keep = set(map(tuple, keep))
    def around(x, y):
        return [(nx, ny) for nx, ny in ((x-1,y),(x+1,y),(x,y-1),(x,y+1))
                if 0 <= nx < w and 0 <= ny < h]
    for y in range(h):
        for x in range(w):
            k = cells[y][x]
            ns = around(x, y)
            if k == '.' or (x,y) in keep or any(cells[ny][nx] == k for nx,ny in ns):
                continue
            choices = [cells[ny][nx] for nx,ny in ns if cells[ny][nx] != '.'
                       and any(cells[ay][ax] == cells[ny][nx] for ax,ay in around(nx,ny))]
            if choices:
                cells[y][x] = max(dict.fromkeys(choices), key=choices.count)
            elif all(cells[ny][nx] == '.' for nx,ny in ns):
                cells[y][x] = '.'
    return [''.join(row) for row in cells]


def contour(rows, ramps):
    """One inner pixel, in the local ramp; lit upper edges use its next tone."""
    cells = [list(row) for row in rows]
    h, w = len(rows), len(rows[0])
    for y,row in enumerate(rows):
        for x,k in enumerate(row):
            if k == '.': continue
            edge = any(nx < 0 or ny < 0 or nx >= w or ny >= h or rows[ny][nx] == '.'
                       for nx,ny in ((x-1,y),(x+1,y),(x,y-1),(x,y+1)))
            if edge:
                ramp = next(r for r in ramps if k in r)
                cells[y][x] = ramp[1] if y > 0 and rows[y-1][x] == '.' and x < w//2 else ramp[0]
    return [''.join(row) for row in cells]


def gate(rows, inks, keep=()):
    assert len(inks) <= 10, 'a boss/effect may use at most ten inks'
    assert len(set(map(len,rows))) == 1, 'ragged native grid'
    allowed = set(map(tuple,keep))
    for y,row in enumerate(rows):
        for x,k in enumerate(row):
            if k == '.': continue
            assert k in inks and min(inks[k]) > 0 and max(inks[k]) <= 255
            assert inks[k] != (255, 255, 255), 'no pure-white ink'
            neighbours = [(nx,ny) for nx,ny in ((x-1,y),(x+1,y),(x,y-1),(x,y+1))
                          if 0 <= ny < len(rows) and 0 <= nx < len(row)]
            assert (x,y) in allowed or any(rows[ny][nx] == k for nx,ny in neighbours), f'unreviewed singleton {k} at {x},{y}'



# Two ramp steps: 7.5 CIE L* per step, measured before any zoom.
RAMP_STEP = 7.5
SEPARATION = 2 * RAMP_STEP


def lightness(rgb):
    linear = [v / 255 / 12.92 if v / 255 <= .04045 else
              ((v / 255 + .055) / 1.055) ** 2.4 for v in rgb[:3]]
    y = sum(v * w for v, w in zip(linear, (.2126, .7152, .0722)))
    return 116 * (y ** (1 / 3) if y > 216 / 24389 else
                  y * 24389 / 27 / 116 + 16 / 116) - 16


def neighbours(x, y):
    return ((x-1, y), (x+1, y), (x, y-1), (x, y+1))


def components(points):
    remaining = set(points)
    while remaining:
        part = {remaining.pop()}
        todo = list(part)
        while todo:
            for q in neighbours(*todo.pop()):
                if q in remaining:
                    remaining.remove(q)
                    part.add(q)
                    todo.append(q)
        yield part

def linear_pixels(features, rows):
    """Validate recipe-tagged strokes; short patches cannot claim this exemption."""
    from PIL import ImageDraw
    allowed = set()
    covered = set()
    for feature in reversed(features):
        assert feature['tag'] == 'LINEAR', 'narrow features need an explicit LINEAR tag'
        path, width = feature['path'], feature['width']
        assert 1 <= width <= 3 and len(path) >= 2, feature['name']
        length = sum(max(abs(b[0]-a[0]), abs(b[1]-a[1])) for a,b in zip(path,path[1:]))
        assert length >= 5, 'LINEAR strokes must be at least five native pixels long'
        mask = Image.new('1', (len(rows[0]),len(rows)))
        ImageDraw.Draw(mask).line([tuple(p) for p in path], fill=1, width=width)
        expected = {(x,y) for y in range(mask.height) for x in range(mask.width) if mask.getpixel((x,y))}
        points = {tuple(p) for p in feature['pixels']}
        assert points and points <= expected, feature['name']
        xs,ys=zip(*points)
        assert max(max(xs)-min(xs),max(ys)-min(ys)) >= 5, 'loose patches cannot claim a LINEAR exemption'
        if 'underlay' in feature:
            assert {tuple(p[:2]) for p in feature['underlay']} == points, feature['name']
        # The saved pixels can be clipped by later authored layers, never expanded.
        assert all(rows[y][x] == feature['ink'] for x,y in points-covered), feature['name']
        allowed.update(points)
        covered.update(points)
    return allowed


def region_errors(rows, ramps, min_side=4, linear_features=()):
    """Find material islands too narrow to hold a solid native tone mass.

    Explicit LINEAR recipes retain their narrow strokes and their saved
    underlay, so a hoop splitting a large mass cannot disguise new mottling.
    Untagged 2–3px patches and flecks still need an actual solid square.
    """
    allowed = linear_pixels(linear_features, rows)
    # Reconstruct broad masses below the explicitly named narrow overlays.
    cells = list(map(list, rows))
    for feature in reversed(linear_features):
        for x,y,k in feature.get('underlay', []):
            assert (x,y) in {tuple(p) for p in feature['pixels']}
            cells[y][x] = k
    rows = [''.join(r) for r in cells]
    for k in dict.fromkeys(''.join(ramps)):
        points = {(x,y) for y,row in enumerate(rows) for x,c in enumerate(row) if c == k}
        for part in components(points):
            if not part <= allowed and not any(all((x+dx,y+dy) in part for dx in range(min_side)
                           for dy in range(min_side)) for x,y in part):
                yield k, part

def region_gate(rows, ramps, min_side=4, linear_features=()):
    bad = list(region_errors(rows, ramps, min_side, linear_features))
    assert not bad, f'mottled material regions without {min_side}x{min_side} mass: ' + str([(k,len(p)) for k,p in bad])

def separation(im, floor, head, body):
    """All outer-ring pixels, and median material values in authored boxes.

    Boxes exclude signature lights and empty pixels. The median avoids a tiny
    eye passing a head whose broad material still merges into its torso.
    """
    solid = {(x,y) for y in range(im.height) for x in range(im.width)
             if im.getpixel((x,y))[3] > 127}
    ring = [lightness(im.getpixel(q)) - lightness(floor) for q in solid
            if any(n not in solid for n in neighbours(*q))]
    def material(box):
        x0,y0,x1,y1 = box
        values = [lightness(im.getpixel((x,y))) for x,y in solid
                  if x0 <= x < x1 and y0 <= y < y1]
        assert values, 'empty head/body material box'
        return statistics.median(values)
    return min(ring), abs(material(head) - material(body))


def signature_gate(spec, colours):
    errors = []
    rows = signature_rows(spec)
    for light in spec['signature_lights']:
        name = light['name']
        points = {(x,y) for x,y,_ in light['pixels']}
        # Check both the whole form and each ink cluster: a rounded outer
        # sticker cannot hide a rectangular bright screen inside it.
        shapes = [(True, part) for part in components(points)]
        for k in {k for _,_,k in light['pixels']}:
            shapes += [(False, part) for part in components({(x,y) for x,y,ink in light['pixels'] if ink == k})]
        for whole_form, part in shapes:
            xs,ys = zip(*part)
            x0,x1,y0,y1 = min(xs),max(xs),min(ys),max(ys)
            if max(x1-x0, y1-y0) >= 2 and min(x1-x0, y1-y0) >= (1 if whole_form else 2):
                rim = {(x,y) for x in range(x0,x1+1) for y in (y0,y1)}
                rim |= {(x,y) for y in range(y0,y1+1) for x in (x0,x1)}
                if rim <= part:
                    errors.append(f'{name}: straight four-sided rim {x1-x0+1}x{y1-y0+1}')
        collar = {(x,y):k for x,y,k in light.get('falloff', [])}
        if not collar:
            errors.append(f'{name}: missing 1px body falloff')
        for q,k in collar.items():
            assert q not in points, 'falloff belongs to neighbouring body pixels'
            assert any(n in points for n in neighbours(*q)), 'falloff must touch its light at 1px'
            adjacent = [lightness(colours[rows[y][x]]) for x,y in neighbours(*q) if (x,y) in points]
            if max(adjacent) - lightness(colours[k]) < RAMP_STEP:
                errors.append(f'{name}: falloff at {q} is less than one ramp step')
        for part in components(points):
            if not any(any(n in collar for n in neighbours(*q)) for q in part):
                errors.append(f'{name}: light component has no neighbouring body falloff')
    return errors


def numeric_gates(entries):
    errors = []
    all_inks = palette()
    for entry in entries:
        name = Path(entry['boss']).stem
        spec = entry['frames']['v2'][0]
        colours = frame_inks(spec, {k:all_inks[k] for k in entry['inks']})
        evidence = spec.get('separation')
        if evidence:
            floor = tuple(bytes.fromhex(evidence['floor'].lstrip('#')))
            edge, materials = separation(render(signature_rows(spec), colours), floor,
                                         evidence['head'], evidence['body'])
            print(f'{name}: native outer/floor {edge:.2f} L*, head/body {materials:.2f} L* (need >= {SEPARATION:g})')
            if edge + 1e-6 < SEPARATION: errors.append(f'{name}: outer ring merges with floor')
            if materials + 1e-6 < SEPARATION: errors.append(f'{name}: head/cap merges with body')
        else:
            errors.append(f'{name}: missing floor and head/body separation evidence')
        errors.extend(f'{name}: {e}' for e in signature_gate(spec, colours))
        for role in ('projectile','tell','impact'):
            for i,frame in enumerate(entry['frames'][role]):
                im = render(signature_rows(frame), frame_inks(frame, all_inks))
                near_white = {(x,y) for y in range(im.height) for x in range(im.width)
                              if im.getpixel((x,y))[3] and min(im.getpixel((x,y))[:3]) >= 240}
                clusters = [len(c) for c in components(near_white) if len(c) >= 2]
                print(f'{name}: {role} {i} hypercontrast clusters={clusters}')
                if clusters: errors.append(f'{name}: {role} {i} near-white cluster on black')
        tells = [render(signature_rows(f), frame_inks(f, all_inks)) for f in entry['frames']['tell']]
        for i,(before,after) in enumerate(zip(tells,tells[1:])):
            changed = sum(a != b for a,b in zip(before.get_flattened_data(),after.get_flattened_data()))
            print(f'{name}: authored tell {i}->{i+1} changed={changed}px')
            if not changed: errors.append(f'{name}: identical authored tell stages {i}/{i+1}')
    return errors


def identity_gates(entries):
    """Human checklist: metrics cannot approve a boss which lost its props.

    Untouched archived batches retain their recorded reviews. Changed boss
    assets and every new boss require the checklist; the identity-repair
    batch cannot fall back to the archived schema.
    """
    errors = []
    archived = {'cinderjaw', 'the-index', 'mother-of-spores',
                'waxen-warden', 'the-bone-choir', 'crypt-tithe-marshal'}
    changed = subprocess.check_output(
        ['git', 'diff', '--name-only', 'HEAD', '--', 'cockpit/assets/dungeon/bosses'],
        cwd=ROOT, text=True).splitlines()
    for entry in entries:
        name = Path(entry['boss']).stem
        record = entry.get('review', {}).get('identity_props')
        modified = any(p == entry['boss'] or p.startswith(entry['output']+'/') for p in changed)
        if record is None and name in archived and not modified:
            continue
        if not isinstance(record, dict):
            errors.append(f'{name}: missing identity-props checklist')
            continue
        props = record.get('props', [])
        if not 2 <= len(props) <= 3 or not all(isinstance(p, str) and p.strip() for p in props):
            errors.append(f'{name}: name 2-3 identity props before drawing')
        text = (ROOT/entry['boss']).read_text().split('\ntext ', 1)[1].split('\n', 1)[0]
        if record.get('source_text') != text:
            errors.append(f'{name}: checklist must use the unchanged .boss text')
        for field in ('compared_original_1x', 'compared_original_cells', 'inspected_cells_2x'):
            if record.get(field) is not True:
                errors.append(f'{name}: identity checklist incomplete: {field}')
        colour_fields = ('preserved_original_hues', 'preserved_original_value_contrast',
                         'judged_cells_2x_first', 'identity_colour_blocks_distinct')
        legacy_colour_reviews = archived | {'crypt-tax-keeper', 'crypt-relic-liege', 'iron-tusk-chief',
                                            'red-pick-chief', 'the-foreman', 'late-fee-leviathan'}
        if modified or name not in legacy_colour_reviews or any(field in record for field in colour_fields):
            for field in colour_fields:
                if record.get(field) is not True:
                    errors.append(f'{name}: colour identity checklist incomplete: {field}')
        evidence = record.get('evidence', [])
        if len(evidence) != 2 or not all(Path(p).is_file() for p in evidence):
            errors.append(f'{name}: identity review needs inspected evidence for both kits')
    return errors


ORIGINAL_REVISION = '7d711a03'


def original_rows(entry):
    """Read the immutable pre-pilot grid, including its original alpha."""
    raw = subprocess.check_output(
        ['git', 'show', f"{ORIGINAL_REVISION}:{entry['boss']}"], cwd=ROOT, text=True)
    rows = raw.split('\nart\n', 1)[1].splitlines()
    width = max(map(len, rows))
    return [row.ljust(width, '.') for row in rows]


def body_points(rows, size, origin):
    """Runtime integer-floor nearest sampling, at native room resolution."""
    w, h = size
    bx, by = origin
    return [(bx+x, by+y) for y in range(h) for x in range(w)
            if rows[y*len(rows)//h][x*len(rows[0])//w] != '.']


def terminal_colour(cell, dx, dy):
    """Decode the emitted glyph's 2x4 coverage, not its unused fg colour.

    A half block owns four subcells. Braille owns only its authored dot bits.
    This is a font-independent measurement of the actual cell buffer; each
    room pixel samples the subcell it occupies. HUD text in a body box fails.
    """
    glyph = cell['glyph']
    if glyph == '▀':
        foreground = dy < 2
    elif glyph == '▄':
        foreground = dy >= 2
    elif glyph == '█':
        foreground = True
    elif glyph in (' ', '\u2800'):
        foreground = False
    elif len(glyph) == 1 and 0x2800 <= ord(glyph) <= 0x28ff:
        foreground = bool((ord(glyph)-0x2800) & ((1,2,4,64),(8,16,32,128))[dx][dy])
    else:
        raise ValueError(f'non-world glyph {glyph!r} in body fixture')
    return cell['fg' if foreground else 'bg']


def presence(values, floor):
    assert values, 'empty body sample'
    return statistics.median(values), sum(v-lightness(floor) >= SEPARATION for v in values)


def presence_regressions(old, new):
    # No percentage allowance: even one lost body pixel fails. The epsilon
    # only accommodates arithmetic equality in the CIE conversion.
    return [label for label, a, b in zip(('median L*', 'presence pixels'), old, new)
            if b + 1e-6 < a]


def original_gates(entries, captures, output=None, contrast=None):
    """Library floor contrast; exact original-value gates for other bosses.

    Reference captures must come from the fixture's old-sprite fallback with
    grids exported from ORIGINAL_REVISION, not the previous art batch. All
    opaque material counts, including limbs and weapons; a small emitter
    cannot substitute for the body. Old/new use their own protected masks.
    """
    errors = []
    registry = json.loads((ROOT/'scripts/boss-library-registry.json').read_text())
    registry = {boss: policy for boss, policy in registry.items() if policy.get('active', True)}
    library = [entry for entry in entries if Path(entry['boss']).stem in registry]
    if library:
        # Library bodies use local graded guardian-floor contrast. Other IDs
        # retain their exact original-value and area comparison below.
        gate = runpy.run_path(str(ROOT/'scripts/check-boss-library.py'))
        selected = {Path(entry['boss']).stem: registry[Path(entry['boss']).stem] for entry in library}
        errors.extend(gate['floor_contrast_gates'](selected, contrast or captures)[0])
    entries = [entry for entry in entries if Path(entry['boss']).stem not in registry]
    font = ImageFont.truetype('/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf', 14)
    if output:
        output.mkdir(parents=True, exist_ok=True)
    for entry in entries:
        name = Path(entry['boss']).stem
        ev = entry['frames']['v2'][0]['separation']
        old_rows = original_rows(entry)
        sheet = Image.new('RGB', (1520, 1140), '#080b0e') if output else None
        for kit in (1, 2):
            divisor = 2 if kit == 1 else 1
            size = [v//divisor for v in ev['room_size']]
            origin = [v//divisor for v in ev['room_origin']]
            cw, ch = 8//divisor, 16//divisor
            results = []
            for index, (kind, rows) in enumerate((('old', old_rows), ('current', signature_rows(entry['frames'][f'v{kit}'][0])))):
                stem = captures/f'{name}-{kind}-0-v{kit}'
                rgb = Image.open(str(stem)+'-rgb.png').convert('RGB')
                raw = json.loads(Path(str(stem)+'-cells.json').read_text())
                assert rgb.size == (768//divisor,448//divisor) and len(raw) == 96*28
                points = body_points(rows, size, origin)
                floor = rgb.getpixel(tuple(v//divisor for v in ev['floor_sample']))
                room = presence([lightness(rgb.getpixel(p)) for p in points], floor)
                # The body fixture sits over the same unobstructed floor in
                # both paths; sample that floor through the emitted glyph too.
                fx, fy = [v//divisor for v in ev['floor_sample']]
                cell_floor = terminal_colour(raw[fy//ch*96+fx//cw], fx%cw*2//cw, fy%ch*4//ch)
                cells = presence([lightness(terminal_colour(raw[y//ch*96+x//cw], x%cw*2//cw, y%ch*4//ch))
                                  for x,y in points], cell_floor)
                results.append((room, cells))
                if sheet:
                    # 12 views: before/after x two kits x (4x crop, 1x, cells).
                    bx,by = origin; w,h = size
                    box = (bx-4,by-4,bx+w+4,by+h+4)
                    crop = rgb.crop(box)
                    y0 = 20 + (kit-1)*560 + index*275
                    draw = ImageDraw.Draw(sheet)
                    draw.text((12,y0),f'{name} / v{kit} / {ORIGINAL_REVISION if kind == "old" else "NEW"}',fill='#c2b494')
                    sheet.paste(crop.resize((crop.width*4,crop.height*4),Image.Resampling.NEAREST),(12,y0+22))
                    sheet.paste(crop,(380,y0+22))
                    x0,y1,x1,y2 = box[0]//cw,box[1]//ch,math.ceil(box[2]/cw),math.ceil(box[3]/ch)
                    ci=Image.new('RGB',((x1-x0)*8,(y2-y1)*16));cd=ImageDraw.Draw(ci)
                    for cy in range(y1,y2):
                        for cx in range(x0,x1):
                            c=raw[cy*96+cx];px,py=(cx-x0)*8,(cy-y1)*16
                            cd.rectangle((px,py,px+7,py+15),fill=tuple(c['bg']))
                            cd.text((px,py-2),c['glyph'],font=font,fill=tuple(c['fg']))
                    sheet.paste(ci,(600,y0+22))
                    draw.text((820,y0+40),f'ROOM median {room[0]:.2f} L* / presence {room[1]}px',fill='#c2b494')
                    draw.text((820,y0+65),f'CELLS median {cells[0]:.2f} L* / presence {cells[1]}px',fill='#c2b494')
            for path, old, new in zip(('ROOM','CELLS'),results[0],results[1]):
                print(f'{name}: ORIGINAL v{kit} {path} median {old[0]:.2f}->{new[0]:.2f} L*, presence {old[1]}->{new[1]}px (delta floor >=15 L*; both must be >=original)')
                errors.extend(f'{name}: v{kit} {path} below original {label}' for label in presence_regressions(old,new))
        if sheet:
            sheet.save(output/(name+'-original.png'))
    return errors


def player_review(entries, captures, output):
    """Only real paint_room_ui cells and native room exports are accepted.

    The fixture exports unscaled 768x448 RGB and 96x28 terminal buffers. Crop
    coordinates below refer to its staged points/lanes, never resized studies.
    Each saved sheet has at most twelve views and is under 1800px wide.
    """
    output.mkdir(parents=True, exist_ok=True)
    errors = []
    layout_path = captures/'review-layout.json'
    layout = json.loads(layout_path.read_text()) if layout_path.exists() else {}
    projectile_box = tuple(layout.get('projectile', (368,224,400,256)))
    font = ImageFont.truetype('/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf', 14)
    def cell_image(raw, box):
        x0,y0,x1,y1 = box
        im = Image.new('RGB', ((x1-x0)*8,(y1-y0)*16))
        d = ImageDraw.Draw(im)
        for y in range(y0,y1):
            for x in range(x0,x1):
                c = raw[y*96+x]
                px,py = (x-x0)*8,(y-y0)*16
                d.rectangle((px,py,px+7,py+15), fill=tuple(c['bg']))
                d.text((px,py-2),c['glyph'],font=font,fill=tuple(c['fg']))
        return im
    def buffers(name, stage=None):
        stem = f'{name}-current-0-v2' if stage is None else f'{name}-effects-{stage}-v2'
        rgb = Image.open(captures/(stem+'-rgb.png')).convert('RGB')
        raw = json.loads((captures/(stem+'-cells.json')).read_text())
        assert rgb.size == (768,448) and len(raw) == 96*28, 'native fixture dimensions changed'
        return rgb, raw
    def cell_box(box):
        x0,y0,x1,y1 = box
        return x0//8,y0//16,math.ceil(x1/8),math.ceil(y1/16)
    def cell_crop(raw, box):
        x0,y0,x1,y1 = cell_box(box)
        return [raw[y*96+x] for y in range(y0,y1) for x in range(x0,x1)]
    def row(sheet, y, label, native, cells):
        d = ImageDraw.Draw(sheet)
        d.text((12,y),label,fill='#c2b494')
        if native.width > 256:
            # A full committed area stays intact. Enlarging a whole room 3x
            # would crop away both the far boundary and the actual cell view.
            d.text((12,y+20),'1x native room / full area',fill='#96a5a9')
            sheet.paste(native,(12,y+40))
            d.text((580,y+20),'actual cells / full area',fill='#96a5a9')
            sheet.paste(cells,(580,y+40))
            return
        d.text((12,y+20),'3x native room',fill='#96a5a9')
        sheet.paste(native.resize((native.width*3,native.height*3),Image.Resampling.NEAREST),(12,y+40))
        middle = max(580, 32 + native.width*3)
        right = max(930, middle + native.width + 32)
        d.text((middle,y+20),'1x native room',fill='#96a5a9');sheet.paste(native,(middle,y+40))
        d.text((right,y+20),'actual cells',fill='#96a5a9');sheet.paste(cells,(right,y+40))
    body_row = max(240, max(50 + 3*(box[3]-box[1]) for entry in entries
        for box in [layout.get('bosses', {}).get(Path(entry['boss']).stem, {}).get('body', (342,144,426,196))]))
    bodies = Image.new('RGB',(1540,10+len(entries)*body_row),'#080b0e')
    context = Image.new('RGB',(1560,1450),'#080b0e')
    busy = Image.new('RGB',(1560,1450),'#080b0e')
    for j,entry in enumerate(entries):
        name = Path(entry['boss']).stem
        geometry = layout.get('bosses', {}).get(name, {})
        projectile_box = tuple(geometry.get('projectile', layout.get('projectile', (368,224,400,256))))
        impact_box = tuple(geometry.get('impact', (456,216,504,264)))
        rgb,raw = buffers(name)
        box = tuple(geometry.get('body', (342,144,426,196)))
        row(bodies,j*body_row+10,name+' / BODY',rgb.crop(box),cell_image(raw,cell_box(box)))
        for sheet,age in [(context,0),(busy,133)]:
            stem=f'{name}-current-{age}-v2'
            room=Image.open(captures/(stem+'-rgb.png')).convert('RGB')
            cells=json.loads((captures/(stem+'-cells.json')).read_text())
            d=ImageDraw.Draw(sheet);y=10+j*480
            d.text((10,y),f'{name} / ROOM 1x / tick {age}',fill='#c2b494')
            d.text((790,y),'ACTUAL HALF-BLOCK / BRAILLE CELLS',fill='#c2b494')
            sheet.paste(room,(10,y+20));sheet.paste(cell_image(cells,(0,0,96,28)),(790,y+20))

        # Unlit floor in this room is black. Measure the actual silhouette at
        # its displayed size, using alpha from its nearest-scaled native grid.
        spec=entry['frames']['v2'][0]
        ev=spec['separation']
        bw,bh=ev['room_size'];bx,by=ev['room_origin']
        source=render(signature_rows(spec),frame_inks(spec,palette())).getchannel('A')
        # Match the runtime's integer floor sampler, not Pillow's pixel-centre
        # nearest mapping (they differ when the stature enlarges a boss).
        alpha=Image.new('L',(bw,bh))
        alpha.putdata([source.getpixel((x*source.width//bw,y*source.height//bh))
                       for y in range(bh) for x in range(bw)])
        actual=rgb.crop((bx,by,bx+bw,by+bh)).convert('RGBA');actual.putalpha(alpha)
        floor=rgb.getpixel(tuple(ev['floor_sample']))
        def scaled(b):
            return [b[0]*bw//len(spec['rows'][0]),b[1]*bh//len(spec['rows']),
                    math.ceil(b[2]*bw/len(spec['rows'][0])),math.ceil(b[3]*bh/len(spec['rows']))]
        edge,material=separation(actual,floor,scaled(ev['head']),scaled(ev['body']))
        print(f'{name}: ROOM outer/floor {edge:.2f} L*, head/body {material:.2f} L* (need >=15)')
        if edge+1e-6<SEPARATION or material+1e-6<SEPARATION:
            errors.append(f'{name}: actual room separation fails')
        if name == 'cinderjaw':
            def orange(c):
                return c[0] >= c[1]*1.25 and c[1] >= c[2]*1.5 and c[1] > 0
            old_rgb=Image.open(captures/'cinderjaw-current-0-v1-rgb.png').convert('RGB')
            old_cells=json.loads((captures/'cinderjaw-current-0-v1-cells.json').read_text())
            def peak(im):
                return max(lightness(c) for c in im.get_flattened_data() if orange(c))
            def cell_peak(raw):
                return max(lightness(c[k]) for y in range(8,13) for x in range(43,53)
                           for k in ('fg','bg') if orange((c:=raw[y*96+x])[k]))
            old_peak=peak(old_rgb.crop((178,73,207,97)))
            new_peak=peak(actual.convert('RGB'))
            old_cell,new_cell=cell_peak(old_cells),cell_peak(raw)
            print(f'cinderjaw: orange ROOM {new_peak:.2f} >= v1 {old_peak:.2f} L*, CELLS {new_cell:.2f} >= v1 {old_cell:.2f} L*')
            if new_peak<old_peak or new_cell<old_cell:errors.append('cinderjaw: orange core is dimmer than v1')
        tellbox=tuple(geometry.get('tell', (376,160,392,416) if name=='the-index' else (304,224,336,256)))
        ring = bool(geometry.get('tell_ignore'))
        row_height = max(150, 50 + (tellbox[3]-tellbox[1])*(1 if tellbox[2]-tellbox[0]>256 else 3))
        tells=Image.new('RGB',(1540,860 if name=='the-index' else 10+4*row_height),'#080b0e')
        previous=None
        for stage in range(4):
            rgb,raw=buffers(name,stage)
            crop=rgb.crop(tellbox);cell=cell_crop(raw,tellbox)
            measured=crop.copy()
            if ring:
                # Boss hurt/rage exposure must not pass a frozen ring. The
                # fixture records the true central body and health-bar box.
                draw=ImageDraw.Draw(measured)
                for x0,y0,x1,y1 in geometry['tell_ignore']:
                    draw.rectangle((x0-tellbox[0],y0-tellbox[1],x1-tellbox[0]-1,y1-tellbox[1]-1),fill=(0,0,0))
                x0,y0,x1,y1=cell_box(tellbox)
                cell=[c for i,c in enumerate(cell) if not any(
                    a//8 <= x0+i%(x1-x0) < math.ceil(c_/8) and b//16 <= y0+i//(x1-x0) < math.ceil(d/16)
                    for a,b,c_,d in geometry['tell_ignore'])]
            if name=='the-index':
                # Ignore the separately advancing horizontal lane at the
                # crossing and the legacy fixture's moving projectile.
                # Neither may pass a frozen vertical warning-stage gate.
                draw=ImageDraw.Draw(measured)
                for y0,y1 in [(224,256),(336,368)]:
                    draw.rectangle((0,y0-160,15,y1-161),fill=(0,0,0))
                cell=[c for i,c in enumerate(cell) if 10+i//2 not in (14,15,21,22)]
            label=f'{name} / TELL {stage} / {25*(stage+1)}%'
            if previous:
                diff=sum(a!=b and abs(lightness(a)-lightness(b))>=RAMP_STEP
                         for a,b in zip(previous[0].get_flattened_data(),measured.get_flattened_data()))
                cdiff=sum(a!=b for a,b in zip(previous[1],cell))
                print(f'{name}: ROOM tell {stage-1}->{stage} visible diff={diff}px, CELLS diff={cdiff}')
                label+=f' / diff {diff}px, {cdiff} cells'
                if not diff or not cdiff: errors.append(f'{name}: identical/unreadable player tell {stage-1}/{stage}')
            previous=measured,cell
            if name=='the-index':
                x=12+stage*382;d=ImageDraw.Draw(tells)
                d.text((x,12),f'TELL {stage} / {25*(stage+1)}%',fill='#c2b494')
                if stage:d.text((x,30),f'diff {diff}px / {cdiff} cells',fill='#c2b494')
                for offset,title,view in [(0,'3x room',crop.resize((48,768),Image.Resampling.NEAREST)),
                                          (120,'1x room',crop),
                                          (240,'cells',cell_image(raw,cell_box(tellbox)))]:
                    d.text((x+offset,50),title,fill='#96a5a9');tells.paste(view,(x+offset,72))
            else:
                row(tells,10+stage*row_height,label,crop,cell_image(raw,cell_box(tellbox)))
        tells.save(output/(name+'-tell-player.png'))
        # Six projectile views, five impact enlargements and one labelled
        # impact cell row: exactly twelve images per sheet.
        shot_row = max(145, 50 + (projectile_box[3]-projectile_box[1])*3)
        impact_y = 20 + shot_row*2
        cell_y = impact_y + 25 + (impact_box[3]-impact_box[1])*3 + 32
        impact_cells_height = (cell_box(impact_box)[3]-cell_box(impact_box)[1])*16
        fx=Image.new('RGB',(1540,cell_y+20+impact_cells_height+12),'#080b0e');d=ImageDraw.Draw(fx)
        for role,box,count in [('projectile',projectile_box,2),('tell',tellbox,4),('impact',impact_box,5)]:
            for stage in range(count):
                rgb,raw_effect= buffers(name,stage);crop=rgb.crop(box)
                effect_cells=cell_crop(raw_effect,box)
                if not any(any(c) for c in crop.get_flattened_data()) or not any(
                        any(c['fg']) or any(c['bg']) for c in effect_cells):
                    errors.append(f'{name}: missing visible {role} {stage} room/cell row')
                white={(x,y) for y in range(crop.height) for x in range(crop.width)
                       if min(crop.getpixel((x,y)))>=240}
                clusters=[len(c) for c in components(white) if len(c)>=2]
                print(f'{name}: ROOM {role} {stage} hypercontrast clusters={clusters}')
                if clusters:errors.append(f'{name}: rendered {role} {stage} near-white cluster')
        for stage in range(2):
            rgb,raw=buffers(name,stage)
            box=projectile_box
            row(fx,10+stage*shot_row,f'{name} / PROJECTILE {stage}',rgb.crop(box),cell_image(raw,cell_box(box)))
        for stage in range(5):
            rgb,raw=buffers(name,stage);box=impact_box;x=12+stage*290
            d.text((x,impact_y),f'IMPACT {stage} / 3x',fill='#c2b494');crop=rgb.crop(box)
            fx.paste(crop.resize((crop.width*3,crop.height*3),Image.Resampling.NEAREST),(x,impact_y+25))
        rgb,raw=buffers(name,0)
        d.text((12,cell_y),'IMPACT / ACTUAL CELL BUFFER',fill='#c2b494')
        fx.paste(cell_image(raw,cell_box(impact_box)),(350,cell_y+20))
        fx.save(output/(name+'-effects-player.png'))
    bodies.save(output/'bodies-player.png')
    context.save(output/'rooms-player.png')
    busy.save(output/'combat-player.png')
    return errors


def render(rows, inks):
    im = Image.new('RGBA',(len(rows[0]),len(rows)))
    for y,row in enumerate(rows):
        for x,k in enumerate(row):
            if k != '.': im.putpixel((x,y),inks[k]+(255,))
    return im


def write_changed(path, data):
    """Keep include-file mtimes stable when a reviewed recipe is unchanged."""
    if not path.exists() or path.read_bytes() != data:
        path.write_bytes(data)


def native_recipe(entries):
    """Reviewed native grids are the final reproducible hand-cleanup stage."""
    errors = numeric_gates(entries)
    assert not errors, '\n'.join(errors)
    all_inks = palette()
    # Validate the whole batch before touching any accepted asset.
    for entry in entries:
        inks = {k:all_inks[k] for k in entry['inks']}
        assert len(inks) <= 10
        assert {k:len(v) for k,v in entry['frames'].items()} == {
            'v1':1,'v2':1,'projectile':2,'tell':4,'impact':5}
        v1 = entry['frames']['v1'][0]['rows']
        old = (ROOT/entry['boss']).read_text().split('\nart\n',1)[1].splitlines()
        assert (len(v1[0]),len(v1)) == (max(map(len,old)),len(old))
        for role,frames in entry['frames'].items():
            for spec in frames:
                rows = signature_rows(spec)
                gate(rows,frame_inks(spec,inks),hand_pixels(spec))
                w,h = len(rows[0]),len(rows)
                if role == 'projectile': assert w >= 7 and h >= 7
                if role == 'impact': assert 24 <= w <= 32 and 24 <= h <= 32
                if role == 'v2': assert (w,h) == (len(v1[0])*2,len(v1)*2)
                if role in ('v1','v2'):
                    assert 1 <= len(spec.get('signature_lights', [])) <= 2
        assert entry['review']['theme'] and entry['review']['originality']
    for entry in entries:
        output = ROOT/entry['output']
        output.mkdir(parents=True,exist_ok=True)
        inks = {k:all_inks[k] for k in entry['inks']}
        for role, frames in entry['frames'].items():
            for index, spec in enumerate(frames):
                rows, keep = signature_rows(spec), hand_pixels(spec)
                colours = frame_inks(spec,inks)
                gate(rows,colours,keep)
                name = role if index == 0 else f'{role}-{index}'
                png = io.BytesIO()
                render(rows,colours).save(png,format='PNG')
                write_changed(output/(name+'.png'),png.getvalue())
                write_changed(output/(name+'.art'),('\n'.join(rows)+'\n').encode())
                if role == 'v2':
                    # A separate emissive layer exposes only reviewed lights
                    # in terminal cells, after room lighting reduces the body.
                    lights = [['.'] * len(rows[0]) for _ in rows]
                    for light in spec['signature_lights']:
                        for x, y, k in light['pixels']:
                            lights[y][x] = k
                    png = io.BytesIO()
                    render([''.join(row) for row in lights],colours).save(png,format='PNG')
                    write_changed(output/'v2-lights.png',png.getvalue())
                if role == 'v1':
                    boss = ROOT/entry['boss']
                    header, old = boss.read_text().split('\nart\n',1)
                    assert (len(rows[0]),len(rows)) == (max(map(len,old.splitlines())),len(old.splitlines()))
                    write_changed(boss,(header+'\nart\n'+'\n'.join(rows)+'\n').encode())


def palette():
    source = (ROOT / 'crates/delve/src/render/overworld/ink.rs').read_text()
    source = source.split('pub fn ink(ch: char)')[1].split('Some(rgb(v))')[0]
    return {k: tuple(bytes.fromhex(v)) for k, v in re.findall(r"'(.)' => 0x([0-9a-f]{6})", source)}


def extract(im, background, tolerance):
    """Flood-key the border; enclosed dark details and existing alpha survive."""
    im = im.convert('RGBA')
    if im.getchannel('A').getextrema()[0] == 0:
        return im
    px = im.load()
    w, h = im.size
    samples = [px[0, 0], px[w-1, 0], px[0, h-1], px[w-1, h-1]]
    key = tuple(sorted(c[i] for c in samples)[2] for i in range(3))
    def is_background(x, y):
        r, g, b, _ = px[x, y]
        if background == 'black':
            return max(r, g, b) <= tolerance
        # Neutral backgrounds may have a small lighting gradient. Colourful
        # slate contours stay attached even when their luminance matches it.
        return max(r, g, b)-min(r, g, b) < 12 and max(abs(c-k) for c, k in zip((r,g,b), key)) <= tolerance
    todo = deque((x, y) for x in range(w) for y in (0, h-1))
    todo.extend((x, y) for y in range(h) for x in (0, w-1))
    seen = set()
    while todo:
        x, y = todo.popleft()
        if (x, y) in seen or not is_background(x, y):
            continue
        seen.add((x, y))
        px[x, y] = (0, 0, 0, 0)
        for nx, ny in ((x-1,y),(x+1,y),(x,y-1),(x,y+1)):
            if 0 <= nx < w and 0 <= ny < h and (nx, ny) not in seen:
                todo.append((nx, ny))
    # Black effect paper can be enclosed inside a curl; it is still paper.
    if background == 'black':
        for y in range(h):
            for x in range(w):
                if max(px[x,y][:3]) <= tolerance:
                    px[x,y] = (0,0,0,0)
    return im


def fit(im, size, inset=0):
    box = im.getchannel('A').getbbox()
    if box is None:
        raise ValueError('background key removed the whole subject')
    im = im.crop(box)
    scale = min((size[0]-2*inset)/im.width, (size[1]-2*inset)/im.height)
    im = im.resize((max(1,round(im.width*scale)), max(1,round(im.height*scale))), Image.Resampling.NEAREST)
    out = Image.new('RGBA', size)
    out.paste(im, ((size[0]-im.width)//2, size[1]-inset-im.height))
    return out


def snap(im, inks):
    out = Image.new('RGBA', im.size)
    rows = []
    for y in range(im.height):
        row = ''
        for x in range(im.width):
            c = im.getpixel((x,y))
            if c[3] < 128:
                row += '.'
                continue
            k = min(inks, key=lambda k: sum((a-b)**2 for a,b in zip(c[:3],inks[k])))
            out.putpixel((x,y), inks[k]+(255,))
            row += k
        rows.append(row)
    return out, rows


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('mode', choices=['boss','effect','recipe','prompt','check','review'])
    p.add_argument('input', type=Path)
    p.add_argument('--room-captures', type=Path, help='actual write_boss_art_terminal_shots output; required for review')
    p.add_argument('--floor-contrast', type=Path, help='paired guardian-room captures for library IDs; defaults to room-captures')
    p.add_argument('--boss', type=Path, help='existing .boss; replace only its art, preserving the grid envelope')
    p.add_argument('--output', type=Path, help='directory for beside-the-boss assets')
    p.add_argument('--concepts', type=Path, help='source image directory when running a saved recipe')
    p.add_argument('--crop', type=int, nargs=4, metavar=('LEFT','TOP','RIGHT','BOTTOM'))
    p.add_argument('--background', choices=['neutral','black'], default='neutral')
    p.add_argument('--tolerance', type=int, default=48)
    p.add_argument('--inks', default='sSuUPRotT6', help='at most ten existing inks, arranged in 3–4 tone material ramps')
    p.add_argument('--ramps', nargs='+', default=['sSuU','PRot','otT6'], help='dark to light, 3–4 inks per material ramp, used for the selective contour')
    p.add_argument('--hand-pixels', type=Path, help='JSON coordinates of reviewed eyes/highlights allowed to be single pixels')
    p.add_argument('--separation', type=Path, help='v2 floor and head/body material boxes; required for boss conversion')
    p.add_argument('--signature-lights', type=Path, help='JSON v1/v2 lists of named lights with locked hues and explicit pixels; applied after all cleanup')
    p.add_argument('--role', choices=['projectile','tell','impact'], default='projectile')
    p.add_argument('--patch', type=Path, help='reviewed ink rectangles; spaces retain the sampled pixel, dots clear it')
    p.add_argument('--flip-x', action='store_true', help='orient an effect concept to face right')
    p.add_argument('--study-exposure', type=float, default=1.0, help='retained recipe compatibility; native studies use the same ten inks')
    p.add_argument('--size', type=int, nargs=2, help='effect dimensions; default projectile 9x9, tell 25x25, impact 28x28')
    a = p.parse_args()
    if a.mode == 'prompt':
        print(PROMPT.format(width=(a.size or [28,23])[0], height=(a.size or [28,23])[1],subject=a.input))
        return
    if a.mode in ('check', 'review'):
        entries = json.loads(a.input.read_text())
        errors = numeric_gates(entries) + identity_gates(entries)
        if not a.room_captures:
            p.error('check/review requires --room-captures: native recipe gates alone cannot approve an art build')
        errors += original_gates(entries, a.room_captures, a.output if a.mode == 'review' else None, a.floor_contrast)
        if a.mode == 'review':
            if not a.room_captures or not a.output: p.error('review needs --room-captures and --output')
            errors += player_review(entries, a.room_captures, a.output)
        for error in errors: print('FAIL: '+error)
        print(f'{len(errors)} failure(s)')
        raise SystemExit(bool(errors))
    if a.mode == 'recipe':
        entries = json.loads(a.input.read_text())
        if entries and 'frames' in entries[0]:
            native_recipe(entries)
            return
        if not a.concepts:
            p.error('recipe mode requires --concepts')
        for entry in entries:
            command = [sys.executable, str(Path(__file__).resolve()), entry['mode'], str(a.concepts/entry['input'])]
            for key, value in entry.items():
                if key in ('mode','input'):
                    continue
                command.append('--'+key)
                if value is True:
                    continue
                command.extend(map(str, value if isinstance(value,list) else [value]))
            subprocess.run(command, cwd=ROOT, check=True)
        return
    if not a.output:
        p.error('boss/effect modes require --output')
    im = Image.open(a.input)
    if a.crop:
        im = im.crop(a.crop)
    im = extract(im, a.background, a.tolerance)
    if a.flip_x:
        im = im.transpose(Image.Transpose.FLIP_LEFT_RIGHT)
    a.output.mkdir(parents=True, exist_ok=True)
    all_inks = palette()
    inks = {k:v for k,v in all_inks.items() if k in a.inks}
    if len(inks) > 10:
        p.error('choose at most ten inks in deliberate 3–4 tone ramps')
    if any(not 3 <= len(r) <= 4 for r in a.ramps) or not set(inks) <= set(''.join(a.ramps)):
        p.error('cover the chosen inks with overlapping 3–4 tone ramps')
    keep = json.loads(a.hand_pixels.read_text()) if a.hand_pixels else []
    lights = json.loads(a.signature_lights.read_text()) if a.signature_lights else {}
    def finish(rows, role):
        spec = {'rows':rows,'hand_pixels':keep,'signature_lights':lights.get(role,[])}
        if a.mode == 'boss':
            assert 1 <= len(spec['signature_lights']) <= 2, 'bosses require protected signature lights'
        rows = signature_rows(spec)
        colours = frame_inks(spec,inks)
        gate(rows,colours,hand_pixels(spec))
        im = render(rows,colours)
        if role == 'v2':
            if not a.separation: p.error('boss conversion needs --separation for the v2 visibility gate')
            ev = json.loads(a.separation.read_text())
            floor = tuple(bytes.fromhex(ev['floor'].lstrip('#')))
            edge,materials = separation(im,floor,ev['head'],ev['body'])
            assert edge >= SEPARATION and materials >= SEPARATION, f'native separation fails: {edge:.2f}/{materials:.2f} L*'
            errors = signature_gate(spec,colours)
            assert not errors, '\n'.join(errors)
        return rows, im
    if a.mode == 'boss':
        if not a.boss:
            p.error('boss mode requires --boss')
        header, old = a.boss.read_text().split('\nart\n', 1)
        rows = old.splitlines()
        size = (max(map(len, rows)),len(rows))
        small, rows = snap(fit(im,size),inks)
        if a.patch:
            cells = [list(row) for row in rows]
            for patch in json.loads(a.patch.read_text()):
                for dy, row in enumerate(patch['rows']):
                    for dx, k in enumerate(row):
                        if k != ' ':
                            cells[patch['y']+dy][patch['x']+dx] = k
            rows = [''.join(row) for row in cells]
            small = Image.new('RGBA', size)
            for y, row in enumerate(rows):
                for x, k in enumerate(row):
                    if k != '.':
                        small.putpixel((x,y),all_inks[k]+(255,))
        rows = despeckle(contour(despeckle(rows,keep),a.ramps),keep)
        rows, small = finish(rows,'v1')
        body_rows = rows
        study_rows = snap(fit(im,(size[0]*2,size[1]*2)),inks)[1]
        study_rows = despeckle(contour(despeckle(study_rows,keep),a.ramps),keep)
        study_rows, study = finish(study_rows,'v2')
        # Both kits must pass before either accepted asset is changed.
        a.boss.write_text(header+'\nart\n'+'\n'.join(body_rows)+'\n')
        small.save(a.output/'v1.png')
        study.save(a.output/'v2.png')
    else:
        size = tuple(a.size or {'projectile':(9,9),'tell':(25,25),'impact':(28,28)}[a.role])
        small, rows = snap(fit(im,size),inks)
        rows = despeckle(contour(despeckle(rows,keep),a.ramps),keep)
        gate(rows,inks,keep)
        small = render(rows,inks)
        small.save(a.output/(a.role+'.png'))
        (a.output/(a.role+'.art')).write_text('\n'.join(rows)+'\n')
    print(a.output)


if __name__ == '__main__':
    main()
