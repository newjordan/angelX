from pathlib import Path
from PIL import Image,ImageDraw
import hashlib,json,importlib.util
root=Path(__file__).resolve().parents[5];out=Path('/work/artifacts/boss-art-20261015/polish-a/prepared')
spec=importlib.util.spec_from_file_location('imp',root/'scripts/slot-art-import.py');imp=importlib.util.module_from_spec(spec);spec.loader.exec_module(imp)
EXPECTED={'cinder.knight': 'd95cb92f22a8f9f7432ee3a20ed5561dc3449adbc35368ea8490407cb8a8fe60', 'latch.knight': '5aa3ed147d31f3e5519ead282f66a0bc64fca02e945a1a72998caa6b9cd4c4fd'}
for slot in ['cinder.knight','latch.knight']:
 src=Path('/work/artifacts/art-prod/slots')/slot/'v01'; dst=out/slot;dst.mkdir(parents=True,exist_ok=True)
 status=json.loads(Path('/work/artifacts/art-prod/slots/status.json').read_text())[slot];assert status.startswith('OK') and '?' not in status
 sheet=Image.open(src/'source-sheet.png').convert('RGBA');meta=json.loads((src/'source-sheet.json').read_text());notes=(src/'notes.md').read_text()
 original=hashlib.sha256((src/'source-sheet.png').read_bytes()).hexdigest()
 assert original==EXPECTED[slot], 'Delivery changed: inspect and remap before importing.'
 if slot=='latch.knight':
  # Inspected transparent gutters. Keep all twelve poses, including the rejected merged-measurement groups.
  rects=[[30,143,301,280],[351,194,297,229],[658,135,286,289],[958,204,290,220],[30,527,366,274],[336,553,341,249],[651,552,382,250],[1024,524,221,278],[19,907,292,277],[321,980,338,205],[660,917,313,268],[977,947,265,238]]
  meta={'pieces':[{'index':i,'source_rect_xywh':r} for i,r in enumerate(rects)],'review':'Manual transparent-gutter cuts of the inspected 12-pose sheet; merged component labels are not pose order.'}
 # Threshold source alpha while preserving source RGB and dark contours.
 sheet.putdata([c[:3]+(255,) if c[3]>=128 else (0,0,0,0) for c in sheet.get_flattened_data()]);sheet.save(dst/'source-sheet.png')
 (dst/'source-sheet.json').write_text(json.dumps(meta,indent=2)+'\n');(dst/'notes.md').write_text(notes)
 m=imp.initial_map(slot,imp.requests()[slot],meta,notes,sheet)
 m.update(source_sha256=hashlib.sha256((dst/'source-sheet.png').read_bytes()).hexdigest(),runtime=True,review='reviewed-subset',mapping_basis='Inspected 12 posed figures; explicit native feet pivots and selected existing states.')
 m['source_preparation']={'script':'crates/delve/assets/art-slots/cinder.chrome/actors.py','original_path':str(src/'source-sheet.png'),'original_sha256':original,'metadata_sha256':hashlib.sha256((src/'source-sheet.json').read_bytes()).hexdigest(),'alpha_threshold':128}
 m['comments']=['Only explicitly mapped existing states are active; source count warnings and supplemental poses remain visible.','Seat masks include plume, cloth and painted shield field; steel/brass/leather are excluded before quantization.']
 for p in m['pieces']:
  p.update(name=f'proposal-{p["piece"]:02}',approved=True,runtime=False)
  w,h=p['source_rect_xywh'][2:];p['source_pivot_xy']=[w/2,h]
 if slot=='latch.knight':
  m['anchor_xy']=[38,80]
  for p in m['pieces']:p['source_pivot_xy'][0]=min(128,p['source_pivot_xy'][0])
  for i,name,pivot in [(2,'knight-guard-high',122),(3,'knight-guard-low',135),(0,'knight-thrust-brace',133),(4,'knight-thrust-contact',128),(11,'knight-thrust-recover',122)]:
   m['pieces'][i].update(name=name,state=name,frame=0,runtime=True,source_pivot_xy=[pivot,m['pieces'][i]['source_rect_xywh'][3]])
 else:
  m['sizes']={'v1':[32,24],'v2':[32,24]};m['anchor_canvas_wh']=[32,24];m['anchor_xy']=[16,24]
  m['pieces'][0].update(name='watch',state='watch',frame=0,runtime=True)
 (root/'crates/delve/assets/art-slots'/slot/(slot+'.map.json')).write_bytes(imp.js(m))
 print(slot,original)
