#!/usr/bin/env python3
"""Nearest-sample the retained concepts and hand-clean the native Pennon Rush recipes.
Usage: python scripts/pennon-art.py SOURCE_DIRECTORY REVIEW_DIRECTORY
       python scripts/pennon-art.py --check
"""
from pathlib import Path
from collections import Counter
import importlib.util,json,sys
from PIL import Image,ImageDraw
spec=importlib.util.spec_from_file_location('rafter_art',Path(__file__).with_name('rafterwake-art.py'))
rafter=importlib.util.module_from_spec(spec);spec.loader.exec_module(rafter)
boss=rafter.boss
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'crates/delve/assets/pennon'
COOL='abcd';WARM='efgh';RAMPS=[COOL,WARM]
INK=dict(zip(COOL+WARM+'ij',[(40,55,70),(57,84,103),(102,133,144),(148,166,164),(73,38,36),(121,63,47),(175,109,63),(214,159,91),(224,150,44),(115,76,40)]))
REC={}
POSES=['idle','walk-1','walk-2','drop-tell','drop','hook-tell','hook','flinch','damage-1','damage-2','defeat','recover']
KNIGHTS=['idle','run-1','run-2','jump','fall','land','aim-up','aim-down','hit','victory','duck','shoot']
def rows(im):return boss.snap(im,{k:INK[k] for k in COOL+WARM})[1]
def picture(rs):return boss.render(rs,INK)
def clustered(rs,ramps=RAMPS):
    cells=list(map(list,rs));w=len(rs[0]);h=len(rs)
    while True:
        bad=list(boss.region_errors([''.join(r) for r in cells],ramps))
        if not bad:break
        k,part=bad[0]
        ns=Counter(cells[y][x] for px,py in part for x,y in boss.neighbours(px,py) if 0<=x<w and 0<=y<h and (x,y) not in part and cells[y][x]!='.')
        ramp=next(r for r in ramps if k in r)
        choices={c:n for c,n in ns.items() if c in ramp and c!=k} or {c:n for c,n in ns.items() if c!=k}
        if not choices:
            for x,y in part:cells[y][x]='.'
        else:
            use=max(choices,key=lambda c:(choices[c],-abs(boss.lightness(INK[k])-boss.lightness(INK[c]))))
            for x,y in part:cells[y][x]=use
    return [''.join(r) for r in cells]
def stroke(asset,name,path,width,k):
    rs=asset['rows'];w=len(rs[0]);h=len(rs)
    mask=Image.new('1',(w,h));ImageDraw.Draw(mask).line(path,fill=1,width=width)
    pts={(x,y) for y in range(h) for x in range(w) if mask.getpixel((x,y))}
    if not pts:return
    under=[[x,y,rs[y][x]] for x,y in sorted(pts)]
    asset['rows']=[''.join(k if (x,y) in pts else c for x,c in enumerate(row)) for y,row in enumerate(rs)]
    asset.setdefault('linear_features',[]).append(dict(tag='LINEAR',name=name,path=path,width=width,ink=k,pixels=sorted(pts),underlay=under))
def perimeter(asset):
    # Trace the deliberate one-native-pixel contour, retaining exact mass underlays.
    rs=asset['rows'];w=len(rs[0]);h=len(rs)
    boundary={(x,y) for y,r in enumerate(rs) for x,k in enumerate(r) if k!='.' and any(nx<0 or ny<0 or nx>=w or ny>=h or rs[ny][nx]=='.' for nx,ny in boss.neighbours(x,y))}
    for part in boss.components(boundary):
        # A connected perimeter walk retraces branches; it never tags interior texture.
        if len(part)<6 or max(max(x for x,y in part)-min(x for x,y in part),max(y for x,y in part)-min(y for x,y in part))<5:continue
        start=min(part);path=[start];seen={start}
        def walk(p):
            for q in boss.neighbours(*p):
                if q in part and q not in seen:seen.add(q);path.append(q);walk(q);path.append(p)
        walk(start)
        if len(path)<6:continue
        k=COOL[0] if sum(rs[y][x] in COOL for x,y in part)>len(part)//2 else WARM[0]
        stroke(asset,'selective coloured silhouette contour',path,1,k)
def light(asset,cx,cy):
    rs=asset['rows'];w=len(rs[0]);h=len(rs)
    core={(cx,cy),(cx+1,cy),(cx-1,cy+1),(cx,cy+1),(cx+1,cy+1)}
    fall={p for q in core for p in boss.neighbours(*q)}-core
    assert all(0<=x<w and 0<=y<h and rs[y][x]!='.' for x,y in core|fall),(cx,cy)
    s=dict(name='curved amber visor',pixels=[[x,y,'i'] for x,y in sorted(core)],falloff=[[x,y,'j'] for x,y in sorted(fall)],hues={k:'#%02x%02x%02x'%INK[k] for k in 'ij'})
    asset['signature_lights']=[s]
    asset['rows']=boss.signature_rows(asset)
