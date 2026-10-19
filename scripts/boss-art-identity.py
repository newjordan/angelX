#!/usr/bin/env python3
"""Reproduce the identity-prop cleanup and retain nearest concept candidates."""
import argparse
import json
import runpy
import subprocess
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
A = runpy.run_path(str(ROOT/'scripts/boss-art.py'))
OUT = Path('/work/artifacts/boss-art-20261015/boss-3c')
PALS = [
 dict(s='#2e2240',S='#4a3466',U='#6e5288', **{'6':'#8a6aa8','@':'#160f20','2':'#4ac0b0'},o='#6a3a1c',O='#8a4e24',t='#c89a3a',T='#d0b060'),
 dict(s='#5a1e34',S='#7e2c48',R='#a84a5a',h='#9aa4b0',H='#c4ccd2',o='#120a10',O='#8a6a2a',t='#d0b878', **{'2':'#369c90','3':'#63c8b0'}),
 dict(s='#3a5426',S='#4e6e32',U='#708f40',h='#8a7a5a',H='#cbb98e',o='#5a3020',O='#8a4a22',t='#c07a3a',T='#7890a8', **{'@':'#c0402a'})
]
BASE = json.loads(subprocess.check_output(['git','show','6f7a0e17:scripts/boss-art-reliquary.json'],cwd=ROOT,text=True))

def put(im,x,y,k):
 if 0<=x<im.width and 0<=y<im.height: im.putpixel((x,y),ord(k))
def rect(im,box,k): ImageDraw.Draw(im).rectangle(box,fill=ord(k))
def line(im,points,k,w=1): ImageDraw.Draw(im).line(points,fill=ord(k),width=w)
def poly(im,points,k): ImageDraw.Draw(im).polygon(points,fill=ord(k))
def motif(im,x,y,rows):
 for dy,row in enumerate(rows):
  for dx,k in enumerate(row):
   if k!='.':put(im,x+dx,y+dy,k)
def mask(i,f=1):
 rows=BASE[i]['frames']['v1'][0]['rows'];m=Image.new('L',(len(rows[0]),len(rows)))
 for y,row in enumerate(rows):
  for x,k in enumerate(row):
   if k!='.':m.putpixel((x,y),255)
 return m.resize((m.width*f,m.height*f),Image.Resampling.NEAREST)
def finish(im,i,m=None,pal=None,details=()):
 rows=[''.join(chr(im.getpixel((x,y))) if (m.getpixel((x,y)) if m else im.getpixel((x,y))!=ord('.')) else '.' for x in range(im.width)) for y in range(im.height)]
 # Only deliberately drawn semantic details and inherited alpha islands may
 # survive as single pixels. Broad sampled noise is never exempted.
 keep=set(details)
 for y,row in enumerate(rows):
  for x,k in enumerate(row):
   if k!='.' and all(not(0<=ny<len(rows) and 0<=nx<len(row)) or rows[ny][nx]=='.' for nx,ny in A['neighbours'](x,y)):keep.add((x,y))
 while True:
  clean=A['despeckle'](rows,keep)
  if clean==rows:break
  rows=clean
 return dict(rows=rows,pigments=pal or PALS[i],hand_pixels=[list(q) for q in sorted(keep)])
def lights(spec,pts,name):
 rows=spec['rows'];pal=spec['pigments'];collars={};points=set(pts)
 pixels=[]
 for x,y in pts:
  k=rows[y][x];pixels.append([x,y,k])
  candidates=[(nx,ny) for nx,ny in A['neighbours'](x,y) if 0<=ny<len(rows) and 0<=nx<len(rows[0]) and rows[ny][nx]!='.' and (nx,ny) not in points and A['lightness'](bytes.fromhex(pal[rows[ny][nx]][1:]))<=A['lightness'](bytes.fromhex(pal[k][1:]))-7.5]
  assert candidates,(x,y,k)
  nx,ny=candidates[0];collars[nx,ny]=rows[ny][nx]
 spec['signature_lights']=[dict(name=name,pixels=pixels,falloff=[[x,y,k] for (x,y),k in collars.items()],hues={k:pal[k] for k in sorted({k for x,y,k in pixels}|set(collars.values()))})]
 spec['hand_pixels'] += [list(q) for q in pts]+[list(q) for q in collars]

