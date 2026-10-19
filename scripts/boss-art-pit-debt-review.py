#!/usr/bin/env python3
"""Identical-room identity sheets and complete effect crops for both kits."""
import argparse
import json
import math
import runpy
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
A = runpy.run_path(str(ROOT / 'scripts/boss-art.py'))
FONT = ImageFont.truetype('/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf', 14)


def cells(path, box):
    raw = json.loads(path.read_text())
    x0, y0, x1, y1 = box[0] // 8, box[1] // 16, math.ceil(box[2] / 8), math.ceil(box[3] / 16)
    im = Image.new('RGB', ((x1 - x0) * 8, (y1 - y0) * 16))
    draw = ImageDraw.Draw(im)
    for y in range(y0, y1):
        for x in range(x0, x1):
            c = raw[y * 96 + x]
            px, py = (x - x0) * 8, (y - y0) * 16
            draw.rectangle((px, py, px + 7, py + 15), fill=tuple(c['bg']))
            draw.text((px, py - 2), c['glyph'], font=FONT, fill=tuple(c['fg']))
    return im


def label(draw, x, y, text):
    draw.text((x, y), text, font=FONT, fill='#bdb8a2')


def source(entries, output, tag):
    sheet = Image.new('RGB', (1560, 960), '#080b0e')
    draw = ImageDraw.Draw(sheet)
    for i, entry in enumerate(entries):
        name = Path(entry['boss']).stem
        old = A['render'](A['original_rows'](entry), A['palette']())
        new = A['render'](entry['frames']['v1'][0]['rows'], A['frame_inks'](entry['frames']['v1'][0], A['palette']()))
        study = A['render'](entry['frames']['v2'][0]['rows'], A['frame_inks'](entry['frames']['v2'][0], A['palette']()))
        grey = Image.new('RGBA', new.size, '#858d8e'); grey.putalpha(new.getchannel('A'))
        for j, (title, im, scale) in enumerate([('ORIGINAL', old, 6), ('CLEANUP', new, 6), ('STUDY', study, 4), ('GREY SILHOUETTE', grey, 6)]):
            x, y = 12 + j * 388, 12 + i * 316
            label(draw, x, y, name); label(draw, x, y + 20, title)
            im = im.resize((im.width * scale, im.height * scale), Image.Resampling.NEAREST)
            sheet.paste(im, (x, y + 46), im)
    sheet.save(output / f'qa-source-{tag}.png')
    # Six nearest candidates, each shown at source size and 3x: 12 views.
    sheet = Image.new('RGB', (1560, 460), '#080b0e'); draw = ImageDraw.Draw(sheet)
    for i, entry in enumerate(entries):
        name = Path(entry['boss']).stem
        for j, role in enumerate(('body', 'impact')):
            p = output / 'concepts/nearest' / f'{name}-{role}.png'
            if not p.exists(): continue
            im = Image.open(p); x, y = 12 + i * 516, 12 + j * 220
            label(draw, x, y, f'{name} / REJECTED {role}')
            sheet.paste(im, (x, y + 35), im)
            im = im.resize((im.width * 3, im.height * 3), Image.Resampling.NEAREST)
            sheet.paste(im, (x + 100, y + 35), im)
    sheet.save(output / 'qa-nearest.png')
    for entry in entries:
        name = Path(entry['boss']).stem
        # Eleven separate frames per sheet, no labels colliding with art.
        sheet = Image.new('RGB', (1560, 450), '#080b0e'); draw = ImageDraw.Draw(sheet)
        index = 0
        for role in ('projectile', 'tell', 'impact'):
            for stage, spec in enumerate(entry['frames'][role]):
                x, y = 12 + index % 6 * 258, 12 + index // 6 * 220
                label(draw, x, y, f'{role} {stage}')
                im = A['render'](A['signature_rows'](spec), A['frame_inks'](spec, A['palette']()))
                im = im.resize((im.width * 5, im.height * 5), Image.Resampling.NEAREST)
                sheet.paste(im, (x, y + 30), im); index += 1
        sheet.save(output / f'qa-{name}-effects-{tag}.png')


