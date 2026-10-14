#!/usr/bin/env python3
"""Assemble real-terminal v1 | last v2 | new native | new cells comparisons."""
import argparse
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

SCENES=['handgonne','flower','ripper','grinwick','kingdom','college','ranch','fields','privy','race','vote']
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('directory',type=Path)
a=p.parse_args();d=a.directory
import subprocess
font=ImageFont.truetype(subprocess.check_output(['fc-match','-f','%{file}','DejaVu Sans']).decode(),18)
parts=['<!doctype html><meta charset="utf-8"><title>angelX black space review</title>',
'<style>body{background:#000;color:#a7b4b3;font:16px system-ui;margin:24px}img{max-width:100%}a{color:#a07943}</style>',
'<h1>angelX — black space and braille</h1><p>Real Kitty pictures and terminal cells. Same deterministic moments. <a href="timings.csv">Frame timings</a></p>']
previews=[]
for name in SCENES:
    comp=Image.new('RGB',(1968,650),'black');draw=ImageDraw.Draw(comp)
    for col,(suffix,label) in enumerate([('last-v1-native','TODAY V1'),('last-v2-native','LAST V2'),('native','NEW V2 / KITTY'),('cells','NEW V2 / CELLS')]):
        pic=Image.open(d/f'{name}-{suffix}.png').convert('RGB');pic.thumbnail((960,560),Image.Resampling.NEAREST)
        x=8+(col%2)*984;y=8+(col//2)*320
        draw.text((x,y),f'{name.upper()} / {label}',fill='#a07943',font=font)
        pic.thumbnail((960,280),Image.Resampling.NEAREST)
        comp.paste(pic,(x+(960-pic.width)//2,y+30+(280-pic.height)//2))
    comp.save(d/f'{name}-four-way.png')
    previews.append(comp.resize((984,325),Image.Resampling.NEAREST))
    parts.append(f'<p><a href="{name}-four-way.png"><img src="{name}-four-way.png"></a></p>')
# Compare the mini-viz map with the Delve map at the identical world camera.
comp=Image.new('RGB',(1968,650),'black');draw=ImageDraw.Draw(comp)
for i,(name,suffix,label) in enumerate([('kingdom','native','DELVE KINGDOM / KITTY'),('walk-real','native','MINI KINGDOM WALK / KITTY'),('kingdom','cells','DELVE KINGDOM / CELLS'),('walk-real','cells','MINI KINGDOM WALK / CELLS')]):
    pic=Image.open(d/f'{name}-{suffix}.png').convert('RGB');pic.thumbnail((960,280),Image.Resampling.NEAREST)
    x=8+(i%2)*984;y=8+(i//2)*320;draw.text((x,y),label,fill='#a07943',font=font);comp.paste(pic,(x+(960-pic.width)//2,y+30))
comp.save(d/'kingdom-walk-beside-delve.png');previews.append(comp.resize((984,325),Image.Resampling.NEAREST))
sheet=Image.new('RGB',(1984,40+len(previews)//2*341),'black');draw=ImageDraw.Draw(sheet)
draw.text((12,10),'ANGELX — V1 | LAST V2 | NEW KITTY | NEW CELLS',font=font,fill='#a07943')
for i,pic in enumerate(previews):sheet.paste(pic,(8+(i%2)*992,40+(i//2)*341))
sheet.save(d/'contact-sheet.png')
parts.append('<p><img src="kingdom-walk-beside-delve.png"></p>')
(d/'index.html').write_text('\n'.join(parts))
print(d/'contact-sheet.png')
