#!/usr/bin/env python3
"""Convert Brasswake sheets at one shared scale with the realm's existing inks."""
from pathlib import Path
from PIL import Image, ImageDraw
import argparse, importlib.util, json
from collections import Counter
spec=importlib.util.spec_from_file_location('cinder',Path(__file__).with_name('cinder-art.py'))
base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
P=[0x192934,0x263b40,0x2f4a52,0x12516a,0x3a6d5d,0x6e8a8e,0x8fa3a8,
   0x45291c,0x652c10,0x784620,0x924f1d,0xa97639,0xbe914b,0xd8a95e,0xe2cb8b,0x431e0f]
K=[0x192934,0x263b40,0x2f4a52,0x456070,0x6e8a8e,0x8fa3a8,0xb3c2c4,
   0x431e0f,0xa8341f,0x784620,0xa97639,0xd8a95e,0xe2cb8b]
STATES=['idle','walk-1','walk-2','slam-tell','slam','rush-tell','rush','flinch','damage-1','damage-2','defeat','recover']
KNIGHTS=['idle','walk-1','walk-2','jab','cross','kick','grab','throw','flinch','down','guard','recover']
def components(im):
    p=im.load();seen=set();parts=[]
    for y in range(im.height):
        for x in range(im.width):
            if (x,y) in seen or not p[x,y][3]:continue
            todo=[(x,y)];seen.add((x,y));part=[]
            while todo:
                xx,yy=todo.pop();part.append((xx,yy))
                for n in [(xx-1,yy),(xx+1,yy),(xx,yy-1),(xx,yy+1)]:
                    if 0<=n[0]<im.width and 0<=n[1]<im.height and n not in seen and p[n[0],n[1]][3]:seen.add(n);todo.append(n)
            parts.append(part)
    if parts:
        largest=max(parts,key=len)
        for part in parts:
            if part is not largest:
                for x,y in part:p[x,y]=(0,0,0,0)
    return im

def strip(im):
    im=im.convert('RGBA');p=im.load()
    seen=set();todo=[(x,y) for x in range(im.width) for y in [0,im.height-1]]+[(x,y) for y in range(im.height) for x in [0,im.width-1]]
    while todo:
        x,y=todo.pop()
        if (x,y) in seen or not (0<=x<im.width and 0<=y<im.height):continue
        seen.add((x,y));r,g,b,a=p[x,y]
        if max(r,g,b)-min(r,g,b)<28 and 60<r<195:
            p[x,y]=(0,0,0,0);todo.extend([(x-1,y),(x+1,y),(x,y-1),(x,y+1)])
    return components(im)

def snap(im,palette):
    cs=[((v>>16)&255,(v>>8)&255,v&255) for v in palette];p=im.load()
    for y in range(im.height):
        for x in range(im.width):
            if p[x,y][3]:p[x,y]=(*min(cs,key=lambda c:sum((c[k]-p[x,y][k])**2 for k in range(3))),255)
    return base.clean(im)

def sheet(items,path,scale):
    assert len(items)<=12
    w=max(im.width for _,im in items);h=max(im.height for _,im in items);cw=w*scale+20;ch=h*scale+28
    out=Image.new('RGB',(cw*4,ch*3));d=ImageDraw.Draw(out)
    for n,(name,im) in enumerate(items):
        x=n%4*cw+10;y=n//4*ch;d.text((x,y+5),name,fill='#b0b2a8')
        big=im.resize((im.width*scale,im.height*scale),Image.Resampling.NEAREST);out.paste(big,(x,y+24),big)
    out.save(path)

