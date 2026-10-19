#!/usr/bin/env python3
"""Convert Clasp's state set at one shared scale, retaining coloured pixel clusters."""
from pathlib import Path
from PIL import Image, ImageDraw
import argparse, importlib.util
from collections import Counter
spec=importlib.util.spec_from_file_location('cinder_art',Path(__file__).with_name('cinder-art.py'))
base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
PALETTE=[0x132724,0x192934,0x263b40,0x2f4a52,0x3a6d5d,0x6e8a8e,0x8fa3a8,0xb0b2a8,
         0x45291c,0x652c10,0x784620,0x924f1d,0xa97639,0xbe914b,0xd8a95e,0xe2cb8b]
base.COLOURS=[((p>>16)&255,(p>>8)&255,p&255) for p in PALETTE]
STATES=['idle','high-tell','high-attack','low-tell','low-attack','flinch','damage-1','damage-2','defeat']
# Each pair shares a visor row. These lights are restored after every cleanup.
EYES=[(31,39),(37,34),(34,40),(25,52),(28,53),(36,35),(31,39),(30,40),(36,60)]
def polish(tile,pose):
    d=ImageDraw.Draw(tile)
    if pose==3:
        # Lower the whole free forearm, retaining its rounded fist and knuckles.
        arm=tile.crop((43,37,79,62))
        d.rectangle((43,37,79,61),fill=(0,0,0,0))
        tile.alpha_composite(arm,(43,49))
        d=ImageDraw.Draw(tile)
        d.line([(43,55),(48,58),(51,60)],fill='#263b40',width=5)
        d.polygon([(41,53),(45,51),(49,54),(54,58),(53,62),(47,60),(42,59)],fill='#192934')
        d.polygon([(42,54),(45,53),(49,56),(52,59),(50,60),(45,58)],fill='#2f4a52')
        d.line([(44,54),(48,57)],fill='#a97639',width=2)
        # The rake's tip now rests on the same floor as the feet.
        d.line([(7,72),(7,77),(12,78),(15,76)],fill='#192934',width=4)
        d.line([(7,72),(8,76),(12,77),(14,76)],fill='#a97639',width=2)
    if pose==7:
        # One broad buckled crown, rather than alternating diagonal bands.
        d.polygon([(25,35),(26,30),(33,24),(38,25),(45,30),(48,36),
                   (44,38),(38,35),(33,36),(28,38)],fill='#784620')
        d.polygon([(26,33),(28,29),(34,25),(38,26),(42,29),(36,29),
                   (32,32),(30,35)],fill='#be914b')
        d.polygon([(29,30),(34,26),(38,27),(34,29),(32,31)],fill='#d8a95e')
        d.polygon([(37,32),(43,30),(46,34),(43,36),(38,35)],fill='#a97639')
    ex,ey=EYES[pose]
    d.rectangle((ex-2,ey-1,ex+7,ey+2),fill='#192934')
    for x in [ex,ex+4]:
        d.rectangle((x,ey,x+1,ey+1),fill='#e2cb8b')
    return tile
def strip(im):
    im=im.convert('RGBA');p=im.load()
    for y in range(im.height):
        for x in range(im.width):
            r,g,b,a=p[x,y]
            if min(r,g,b)>65 and max(r,g,b)<190 and max(r,g,b)-min(r,g,b)<28:p[x,y]=(0,0,0,0)
    return im

def clean(im, connected=False):
    im=base.clean(im);old=im.copy();p=im.load();q=old.load()
    for y in range(im.height):
        for x in range(im.width):
            c=q[x,y]
            if not c[3]:continue
            ns=[q[x+dx,y+dy] for dx,dy in [(-1,0),(1,0),(0,-1),(0,1)]
                if 0<=x+dx<im.width and 0<=y+dy<im.height]
            # Remove isolated interior ink pixels before adding signature lights.
            solid=[n for n in ns if n[3]]
            if len(solid)==4 and c not in solid:p[x,y]=Counter(solid).most_common(1)[0][0]
            if any(not n[3] for n in ns):p[x,y]=(25,41,52,255)
    # Source neighbours may intrude across panel boundaries. Keep the body and
    # its attached weapon, dropping detached conversion fragments.
    seen=set();components=[]
    for yy in range(im.height):
        for xx in range(im.width):
            if (xx,yy) in seen or not p[xx,yy][3]:continue
            todo=[(xx,yy)];seen.add((xx,yy));component=[]
            while todo:
                x,y=todo.pop();component.append((x,y))
                for dx,dy in [(-1,0),(1,0),(0,-1),(0,1)]:
                    n=(x+dx,y+dy)
                    if 0<=n[0]<im.width and 0<=n[1]<im.height and n not in seen and p[n[0],n[1]][3]:seen.add(n);todo.append(n)
            components.append(component)
    if components:
        keep=max(components,key=len)
        for component in components:
            if component is not keep and (connected or len(component)<12):
                for x,y in component:p[x,y]=(0,0,0,0)
    return im

