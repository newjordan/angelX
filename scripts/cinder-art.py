#!/usr/bin/env python3
"""Convert Crock and the slag lifecycle to crisp palette sprites; keep source shape."""
import argparse
from pathlib import Path
from PIL import Image, ImageDraw

PALETTE = [0x132724, 0x192934, 0x263b40, 0x2f4a52, 0x3a6d5d, 0x6e8a8e,
           0x331a0c, 0x45291c, 0x582b13, 0x652c10, 0x784620, 0x924f1d,
           0xa97639, 0xbe914b, 0xd8a95e, 0xe2cb8b]
COLOURS = [((p >> 16) & 255, (p >> 8) & 255, p & 255) for p in PALETTE]

def convert(im, size):
    im = im.convert('RGBA')
    # The source's neutral backdrop has no saturation. Keep coloured contours.
    px = im.load()
    for y in range(im.height):
        for x in range(im.width):
            r, g, b, a = px[x, y]
            if max(r, g, b) - min(r, g, b) < 24:
                px[x, y] = (0, 0, 0, 0)
    im = im.crop(im.getbbox())
    im.thumbnail(size, Image.Resampling.NEAREST)
    out = Image.new('RGBA', size)
    out.alpha_composite(im, ((size[0]-im.width)//2, size[1]-im.height))
    px = out.load()
    cache = {}
    for y in range(out.height):
        for x in range(out.width):
            r, g, b, a = px[x,y]
            if not a: continue
            key = (r, g, b)
            if key not in cache:
                cache[key] = min(COLOURS, key=lambda c: sum((c[i]-key[i])**2 for i in range(3)))
            px[x,y] = (*cache[key], 255)
    return out

def clean(im):
    """Drop disconnected single pixels and keep a selective coloured contour."""
    px=im.load()
    singles=[]
    for y in range(im.height):
        for x in range(im.width):
            if not px[x,y][3]: continue
            if not any(0<=x+dx<im.width and 0<=y+dy<im.height and px[x+dx,y+dy][3]
                       for dx,dy in [(-1,0),(1,0),(0,-1),(0,1)]): singles.append((x,y))
    for x,y in singles: px[x,y]=(0,0,0,0)
    return im

def panels(path, cols, rows, size):
    src=Image.open(path)
    return [clean(convert(src.crop((x*src.width//cols,y*src.height//rows,
              (x+1)*src.width//cols,(y+1)*src.height//rows)),size))
            for y in range(rows) for x in range(cols)]

def review(images, path, columns=4):
    sheet=Image.new('RGB',(columns*268,((len(images)+columns-1)//columns)*276),(0,0,0))
    draw=ImageDraw.Draw(sheet)
    for i,(name,im) in enumerate(images):
        x=(i%columns)*268+10; y=(i//columns)*276
        draw.text((x,y+6),name,fill=(176,178,168))
        large=im.resize((im.width*3,im.height*3),Image.Resampling.NEAREST)
        sheet.paste(large,(x,y+28),large)
    sheet.save(path)

def refine(base):
    """Author connected seams over the approved ceramic, never repaint its ramp."""
    teal = '#132724'
    seams = [[(29, 34), (25, 41), (29, 47), (27, 56), (33, 63)],
             [(55, 34), (59, 41), (55, 48), (59, 56), (54, 63)],
             [(30, 65), (36, 62), (43, 68), (51, 65)]]
    images = []
    for stage in range(1, 4):
        out = base.copy()
        overlay = Image.new('RGBA', base.size)
        d = ImageDraw.Draw(overlay)
        for seam in seams[:1 if stage == 1 else 3]:
            d.line(seam, fill=teal, width=3 if stage < 3 else 5, joint='curve')
            if stage == 3:
                d.line(seam, fill='#d8a95e', width=2, joint='curve')
        # The seam masks cannot grow the creature's footprint.
        overlay.putalpha(Image.composite(overlay.getchannel('A'),
                                        Image.new('L', base.size), base.getchannel('A')))
        out.alpha_composite(overlay)
        if stage == 3:
            # Two attached plates, a missing wedge and a warm exposed vent.
            # Retain the original lid pixels; the reference informs only the split.
            out.paste((0, 0, 0, 0), (25, 0, 57, 12))
            lid = base.crop((26, 0, 55, 11))
            out.alpha_composite(lid.crop((0, 0, 15, 11)), (24, 0))
            out.alpha_composite(lid.crop((18, 0, 29, 11)), (46, 3))
            d = ImageDraw.Draw(out)
            d.polygon([(38, 3), (42, 5), (40, 8), (44, 10), (39, 12), (37, 8)], fill=teal)
            d.line([(40, 4), (41, 6), (40, 8), (43, 10)], fill='#d8a95e', width=2)
            d.line([(33, 12), (39, 14), (47, 13)], fill='#a97639', width=2)
        images.append((f'crock-crack-{stage}', out))
    for frame, dx in [(1, 2), (2, -2)]:
        out = base.copy()
        head = base.crop((0, 0, 80, 35))
        d = ImageDraw.Draw(head)
        # Squint within the same eye sockets; keep brows, cheeks and nose.
        d.polygon([(29, 24), (36, 27), (38, 30), (32, 29)], fill='#be914b')
        d.polygon([(45, 29), (49, 25), (53, 25), (51, 29)], fill='#be914b')
        d.line([(29, 25), (33, 27), (37, 28)], fill=teal, width=2)
        d.line([(45, 28), (49, 27), (52, 25)], fill=teal, width=2)
        # Translate the head rigidly by two pixels; jolt the lid two pixels sideways.
        lid = head.crop((25, 0, 57, 11))
        head.paste((0, 0, 0, 0), (25, 0, 57, 11))
        head.alpha_composite(lid, (23 if frame == 1 else 27, 0))
        out.paste((0, 0, 0, 0), (0, 0, 80, 35))
        out.alpha_composite(head, (dx, 0))
        images.append((f'flinch-{frame}', out))
    impact = Image.new('RGBA', (40, 24))
    d = ImageDraw.Draw(impact)
    # Angular ceramic plates splay out from one low connected molten splash.
    d.polygon([(2, 18), (8, 14), (12, 15), (16, 11), (20, 15), (25, 13),
               (29, 17), (37, 19), (34, 22), (7, 22)], fill='#784620')
    d.polygon([(7, 18), (13, 16), (17, 14), (21, 17), (26, 16), (32, 20),
               (11, 20)], fill='#be914b')
    d.polygon([(13, 18), (17, 16), (22, 18), (26, 18), (23, 20), (16, 20)], fill='#d8a95e')
    for shape in [[(2, 9), (8, 11), (10, 16), (4, 14)],
                  [(29, 7), (36, 5), (34, 12), (29, 14)],
                  [(16, 6), (19, 9), (18, 12), (15, 10)]]:
        d.polygon(shape, fill='#2f4a52')
        inner = [(x, y-2) for x, y in shape]
        d.polygon(inner, fill='#a97639')
    images.append(('impact-3', impact))
    for frame in range(3):
        burst = Image.new('RGBA', (24, 24))
        d = ImageDraw.Draw(burst)
        radius = [7, 12, 5][frame]
        # Filled concentric clusters; no outline square and no dither.
        d.ellipse((12-radius, 12-radius, 12+radius, 12+radius), fill='#784620')
        d.ellipse((14-radius, 14-radius, 10+radius, 10+radius), fill='#be914b')
        core = [3, 4, 2][frame]
        d.ellipse((12-core, 12-core, 12+core, 12+core), fill='#d8a95e')
        for x, y in [(1, 10), (19, 10), (10, 1), (10, 19)]:
            d.polygon([(x, y+1), (x+2, y), (x+3, y+2), (x+1, y+3)], fill='#3a6d5d')
        images.append((f'clear-{frame+1}', burst))
    # A broad dark eye pair survives the lab copy's nearest reduction.
    small = base.resize((28, 28), Image.Resampling.NEAREST)
    d = ImageDraw.Draw(small)
    d.line([(10, 9), (12, 10)], fill=teal, width=2)
    d.line([(16, 10), (18, 9)], fill=teal, width=2)
    d.point((11, 9), fill='#e2cb8b')
    d.point((17, 9), fill='#e2cb8b')
    d.rectangle((11, 17, 16, 20), fill=teal)
    d.rectangle((13, 19, 15, 20), fill='#be914b')
    images.append(('crock-small', small))
    for stage in range(1, 4):
        mask = Image.new('RGBA', base.size)
        d = ImageDraw.Draw(mask)
        # Four-pixel connected steps align with the half-block sampler.
        paths = [[(28, 34), (28, 42), (24, 42), (24, 48), (28, 48),
                  (28, 58), (32, 58), (32, 64)],
                 [(56, 34), (56, 42), (60, 42), (60, 48), (56, 48),
                  (56, 58), (52, 58), (52, 64)],
                 [(32, 64), (36, 64), (36, 68), (44, 68), (44, 64), (48, 64)]]
        for seam in paths[:1 if stage == 1 else 3]:
            aligned = [(x-1, y) for x, y in seam]
            d.line(aligned, fill=teal, width=5 if stage == 3 else 4)
            if stage == 3:
                d.line(aligned, fill='#d8a95e', width=1)
        images.append((f'seam-cells-{stage}', mask))
    steam = Image.new('RGBA', (56, 36))
    d = ImageDraw.Draw(steam)
    # Open curls, each with a two-cell hole, stay separate beside the feet.
    for outer, inner, opening in [((2, 12, 24, 32), (8, 18, 18, 26), (16, 24, 25, 35)),
                                  ((32, 10, 54, 32), (38, 16, 48, 25), (31, 24, 40, 35))]:
        d.ellipse(outer, fill='#2f4a52')
        d.arc(outer, 180, 290, fill='#3a6d5d', width=4)
        d.ellipse(inner, fill=(0, 0, 0, 0))
        d.rectangle(opening, fill=(0, 0, 0, 0))
    images.append(('tell-cells', steam))
    return images

if __name__ == '__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('boss',type=Path,nargs='?');p.add_argument('effects',type=Path,nargs='?')
    p.add_argument('--out',type=Path,default=Path('crates/delve/assets/cinder'))
    p.add_argument('--review',type=Path,required=True)
    p.add_argument('--states',type=Path,help='Send-back source folder: cracks/actions/effects.png')
    p.add_argument('--draft',action='store_true')
    p.add_argument('--refine',action='store_true',help='Scripted second send-back, from crock.png only')
    a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
    if a.refine:
        images = refine(Image.open(a.out/'crock.png').convert('RGBA'))
        for name, im in images: im.save(a.out/(name+'.png'))
        review([('base',Image.open(a.out/'crock.png'))]+images[:5]+[images[9]], a.review)
        review(images[5:9]+images[10:], a.review.with_name('effects-3x.png'))
    elif a.states:
        base=Image.open(a.out/'crock.png').convert('RGBA')
        cracks=panels(a.states/'cracks.png',3,1,(80,80))
        poses=panels(a.states/'actions.png',2,2,(80,80))
        if a.draft:
            review([('base',base)]+[(f'damage-{i+1}',im) for i,im in enumerate(cracks)]+
                    [(f'pose-{i+1}',im) for i,im in enumerate(poses)],a.review)
            raise SystemExit()
        images=[]
        # Only the generated crack seams replace existing ceramic. All other
        # body, face, hands and feet pixels remain the approved base sprite.
        seams=[[(18,29),(22,34),(23,42)],
               [(55,35),(58,43),(54,50),(49,57),(48,67),(40,73),(35,65),(28,60)],
               [(20,37),(16,42),(21,48),(23,56)]]
        for stage in range(3):
            out=base.copy()
            for j in range(stage+1):
                mask=Image.new('L',(80,80)); d=ImageDraw.Draw(mask)
                d.line(seams[j],fill=255,width=6)
                # Preserve the source's connected bright/dark crack cluster.
                out.paste(cracks[stage],(0,0),mask)
            if stage==2:
                out.paste(cracks[stage].crop((0,0,80,35)),(0,0))
            images.append((f'crock-crack-{stage+1}',clean(out)))
        for i,pose in enumerate(poses):
            out=base.copy()
            # Head/lid/eyes only: keep the original body below the eyebrows.
            out.paste((0,0,0,0),(0,0,80,35))
            head=pose.crop((0,0,80,35))
            out.paste(head,(0,0))
            images.append((('flinch-' if i<2 else 'windup-')+str(i%2+1),clean(out)))
        images.append(('crock-small',clean(base.resize((28,28),Image.Resampling.NEAREST))))
        if (a.states/'effects.png').exists():
            src=Image.open(a.states/'effects.png')
            sizes=[(56,36),(18,24)]+[(40,24)]*4
            names=['tell','slag']+[f'impact-{i}' for i in range(1,5)]
            for i,(name,size) in enumerate(zip(names,sizes)):
                x=i%3;y=i//3
                left=[0,550,1024][x]; right=[550,1024,1536][x]
                crop=src.crop((left,0 if y==0 else 570,right,570 if y==0 else src.height))
                images.append((name,clean(convert(crop,size))))
        for name,im in images: im.save(a.out/(name+'.png'))
        review(images[:7],a.review)
        review(images[8:],a.review.with_name('effects-3x.png'))
    else:
        images=[('crock',convert(Image.open(a.boss),(80,80)))]
        vfx=Image.open(a.effects)
        for name,size,left,right in [('tell',(24,24),0,.32),('slag',(16,24),.36,.60),('impact',(32,18),.64,1)]:
            crop=vfx.crop((int(left*vfx.width),0,int(right*vfx.width),vfx.height))
            images.append((name,convert(crop,size)))
        for name,im in images: im.save(a.out/(name+'.png'))
        review(images,a.review)
    print(a.review)