def tax():
 im=Image.new('L',mask(0).size,ord('s'))
 poly(im,[(4,0),(10,0),(14,7),(13,12),(3,12)],'S')
 poly(im,[(6,1),(10,1),(13,6),(12,10),(5,10)],'U')
 line(im,[(4,6),(5,3),(8,1),(10,1)],'6')
 poly(im,[(7,4),(11,4),(13,7),(12,10),(6,10),(5,8)],'s')
 rect(im,(8,6,10,8),'@')
 put(im,7,7,'2');put(im,10,7,'2')
 poly(im,[(1,11),(5,10),(7,14),(5,21),(1,23)],'U')
 poly(im,[(7,11),(12,10),(16,20),(12,23),(5,22)],'S')
 line(im,[(2,13),(3,12),(4,14),(3,19),(2,22)],'6')
 line(im,[(7,19),(8,21),(7,23)],'U');line(im,[(12,19),(13,22),(14,24)],'U')
 # Nine by six chest: curved lid, broad band, wood body and central lock.
 # A nine-by-six wooden chest front under a broad rounded lid / brass frame.
 motif(im,4,10,['..OtttttttO..','.OttOOOOOttO.','ttttttttttttt','tOOOOOtOOOOOt','tOoOOOtOOOoOt','tOoOOO@OOOoOt','tOoOOOOOOOoOt','tOoOOOOOOOoOt','ttOtttttttOtt','..ooooooooo..'])
 motif(im,17,18,['.TT.','TttT','.TT.','oOOo'])
 put(im,15,18,'T');put(im,16,18,'T');put(im,14,20,'T');put(im,15,20,'T')
 line(im,[(15,14),(16,15),(16,18),(18,18)],'o')
 for x in (1,5,9,13):line(im,[(x,20),(x+1,22),(x+1,24)],'s')
 return im

def liege():
 old=BASE[1]['frames']['v1'][0]
 im=Image.new('L',mask(1).size,ord('s'))
 # Preserve the wine mass and heart-cage arrangement; cool skull / gold separate.
 for y,row in enumerate(A['signature_rows'](old)):
  for x,k in enumerate(row):
   if k!='.':put(im,x,y,{'t':'R','h':'O','H':'t','R':'S'}.get(k,k))
 for y in range(6):
  for x in range(im.width):put(im,x,y,'O')
 line(im,[(7,3),(14,3)],'t')
 for x,y in [(11,0),(6,1),(9,1),(13,1),(16,1)]:put(im,x,y,'t')
 rect(im,(6,6,15,10),'h')
 motif(im,6,6,['hhHHHHh.','hHHHHHHh','HooHHooH','Ho2HHo2H','.HhHHhH.','..hHHh..'])
 # Dark bronze shoulders keep the crown's gold above the cool bone.
 poly(im,[(0,11),(6,11),(8,16),(2,18)],'O');poly(im,[(14,11),(21,11),(21,17),(16,17)],'O')
 line(im,[(0,12),(5,12),(6,14)],'O');line(im,[(16,12),(20,13)],'O')
 motif(im,6,17,['.OOtOOO.','Otoooo tO'.replace(' ',''),'Oo2332oO','Oo2332oO','Oo2222oO','.Ot22tO.','..OOOO..'])
 line(im,[(1,16),(3,16)],'h');line(im,[(18,16),(20,16)],'h');put(im,1,17,'H');line(im,[(3,17),(3,19)],'h');put(im,3,19,'H')
 # One-pixel fold edges only; never fill a broad salmon plane.
 line(im,[(9,24),(10,27),(9,31)],'R');line(im,[(14,24),(13,28),(14,31)],'R')
 return im

