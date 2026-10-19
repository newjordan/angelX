from pathlib import Path
import importlib.util,json,hashlib,shutil
from PIL import Image
root=Path(__file__).resolve().parents[1];out=root/'crates/delve/assets/art-slots';source=root/'crates/delve/assets/dungeon-src'
spec=importlib.util.spec_from_file_location('imp',root/'scripts/slot-art-import.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
reqs=m.requests(root/'crates/delve/assets/art-slots/dgn.requests.json')
# The production detector joined neighboring cells. Review rectangles partition
# the delivered 4x3 grid; the original sheets and measurements remain read-only.
for slot in ['dgn.odoor.mines','dgn.odoor.keep','dgn.odoor.fungal','dgn.hud.chrome']:
 src=Path('/work/artifacts/art-prod/slots')/slot/'v01';dst=source/slot;dst.mkdir(parents=True,exist_ok=True)
 sheet=src/('source-sheet-3.png' if slot.endswith('mines') else 'source-sheet.png')
 shutil.copyfile(sheet,dst/'source-sheet.png');shutil.copyfile(src/'notes.md',dst/'notes.md')
 im=Image.open(sheet).convert('RGBA');meta=json.loads((src/'source-sheet.json').read_text())
 if slot in ['dgn.odoor.mines','dgn.odoor.keep']:
  pieces=[]
  for i in range(12):
   x0,x1=round(i%4*im.width/4),round((i%4+1)*im.width/4)
   y0,y1=round(i//4*im.height/3),round((i//4+1)*im.height/3)
   a=im.crop((x0,y0,x1,y1)).getchannel('A').point(lambda a:255 if a>=128 else 0);b=a.getbbox();assert b
   x,y,r,bottom=b;pieces.append(dict(index=i,source_rect_xywh=[x+x0,y+y0,r-x,bottom-y],alpha_area=sum(v>0 for v in a.get_flattened_data())))
  meta=dict(file='source-sheet.png',size_wh=list(im.size),alpha_threshold=128,pieces=pieces,warnings=[],review='Measured per grid cell after connected-component merging; front states inspected intact.')
 (dst/'source-sheet.json').write_bytes(m.js(meta))
 (dst/'provenance.json').write_bytes(m.js(dict(delivery=str(sheet),source_sha256=m.digest(sheet),original_metadata_sha256=m.digest(src/'source-sheet.json'),notes_sha256=m.digest(src/'notes.md'),crop_basis=meta.get('review','Delivered separated components'))))
 if slot=='dgn.hud.chrome':
  names=['ready','cooldown','disabled','pressed','health','stamina','boss','seat','floor','minimap','status','toast']
  dims=[[18,18]]*4+[[28,10],[24,9],[48,18],[18,11],[22,22],[38,38],[12,12],[40,14]]
  req=dict(id=slot,role='hud',fit='18x18 action plate; other envelopes per piece',timing='Read-only live readiness, health, charge and map data; no presentation clock.',geometry='Existing labels, counts, meters and keybindings draw after the chrome.',frame_groups=[dict(state='hud',names=names,source_pose_count=12)])
  reqs[slot]=req
 else:
  names=['barred','open','opening','west-barred','west-open','east-barred','east-open','south-barred','south-open','stairs-down','boss','seal']
  dims=[[32,24]]*3+[[16,32]]*4+[[32,16]]*2+[[32,32],[32,32],[16,16]]
  req=reqs[slot]
 mapping=m.initial_map(slot,req,meta,(dst/'notes.md').read_text(),im,dst)
 mapping.update(review='reviewed-native',mapping_basis='Reviewed delivered reading order; per-grid Mines/Keep crops repair merged detector groups.',runtime=True,embed_kits=['v1','v2'],sizes={'v1':dims[0],'v2':[n*2 for n in dims[0]]},anchor_xy=[dims[0][0]/2,dims[0][1]/2],anchor_canvas_wh=dims[0],comments=['Source design unchanged; BOX reduction, binary alpha and shared inks.','Side states remain staged where native barred/open silhouettes are ambiguous.'])
 for i,p in enumerate(mapping['pieces']):
  p.update(name=names[i],state='hud' if slot.endswith('chrome') else 'odoor',frame=i,approved=True,runtime=True,sizes={'v1':dims[i],'v2':[n*2 for n in dims[i]]},anchor_fraction=[.5,.5],sampling='contain',grade={'value':.8,'saturation':.8})
  if not slot.endswith('chrome') and i in range(3,7):p.update(approved=False,runtime=False,review_note='Side opening remains ambiguous after native reduction; retain dim pack-coloured passage placeholders.')
  if slot.endswith('mines') and i in [7,8]:p.update(approved=False,runtime=False,review_note='South open is an opaque timber crosspiece and still reads closed at native size; preserve the readable existing fallback for this pair.')
  if slot.endswith('chrome'):p['grade']={'value':.72,'saturation':.65}
 dest=out/slot;dest.mkdir(exist_ok=True);(dest/(slot+'.map.json')).write_bytes(m.js(mapping))
 if slot.endswith('chrome'):
  manifest=json.loads((out/'dgn.requests.json').read_text());manifest['games'][0]['assets']=[r for r in manifest['games'][0]['assets'] if r['id']!=slot]+[req];(out/'dgn.requests.json').write_bytes(m.js(manifest))
 print(slot,dst)
