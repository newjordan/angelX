#!/usr/bin/env python3
"""Verify the assigned art batch against its saved byte/mask baseline."""
import argparse
import hashlib
import json
from pathlib import Path
from PIL import Image

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('baseline',type=Path)
a=p.parse_args()
root=Path(__file__).resolve().parents[1]
entries=json.loads((root/'scripts/boss-art-reliquary.json').read_text())
assigned={e['boss'] for e in entries}
assigned_names={Path(e['boss']).stem for e in entries}
protected=json.loads((a.baseline/'protected-hashes.json').read_text())
for path,digest in protected.items():
    if path not in assigned:
        assert hashlib.sha256((root/path).read_bytes()).hexdigest()==digest,path
print('Approved kits/recipes, unassigned bosses and master ink: exact bytes')
for e in entries:
    path=Path(e['boss'])
    old=(a.baseline/'before/bosses'/path.name).read_text()
    new=(root/path).read_text()
    before,old=old.split('\nart\n')
    after,new=new.split('\nart\n')
    assert before==after,path
    old,new=old.splitlines(),new.splitlines()
    assert (len(old),list(map(len,old)))==(len(new),list(map(len,new))),path
    assert [[k!='.' for k in r] for r in old]==[[k!='.' for k in r] for r in new],path
    for role,factor in [('v1',1),('v2',2)]:
        im=Image.open(root/e['output']/(role+'.png')).convert('RGBA')
        assert im.size==(len(old[0])*factor,len(old)*factor)
        for y in range(im.height):
            for x in range(im.width):
                assert bool(im.getpixel((x,y))[3])==(old[y//factor][x//factor]!='.'),(path,role,x,y)
    print(f'{path.stem}: header/numbers, dimensions, alpha and feet exact; v2 doubles the mask')
families=[]
for recipe in ['boss-art-pilots.json','boss-art-crypt.json','boss-art-reliquary.json']:
    for e in json.loads((root/'scripts'/recipe).read_text()):
        name=Path(e['boss']).stem
        for frame in ['projectile.png','projectile-1.png']:
            im=Image.open(root/e['output']/frame).convert('RGBA')
            assert im.size==((24,24) if name in assigned_names else (9,9))
            mask=im.getchannel('A').tobytes()
            assert all(name==other or mask!=m for other,m in families),(name,frame)
            families.append((name,mask))
print(f'All {len({name for name,_ in families})} projectile families (both frames) have distinct native alpha silhouettes')
if (a.baseline/'terminal').exists():
    count=0
    assigned_names={Path(e['boss']).stem for e in entries}
    for before in (a.baseline/'before/terminal').iterdir():
        if before.name.startswith(tuple(assigned_names)) or before.name=='review-layout.json':continue
        after=a.baseline/'terminal'/before.name
        assert before.read_bytes()==after.read_bytes(),before.name
        count+=1
    print(f'{count} approved RGB/cell captures match the baseline byte-for-byte')
