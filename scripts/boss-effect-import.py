#!/usr/bin/env python3
"""Retain delivered effect crops and fit them to simulation-owned sprite slots."""
import argparse
import colorsys
import hashlib
import importlib.util
import io
import json
import runpy
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT/'cockpit/assets/dungeon/bosses'
RETAINED = ROOT/'crates/delve/assets/boss-effect-src'
SPEC = ROOT/'scripts/boss-effect-slots.json'
spec = importlib.util.spec_from_file_location('library_import', ROOT/'scripts/boss-library-import.py')
lib = importlib.util.module_from_spec(spec); spec.loader.exec_module(lib)
LIGHTNESS = runpy.run_path(str(ROOT/'scripts/boss-art.py'))['lightness']
# Every delivered bank names its source socket and native display in the manifest.
BOSSES = {boss: (entry['display']['v1'], entry['display']['v2'], entry['source_socket']['projectile'])
          for boss, entry in json.loads(SPEC.read_text()).items() if 'readability' in entry}



def digest(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def encoded(im):
 b=io.BytesIO(); im.save(b, format='PNG'); return b.getvalue()
def write(p, data, check):
 if check: assert p.read_bytes()==data, f'stale {p}'
 else:
  p.parent.mkdir(parents=True, exist_ok=True); p.write_bytes(data)


def retain(deliveries):
 for boss in BOSSES:
  for role in ('projectile','tell','impact'):
   folder=deliveries/f'bossfx.{boss}.{role}'/'v01'
   meta=json.loads((folder/'source-sheet.json').read_text())
   # The measurement names the accepted retry, not necessarily source-sheet.png.
   sheet=folder/Path(meta['file']).name
   im=Image.open(sheet).convert('RGBA'); assert list(im.size)==meta['size_wh']
   dest=RETAINED/boss/role; dest.mkdir(parents=True,exist_ok=True)
   frames=[]
   for piece in meta['pieces']:
    x,y,w,h=piece['source_rect_xywh']; p=dest/f"{piece['index']:02}.png"
    im.crop((x,y,x+w,y+h)).save(p)
    frames.append(dict(piece, file=p.name, sha256=digest(p)))
   rec=dict(source_path=str(sheet), source_sha256=digest(sheet),
            original_sheet_sha256=digest(folder/'source-sheet.png'),
            metadata_sha256=digest(folder/'source-sheet.json'), size=list(im.size), frames=frames)
   (dest/'provenance.json').write_text(json.dumps(rec,indent=2)+'\n')


def clean(im):
 # Remove only the thin reddish matte contamination at a transparent edge.
 # Interior rust, flame and robe reds remain source-owned.
 out=im.copy(); px=out.load()
 for y in range(im.height):
  for x in range(im.width):
   r,g,b,a=im.getpixel((x,y))
   if a and r>g*1.65 and r>b*1.65 and g<65:
    ns=[im.getpixel(n) for n in lib.neighbours(x,y,im.size)]
    if a<128 and any(c[3]<32 for c in ns): px[x,y]=(0,0,0,0)
    elif a<224 and any(c[3]<32 for c in ns):
     candidates=[c for c in ns if c[3]>=224]
     if candidates: px[x,y]=min(candidates,key=lambda c:lib.distance(c,(r,g,b)))[:3]+(a,)
 return out


def registered(frames, size, socket):
 # One scale across a whole animation; every contact socket maps to canvas centre.
 radii=[]
 for im in frames:
  sx,sy=im.width*socket[0],im.height*socket[1]
  radii.append((max(sx,im.width-sx),max(sy,im.height-sy)))
 rx=max(r[0] for r in radii); ry=max(r[1] for r in radii)
 scale=min((size[0]-2)/2/rx,(size[1]-2)/2/ry)
 out=[]
 for im in frames:
  dims=(max(1,round(im.width*scale)),max(1,round(im.height*scale)))
  small=clean(im).resize(dims,Image.Resampling.BOX)
  canvas=Image.new('RGBA',size)
  canvas.paste(small,(round(size[0]//2-im.width*socket[0]*scale),round(size[1]//2-im.height*socket[1]*scale)))
  out.append(canvas)
 return out


def load_frames(boss,role,indices):
 folder=RETAINED/boss/role; rec=json.loads((folder/'provenance.json').read_text())
 result=[]
 for i in indices:
  f=next(f for f in rec['frames'] if f['index']==i); p=folder/f['file']
  assert digest(p)==f['sha256']; result.append(Image.open(p).convert('RGBA'))
 return result


def skin(reference, mask):
 # Geometry is the existing mark, including lane endpoints and phase shape.
 source=[(x,y,reference.getpixel((x,y))) for y in range(reference.height) for x in range(reference.width) if reference.getpixel((x,y))[3]>=128]
 assert source
 out=Image.new('RGBA',mask.size)
 for y in range(mask.height):
  for x in range(mask.width):
   if mask.getpixel((x,y))[3]>=128:
    c=reference.getpixel((x,y))
    if c[3]<128: c=min(source,key=lambda p:(x-p[0])**2+(y-p[1])**2)[2]
    out.putpixel((x,y),c[:3]+(255,))
 return out


def projectile_pigment(c, policy):
 """Lift value without mixing the source's fire into the room's brown ramp."""
 h,s,v=colorsys.rgb_to_hsv(*(k/255 for k in c[:3]))
 if 'hue_reference' in policy:
  h=colorsys.rgb_to_hsv(*(k/255 for k in policy['hue_reference']))[0]
  s=max(.44,min(.72,s))
 v=min(242/255,v**policy['value_power']*policy['exposure'])
 return tuple(round(k*255) for k in colorsys.hsv_to_rgb(h,s,v))


def projectile_signals(source, policy):
 # Sample before shrinking: a furnace glint must not disappear in a BOX mean.
 colours=sorted((projectile_pigment(c,policy) for frame in source
                 for c in frame.get_flattened_data() if c[3]>=224),key=LIGHTNESS)
 signals = [colours[int(len(colours)*q)] for q in (.99,policy.get('rim_quantile',.95))]
 if 'signal_saturation_floor' in policy:
  signals = [tuple(round(k*255) for k in colorsys.hsv_to_rgb(h,max(s,policy['signal_saturation_floor']),v))
             for h,s,v in [colorsys.rgb_to_hsv(*(k/255 for k in c)) for c in signals]]
 return signals


def prepare_projectile(ref, policy, signals):
 """Keep the delivered alpha; expose its core and leading contour offline."""
 core,rim=signals
 pixels=sorted((projectile_pigment(c,policy) for c in ref.get_flattened_data()
                if c[3]>=128),key=LIGHTNESS)
 threshold=LIGHTNESS(pixels[int(len(pixels)*policy['core_quantile'])])
 out=Image.new('RGBA',ref.size)
 for y in range(ref.height):
  for x in range(ref.width):
   c=ref.getpixel((x,y))
   if c[3]<128: continue
   colour=projectile_pigment(c,policy)
   edge=any(ref.getpixel(n)[3]<128 for n in lib.neighbours(x,y,ref.size))
   if edge and (y<ref.height//2 or x>ref.width//2): colour=rim
   elif LIGHTNESS(colour)>=threshold: colour=core
   out.putpixel((x,y),colour+(255,))
 # A thin arrow may contain only edge samples after shrinking. Retain its
 # brightest authored sample as a core instead of replacing every glint by rim.
 if core+(255,) not in set(out.get_flattened_data()):
  point=max(((x,y) for y in range(ref.height) for x in range(ref.width)
             if ref.getpixel((x,y))[3]>=128),
            key=lambda p:(LIGHTNESS(projectile_pigment(ref.getpixel(p),policy)),
                          -((p[0]-ref.width/2)**2+(p[1]-ref.height/2)**2)))
  out.putpixel(point,core+(255,))
 return out


def reduce_projectile(ref, signals, policy=None):
 # A local source-derived bank preserves emission hues the realm's material
 # bank cannot represent (especially pink spores). The two signal inks are exact.
 pixels=[c[:3] for c in ref.get_flattened_data() if c[3]]
 samples=Image.new('RGB',(len(pixels),1));samples.putdata(pixels)
 method,kmeans=(policy or {}).get('quantizer',[0,0])
 quantized=samples.quantize(colors=10,method=method,kmeans=kmeans).convert('RGB')
 inks=sorted(set(quantized.get_flattened_data())|set(signals))
 out=Image.new('RGBA',ref.size)
 out.putdata([min(inks,key=lambda ink:lib.distance(c,ink))+(255,) if c[3] else (0,0,0,0)
              for c in ref.get_flattened_data()])
 return out,inks


def run(check=False):
 manifest=json.loads(SPEC.read_text())
 for boss,(v1,v2,socket) in BOSSES.items():
  entry=manifest[boss]; records=[]; bank_inks=set()
  entry.pop('projectile_inks',None)
  display=entry.get('display',{})
  v1=display.get('v1',v1);v2=display.get('v2',v2)
  sockets=entry.get('source_socket',{'projectile':socket,'tell':[.5,.5],'impact':[.5,.5]})
  socket=sockets['projectile']
  entry['source_socket']=sockets
  entry['collision']='simulation-owned point; 0.16-unit expansion; legacy9x9 canvas is not a visual size'
  entry['flight_ticks']=[4,4];entry['impact_ticks']=[2,1,1,1,1]
  if boss=='cinderjaw': entry.setdefault('point_rim',[[146,79,29],[169,118,57],[216,169,94],[236,182,74]])
  if boss=='mother-of-spores': entry.setdefault('point_rim',[[122,74,18],[146,79,29],[190,145,75],[216,169,94]])
  # Flight A1/A2; tells A1..A4; impact A contact/peak/break/fade/dust.
  # Most impact deliveries use 4 columns: dust is the first cell of row two.
  mapping={'projectile':[0,1], 'tell':[0,1,2,3], 'impact':[0,1,2,3,4]}
  if boss=='iron-tusk-chief':
   mapping['projectile']=[5,7]
   mapping['impact']=[0,1,2,3,11]
   entry['cell_warning_inks']=[[236,182,74]]
  for role,indices in mapping.items():
   if role in entry.get('rejected_roles', {}):
    mask_boss=entry['legacy_mask_boss'];names=[]
    for phase in range(4):
     old=ASSETS/mask_boss/('tell'+('' if phase==0 else f'-{phase}')+'.png')
     name=f'legacy-tell-{phase}.png';names.append(name)
     write(ASSETS/boss/name,old.read_bytes(),check)
     records.append(dict(file=name,sha256=digest(old),replaced_file=str(old.relative_to(ASSETS)),
                         retained_legacy_mask=True,rejection=entry['rejected_roles'][role]))
    entry[role]=names
    mapping[role]=[]
    continue
   src=load_frames(boss,role,indices)
   policy=entry['readability']
   signals=projectile_signals(src,policy) if role=='projectile' else None
   projectile_inks=set()
   kits=('v1','v2') if role=='projectile' else ('v1',)
   for kit in kits:
    mask_boss=entry.get('legacy_mask_boss',boss)
    size=(v1 if kit=='v1' else v2) if role=='projectile' else entry.get(role+'_size',list(Image.open(ASSETS/mask_boss/(role+'.png')).size))
    refs=registered(src,size,sockets[role])
    names=[]
    for phase,ref in enumerate(refs):
     old=ASSETS/mask_boss/(role+('' if phase==0 else f'-{phase}')+'.png')
     if role=='tell': ref=skin(ref,Image.open(old).convert('RGBA'))
     raw_ref=ref
     gain=policy['exposure'] if role=='projectile' else (
         .85 if boss in ('cinderjaw','mother-of-spores') else .9 if boss=='waxen-warden' else entry.get(role+'_exposure',1.0))
     if role=='projectile':
      assert gain>=1.0, 'hostile emission cannot be dimmed'
      ref=prepare_projectile(ref,policy,signals)
      native,inks=reduce_projectile(ref,signals,policy);projectile_inks.update(inks)
     else:
      # Existing tell/impact material preparation and warning masks stay exact.
      ref=ref.copy(); ref.putdata([tuple(round(v*gain) for v in c[:3])+(c[3],) for c in ref.get_flattened_data()])
      native=lib.convert(ref)
     if role=='tell':
      warning_inks=[c for c in lib.palette() if LIGHTNESS(c)<=80]
      native.putdata([min(warning_inks,key=lambda ink:lib.distance(c,ink))+(255,) if c[3] and LIGHTNESS(c[:3])>80 else c for c in native.get_flattened_data()])
     score=lib.fidelity(ref,native)
     for k,t in lib.LIMITS.items(): assert score[k]>=t, (boss,role,phase,k,score)
     name=f'fx-{kit}-{role}-{phase}.png'; names.append(name)
     write(ASSETS/boss/name,encoded(native),check)
     records.append(dict(file=name, source_cell=indices[phase], size=size, socket=sockets[role],
                         sha256=hashlib.sha256(encoded(native)).hexdigest(), replaced_file=old.name if mask_boss==boss else str(old.relative_to(ASSETS)),
                         replaced_sha256=digest(old), source_exposure=gain, raw_source_fidelity=lib.fidelity(raw_ref,native), fidelity=score))
    if kit=='v1': entry[role]=names
    else: entry['projectile_v2']=names
   if role=='tell' and entry.get('source_point_rim',False):
    # Four cached boundary values come from this delivered bank's inks.
    # The runtime still clips the mark to the same collision envelope.
    colours={c[:3] for name in entry['tell'] for c in Image.open(ASSETS/boss/name).convert('RGBA').get_flattened_data() if c[3] and 25<=LIGHTNESS(c[:3])<=80}
    flight_source=load_frames(boss,'projectile',mapping['projectile'])
    for frame in registered(flight_source,v2,sockets['projectile']):
     colours.update(c[:3] for c in lib.convert(frame).get_flattened_data() if c[3] and 25<=LIGHTNESS(c[:3])<=80)
    levels={LIGHTNESS(c):c for c in sorted(colours)}
    ordered=[levels[v] for v in sorted(levels)]
    assert len(ordered)>=4, (boss,'four source warning values required')
    entry['point_rim']=[list(ordered[round((len(ordered)-1)*q)]) for q in (.25,.5,.75,1.)]
   if role=='projectile':
    entry['projectile_signal_inks']=[list(c) for c in signals]
    bank_inks.update(projectile_inks)
  entry['display']={'v1':v1,'v2':v2,'study_v1':[[12,12],[18,18],[24,24]],'study_v2':[[20,20],[32,32],[40,40]]}
  entry['anchor']='centre; registered source body/contact socket'
  entry['source']='crates/delve/assets/boss-effect-src/'+boss
  entry['unwired']='all cells outside effect-import.json mapping retained; no runtime states for alternate flight, directional tells, fade/debris or alternate impacts'
  write(ASSETS/boss/'effect-import.json',(json.dumps(dict(boss=boss,definition_sha256=digest(ASSETS/(boss+'.boss')),mapping=mapping,point_rim=entry.get('point_rim'),outputs=records,
       readability=policy,projectile_signal_inks=entry['projectile_signal_inks'],projectile_inks=[list(c) for c in sorted(bank_inks)],
       resample='BOX; binary alpha 128; local source-derived flight inks, shared 75 tell/impact inks; edge despill; source-centre registration',
       tell_geometry='legacy alpha mask; point clip, lane half-width, ray reach and safe gap remain runtime-owned'),indent=2)+'\n').encode(),check)
 write(SPEC,(json.dumps(manifest,indent=2)+'\n').encode(),check)
 print(f'{len(BOSSES)} effect banks: fidelity, palette and binary alpha checked')

if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__); p.add_argument('--retain',type=Path);p.add_argument('--check',action='store_true'); a=p.parse_args()
 if a.retain: retain(a.retain)
 run(a.check)
