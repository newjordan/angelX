#!/usr/bin/env python3
"""Measure BOSS-3 silhouettes and warning glyphs in actual terminal buffers."""
import argparse
import json
import math
import runpy
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
ART = runpy.run_path(str(ROOT/'scripts/boss-art.py'))
NAMES = ('crypt-tax-keeper', 'crypt-relic-liege', 'iron-tusk-chief')


def shape(raw, background, box):
    """Covered 2x4 glyph samples above the same frame's effect-free room."""
    out = {}
    for y in range(box[1]//16, math.ceil(box[3]/16)):
        for x in range(box[0]//8, math.ceil(box[2]/8)):
            bits = 0
            for dy in range(4):
                for dx in range(2):
                    a = ART['terminal_colour'](raw[y*96+x], dx, dy)
                    b = ART['terminal_colour'](background[y*96+x], dx, dy)
                    if ART['lightness'](a)-ART['lightness'](b) >= 15:
                        bits |= 1 << (dy*2+dx)
            out[x,y] = bits
    return out


def half_cells(mask):
    return {(x,y,h) for (x,y),bits in mask.items() for h in (0,1)
            if bits & (15 << (h*4))}


def pigment_shape(raw, box, inks):
    """Exact emitted warning coverage, including strokes over a bright body.

    The runtime priority bank uses these same source pigments. Its regression
    test rejects collisions with every cached library pose/material/flash.
    Keep the15 L* dark-floor requirement and the30% stage-change floor.
    """
    out = {}
    for y in range(box[1]//16, math.ceil(box[3]/16)):
        for x in range(box[0]//8, math.ceil(box[2]/8)):
            bits = 0
            for dy in range(4):
                for dx in range(2):
                    c = tuple(ART['terminal_colour'](raw[y*96+x], dx, dy))
                    if c in inks and ART['lightness'](c) >= 15:
                        bits |= 1 << (dy*2+dx)
            out[x,y] = bits
    return out


def notches(mask):
    """Empty half-cells between two teeth, open on one side of a ward."""
    occupied = {(x,2*y+h) for x,y,h in half_cells(mask)}
    if not occupied: return 0
    x0,x1=min(x for x,y in occupied),max(x for x,y in occupied)
    y0,y1=min(y for x,y in occupied),max(y for x,y in occupied)
    return sum((x,y) not in occupied and (x,y-1) in occupied and (x,y+1) in occupied
               and ((x-1,y) in occupied or (x+1,y) in occupied)
               for x in range(x0,x1+1) for y in range(y0,y1+1))


def measure(captures, recipe=ROOT/'scripts/boss-art-reliquary.json', names=NAMES, display=None):
    geometry = json.loads((captures/'review-layout.json').read_text())['bosses']
    entries = json.loads(recipe.read_text())
    errors = []
    metrics = {}
    for name,entry in zip(names,entries):
        g = geometry[name]
        for stage,frame in enumerate(entry['frames']['tell']):
            colours = ART['frame_inks'](frame,ART['palette']())
            peak = max(ART['lightness'](colours[k]) for row in frame['rows'] for k in row if k != '.')
            if peak > 80+1e-6: errors.append(f'{name}: tell {stage} brightest ink L* {peak:.2f} >80')
        for kit in (1,2):
            ignore = list(g['tell_ignore'])
            frames = [json.loads((captures/f'{name}-effects-{s}-v{kit}-cells.json').read_text()) for s in range(5)]
            bg = [json.loads((captures/f'{name}-effects-{s}-v{kit}-background-cells.json').read_text()) for s in range(5)]
            shots = [half_cells(shape(frames[s], bg[s], g['projectile'])) for s in (0,1)]
            for stage in (0,1):
                occupied = {(x,y) for x,y,h in shots[stage]}
                width = max((x for x,y in occupied), default=0)-min((x for x,y in occupied), default=0)+1
                height = max((y for x,y in occupied), default=0)-min((y for x,y in occupied), default=0)+1
                print(f'{name} v{kit}: projectile {stage} covers {len(occupied)} actual cells, {width}x{height}')
                if len(occupied)<9 or width<3 or height<3:
                    errors.append(f'{name} v{kit}: projectile {stage} below 3x3 actual cells')
            changed = len(shots[0]^shots[1])
            print(f'{name} v{kit}: projectile silhouette changes {changed} half-block cells (need >=2)')
            if changed < 2: errors.append(f'{name} v{kit}: projectile silhouette collapsed')
            values = {'projectile_changed_half_cells': changed, 'tells':[], 'impacts':[]}
            slots = json.loads((ROOT/'scripts/boss-effect-slots.json').read_text()).get(name,{})
            if kit == 1 and display and name in display and slots.get('cell_warning_priority'):
                inks = set(map(tuple,slots.get('cell_warning_inks',[])))
                for file in ([] if 'cell_warning_inks' in slots else slots['tell']):
                    im = Image.open(ROOT/'cockpit/assets/dungeon/bosses'/name/file).convert('RGBA')
                    inks.update(c[:3] for c in im.get_flattened_data() if c[3])
                masks = [pigment_shape(frames[s], g['tell'], inks) for s in range(4)]
                ignore = []  # Known pigments cannot be a changed body pose.
                print(f'{name} v{kit}: exact registered warning pigments; includes body overlap')
            else:
                masks = [shape(frames[s], bg[s], g['tell']) for s in range(4)]
            for stage in range(1,4):
                active = [q for q in masks[stage] if (masks[stage][q] or masks[stage-1][q]) and not any(
                    b[0]//8<=q[0]<math.ceil(b[2]/8) and b[1]//16<=q[1]<math.ceil(b[3]/16) for b in ignore)]
                # V1 always emits the same half-block codepoint. Its occupied
                # upper/lower halves are the glyph shape, just as braille bits
                # are in V2; changing ink alone cannot change this mask.
                diff = sum(masks[stage][q] != masks[stage-1][q] for q in active)
                ratio = diff/len(active) if active else 0
                print(f'{name} v{kit}: tell {stage-1}->{stage} glyphs change {diff}/{len(active)} = {ratio:.1%} (need >=30%)')
                values['tells'].append({'changed':diff,'active':len(active),'ratio':ratio})
                if ratio < .30: errors.append(f'{name} v{kit}: tell {stage-1}/{stage} shape collapsed')
            for stage in (1,2):
                impact = shape(frames[stage],bg[stage],g['impact'])
                if name=='crypt-tax-keeper':
                    cutouts = notches(impact)
                    print(f'{name} v{kit}: impact {stage} has {cutouts} visible ward notches')
                    if not cutouts: errors.append(f'{name} v{kit}: impact {stage} wards collapsed to bars')
                active = [q for q,bits in impact.items() if bits]
                w = max(x for x,y in active)-min(x for x,y in active)+1 if active else 0
                h = max(y for x,y in active)-min(y for x,y in active)+1 if active else 0
                print(f'{name} v{kit}: impact {stage} covers {len(active)} cells, {w}x{h}')
                values['impacts'].append({'cells':len(active),'width':w,'height':h})
                if name=='crypt-relic-liege' and (len(active)<16 or w<4 or h<4):
                    errors.append(f'{name} v{kit}: impact {stage} below 4x4 cells')
            metrics[f'{name}-v{kit}'] = values
    for error in errors: print('FAIL: '+error)
    print(f'{len(errors)} send-back failure(s)')
    return errors,metrics


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('captures',type=Path)
    p.add_argument('--metrics',type=Path)
    p.add_argument('--recipe',type=Path,default=ROOT/'scripts/boss-art-reliquary.json',help='recipe matching these captures; select the saved recipe for an old audit')
    args=p.parse_args()
    errors,metrics=measure(args.captures,args.recipe)
    if args.metrics: args.metrics.write_text(json.dumps(metrics,indent=2)+'\n')
    raise SystemExit(bool(errors))