def review(entries, output):
    target = output / 'review'; target.mkdir(exist_ok=True)
    layout = json.loads((output / 'terminal/review-layout.json').read_text())['bosses']
    for entry in entries:
        name = Path(entry['boss']).stem; ev = entry['frames']['v2'][0]['separation']; g = layout[name]
        for kit in (1, 2):
            divisor = 2 if kit == 1 else 1
            bx, by = ev['room_origin']; w, h = ev['room_size']; box = [bx - 8, by - 8, bx + w + 8, by + h + 8]
            sheet = Image.new('RGB', (1560, 1020), '#080b0e'); draw = ImageDraw.Draw(sheet)
            for row, kind in enumerate(('old', 'boss4', 'current')):
                stem = (output / 'before/terminal' / f'{name}-current-0-v{kit}') if kind == 'boss4' else (output / 'terminal' / f'{name}-{kind}-0-v{kit}')
                rgb = Image.open(str(stem) + '-rgb.png').crop(tuple(v // divisor for v in box))
                cell = cells(Path(str(stem) + '-cells.json'), box)
                y = 12 + row * 340
                label(draw, 12, y, f'{name} / V{kit} / {"ORIGINAL" if kind == "old" else "BOSS-4" if kind == "boss4" else "NEW"}')
                for x, title, im in [(12, '3x identical room', rgb.resize((rgb.width * 3, rgb.height * 3), Image.Resampling.NEAREST)),
                                      (440, 'native 1x', rgb),
                                      (800, 'actual cells 2x - judge first', cell.resize((cell.width * 2, cell.height * 2), Image.Resampling.NEAREST))]:
                    label(draw, x, y + 22, title); sheet.paste(im, (x, y + 48))
            sheet.save(target / f'{name}-v{kit}.png')
            # Six actual-cell views per sheet, including every warning stage.
            row_height = max(180, g['tell'][3] - g['tell'][1] + 64)
            sheet = Image.new('RGB', (1560, 4 * row_height + 20), '#080b0e'); draw = ImageDraw.Draw(sheet)
            for stage in range(4):
                stem = output / 'terminal' / f'{name}-effects-{stage}-v{kit}'
                box = g['tell']; cell = cells(Path(str(stem) + '-cells.json'), box)
                rgb = Image.open(str(stem) + '-rgb.png').crop(tuple(v // divisor for v in box))
                y = 12 + stage * row_height
                label(draw, 12, y, f'V{kit} / TELL {stage} / actual cells'); sheet.paste(cell, (12, y + 26))
                label(draw, 500, y, 'native 1x'); sheet.paste(rgb, (500, y + 26))
                if stage < 2:
                    box = g['projectile']; shot = cells(Path(str(stem) + '-cells.json'), box)
                    label(draw, 1000, y, f'PROJECTILE {stage} / actual cells 2x')
                    sheet.paste(shot.resize((shot.width * 2, shot.height * 2), Image.Resampling.NEAREST), (1000, y + 26))
            sheet.save(target / f'{name}-v{kit}-shots-tells.png')
            if 'laser' in g:
                sheet = Image.new('RGB', (1560, 650), '#080b0e'); draw = ImageDraw.Draw(sheet)
                for stage in range(5):
                    stem = output / 'terminal' / f'{name}-effects-{stage}-v{kit}'
                    cell = cells(Path(str(stem) + '-cells.json'), g['laser']); x = 12 + stage * 306
                    label(draw, x, 12, f'V{kit} Laser {stage}'); sheet.paste(cell, (x, 42))
                sheet.save(target / f'{name}-v{kit}-laser.png')
        sheet = Image.new('RGB', (1560, 560), '#080b0e'); draw = ImageDraw.Draw(sheet)
        for kit in (1, 2):
            for stage in range(5):
                stem = output / 'terminal' / f'{name}-effects-{stage}-v{kit}'
                cell = cells(Path(str(stem) + '-cells.json'), g['impact']); x, y = 12 + stage * 310, 12 + (kit - 1) * 270
                label(draw, x, y, f'V{kit} IMPACT {stage} / cells 2x')
                sheet.paste(cell.resize((cell.width * 2, cell.height * 2), Image.Resampling.NEAREST), (x, y + 30))
        sheet.save(target / f'{name}-impacts.png')
        if name == 'late-fee-leviathan':
            for kit in (1, 2):
                if not (output / 'terminal' / f'{name}-ravage-36-v{kit}-cells.json').exists():
                    continue
                sheet = Image.new('RGB', (1560, 1390), '#080b0e'); draw = ImageDraw.Draw(sheet)
                for index, age in enumerate((36, 39, 42, 45, 48, 52, 60, 68, 80)):
                    path = output / 'terminal' / f'{name}-ravage-{age}-v{kit}-cells.json'
                    cell = cells(path, g['tell']); x, y = 12 + index % 3 * 516, 12 + index // 3 * 460
                    label(draw, x, y, f'V{kit} Ravage / true age {age}'); sheet.paste(cell, (x, y + 28))
                sheet.save(target / f'{name}-v{kit}-ravage-beats.png')
    # Busy rooms show the overlaps with actors, names and HUD in both paths.
    for kit in (1, 2):
        sheet = Image.new('RGB', (1560, 1450), '#080b0e'); draw = ImageDraw.Draw(sheet)
        for row, entry in enumerate(entries):
            name = Path(entry['boss']).stem; y = 12 + row * 480
            stem = output / 'terminal' / f'{name}-current-133-v{kit}'
            label(draw, 12, y, f'{name} / V{kit} busy room / native 1x')
            label(draw, 790, y, 'actual half-block / braille cells')
            sheet.paste(Image.open(str(stem) + '-rgb.png'), (12, y + 26))
            sheet.paste(cells(Path(str(stem) + '-cells.json'), (0, 0, 768, 448)), (790, y + 26))
        sheet.save(target / f'busy-v{kit}.png')


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('output', type=Path)
    p.add_argument('--tag', default='current')
    p.add_argument('--source-only', action='store_true')
    args = p.parse_args()
    entries = json.loads((ROOT / 'scripts/boss-art-pit-debt.json').read_text())
    source(entries, args.output, args.tag)
    if not args.source_only:
        review(entries, args.output)