def save(name,asset):
    REC[name]=asset;picture(asset['rows']).save(OUT/(name+'.png'))

def tile(src,n,cols,nrows,size,fill):return rafter.tile(src,n%cols,n//cols,cols,nrows,size,fill)
def sheet(names,path,scale=1,cols=4):
    ims=[Image.open(OUT/(n+'.png')).convert('RGBA') for n in names];w=max(max(i.width for i in ims)+12,max(len(n)*6+6 for n in names));h=max(i.height for i in ims)+22
    canvas=Image.new('RGB',(w*cols*scale,h*((len(ims)+cols-1)//cols)*scale),(8,12,16));d=ImageDraw.Draw(canvas)
    for n,(name,im) in enumerate(zip(names,ims)):
        x=(n%cols*w+(w-im.width)//2)*scale;y=(n//cols*h+3)*scale;im=im.resize((im.width*scale,im.height*scale),Image.Resampling.NEAREST);canvas.paste(im,(x,y),im);d.text((n%cols*w*scale+3,(n//cols*h+h-16)*scale),name,fill=(148,166,164))
    canvas.save(path)
def sampled(src,review):
    review.mkdir(parents=True,exist_ok=True)
    source=Image.open(src/'captain.png').convert('RGBA')
    for n,p in enumerate(POSES):
        im=tile(source,n,4,3,(64,80),74);asset={'rows':rows(im),'ramps':RAMPS}
        picture(asset['rows']).save(review/f'raw-captain-{p}.png')
    out=Image.new('RGB',(64*4*3,80*3*3),(8,12,16))
    for n,p in enumerate(POSES):
        im=Image.open(review/f'raw-captain-{p}.png').resize((192,240),Image.Resampling.NEAREST);out.paste(im,(n%4*192,n//4*240),im)
    out.save(review/'rejected-direct-3x.png')
if __name__=='__main__':
    if sys.argv[1]=='--sample':sampled(Path(sys.argv[2]),Path(sys.argv[3]))
def prepare(im,authored=False):
    rs=rows(im)
    # Retain the silhouette from nearest sampling, erase only detached background debris.
    pts={(x,y) for y,r in enumerate(rs) for x,k in enumerate(r) if k!='.'}
    parts=sorted(boss.components(pts),key=len,reverse=True)
    if parts and not authored:
        for part in parts[1:]:
            for x,y in part: im.putpixel((x,y),(0,0,0,0))
    rs=clustered(rows(im));return dict(rows=rs,ramps=RAMPS,linear_features=[])
def finalize(a):
    a['material_rows']=a['rows'][:]
    if a.get('signature_lights'):a['rows']=boss.signature_rows(a)
    return a
def add_light(a,x,y):
    # Position the explicit curved slit inside the nearest available full face plane.
    rs=a['rows'];w=len(rs[0]);h=len(rs)
    candidates=[]
    for yy in range(max(2,y-7),min(h-3,y+8)):
        for xx in range(max(2,x-7),min(w-3,x+8)):
            if all(rs[yy+dy][xx+dx]!='.' for dy in [-1,0,1,2] for dx in [-2,-1,0,1,2]):candidates.append((abs(xx-x)+abs(yy-y),xx,yy))
    assert candidates,(x,y)
    _,x,y=min(candidates)
    # Signatures belong after the material recipe and never erase its provenance.
    material=a['rows'];light(a,x,y);a['rows']=material
    return (x,y)
def damage(a,level):
    import copy
    a=copy.deepcopy(a)
    # Rebuild the broad plates below the retained rims, then split real material.
    old=a.get('linear_features',[]);cells=list(map(list,a['material_rows']))
    for feature in reversed(old):
        for x,y,k in feature['underlay']:cells[y][x]=k
    base=picture([''.join(r) for r in cells]);mask=Image.new('1',base.size);d=ImageDraw.Draw(mask)
    d.polygon([(31,41),(40,41),(44,46),(40,51),(42,59),(34,64),(30,58)],fill=1)
    if level==2:d.polygon([(19,41),(31,41),(32,47),(28,53),(33,60),(25,66),(18,59)],fill=1)
    hx,hy=a.get('helmet_origin',[15,12])
    for y in range(80):
        for x in range(64):
            if mask.getpixel((x,y)) and base.getpixel((x,y))[3] and not (hx<=x<hx+29 and hy<=y<hy+29):base.putpixel((x,y),INK['a']+(255,))
    a['rows']=clustered(rows(base));a['linear_features']=[];a['signature_lights']=[]
    for f in old:
        if any(word in f['name'] for word in ['fracture','dented','branch']):continue
        stroke(a,f['name'],f['path'],f['width'],f['ink'])
    stroke(a,'right plate fracture core',[(36,43),(37,49),(34,54),(37,60)],1,'h')
    if level==2:
        stroke(a,'left plate fracture core',[(25,46),(24,51),(27,57),(24,63)],1,'h')
        stroke(a,'dented crest',[(29,10),(30,17),(36,21)],3,'a')
    return a
def props():
    for name,size in [('merlon',(32,40)),('brick',(32,16)),('repair',(24,24)),('pennant',(32,48)),('bolt',(16,8)),('enemy-bolt',(12,12))]:
        im=Image.new('RGBA',size);d=ImageDraw.Draw(im)
        if name=='merlon':
            d.rounded_rectangle((1,1,30,39),4,fill=INK['a']);d.rounded_rectangle((3,3,28,38),3,fill=INK['b']);d.rectangle((5,5,24,18),fill=INK['c']);d.rectangle((6,5,17,9),fill=INK['d'])
        elif name=='brick':
            d.rounded_rectangle((0,0,31,15),3,fill=INK['a']);d.rounded_rectangle((2,2,29,13),2,fill=INK['b']);d.rectangle((4,2,27,6),fill=INK['c']);d.rectangle((6,2,17,5),fill=INK['d'])
        elif name=='repair':
            d.polygon([(6,2),(17,2),(21,9),(17,21),(6,21),(2,9)],fill=INK['e']);d.polygon([(7,4),(16,4),(18,10),(15,18),(8,18),(5,10)],fill=INK['f']);d.rectangle((8,6,15,13),fill=INK['g']);d.rectangle((8,6,12,9),fill=INK['h'])
        elif name=='pennant':
            d.rectangle((2,2,7,47),fill=INK['a']);d.rectangle((4,2,7,40),fill=INK['c']);d.polygon([(8,4),(30,7),(23,15),(30,22),(8,19)],fill=INK['e']);d.polygon([(10,6),(26,8),(20,15),(26,19),(10,17)],fill=INK['f']);d.rectangle((11,7,16,13),fill=INK['g'])
        else:
            colour='f' if name=='enemy-bolt' else 'b'
            d.polygon([(1,size[1]//2),(size[0]//2,1),(size[0]-1,size[1]//2),(size[0]//2,size[1]-2)],fill=INK[colour]);d.rectangle((size[0]//2-2,2,size[0]//2+2,size[1]-3),fill=INK['g' if name=='enemy-bolt' else 'd'])
        a=prepare(im);perimeter(a)
        if name=='merlon':stroke(a,'curved tie rod',[(5,26),(12,29),(23,28),(28,24)],2,'a')
        if name=='repair':stroke(a,'shield repair seam',[(8,7),(12,11),(16,7),(12,11),(12,17)],2,'h')
        save(name,finalize(a))
def validate(recipe):
    log=[]
    for name,a in recipe['assets'].items():
        material=a.get('material_rows',a['rows']);linear=boss.linear_pixels(a.get('linear_features',[]),material)
        boss.region_gate(material,a['ramps'],4,a.get('linear_features',[]))
        lights={(x,y) for s in a.get('signature_lights',[]) for x,y,_ in s['pixels']+s['falloff']}
        under=list(map(list,material));w=len(material[0]);h=len(material)
        for f in reversed(a.get('linear_features',[])):
            for x,y,k in f['underlay']:under[y][x]=k
        split={(x,y) for y,r in enumerate(a['rows']) for x,k in enumerate(r) if any((nx,ny) in linear and 0<=nx<w and 0<=ny<h and under[ny][nx]==k for nx,ny in boss.neighbours(x,y))}

        try:boss.gate(a['rows'],{k:INK[k] for k in set(''.join(a['rows']))-{'.'}},linear|lights|split)
        except AssertionError as e:raise AssertionError(name+': '+str(e))
        assert not boss.signature_gate(a,INK) if a.get('signature_lights') else True,name
        assert max(max(INK[k]) for k in set(''.join(a['rows']))-{'.'})<240
    for k,part in boss.region_errors(['aaa']*3,['a']):assert k=='a'
    assert list(boss.region_errors(['a'],['a']))
    assert not list(boss.region_errors(['aaaa']*4,['a']))
    idle=picture(recipe['assets']['captain-idle']['rows'])
    knight=picture(recipe['assets']['knight-idle']['rows'])
    height=idle.getbbox()[3]-idle.getbbox()[1];kh=knight.getbbox()[3]-knight.getbbox()[1]
    assert height>=64 and kh>=41 and 1.5<=height/kh<=1.8
    def mean(box):
        colours=[c for c in idle.crop(box).get_flattened_data() if c[3]]
        return sum(boss.lightness(c) for c in colours)/len(colours)
    separation=abs(mean((15,12,44,40))-mean((20,39,44,61)))
    assert separation>=15
    floor=boss.lightness((6,11,16))
    boundary=[INK[k] for y,row in enumerate(recipe['assets']['captain-idle']['rows']) for x,k in enumerate(row) if k!='.' and any(nx<0 or ny<0 or nx>=64 or ny>=80 or recipe['assets']['captain-idle']['rows'][ny][nx]=='.' for nx,ny in boss.neighbours(x,y))]
    edge=min(boss.lightness(c)-floor for c in boundary)
    assert edge>=15
    log.append(f'Captain/knight {height}/{kh}px; captain fills {height/80:.1%} of its cell. Head/body {separation:.1f} L*, weakest outer/dark backdrop {edge:.1f} L*.')
    log.append(f'{len(recipe["assets"])} native assets: strict four-tone masses, 4x4 gate, explicit LINEAR provenance, signatures, <=10 inks pass.')
    return log
def canvas(size):
    im=Image.new('RGBA',size);d=ImageDraw.Draw(im)
    def poly(points,k):d.polygon(points,fill=INK[k])
    def rect(box,k):d.rectangle(box,fill=INK[k])
    def cap(path,width,k):
        d.line(path,fill=INK[k],width=width)
        r=width//2
        for x,y in path:d.ellipse((x-r,y-r,x+r,y+r),fill=INK[k])
    return im,poly,rect,cap

def captain(source,pose,n,stage=3):
    # Source silhouette and identity survive as an authored cluster reconstruction.
    im,poly,rect,cap=canvas((64,80))
    crouch=9 if pose.startswith('hook') or pose=='recover' else 0
    if pose=='hook-tell':crouch=18-stage*6
    lean=-3 if pose=='flinch' else (stage-1)*2 if pose=='drop-tell' else 0
    if pose=='defeat':
        poly([(9,64),(18,51),(39,51),(50,63),(47,76),(12,77)],'a')
        poly([(13,64),(21,55),(36,55),(45,64),(42,72),(14,72)],'b')
        rect((20,57,32,63),'c');rect((20,57,25,60),'d');hx,hy=19,42
    else:
        stride=3 if pose=='walk-1' else -3 if pose=='walk-2' else 0
        for x,yy in [(12-stride,64),(36+stride,64)]:
            poly([(x,yy),(x+13,yy),(x+15,77),(x-3,77),(x-3,72)],'a')
            poly([(x+2,yy+2),(x+11,yy+2),(x+11,74),(x-1,74),(x-1,71)],'b')
            rect((x+2,yy+3,x+8,yy+7),'c')
        # Split cloth below the breastplate remains a separate warm silhouette.
        poly([(20,49),(43,49),(46,69),(35,67),(31,71),(17,67)],'e')
        poly([(23,51),(39,51),(41,64),(32,62),(26,66),(20,64)],'f')
        rect((25,53,31,60),'g');rect((25,53,28,56),'h')
        def arm(path):
            cap(path,13,'a');cap([(x-1,y-1) for x,y in path],9,'b');cap([(x-2,y-2) for x,y in path],5,'c')
        if pose=='drop-tell':
            paths=[[(44,42),(53,34),(54,26)],[(44,40),(52,28),(51,18)],[(44,40),(51,21),(44,12)],[(44,40),(46,18),(35,8)]]
            arm(paths[stage]);ex,ey=paths[stage][-1]
            poly([(ex-7,ey-4),(ex+5,ey-4),(ex+8,ey+1),(ex+4,ey+6),(ex-6,ey+5)],'e');rect((ex-4,ey-2,ex+2,ey+3),'g')
        elif pose=='drop':arm([(44,43),(51,58),(40,69)])
        elif pose=='hook-tell':arm([(45,47+crouch//2),(54,48-stage*3),(56,60-stage*4)])
        elif pose=='hook':arm([(45,47),(30,58),(10,65)])
        else:arm([(44,43+crouch//2),(52,52+crouch//2),(49,63)])
        top=34+crouch//2
        poly([(20+lean,top),(42+lean,top),(49,top+9),(46,62),(39,66),(21,64),(15,54),(14,top+10)],'a')
        poly([(21+lean,top+3),(39+lean,top+3),(44,top+9),(42,59),(23,60),(19,53),(18,top+11)],'g')
        poly([(23+lean,top+4),(35,top+4),(40,top+9),(37,top+18),(24,top+19),(20,top+11)],'h')
        rect((24+lean,top+5,30+lean,top+9),'g')
        # Near pauldron has a broad plane and a copper rim, separate from the chest.
        poly([(10,top),(20,top-2),(27,top+4),(25,top+12),(13,top+13),(7,top+7)],'a')
        poly([(12,top+2),(19,top+1),(23,top+5),(22,top+9),(12,top+9),(10,top+6)],'c')
        if pose=='flinch':arm([(15,43),(8,35),(10,27)])
        elif pose.startswith('hook'):arm([(17,47+crouch//2),(11,59),(20,65)])
        elif pose=='drop':arm([(17,43),(9,52),(16,59)])
        else:arm([(16,44+crouch//2),(10,53+crouch//2),(12,64)])
        hx,hy=15+lean,(27-stage*5 if pose=='drop-tell' else 12+crouch//2)
    # Rounded helmet and flush faceplate. The face stays distinct from the torso.
    poly([(hx+6,hy),(hx+19,hy),(hx+26,hy+6),(hx+28,hy+17),(hx+24,hy+26),(hx+8,hy+28),(hx+1,hy+20),(hx,hy+10)],'a')
    poly([(hx+7,hy+3),(hx+18,hy+3),(hx+22,hy+7),(hx+23,hy+15),(hx+6,hy+17),(hx+3,hy+11)],'b')
    poly([(hx+8,hy+4),(hx+16,hy+4),(hx+20,hy+8),(hx+18,hy+12),(hx+5,hy+12),(hx+4,hy+9)],'c')
    rect((hx+8,hy+4,hx+12,hy+7),'d')
    poly([(hx+4,hy+15),(hx+21,hy+14),(hx+25,hy+18),(hx+22,hy+24),(hx+8,hy+25),(hx+3,hy+21)],'a')
    # Exposed cheek, with a dark rim against the metal face.
    poly([(hx+8,hy+22),(hx+20,hy+21),(hx+19,hy+27),(hx+11,hy+28),(hx+7,hy+25)],'f')
    rect((hx+10,hy+23,hx+15,hy+26),'g')
    # The swept fan and pennant are broad native masses, never sampled texture.
    poly([(hx+1,hy+4),(hx-2,hy-7),(hx+5,hy-11),(hx+16,hy-10),(hx+22,hy-3),(hx+23,hy+7),(hx+16,hy+3)],'e')
    poly([(hx+2,hy),(hx+1,hy-6),(hx+6,hy-8),(hx+14,hy-7),(hx+18,hy-2),(hx+17,hy+2)],'f')
    rect((hx+4,hy-6,hx+10,hy-2),'g')
    if pose!='defeat':
        by=10 if not pose.startswith('hook') else 15
        poly([(49,by),(63,by+3),(56,by+9),(63,by+16),(49,by+14)],'e')
        poly([(51,by+2),(59,by+4),(53,by+8),(58,by+12),(51,by+11)],'f')
        rect((51,by+3,54,by+7),'g')
    a=prepare(im,True);perimeter(a)
    a['helmet_origin']=[hx,hy]
    for x,yy in ([(12,64),(36,64)] if pose!='defeat' else []):
        stroke(a,'rounded steel boot rim',[(x-1,74),(x+4,72),(x+12,74)],2,'c')
    stroke(a,'cheek mouth edge',[(hx+9,hy+25),(hx+13,hy+26),(hx+19,hy+24)],1,'e')
    # Long ribs and rims articulate the clean masses at the same grain.
    for dx in [2,7,12,17]:stroke(a,'copper crest structural rib',[(hx+dx,hy-8),(hx+dx+3,hy+1)],1,'g')
    stroke(a,'crest raised curved rim',[(hx-1,hy-7),(hx+6,hy-10),(hx+15,hy-9),(hx+21,hy-3)],2,'h')
    stroke(a,'rounded visor lip',[(hx+3,hy+14),(hx+10,hy+12),(hx+22,hy+13)],1,'d')
    if pose!='defeat':
        stroke(a,'offset pennant mast',[(48,5),(48,66)],3,'a')
        stroke(a,'mast lit side',[(47,6),(47,60)],1,'c')
        stroke(a,'swallowtail banner hem',[(50,by+1),(61,by+4),(55,by+9),(61,by+14),(50,by+13)],1,'g')
        stroke(a,'pauldron curved copper rim',[(9,top+8),(14,top+12),(22,top+11),(25,top+7)],2,'g')
        stroke(a,'breastplate belt rim',[(20,60),(29,63),(39,63),(45,59)],2,'a')
        stroke(a,'belt warm upper edge',[(20,59),(29,61),(39,61),(44,58)],1,'h')
        for yy in [50,58]:stroke(a,'near gauntlet band',[(7,yy),(11,yy+2),(16,yy+1)],2,'g')
    hookx=8 if pose not in ['hook-tell','hook'] else 54 if pose=='hook-tell' else 9
    hooky=65 if pose not in ['drop','defeat'] else 72
    if pose=='hook-tell':hooky=68-stage*5
    stroke(a,'broad ladder hook',[(hookx+5,hooky-7),(hookx,hooky-7),(hookx-4,hooky-2),(hookx-3,hooky+5),(hookx+3,hooky+7),(hookx+7,hooky+3)],3,'b')
    stroke(a,'hook inner edge',[(hookx+4,hooky-6),(hookx,hooky-5),(hookx-2,hooky),(hookx+1,hooky+4),(hookx+6,hooky+2)],1,'d')
    add_light(a,hx+8,hy+18)
    return finalize(a)

def knight(source,n):
    im,poly,rect,cap=canvas((48,48))
    crouch=16 if n==10 else 7 if n==5 else 0
    lean=-3 if n==8 else 0
    hx,hy=15+lean,9+crouch
    # Whole-body silhouettes change, including both arms and the knee lines.
    if n==9:hx,hy=16,7
    stride=4 if n==1 else -4 if n==2 else 0
    legs=[(15-stride,35),(28+stride,35)]
    if n==3:legs=[(14,31),(28,29)]
    if n==4:legs=[(14,35),(29,36)]
    if n in [5,10]:legs=[(12,39),(30,39)]
    for x,y in legs:
        poly([(x,y),(x+7,y),(x+9,min(45,y+10)),(x-3,min(45,y+10)),(x-3,min(43,y+7))],'a')
        rect((x+1,y+1,x+5,min(43,y+7)),'b');rect((x+1,y+2,x+4,min(42,y+5)),'c')
    poly([(17+lean,23+crouch),(29+lean,23+crouch),(33,36+crouch//2),(26,40),(16,38),(13,29+crouch)],'e')
    poly([(19+lean,25+crouch),(27+lean,25+crouch),(29,35+crouch//2),(23,37),(17,35),(16,29+crouch)],'f')
    rect((20+lean,27+crouch,24+lean,32+crouch),'g')
    # Broad helm, two raised steel planes and a warm slit; source plume stays swept.
    poly([(hx+4,hy),(hx+14,hy),(hx+19,hy+5),(hx+20,hy+14),(hx+15,hy+19),(hx+4,hy+18),(hx,hy+12),(hx,hy+5)],'a')
    poly([(hx+4,hy+2),(hx+13,hy+2),(hx+16,hy+5),(hx+16,hy+12),(hx+4,hy+14),(hx+2,hy+10),(hx+2,hy+5)],'b')
    poly([(hx+5,hy+3),(hx+12,hy+3),(hx+15,hy+6),(hx+13,hy+9),(hx+3,hy+9)],'c')
    rect((hx+5,hy+3,hx+9,hy+6),'d')
    poly([(hx-9,hy-4),(hx-3,hy-8),(hx+5,hy-7),(hx+9,hy-3),(hx+1,hy-1),(hx-6,hy+2),(hx-12,hy+2)],'e')
    poly([(hx-7,hy-4),(hx-2,hy-6),(hx+4,hy-5),(hx+5,hy-3),(hx-3,hy-1),(hx-8,hy)],'g')
    def arm(path):cap(path,8,'a');cap([(x-1,y-1) for x,y in path],6,'b');cap([(x-1,y-2) for x,y in path],4,'c')
    if n==6:arm([(16,29),(24,25),(33,17)]);arm([(29,28),(34,25),(38,17)]);bow=[(33,8),(41,14),(42,22),(36,25)]
    elif n==7:arm([(17,29),(24,32),(34,35)]);arm([(29,29),(34,31),(38,35)]);bow=[(38,27),(44,31),(43,39),(37,43)]
    elif n==9:arm([(16,28),(10,19),(12,12)]);arm([(29,27),(36,20),(35,12)]);bow=[(33,5),(41,10),(42,18),(36,23)]
    elif n==8:arm([(15,30),(8,24),(9,17)]);arm([(28,30),(32,32),(36,35)]);bow=[(35,28),(43,32),(43,40),(37,44)]
    else:
        yy=31+crouch//2-(3 if n in [3,10] else 0)
        arm([(16,29+crouch//2),(23,yy),(32,yy)]);arm([(28,28+crouch//2),(32,yy),(38,yy)])
        bow=[(37,yy-10),(44,yy-6),(45,yy+2),(38,yy+8)]
    a=prepare(im,True);perimeter(a)
    stroke(a,'rounded visor opening',[(hx+4,hy+12),(hx+14,hy+11)],3,'a')
    stroke(a,'warm visor slit',[(hx+5,hy+12),(hx+13,hy+11)],1,'h')
    stroke(a,'helmet brow rim',[(hx+3,hy+9),(hx+9,hy+8),(hx+16,hy+9)],1,'d')
    stroke(a,'plume root',[(hx+4,hy-3),(hx+8,hy+3)],3,'f')
    stroke(a,'plume swept rib',[(hx-8,hy-3),(hx-2,hy-5),(hx+5,hy-3)],1,'h')
    stroke(a,'forked hook bow',bow,3,'e');stroke(a,'bow upper warm edge',bow[:3],1,'h')
    stroke(a,'bow string',[bow[0],bow[-1]],1,'b')
    for x,y in legs:stroke(a,'boot steel rim',[(x-1,min(44,y+8)),(x+3,min(44,y+7)),(x+7,min(44,y+8))],1,'c')
    return finalize(a)

def sentry(source,n):
    im,poly,rect,cap=canvas((48,48));lower=5 if n in [5,6,11] else 0
    lean=-3 if n==7 else 0
    if n==10:
        poly([(6,36),(15,26),(31,29),(39,40),(35,45),(7,45)],'e');rect((12,32,27,40),'f');rect((15,32,24,36),'g');hx,hy=14,24
    else:
        stride=3 if n==1 else -3 if n==2 else 0
        for x in [13-stride,29+stride]:rect((x,34,x+7,44),'a');rect((x+1,36,x+5,41),'c')
        poly([(13+lean,19+lower),(32+lean,19+lower),(38,30+lower),(32,38),(15,38),(8,29+lower)],'e')
        poly([(15+lean,22+lower),(29+lean,22+lower),(33,29+lower),(29,35),(17,35),(12,29+lower)],'f');rect((17+lean,23+lower,25+lean,29+lower),'g');rect((18+lean,24+lower,21+lean,27+lower),'h')
        hx,hy=13+lean,8+lower
    poly([(hx+3,hy),(hx+15,hy),(hx+19,hy+5),(hx+18,hy+14),(hx+5,hy+16),(hx,hy+11),(hx,hy+4)],'a')
    poly([(hx+4,hy+3),(hx+14,hy+3),(hx+15,hy+7),(hx+13,hy+12),(hx+4,hy+12),(hx+2,hy+8)],'c')
    a=prepare(im,True);perimeter(a)
    stroke(a,'twin pronged wind vane',[(hx+4,hy),(hx+1,max(1,hy-6)),(hx-3,max(1,hy-6)),(hx+1,max(1,hy-6)),(hx+10,max(1,hy-3)),(hx+14,max(1,hy-7)),(hx+18,max(1,hy-7))],2,'g')
    by=25+lower-(5 if n in [3,4] else 0)
    stroke(a,'forked wooden bow',[(8,by-5),(3,by),(5,by+7),(11,by+9)],3,'f')
    stroke(a,'bow warm rim',[(7,by-4),(4,by),(6,by+6)],1,'h')
    stroke(a,'bow stock',[(5,by+2),(26,by+3)],3,'e')
    stroke(a,'seed weight chain',[(35,22+lower),(38,36)],2,'b')
    # Pear weight is a large shaped core, carried by an explicit long handle.
    stroke(a,'seed weight',[(38,34),(36,39),(39,43),(42,40),(40,34)],3,'g')
    for xx in [18,26]:stroke(a,'bent stave seam',[(xx,23+lower),(xx+2,28+lower),(xx,35)],1,'e')
    if n in [8,9]:
        stroke(a,'sentry dark fracture',[(16,22),(23,27),(20,34)],3,'a');stroke(a,'sentry fracture lit edge',[(17,22),(24,27),(21,34)],1,'h')
    if n==9:stroke(a,'sentry second dark fracture',[(30,21),(27,31),(31,37)],3,'a')
    add_light(a,hx+6,hy+7);return finalize(a)

def main(src,review):
    OUT.mkdir(exist_ok=True);review.mkdir(parents=True,exist_ok=True)
    source=Image.open(src/'captain.png').convert('RGBA')
    for n,p in enumerate(POSES):
        a=captain(source,p,n)
        if p in ['damage-1','damage-2']:
            base=captain(source,'idle',0);a=damage(base,1 if p=='damage-1' else 2);a['signature_lights']=base['signature_lights'];a=finalize(a)
        save('captain-'+p,a)
        if p!='defeat':
            for level in [1,2]:
                v=damage(a,level);v['signature_lights']=a['signature_lights'];save(f'captain-{p}-stage-{level}',finalize(v))
    for attack,n in [('drop',3),('hook',5)]:
        for stage in range(4):
            a=captain(source,attack+'-tell',n,stage);save(f'captain-{attack}-tell-{stage}',a)
            for level in [1,2]:
                v=damage(a,level);v['signature_lights']=a['signature_lights'];save(f'captain-{attack}-tell-{stage}-stage-{level}',finalize(v))
    source=Image.open(src/'knight.png').convert('RGBA')
    for n,p in enumerate(KNIGHTS):save('knight-'+p,knight(source,n))
    source=Image.open(src/'sentry.png').convert('RGBA')
    for n,p in enumerate(POSES):save('sentry-'+p,sentry(source,n))
    props()
    # Reconstruct the nearest-sampled effect motifs as connected native shapes.
    for kind in ['drop','hook','impact']:
        for n in range(4):
            im,poly,rect,cap=canvas((48,32))
            if kind=='drop':
                yy=[5,17,24,27][n]
                if n<2:
                    poly([(19,yy-4),(27,yy-4),(32,yy+3),(25,yy+9),(18,yy+6),(15,yy+1)],'e')
                    rect((20,yy-1,26,yy+4),'g');rect((21,yy,24,yy+3),'h')
                if n>0:
                    poly([(3,30),(6,22-n*2),(15,25),(19,18+n),(25,26),(34,20+n),(44,28),(45,31)],'a')
                    poly([(7,29),(9,23),(17,27),(21,22),(25,28),(35,23+n),(41,29)],'c')
                    rect((14,27,30,30),'g')
            elif kind=='hook':
                x0=5+n*2
                poly([(x0,29),(x0+10,26),(x0+19,18),(x0+17,11),(x0+8,5),(x0+20,8),(x0+30,17),(x0+28,25),(x0+18,30)],'a')
                poly([(x0+11,28),(x0+22,21),(x0+22,15),(x0+16,10),(x0+25,14),(x0+27,21),(x0+23,26)],'c')
                rect((x0+17,23,x0+21,27),'d')
            else:
                radius=[9,14,12,6][n];cx,cy=24,16
                poly([(cx-radius,cy),(cx-5,cy-4),(cx,cy-radius),(cx+4,cy-5),(cx+radius,cy),(cx+5,cy+4),(cx,cy+radius),(cx-4,cy+5)],'e')
                poly([(cx-radius+3,cy),(cx-3,cy-3),(cx,cy-radius+3),(cx+3,cy-3),(cx+radius-3,cy),(cx+3,cy+3),(cx,cy+radius-3),(cx-3,cy+3)],'g')
                rect((21,13,26,18),'h')
            save(f'fx-{kind}-{n}',finalize(prepare(im,True)))
    # Low, solid two-tone landing dust is behind all bodies.
    for n in range(4):
        im=Image.new('RGBA',(48,24));d=ImageDraw.Draw(im);h=[7,10,12,6][n]
        for x in [3-n,29+n]:
            d.ellipse((x,23-h,x+15,27),fill=INK['b']);d.rectangle((x+4,24-h,x+11,23),fill=INK['c'])
        save(f'fx-dust-{n}',finalize(prepare(im)))
    recipe={'inks':INK,'assets':REC};log=[]
    (OUT/'native.json').write_text(json.dumps(recipe,separators=(',',':'))+'\n')
    for scale in [1,3]:
        sheet(['captain-'+p for p in POSES],review/f'captain-{scale}x.png',scale)
        variants=[n for n in REC if '-stage-' in n]
        for start in range(0,len(variants),12):sheet(variants[start:start+12],review/f'variants-{start//12+1}-{scale}x.png',scale,3)
        sheet([f'fx-dust-{n}' for n in range(4)],review/f'dust-{scale}x.png',scale)
        sheet(['knight-'+p for p in KNIGHTS],review/f'knights-{scale}x.png',scale)
        sheet(['sentry-'+p for p in POSES],review/f'sentry-{scale}x.png',scale)
        sheet([f'captain-{a}-tell-{n}' for a in ['drop','hook'] for n in range(4)],review/f'tells-{scale}x.png',scale)
        sheet(['merlon','brick','repair','pennant','bolt','enemy-bolt'],review/f'props-{scale}x.png',scale,3)
        if (src/'effects.png').exists():sheet([f'fx-{a}-{n}' for a in ['drop','hook','impact'] for n in range(4)],review/f'effects-{scale}x.png',scale)
    log=validate(recipe)
    (review/'art-gates.txt').write_text('\n'.join(log)+'\n')
if __name__=='__main__' and sys.argv[1]!='--sample':
    if sys.argv[1]=='--check':print('\n'.join(validate(json.loads((OUT/'native.json').read_text()))))
    else:main(Path(sys.argv[1]),Path(sys.argv[2]))
