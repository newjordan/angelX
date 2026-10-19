#!/usr/bin/env python3
"""Compose delivered Mossbell material; retain rule-owned registration masks."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / 'crates/delve/assets/mossbell/composed'
DELIVERIES = Path('/work/artifacts/art-prod/slots')
spec = importlib.util.spec_from_file_location('slots', Path(__file__).with_name('slot-art-import.py'))
slots = importlib.util.module_from_spec(spec)
spec.loader.exec_module(slots)


def three_slice(im, width, caps, stretch=False):
    """Fixed native caps, repeated middle. No horizontal rescaling of the ends."""
    left, right = caps
    assert width >= left + right and im.width > left + right
    out = Image.new('RGBA', (width, im.height))
    out.paste(im.crop((0, 0, left, im.height)), (0, 0))
    out.paste(im.crop((im.width-right, 0, im.width, im.height)), (width-right, 0))
    mid = im.crop((left, 0, im.width-right, im.height))
    if stretch:
        out.paste(mid.resize((width-left-right,im.height),Image.Resampling.NEAREST),(left,0))
        return out
    for x in range(left, width-right, mid.width):
        part = mid.crop((0, 0, min(mid.width, width-right-x), im.height))
        out.paste(part, (x, 0))
    return out


def native(im, height):
    # Source conversion only. Composition never enlarges the native end caps.
    im, _ = slots.despill(im)
    im = im.resize((round(im.width * height / im.height), height), Image.Resampling.BOX)
    out = Image.new('RGBA', im.size)
    palette = [c for c in slots.palette_cache if max(c) <= 224]
    for y in range(im.height):
        for x in range(im.width):
            r,g,b,a = im.getpixel((x,y))
            if a >= 128:
                rgb = min(palette, key=lambda c: sum((a-b)**2 for a,b in zip(c,(r,g,b))))
                out.putpixel((x,y), rgb+(255,))
    return out


def run(check=False):
    manifest = {'schema':1, 'method':'native fixed caps + repeated straight middle; rule-mask clipping for crown',
                'sources':{}, 'outputs':{}, 'physics':'unchanged; x8/5 presentation; original art fallback',
                'states':{'flippers':['resting-pair','left-raised','right-raised','both-raised'],
                          'root':'four growing tell frames; attack holds frame 3; renderer mirrors for left sweep',
                          'crown':'four growing tell frames; attack holds frame 3; old art retained in cells'},
                'timing':'existing 36/30-tick tell selection and attack timing in rules/mossbell.rs; no changes',
                'contracts':{}}
    def save(name, im):
        data = slots.png(im)
        slots.write(DEST/name, data, check)
        manifest['outputs'][name] = {'wh':list(im.size), 'sha256':hashlib.sha256(data).hexdigest()}
    pieces = {}
    for kind in ['flippers','root','crown']:
        folder = DELIVERIES/f'mossbell.{kind}.geometry-1'/'v01'
        path = folder/'source-sheet.png'
        x,y,w,h = json.loads((folder/'source-sheet.json').read_text())['pieces'][0]['source_rect_xywh']
        # One half of the paired crown delivery supplies a single leaf bar.
        if kind == 'crown': w = 215
        crop = Image.open(path).convert('RGBA').crop((x,y,x+w,y+h))
        save(f'source-{kind}.png', crop)
        manifest['sources'][kind] = {'path':str(path), 'sha256':slots.digest(path), 'rect_xywh':[x,y,w,h],
                                     'crop_sha256':manifest['outputs'][f'source-{kind}.png']['sha256']}
        pieces[kind] = native(crop, {'flippers':5,'root':11,'crown':6}[kind])
        save(f'native-{kind}.png', pieces[kind])
    for scale, tag in [(5/4,'legacy'),(8/5,'current')]:
        fx = lambda x: int(x*scale)
        # A bar is registered by its physical pivot and tip; slope is an integer
        # column shear, not a sprite rotation that moves or inflates the rail.
        span = fx(70)-fx(37)
        bar = three_slice(pieces['flippers'], int(33*scale)+1, (4,3))
        save(f'{tag}-paddle.png',bar)
        width = fx(123)-fx(37)+7
        manifest['contracts'][tag] = {'paddle_wh':[bar.width,5], 'pair_wh':[width,26],
            'pair_origin_table_xy':[fx(37)-3,108], 'pair_pivots_xy':[[3,11],[width-4,11]],
            'pair_tip_x':[3+span,width-4-span], 'pair_tip_y':{'raised':3,'resting':22},
            'root_wh':[fx(150)-fx(7),11], 'root_origin_table_xy':[fx(7),72],
            'crown_wh':[fx(100)-fx(60)+1,24], 'crown_origin_table_xy':[fx(60),20]}
        for state in range(4):
            pair = Image.new('RGBA',(width,26))
            for side in range(2):
                raised = state & (1<<side)
                for along in range(span+1):
                    x = 3+along if side == 0 else width-4-along
                    cy = 11 + int(along * (-8 if raised else 11) / span)
                    for y in range(5):
                        pair.putpixel((x,cy+y-2),bar.getpixel((along*(bar.width-1)//span,y)))
            save(f'{tag}-flippers-{state}.png',pair)
        width = fx(150)-fx(7)
        for stage in range(4):
            # Preserve the existing progressive reach, including the last bud's
            # clearance from the plunger wall, at both presentation widths.
            guide = Image.open(DEST.parent/f'root-vines-{stage}.png').convert('RGBA')
            reach = (guide.getbbox()[2]*width + guide.width-1)//guide.width
            root = Image.new('RGBA',(width,11))
            root.paste(three_slice(pieces['root'],reach,(10,13)),(0,0))
            save(f'{tag}-root-{stage}.png',root)
        width = fx(100)-fx(60)+1
        for stage in range(4):
            guide = Image.open(DEST.parent/f'crown-vines-{stage}.png').convert('RGBA')
            original = guide
            guide = Image.new('RGBA',(width,24))
            for y in range(24):
                for x in range(width):
                    guide.putpixel((x,y),original.getpixel((x*original.width//width,y)))
            # The existing growing jaws own the air gap and outside contour.
            # The leaf's straight middle stretches inside those registered jaws.
            crown = Image.new('RGBA',(width,24))
            solid = {(x,y) for y in range(24) for x in range(width)
                     if guide.getpixel((x,y))[3] and guide.getpixel((x,y))[:3] != (24,43,43)}
            leaf = three_slice(pieces['crown'],(width+1)//2,(3,3),stretch=True)
            for y in range(24):
                xs = [x for x in range(width) if (x,y) in solid]
                if not xs: continue
                for side in range(2):
                    row = [x for x in xs if (x <= width//2 if side == 0 else x > width//2)]
                    if not row: continue
                    for x in row:
                        # Project the delivered material into the old solid jaw.
                        # Transparent source gutters must not perforate its tell.
                        sx = x if side == 0 else width-1-x
                        column = [yy for yy in range(6) if leaf.getpixel((sx,yy))[3]]
                        assert column, (stage, x)
                        ys = [yy for yy in range(24) if (x,yy) in solid]
                        at = (y-min(ys))*5//max(1,max(ys)-min(ys))
                        sy = min(column,key=lambda yy:abs(yy-at))
                        crown.putpixel((x,y),leaf.getpixel((sx,sy)))
            save(f'{tag}-crown-{stage}.png',crown)
    slots.write(DEST/'composition.json',slots.js(manifest),check)
    print('Verified' if check else 'Wrote',len(manifest['outputs']),'source crops and composed native frames')


if __name__ == '__main__':
    p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');run(p.parse_args().check)
