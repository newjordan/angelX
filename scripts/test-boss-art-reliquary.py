#!/usr/bin/env python3
"""Negative controls on authored stages and the real cached ring captures."""
import argparse
import copy
import json
import math
import runpy
from pathlib import Path
from unittest.mock import patch
from PIL import Image

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('captures',type=Path)
a=p.parse_args()
root=Path(__file__).resolve().parents[1]
art=runpy.run_path(str(root/'scripts/boss-art.py'))
entries=json.loads((root/'scripts/boss-art-reliquary.json').read_text())
layout=json.loads((a.captures/'review-layout.json').read_text())['bosses']
image_open=Image.open
read_text=Path.read_text

def frozen_image(file,*args,**kwargs):
    im=image_open(file,*args,**kwargs)
    name=Path(file).stem
    for e in entries:
        boss=Path(e['boss']).stem
        if name.startswith(boss+'-effects-') and name.endswith('-v2-rgb'):
            im=im.copy()
            before=image_open(a.captures/f'{boss}-effects-0-v2-rgb.png')
            geometry=layout[boss]
            centre=[(box,im.crop(box)) for box in map(tuple,geometry['tell_ignore'])]
            box=tuple(geometry['tell'])
            im.paste(before.crop(box),box)
            for box,crop in centre:im.paste(crop,box)
    return im

def frozen_text(path,*args,**kwargs):
    text=read_text(path,*args,**kwargs)
    for e in entries:
        boss=Path(e['boss']).stem
        if path.name.startswith(boss+'-effects-') and path.name.endswith('-v2-cells.json'):
            raw=json.loads(text)
            before=json.loads(read_text(a.captures/f'{boss}-effects-0-v2-cells.json'))
            x0,y0,x1,y1=layout[boss]['tell']
            for y in range(y0//16,math.ceil(y1/16)):
                for x in range(x0//8,math.ceil(x1/8)):
                    if not any(b[0]//8<=x<math.ceil(b[2]/8) and b[1]//16<=y<math.ceil(b[3]/16) for b in layout[boss]['tell_ignore']):raw[y*96+x]=before[y*96+x]
            return json.dumps(raw)
    return text

with patch.object(Image,'open',frozen_image),patch.object(Path,'read_text',frozen_text),patch.object(Image.Image,'save'):
    failures=art['player_review'](entries,a.captures,a.captures.parent/'negative-review')
assert len(failures)==9,failures
assert all('identical/unreadable player tell' in f for f in failures),failures
print('FROZEN RING: all nine RGB/cell transitions rejected despite independent boss/projectile/impact animation')
bad=copy.deepcopy(entries)
bad[0]['frames']['tell'][1]=copy.deepcopy(bad[0]['frames']['tell'][0])
assert any('identical authored tell' in f for f in art['numeric_gates'](bad))
print('REPEATED AUTHORED STAGE: rejected')
bad=copy.deepcopy(entries)
for f in bad[0]['frames']['projectile']:
    f['pigments']={k:'#ffffff' for k in bad[0]['inks']}
assert any('near-white cluster' in f for f in art['numeric_gates'](bad))
print('WHITE EFFECT CORE: rejected')
