#!/usr/bin/env python3
"""Make native-sized Cinder room sheets and display the actual ANSI cell buffer."""
from pathlib import Path
from PIL import Image, ImageDraw
import argparse, html, re

def sheet(paths, dest, cols=3, size=(384,224)):
    paths=list(paths)
    assert len(paths)<=12
    w,h=size; tile_w=w+16;tile_h=h+28
    out=Image.new('RGB',(cols*tile_w,((len(paths)+cols-1)//cols)*tile_h))
    d=ImageDraw.Draw(out)
    for n,(p,label) in enumerate(paths):
        im=Image.open(p).convert('RGBA').resize(size,Image.Resampling.NEAREST)
        x=n%cols*tile_w+8;y=n//cols*tile_h
        d.text((x,y+6),label,fill='#b0b2a8');out.paste(im,(x,y+24),im)
    out.save(dest)

def ansi(files,dest):
    assert len(files)<=12
    parts=['''<!doctype html><meta charset="utf-8"><style>
body{margin:0;background:#000;color:#b0b2a8;font:14px monospace;display:grid;grid-template-columns:768px 768px}
.screen{position:relative;width:768px;height:448px}span{position:absolute;width:8px;height:16px;overflow:hidden;font:14px/16px "DejaVu Sans Mono",monospace}
h3{height:22px;margin:4px 8px;font:14px monospace}::-webkit-scrollbar{display:none}</style>''']
    for p in files:
        parts.append('<section><h3>'+html.escape(p.parent.name+'/'+p.stem)+'</h3><div class="screen">')
        for m in re.finditer(r'\x1b\[(\d+);(\d+)H(.*?)(?=\x1b\[\d+;\d+H|$)',p.read_text(),re.S):
            y,x=int(m[1])-1,int(m[2])-1;t=m[3];fg=bg='#000'
            for code,r,g,b in re.findall(r'\x1b\[(38|48);2;(\d+);(\d+);(\d+)m',t):
                c=f'rgb({r},{g},{b})'
                if code=='38':fg=c
                else:bg=c
            char=re.sub(r'\x1b\[[0-9;?]*[A-Za-z]','',t)
            if char=='▀':style=f'background:linear-gradient(to bottom,{fg} 50%,{bg} 50%);';char=''
            else:style=f'color:{fg};background:{bg};'
            parts.append(f'<span style="left:{x*8}px;top:{y*16}px;{style}">{html.escape(char)}</span>')
        parts.append('</div></section>')
    dest.write_text('\n'.join(parts))

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('root',type=Path);p.add_argument('phase',choices=['before','after'])
    a=p.parse_args();r=a.root;phase=a.phase
    moments=sorted((r/phase/'moments').glob('*.png'))
    for start in range(0,len(moments),12):
        sheet([(f,f.stem) for f in moments[start:start+12]],r/f'{phase}-room-states-{start//12+1}-1x.png')
    live=[f for f in sorted((r/phase).glob('*.png')) if f.stem.startswith(('entrance','tick-','podium'))]
    for start in range(0,len(live),12):
        sheet([(f,f.stem) for f in live[start:start+12]],r/f'{phase}-rooms-{start//12+1}-1x.png')
    cells=sorted((r/f'{phase}-terminal').glob('*-cells.ansi'))
    for start in range(0,len(cells),12):
        ansi(cells[start:start+12],r/f'{phase}-cells-{start//12+1}.html')
    if phase=='after':
        sheet([(r/'before'/'entrance.png','Before: lab'),(r/'after'/'entrance.png','After: lab'),
               (r/'before'/'moments/cracks-24.png','Before: room'),(r/'after'/'moments/cracks-24.png','After: room')],r/'room-before-after-1x.png',2)
        assets=Path(__file__).resolve().parents[1]/'crates/delve/assets/cinder'
        sheet([(assets/(f+'.png'),str(n)) for f,n in [('crock',0),('crock-crack-1',12),('crock-crack-2',24),('crock-crack-3',36)]],r/'crack-stages-1x.png',4,(80,80))
        ansi([r/f'{p}-terminal'/f'{s}-cells.ansi' for s in ['cinder-podium','cinder-tell','hit-chain-2'] for p in ['before','after']],r/'cell-before-after.html')
        ansi([r/'after-terminal'/f'{s}-cells.ansi' for s in ['cracks-00','cracks-12','cracks-24','cracks-36']],r/'crack-cells.html')
