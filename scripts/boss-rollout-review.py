#!/usr/bin/env python3
"""Before/after and size evidence using native RGB and emitted 96x28 cells."""
import argparse
import json
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
FONT = ImageFont.truetype('/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf', 14)


def cells(path):
    raw = json.loads(path.read_text())
    image = Image.new('RGB', (768, 448))
    draw = ImageDraw.Draw(image)
    for i, cell in enumerate(raw):
        x, y = i % 96 * 8, i // 96 * 16
        draw.rectangle((x, y, x+7, y+15), fill=tuple(cell['bg']))
        draw.text((x, y-2), cell['glyph'], font=FONT, fill=tuple(cell['fg']))
    return image


def row(sheet, index, label, stem):
    y = index * 480 + 28
    draw = ImageDraw.Draw(sheet)
    draw.text((10, y-20), label + ' / NATIVE 1X', fill='wheat')
    draw.text((798, y-20), 'ACTUAL 96x28 CELLS', fill='wheat')
    sheet.paste(Image.open(str(stem) + '-rgb.png'), (10, y))
    sheet.paste(cells(Path(str(stem) + '-cells.json')), (798, y))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('boss', nargs='+')
    args = parser.parse_args()
    out = args.output
    registry = json.loads((ROOT/'scripts/boss-library-registry.json').read_text())
    records = {r['name']: r for r in json.loads((out/'study/records.json').read_text())}
    review = []
    for boss in args.boss:
        sheet = Image.new('RGB', (1570, 2720), (8, 11, 14))
        for i, (kit, version) in enumerate((k, v) for k in (1, 2) for v in ('before', 'study')):
            folder = 'production' if version == 'study' else version
            label = 'AFTER' if version == 'study' else 'BEFORE'
            if version == 'study' and not registry[boss].get('active',True):
                label += ' (OLD ART RETAINED)'
            stem = out/folder/f'{boss}-sample-0-133-v{kit}'
            row(sheet, i, f'{boss} V{kit} / {label} / FOUR SEATS + SHOTS', stem)
        # The unchanged tick95 fixture isolates identity, at twice the actual cell grain.
        for i, kit in enumerate((1, 2)):
            stem = out/'study'/f'{boss}-sample-0-0-v{kit}'
            box = (144*kit, 22*kit, 240*kit, 105*kit)
            rgb = Image.open(str(stem)+'-rgb.png').crop(box)
            cell = cells(Path(str(stem)+'-cells.json')).crop(tuple(v*2//kit for v in box))
            y = 1948+i*380
            label = 'ACCEPTED' if registry[boss].get('active',True) else 'REJECTED CANDIDATE'
            ImageDraw.Draw(sheet).text((10,y-20), f'V{kit} {label} IDENTITY / RGB 2X + ACTUAL CELLS 2X',fill='wheat')
            sheet.paste(rgb.resize((rgb.width*2,rgb.height*2),Image.Resampling.NEAREST),(10,y))
            sheet.paste(cell.resize((cell.width*2,cell.height*2),Image.Resampling.NEAREST),(798,y))
        name = f'{boss}-combat.png'
        sheet.save(out/name)
        review.append(name)
        sheet = Image.new('RGB', (1570, 2880), (8, 11, 14))
        for i, (kit, sample) in enumerate([(1,1),(1,2),(1,4),(2,1),(2,2),(2,3)]):
            name = f'{boss}-sample-{sample}-133-v{kit}'
            record = records[name]
            row(sheet,i,f'{boss} V{kit} {record["size"]} / {record["shots"]} SHOTS / FOUR SEATS',out/'study'/name)
        name = f'{boss}-study.png'
        sheet.save(out/name)
        review.append(name)
        paths = [out/'live'/f'{boss}-v{kit}.png' for kit in (1,2)]
        if all(p.exists() for p in paths):
            sheet = Image.new('RGB',(1792,530),(8,11,14))
            for i,path in enumerate(paths):
                ImageDraw.Draw(sheet).text((i*896+10,6),f'{boss} V{i+1}: BROWSER + THREE MOVING/FIRING BOTS',fill='wheat')
                sheet.paste(Image.open(path).resize((896,504),Image.Resampling.NEAREST),(i*896,26))
            name=f'{boss}-live.png'
            sheet.save(out/name)
            review.append(name)
    (out/'review-images.json').write_text(json.dumps(review,indent=2)+'\n')


if __name__ == '__main__':
    main()
