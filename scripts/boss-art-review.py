#!/usr/bin/env python3
"""Native room comparisons and HTML of the actual half-block/braille buffers.

python scripts/boss-art-review.py OUT --before OUT/before
Each boss has six 1x room views and six actual cell views (old/pilot/new,
both kits). No sheet is wider than 1800 pixels or has over twelve views.
"""
import argparse,html,json
from pathlib import Path
from PIL import Image,ImageDraw
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('output',type=Path)
p.add_argument('--before',type=Path,required=True)
a=p.parse_args()
names=['cinderjaw','the-index','mother-of-spores']

def cells(path):
    raw=json.loads(path.read_text());spans=[]
    for i,c in enumerate(raw):
        fg=','.join(map(str,c['fg']));bg=','.join(map(str,c['bg']))
        spans.append(f'<span style="color:rgb({fg});background:rgb({bg})">{html.escape(c["glyph"])}</span>')
        if i%96==95:spans.append('\n')
    return '<pre>'+''.join(spans)+'</pre>'

style='''<!doctype html><meta charset="utf-8"><style>
*{box-sizing:border-box}body{overflow:hidden;margin:0;background:#080b0e;color:#c2b494;font:12px monospace}
main{display:grid;grid-template-columns:768px 768px;gap:12px;padding:10px}
section{width:768px}p{margin:0;height:24px}pre{font:13px/16px "DejaVu Sans Mono",monospace;margin:0;white-space:pre}
pre span{display:inline-block;width:8px;height:16px}</style><main>'''
for name in names:
    room=Image.new('RGB',(1192,1450),(8,11,14));d=ImageDraw.Draw(room);page=style
    for row,(label,source,variant) in enumerate([('OLD',a.before,'old'),('CURRENT',a.before,'current'),('NEW',a.output,'current')]):
        for kit in [1,2]:
            stem=f'{name}-{variant}-0-v{kit}'
            im=Image.open(source/'terminal'/(stem+'-rgb.png')).convert('RGB')
            x=10 if kit==1 else 408;y=35+row*480
            room.paste(im,(x,y));d.text((x,y-18),f'{name.upper()} / {label} / V{kit} / NATIVE 1X',fill=(194,180,148))
            page+=f'<section><p>{name.upper()} / {label} / V{kit} / ACTUAL CELLS</p>'+cells(source/'terminal'/(stem+'-cells.json'))+'</section>'
    room.save(a.output/(name+'-room-1x.png'))
    (a.output/(name+'-terminal.html')).write_text(page+'</main>')
page=style
for name in names:
    for kit in [1,2]:
        page+=f'<section><p>{name.upper()} / V{kit} / LIVE SIMULATION 133 TICKS</p>'+cells(a.output/'terminal'/f'{name}-current-133-v{kit}-cells.json')+'</section>'
(a.output/'combat-terminal.html').write_text(page+'</main>')
for name in names:
    room=Image.new('RGB',(1192,2410),(8,11,14));d=ImageDraw.Draw(room);page=style
    for age in range(5):
        for kit in [1,2]:
            stem=f'{name}-effects-{age}-v{kit}'
            im=Image.open(a.output/'terminal'/(stem+'-rgb.png')).convert('RGB')
            x=10 if kit==1 else 408;y=35+age*480
            room.paste(im,(x,y));d.text((x,y-18),f'{name.upper()} / EFFECT STAGE {age} / V{kit} / NATIVE 1X',fill=(194,180,148))
            page+=f'<section><p>{name.upper()} / EFFECT STAGE {age} / V{kit} / ACTUAL CELLS</p>'+cells(a.output/'terminal'/(stem+'-cells.json'))+'</section>'
    room.save(a.output/(name+'-effects-room-1x.png'))
    (a.output/(name+'-effects-terminal.html')).write_text(page+'</main>')
# Exact native rows and every authored animation frame, without smoothing.
import runpy
root=Path(__file__).resolve().parents[1]
workflow=runpy.run_path(str(root/'scripts/boss-art.py'))
palette=workflow['palette']()
entries=json.loads((root/'scripts/boss-art-pilots.json').read_text())
sheet=Image.new('RGB',(1260,700),(8,11,14));d=ImageDraw.Draw(sheet)
for i,entry in enumerate(entries):
    name=names[i];d.text((10,12+i*230),name,fill=(194,180,148))
    for j,role in enumerate(['v1','v2']):
        spec=entry['frames'][role][0]
        im=workflow['render'](workflow['signature_rows'](spec),workflow['frame_inks'](spec,palette))
        for factor,x in [(3,30+j*420)]:
            q=im.resize((im.width*factor,im.height*factor),Image.Resampling.NEAREST)
            sheet.paste(q,(x,42+i*230),q)
        d.text((10+j*420,27+i*230),role+' / 3x nearest',fill=(150,165,169))
    im=Image.open(a.before/name/'v2.png').convert('RGBA')
    q=im.resize((im.width*3,im.height*3),Image.Resampling.NEAREST)
    sheet.paste(q,(900,42+i*230),q)
    d.text((900,27+i*230),'v2 current / 3x',fill=(150,165,169))
    fx=Image.new('RGB',(1200,370),(8,11,14));fd=ImageDraw.Draw(fx)
    for row,role in enumerate(['projectile','tell','impact']):
        for j,frame in enumerate(entry['frames'][role]):
            q=workflow['render'](workflow['signature_rows'](frame),workflow['frame_inks'](frame,palette))
            q=q.resize((q.width*3,q.height*3),Image.Resampling.NEAREST)
            fx.paste(q,(20+j*220,40+row*110),q)
            fd.text((20+j*220,20+row*110),role+f' {j}',fill=(194,180,148))
    fx.save(a.output/(name+'-vfx-review.png'))
sheet.save(a.output/'native-review.png')
# Busy-room gate also covers both old-body captures, whose effect banks differ.
for name in names:
    sheet=Image.new('RGB',(1192,1930),(8,11,14));d=ImageDraw.Draw(sheet)
    for row,(label,source,variant) in enumerate([
        ('OLD BODY / CURRENT EFFECTS',a.before,'old'),('CURRENT',a.before,'current'),
        ('OLD BODY / NEW EFFECTS',a.output,'old'),('NEW',a.output,'current')]):
        for kit in [1,2]:
            im=Image.open(source/'terminal'/f'{name}-{variant}-133-v{kit}-rgb.png').convert('RGB')
            x=10 if kit==1 else 408;y=35+row*480
            sheet.paste(im,(x,y));d.text((x,y-18),f'{name.upper()} / {label} / V{kit} / 1X',fill=(194,180,148))
    sheet.save(a.output/(name+'-combat-room-1x.png'))
