#!/usr/bin/env python3
"""Bounded review sheets from native PNGs and the real exported cell buffers."""
import importlib.util,json,math,sys
from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
spec=importlib.util.spec_from_file_location('slots',Path(__file__).with_name('slot-art-import.py'));s=importlib.util.module_from_spec(spec);spec.loader.exec_module(s)
out=Path(sys.argv[1]);root=s.OUTPUT;cap=out/'captures'
font=ImageFont.truetype('/usr/share/fonts/TTF/DejaVuSansMono.ttf',13) if Path('/usr/share/fonts/TTF/DejaVuSansMono.ttf').exists() else ImageFont.load_default()

def cells(path):
    data=json.loads(path.read_text());im=Image.new('RGB',(768,448));d=ImageDraw.Draw(im)
    for i,c in enumerate(data):
        x=i%96*8;y=i//96*16;fg=tuple(c['fg']);bg=tuple(c['bg']);ch=c['glyph']
        d.rectangle((x,y,x+7,y+15),fill=bg)
        if ch=='▀':d.rectangle((x,y,x+7,y+7),fill=fg)
        elif ch and 0x2800<=ord(ch[0])<=0x28ff:
            bits=ord(ch[0])-0x2800
            for k,(dx,dy) in enumerate([(0,0),(0,1),(0,2),(1,0),(1,1),(1,2),(0,3),(1,3)]):
                if bits&(1<<k):d.rectangle((x+dx*4+1,y+dy*4+1,x+dx*4+2,y+dy*4+2),fill=fg)
        else:d.text((x,y),ch,font=font,fill=fg)
    return im

# All distinct imported frames, plus v2 when its dimensions differ.
for game in ['cinder','latch','mossbell','brasswake','rafterwake','pennon']:
    folders=sorted(root.glob(game+'.*'));tiles=[]
    for f in folders:
        m=json.loads((f/(f.name+'.map.json')).read_text());tile=Image.new('RGB',(480,424),(10,12,16));d=ImageDraw.Draw(tile);d.text((6,4),f.name,fill='wheat')
        pieces=m['pieces'];cols=4;cw=118;ch=min(96,390//max(1,math.ceil(len(pieces)/cols)))
        for i,p in enumerate(pieces):
            im=Image.open(f/'v1'/(p['name']+'.png')).convert('RGBA');im.thumbnail((cw-4,ch-12),Image.Resampling.NEAREST)
            x=i%cols*cw+4;y=i//cols*ch+25;tile.paste(im,(x,y+10),im);d.text((x,y),str(p['piece']),fill='#b0b2a8')
        tiles.append(tile)
    for page,start in enumerate(range(0,len(tiles),12),1):
        group=tiles[start:start+12]
        sheet=Image.new('RGB',(1440,math.ceil(len(group)/3)*424),(0,0,0))
        for i,t in enumerate(group):sheet.paste(t,(i%3*480,i//3*424))
        sheet.save(out/f'imported-{game}-{page}.png')

# Three edge/detail trials: retained alpha allows direct RGB-fringe comparison.
proof=Image.new('RGB',(1536,3*290),(10,12,16));d=ImageDraw.Draw(proof);proofmetrics={}
for row,slot in enumerate(['pennon.knight','latch.knight','cinder.impact']):
    m=json.loads((root/slot/(slot+'.map.json')).read_text());p=m['pieces'][0]
    x,y,w,h=p['source_rect_xywh'];raw=Image.open(s.SOURCES/slot/'v01/source-sheet.png').convert('RGBA').crop((x,y,x+w,y+h));clean,metric=s.despill(raw)
    fringe=[(x,y) for y in range(raw.height) for x in range(raw.width) if s.fringe(raw.getpixel((x,y)))]
    cx,cy=fringe[0] if fringe else (raw.width//2,4)
    box=(max(0,min(raw.width-96,cx-48)),max(0,min(raw.height-80,cy-40)));box=(*box,box[0]+96,box[1]+80)
    for col,(im,label) in enumerate([(raw,'edge before'),(clean,'edge despilled')]):
        patch=im.crop(box).resize((288,240),Image.Resampling.NEAREST)
        proof.paste(patch,(col*384+8,row*290+40),patch);d.text((col*384+8,row*290+8),slot+' '+label,fill='wheat')
    record=json.loads((root/slot/'slot.json').read_text());meta=record['outputs']['v1/'+p['name']+'.png'];size=tuple(meta['canvas_wh']);anchor=meta['anchor_xy'];scale=meta['source_scale']
    _,before=s.sample(raw,size,p['source_pivot_xy'],anchor,scale);after=Image.open(root/slot/'v1'/(p['name']+'.png')).convert('RGBA')
    for col,(im,label) in enumerate([(before,'native before edge cleanup'),(after,'native after edge cleanup')],2):
        im=im.resize((im.width*2,im.height*2),Image.Resampling.NEAREST);proof.paste(im,(col*384+8,row*290+40),im);d.text((col*384+8,row*290+8),label,fill='wheat')
    proofmetrics[slot]=metric
proof.save(out/'despill-proof.png');(out/'despill-proof.json').write_text(json.dumps(proofmetrics,indent=2)+'\n')

names=['high','low','contact','recover']
sheet=Image.new('RGB',(4*400,2*252),(0,0,0));d=ImageDraw.Draw(sheet)
for row,phase in enumerate(['before','after']):
    for col,name in enumerate(names):
        im=Image.open(cap/f'{phase}-{name}.png');sheet.paste(im,(col*400+8,row*252+24));d.text((col*400+8,row*252+4),phase+' '+name,fill='wheat')
sheet.save(out/'pilots-native-1x.png')
sheet=Image.new('RGB',(1600,2*312),(0,0,0));d=ImageDraw.Draw(sheet)
for row,phase in enumerate(['before','after']):
    for col,name in enumerate(['high','low','contact','recover']):
        box=(116,70,224,158) if col<3 else (202,103,274,175)
        factor=3 if col<3 else 4
        im=Image.open(cap/f'{phase}-{name}.png').crop(box);im=im.resize((im.width*factor,im.height*factor),Image.Resampling.NEAREST)
        sheet.paste(im,(col*400+8,row*312+24));d.text((col*400+8,row*312+4),phase+' '+('strike peak 4x' if col==3 else name+' 3x'),fill='wheat')
sheet.save(out/'pilots-enlarged.png')
sheet=Image.new('RGB',(1568,4*476),(0,0,0));d=ImageDraw.Draw(sheet)
items=[('before','contact'),('before','low'),('after','contact'),('after','low')]+[('after',n) for n in ['recover','seat-2','seat-3','seat-4']]
for i,(phase,name) in enumerate(items):
    sheet.paste(cells(cap/f'{phase}-{name}-cells.json'),(i%2*784+8,i//2*476+24));d.text((i%2*784+8,i//2*476+4),phase+' '+name+' actual 96-column cockpit writer',fill='wheat')
sheet.save(out/'pilots-cells.png')
print('Wrote bounded import overviews, three-slot edge proof, native/enlarged pilots and eight actual cell screens.')