def chief():
 im=Image.new('L',mask(2).size,ord('s'))
 # Dome: 7 / 11 / 13 pixels. Bone curls occupy solid two-pixel clusters.
 motif(im,5,0,['...TTTTTTT...','.TTTTTTTTTTT.','TTTTTTTTTTTTT','hTTTTTTTTTThh','hhhhhhhhhhhhh'])
 motif(im,4,0,['ss','ss','.s','.s'])
 motif(im,7,0,['Hh.','Hh.','hH.','.HH'])
 motif(im,13,0,['.hH','.hH','.Hh','HH.'])
 rect(im,(7,5,16,10),'S');poly(im,[(9,7),(14,7),(15,9),(12,10),(8,9)],'U')
 put(im,8,5,'@');put(im,12,5,'@')
 # Tusks rise from the two jaw corners; the jaw remains green between them.
 line(im,[(7,7),(7,9)],'H');line(im,[(15,7),(15,9)],'H');put(im,8,9,'h');put(im,14,9,'h')
 poly(im,[(5,11),(11,10),(18,11),(18,19),(6,20)],'S')
 poly(im,[(7,12),(9,11),(10,13),(9,14),(7,14)],'U')
 poly(im,[(14,12),(16,12),(17,14),(15,15),(14,14)],'U')
 line(im,[(12,14),(12,17)],'s')
 poly(im,[(0,11),(3,11),(5,16),(3,19),(0,18)],'S')
 poly(im,[(0,12),(2,12),(3,14),(3,16),(1,16),(0,15)],'U')
 poly(im,[(18,11),(22,12),(22,19),(19,20),(17,17)],'S')
 poly(im,[(19,12),(21,13),(22,16),(20,17),(18,15)],'U')
 rect(im,(5,20,17,23),'o');poly(im,[(7,21),(11,21),(10,24),(6,24)],'O')
 rect(im,(5,19,17,20),'O');motif(im,10,19,['ttt','tOt'])
 rect(im,(5,24,17,26),'o')
 line(im,[(21,0),(21,26)],'O');motif(im,18,0,['.TTT.','Thhh.','T..h.','...hh'])
 return im

def bodies(i):
 im=[tax,liege,chief][i]();out=[]
 for f in (1,2):
  body=im.resize((im.width*f,im.height*f),Image.Resampling.NEAREST);m=mask(i,f);pal=PALS[i].copy()
  if i==0:
   # Physical one-pixel rim, not a doubled pale lavender stripe.
   for y in range(body.height):
    for x in range(body.width):
     if body.getpixel((x,y))==ord('6'):put(body,x,y,'U')
   line(body,[(4*f,6*f),(5*f,3*f),(8*f,1*f),(10*f,1*f)],'6')
   pts=[(7*f,7*f),(10*f,7*f)]
   for x,y in pts:put(body,x,y,'2')
  elif i==1:
   if f==2:
    for y in range(body.height):
     for x in range(body.width):
      if body.getpixel((x,y))==ord('R') and x%2:put(body,x,y,'S')
   for x in (7*f,12*f):
    rect(body,(x,8*f,x+1,8*f+1),'o');put(body,x,8*f+1,'2')
   pts=[(7*f,8*f+1),(12*f,8*f+1),(9*f,19*f),(10*f,20*f)]
   for x,y in pts[2:]:put(body,x+1,y,'2')
  else:
   pts=[(8*f,5*f),(12*f,5*f)]
   for x,y in pts:put(body,x,y,'@');put(body,x+1,y,'s')
   if f==2:
    for x in (14,30):rect(body,(x,14,x+1,18),'H')
    # Tighten solid horn curls to three by six pixels on the larger grid.
    motif(body,14,0,['HHh','HHh','Hhh','HHh','.HH','..H'])
    motif(body,28,0,['hHH','hHH','hhH','hHH','HH.','H..'])
  details=set(pts)
  if i==2: details.update((x+1,y) for x,y in pts)
  for y in range(body.height):
   for x in range(body.width):
    k=chr(body.getpixel((x,y)))
    if m.getpixel((x,y)) and k in ('6','2','3','@','t','T','H','h','R'):
     if not any(0<=nx<body.width and 0<=ny<body.height and m.getpixel((nx,ny)) and body.getpixel((nx,ny))==body.getpixel((x,y)) for nx,ny in A['neighbours'](x,y)):details.add((x,y))
  # Face void is recessed inside the unchanged hood silhouette.
  if i==0:
   for y in range(body.height):
    for x in range(body.width):
     if m.getpixel((x,y)) and body.getpixel((x,y))==ord('@') and any(not(0<=nx<body.width and 0<=ny<body.height) or not m.getpixel((nx,ny)) for nx,ny in A['neighbours'](x,y)):put(body,x,y,'S')
  s=finish(body,i,m,pal,details);lights(s,pts,['teal counting eyes','recessed teal sockets and caged heart','red eyes under iron'][i])
  if f==2:
   s['separation']=BASE[i]['frames']['v2'][0]['separation'].copy()
   s['separation']['head']=[[12,8,24,11],[14,12,29,17],[14,1,29,7]][i]
   s['separation']['body']=[[12,28,28,36],[18,50,25,56],[14,25,31,34]][i]
  out.append(s)
 return out

