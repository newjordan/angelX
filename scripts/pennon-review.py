#!/usr/bin/env python3
"""Native room sheets, contiguous rampart and actual half-block/braille cell gates."""
from pathlib import Path
import importlib.util,re,sys
from PIL import Image,ImageDraw
spec=importlib.util.spec_from_file_location('review',Path(__file__).with_name('cinder-review.py'));review=importlib.util.module_from_spec(spec);spec.loader.exec_module(review)
spec=importlib.util.spec_from_file_location('boss',Path(__file__).with_name('boss-art.py'));boss=importlib.util.module_from_spec(spec);spec.loader.exec_module(boss)
root=Path(sys.argv[1]);rooms=root/'rooms';terminal=root/'terminal'
for names,dest in [(['entrance','start','sentry','jump','fall','land','hit','duck','aim-up','aim-down','shoot'],'movement-1x'),(['idle','drop-tell','drop','hook-tell','hook','flinch','damage-1','damage-2','defeat','recover','podium'],'states-1x')]:review.sheet([(rooms/(n+'.png'),n) for n in names],root/(dest+'.png'))
whole=Image.new('RGB',(1200,144))
for cam in [0,254,508,762,946]:whole.paste(Image.open(rooms/f'rampart-{cam}.png').crop((12,38,266,182)),(cam,0))
whole.save(root/'whole-rampart-1x.png');whole.save(root/'whole-tower-1x.png')
for tick in [120,360,600]:
    if (rooms/f'live-{tick}-seat-1.png').exists():review.sheet([(rooms/f'live-{tick}-seat-{i}.png',f'{tick//30} S, seat {i}') for i in range(1,5)],root/f'live-{tick}-1x.png',4)
for kinds,dest in [(['drop','hook'],'attacks-frames-1x'),(['flinch','land'],'contact-frames-1x')]:
    review.sheet([(rooms/f'fx-{kind}-{n}.png',f'{kind} frame {n+1}') for kind in kinds for n in range(4)],root/(dest+'.png'),4)
if terminal.exists():
    for names,dest in [(['start','sentry','jump','duck','aim-up','shoot'],'terminal-movement'),(['idle','drop-tell','hook-tell','damage-1','damage-2','defeat'],'terminal-review'),(['drop','hook','flinch','recover','overlap','podium'],'terminal-effects'),(['standings-fourth','standings-tie','podium'],'terminal-standings'),([f'{a}-tell-{n}' for a in ['drop','hook'] for n in range(4)],'terminal-tells')]:review.ansi([terminal/(n+'-cells.ansi') for n in names],root/(dest+'.html'))
    for kind in ['drop','hook','flinch','land']:review.ansi([terminal/f'fx-{kind}-{n}-cells.ansi' for n in range(4)],root/f'terminal-fx-{kind}.html')
    def cells(name):
        s=(terminal/(name+'-cells.ansi')).read_text()
        return {(int(m[2])-1,int(m[1])-1):tuple(tuple(map(int,c)) for c in re.findall(r'\x1b\[(?:38|48);2;(\d+);(\d+);(\d+)m',m[3])) for m in re.finditer(r'\x1b\[(\d+);(\d+)H(.*?)(?=\x1b\[\d+;\d+H|$)',s,re.S)}
    log=[]
    for a,b in [('idle','damage-1'),('damage-1','damage-2')]+[(f'{a}-tell-{n}',f'{a}-tell-{n+1}') for a in ['drop','hook'] for n in range(3)]:
        aa,bb=cells(a),cells(b)
        changed=[k for k in aa if 41<=k[0]<=58 and 7<=k[1]<=19 and max((abs(boss.lightness(x)-boss.lightness(y)) for x,y in zip(aa[k],bb[k])),default=0)>=7.5]
        log.append(f'{a} -> {b}: {len(changed)} actual boss-area cells differ by >=7.5 L* (minimum 15)')
        assert len(changed)>=15,(a,b,len(changed))
        if 'tell-' in a:
            marks=[k for k in aa if 4<=k[0]<=39 and 6<=k[1]<=19 and aa[k]!=bb[k]]
            assert marks,(a,b,'danger marks did not change outside boss')
            log.append(f'  {len(marks)} danger-mark cells also differ outside the captain')
    (root/'cell-gates.txt').write_text('\n'.join(log)+'\n')
    print('\n'.join(log))
