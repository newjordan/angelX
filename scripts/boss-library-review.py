#!/usr/bin/env python3
"""Bounded review sheets from the exact RGB frames and real emitted cells."""
import argparse, json, runpy
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont
ROOT=Path(__file__).resolve().parents[1]
A=runpy.run_path(str(ROOT/'scripts/boss-art.py'))
I=runpy.run_path(str(ROOT/'scripts/boss-library-import.py'))
p=argparse.ArgumentParser(description=__doc__);p.add_argument('output',type=Path);a=p.parse_args();out=a.output
font=ImageFont.truetype('/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf',14)

def cells(path):
    raw=json.loads(path.read_text());im=Image.new('RGB',(768,448));d=ImageDraw.Draw(im)
    for i,c in enumerate(raw):
        x=i%96*8;y=i//96*16;d.rectangle((x,y,x+7,y+15),fill=tuple(c['bg']));d.text((x,y-2),c['glyph'],font=font,fill=tuple(c['fg']))
    return im

# Twelve native frames, each shown at 3x; in-room plates below show native 1x.
sheet=Image.new('RGB',(1540,720),(8,11,14));d=ImageDraw.Draw(sheet)
for row,boss in enumerate(I['BOSSES']):
    for col,(kit,pose) in enumerate([(1,'rest'),(1,'windup'),(2,'rest'),(2,'windup')]):
        q=Image.open(ROOT/'cockpit/assets/dungeon/bosses'/boss/f'library-v{kit}-{pose}.png').convert('RGBA')
        x=col*380+12;y=row*235+35;d.text((x,y-23),f'{boss} V{kit} {pose} / 3x',fill='wheat')
        q=q.resize((q.width*3,q.height*3),Image.Resampling.NEAREST);sheet.paste(q,(x,y),q)
sheet.save(out/'native-3x.png')
for boss in I['BOSSES']:
    sheet=Image.new('RGB',(1570,2880),(8,11,14));d=ImageDraw.Draw(sheet)
    for row,(label,folder,variant,kit) in enumerate([(label,folder,variant,kit) for label,folder,variant in [('ORIGINAL 7d711a03','before','old'),('EARLIER REDO','before','current'),('LIBRARY','after','current')] for kit in (1,2)]):
        stem=out/folder/'terminal'/f'{boss}-{variant}-0-v{kit}'
        rgb=Image.open(str(stem)+'-rgb.png');terminal=cells(Path(str(stem)+'-cells.json'))
        y=row*480+28;d.text((10,y-20),f'{boss} / {label} / V{kit} NATIVE 1X',fill='wheat');d.text((796,y-20),'ACTUAL HALF-BLOCK / BRAILLE CELLS',fill='wheat')
        sheet.paste(rgb,(10,y));sheet.paste(terminal,(796,y))
    sheet.save(out/(boss+'-rooms.png'))
# Busy rooms at tick133: six RGB and six cell views, same authoritative rooms.
sheet=Image.new('RGB',(1570,2880),(8,11,14));d=ImageDraw.Draw(sheet)
for row,(boss,kit) in enumerate((b,k) for b in I['BOSSES'] for k in (1,2)):
    stem=out/'after/terminal'/f'{boss}-current-133-v{kit}';y=row*480+28
    d.text((10,y-20),f'{boss} V{kit} / LIVE RULES TICK133',fill='wheat')
    sheet.paste(Image.open(str(stem)+'-rgb.png'),(10,y));sheet.paste(cells(Path(str(stem)+'-cells.json')),(796,y))
sheet.save(out/'combat.png')
# Existing tell/projectile/impact bank beside the new body, twelve views.
sheet=Image.new('RGB',(1540,720),(8,11,14));d=ImageDraw.Draw(sheet)
for row,boss in enumerate(I['BOSSES']):
    for col,role in enumerate(['library-v2-rest','tell-2','projectile','impact-2']):
        q=Image.open(ROOT/'cockpit/assets/dungeon/bosses'/boss/(role+'.png')).convert('RGBA');q=q.resize((q.width*3,q.height*3),Image.Resampling.NEAREST)
        x=col*380+12;y=row*235+35;d.text((x,y-23),f'{boss} / {role} / 3x',fill='wheat');sheet.paste(q,(x,y),q)
sheet.save(out/'effects-context.png')
