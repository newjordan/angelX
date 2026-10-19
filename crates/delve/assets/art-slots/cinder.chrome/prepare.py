from pathlib import Path
from PIL import Image,ImageEnhance
import json,hashlib,importlib.util,re
import argparse
a=argparse.ArgumentParser(description='Prepare the reviewed Cinder/Latch crops for the unchanged slot importer.');a.add_argument('--root',type=Path,default=Path(__file__).resolve().parents[5]);a.add_argument('--output',type=Path,required=True);a.add_argument('--slot');args=a.parse_args()
ROOT=args.root; OUT=args.output
spec=importlib.util.spec_from_file_location('imp',ROOT/'scripts/slot-art-import.py');imp=importlib.util.module_from_spec(spec);spec.loader.exec_module(imp)
reqs=imp.requests()
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
for slot in sorted(s for s in reqs if s.startswith(('cinder.','latch.')) and (not args.slot or s==args.slot)):
 src=Path('/work/artifacts/art-prod/slots')/slot/'v01';dest=OUT/slot;dest.mkdir(parents=True,exist_ok=True)
 prior_path=ROOT/'crates/delve/assets/art-slots'/slot/(slot+'.map.json')
 prior=json.loads(prior_path.read_text())
 expected=prior.get('source_preparation',{}).get('original_sha256')
 if expected:assert sha(src/'source-sheet.png')==expected,'Delivery changed: inspect the new sheet and explicitly update its reviewed source SHA first.'
 im=Image.open(src/'source-sheet.png').convert('RGBA'); notes=(src/'notes.md').read_text()
 meta=json.loads((src/'source-sheet.json').read_text()) if (src/'source-sheet.json').exists() else {'pieces':[{'index':0,'source_rect_xywh':[0,0,im.width,im.height]}]}
 # Explicit cuts across inspected transparent gutters, not a generated grid.
 manual={
 'cinder.crock':[[55,120,236,306],[369,112,260,318],[650,145,327,285],[995,178,226,255],[34,475,279,334],[325,475,307,334],[645,475,304,334],[985,532,241,277],[43,881,249,287],[363,849,254,317],[659,861,282,312],[950,861,287,312]],
 'latch.clasp':[[21,170,283,239],[352,77,268,329],[623,209,310,211],[941,209,293,211],[21,577,312,203],[340,472,304,308],[674,554,248,227],[970,558,260,225],[29,938,290,234],[333,938,294,234],[649,910,287,224],[944,910,291,224]],
 }
 if slot in manual:meta={'pieces':[{'index':i,'source_rect_xywh':r} for i,r in enumerate(manual[slot])],'review':'Transparent gutters split by visual inspection 2026-10-15'}
 # Clamp pale effect cores and remove warm boundary spill before quantization.
 effect=slot in ['cinder.clear','cinder.impact','latch.clash','latch.parry','latch.strike']
 if slot=='latch.parry':
  a=im.getchannel('A');im=ImageEnhance.Color(im).enhance(.78);im.putalpha(a)
 px=im.load()
 for y in range(im.height):
  for x in range(im.width):
   r,g,b,a=px[x,y]
   if a<128:px[x,y]=(0,0,0,0);continue
   # Low-alpha warm-red fringe is outside the painted dark contour.
   if a<224 and r>g*1.65 and r>b*1.65:
    px[x,y]=(0,0,0,0);continue
   # Bring warm hot cores onto the established ochre ramp; preserve cool sparks.
   if slot in ['cinder.clear','cinder.impact','latch.clash']:
    if min(r,g,b)>180:r,g,b=216,169,94
    elif r>g and g>b*1.3:r=min(r,208);g=min(g,round(r*.82));b=max(b,min(round(r*.30),round(g*.95)))
   gain=min(1,224/max(1,r,g,b)) if effect else 1
   if slot.endswith('.room'):
    # Keep source architecture at the rim and a quiet field behind action/HUD.
    nx=x/im.width*384;ny=y/im.height*224
    gain=.54
    if slot=='latch.room' and 112<nx<320 and 68<ny<169:gain=.30
    if slot=='cinder.room' and 254<nx<365 and 67<ny<177:gain=.30
   px[x,y]=(round(r*gain),round(g*gain),round(b*gain),255)
 im.save(dest/'source-sheet.png');(dest/'notes.md').write_text(notes);(dest/'source-sheet.json').write_text(json.dumps(meta,indent=2)+'\n')
 m=imp.initial_map(slot,reqs[slot],meta,notes,im)
 m['source_sha256']=sha(dest/'source-sheet.png');m['source_preparation']={'script':'crates/delve/assets/art-slots/cinder.chrome/prepare.py','original_path':str(src/'source-sheet.png'),'original_sha256':sha(src/'source-sheet.png'),'metadata_sha256':sha(src/'source-sheet.json') if (src/'source-sheet.json').exists() else None,'alpha_threshold':128,'effect_value_cap':224 if effect else None,'warm_boundary_cleanup':'alpha <224, R >1.65G and R >1.65B','room_gain':.54 if slot.endswith('.room') else None,'warm_effect_grade':'pale cores to 216/169/94; warm R<=208, G<=0.82R, B raised toward 0.30R within G'}
 m['comments']=['Visually reviewed against sources-1.png through sources-5.png on 2026-10-15.','Supplemental pieces retained as reviewed proposals; runtime selects existing states only.']
 m['mapping_basis']='Inspected source poses and manually measured transparent gutters; runtime names match existing renderer states.'
 m['review']='reviewed';m['runtime']=True
 for p in m['pieces']:p['approved']=True;p['runtime']=False
 def select(i,name,state=None):
  p=m['pieces'][i];p.update(name=name,state=state or name,frame=0,runtime=True);return p
 if slot=='cinder.crock':
  names=['crock','windup-1','cough-1','recovery','windup-2','cough-2','step-1','step-2','flinch-1','crock-crack-1','crock-crack-2','defeat']
  for p,n in zip(m['pieces'],names):p['name']=n;p['state']=n
  m['runtime']=False;m['review']='reviewed-needs-rerun';m['comments']+=['Missing second flinch, registered damage variants for acting poses and terminal seam masks; retain coherent original bank. Final damage level cannot be substituted by a collapse.']
 elif slot=='cinder.knight':
  m['sizes']={'v1':[32,24],'v2':[32,24]};m['anchor_canvas_wh']=[32,24];m['anchor_xy']=[16,24]
  m['runtime']=False;m['review']='reviewed-needs-rerun';m['comments']+=['Red cloth, fixed gear and seated side camera do not preserve the player overhead modular figure or four seat masks; fallback retained.']
  for i,p in enumerate(m['pieces']):p['name']='watch' if i==0 else f'proposal-{i:02}'
 elif slot=='cinder.crock-lab':select(0,'crock-small')
 elif slot=='cinder.pots':
  m['anchor_xy']=[12,18];m['registration']='own-bounds'
  for i in range(3):
   p=select(i,f'pot-{i+1}');x,y,w,h=p['source_rect_xywh'];p['source_pivot_xy']=[w/2,h]
  m['comments']+=['Ghost cuts are reviewed but rejected at native size (flask vanishes); retain exact renderer inset outlines. Blue/green warm emblems get cached material-hue grading for terminal cells.']
  for i in [3,7,11]:
   p=select(i,{3:'ghost-1',7:'ghost-2',11:'ghost-3'}[i]);p['runtime']=False;x,y,w,h=p['source_rect_xywh'];p['source_pivot_xy']=[w/2,h]
 elif slot=='cinder.slag':select(0,'slag')
 elif slot=='cinder.tell':select(4,'tell')
 elif slot=='cinder.impact':
  m['registration']='shared-pivot'
  for i in range(4):select(i,f'impact-{i+1}')
 elif slot=='cinder.clear':
  m['registration']='shared-pivot'
  for i,n in zip([1,3,7],range(1,4)):select(i,f'clear-{n}')
 elif slot.endswith('.room'):select(0,'room')
 elif slot.endswith('.chrome'):
  m['sizes']={'v1':[96,64],'v2':[96,64]};m['anchor_canvas_wh']=[96,64];m['anchor_xy']=[48,32]
  select(1 if slot=='cinder.chrome' else 2,'panel')
 elif slot=='latch.clasp':
  for i,n in enumerate(['idle','high-tell','high-attack','low-tell','low-attack','flinch','damage-1','damage-2','defeat']):select(i,n)
  m['comments']+=['Every body already faces left in the delivered sheet. Twin eyes enlarged by existing visor treatment at reviewed face sockets; action seams remain clipped to opaque body.']
  for p in m['pieces']:
   w,h=p['source_rect_xywh'][2:];p['source_pivot_xy']=[w/2,h]
 elif slot=='latch.knight':
  # Preserve the original 128x80 envelope and the narrow-body foot socket.
  m['anchor_xy']=[38,80]
  for i,n in [(2,'knight-guard-high'),(1,'knight-guard-low'),(3,'knight-thrust-contact'),(4,'knight-thrust-recover')]:select(i,n)
  p=dict(m['pieces'][2]);p['piece']=12;p['name']='knight-thrust-brace';p['state']='counter-brace';p['runtime']=True;m['pieces'].append(p)
  meta['pieces'].append({'index':12,'source_rect_xywh':p['source_rect_xywh']});(dest/'source-sheet.json').write_text(json.dumps(meta,indent=2)+'\n');m['delivered_pieces']=13
  m['comments']+=['Horizontal brace reused as high guard; four distinct horizontal bodies selected. Upright spear poses stay supplemental. Missing dedicated high patch pose and shield-at-shins guard.']
 elif slot=='latch.parry':
  m['registration']='shared-pivot'
  for i in range(4):select(i,f'parry-{i+1}')
 elif slot=='latch.clash':
  m['registration']='shared-pivot'
  for i,n in zip([0,3,6,9],range(1,5)):select(i,f'clash-{n}')
 elif slot=='latch.strike':
  m['registration']='shared-pivot';m['anchor_xy']=[20,16]
  for i in range(4):
   p=select(i,f'strike-{i+1}');x,y,w,h=p['source_rect_xywh'];p['source_pivot_xy']=[max(0,w*.06),h*.90]
 mapfile=ROOT/'crates/delve/assets/art-slots'/slot/(slot+'.map.json');mapfile.write_bytes(imp.js(m))
 print(slot,sha(src/'source-sheet.png'))
