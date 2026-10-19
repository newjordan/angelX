#!/usr/bin/env python3
"""Render exported real terminal glyphs beside native Forest Road captures."""
from pathlib import Path
import json,sys
from PIL import Image,ImageDraw,ImageFont
root=Path(sys.argv[1])
font=ImageFont.truetype('/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf',14)
sheet=Image.new('RGB',(1000,1464),'#080b0e');draw=ImageDraw.Draw(sheet)
for row,name in enumerate(['start','midboss','feller','freeing']):
    cells=Image.new('RGB',(768,448));pen=ImageDraw.Draw(cells)
    buffer=json.loads((root/(name+'-cells.json')).read_text())
    for i,cell in enumerate(buffer):
        x,y=i%96*8,i//96*16
        pen.rectangle((x,y,x+7,y+15),fill=tuple(cell['bg']))
    for i,cell in enumerate(buffer):
        x,y=i%96*8,i//96*16
        pen.text((x,y-2),cell['glyph'],font=font,fill=tuple(cell['fg']))
    cells.save(root/(name+'-cells.png'))
    y=row*366
    draw.text((8,y+4),name+' native 1x / actual 96x28 cockpit cells',font=font,fill='#b0b2a8')
    sheet.paste(Image.open(root/(name+'-native.png')),(8,y+40))
    sheet.paste(cells.resize((576,336),Image.Resampling.NEAREST),(412,y+24))
sheet.save(root/'review.png')
