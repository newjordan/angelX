#!/usr/bin/env python3
"""Construction, acting and exact-scale Latch comparison for the native kit."""
import argparse,json,itertools,colorsys
from pathlib import Path
from collections import Counter
from PIL import Image,ImageDraw,ImageFont,ImageChops
ROOT=Path(__file__).resolve().parents[1];ASSETS=ROOT/'crates/delve/assets/arcade'
p=argparse.ArgumentParser(__doc__);p.add_argument('--output',type=Path,required=True);p.add_argument('--latch',type=Path,required=True);args=p.parse_args();out=args.output;out.mkdir(parents=True,exist_ok=True)
font=ImageFont.truetype('/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf',12);bg='#080b0e';text='#b0b2a8'
def paste(sheet,a,x,y,z=1):
 a=a.resize((a.width*z,a.height*z),Image.Resampling.NEAREST);sheet.paste(a,(x,y),a if a.mode=='RGBA' else None)
def label(sheet,xy,s):ImageDraw.Draw(sheet).text(xy,s,font=font,fill=text)
new=Image.open(ASSETS/'knight/idle-0.png');old=Image.open(ROOT/'crates/delve/tests/fixtures/arcade/stiff-atlas.png').crop((0,0,64,56))
# The retained renderer export is already 3x. Recover its actual 70px model,
# omitting only the clipped final expanded scanline, never normalising scale.
a=Image.open(args.latch).convert('RGB').crop((40,75,201,290));box=ImageChops.difference(a,Image.new('RGB',a.size,bg)).getbbox();latch=a.crop(box);assert latch.width%3==0,(box,latch.size);latch=latch.crop((0,0,latch.width,latch.height//3*3));latch=latch.resize((latch.width//3,latch.height//3),Image.Resampling.NEAREST)
sheet=Image.new('RGB',(1080,480),bg)
for i,(name,a) in enumerate([('Rejected stiff model',old),('Acting / same construction',new),('Original Latch renderer',latch)]):
 x=20+i*355;label(sheet,(x,10),name);label(sheet,(x,32),'1x / original scale');paste(sheet,a,x+40,145-a.height);label(sheet,(x,170),'3x / original samples');paste(sheet,a,x+25,450-a.height*3,3)
sheet.save(out/'old-new-latch-1x-3x.png')
m=json.loads((ASSETS/'knight/manifest.json').read_text());bank=json.loads((ASSETS/'palette.json').read_text());ramps=bank['ramps']
sheet=Image.new('RGB',(1252,560),bg);d=ImageDraw.Draw(sheet)
label(sheet,(20,12),'CONSTRUCTION TURNAROUND / act beyond these poses / 46px standing / [30,51] anchor')
paste(sheet,new,20,62);paste(sheet,new,110,43,3)
for label0,y0,y1 in [('plume 6',6,12),('helmet / face 13',12,25),('torso / gorget 14',25,39),('legs / boots 13',39,52)]:
 x=310;d.line((x,43+y0*3,x,43+y1*3-1),fill=text);d.line((x-4,43+y0*3,x+4,43+y0*3),fill=text);label(sheet,(x+12,43+(y0+y1)*3//2-6),label0)
label(sheet,(535,48),'Dented crown / wrapped repair / raised visor / dark moustache')
label(sheet,(535,73),'8x6 pauldron / steel fist, 2px brass repair / 8px plated boots')
label(sheet,(535,98),'Broad cel folds. Match volumes and ramps; exaggerate acting.')
for j,(name,box) in enumerate([('helmet repair',(23,12,42,26)),('pauldron',(35,25,43,31)),('steel fist',(45,34,52,41)),('boot / 2px sole',(34,46,42,52))]):
 x=540+j*165;label(sheet,(x,140),name);paste(sheet,new.crop(box),x,165,3)
for j,mat in enumerate(['steel','cloth','bandage']):
 x=20+j*400;label(sheet,(x,245),mat)
 for i,c in enumerate(ramps[mat][1:] if mat!='bandage' else ramps[mat]):d.rectangle((x+75+i*28,243,x+100+i*28,260),fill='#'+c)
label(sheet,(20,288),'FOUR EXPRESSIONS / actual pose pixels / 1x then 3x / dark eye and moustache stay in cells')
for j,(st,i,name) in enumerate([('idle',0,'neutral'),('attack',2,'gritted'),('hit',0,'open shout'),('victory',2,'grin')]):
 f=m['states'][st]['frames'][i];im=Image.open(ASSETS/'knight'/f['file']);hx,hy=f['landmarks']['head'];hx-=9;hy-=6;head=im.crop((hx,hy,hx+20,hy+15));x=20+j*305
 label(sheet,(x,320),name);paste(sheet,head,x,350);paste(sheet,head,x+45,341,3);paste(sheet,im,x+115,335,3)
label(sheet,(20,520),'Pose/timing reference: run, slash and back landing are actions, never static model stamps.')
sheet.save(out/'model-sheet.png')
# One small review page for the three requested action strips, retaining timing.
sheet=Image.new('RGB',(1252,620),bg)
for row,state in enumerate(['run','attack','defeat']):
 label(sheet,(10,row*205+8),state+' / '+str(m['states'][state]['durations_ms'])+' ms')
 strip=Image.open(out/f'knight-{state}-strip.png');paste(sheet,strip,80,row*205+30,3)
sheet.save(out/'acting-strips-3x.png')
a=Image.open(out/'knight-sheet-1x.png');b=Image.open(out/'knight-sheet-3x.png');sheet=Image.new('RGB',(max(a.width,b.width),a.height+b.height+30),bg);paste(sheet,a,0,0);paste(sheet,b,0,a.height+30);sheet.save(out/'knight-sheet-1x-3x.png')
def luma(c):return sum(w*x for w,x in zip([299,587,114],bytes.fromhex(c)))//1000
measure=[];images={}
for state,anim in m['states'].items():
 for f in anim['frames']:
  im=Image.open(ASSETS/'knight'/f['file']);images[f['file']]=im;cs=Counter(bytes(p[:3]).hex() for p in im.get_flattened_data() if p[3]);mass=sum(cs.values());r=f['material_regions'][1];coat=Counter(bytes(im.getpixel(tuple(p))[:3]).hex() for p in r['pixels']);n=sum(coat[c] for c in ramps['cloth']);helmet=f['parts']['helmet'];box=[min(p[0] for p in helmet),min(p[1] for p in helmet),max(p[0] for p in helmet)+1,max(p[1] for p in helmet)+1];sn=sum(cs[c] for c in ramps['steel'][1:]);steel=sum(cs[c]*luma(c) for c in ramps['steel'][1:])//sn
  contrasts=[abs(steel-sum(coat[c]*luma(seat[ramps['cloth'].index(c)]) for c in ramps['cloth'])//n) for seat in bank['seats']]
  measure.append(dict(file=f['file'],state=state,mass=mass,contour_percent=round(sum(cs[c] for c in ['22303a','322b2b','0a0c12'])/mass*100,2),coat_percent={c:round(coat[c]/n*100,2) for c in ramps['cloth']},helmet_box=box,coat_steel_delta_l=contrasts,head_lead=f['landmarks']['head'][0]-f['landmarks']['hip'][0],expression=f['expression']))
poses=['idle-0.png','guard-0.png','hit-0.png','attack-2.png','victory-2.png'];solid=lambda im:{(x,y) for y in range(im.height) for x in range(im.width) if im.getpixel((x,y))[3]};ious=[]
for a,b in itertools.combinations(poses,2):
 aa,bb=solid(images[a]),solid(images[b]);ious.append(dict(a=a,b=b,iou=round(len(aa&bb)/len(aa|bb),4)))
(out/'model-measurements.json').write_text(json.dumps(dict(frames=measure,pose_iou=ious,latch_native=list(latch.size)),indent=2)+'\n')
pack=['model-sheet.png','old-new-latch-1x-3x.png','knight-sheet-1x-3x.png','acting-strips-3x.png','seat-swaps-3x.png','fx-explosion-3x.png','fx-sheet-3x.png']+[f'knight-seat{i}-cells-all.png' for i in range(1,5)]+['fx-cells-all.png']
assert len(pack)==12
for name in pack:
 if (out/name).exists():assert Image.open(out/name).width<1800,name
(out/'review-images.json').write_text(json.dumps(pack,indent=2)+'\n')
print('Latch native:',latch.size,'; contour:',min(f['contour_percent'] for f in measure),max(f['contour_percent'] for f in measure),'; max IoU:',max(f['iou'] for f in ious),'; minimum coat delta-L:',[min(f['coat_steel_delta_l'][i] for f in measure) for i in range(4)])
