import json
import argparse
from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,required=True);args=parser.parse_args();out=args.output
root=Path(__file__).resolve().parents[1]
data=json.loads((out/'review-frames.json').read_text())
font=ImageFont.truetype('/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf',14)
def native(f):
 a=Image.new('RGB',(64,80));a.putdata([tuple(c) for c in f['rgb']]);return a
def cells(f):
 a=Image.new('RGB',(128,160));d=ImageDraw.Draw(a)
 for i,c in enumerate(f['cells']):
  x,y=i%16*8,i//16*16;d.rectangle((x,y,x+7,y+15),fill=tuple(c['bg']));d.text((x,y-2),c['glyph'],font=font,fill=tuple(c['fg']))
 return a
for kit in ['knight','fx']:
 for seat in range(4 if kit=='knight' else 1):
  states=list(dict.fromkeys(f['state'] for f in data if f['kit']==kit));sheet=Image.new('RGB',(1660,20+len(states)*190),'#080b0e');d=ImageDraw.Draw(sheet)
  for row,state in enumerate(states):
   frames=[f for f in data if f['kit']==kit and f['state']==state and f['seat']==seat];y=12+row*190;d.text((8,y),state,font=font,fill='#b0b2a8')
   for i,f in enumerate(frames):sheet.paste(cells(f),(100+i*128,y+20))
  sheet.save(out/f'{kit}-seat{seat+1}-cells-all.png')
  if seat==0:sheet.save(out/f'{kit}-cells-all.png')
# Four seat strips, real buffer and exact graded native together.
sheet=Image.new('RGB',(1200,850),'#080b0e');d=ImageDraw.Draw(sheet)
for seat in range(4):
 frames=[f for f in data if f['kit']=='knight' and f['state']=='idle' and f['seat']==seat]
 for i,f in enumerate(frames):
  x=20+i*280;y=seat*210;d.text((x,y+5),f'seat {seat+1} idle {i}',font=font,fill='#b0b2a8');sheet.paste(native(f),(x,y+35));sheet.paste(cells(f),(x+85,y+25))
sheet.save(out/'seats-real-cells.png')
# Reproducible self-contained viewer: all frames, exact timing, real cell glyphs.
manifest={kit:json.loads((root/'crates/delve/assets/arcade'/kit/'manifest.json').read_text()) for kit in ['knight','fx']}
html='''<!doctype html><meta charset="utf-8"><title>Arcade kit review</title><style>
body{margin:32px;background:#080b0e;color:#b0b2a8;font:14px monospace}button,select{background:#192934;color:#b0b2a8;padding:8px;border:1px solid #456070}canvas{image-rendering:pixelated;background:#000;margin:16px 28px 8px 0;vertical-align:top}p{max-width:850px;line-height:1.5}.row{display:flex;align-items:flex-start;gap:24px}h1{font-size:20px}</style>
<h1>Salvage knight & arcade effects</h1><p>Native stage at 1× · actor grade · actual cockpit half-block / braille buffer</p>
<select id="kit"><option>knight</option><option>fx</option></select> <select id="state"></select> <select id="seat"><option value="0">Seat 1</option><option value="1">Seat 2</option><option value="2">Seat 3</option><option value="3">Seat 4</option></select> <button id="pause">Pause</button> <button id="step">Next frame</button><p id="label"></p>
<div class="row"><div>1× native 384×224<br><canvas id="native" width="384" height="224"></canvas></div><div>Real 96×28 cells (central 16×10 crop)<br><canvas id="cells" width="128" height="160"></canvas></div></div>
<div>3× native crop<br><canvas id="zoom" width="192" height="240"></canvas></div><p>All states can repeat here for inspection, including attacks and defeats. Source timings are retained. No simulation or collision geometry is used.</p><div class="row"><figure>Run<br><img src="knight-run.gif"></figure><figure>Attack<br><img src="knight-attack.gif"></figure><figure>Defeat<br><img src="knight-defeat.gif"></figure></div><script>
window.__angel={status:'playing'};const data=DATA, manifests=MANIFEST;const $=id=>document.getElementById(id);let start=performance.now(),paused=false,index=0;
function reset(){const m=manifests[$('kit').value];$('state').innerHTML=Object.keys(m.states).map(s=>`<option>${s}</option>`).join('');start=performance.now();index=0;}reset();$('kit').onchange=reset;$('state').onchange=()=>{start=performance.now();index=0};$('pause').onclick=()=>{paused=!paused;$('pause').textContent=paused?'Play':'Pause';start=performance.now()};$('step').onclick=()=>{paused=true;$('pause').textContent='Play';index++};
const nc=$('native').getContext('2d'),cc=$('cells').getContext('2d'),zc=$('zoom').getContext('2d');let off=document.createElement('canvas');off.width=64;off.height=80;const oc=off.getContext('2d');
function draw(now){const kit=$('kit').value,state=$('state').value,seat=kit==='fx'?0:+$('seat').value,anim=manifests[kit].states[state],frames=data.filter(f=>f.kit===kit&&f.state===state&&f.seat===seat);if(!paused){let time=(now-start)%anim.durations_ms.reduce((a,b)=>a+b,0);index=0;while(time>=anim.durations_ms[index]){time-=anim.durations_ms[index++];}}index%=frames.length;const f=frames[index];let id=oc.createImageData(64,80);f.rgb.forEach((p,i)=>{id.data.set([...p,255],i*4)});oc.putImageData(id,0,0);nc.fillStyle='#000';nc.fillRect(0,0,384,224);nc.drawImage(off,160,112);zc.imageSmoothingEnabled=false;zc.drawImage(off,0,0,192,240);cc.textBaseline='top';cc.font='14px "JetBrainsMono Nerd Font",monospace';f.cells.forEach((c,i)=>{let x=i%16*8,y=Math.floor(i/16)*16;cc.fillStyle=`rgb(${c.bg})`;cc.fillRect(x,y,8,16);cc.fillStyle=`rgb(${c.fg})`;cc.fillText(c.glyph,x,y-2)});$('label').textContent=`${kit} / ${state} / frame ${index+1} of ${frames.length} / seat ${seat+1}`;requestAnimationFrame(draw)}requestAnimationFrame(draw);
</script>'''.replace('DATA',json.dumps(data,separators=(',',':'))).replace('MANIFEST',json.dumps(manifest,separators=(',',':')))
(out/'review.html').write_text(html)
