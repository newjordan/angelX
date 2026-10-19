from pathlib import Path
from PIL import Image,ImageDraw
import json,hashlib,sys
root=Path(__file__).resolve().parents[5];check='--check' in sys.argv
for slot in ['cinder.knight','latch.knight']:
 base=root/'crates/delve/assets/art-slots'/slot;m=json.loads((base/(slot+'.map.json')).read_text());record=json.loads((base/'slot.json').read_text());src=Image.open(record['source_path']).convert('RGBA')
 for name,info in record['outputs'].items():
  piece=next(p for p in m['pieces'] if p['piece']==info['piece']);x,y,w,h=piece['source_rect_xywh'];mask=Image.new('RGBA',(w,h));region=Image.new('L',(w,h));d=ImageDraw.Draw(region)
  if slot=='cinder.knight' and piece['name']=='watch':
   # Source-registered cloth and plume only; no helmet, gloves, belt or buckle.
   d.polygon([(89,154),(162,154),(173,195),(147,215),(160,259),(93,282),(83,248),(88,216),(71,210)],fill=255)
   d.polygon([(10,0),(100,0),(133,26),(101,48),(84,78),(58,103),(12,103)],fill=255)
  for yy in range(h):
   for xx in range(w):
    r,g,b,a=src.getpixel((x+xx,y+yy));paint=False
    if slot=='cinder.knight':paint=bool(region.getpixel((xx,yy))) and r>g and g>b and r-b>16 and g-b>8 and r-g<48
    else:paint=b>r+12 and b>g+5
    if a>=128 and paint:
     v=min(3,max(0,(max(r,g,b)-50)//45))*70;mask.putpixel((xx,yy),(v,v,v,255))
  scale=info['source_scale'];small=mask.resize((max(1,round(w*scale)),max(1,round(h*scale))),Image.Resampling.BOX);canvas=Image.new('RGBA',tuple(info['canvas_wh']));anchor=info['anchor_xy'];pivot=piece['source_pivot_xy'];canvas.paste(small,(round(anchor[0]-pivot[0]*scale),round(anchor[1]-pivot[1]*scale)))
  native=Image.open(base/name).convert('RGBA')
  if slot=='latch.knight' and piece['name']=='knight-thrust-contact':
   # The source neighbour's detached boot falls inside the long-spear bounding box.
   # This empty gutter starts beyond the active knight's right boot and below its spear.
   for yy in range(45,native.height):
    for xx in range(90,native.width):native.putpixel((xx,yy),(0,0,0,0))
   import io
   stream=io.BytesIO();native.save(stream,format='PNG');clean=stream.getvalue()
   if check:assert (base/name).read_bytes()==clean
   else:
    (base/name).write_bytes(clean);info['sha256']=hashlib.sha256(clean).hexdigest();info['opaque_pixels']=sum(c[3]>0 for c in native.get_flattened_data())
  canvas.putdata([(c[0],)*3+(255,) if c[3]>=100 and p[3] else (0,0,0,0) for c,p in zip(canvas.get_flattened_data(),native.get_flattened_data())])
  import io
  b=io.BytesIO();canvas.save(b,format='PNG');data=b.getvalue();path=base/name.replace('.png','-seat-mask.png')
  if check:assert path.read_bytes()==data,path
  else:path.write_bytes(data)
  digest=hashlib.sha256(data).hexdigest()
  if check:assert info['seat_mask_sha256']==digest
  else:info['seat_mask_sha256']=digest
 if slot=='latch.knight':record['native_cleanup']={'latch.knight/knight-thrust-contact':[90,45,38,35],'reason':'Remove detached neighbouring boot in the manually separated source gutter.'}
 else:record.pop('native_cleanup',None)
 record['seat_mask_cleanup']={'script':'crates/delve/assets/art-slots/cinder.chrome/masks.py','cinder':'source polygons for ivory plume and tabard; warm cream only; alpha coverage 100/255','latch':'blue painted plume/tabard/shield, B>R+12 and B>G+5; alpha coverage 100/255'}
 if not check:(base/'slot.json').write_text(json.dumps(record,indent=2,sort_keys=True)+'\n')
 print(slot,'masks verified' if check else 'masks widened')

 # Review the final native pixels after cleanup, with the same twelve-frame layout.
 for start in range(0,len(m['pieces']),12):
  chunk=m['pieces'][start:start+12];review=Image.new('RGB',(160*min(4,len(chunk)),110*((len(chunk)+3)//4)),(11,13,17));d=ImageDraw.Draw(review)
  for i,piece in enumerate(chunk):
   im=Image.open(base/'v1'/(piece['name']+'.png')).convert('RGBA');im.thumbnail((152,82),Image.Resampling.NEAREST);x=i%4*160;y=i//4*110;review.paste(im,(x+4,y+24),im);d.text((x+4,y+4),f"{piece['piece']}: {piece['name']}"[:50],fill='#b0b2a8')
  import io
  buffer=io.BytesIO();review.save(buffer,format='PNG');data=buffer.getvalue();path=base/f'review-{start//12+1}.png'
  if check:assert path.read_bytes()==data
  else:path.write_bytes(data)
