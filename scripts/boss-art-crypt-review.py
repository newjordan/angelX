#!/usr/bin/env python3
"""Native before/after rooms and actual cells for the Crypt batch."""
import argparse
import json
import math
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('output', type=Path)
p.add_argument('--before', type=Path, required=True)
a = p.parse_args()
out = a.output / 'review'
out.mkdir(parents=True, exist_ok=True)
font = ImageFont.truetype('/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf', 14)
names = ['waxen-warden', 'the-bone-choir', 'crypt-tithe-marshal']

def cells(path, box=(0, 0, 96, 28)):
    raw = json.loads(path.read_text())
    x0, y0, x1, y1 = box
    im = Image.new('RGB', ((x1-x0)*8, (y1-y0)*16))
    d = ImageDraw.Draw(im)
    for y in range(y0, y1):
        for x in range(x0, x1):
            c = raw[y*96+x]
            px, py = (x-x0)*8, (y-y0)*16
            d.rectangle((px, py, px+7, py+15), fill=tuple(c['bg']))
            d.text((px, py-2), c['glyph'], font=font, fill=tuple(c['fg']))
    return im

for name in names:
    sheet = Image.new('RGB', (1560, 1930), '#080b0e')
    d = ImageDraw.Draw(sheet)
    for row, (kit, label, source) in enumerate([
        (1, 'BEFORE', a.before), (1, 'AFTER', a.output),
        (2, 'BEFORE', a.before), (2, 'AFTER', a.output),
    ]):
        stem = source / 'terminal' / f'{name}-current-0-v{kit}'
        y = 12 + row*480
        d.text((10, y), f'{name} / V{kit} / {label} / NATIVE 1X', fill='#c2b494')
        d.text((790, y), 'ACTUAL HALF-BLOCK / BRAILLE CELLS', fill='#c2b494')
        sheet.paste(Image.open(str(stem)+'-rgb.png'), (10, y+20))
        sheet.paste(cells(Path(str(stem)+'-cells.json')), (790, y+20))
    sheet.save(out / (name+'-before-after.png'))
    # Every impact has a corresponding actual cell crop. Ten images per sheet.
    sheet = Image.new('RGB', (1540, 410), '#080b0e')
    d = ImageDraw.Draw(sheet)
    for stage in range(5):
        stem = a.output / 'terminal' / f'{name}-effects-{stage}-v2'
        crop = Image.open(str(stem)+'-rgb.png').crop((576,256,640,320))
        x = 12+stage*300
        d.text((x,12),f'IMPACT {stage} / 3x room',fill='#c2b494')
        sheet.paste(crop.resize((192,192),Image.Resampling.NEAREST),(x,36))
        d.text((x,254),f'IMPACT {stage} / ACTUAL CELLS',fill='#c2b494')
        sheet.paste(cells(Path(str(stem)+'-cells.json'),(72,16,80,20)),(x,284))
    sheet.save(out / (name+'-impact-cells.png'))

    # The default kit uses the same world anchors at half the RGB dimensions.
    layout = json.loads((a.output/'terminal/review-layout.json').read_text())['bosses'][name]
    box = layout['tell']
    native_box = tuple(v//2 for v in box)
    cell_box = (box[0]//8, box[1]//16, math.ceil(box[2]/8), math.ceil(box[3]/16))
    sheet = Image.new('RGB', (1540, 470), '#080b0e')
    d = ImageDraw.Draw(sheet)
    previous = None
    for stage in range(4):
        stem = a.output/'terminal'/f'{name}-effects-{stage}-v1'
        native = Image.open(str(stem)+'-rgb.png').crop(native_box)
        cell = cells(Path(str(stem)+'-cells.json'), cell_box)
        measured = cell.copy()
        md = ImageDraw.Draw(measured)
        for x0,y0,x1,y1 in layout['tell_ignore']:
            md.rectangle((x0//8*8-cell_box[0]*8,y0//16*16-cell_box[1]*16,
                          math.ceil(x1/8)*8-cell_box[0]*8-1,math.ceil(y1/16)*16-cell_box[1]*16-1),fill='#080b0e')
        if previous:
            diff = sum(x!=y for x,y in zip(previous.get_flattened_data(), measured.get_flattened_data()))
            assert diff, f'{name}: default-kit tell {stage-1}/{stage} is unreadable'
            print(f'{name}: V1 actual-cell tell {stage-1}->{stage} changes {diff} glyph pixels')
        previous = measured
        x = 12+stage*382
        d.text((x,12),f'V1 TELL {stage} / native 1x',fill='#c2b494')
        sheet.paste(native,(x,36))
        d.text((x,170),'V1 TELL / ACTUAL CELLS',fill='#c2b494')
        sheet.paste(cell,(x,196))
    sheet.save(out/(name+'-v1-tell-cells.png'))

    sheet = Image.new('RGB',(1540,480),'#080b0e')
    d = ImageDraw.Draw(sheet)
    for role,count,y in [('projectile',2,12),('impact',5,230)]:
        box = layout[role]
        cb = (box[0]//8,box[1]//16,math.ceil(box[2]/8),math.ceil(box[3]/16))
        for stage in range(count):
            stem = a.output/'terminal'/f'{name}-effects-{stage}-v1'
            x = 12+stage*300
            d.text((x,y),f'V1 {role.upper()} {stage} / ACTUAL CELLS',fill='#c2b494')
            sheet.paste(cells(Path(str(stem)+'-cells.json'),cb),(x,y+24))
            if role=='projectile' or stage in [0,2,4]:
                d.text((x,y+104),'NATIVE 1X',fill='#96a5a9')
                sheet.paste(Image.open(str(stem)+'-rgb.png').crop(tuple(v//2 for v in box)),(x,y+124))
    sheet.save(out/(name+'-v1-effects-cells.png'))
