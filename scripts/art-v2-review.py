#!/usr/bin/env python3
"""Build uncropped v1 | accepted upgrade | v2 comparisons from exact Rust frames."""
import argparse
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

SCENES = {
    'handgonne': 'integrated_handgonne', 'flower': 'flower',
    'ripper': 'ripper-07-mid-rip', 'grinwick': 'engine-03-rails-pulse',
    'kingdom': 'sweep-college', 'college': '02-great-hall',
    'ranch': '02-ranch', 'fields': '04-fields-160', 'privy': '12_Royal_24',
    'race': 'hippodrome_race', 'vote': 'party-averages',
}
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('directory', type=Path)
parser.add_argument('--plates', type=Path, default=Path('/work/artifacts/art-concepts/upgrade'))
args = parser.parse_args()
try:
    font = ImageFont.truetype('/usr/share/fonts/TTF/DejaVuSans.ttf', 18)
except OSError:
    font = ImageFont.load_default()
background = (14, 27, 40)
parts = ['<!doctype html><meta charset="utf-8"><title>Delve upgrade match</title>',
         '<style>body{background:#0e1b28;color:#edc477;font:16px system-ui;margin:24px}img{max-width:100%}a{color:#bdd0d2}figure{margin:24px 0}</style>',
         '<h1>Delve / upgrade match</h1><p>Today v1 | accepted plate | v2. All images uncropped. Runtime pairs share one simulation state. Plates have a taller aspect ratio. <a href="timings.csv">CPU timings</a></p>']
sheet = Image.new('RGB', (1200, 80 + len(SCENES) * 302 + 290), background)
sheet_draw = ImageDraw.Draw(sheet)
sheet_draw.text((18, 14), 'DELVE / UPGRADE MATCH — v1 | plate | v2', font=font, fill='#edc477')
for row, (name, plate_name) in enumerate(SCENES.items()):
    comparison = Image.new('RGB', (2352, 562), background)
    draw = ImageDraw.Draw(comparison)
    for column, (path, label) in enumerate([
        (args.directory / f'{name}-v1-00.png', 'TODAY V1'),
        (args.plates / f'{plate_name}-upgrade.png', 'ACCEPTED UPGRADE'),
        (args.directory / f'{name}-v2-00.png', 'ANGEL_ART=v2'),
    ]):
        with Image.open(path) as source:
            pic = source.convert('RGB')
            pic.thumbnail((768, 512), Image.Resampling.NEAREST)
        x = 8 + column * 784 + (768 - pic.width) // 2
        comparison.paste(pic, (x, 42 + (512 - pic.height) // 2))
        draw.text((12 + column * 784, 12), f'{name.upper()} / {label}', font=font, fill='#edc477')
    comparison.save(args.directory / f'{name}-three-way.png')
    preview = comparison.resize((1176, 281), Image.Resampling.NEAREST)
    sheet.paste(preview, (12, 64 + row * 302))
    parts.append(f'<figure><a href="{name}-three-way.png"><img src="{name}-three-way.png" alt="{name}: v1, plate, v2"></a></figure>')
with Image.open(args.plates / 'knights-sheet.png') as source:
    pic = source.convert('RGB'); pic.thumbnail((1176, 256), Image.Resampling.NEAREST)
sheet.paste(pic, ((1200 - pic.width) // 2, 80 + len(SCENES) * 302))
sheet.save(args.directory / 'contact-sheet.png')
parts.append('<figure><a href="figures-v2.png">Runtime figure study</a></figure>')
(args.directory / 'index.html').write_text('\n'.join(parts))
print(args.directory / 'contact-sheet.png')
