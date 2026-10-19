#!/usr/bin/env python3
"""Keep nearest/palette candidates for the BOSS-3B native cleanup."""
import argparse,json,runpy
from pathlib import Path
from PIL import Image,ImageDraw
ROOT=Path(__file__).resolve().parents[1]
A=runpy.run_path(str(ROOT/'scripts/boss-art.py'))
p=argparse.ArgumentParser(description=__doc__);p.add_argument('concepts',type=Path);p.add_argument('output',type=Path);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
entries=json.loads((ROOT/'scripts/boss-art-reliquary.json').read_text())
sheet=Image.new('RGB',(1380,600),'#080b0e');d=ImageDraw.Draw(sheet)
for i,(name,index,size) in enumerate([('iron-tusk-chief',2,(23,27)),('tax-key',0,(9,9)),('liege-clasp',1,(9,9)),('pick-tooth',2,(9,9)),('ward-arc',0,(11,11))]):
 src=A['extract'](Image.open(a.concepts/(name+'.png')),'neutral',30)
 colours=A['frame_inks'](entries[index]['frames']['v1'][0],{k:A['palette']()[k] for k in entries[index]['inks']})
 im,rows=A['snap'](A['fit'](src,size),colours)
 im.save(a.output/(name+'.png'));(a.output/(name+'.art')).write_text('\n'.join(rows)+'\n')
 x=20+(i%3)*460;y=36+(i//3)*280
 d.text((x,y-18),name+' / rejected nearest candidate',fill='#c2b494')
 sheet.paste(im.resize((size[0]*6,size[1]*6),Image.Resampling.NEAREST),(x,y),im.resize((size[0]*6,size[1]*6),Image.Resampling.NEAREST))
sheet.save(a.output/'nearest-candidates.png')
