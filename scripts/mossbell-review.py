#!/usr/bin/env python3
"""Review native Mossbell rooms beside the real half-block/braille terminal buffer."""
from pathlib import Path
from PIL import Image, ImageChops, ImageDraw, ImageFont
import argparse, importlib.util, re, json, subprocess

ROOT = Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('review',Path(__file__).with_name('cinder-review.py'))
r=importlib.util.module_from_spec(spec);spec.loader.exec_module(r)
FONT=ImageFont.truetype(subprocess.check_output(['fc-match','monospace','-f','%{file}']).decode(),14)

def cells(path):
    result={}
    for m in re.finditer(r'\x1b\[(\d+);(\d+)H(.*?)(?=\x1b\[\d+;\d+H|$)',path.read_text(),re.S):
        y,x=int(m[1])-1,int(m[2])-1; fg=bg=(0,0,0)
        for code,rr,g,b in re.findall(r'\x1b\[(38|48);2;(\d+);(\d+);(\d+)m',m[3]):
            if code=='38': fg=tuple(map(int,(rr,g,b)))
            else: bg=tuple(map(int,(rr,g,b)))
        ch=re.sub(r'\x1b\[[0-9;?]*[A-Za-z]','',m[3])
        result[x,y]=(ch,fg,bg)
    return result

def cell_image(buf, box):
    x0,y0,x1,y1=box
    out=Image.new('RGB',((x1-x0)*8,(y1-y0)*16)); d=ImageDraw.Draw(out)
    for y in range(y0,y1):
        for x in range(x0,x1):
            ch,fg,bg=buf[x,y]; xx,yy=(x-x0)*8,(y-y0)*16
            d.rectangle((xx,yy,xx+7,yy+15),fill=bg)
            if ch=='▀': d.rectangle((xx,yy,xx+7,yy+7),fill=fg)
            else: d.text((xx,yy-1),ch,font=FONT,fill=fg)
    return out