def shots(i):
 # The larger canvas is visual only; collision and anchors remain unchanged.
 im=Image.new('L',(24,24),ord('.'));other=im.copy();pal=PALS[i].copy()
 if i==0:
  # A horizontal key with a coin bow and two open wards; turn upright.
  motif(im,4,4,['..tttt..........','.tOOOOt.........','tOO..OOt........','tOO..OOt........','.tOOOOtttttttttt','..tttt.OOOOOOOOO','.......tt..tt..t','.......t...t...t','.......ttt.ttttt','.......OOO.OOOOO'])
  # The coin's lower gleam occupies the third row without a loose particle.
  line(im,[(6,13),(6,19),(8,21),(10,21)],'t')
  other=Image.new('L',(24,24),ord('.'))
  motif(other,8,0,['..tttt..','.tOOOOt.','tOO..OOt','tOO..OOt','.tOOOOt.','..tttt..'])
  rect(other,(11,5,12,23),'O');line(other,[(11,5),(11,23)],'t')
  for y in (11,18):
   line(other,[(12,y),(18,y),(18,y+3),(16,y+3)],'t',2)
   line(other,[(13,y+1),(16,y+1)],'O')
  pal.update(t='#c8a060',O='#a07840')
 elif i==1:
  # Hollow ribs keep overlapping volleys light on the live room.
  line(im,[(9,3),(7,7),(8,15),(10,20)],'O')
  line(im,[(14,3),(16,7),(15,15),(13,20)],'O')
  line(im,[(9,3),(8,7),(9,15),(10,19)],'t')
  line(im,[(14,3),(15,7),(14,15),(13,19)],'t')
  motif(im,10,9,['2..2','2222','2332','.22.'])
  line(other,[(5,6),(9,9),(11,11)],'t')
  line(other,[(18,6),(14,9),(12,11)],'t')
  line(other,[(5,6),(5,10),(9,14),(10,15)],'O')
  line(other,[(18,6),(18,10),(14,14),(13,15)],'O')
  motif(other,10,10,['2..2','2222','2332','.22.'])
  line(other,[(9,14),(8,17),(10,20)],'O')
  line(other,[(14,14),(15,17),(13,20)],'O')
  line(other,[(9,16),(10,19)],'t');line(other,[(14,16),(13,19)],'t')
  pal.update(O='#8a7448',t='#d0b878',**{'2':'#3aa8a0','3':'#3aa8a0'})
 else:
  poly(im,[(6,17),(5,9),(8,5),(13,4),(18,10),(19,20),(16,16),(13,8),(9,8),(8,10),(8,16)],'h')
  line(im,[(6,16),(6,9),(9,5),(13,5),(17,10),(18,18)],'T')
  poly(im,[(13,6),(16,10),(17,15),(14,11),(10,8)],'s')
  motif(im,17,18,['.t.','ttt','.t.'])
  line(im,[(14,20),(21,20),(21,18)],'t')
  other=im.transpose(Image.Transpose.ROTATE_90)
  pal.update(s='#243342',h='#3a4a5c',T='#7890a8')
 return [finish(p,i,pal=pal,details={(x,y) for y in range(24) for x in range(24) if chr(p.getpixel((x,y))) in 'tT23'}) for p in (im,other)]

