#!/usr/bin/env python3
"""Rebuild the hand-drawn knight and retained arcade effects at native grain.

python scripts/arcade-art.py build
python scripts/arcade-art.py review --output /work/artifacts/boss-art-20261015/style-1
Cargo's build script owns the hard gate; no filtering or palette work at runtime.
"""
import argparse
from collections import Counter
import json
import os
import subprocess
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT/'crates/delve/assets/arcade'
# The only extra pigments: six cloth reds, two ember oranges.
EXTRA = ['38212d','61303b','853d43','ad4e49','cc6854','dc8768','a34828','d77b39']
RAMPS = {
    'bandage': ['7a7c72','94968c','b0b2a8'],
    'steel': ['192934','2f4a52','456070','6e8a8e','8fa3a8','b3c2c4'],
    'cloth': EXTRA[:6],
    'leather': ['331a0c','45291c','684629','924f1d','a97639','be914b'],
    'fire': ['652c10','a34828','d77b39','d8a95e','e2cb8b'],
    'smoke': ['22303a','322b2b','3d3a36','515349','63655c','7a7c72'],
}
SEATS = [RAMPS['cloth'], ['192934','12516a','2f4a52','456070','527597','639dc2'],
         ['263b1c','335022','35543f','3f6428','5a7330','8aa34a'],
         ['263b40','12516a','35543f','3a6d5d','6fa46d','a8c05f']]
CONTOUR = ['22303a','322b2b']
STATES = [('idle',4,140),('run',6,90),('jump',2,110),('fall',1,160),('land',2,80),
          ('attack',6,110),('hit',2,110),('guard',2,160),('defeat',5,160),('victory',3,170)]
FX = [('explosion',12,75),('muzzle',3,65),('impact',4,75),('dust',5,90),('spark',3,65)]

def rgb(h): return tuple(bytes.fromhex(h))
def hexrgb(c): return bytes(c[:3]).hex()
def ns(p):
    x,y=p
    return [(x-1,y),(x+1,y),(x,y-1),(x,y+1)]
def parts(points):
    left=set(points)
    while left:
        part={left.pop()}; todo=list(part)
        while todo:
            for p in ns(todo.pop()):
                if p in left: left.remove(p);part.add(p);todo.append(p)
        yield part

