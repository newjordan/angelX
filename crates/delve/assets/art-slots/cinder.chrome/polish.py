from pathlib import Path
from PIL import Image
import json,hashlib,math
root=Path(__file__).resolve().parents[5]
slot='latch.room'; dst=Path('/work/artifacts/boss-art-20261015/polish-a/prepared')/slot;dst.mkdir(parents=True,exist_ok=True)
mfile=root/'crates/delve/assets/art-slots'/slot/(slot+'.map.json');m=json.loads(mfile.read_text())
src=Path(m['source_preparation']['original_path']);assert hashlib.sha256(src.read_bytes()).hexdigest()==m['source_preparation']['original_sha256']
im=Image.open(src).convert('RGBA');px=im.load()
for y in range(im.height):
 for x in range(im.width):
  r,g,b,a=px[x,y]; nx=x/im.width*384; ny=y/im.height*224
  radius=((nx-216)/150)**2+((ny-121)/90)**2
  t=min(1,max(0,radius));t=t*t*(3-2*t)
  gain=.13+.27*t
  px[x,y]=(round(r*gain),round(g*gain),round(b*gain),255 if a>=128 else 0)
im.save(dst/'source-sheet.png');(dst/'notes.md').write_text((src.parent/'notes.md').read_text())
m['source_sha256']=hashlib.sha256((dst/'source-sheet.png').read_bytes()).hexdigest()
m['source_preparation'].update(script='crates/delve/assets/art-slots/cinder.chrome/polish.py',room_gain={'centre':.13,'rim':.40,'centre_xy':[216,121],'radius_xy':[150,90],'falloff':'smoothstep squared elliptical radius'})
m['comments'].append('POLISH-A: continuous smooth radial grade replaces the rectangular fighter recess; native and cell caches apply independent final luminance grades.')
mfile.write_text(json.dumps(m,indent=2,sort_keys=True)+'\n')