def review(images,dest,scale,cols=3):
    tw=80*scale+20;th=80*scale+30
    out=Image.new('RGB',(cols*tw,((len(images)+cols-1)//cols)*th));d=ImageDraw.Draw(out)
    for i,(name,im) in enumerate(images):
        x=i%cols*tw+10;y=i//cols*th;d.text((x,y+5),name,fill='#b0b2a8')
        big=im.resize((im.width*scale,im.height*scale),Image.Resampling.NEAREST);out.paste(big,(x,y+24),big)
    out.save(dest)

def run(src,out,reviewdir):
    out.mkdir(parents=True,exist_ok=True);reviewdir.mkdir(parents=True,exist_ok=True)
    source=Image.open(src/'clasp.png');images=[]
    # Shared source scale keeps the idle body the same size as its raised hook.
    for i,name in enumerate(STATES):
        x=i%3;y=i//3
        im=strip(source.crop((x*source.width//3,y*source.height//3,(x+1)*source.width//3,(y+1)*source.height//3)))
        im=im.crop(im.getbbox());im=im.resize((round(im.width*.205),round(im.height*.205)),Image.Resampling.NEAREST)
        tile=Image.new('RGBA',(80,80));tile.alpha_composite(im,((80-im.width)//2,79-im.height))
        # Use the shared converter's nearest palette, without re-scaling each pose.
        p=tile.load()
        for yy in range(80):
            for xx in range(80):
                if p[xx,yy][3]:p[xx,yy]=(*min(base.COLOURS,key=lambda c:sum((c[k]-p[xx,yy][k])**2 for k in range(3))),255)
        tile=clean(tile, connected=True)
        tile=polish(tile,i)
        tile.save(out/(name+'.png'));images.append((name,tile))
    fx=Image.open(src/'effects.png');effects=[]
    for row,name in enumerate(['parry','clash','strike']):
        for col in range(4):
            tile=fx.crop((col*fx.width//4,row*fx.height//3,(col+1)*fx.width//4,(row+1)*fx.height//3))
            tile=strip(tile);tile=tile.crop(tile.getbbox());tile.thumbnail((32,32),Image.Resampling.NEAREST)
            canvas=Image.new('RGBA',(32,32));canvas.alpha_composite(tile,((32-tile.width)//2,(32-tile.height)//2));tile=canvas
            p=tile.load()
            for yy in range(32):
                for xx in range(32):
                    if p[xx,yy][3]:p[xx,yy]=(*min(base.COLOURS,key=lambda c:sum((c[k]-p[xx,yy][k])**2 for k in range(3))),255)
            tile=clean(tile)
            # Keep impact petals connected: this is a single contact, not particles.
            if row==1 and col==2:
                d=ImageDraw.Draw(tile);d.polygon([(10,14),(16,10),(21,15),(17,21)],fill='#a97639')
            factor=[[.65,.9,1,.65],[.55,1,.9,.65],[.65,.9,1,.65]][row][col]
            if factor<1:
                small=tile.resize((round(32*factor),round(32*factor)),Image.Resampling.NEAREST)
                tile=Image.new('RGBA',(32,32));tile.alpha_composite(small,((32-small.width)//2,(32-small.height)//2))
            n=f'{name}-{col+1}';tile.save(out/(n+'.png'));effects.append((n,tile))
    review(images,reviewdir/'states-3x.png',3)
    review(images,reviewdir/'states-1x.png',1)
    review(effects,reviewdir/'effects-3x.png',3,4)
def knights(out, reviewdir):
    """Five authored bodies in native room pixels; the palette is recoloured per seat."""
    import re
    inkfile=Path(__file__).resolve().parents[1]/'crates/delve/src/render/overworld/ink.rs'
    inks={k:'#'+v for k,v in re.findall(r"'(.)' => 0x([0-9a-f]{6})",inkfile.read_text())}
    images=[]
    for pose,name in enumerate(['guard-high','guard-low','thrust-brace','thrust-contact','thrust-recover']):
        im=Image.new('RGBA',(128,80));d=ImageDraw.Draw(im)
        lean=[0,0,2,3,2][pose];crouch=2 if pose==1 else 0
        def poly(points,ink,shift=0):
            d.polygon([(x+shift,y) for x,y in points],fill=inks[ink])
        def rect(box,ink):d.rectangle(box,fill=inks[ink])
        # Rear leg bends under a turned hip; the two feet retain their floor pivot.
        poly([(25,52),(34,54),(31,65),(27,74),(27,78),(17,78),(18,74),(22,64)],'s')
        poly([(26,55),(31,56),(28,64),(24,73),(20,73),(24,63)],'S')
        poly([(26,57),(29,57),(26,65),(23,69),(22,66)],'U')
        poly([(19,73),(25,73),(27,77),(30,78),(30,79),(14,79),(14,77)],'S')
        rect((16,76,25,77),'U')
        # Front thigh, knee and boot are articulated rather than rectangular heels.
        poly([(34,51),(43,52),(46,62),(44,72),(47,76),(52,77),(53,79),(36,79),(35,75),(37,64),(32,58)],'s')
        poly([(36,54),(41,54),(43,62),(41,67),(37,65),(38,60)],'U')
        poly([(38,55),(41,56),(42,61),(39,61)],'V')
        poly([(38,67),(42,67),(41,74),(44,76),(37,76)],'S')
        rect((38,76,48,77),'U')
        # Hip, cloth and breastplate follow the forward lean, while feet stay planted.
        poly([(22,43),(41,43),(43,56),(36,61),(28,59),(21,56)],'s',lean)
        poly([(26,46),(38,46),(38,56),(33,60),(27,56)],'8',lean)
        poly([(28,47),(36,46),(35,56),(32,58),(28,55)],'7',lean)
        poly([(25,45),(40,44),(40,49),(25,50)],'S',lean)
        poly([(29,46),(35,46),(35,48),(29,48)],'t',lean)
        poly([(24,31+crouch),(32,28+crouch),(40,31+crouch),(44,41),(41,47),(24,47),(21,40)],'s',lean)
        poly([(25,32+crouch),(31,31+crouch),(39,33+crouch),(41,40),(37,44),(25,44),(23,39)],'S',lean)
        poly([(26,33+crouch),(31,32+crouch),(37,34+crouch),(37,39),(33,43),(26,42),(24,38)],'U',lean)
        poly([(26,33+crouch),(30,32+crouch),(33,34+crouch),(31,38),(26,38)],'V',lean)
        # Rounded side-facing helmet: a broad brow, dark opening and folded neck.
        hy=crouch
        poly([(25,17+hy),(28,13+hy),(36,13+hy),(41,17+hy),(43,24+hy),(40,30+hy),(33,33+hy),(25,29+hy),(23,23+hy)],'s',lean)
        poly([(26,18+hy),(29,15+hy),(35,15+hy),(39,18+hy),(40,23+hy),(34,25+hy),(27,24+hy),(25,22+hy)],'U',lean)
        poly([(27,18+hy),(30,16+hy),(35,16+hy),(37,18+hy),(35,20+hy),(27,21+hy)],'V',lean)
        poly([(26,23+hy),(33,24+hy),(39,23+hy),(42,24+hy),(39,27+hy),(32,28+hy),(26,27+hy)],'s',lean)
        poly([(27,27+hy),(32,29+hy),(40,27+hy),(38,30+hy),(33,31+hy),(28,29+hy)],'U',lean)
        poly([(28,27+hy),(32,28+hy),(35,28+hy),(32,30+hy),(29,29+hy)],'V',lean)
        # A connected swept plume keeps the identity colours in each pose.
        poly([(30,15+hy),(29,10+hy),(24,8+hy),(17,10+hy),(16,14+hy),(21,13+hy),(26,14+hy)],'8',lean)
        poly([(29,13+hy),(27,10+hy),(22,9+hy),(18,11+hy),(18,12+hy),(23,11+hy),(27,13+hy)],'7',lean)
        # Sword arm changes at shoulder, elbow and wrist in every thrust drawing.
        end=[44,44,63,76,66][pose];ey=[49,50,53,53,54][pose]
        if pose>=2:
            poly([(38+lean,33+crouch),(43+lean,34+crouch),(50,43),(end,ey-4),(end+2,ey),(end,ey+3),(48,49),(40+lean,43)],'s')
            poly([(40+lean,35),(43+lean,37),(50,45),(end,ey-2),(end,ey+1),(49,47),(41+lean,41)],'U')
            poly([(43+lean,38),(46+lean,41),(48,44),(46,45),(42+lean,41)],'V')
            rect((end-3,ey-3,end+1,ey+1),'S')
            rect((end-1,ey-3,end+1,ey-1),'V')
        else:
            poly([(39,34+crouch),(44,35+crouch),(48,43+crouch),(47,50+crouch),(42,51+crouch),(39,43+crouch)],'s')
            poly([(41,36+crouch),(44,37+crouch),(46,44+crouch),(45,48+crouch),(42,47+crouch)],'U')
        # Shield arm follows the shield; thrust drops it 6px away from the face.
        sy=49 if pose==1 else (23 if pose>=2 else 17);sx=46+lean
        poly([(25+lean,35+crouch),(31+lean,36+crouch),(37+lean,sy+11),(sx+3,sy+9),(sx+4,sy+15),(36+lean,sy+17),(26+lean,42+crouch)],'s')
        poly([(27+lean,36+crouch),(30+lean,38+crouch),(37+lean,sy+13),(sx+2,sy+11),(sx+2,sy+13),(36+lean,sy+15),(28+lean,41+crouch)],'U')
        poly([(sx+2,sy),(sx+10,sy),(sx+14,sy+3),(sx+14,sy+13),(sx+11,sy+19),(sx+7,sy+22),(sx+2,sy+18),(sx,sy+12),(sx,sy+3)],'I')
        poly([(sx+2,sy+1),(sx+10,sy+1),(sx+13,sy+4),(sx+13,sy+12),(sx+10,sy+18),(sx+7,sy+20),(sx+3,sy+17),(sx+1,sy+11),(sx+1,sy+4)],'o')
        poly([(sx+3,sy+2),(sx+10,sy+2),(sx+12,sy+4),(sx+11,sy+5),(sx+3,sy+4),(sx+2,sy+10),(sx+2,sy+4)],'t')
        poly([(sx+4,sy+5),(sx+10,sy+5),(sx+11,sy+7),(sx+10,sy+14),(sx+7,sy+18),(sx+4,sy+15),(sx+3,sy+10)],'8')
        poly([(sx+5,sy+6),(sx+9,sy+6),(sx+10,sy+8),(sx+9,sy+14),(sx+7,sy+16),(sx+5,sy+14)],'7')
        # The same warm emblem in every seat, clear of the coloured field.
        rect((sx+7,sy+7,sx+8,sy+13),'t');rect((sx+5,sy+9,sx+10,sy+10),'t')
        if pose>=2:
            tip=[93,106,98][pose-2]
            rect((end+2,ey-5,end+3,ey+5),'o');rect((end+2,ey-4,end+2,ey+3),'T')
            poly([(end+4,ey-2),(tip-5,ey-2),(tip,ey),(tip-5,ey+2),(end+4,ey+2)],'o')
            poly([(end+4,ey-2),(tip-6,ey-2),(tip-1,ey),(end+4,ey)],'t')
            rect((end+5,ey-2,tip-8,ey-1),'T')
        colours=set(im.get_flattened_data())-{(0,0,0,0)};assert len(colours)<=16
        solid={(x,y) for y in range(im.height) for x in range(im.width) if im.getpixel((x,y))[3]}
        seen=set();todo=[next(iter(solid))]
        while todo:
            x,y=todo.pop()
            if (x,y) in seen:continue
            seen.add((x,y))
            todo.extend(n for n in [(x-1,y),(x+1,y),(x,y-1),(x,y+1)] if n in solid and n not in seen)
        assert seen==solid, f'{name}: detached pixel cluster'
        im.save(out/('knight-'+name+'.png'));images.append((name,im))
    reviewdir.mkdir(parents=True,exist_ok=True)
    # The native body crop, with weapons kept inside a shared frame.
    # Keep review sheets below the reviewer's width limit.
    for scale in [1,3]:
        tw=128*scale+16;th=80*scale+28;cols=3
        sheet=Image.new('RGB',(tw*cols,th*2));sd=ImageDraw.Draw(sheet)
        for i,(name,im) in enumerate(images):
            x=i%cols*tw+8;y=i//cols*th
            sd.text((x,y+5),name,fill='#b0b2a8');big=im.resize((128*scale,80*scale),Image.Resampling.NEAREST)
            sheet.paste(big,(x,y+24),big)
        sheet.save(reviewdir/f'knights-{scale}x.png')

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('source',type=Path,nargs='?');p.add_argument('--out',type=Path,default=Path('crates/delve/assets/latch'));p.add_argument('--review',type=Path,required=True);p.add_argument('--knights',action='store_true');a=p.parse_args()
    if a.knights:knights(a.out,a.review)
    else:run(a.source,a.out,a.review)
