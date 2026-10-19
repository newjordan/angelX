#!/usr/bin/env python3
"""Compose native room and actual-terminal review sheets; no interpolated pixels."""
from pathlib import Path
import importlib.util,sys
from PIL import Image
spec=importlib.util.spec_from_file_location('review',Path(__file__).with_name('cinder-review.py'));review=importlib.util.module_from_spec(spec);spec.loader.exec_module(review)
root=Path(sys.argv[1]);rooms=root/'rooms'
for names,dest in [(['entrance','bottom','middle','climb','jump','fall','land','hit'],'room-movement-1x'),(['idle','drop-tell','drop','sweep-tell','sweep','flinch','damage-1','damage-2','defeat','recover','podium'],'room-states-1x')]:
    review.sheet([(rooms/(n+'.png'),n) for n in names],root/(dest+'.png'))
for t in [120,360,480]:
    if (rooms/f'live-{t}-seat-1.png').exists():
        review.sheet([(rooms/f'live-{t}-seat-{i}.png',f'{t//30} S, seat {i}') for i in range(1,5)],root/f'live-{t}-1x.png',4)
# Stitched from contiguous native viewport strips with a common seed and tick.
# No sprite or scenery scaling; the room sheets above retain the complete HUD.
whole=Image.new('RGB',(254,552))
for cam in [0,144,288,408]:
    im=Image.open(rooms/f'tower-{cam}.png');whole.paste(im.crop((12,38,266,182)),(0,cam))
whole.save(root/'whole-tower-1x.png')
if(root/'terminal').exists():
    for names,dest in [(['bottom','middle','climb','jump','land','idle'],'terminal-movement'),(['drop-tell','drop','sweep-tell','sweep','flinch','damage-1','damage-2','defeat','recover','podium'],'terminal-review')]:
        review.ansi([root/'terminal'/(n+'-cells.ansi') for n in names],root/(dest+'.html'))
if (root/'terminal').exists():
    import re
    spec=importlib.util.spec_from_file_location('boss',Path(__file__).with_name('boss-art.py'));boss=importlib.util.module_from_spec(spec);spec.loader.exec_module(boss)
    def cells(name):
        s=(root/'terminal'/(name+'-cells.ansi')).read_text()
        return {(int(m[2])-1,int(m[1])-1):tuple(tuple(map(int,c)) for c in re.findall(r'\x1b\[(?:38|48);2;(\d+);(\d+);(\d+)m',m[3])) for m in re.finditer(r'\x1b\[(\d+);(\d+)H(.*?)(?=\x1b\[\d+;\d+H|$)',s,re.S)}
    log=[]
    for a,b in [('idle','damage-1'),('damage-1','damage-2'),('drop-tell','sweep-tell')]:
        aa,bb=cells(a),cells(b)
        changed=[k for k in aa if 35<=k[0]<=60 and 7<=k[1]<=19 and max((abs(boss.lightness(x)-boss.lightness(y)) for x,y in zip(aa[k],bb[k])),default=0)>=7.5]
        assert len(changed)>=15,(a,b,len(changed))
        log.append(f'{a} -> {b}: {len(changed)} actual boss-area cells change by >=7.5 L* (gate 15)')
    (root/'cell-gates.txt').write_text('\n'.join(log)+'\n')
    if (root/'terminal/drop-tell-0-cells.ansi').exists():
        review.ansi([root/'terminal'/f'{attack}-tell-{n}-cells.ansi' for attack in ['drop','sweep'] for n in range(4)],root/'tell-stages-cells.html')
        for attack in ['drop','sweep']:
            for n in range(3):
                aa,bb=cells(f'{attack}-tell-{n}'),cells(f'{attack}-tell-{n+1}')
                changed=[k for k in aa if 35<=k[0]<=60 and 7<=k[1]<=19 and max((abs(boss.lightness(x)-boss.lightness(y)) for x,y in zip(aa[k],bb[k])),default=0)>=7.5]
                assert len(changed)>=15,(attack,n,len(changed))
                log.append(f'{attack} stage {n}->{n+1}: {len(changed)} keeper-area cells change by >=7.5 L* (gate 15)')
                # Isolate the marks left of the figures; pose motion cannot pass this gate.
                mark_changed=[k for k in aa if 4<=k[0]<=34 and 6<=k[1]<=18
                              and aa[k]!=bb[k]]
                assert mark_changed,(attack,n,'danger marks did not change in actual cells')
                log.append(f'{attack} stage {n}->{n+1}: {len(mark_changed)} danger-mark cells differ outside keeper')
        (root/'cell-gates.txt').write_text('\n'.join(log)+'\n')
        review.ansi([root/'terminal'/f'{n}-cells.ansi' for n in ['idle','drop-overlap','damage-1','damage-2','land-dust','climb-dust']],root/'keeper-dust-cells.html')
        review.ansi([root/'terminal'/f'{n}-cells.ansi' for n in ['standings-first','standings-third','standings-fourth','standings-tie','podium']],root/'standings-podium-cells.html')
        review.sheet([(rooms/f'{attack}-tell-{n}.png',f'{attack} stage {n+1}') for attack in ['drop','sweep'] for n in range(4)],root/'tell-stages-1x.png',4)
        review.sheet([(rooms/f'{n}.png',n) for n in ['land-dust','climb-dust','drop-overlap','standings-first','standings-third','standings-fourth','standings-tie','podium']],root/'dust-standings-podium-1x.png',4)