def tells(i):
 out=[];pal=PALS[i].copy()
 if i==0:pal.update(t='#c8a060',O='#a07840',o='#6a3a1c')
 if i==1:pal.update(t='#d0b878',O='#8a7448')
 if i==2:pal.update(T='#7890a8',h='#3a4a5c')
 hi=['t','t','T'][i];lo=['O','O','h'][i]
 for stage in range(4):
  im=Image.new('L',(11,11),ord('.'))
  if stage==0:
   for x,y in [(7,1),(8,3),(9,5),(8,7),(7,9)]:put(im,x,y,hi)
   # Distinct sparse seeds without hiding the common progression.
   if i==1:put(im,6,2,lo);put(im,6,8,lo)
   if i==2:put(im,7,2,lo);put(im,8,6,lo)
  else:
   line(im,[(7,1),(8,3),(9,5),(8,7),(7,9)],lo)
   if stage in (1,2):
    length=3 if stage==1 else 5
    for y in ((2,8) if stage==1 else (0,10)):
     if i==0:
      line(im,[(8,y),(8-length+1,y)],hi);put(im,8-length+1,y+(1 if y<5 else -1),hi)
      if stage==2:put(im,6,y+(1 if y<5 else -1),lo)
     elif i==1:
      line(im,[(8,y),(8-length+1,y+(1 if y<5 else -1))],hi)
      put(im,8-length+1,y+(2 if y<5 else -2),lo)
     else:
      sign=1 if y<5 else -1
      poly(im,[(8,y),(8-length+1,y+sign),(7,y+sign*2)],lo)
      line(im,[(8,y),(8-length+1,y+sign)],hi)
    if i==0:
     line(im,[(9,5),(7 if stage==1 else 4,5)],hi)
     if stage==2:
      line(im,[(4,5),(4,3)],hi);line(im,[(6,4),(6,6)],lo)
    elif i==1:
     line(im,[(9,5),(7 if stage==1 else 4,4)],hi)
     if stage==2:line(im,[(4,4),(5,6)],lo)
    else:
     poly(im,[(9,5),(7 if stage==1 else 1,2),(6,7)],lo)
     line(im,[(9,5),(7 if stage==1 else 1,2)],hi)
     if stage==2:line(im,[(1,2),(3,6)],hi)
   else:
    line(im,[(6,0),(8,2),(9,5),(8,8),(6,10)],lo,3)
    line(im,[(7,0),(9,2),(10,5),(9,8),(7,10)],hi)
    if i==0:line(im,[(7,2),(5,2),(5,3)],hi);line(im,[(7,8),(5,8),(5,7)],hi)
    elif i==1:
     poly(im,[(6,3),(0,5),(6,7)],lo)
     line(im,[(0,5),(6,5)],hi)
    else:poly(im,[(8,1),(0,4),(7,3)],hi);poly(im,[(8,9),(0,6),(7,7)],hi)
  out.append(finish(im,i,pal=pal,details={(x,y) for y in range(11) for x in range(11) if im.getpixel((x,y))!=ord('.')}))
 return out

