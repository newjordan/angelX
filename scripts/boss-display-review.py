#!/usr/bin/env python3
"""Bounded sheets from source-sized rooms and actual emitted cell buffers."""
import argparse,json,math
from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
ROOT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('output',type=Path);a=p.parse_args();out=a.output
old=out.parent/'integ-0';study=out/'study'
registry=json.loads((ROOT/'scripts/boss-library-registry.json').read_text())
records={r['name']:r for r in json.loads((study/'records.json').read_text())}
font=ImageFont.truetype('/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf',14)
def cells(path):
 raw=json.loads(path.read_text());im=Image.new('RGB',(768,448));d=ImageDraw.Draw(im)
 for i,c in enumerate(raw):
  x=i%96*8;y=i//96*16;d.rectangle((x,y,x+7,y+15),fill=tuple(c['bg']));d.text((x,y-2),c['glyph'],font=font,fill=tuple(c['fg']))
 return im

def row(sheet,index,label,stem):
 y=index*480+28;d=ImageDraw.Draw(sheet);d.text((10,y-20),label+' / NATIVE 1X',fill='wheat');d.text((798,y-20),'ACTUAL 96x28 CELLS',fill='wheat')
 sheet.paste(Image.open(str(stem)+'-rgb.png'),(10,y));sheet.paste(cells(Path(str(stem)+'-cells.json')),(798,y))

review=[]
for boss in registry:
 sheet=Image.new('RGB',(1570,2880),(8,11,14))
 for i,(kit,sample) in enumerate([(1,1),(1,2),(1,4),(2,1),(2,2),(2,3)]):
  name=f'{boss}-sample-{sample}-133-v{kit}';r=records[name]
  row(sheet,i,f'{boss} V{kit} {r["size"]} / {r["shots"]} SHOTS / 4 SEATS',study/name)
 path=out/f'{boss}-study.png';sheet.save(path);review.append(path.name)
 sheet=Image.new('RGB',(1570,2880),(8,11,14))
 for i,(kit,label,stem) in enumerate((kit,label,stem) for kit in (1,2) for label,stem in [
  ('ORIGINAL 7d711a03',old/'before/terminal'/f'{boss}-old-133-v{kit}'),
  ('INTEG-0 OLD ENVELOPE',old/'after/terminal'/f'{boss}-current-133-v{kit}'),
  ('INTEG-1 DISPLAY POLICY',study/f'{boss}-sample-0-133-v{kit}')]):
  row(sheet,i,f'{boss} V{kit} / {label} / TICK133',stem)
 path=out/f'{boss}-combat.png';sheet.save(path);review.append(path.name)
 sheet=Image.new('RGB',(1540,2400),(8,11,14));d=ImageDraw.Draw(sheet)
 for i,(kit,label,stem) in enumerate((kit,label,stem) for kit in (1,2) for label,stem in [
  ('ORIGINAL',old/'before/terminal'/f'{boss}-old-0-v{kit}'),
  ('INTEG-0',old/'after/terminal'/f'{boss}-current-0-v{kit}'),
  ('INTEG-1',study/f'{boss}-sample-0-0-v{kit}')]):
  k=kit;cx=192*k;fy=96*k
  box=(cx-90*k//2,fy-74*k,cx+90*k//2,fy+12*k)
  rgb=Image.open(str(stem)+'-rgb.png').crop(box)
  cell=cells(Path(str(stem)+'-cells.json')).crop(tuple(v*2//k for v in box))
  rgb=rgb.resize((rgb.width*2,rgb.height*2),Image.Resampling.NEAREST)
  cell=cell.resize((cell.width*2,cell.height*2),Image.Resampling.NEAREST)
  y=i*400+26;d.text((12,y-20),f'{boss} V{kit} / {label} / RGB 2X + ACTUAL CELL CROP 2X',fill='wheat')
  sheet.paste(rgb,(12,y));sheet.paste(cell,(500,y))
 path=out/f'{boss}-identity.png';sheet.save(path);review.append(path.name)
for j,names in enumerate([['kingdom','crypt'],['hall','book-hop']]):
 sheet=Image.new('RGB',(1570,1920),(8,11,14))
 for i,(name,kit) in enumerate((name,kit) for name in names for kit in (1,2)):
  row(sheet,i,f'{name} V{kit} / 4 SEATS',study/f'coverage-{name}-v{kit}')
 path=out/f'coverage-{j+1}.png';sheet.save(path);review.append(path.name)
(out/'review-images.json').write_text(json.dumps(review,indent=2)+'\n')
