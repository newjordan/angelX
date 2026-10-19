#!/usr/bin/env python3
"""Nearest-sample concepts, clean native clusters, and keep reproducible pixel recipes.
Usage: python scripts/rafterwake-art.py SOURCE_DIRECTORY REVIEW_DIRECTORY
SOURCE_DIRECTORY contains keeper.png, knight.png and effects.png.
"""
from pathlib import Path
from collections import Counter
import importlib.util,json,sys
from PIL import Image,ImageDraw
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('boss',Path(__file__).with_name('boss-art.py'));boss=importlib.util.module_from_spec(spec);spec.loader.exec_module(boss)
INK=boss.palette();rev={v:k for k,v in INK.items()}
RAMPS=[''.join(rev[v] for v in ramp) for ramp in [
[(69,39,18),(120,70,32),(169,118,57),(216,169,94)],
[(34,48,58),(47,74,82),(110,138,142),(143,163,168)]]]
INKS={k:INK[k] for ramp in RAMPS for k in ramp}
OUT=ROOT/'crates/delve/assets/rafterwake';OUT.mkdir(exist_ok=True)
REC={}
def clean(im,ramps=RAMPS):
    inks={k:INK[k] for r in ramps for k in r};_,rows=boss.snap(im,inks)
    cells=[list(r) for r in rows]
    # Erase detached concept debris, then merge tiny tone islands into their material.
    solid={(x,y) for y,r in enumerate(cells) for x,k in enumerate(r) if k!='.'}
    parts=sorted(boss.components(solid),key=len,reverse=True)
    for part in parts[1:]:
        for x,y in part:cells[y][x]='.'
    while True:
        changed=False
        for k in inks:
            parts=boss.components({(x,y) for y,r in enumerate(cells) for x,c in enumerate(r) if c==k})
            for part in parts:
                if len(part)>3:continue
                ns=Counter(cells[y][x] for px,py in part for x,y in boss.neighbours(px,py) if 0<=x<im.width and 0<=y<im.height and (x,y) not in part and cells[y][x] not in ('.',k))
                if ns:
                    ramp=next(r for r in ramps if k in r);same={c:n for c,n in ns.items() if c in ramp};choices=same or ns
                    c=max(choices,key=lambda c:(choices[c],-abs(boss.lightness(inks[c])-boss.lightness(inks[k]))))
                    for x,y in part:cells[y][x]=c
                    changed=True
        if not changed:break
    rows=boss.contour([''.join(r) for r in cells],ramps)
    while True:
        new=boss.despeckle(rows)
        if new==rows:break
        rows=new
    boss.gate(rows,inks)
    return boss.render(rows,inks)