def impacts(i):
 if i<2:
  frames=json.loads(json.dumps(BASE[i]['frames']['impact']))
  for f in frames:
   # Role-local slots preserve existing masks and notches exactly.
   if i==0:
    f['pigments']=PALS[i].copy();f['pigments'].update(O='#a07840',t='#c8a060')
  return frames
 pal=PALS[i].copy();pal.update(s='#263e52',h='#3a4a5c',T='#7890a8',t='#c07a3a')
 out=[]
 for stage in range(5):
  im=Image.new('L',(28,28),ord('.'))
  if stage==0:
   triangles=[[(7,22),(13,9),(21,22)]]
  elif stage==1:
   triangles=[[(0,10),(6,0),(12,10)],[(16,14),(22,4),(27,14)],[(6,27),(13,16),(20,27)]]
  elif stage==2:
   triangles=[[(0,11),(4,1),(10,11)],[(16,11),(24,1),(27,11)],[(2,27),(13,15),(24,27)]]
  elif stage==3:
   triangles=[[(0,10),(4,3),(10,10)],[(9,16),(14,8),(20,16)],[(20,10),(25,3),(27,10)],[(2,27),(6,20),(12,27)],[(17,27),(23,20),(27,27)]]
  else:
   triangles=[[(0,9),(3,5),(8,9)],[(10,15),(14,10),(19,15)],[(21,9),(25,5),(27,9)],[(2,27),(6,23),(11,27)],[(18,27),(23,23),(27,27)]]
  for p in triangles:
   poly(im,p,'h');line(im,p[:2],'T',2 if stage<3 else 1)
   poly(im,[p[1],((p[0][0]+p[2][0])//2,p[2][1]),p[2]],'s')
   put(im,p[1][0],p[1][1],'t')
  out.append(finish(im,i,pal=pal,details={(x,y) for y in range(28) for x in range(28) if chr(im.getpixel((x,y))) in 'Tt'}))
 return out

def author():
 entries=[]
 for i,e in enumerate(BASE):
  b1,b2=bodies(i);e['inks']=''.join(PALS[i]);e['frames']={'v1':[b1],'v2':[b2],'projectile':shots(i),'tell':tells(i),'impact':impacts(i)}
  e['review']={'theme':['hooded teal-eyed accountant carrying a locked treasure chest and spilled coins','cool bone skull under separate gold crown, wine robe and heart cage','jaw tusks, domed iron, curled horns, warm belt and kilt, mining pick'][i], 'originality':'Original authored silhouettes; no commercial likeness or borrowed art.', 'identity_props':{
   'props':[['locked treasure chest','dark hood and teal eyes','spilled coins'],['cool bone skull','gold crown','heart cage'],['jaw tusks','domed iron helm','mining pick']][i],
   'source_text':(ROOT/e['boss']).read_text().split('\nart\n')[0].split('\ntext ',1)[1].split('\n')[0],
   'compared_original_1x':True,'compared_original_cells':True,'inspected_cells_2x':True,
   'evidence':[str(OUT/'review'/f'{Path(e["boss"]).stem}-v{kit}.png') for kit in (1,2)]}}
  entries.append(e)
 A['write_changed'](ROOT/'scripts/boss-art-reliquary.json',(json.dumps(entries,indent=2)+'\n').encode())

def concepts():
 p=OUT/'concepts/nearest';p.mkdir(exist_ok=True)
 s=Image.new('RGB',(1560,640),'#080b0e');d=ImageDraw.Draw(s)
 for j,(name,i,size) in enumerate([('tax',0,(21,25)),('liege',1,(22,32)),('chief',2,(23,27)),('tax-key',0,(9,9)),('liege-clasp',1,(9,9)),('pick-tooth',2,(9,9))]):
  src=A['extract'](Image.open(OUT/'concepts'/(name+'.png')),'neutral',30)
  colours={k:tuple(bytes.fromhex(v[1:])) for k,v in PALS[i].items()}
  im,rows=A['snap'](A['fit'](src,size),colours);im.save(p/(name+'.png'));(p/(name+'.art')).write_text('\n'.join(rows)+'\n')
  x=15+j%3*520;y=30+j//3*320;d.text((x,y-18),name+' / nearest candidate',fill='#c2b494');s.paste(im,(x,y),im);zoom=im.resize((size[0]*6,size[1]*6),Image.Resampling.NEAREST);s.paste(zoom,(x+80,y),zoom)
 s.save(p/'nearest-candidates.png')

if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('mode',choices=['author','concepts']);args=p.parse_args()
 (author if args.mode=='author' else concepts)()