def cut(source,index,kind):
    # Fixed equal-cell layout is retained with the source, not detected from
    # current PNG content. Flood neutral-grey paper; do not key dark pigments.
    w,h=source.size; col,row=index%6,index//6
    ys=[0,257,520,750,940,h] if kind=='fx' else [i*h//5 for i in range(6)]
    box=(col*w//6,ys[row],(col+1)*w//6,ys[row+1])
    if kind=='fx' and index>=12:
        if index<19:
            edges=[20,195,445,645,825,1030,1205,1374];j=index-12;box=(edges[j],530,edges[j+1],745)
        elif index<24:
            edges=[20,180,430,735,990,1210];j=index-19;box=(edges[j],770,edges[j+1],935)
        else:
            edges=[20,200,460,700];j=index-24;box=(edges[j],955,edges[j+1],h)
    a=source.crop(box).convert('RGBA')
    paper={(x,y) for y in range(a.height) for x in range(a.width)
           if max(a.getpixel((x,y))[:3])-min(a.getpixel((x,y))[:3])<15
           and 70<min(a.getpixel((x,y))[:3])<170}
    for group in parts(paper):
        if any(x in (0,a.width-1) or y in (0,a.height-1) for x,y in group):
            for p in group:a.putpixel(p,(0,0,0,0))
    box=a.getbbox();return a.crop(box)

def clean(a,ramps):
    inks=[rgb(h) for h in CONTOUR+sum([RAMPS[r] for r in ramps],[]) + ['be914b']]
    px={p:min(inks,key=lambda c:sum((c[k]-a.getpixel(p)[k])**2 for k in range(3)))
        for y in range(a.height) for x in range(a.width) if a.getpixel(p:=(x,y))[3]>127}
    # Every exterior sample is the same readable indigo; internal contours from
    # the source survive. Tiny clusters become a neighbouring material mass.
    edge={p for p in px if any(q not in px for q in ns(p))}
    for p in edge:px[p]=rgb(CONTOUR[0])
    for _ in range(4):
        changes={}
        for c in set(px.values()):
            for group in parts(p for p in px if px[p]==c):
                if len(group)>=4:continue
                if group & edge:continue
                choices=Counter(px[q] for p in group for q in ns(p) if q in px and q not in group)
                if choices:
                    ink=choices.most_common(1)[0][0]
                    for p in group:changes[p]=ink
        if not changes:break
        px.update(changes)
    # Tiny detached islands are paper, not particle texture.
    for group in list(parts(px)):
        if len(group)<4:
            for p in group:del px[p]
    a=Image.new('RGBA',a.size)
    for p,c in px.items():a.putpixel(p,c+(255,))
    return a

def finish(a,features):
    px={p:a.getpixel(p) for y in range(a.height) for x in range(a.width) if a.getpixel(p:=(x,y))[3]}
    edge={p for p in px if any(q not in px for q in ns(p))}
    for p in edge:px[p]=rgb(CONTOUR[0])+(255,)
    keep={tuple(p) for f in features for p in f['pixels']}
    def connected(points):
        left=set(points)
        while left:
            group={left.pop()};todo=list(group)
            while todo:
                x,y=todo.pop()
                for dx,dy in [(-1,-1),(0,-1),(1,-1),(-1,0),(1,0),(-1,1),(0,1),(1,1)]:
                    q=x+dx,y+dy
                    if q in left:left.remove(q);group.add(q);todo.append(q)
            yield group
    def checker(x,y):
        a=px.get((x,y));b=px.get((x+1,y))
        return a is not None and b is not None and a!=b and a==px.get((x+1,y+1)) and b==px.get((x,y+1))
    for y in range(a.height-2):
        for x in range(a.width-2):
            if checker(x,y) and (checker(x+1,y) or checker(x,y+1)):
                for p in [(x+1,y+1),(x,y),(x+1,y),(x,y+1)]:
                    if p not in edge and p not in keep:
                        px[p]=Counter(px[q] for q in ns(p) if q in px and q not in keep).most_common(1)[0][0];break
    while True:
        changes={}
        clusters=[g for c in sorted(set(px.values())) for g in connected(p for p in px if px[p]==c)]
        big=set().union(*(g for g in clusters if len(g)>=4))
        for group in clusters:
            if len(group)>=4 or group<=keep:continue
            choices=Counter(px[q] for p in group for q in ns(p) if q in big and p not in edge)
            if choices:
                c=choices.most_common(1)[0][0]
                for p in group:
                    if p not in edge and p not in keep:changes[p]=c
        if not changes:break
        px.update(changes)
    a=Image.new('RGBA',a.size)
    for p,c in px.items():a.putpixel(p,c)
    return a

def write_frame(a,folder,name,materials,extra=None):
    a=finish(a,extra or [])
    path=folder/(name+'.png');a.save(path)
    regions=[]
    for material,minimum in materials.items():
        colours=set(map(rgb,RAMPS[material]));points={(x,y) for y in range(a.height) for x in range(a.width) if a.getpixel((x,y))[:3] in colours and a.getpixel((x,y))[3]}
        group=max(parts(points),key=len)
        if material=='cloth':
            # Belt and baldric split the coat's connected mask, but remain
            # within the same authored material region; exclude the plume.
            top=min(y for x,y in group)-5;group={p for p in points if p[1]>=top}
        box=[min(x for x,y in group),min(y for x,y in group),max(x for x,y in group)+1,max(y for x,y in group)+1]
        regions.append({'name':'helmet' if material=='steel' else 'surcoat','material':material,'minimum_tones':minimum,'box':box})
    return {'file':name+'.png','required_materials':materials,'material_regions':regions,'features':extra or []}

def build():
    ASSETS.mkdir(parents=True,exist_ok=True)
    palette={'version':1,'extension':EXTRA,'contours':CONTOUR,'ramps':RAMPS,'seats':SEATS,
             'reason':'Shared material banks lack a five-step red; signal red is reserved. Six cloth and two ember pigments stay local.'}
    (ASSETS/'palette.json').write_text(json.dumps(palette,indent=2)+'\n')
    import importlib.util
    spec=importlib.util.spec_from_file_location('knight',ROOT/'scripts/arcade-knight.py')
    knight=importlib.util.module_from_spec(spec);spec.loader.exec_module(knight)
    folder=ASSETS/'knight';folder.mkdir(exist_ok=True)
    knight.build(folder,STATES)
    fxspec=importlib.util.spec_from_file_location('fx_polish',ROOT/'scripts/arcade-fx-polish.py')
    fx_polish=importlib.util.module_from_spec(fxspec);fxspec.loader.exec_module(fx_polish)
    if (ASSETS/'source/fx.png').exists():
        source=Image.open(ASSETS/'source/fx.png');folder=ASSETS/'fx';folder.mkdir(exist_ok=True)
        manifest={'version':1,'canvas':[64,64],'anchor':[32,54],'states':{}};index=0
        for state,n,duration in FX:
            frames=[]
            for i in range(n):
                crop=cut(source,index,'fx');index+=1
                scale=48/200
                a=clean(crop.resize((max(3,round(crop.width*scale)),max(3,round(crop.height*scale))),Image.Resampling.NEAREST),['fire','smoke'])
                canvas=Image.new('RGBA',(64,64));canvas.alpha_composite(a,(32-a.width//2,54-a.height))
                canvas=fx_polish.polish(canvas,state,i)
                frames.append(write_frame(canvas,folder,f'{state}-{i}',{}))
            manifest['states'][state]={'durations_ms':[duration]*n,'loop':False,'frames':frames}
        (folder/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')

def review(out):
    out.mkdir(parents=True,exist_ok=True);font=ImageFont.truetype('/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf',12)
    for kit in ['knight','fx']:
        m=json.loads((ASSETS/kit/'manifest.json').read_text());w,h=m['canvas']
        for zoom in [1,3]:
            sheet=Image.new('RGB',(max(620,100+max(len(s['frames']) for s in m['states'].values())*w*zoom),len(m['states'])*(h*zoom+25)+10),'#080b0e');d=ImageDraw.Draw(sheet)
            for row,(state,s) in enumerate(m['states'].items()):
                y=10+row*(h*zoom+25);d.text((8,y),state,font=font,fill='#b0b2a8')
                strip=Image.new('RGB',(len(s['frames'])*w,h),'#080b0e');gif=[]
                for i,f in enumerate(s['frames']):
                    a=Image.open(ASSETS/kit/f['file']);strip.paste(a,(i*w,0),a)
                    b=Image.new('RGB',(w,h),'#080b0e');b.paste(a,(0,0),a);gif.append(b.resize((w*3,h*3),Image.Resampling.NEAREST))
                sheet.paste(strip.resize((strip.width*zoom,strip.height*zoom),Image.Resampling.NEAREST),(100,y+15))
                if zoom==1:
                    strip.save(out/f'{kit}-{state}-strip.png')
                    gif[0].save(out/f'{kit}-{state}.gif',save_all=True,append_images=gif[1:],duration=s['durations_ms'],loop=0,disposal=2)
            # Explosion's twelve frames need two rows at 3x to keep <1800.
            if sheet.width>=1800:
                # Individual strips are native size; split expanded review.
                for row,(state,s) in enumerate(m['states'].items()):
                    if state!='explosion':continue
                    strip=Image.open(out/f'{kit}-{state}-strip.png');a=Image.new('RGB',(1252,2*(h*3+25)),'#080b0e');ad=ImageDraw.Draw(a)
                    for half in range(2):
                        crop=strip.crop((half*6*w,0,min((half+1)*6*w,strip.width),h));ad.text((8,half*(h*3+25)+3),f'{state} {half*6}+',font=font,fill='#b0b2a8');a.paste(crop.resize((crop.width*3,h*3),Image.Resampling.NEAREST),(90,half*(h*3+25)+20))
                    if state=='explosion':a.save(out/f'{kit}-{state}-3x.png')
                # Other FX fit a separate 3x sheet.
                sheet=sheet.crop((0,h*3+35,1252,sheet.height))
            sheet.save(out/f'{kit}-sheet-{zoom}x.png')
    m=json.loads((ASSETS/'knight/manifest.json').read_text());im=Image.open(ASSETS/'knight/idle-0.png');sheet=Image.new('RGB',(1000,240),'#080b0e');d=ImageDraw.Draw(sheet)
    for seat,ramp in enumerate(SEATS):
        a=im.copy();swap=dict(zip(map(rgb,RAMPS['cloth']),map(rgb,ramp)))
        for y in range(a.height):
            for x in range(a.width):
                c=a.getpixel((x,y));a.putpixel((x,y),swap.get(c[:3],c[:3])+(c[3],))
        a=a.resize((192,168),Image.Resampling.NEAREST);sheet.paste(a,(seat*240+10,30),a);d.text((seat*240+10,10),f'SEAT {seat+1}',font=font,fill='#b0b2a8')
    sheet.save(out/'seat-swaps-3x.png')

if __name__=='__main__':
    p=argparse.ArgumentParser(__doc__);p.add_argument('command',choices=['build','review','check']);p.add_argument('--output',type=Path,default=ROOT/'output/arcade');a=p.parse_args()
    if a.command=='build':build()
    elif a.command=='review':review(a.output)
    else:
        env=dict(os.environ,TMPDIR='/work/tmp',ANGEL_CARTRIDGES='/work/tmp/empty-cartridges')
        env.setdefault('CARGO_TARGET_DIR','/work/build-cache/bossart-crates')
        subprocess.run(['cargo','build','--manifest-path',str(ROOT/'crates/Cargo.toml'),'-p','angel-delve','--locked'],env=env,check=True)