def run(src,out,review):
    out.mkdir(exist_ok=True,parents=True);review.mkdir(exist_ok=True,parents=True)
    anchors={}
    for kind,size,names,palette in [('keeper',64,STATES,P),('porter',48,STATES,P),('knight',48,KNIGHTS,K),('effects',32,[f'{r}-{f}' for r in ['contact','slam','steam'] for f in range(4)],P)]:
        source=Image.open(src/(kind+'.png'));tiles=[];anchors[kind]=[]
        for n in range(12):
            tile=strip(source.crop((n%4*source.width//4,n//4*source.height//3,(n%4+1)*source.width//4,(n//4+1)*source.height//3)))
            tiles.append(tile.crop(tile.getbbox()))
        # One source scale across the complete state set: a raised arm does not shrink the body.
        factor=min((size-2)/max(t.width for t in tiles),(size-2)/max(t.height for t in tiles));items=[]
        for n,t in enumerate(tiles):
            t=t.resize((round(t.width*factor),round(t.height*factor)),Image.Resampling.NEAREST)
            if kind=='porter':
                p=t.load()
                for yy in range(t.height):
                    for xx in range(t.width):
                        r,g,b,a=p[xx,yy]
                        if a and r>g*1.3 and b>g*1.2 and b>r*.65:
                            p[xx,yy]=(*( (47,74,82) if r>80 else (25,41,52)),a)
            tile=Image.new('RGBA',(size,size));tile.alpha_composite(t,((size-t.width)//2,size-t.height));tile=components(snap(tile,palette))
            old=tile.copy();p=tile.load();q=old.load()
            for yy in range(1,size-1):
                for xx in range(1,size-1):
                    ns=[q[xx-1,yy],q[xx+1,yy],q[xx,yy-1],q[xx,yy+1]]
                    if q[xx,yy][3] and all(c[3] for c in ns) and q[xx,yy] not in ns:
                        p[xx,yy]=max(ns,key=ns.count)
            if kind in ['keeper','porter']:
                eye_sources={'keeper':[(140,135),(500,135),(860,135),(1250,115),(132,524),(548,462),(883,512),(1232,450),(140,768),(495,778),(885,925),(1255,780)],
                             'porter':[(120,140),(480,130),(840,130),(1235,140),(145,526),(500,529),(840,500),(1260,474),(120,800),(482,801),(980,930),(1252,820)]}
                panel=strip(source.crop((n%4*source.width//4,n//4*source.height//3,(n%4+1)*source.width//4,(n//4+1)*source.height//3)))
                box=panel.getbbox();sx,sy=eye_sources[kind][n]
                cx=round((sx-n%4*source.width//4-box[0])*factor)+(size-t.width)//2
                ey=round((sy-n//4*source.height//3-box[1])*factor)+size-t.height
                anchors[kind].append([cx,ey])
                d=ImageDraw.Draw(tile)
                for ex in [cx-3,cx+3]:
                    d.polygon([(ex-1,ey-2),(ex+1,ey-2),(ex+2,ey),(ex+1,ey+2),(ex-1,ey+1)],fill='#192934')
                    d.polygon([(ex,ey-1),(ex+1,ey-1),(ex+1,ey),(ex,ey+1),(ex-1,ey)],fill='#d8a95e' if n!=10 else '#a97639')
            tile.save(out/(kind+'-'+names[n]+'.png'));items.append((names[n],tile))
        for scale in [1,3]:sheet(items,review/f'{kind}-{scale}x.png',scale)
    (out/"anchors.json").write_text(json.dumps(anchors,indent=2)+"\n")
    # Two small props share the same native grain as feet and fists.
    for name in ['cask','bread']:
        im=Image.new('RGBA',(24,24));d=ImageDraw.Draw(im)
        if name=='cask':
            d.rounded_rectangle((3,2,20,22),radius=5,fill='#192934');d.rounded_rectangle((4,3,19,21),radius=4,fill='#784620');d.rectangle((7,4,11,20),fill='#be914b');d.rectangle((12,4,17,20),fill='#a97639')
            for y in [7,16]:d.rectangle((3,y,20,y+2),fill='#2f4a52');d.line((4,y,18,y),fill='#6e8a8e')
        else:
            d.rounded_rectangle((2,8,21,20),radius=5,fill='#45291c');d.rounded_rectangle((3,8,20,18),radius=5,fill='#a97639');d.arc((5,7,17,18),190,310,fill='#d8a95e',width=2)
        im.save(out/(name+'.png'))
    sheet([(n,Image.open(out/(n+'.png'))) for n in ['cask','bread']],review/'props-3x.png',3)
def audit(out,review):
    def light(c):
        values=[((v/255+.055)/1.055)**2.4 if v/255>.04045 else v/255/12.92 for v in c[:3]]
        y=sum(a*b for a,b in zip(values,[.2126,.7152,.0722]))
        return 116*y**(1/3)-16 if y>.008856 else 903.3*y
    stats={}
    for path in sorted(out.glob('*.png')):
        im=Image.open(path).convert('RGBA');p=im.load()
        solid=[c for c in im.get_flattened_data() if c[3]]
        edges=[p[x,y] for y in range(im.height) for x in range(im.width) if p[x,y][3] and any(not(0<=x+dx<im.width and 0<=y+dy<im.height) or not p[x+dx,y+dy][3] for dx,dy in [(1,0),(-1,0),(0,1),(0,-1)])]
        row={'inks':len(set(solid)),'near_white':sum(min(c[:3])>235 for c in solid),'outer_floor_L':round(min(map(light,edges))-light((11,10,8)),1)}
        assert row['inks']<=16 and row['near_white']==0,path
        if path.stem.startswith(('keeper','porter','knight')):assert row['outer_floor_L']>=15,path
        if path.stem.startswith(('keeper','porter')):
            face=Counter(c for c in solid if c[2]>c[0] and c[1]>c[0])
            body=Counter(p[x,y] for y in range(im.height//2,im.height*4//5) for x in range(im.width//3,im.width*2//3) if p[x,y][3] and p[x,y][0]>p[x,y][1]*1.15)
            if face and body:
                row['head_body_L']=round(abs(light(body.most_common(1)[0][0])-light(face.most_common(1)[0][0])),1)
                assert row['head_body_L']>=15,path
        stats[path.stem]=row
    for kind in ['keeper','porter']:
        idle=Image.open(out/(kind+'-idle.png'))
        for pose in ['slam-tell','rush-tell']:
            other=Image.open(out/(kind+'-'+pose+'.png'))
            diff=sum(a!=b for a,b in zip(idle.get_flattened_data(),other.get_flattened_data()))/(idle.width*idle.height)
            assert diff>.2,(kind,pose,diff)
            stats[kind+'-'+pose]['idle_diff']=round(diff,3)
    (review/'gates.json').write_text(json.dumps(stats,indent=2)+'\n')
    print(f"Art gates: {len(stats)} sprites, <=16 inks, no near-white; outer/floor and creature head/body >=15 L*, distinct tells.")

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('source',type=Path);p.add_argument('--out',type=Path,default=Path('crates/delve/assets/brasswake'));p.add_argument('--review',type=Path,required=True);a=p.parse_args();run(a.source,a.out,a.review);audit(a.out,a.review)
