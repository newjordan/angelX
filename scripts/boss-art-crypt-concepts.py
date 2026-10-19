#!/usr/bin/env python3
"""Retain unsmoothed native candidates before explicit Crypt row cleanup.

python scripts/boss-art-crypt-concepts.py CONCEPTS OUTPUT
The checked native recipe, boss-art-crypt.json, owns the final drawings.
"""
import argparse
import json
import runpy
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
art = runpy.run_path(str(ROOT / 'scripts/boss-art.py'))
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('concepts', type=Path)
p.add_argument('output', type=Path)
p.add_argument('--sendback', action='store_true', help='sample the ivory, tallow, bronze and coin-shard references')
a = p.parse_args()
a.output.mkdir(parents=True, exist_ok=True)
pal = art['palette']()
if a.sendback:
    recipe = json.loads((ROOT/'scripts/boss-art-crypt.json').read_text())
    sheet = Image.new('RGB',(1280,300),'#080b0e')
    for i,(name,index,size) in enumerate([
        ('wax',0,(28,24)),('choir',1,(28,24)),('marshal',2,(23,27)),('coin-shards',2,(28,28))]):
        colours=art['frame_inks'](recipe[index]['frames']['v1'][0],pal)
        src=art['extract'](Image.open(a.concepts/(name+'.png')),'neutral',30)
        sample,rows=art['snap'](art['fit'](src,size),{k:colours[k] for k in recipe[index]['inks']})
        sample.save(a.output/(name+'.png'))
        (a.output/(name+'.art')).write_text('\n'.join(rows)+'\n')
        ImageDraw.Draw(sheet).text((i*320+10,10),name+' / rejected nearest',fill='#c2b494')
        view=sample.resize((size[0]*6,size[1]*6),Image.Resampling.NEAREST)
        sheet.paste(view,(i*320+10,40),view)
    sheet.save(a.output/'nearest-candidates.png')
    raise SystemExit(0)
families = [
    ('waxen-warden', 'wax-projectile', 'SUJhHotT@6', (28, 24)),
    ('the-bone-choir', 'choir-projectile', 'sSUJhH123t', (28, 24)),
    ('crypt-tithe-marshal', 'marshal-projectile', 'SQUoOtT@6J', (23, 27)),
]
sheet = Image.new('RGB', (1320, 700), '#080b0e')
d = ImageDraw.Draw(sheet)
for i, (boss, effect, inks, size) in enumerate(families):
    for j, (name, dims) in enumerate([(boss, size), (effect, (9, 9))]):
        src = art['extract'](Image.open(a.concepts / (name + '.png')), 'neutral', 30)
        sample, rows = art['snap'](art['fit'](src, dims), {k:pal[k] for k in inks})
        sample.save(a.output / (name + '.png'))
        (a.output / (name + '.art')).write_text('\n'.join(rows) + '\n')
        x, y = 20 + j * 600, 35 + i * 220
        d.text((x, y - 18), name + ' / nearest palette candidate / rejected', fill='#c2b494')
        q = sample.resize((dims[0] * 6, dims[1] * 6), Image.Resampling.NEAREST)
        sheet.paste(q, (x, y), q)
sheet.save(a.output / 'nearest-candidates.png')
