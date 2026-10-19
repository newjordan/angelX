#!/usr/bin/env python3
"""Nearest-sample Mossbell's Keeper and growing tells; lock clusters and lights."""
from pathlib import Path
from PIL import Image, ImageDraw
import argparse, importlib.util, json

STATES = ['idle', 'root-tell', 'root-attack', 'crown-tell', 'crown-attack',
          'flinch', 'damage-1', 'damage-2', 'defeat']
INKS = dict(zip('abcdefghij', [bytes.fromhex(c) for c in
    ['263b40', '2f4a52', '3a6d5d', '6e8a8e', '784620',
     'a97639', 'be914b', 'd8a95e', '192934', 'e2cb8b']]))
INKS = {k: tuple(v) for k,v in INKS.items()}

def sheet(images, dest, scale, cols=3):
    w,h=images[0][1].size; tw=max(120,w*scale+24); th=h*scale+28
    out=Image.new('RGB',(tw*cols,th*((len(images)+cols-1)//cols)))
    d=ImageDraw.Draw(out)
    for i,(name,im) in enumerate(images):
        x=i%cols*tw+12; y=i//cols*th
        d.text((x,y+5),name,fill='#b0b2a8')
        big=im.resize((w*scale,h*scale),Image.Resampling.NEAREST)
        out.paste(big,(x,y+24),big)
    out.save(dest)

def sample(source, index, gates):
    col,row=index%3,index//3
    im=source.crop((col*source.width//3,row*source.height//3,
                   (col+1)*source.width//3,(row+1)*source.height//3)).convert('RGBA')
    p=im.load()
    for y in range(im.height):
        for x in range(im.width):
            c=p[x,y]
            if min(c[:3])>60 and max(c[:3])-min(c[:3])<24: p[x,y]=(0,0,0,0)
    # Drop the flinch's detached sparks and any background-key fragments.
    points={(x,y) for y in range(im.height) for x in range(im.width) if p[x,y][3]}
    parts=list(gates.components(points)); keep=max(parts,key=len)
    for part in parts:
        if part is not keep:
            for q in part: p[q]=(0,0,0,0)
    im=im.crop(im.getbbox())
    h=min(44,round(im.height*42/im.width))
    im=im.resize((42,h),Image.Resampling.NEAREST)
    tile=Image.new('RGBA',(48,48)); tile.alpha_composite(im,(3,47-h))
    def ramp(c):
        r,g,b=c[:3]
        return 'abcd' if g>r*1.05 and g>b*1.05 else 'aefghi'
    def nearest(c):
        if ramp(c)=='abcd':
            return 'a' if max(c[:3])<65 else 'b' if max(c[:3])<105 else 'c' if max(c[:3])<160 else 'd'
        return min(ramp(c),key=lambda k:sum((INKS[k][n]-c[n])**2 for n in range(3)))
    rows=[''.join('.' if not tile.getpixel((x,y))[3] else nearest(tile.getpixel((x,y)))
          for x in range(48)) for y in range(48)]
    rows=gates.despeckle(rows)
    # A selective teal contour separates every silhouette pixel from the field.
    cells=[list(r) for r in rows]
    for y,row in enumerate(rows):
        for x,k in enumerate(row):
            if k!='.' and any(nx<0 or ny<0 or nx>=48 or ny>=48 or rows[ny][nx]=='.'
                             for nx,ny in gates.neighbours(x,y)):
                cells[y][x]='c'
    return gates.render(gates.despeckle([''.join(r) for r in cells]),INKS)

FACES = [(24,22),(24,23),(25,29),(24,21),(25,25),(31,20),(24,19),(29,22),(32,36)]
SEEDS = [(24,37),(25,36),(25,40),(24,36),(25,40),(27,34),(24,35),(25,35),(19,40)]

def polish(tile, i, gates):
    alpha=tile.getchannel('A').copy()
    d=ImageDraw.Draw(tile)
    fx,fy=FACES[i]
    # Remove the sampled face highlights before placing two curved visor lobes.
    d.polygon([(fx-6,fy-4),(fx-2,fy-7),(fx+4,fy-5),(fx+7,fy-1),
               (fx+6,fy+5),(fx-6,fy+5)],fill=INKS['i'])
    lights=[]
    if i!=8:
        pixels=[]
        for ox,oy in [(-4,-1),(2,0)]:
            for dx,dy in [(0,0),(1,0),(1,1),(2,1)]:
                pixels.append((fx+ox+dx,fy+oy+dy,'h'))
        points={(x,y) for x,y,_ in pixels}
        falloff=[(x,y,'e') for x,y in sorted({n for q in points for n in gates.neighbours(*q)}-points)]
        lights.append(dict(name='curved visor',pixels=pixels,falloff=falloff,hues={k:'#'+bytes(INKS[k]).hex() for k in 'he'}))
    sx,sy=SEEDS[i]
    d.polygon([(sx,sy-4),(sx+3,sy-1),(sx+2,sy+3),(sx,sy+5),
               (sx-3,sy+2),(sx-3,sy-1)],fill=INKS['e'])
    pixels=[(sx+dx,sy+dy,'h') for dy,dxs in [(-2,[0]),(-1,[-1,0]),(0,[-1,0,1]),(1,[0,1]),(2,[0])] for dx in dxs]
    points={(x,y) for x,y,_ in pixels}
    falloff=[(x,y,'f') for x,y in sorted({n for q in points for n in gates.neighbours(*q)}-points)]
    lights.append(dict(name='almond seed',pixels=pixels,falloff=falloff,hues={k:'#'+bytes(INKS[k]).hex() for k in 'hf'}))
    if i in (6,7):
        # Two-pixel warm interiors inside five-pixel cool fracture channels.
        cracks=[[(30,27),(28,32),(30,36),(28,43)]]
        cracks+=[[(17,29),(20,33),(17,37)] if i==6 else
                 [(18,27),(20,32),(17,36),(19,42)]]
        for points in cracks:
            d.line(points,fill=INKS['i'],width=6)
            d.line(points,fill=INKS['h'],width=2)
    # Keep all authored marks inside the sampled body; then clean only once.
    tile.putalpha(alpha)
    rows=[''.join('.' if not tile.getpixel((x,y))[3] else min(INKS,key=lambda k:sum((INKS[k][n]-tile.getpixel((x,y))[n])**2 for n in range(3))) for x in range(48)) for y in range(48)]
    rows=gates.despeckle(rows)
    # The hood reads through broad teal planes, without pale edge flecks.
    rows=[row.replace('d','b' if y<27 else 'c') for y,row in enumerate(rows)]
    spec=dict(rows=rows,signature_lights=lights)
    spec['rows']=gates.despeckle(gates.signature_rows(spec),keep=gates.hand_pixels(spec))
    rows=gates.signature_rows(spec)
    cells=[list(r) for r in rows]
    solid={(x,y) for y,row in enumerate(rows) for x,k in enumerate(row) if k!='.'}
    edge={q for q in solid if any(n not in solid for n in gates.neighbours(*q))}
    for x,y in edge: cells[y][x]='c'
    # Connect short contour tips inward instead of exempting singleton pixels.
    for x,y in sorted(edge):
        if not any(n in solid and cells[n[1]][n[0]]=='c' for n in gates.neighbours(x,y)):
            nx,ny=next(n for n in gates.neighbours(x,y) if n in solid)
            cells[ny][nx]='c'
    spec['rows']=gates.despeckle([''.join(r) for r in cells],keep=gates.hand_pixels(spec)+[list(q) for q in edge])
    rows=gates.signature_rows(spec)
    # Protected pixels survive last; no smoothing is applied to signature lights.
    tile=gates.render(rows,INKS)
    return tile,spec

def run(a):
    spec=importlib.util.spec_from_file_location('boss_art',a.gates)
    gates=importlib.util.module_from_spec(spec); spec.loader.exec_module(gates)
    a.out.mkdir(parents=True,exist_ok=True); a.review.mkdir(parents=True,exist_ok=True)
    src=Image.open(a.source); images=[]; specs={}
    for i,name in enumerate(STATES):
        tile,spec=polish(sample(src,i,gates),i,gates)
        specs[name]=spec
        box=tile.getbbox(); assert 40<=box[2]-box[0]<=44,name
        edge,_=gates.separation(tile,(11,10,8),(0,0,48,48),(0,0,48,48))
        assert edge>=15,(name,edge)
        print(f'{name}: width {box[2]-box[0]}px, outer/floor {edge:.2f} L*, shaped lights and singleton gate')
        errors=gates.signature_gate(spec,INKS)
        assert not errors,(name,errors)
        gates.gate(gates.signature_rows(spec),INKS,gates.hand_pixels(spec))
        images.append((name,tile))
    # Six solid, detached seed petals finish the retained jackpot phrase.
    petal=Image.new('RGBA',(32,32)); d=ImageDraw.Draw(petal)
    for shape,k in [([(14,1),(18,3),(17,7),(14,6),(13,3)],'g'),
                    ([(24,5),(28,7),(27,11),(24,10),(22,8)],'h'),
                    ([(25,22),(29,22),(30,26),(27,28),(25,25)],'g'),
                    ([(14,25),(18,26),(19,30),(15,31),(13,28)],'f'),
                    ([(3,22),(7,22),(9,25),(6,28),(2,26)],'g'),
                    ([(2,5),(6,4),(9,7),(7,10),(3,9)],'f')]:
        d.polygon(shape,fill=INKS[k])
    evidence={'floor':'#0b0a08','head':[18,10,25,16],'body':[20,29,28,34]}
    edge,materials=gates.separation(images[0][1],(11,10,8),evidence['head'],evidence['body'])
    print(f'idle: outer/floor {edge:.2f} L*, head/body {materials:.2f} L*')
    assert edge>=15 and materials>=15
    for name,tile in images+[('jackpot-3',petal)]+[(p.stem,Image.open(p).convert('RGBA')) for p in sorted(a.out.glob('*-?.png')) if p.stem.startswith(('impact-','jackpot-','bumper-'))]:
        near={(x,y) for y in range(tile.height) for x in range(tile.width) if tile.getpixel((x,y))[3] and min(tile.getpixel((x,y))[:3])>=240}
        assert not any(len(c)>=2 for c in gates.components(near)),name
    for name,tile in images: tile.save(a.out/(name+'.png'))
    petal.save(a.out/'jackpot-3.png')
    (a.out/'native.json').write_text(json.dumps(dict(inks={k:'#'+bytes(c).hex() for k,c in INKS.items()},separation=evidence,states=specs),indent=2)+'\n')
    sheet(images,a.review/'states-1x.png',1)
    sheet(images,a.review/'states-3x.png',3)

def tell_sample(path, size, gates, inks=None, parts=1):
    """Key neutral grey, nearest-sample the finished subject, snap four inks."""
    im=Image.open(path).convert('RGBA')
    pixels=im.load()
    for y in range(im.height):
        for x in range(im.width):
            c=pixels[x,y]
            if max(c[:3])-min(c[:3])<30:
                pixels[x,y]=(0,0,0,0)
    points={(x,y) for y in range(im.height) for x in range(im.width) if pixels[x,y][3]}
    keep=set().union(*sorted(gates.components(points),key=len,reverse=True)[:parts])
    for q in points-keep: pixels[q]=(0,0,0,0)
    im=im.crop(im.getbbox()).resize(size,Image.Resampling.NEAREST)
    inks=inks or {k:INKS[k] for k in 'acfh'}
    rows=[''.join('.' if not im.getpixel((x,y))[3] else
          min(inks,key=lambda k:sum((inks[k][n]-im.getpixel((x,y))[n])**2 for n in range(3)))
          for x in range(size[0])) for y in range(size[1])]
    return gates.render(gates.despeckle(rows),inks)

def tells(a):
    a.out.mkdir(parents=True,exist_ok=True)
    a.review.mkdir(parents=True,exist_ok=True)
    spec=importlib.util.spec_from_file_location('boss_art',a.gates)
    gates=importlib.util.module_from_spec(spec);spec.loader.exec_module(gates)
    root=tell_sample(a.root_source,(184,11),gates)
    root_inks={k:INKS[k] for k in 'ach'} | {'d':(44,81,70)}
    crown_inks=dict(zip('abcd',[(24,43,43),(38,76,67),(55,118,96),(102,173,143)]))
    crown=tell_sample(a.crown_source,(51,24),gates,crown_inks,parts=2)
    crown.save(a.review/'crown-sampled-1x.png')
    # Remove loose one-pixel tips; keep the source's travelling stem and bud.
    solid={(x,y) for y in range(11) for x in range(184) if root.getpixel((x,y))[3]}
    solid={q for q in solid if sum(n in solid for n in gates.neighbours(*q))>=2}
    solid=max(gates.components(solid),key=len)
    root=Image.new('RGBA',(184,11))
    for x,y in solid:
        edge=any(n not in solid for n in gates.neighbours(x,y))
        root.putpixel((x,y),(*root_inks['a' if edge else 'h' if x>=174 else
                                     'd' if (x,y-1) not in solid or (x,y-2) not in solid else 'c'],255))
    images=[]
    for stage in range(4):
        reach=min((stage+1)*46,179)
        tile=Image.new('RGBA',root.size)
        # Keep native root grain. The authored bud rides the advancing tip.
        tile.alpha_composite(root.crop((0,0,reach-10,11)))
        tile.alpha_composite(root.crop((174,0,184,11)),(reach-10,0))
        images.append((f'root-vines-{stage}',tile))
        # Each jaw advances from its outer corner, across the floor, toward
        # the centre. A dim gate tint marks the full landing area from stage 0.
        tile=Image.new('RGBA',crown.size,(*crown_inks['a'],255))
        mask=Image.new('1',crown.size);d=ImageDraw.Draw(mask)
        left=[[(0,0),(5,0),(5,3),(2,5),(0,5)],
              [(0,0),(6,0),(6,3),(5,4),(8,7),(12,8),(15,5),(18,6),
               (18,10),(15,15),(10,16),(5,14),(1,10),(0,5)],
              [(0,0),(6,0),(6,3),(5,4),(6,9),(11,13),(15,14),
               (19,10),(22,11),(22,17),(19,21),(12,21),(6,18),(2,12),(0,5)],
              [(0,0),(6,0),(6,3),(5,4),(6,9),(9,13),(14,16),
               (19,16),(22,12),(24,13),(25,17),(24,22),(21,23),
               (14,23),(8,21),(3,17),(1,11),(0,5)]][stage]
        d.polygon(left,fill=1)
        d.polygon([(50-x,y) for x,y in left],fill=1)
        # Hand-clean the reference's U-shaped hooks at native grain. Only
        # broad planes remain: no sampled texture or detached thorn pixels.
        points={(x,y) for y in range(24) for x in range(51) if mask.getpixel((x,y))}
        for x,y in points:
            edge=any(n not in points for n in gates.neighbours(x,y))
            ink='d' if edge else 'b' if y<3 else 'c'
            tile.putpixel((x,y),(*crown_inks[ink],255))
        images.append((f'crown-vines-{stage}',tile))
    recipe={}
    for name,tile in images:
        inks=root_inks if name.startswith('root') else crown_inks
        rows=[''.join('.' if not tile.getpixel((x,y))[3] else
              min(inks,key=lambda k:sum((inks[k][n]-tile.getpixel((x,y))[n])**2 for n in range(3)))
              for x in range(tile.width)) for y in range(tile.height)]
        rows=gates.despeckle(rows)
        gates.gate(rows,inks)
        recipe[name]=dict(size=list(tile.size),inks={k:'#'+bytes(c).hex() for k,c in inks.items()},rows=rows)
        tile=gates.render(rows,inks)
        tile.save(a.out/(name+'.png'))
    native=json.loads((a.out/'native.json').read_text())
    native['tells']=recipe
    (a.out/'native.json').write_text(json.dumps(native,indent=2)+'\n')
    # Review sheets keep their different native aspect ratios.
    for kind in ['root','crown']:
        frames=[(n,Image.open(a.out/(n+'.png'))) for n,_ in images if n.startswith(kind)]
        sheet(frames,a.review/(kind+'-vines-1x.png'),1,2)
        sheet(frames,a.review/(kind+'-vines-3x.png'),3,2)

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('source',type=Path)
    p.add_argument('--out',type=Path,default=Path('crates/delve/assets/mossbell'))
    p.add_argument('--review',type=Path,required=True)
    p.add_argument('--gates',type=Path,default=Path('scripts/boss-art.py'))
    p.add_argument('--root-source',type=Path)
    p.add_argument('--crown-source',type=Path)
    p.add_argument('--tells-only',action='store_true')
    a=p.parse_args()
    if not a.tells_only: run(a)
    if a.root_source and a.crown_source: tells(a)
