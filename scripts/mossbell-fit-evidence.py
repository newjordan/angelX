#!/usr/bin/env python3
"""Bounded before/after contacts from native frames and actual cockpit cells."""
import json
import sys
import subprocess
from pathlib import Path
from PIL import Image, ImageChops, ImageDraw, ImageFont

root=Path(sys.argv[1])
font=ImageFont.truetype(subprocess.check_output(['fc-match','monospace','-f','%{file}']).decode(),13)

subprocess.run(['cmp',root/'before/four-bot.trace',root/'after/four-bot.trace'],check=True)
for before in (root/'before').glob('*.png'):
    diff=ImageChops.difference(Image.open(before).convert('RGB'),Image.open(root/'after'/before.name).convert('RGB'))
    d=ImageDraw.Draw(diff)
    for box in [(75,118,303,128),(160,66,224,89),(120,154,263,179)]:d.rectangle(box,fill=0)
    assert diff.getbbox() is None, f'{before.name}: pixels changed outside the three piece rectangles'
for stage in range(4):
    before=json.loads((root/'before'/f'crown-{stage}-cells.json').read_text())
    after=json.loads((root/'after'/f'crown-{stage}-cells.json').read_text())
    for y in range(8,12):
        for x in range(40,57):
            assert before[y*96+x]==after[y*96+x], f'crown {stage}: old-art cell fallback changed'


def cells(path):
    im=Image.new('RGB',(768,448));d=ImageDraw.Draw(im)
    for i,c in enumerate(json.loads(path.read_text())):
        x=i%96*8;y=i//96*16;fg=tuple(c['fg']);bg=tuple(c['bg']);ch=c['glyph']
        d.rectangle((x,y,x+7,y+15),fill=bg)
        if ch=='▀':d.rectangle((x,y,x+7,y+7),fill=fg)
        elif ch and 0x2800<=ord(ch[0])<=0x28ff:
            bits=ord(ch[0])-0x2800
            for k,(dx,dy) in enumerate([(0,0),(0,1),(0,2),(1,0),(1,1),(1,2),(0,3),(1,3)]):
                if bits&(1<<k):d.rectangle((x+dx*4+1,y+dy*4+1,x+dx*4+2,y+dy*4+2),fill=fg)
        else:d.text((x,y),ch,font=font,fill=fg)
    return im


for kind,names in [('root',[f'root-{n}' for n in range(4)]),('crown',[f'crown-{n}' for n in range(4)]),
                   ('flippers',[f'flippers-{n}' for n in range(4)]),('seats',[f'seat-{n}' for n in range(1,5)])]:
    for mode in ['native','cells']:
        w,h=(384,224) if mode=='native' else (768,448)
        sheet=Image.new('RGB',(w*2,(h+22)*4),(11,10,8));d=ImageDraw.Draw(sheet)
        for i,name in enumerate(names):
            for j,phase in enumerate(['before','after']):
                im=Image.open(root/phase/f'{name}.png') if mode=='native' else cells(root/phase/f'{name}-cells.json')
                sheet.paste(im,(j*w,i*(h+22)+22));d.text((j*w+4,i*(h+22)+3),f'{phase} {name}',font=font,fill='wheat')
        sheet.save(root/f'{kind}-{mode}.png')
print('Eight contacts; native 1x and actual 96x28 half-block/braille cells, eight frames per contact')