def tile(source,col,row,cols,rows,size,fill):
    w,h=source.size;part=source.crop((col*w//cols,row*h//rows,(col+1)*w//cols,(row+1)*h//rows))
    part=boss.extract(part,'neutral',80)
    parts=sorted(boss.components({(x,y) for y in range(part.height) for x in range(part.width) if part.getpixel((x,y))[3]}),key=len,reverse=True)
    for debris in parts[1:]:
        for x,y in debris:part.putpixel((x,y),(0,0,0,0))
    box=part.getbbox();part=part.crop(box)
    scale=min((size[0]-4)/part.width,fill/part.height)
    part=part.resize((round(part.width*scale),round(part.height*scale)),Image.Resampling.NEAREST)
    im=Image.new('RGBA',size);im.alpha_composite(part,((size[0]-part.width)//2,size[1]-part.height-2))
    return im

def save(name,im,ramps=RAMPS):
    _,rows=boss.snap(im,{k:INK[k] for r in ramps for k in r});boss.gate(rows,{k:INK[k] for r in ramps for k in r})
    REC[name]={'rows':rows,'ramps':ramps}
    im.save(OUT/(name+'.png'));return im

def lights(im,centres):
    d=ImageDraw.Draw(im)
    for x,y in centres:
        d.rectangle((x-3,y-2,x+3,y+3),fill=INK[RAMPS[1][0]])
    for x,y in centres:
        core={(x,y),(x+1,y),(x-1,y+1),(x,y+1),(x+1,y+1)}
        fall={p for q in core for p in boss.neighbours(*q)}-core
        for points,c in [(fall,RAMPS[1][1]),(core,RAMPS[1][3])]:
            for xx,yy in points:im.putpixel((xx,yy),INK[c]+(255,))
    # Only a collar's isolated corner may merge; the shaped core is locked.
    _,rows=boss.snap(im,INKS)
    keep=[(xx,yy) for x,y in centres for xx,yy in [(x,y),(x+1,y),(x-1,y+1),(x,y+1),(x+1,y+1)]]
    rows=boss.despeckle(rows,keep)
    return boss.render(rows,INKS)

def sheet(names,out,scale=1,cols=4):
    ims=[Image.open(OUT/(n+'.png')).convert('RGBA') for n in names];w=max(max(i.width for i in ims)+12,max(len(n)*6+6 for n in names));h=max(i.height for i in ims)+22
    canvas=Image.new('RGB',(w*cols*scale,h*((len(ims)+cols-1)//cols)*scale),(9,13,16));d=ImageDraw.Draw(canvas)
    for n,(name,im) in enumerate(zip(names,ims)):
        x=(n%cols*w+(w-im.width)//2)*scale;y=(n//cols*h+4)*scale
        canvas.paste(im.resize((im.width*scale,im.height*scale),Image.Resampling.NEAREST),(x,y),im.resize((im.width*scale,im.height*scale),Image.Resampling.NEAREST));d.text((n%cols*w*scale+3,(n//cols*h+h-16)*scale),name,fill=(143,163,168))
    canvas.save(out)

def main(src,review):
    review.mkdir(parents=True,exist_ok=True)
    poses=['idle','walk-1','walk-2','drop-tell','drop','sweep-tell','sweep','flinch','damage-1','damage-2','defeat','recover']
    source=Image.open(src/'keeper.png').convert('RGBA');anchors=[]
    for n,p in enumerate(poses):
        raw=tile(source,n%4,n//4,4,3,(64,80),74)
        greens=[(x,y) for y in range(raw.height) for x in range(raw.width) if (lambda c:c[3]>0 and c[1]>105 and c[0]<c[1]*.82 and c[2]>c[0]*1.1)(raw.getpixel((x,y)))]
        parts=sorted(boss.components(set(greens)),key=len,reverse=True)[:2]
        eyes=[(round(sum(x for x,y in p)/len(p)),round(sum(y for x,y in p)/len(p))) for p in parts]
        eyes=[[(31,28),(39,30)],[(34,35),(42,37)],[(36,34),(44,36)],[(30,44),(37,47)],[(38,37),(47,41)],[(46,48),(51,52)],[(23,50),(29,52)],[(35,32),(44,36)],[(31,28),(40,30)],[(33,33)],[(28,64)],[(37,50)]][n]
        im=clean(raw);im=lights(im,eyes);save('keeper-'+p,im);anchors.append(eyes)
        signatures=[]
        for j,(x,y) in enumerate(eyes):
            core={(x,y),(x+1,y),(x-1,y+1),(x,y+1),(x+1,y+1)}
            fall={p for q in core for p in boss.neighbours(*q)}-core
            signatures.append({'name':f'lens {j+1}','pixels':[[xx,yy,RAMPS[1][3]] for xx,yy in sorted(core)],'falloff':[[xx,yy,RAMPS[1][1]] for xx,yy in sorted(fall)],'hues':{k:'#%02x%02x%02x'%INK[k] for k in RAMPS[1]}})
        REC['keeper-'+p]['signature_lights']=signatures
        assert not boss.signature_gate(REC['keeper-'+p],INKS),p
    source=Image.open(src/'knight.png').convert('RGBA')
    ks=['idle','run-1','run-2','jump','fall','land','climb-1','climb-2','hit','victory']
    for n,p in enumerate(ks):
        raw=tile(source,n%5,n//5,5,2,(48,48),43)
        if p.startswith('climb'):
            # Ladder belongs to the scenery, not the body silhouette.
            px=raw.load()
            for x in range(48):
                for y in range(48):
                    c=px[x,y]
                    if c[3] and c[0]>c[2]*1.15 and (x>34 or x<11 or y<8):px[x,y]=(0,0,0,0)
        if p.startswith('climb'):
            raw=Image.open(OUT/'knight-idle.png').convert('RGBA')
            d=ImageDraw.Draw(raw);x=11 if p.endswith('1') else 34
            d.rounded_rectangle((x-3,10,x+4,17),3,fill=INK[RAMPS[1][0]])
            d.rounded_rectangle((x-2,16,x+3,27),2,fill=INK[RAMPS[1][1]])
            d.rectangle((x-1,11,x+2,14),fill=INK[RAMPS[1][2]])
            d.rectangle((x-1,18,x+1,24),fill=INK[RAMPS[1][2]])
        save('knight-'+p,clean(raw))
    source=Image.open(src/'effects.png').convert('RGBA')
    for row,p in enumerate(['drop','sweep','dust']):
        ramp=RAMPS[1] if p=='sweep' else RAMPS[0]
        for n in range(4):save(f'fx-{p}-{n}',clean(tile(source,n,row,4,3,(48,32),28),[ramp]),[ramp])
    # Scenery is authored in the very same four-tone ramps as the bodies.
    a,b,c,e=[INK[k] for k in RAMPS[0]]
    for name,size in [('plank',(24,10)),('ladder',(20,36)),('brick',(24,18)),('post',(14,36))]:
        im=Image.new('RGBA',size);d=ImageDraw.Draw(im)
        if name=='plank':
            d.rounded_rectangle((0,0,23,9),2,fill=a);d.rectangle((2,1,21,6),fill=b);d.rectangle((3,1,20,3),fill=c);d.line((4,1,18,1),fill=e,width=2)
        elif name=='ladder':
            for x in [2,14]:
                d.rounded_rectangle((x,0,x+3,35),1,fill=a);d.rectangle((x+1,1,x+2,34),fill=c)
            for y in [6,18,30]:d.rectangle((5,y,14,y+3),fill=b);d.rectangle((6,y,13,y+1),fill=e)
        elif name=='post':
            d.rounded_rectangle((2,0,11,35),2,fill=a);d.rectangle((4,1,9,34),fill=b);d.rectangle((4,2,5,33),fill=c);d.rectangle((4,2,5,8),fill=e)
        else:
            d.rounded_rectangle((1,1,22,16),3,fill=a);d.rectangle((3,3,20,12),fill=b);d.rectangle((4,3,19,6),fill=c);d.rectangle((5,3,15,4),fill=e)
        save(name,clean(im,[RAMPS[0]]),[RAMPS[0]])
    (OUT/'native.json').write_text(json.dumps({'inks':INKS,'assets':REC,'eyes':anchors},separators=(',',':'))+'\n')
    for names,prefix,cols in [(list('keeper-'+p for p in poses),'keeper',4),(list('knight-'+p for p in ks),'knights',5),([f'fx-{p}-{n}' for p in ['drop','sweep','dust'] for n in range(4)],'effects',4),(['plank','ladder','brick','post'],'props',4)]:
        for scale in [1,3]:sheet(names,review/f'{prefix}-{scale}x.png',scale,cols)
    # Minimum edge lightness against actual dark floor, plus occupied height.
    checks=[]
    for name,s in REC.items():
        rows=s['rows'];points={(x,y) for y,r in enumerate(rows) for x,k in enumerate(r) if k!='.'}
        edge=[INKS[rows[y][x]] for x,y in points if any(n not in points for n in boss.neighbours(x,y))]
        delta=min(boss.lightness(c)-boss.lightness((9,13,16)) for c in edge)
        assert delta>=15,(name,delta)
        checks.append(f'{name}: zero singleton inks, {len(set("".join(rows))-set("."))} inks; weakest floor edge {delta:.1f} L*')
    idle=REC['keeper-idle']['rows'];head=INKS[idle[18][32]];body=INKS[idle[48][32]]
    head_body=abs(boss.lightness(head)-boss.lightness(body))
    assert head_body>=15,head_body
    occupied=Image.open(OUT/'keeper-idle.png').getbbox();knight=Image.open(OUT/'knight-idle.png').getbbox()
    boss_h=occupied[3]-occupied[1];knight_h=knight[3]-knight[1]
    assert boss_h>=64 and 1.5<=boss_h/knight_h<=1.8
    checks.append(f'Idle head/body separation {head_body:.1f} L*; boss {boss_h}px / knight {knight_h}px = {boss_h/knight_h:.2f}; cell fill {boss_h/80:.1%}')
    (review/'art-gates.txt').write_text('\n'.join(checks)+'\n')
if __name__=='__main__':main(Path(sys.argv[1]),Path(sys.argv[2]))
