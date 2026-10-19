#!/usr/bin/env python3
"""Retain nearest/palette samples for the three Crypt body detail studies."""
import argparse
import json
import runpy
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
ART = runpy.run_path(str(ROOT / 'scripts/boss-art.py'))
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('concepts', type=Path)
p.add_argument('output', type=Path)
a = p.parse_args()
a.output.mkdir(parents=True, exist_ok=True)
entries = json.loads((ROOT / 'scripts/boss-art-crypt.json').read_text())
sheet = Image.new('RGB', (1200, 280), '#080b0e')
for i, (name, entry) in enumerate(zip(('wax', 'choir', 'marshal'), entries)):
    spec = entry['frames']['v1'][0]
    size = len(spec['rows'][0]), len(spec['rows'])
    colours = ART['frame_inks'](spec, ART['palette']())
    source = ART['extract'](Image.open(a.concepts / (name + '.png')), 'neutral', 30)
    sample, rows = ART['snap'](ART['fit'](source, size), colours)
    sample.save(a.output / (name + '.png'))
    (a.output / (name + '.art')).write_text('\n'.join(rows) + '\n')
    ImageDraw.Draw(sheet).text((i * 400 + 12, 12), name + ' / nearest candidate', fill='#c2b494')
    view = sample.resize((size[0] * 7, size[1] * 7), Image.Resampling.NEAREST)
    sheet.paste(view, (i * 400 + 12, 40), view)
sheet.save(a.output / 'nearest-candidates.png')