def tell_review(root, gates, before):
    evidence=json.loads((ROOT/'crates/delve/assets/mossbell/native.json').read_text())
    for prefix,box in [('root-stage',(122,118,306,129)),('crown-stage',(189,66,240,90)),
                       ('root-rage',(122,118,306,129)),('crown-rage',(189,66,240,90))]:
        kind=prefix.split('-')[0]
        x0,y0,x1,y1=box;cbox=(x0//4,y0//8,(x1+3)//4,(y1+7)//8)
        paths=[root/'rooms'/f'{prefix}-{i}.png' for i in range(4)]
        r.sheet([(p,f'{prefix} {i}') for i,p in enumerate(paths)],root/f'{prefix}-table-1x.png',2)
        ims=[Image.open(p).convert('RGB').crop(box) for p in paths]
        bufs=[cells(root/'terminal'/f'{prefix}-{i}-cells.ansi') for i in range(4)]
        proof=Image.new('RGB',(1568,952));d=ImageDraw.Draw(proof)
        for i,buf in enumerate(bufs):
            x=i%2*784+8;y=i//2*476
            d.text((x,y+5),f'{prefix} {i}',fill='#b0b2a8')
            proof.paste(cell_image(buf,(0,0,96,28)),(x,y+24))
        proof.save(root/f'{prefix}-table-cells.png')
        frames=[evidence['tells'][f'{kind}-vines-{i}'] for i in range(4)]
        fronts=[{(x,y) for y,row in enumerate(f['rows']) for x,k in enumerate(row)
                 if k!='.' and (kind!='crown' or k!='a')} for f in frames]
        for i in range(3):
            assert len(fronts[i+1])>len(fronts[i]),(prefix,i,'front must grow')
            n=sum(abs(gates.lightness(p)-gates.lightness(q))>=7.5 for p,q in zip(ims[i].get_flattened_data(),ims[i+1].get_flattened_data()))
            changed={(x,y) for y in range(cbox[1],cbox[3]) for x in range(cbox[0],cbox[2])
                     if bufs[i][x,y]!=bufs[i+1][x,y]}
            # A front advances only where new jaw/stem pixels occupy floor.
            # Colour changes behind that front cannot satisfy this gate.
            fresh={((x0+x)//4,(y0+y)//8) for x,y in fronts[i+1]-fronts[i]}
            advance=len(changed & fresh)
            print(f'{prefix} {i}->{i+1}: {n} pixels >=7.5 L*, {len(changed)} actual cells, {advance} advancing cells')
            assert n and advance>0
            if kind=='crown': assert advance>=15,(prefix,i,advance)
        print(f'{prefix} authored front pixels: '+', '.join(str(len(p)) for p in fronts))
    for name,f in evidence['tells'].items():
        inks={k:tuple(bytes.fromhex(v[1:])) for k,v in f['inks'].items()}
        assert len(inks)==4
        gates.gate(f['rows'],inks)
        tile=gates.render(f['rows'],inks)
        assert tile.tobytes()==Image.open(ROOT/'crates/delve/assets/mossbell'/f'{name}.png').convert('RGBA').tobytes(),name
    print('all eight saved grids reproduce their four-ink PNGs; singleton gates pass')
    idle=Image.open(root/'rooms/idle.png').convert('RGB')
    for prefix in ['root-stage','root-rage']:
        for stage in range(4):
            frame=Image.open(root/'rooms'/f'{prefix}-{stage}.png').convert('RGB')
            for x0 in [204,216]:
                for y in range(118,128):
                    for x in range(x0,x0+8):
                        if idle.getpixel((x,y))!=(11,10,8):
                            assert frame.getpixel((x,y))==idle.getpixel((x,y)),(prefix,stage,'covered centre target')
    print('both centre drop targets stay above all four normal/rage root stages')
    for direction in ['right','left']:
        frame=Image.open(root/'rooms'/f'root-{direction}.png').convert('RGB')
        assert frame.crop((301,118,306,129)).tobytes()==idle.crop((301,118,306,129)).tobytes()
    print('both full-reach root directions stop before the existing plunger wall')
    r.sheet([(root/'rooms'/f'root-{d}.png',f'root {d}, full reach') for d in ['right','left']],
            root/'root-directions-table-1x.png',2)
    proof=Image.new('RGB',(1568,476));d=ImageDraw.Draw(proof)
    for i,direction in enumerate(['right','left']):
        d.text((i*784+8,5),f'root {direction}, full reach',fill='#b0b2a8')
        proof.paste(cell_image(cells(root/'terminal'/f'root-{direction}-cells.ansi'),(0,0,96,28)),(i*784+8,24))
    proof.save(root/'root-directions-table-cells.png')
    checked=0
    for path in sorted((root/'rooms').glob('*.png')):
        old=before/'rooms'/path.name
        if not old.exists(): continue
        diff=ImageChops.difference(Image.open(path).convert('RGB'),Image.open(old).convert('RGB'))
        d=ImageDraw.Draw(diff)
        for box in [(122,118,305,128),(189,66,239,89)]: d.rectangle(box,fill=0)
        assert not diff.getbbox(),f'{path.stem}: changed outside the two tell footprints'
        checked+=1
    print(f'{checked} rooms match the prior review outside the two tell footprints')

def run(a):
    root=a.root
    if a.tells_only:
        spec=importlib.util.spec_from_file_location('boss_art',a.gates)
        gates=importlib.util.module_from_spec(spec);spec.loader.exec_module(gates)
        tell_review(root,gates,a.before)
        return
    states=['idle','root-tell','root-attack','crown-tell','crown-attack','flinch','damage-1','damage-2','defeat']
    r.sheet([(root/'rooms'/(n+'.png'),n) for n in states+['chain','flipper','jackpot']],root/'table-states-1x.png')
    r.sheet([(root/'rooms'/(n+'.png'),n) for n in ['entrance','live-seat-1','live-seat-2','podium']],root/'table-live-1x.png',2)
    for kind in ['root','crown']:
        r.sheet([(root/'rooms'/f'{kind}-stage-{i}.png',f'{kind} stage {i+1}') for i in range(4)],
                root/f'{kind}-table-stages-1x.png',2)
    r.sheet([(root/'rooms'/f'{n}.png',n) for n in ['idle','damage-1','root-attack','crown-attack','live-seat-1','podium']],
            root/'whole-table-1x.png',3)
    r.ansi([root/'terminal'/(n+'-cells.ansi') for n in ['idle','crown-tell','chain','podium']],root/'terminal-review.html')
    tile=Image.new('RGB',(3*180,3*142));d=ImageDraw.Draw(tile)
    for i,name in enumerate(states):
        x=i%3*180+8;y=i//3*142
        d.text((x,y+4),name,fill='#b0b2a8')
        tile.paste(cell_image(cells(root/'terminal'/(name+'-cells.ansi')),(47,5,60,12)),(x,y+24))
    tile.save(root/'keeper-states-cells.png')
    before=Path(a.before)
    tile=Image.new('RGB',(3*220,2*142));d=ImageDraw.Draw(tile)
    for row,base in enumerate([before,root]):
        for col,name in enumerate(['idle','damage-1','damage-2']):
            x=col*220+8;y=row*142
            d.text((x,y+4),('before ' if row==0 else 'after ')+name,fill='#b0b2a8')
            box=(190,43,238,91)
            tile.paste(Image.open(base/'rooms'/(name+'.png')).crop(box),(x,y+24))
            cbox=(47,5,60,12)
            tile.paste(cell_image(cells(base/'terminal'/(name+'-cells.ansi')),cbox),(x+64,y+24))
    tile.save(root/'keeper-before-after-1x-cells.png')
    spec=importlib.util.spec_from_file_location('boss_art',a.gates)
    gates=importlib.util.module_from_spec(spec);spec.loader.exec_module(gates)
    evidence=json.loads((ROOT/'crates/delve/assets/mossbell/native.json').read_text())
    room=Image.open(root/'rooms/idle.png').convert('RGBA').crop((190,43,238,91))
    sprite=Image.open(ROOT/'crates/delve/assets/mossbell/idle.png').convert('RGBA')
    room.putalpha(sprite.getchannel('A'))
    edge,body=gates.separation(room,(11,10,8),evidence['separation']['head'],evidence['separation']['body'])
    assert edge>=15 and body>=15,(edge,body)
    print(f'actual room: outer/floor {edge:.2f} L*, head/body {body:.2f} L*')
    tell_review(root,gates,a.before)
    chain=cells(root/'terminal/chain-cells.ansi')
    row=lambda buf,y: ''.join(buf[x,y][0] for x in range(96))
    assert 'CHAIN X3' in row(chain,3)
    assert re.search(r'1  YOU\s{2,}1375',row(chain,9).replace('▀',' ')),row(chain,9)
    assert '60 S' in row(chain,1)
    podium=cells(root/'terminal/podium-cells.ansi')
    assert '0.00S' not in ''.join(row(podium,y) for y in range(28))
    assert 'YOU WIN 40G + CROWN' in ''.join(row(podium,y) for y in range(28))
    ends=[]
    for y in [5,8,11,14,17]:
        text=row(podium,y)[48:61]
        ends.append(max(i for i,c in enumerate(text) if c!='▀' and not c.isspace()))
    assert len(set(ends))==1,ends
    assert all(f'K{i}' in ''.join(row(chain,y) for y in range(28)) for i in [2,3,4])
    print('podium: YOU WIN 40G + CROWN; SCORE and all values share a terminal right edge; K2-K4 names')
    print('cell glyphs: CHAIN X3, 1  YOU  1375, 60 S; podium has no zero jackpot time')
    # Every pair of the nine body states must differ in the actual Keeper cells.
    bufs=[cells(root/'terminal'/(n+'-cells.ansi')) for n in states]
    counts=[sum(a[x,y]!=b[x,y] for y in range(5,12) for x in range(47,60)) for i,a in enumerate(bufs) for b in bufs[i+1:]]
    assert min(counts)>0
    print(f'nine Keeper states: minimum pair difference {min(counts)} actual cells')
    idle,damage=bufs[0],bufs[6]
    damage_changes=sum(idle[x,y]!=damage[x,y] for y in range(8,11) for x in range(50,57))
    assert damage_changes>=6,damage_changes
    print(f'damage-1 versus idle shell: {damage_changes} actual cells')
    terminal=Image.new('RGB',(1568,952));d=ImageDraw.Draw(terminal)
    for i,name in enumerate(['idle','root-stage-3','crown-stage-3','podium']):
        x=i%2*784+8;y=i//2*476
        d.text((x,y+5),name,fill='#b0b2a8')
        terminal.paste(cell_image(cells(root/'terminal'/f'{name}-cells.ansi'),(0,0,96,28)),(x,y+24))
    terminal.save(root/'terminal-review.png')

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('root',type=Path)
    p.add_argument('--before',type=Path,default=Path('/work/artifacts/boss-art-20261015/game-3c'))
    p.add_argument('--gates',type=Path,default=Path('scripts/boss-art.py'))
    p.add_argument('--tells-only',action='store_true')
    run(p.parse_args())
