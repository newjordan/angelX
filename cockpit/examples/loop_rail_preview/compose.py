"""Offline proof only: rasterize a labelled cockpit fixture behind production rail PNGs."""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont
import json, os, subprocess, tempfile

out = Path(os.environ.get('ANGEL_RAIL_OUT') or Path(tempfile.gettempdir()) / 'loop-rail' / 'shots')
g = json.loads((out / 'geometry.json').read_text())
progress = json.loads((out / 'progress.json').read_text())
font_path = os.environ.get('ANGEL_RAIL_FONT') or subprocess.check_output(['fc-match', '-f', '%{file}', 'monospace'], text=True).strip()
font = ImageFont.truetype(font_path, 14)
small = ImageFont.truetype(font_path, 12)
bg = Image.new('RGBA', (g['width'], g['height']), (14, 16, 20, 255))
d = ImageDraw.Draw(bg)
# Live layout's reserved band: 2 columns, 1 row; content begins at (20,20).
edge, ink, dim, warm = (51, 55, 62), (166, 171, 180), (106, 113, 124), (196, 134, 92)
d.rectangle((0, 0, 1499, 879), outline=(40, 43, 46))
for box in [(20,20,1479,119), (20,119,979,799), (979,119,1479,799), (20,799,1479,859)]:
    d.rectangle(box, outline=edge)
d.text((40,36), 'angelX  0.2.6', font=font, fill=ink)
d.text((1090,36), 'loop-rail · offscreen fixture', font=small, fill=dim)
for y, line in enumerate((out / 'ticker.txt').read_text().splitlines()):
    if y >= 3 and line.strip(): d.text((0,y*20+2), line, font=font, fill=warm)
lines = [
    ('› /loop ship the parser rewrite behind a feature flag', warm),
    ('', dim),
    ('The fourth measured iteration is complete.', ink),
    ('The rail observes the loop; it never changes its decisions.', dim),
    ('', dim),
    ('Changed src/parser.rs and kept the compatibility path intact.', ink),
    ('Focused verifier: 42 passed · 0 failed', dim),
    ('', dim),
    ('tool  cargo test --profile fast parser::tests', dim),
    ('  checking the next candidate on the same protocol…', ink),
    ('', dim),
    ('Waiting on the next iteration. The border stays out of text.', dim),
]
for i, (line, color) in enumerate(lines): d.text((40,155+i*27), line, font=font, fill=color)
d.text((1000,145), 'STAGE  /  loop read-only preview', font=font, fill=dim)
d.text((1000,185), 'knight racer · twin engines', font=font, fill=ink)
d.text((1000,215), '4 / 12 laps   ·   01:24', font=font, fill=warm)
d.text((40,818), '› next candidate', font=font, fill=ink)
d.text((1000,818), 'CPU fixture · not a live screenshot', font=small, fill=dim)
bg.save(out / 'fixture.png')
# Every frame, not a sparse sample: 240 images reviewed on 20 enlarged sheets.
all_frames=[]
for i, rail in enumerate(sorted(out.glob('rail-*.png'))):
    im=Image.alpha_composite(bg, Image.open(rail).convert('RGBA'))
    im.save(out / f'frame-{i:03}.png')
    all_frames.append(im.convert('RGB'))
for start in range(0, len(all_frames), 12):
    sheet=Image.new('RGB',(1200, 3*240),(18,20,24))
    sd=ImageDraw.Draw(sheet)
    for n in range(start,min(start+12,len(all_frames))):
        crop=Image.open(out / f'toy-{n:03}.png').convert('RGBA')
        tile=Image.new('RGBA',(128,128),(14,16,20,255))
        tile.alpha_composite(crop)
        # 88 square around the pose centre, nearest-neighbour 3x.
        tile=tile.crop((20,20,108,108)).resize((220,220),Image.Resampling.NEAREST)
        x,y=(n-start)%4*300,(n-start)//4*240
        sheet.paste(tile.convert('RGB'),(x+40,y+20))
        sd.text((x+10,y+2),f'{n:03}  lap {progress[n]:.3f}',font=small,fill=ink)
    sheet.save(out/f'review-{start//12:02}.png')
for n in [0,45,76,120,165,196,239]: all_frames[n].save(out/f'shot-{n:03}.png')
# Keep GIF native size; no tiny downscaled racer masquerading as a real view.
all_frames[0].save(out/'full-lap.gif',save_all=True,append_images=all_frames[1:],duration=33,loop=0,optimize=False)
compare=Image.new('RGB',(840,270),(14,16,20))
cd=ImageDraw.Draw(compare)
cd.text((10,8),'Hand pixels (production) · 1x / 4x',font=small,fill=ink)
cd.text((440,8),'Race raster · 64px / actual 20px band',font=small,fill=ink)
hand=Image.open(out/'toy-020.png').convert('RGBA')
compare.paste(hand,(8,42),hand)
crop=hand.crop((18,52,102,78)).resize((336,104),Image.Resampling.NEAREST)
compare.paste(crop,(8,160),crop)
for i,n in enumerate([0,4,8,12]):
    im=Image.open(out/f'raster-turn-{n:02}.png').convert('RGBA')
    compare.paste(im,(440+i*95,45),im)
    tiny=im.resize((20,20),Image.Resampling.LANCZOS)
    compare.paste(tiny,(460+i*95,145),tiny)
cd.text((440,205),'Full cinematic shrunk to the rail loses',font=small,fill=dim)
cd.text((440,224),'cables / rider. Pick: hand pixels.',font=small,fill=dim)
compare.save(out/'model-comparison.png')
print(f'Composed {len(all_frames)} frames, 20 all-frame sheets, native full-lap.gif')
