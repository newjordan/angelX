#!/usr/bin/env python3
"""Register reviewed source crops without changing the shared slot converter."""
import argparse, hashlib, importlib.util, json
from pathlib import Path
from PIL import Image
ROOT=Path(__file__).resolve().parents[5]
spec=importlib.util.spec_from_file_location('slots',ROOT/'scripts/slot-art-import.py');imp=importlib.util.module_from_spec(spec);spec.loader.exec_module(imp)
p=argparse.ArgumentParser();p.add_argument('--output',required=True,type=Path);p.add_argument('--check',action='store_true');a=p.parse_args()
specs=json.loads(Path(__file__).with_name('registrations.json').read_text())
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
for slot,cfg in specs.items():
 dst=ROOT/'crates/delve/assets/art-slots'/slot;dst.mkdir(exist_ok=True)
 source=Path('/work/artifacts/art-prod/slots')/slot/'v01';assert sha(source/'source-sheet.png')==cfg['sha256'],'Delivery changed; inspect and update registration explicitly'
 raw=Image.open(source/'source-sheet.png').convert('RGBA');notes=(source/'notes.md').read_text()
 cw,ch=cfg['source_canvas'];ax,ay=cfg['source_anchor'];sheet=Image.new('RGBA',(cw*4+2,ch*3+2));meta={'pieces':[],'warnings':cfg.get('warnings',[])}
 for i,f in enumerate(cfg['frames']):
  x,y,w,h=f['rect'];crop=raw.crop((x,y,x+w,y+h));px,py=f['pivot'];at=(round(ax-px),round(ay-py));assert at[0]>=0 and at[1]>=0 and at[0]+w<=cw and at[1]+h<=ch,(slot,i,at,crop.size)
  # The entire source crop, including its dark contour, is retained. Only alpha and spill are normalized.
  clean,_=imp.despill(crop)
  if f.get("mirror"): clean=clean.transpose(Image.Transpose.FLIP_LEFT_RIGHT)
  clean.putdata([(r,g,b,255) if alpha>=128 else (0,0,0,0) for r,g,b,alpha in clean.get_flattened_data()])
  sheet.paste(clean,(i%4*cw+at[0]+1,i//4*ch+at[1]+1));meta['pieces'].append({'index':i,'source_rect_xywh':[i%4*cw+1,i//4*ch+1,cw,ch]})
 prepared=a.output/slot;prepared.mkdir(parents=True,exist_ok=True);sheet.save(prepared/'source-sheet.png');(prepared/'source-sheet.json').write_bytes(imp.js(meta));(prepared/'notes.md').write_text(notes)
 req=dict(id=slot,game=slot.split('.')[0],role='boss',fit=f"{cfg['size'][0]}x{cfg['size'][1]}",timing=cfg['timing'],geometry=cfg['geometry'],frame_groups=[{'state':'damage-action','source_pose_count':12,'names':[]}])
 m=imp.initial_map(slot,req,meta,notes,sheet);m['source_sha256']=sha(prepared/'source-sheet.png');m['sizes']={k:cfg['size'] for k in ['v1','v2']};m['anchor_canvas_wh']=cfg['size'];m['anchor_xy']=cfg['anchor'];m['registration']='shared-feet';m['runtime']=True;m['review']='reviewed-runtime-subset';m['mapping_basis']='Source figures and feet inspected individually; padded registration canvases retain one scale across the bank.'
 m['source_preparation']={'recipe':'crates/delve/assets/art-slots/cinder.crock-damaged/prepare.py','original_path':str(source/'source-sheet.png'),'original_sha256':cfg['sha256'],'original_metadata_sha256':sha(source/'source-sheet.json'),'original_crops':cfg['frames'],'source_canvas':cfg['source_canvas'],'source_anchor':cfg['source_anchor'],'verification_status':cfg['verification_status']}
 m['comments']=cfg['comments']
 for f,review in zip(m['pieces'],cfg['frames']):f.update(name=review['name'],state=review['name'],frame=0,source_pivot_xy=cfg['source_anchor'],approved=review.get('runtime',True),runtime=review.get('runtime',True),label=review['label'])
 imp.write(dst/(slot+'.map.json'),imp.js(m),a.check);imp.write(dst/'request.json',imp.js(req),a.check)
 args=argparse.Namespace(source_dir=prepared,v1_height=None,v2_height=None,terminal_metrics=None,check=a.check)
 imp.run(slot,req,args,cfg['verification_status'],'')
