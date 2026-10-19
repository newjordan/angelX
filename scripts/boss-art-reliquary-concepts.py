#!/usr/bin/env python3
"""Retain unsmoothed native candidates before explicit native row cleanup.

python scripts/boss-art-reliquary-concepts.py CONCEPTS OUTPUT
The checked native recipe, boss-art-reliquary.json, owns the final drawings.
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
a = p.parse_args()
a.output.mkdir(parents=True, exist_ok=True)
pal = art['palette']()
families = [
    ('crypt-tax-keeper', 'tax-projectile', 'sSUhHoOt@6', (21, 25)),
    ('crypt-relic-liege', 'liege-projectile', 'sSRhHotT23', (22, 32)),
    ('iron-tusk-chief', 'chief-projectile', 'sSUAGoOt@6', (23, 27)),
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
