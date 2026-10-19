#!/usr/bin/env python3
"""Bounded native/cell room sheets with the actual paired floor beneath each body."""
import argparse
import json
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('captures', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--boss', nargs='+', help='review only these delivered IDs')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    fixtures = json.loads((args.captures/'floor-contrast-fixtures.json').read_text())
    review = []
    for boss in dict.fromkeys(f['boss'] for f in fixtures):
        if args.boss and boss not in args.boss:
            continue
        for pose in ('rest','windup'):
            sheet = Image.new('RGB',(1570,1220),(8,11,14))
            draw = ImageDraw.Draw(sheet)
            for kit in (1,2):
                record = next(f for f in fixtures if (f['boss'],f['kit'],f['pose']) == (boss,kit,pose))
                stem = args.captures/record['name']
                y = (kit-1)*610+25
                draw.text((10,y-20),f'{boss} V{kit} {pose}: GENERATED {record["pack"]} GUARDIAN HALL / FOUR SEATS / NATIVE 1X',fill='wheat')
                draw.text((798,y-20),'ACTUAL 96x28 HALF-BLOCK / BRAILLE CELLS',fill='wheat')
                sheet.paste(Image.open(str(stem)+'-rgb.png'),(10,y))
                sheet.paste(cells(Path(str(stem)+'-cells.json')),(798,y))
                # Eight images per sheet. Floors are shown separately so the
                # reviewer can check that the sample uses the same room/grade.
                draw.text((10,y+452),'PAIRED GRADED FLOOR (DOWNSIZED RGB / CELLS)',fill='wheat')
                for x,im in [(10,Image.open(str(stem)+'-floor-rgb.png')),
                             (222,cells(Path(str(stem)+'-floor-cells.json')))]:
                    sheet.paste(im.resize((192,112),Image.Resampling.NEAREST),(x,y+472))
            name = f'{boss}-{pose}-rooms.png'
            sheet.save(args.output/name)
            review.append(name)
        if boss in ('waxen-warden','the-bone-choir'):
            sheet = Image.new('RGB',(1240,1040),(8,11,14))
            draw = ImageDraw.Draw(sheet)
            for row,(kit,pose) in enumerate((k,p) for k in (1,2) for p in ('rest','windup')):
                record = next(f for f in fixtures if (f['boss'],f['kit'],f['pose']) == (boss,kit,pose))
                stem = args.captures/record['name']
                x,y = record['origin']; w,h = record['size']
                box = (x-4*kit,y-4*kit,x+w+4*kit,y+h+4*kit)
                rgb = Image.open(str(stem)+'-rgb.png').crop(box)
                cell = cells(Path(str(stem)+'-cells.json')).crop(tuple(v*2//kit for v in box))
                yy = row*260+25
                draw.text((10,yy-20),f'{boss} V{kit} {pose} / RGB 2X AND EMITTED CELL CROP 2X',fill='wheat')
                for xx,im in [(10,rgb),(550,cell)]:
                    sheet.paste(im.resize((im.width*2,im.height*2),Image.Resampling.NEAREST),(xx,yy))
            name = f'{boss}-identity.png'
            sheet.save(args.output/name)
            review.append(name)
    (args.output/'review-images.json').write_text(json.dumps(review,indent=2)+'\n')


if __name__ == '__main__':
    main()
