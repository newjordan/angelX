#!/usr/bin/env python3
"""Prepare local material samples and keyed pose studies from the accepted guides.
No remote service. The runtime uses independent scenery and live actor anchors.
"""
from pathlib import Path
import argparse
from PIL import Image, ImageDraw
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--plates',type=Path,default=Path('/work/artifacts/art-concepts/upgrade'))
p.add_argument('--output',type=Path,default=Path('cockpit/assets/dungeon/art-v2'))
a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
def crop(name,source,box,size,key=None):
    im=Image.open(a.plates/source).convert('RGBA').crop(box)
    if key:
        pixels=im.load()
        for y in range(im.height):
            for x in range(im.width):
                r,g,b,_=pixels[x,y]
                keep=key(r,g,b,x,y)
                pixels[x,y]=(r,g,b,255 if keep else 0)
    # Hand-bounded silhouettes exclude reference ground/rails and decoration.
    polygons={
        'helios':[(3,99),(25,63),(88,53),(120,34),(177,38),(207,4),(239,17),(262,57),(270,74),(250,80),(233,66),(231,99),(213,137),(201,149),(205,165),(174,169),(151,158),(142,143),(111,132),(89,146),(76,165),(48,165),(46,151),(61,131),(36,105),(7,126)],
        'horse-black':[(12,109),(20,81),(43,64),(63,73),(89,62),(126,22),(160,2),(178,35),(194,55),(183,75),(163,65),(154,103),(149,166),(167,178),(139,182),(121,142),(117,176),(95,177),(96,131),(69,134),(57,174),(32,181),(33,167),(39,122)],
        'horse-bay':[(13,44),(39,33),(76,35),(96,40),(110,20),(140,12),(148,1),(160,22),(177,43),(174,60),(157,65),(155,112),(167,143),(150,150),(135,139),(129,97),(117,102),(106,136),(96,151),(80,144),(90,125),(81,101),(62,104),(55,135),(54,150),(41,153),(34,136),(44,96),(31,86),(18,92),(2,104),(10,48)],
        'cart-gold':[(25,2),(40,2),(42,21),(47,24),(45,35),(52,34),(52,50),(43,59),(39,71),(24,74),(5,74),(0,65),(0,53),(8,47),(8,33),(23,30),(24,24),(21,22)],
        'cart-blue':[(22,2),(36,2),(40,25),(47,28),(45,39),(56,35),(56,52),(43,65),(39,76),(22,78),(3,73),(0,61),(9,49),(10,34),(22,32)],
        'cart-teal':[(22,2),(36,2),(40,25),(47,28),(45,39),(51,38),(51,52),(43,65),(39,78),(22,80),(3,73),(0,61),(9,49),(10,34),(22,32)],
        'barn':[(45,0),(488,0),(519,145),(509,153),(509,281),(16,281),(15,154),(0,149)],
        'college-facade':[(376,0),(486,132),(484,229),(463,236),(470,333),(467,471),(10,471),(8,328),(45,198),(281,198),(280,137)],
        'privy-house':[(94,0),(191,57),(190,243),(22,243),(22,58),(0,60)],
        'engine-boiler':[(29,53),(45,31),(60,18),(93,9),(157,20),(147,47),(155,67),(188,68),(194,106),(217,121),(242,86),(258,110),(257,190),(223,204),(7,204),(6,139),(20,87)],
    }
    if name in polygons:
        mask=Image.new('L',im.size,0);ImageDraw.Draw(mask).polygon(polygons[name],fill=255)
        alpha=im.getchannel('A');im.putalpha(Image.composite(alpha,Image.new('L',im.size,0),mask))
    im=im.resize(size,Image.Resampling.NEAREST)
    if name.startswith('horse-') or name=='ripper':
        pixels=im.load();seen=set()
        for y in range(im.height):
            for x in range(im.width):
                if (x,y) in seen or pixels[x,y][3]==0:continue
                todo=[(x,y)];seen.add((x,y));component=[]
                while todo:
                    q=todo.pop();component.append(q)
                    for dx,dy in [(1,0),(-1,0),(0,1),(0,-1)]:
                        nx,ny=q[0]+dx,q[1]+dy
                        if 0<=nx<im.width and 0<=ny<im.height and (nx,ny) not in seen and pixels[nx,ny][3]:seen.add((nx,ny));todo.append((nx,ny))
                if len(component)<7:
                    for q in component:pixels[q]=(0,0,0,0)
    im.save(a.output/(name+'.png'))
    return im
# Quiet material windows contain no actors or HUD. Each is a reusable tile,
# not a reference scene used as a runtime backdrop.
for i,(x,y) in enumerate([(337,112),(451,126),(653,561),(966,650),(1100,100),(319,703)]):
    crop(f'floor-{i}','integrated_handgonne-upgrade.png',(x,y,x+96,y+96),(48,48))
for i,(x,y) in enumerate([(90,0),(153,0),(245,0),(351,0)]):
    crop(f'wall-{i}','integrated_handgonne-upgrade.png',(x,y,x+76,y+88),(32,32))
crop('wood','02-great-hall-upgrade.png',(518,277,633,362),(64,48))
crop('grass','04-fields-160-upgrade.png',(651,333,804,455),(64,48))
crop('canopy','04-fields-160-upgrade.png',(101,103,333,288),(80,64),lambda r,g,b,x,y: ((x-116)/116)**2+((y-92)/92)**2<1.0)
crop('gravel','02-ranch-upgrade.png',(661,410,808,531),(64,48))
# The navy sheet is a pose study, keyed locally and snapped onto the runtime
# sample grid. Weapons, aim, stride, lights and contacts remain separate layers.
def sheet(r,g,b,x,y):return max(r,g,b)>92 or (r>60 and r>g*1.35) or (g>67 and g>b*1.25)
for name,box,size in [
    ('knight-red',(130,60,310,298),(34,44)),
    ('knight-blue',(486,60,671,298),(34,44)),
    ('knight-green',(852,60,1039,298),(34,44)),
    ('knight-gold',(1202,60,1400,298),(36,44)),
    ('knight-run',(523,402,719,626),(39,44)),
    ('knight-back',(850,402,1045,626),(38,44)),
    ('ghost',(164,723,288,902),(26,38)),
    ('slime',(514,798,692,897),(38,22)),
    ('skeleton',(870,714,1008,923),(30,44)),
    ('imp',(1210,713,1386,920),(31,36)),
]:crop(name,'knights-sheet.png',box,size,sheet)

# ── the hero's family ─────────────────────────────────────────────────────
# The knight studies are brought into the hero the arena draws
# (crates/delve/src/render/overworld/kit/hero.rs): its stone-bank steel
# lit from the upper left with a cream glint and a cool rim, the seat's
# cloth in the arena's own seat inks, and its heater shield, gold-rimmed
# and lit along the top, the seat's field with a pale (bronze-rimmed for
# the gold seat, seen from behind as boards and a strap). Every seat's
# front study is the same knight in its own trim. The studies are painted
# and sampled down, so steel takes the median light of its neighbours and
# falls on ramp steps instead of speckle.
import colorsys

INK = {
    's': 0x192934, 'x': 0x22303a, 'S': 0x2f4a52, 'u': 0x456070, 'U': 0x6e8a8e, 'v': 0x8fa3a8, 'V': 0xb3c2c4, 'W': 0xd5dee0,
    'T': 0xe2cb8b, 'z': 0x639dc2,
    '8': 0x431e0f, '7': 0xa8341f, '0': 0x12414f, '1': 0x2079a6, 'l': 0x3f6428, 'A': 0x6f8738,
    'a': 0x7a4a12, '4': 0xecb64a, '6': 0xfbd070, '5': 0xf7ca58, '9': 0xf4e1cb, 'c': 0xe3d2c3,
    'b': 0x331a0c, 'B': 0x582b13, 'P': 0x684629, 'r': 0x784620,
}
def rgb(k): v = INK[k]; return ((v >> 16) & 255, (v >> 8) & 255, v & 255)
def lum(c): return 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]
STEEL = [rgb(k) for k in 'sxuUvVW']
CREAM = (232, 220, 190)
LEATHER = [rgb(k) for k in 'bBPr']
GOLD = [rgb('a'), rgb('4'), rgb('6')]
# Seat trim as v1 draws it: field and its shadow, the charge, and the rim
# (lit, body, shade). The gold seat's rim goes to bronze so its field stands off it.
SEATS = {
    'red': ((rgb('7'), rgb('8'), (198, 78, 48)), rgb('5'), (rgb('6'), rgb('4'), rgb('a'))),
    'blue': ((rgb('1'), rgb('0'), (78, 146, 196)), rgb('9'), (rgb('6'), rgb('4'), rgb('a'))),
    'green': ((rgb('A'), rgb('l'), (138, 163, 74)), rgb('6'), (rgb('6'), rgb('4'), rgb('a'))),
    'gold': ((rgb('4'), rgb('a'), rgb('6')), rgb('c'), (rgb('4'), rgb('a'), rgb('b'))),
}
def step(ramp, t):
    return ramp[min(len(ramp) - 1, max(0, int(t * len(ramp))))]
def classify(c):
    r, g, b = c
    h, s, v = colorsys.rgb_to_hsv(r / 255, g / 255, b / 255); h *= 360
    if v < 0.2: return 'dark'
    if s < 0.3 or (200 <= h <= 300 and s < 0.5): return 'steel'
    if 180 <= h < 215 and s >= 0.3: return 'rim'
    if h < 14 or h > 335: return 'cloth'
    if 14 <= h < 32: return 'leather' if v < 0.75 else 'gold'
    if 32 <= h < 65: return 'gold'
    return 'steel'

def heater(w, h, field, charge, rim):
    """The hero's shield at `w` x `h`: a flat top, straight sides, curving to
    a point; a rim two samples thick lit along the top and left, shaded on
    the right and below; the field with a shadow down its right; a pale."""
    lit, body, shade = rim
    taper = int(h * 0.6)
    def span(y):
        if y < taper: return 0, w
        k = (y - taper + 1) / (h - taper)
        inset = int(round((w / 2) * k ** 1.3))
        return inset, w - inset
    rows = [span(y) for y in range(h)]
    inside = lambda x, y: 0 <= y < h and rows[y][0] <= x < rows[y][1]
    im = Image.new('RGBA', (w, h)); p = im.load()
    for y in range(h):
        x0, x1 = rows[y]
        for x in range(x0, x1):
            edge = min(x - x0, x1 - 1 - x)
            depth = min(edge, y, *[d for d in range(3) if not inside(x, y + d + 1)] or [3])
            near_right = x1 - 1 - x < 2 or not inside(x, y + 2)
            if y < 2 and x < w * 0.6:
                c = lit
            elif depth < 2 or edge < 2:
                c = shade if near_right and x > w / 2 - 1 else body
            elif x1 - 1 - x == 2:
                c = field[1]
            else:
                c = field[0]
            p[x, y] = c + (255,)
    cx = w // 2 - 1
    for y in range(4, max(5, int(h * 0.48))):
        for x in (cx, cx + 1):
            if inside(x, y): p[x, y] = charge + (255,)
    return im

def restyle(im, seat, box, back=False, plain=None, gamma=0.72):
    """`box` where the study's shield is (redrawn as the hero's); `back`: it
    is seen from behind; `plain`: a box on the tabard whose white cross goes."""
    im = im.convert('RGBA'); px = im.load(); W, H = im.size
    (field, charge, rim) = SEATS[seat]
    kind = {(x, y): classify(px[x, y][:3]) for y in range(H) for x in range(W) if px[x, y][3] > 127}
    # The study was painted and sampled down: take each steel pixel's light
    # as the median of the steel around it, so the ramp falls in grouped
    # steps instead of speckle; edges between materials stay put.
    raw = {p: lum(px[p][:3]) for p, k in kind.items() if k == 'steel'}
    smooth = {}
    for (x, y), L in raw.items():
        near = sorted(raw[(x + dx, y + dy)] for dy in (-1, 0, 1) for dx in (-1, 0, 1) if (x + dx, y + dy) in raw)
        smooth[(x, y)] = near[len(near) // 2] if len(near) >= 5 else L
    lo, hi = min(smooth.values()), max(smooth.values())
    out = Image.new('RGBA', im.size); op = out.load()
    for y in range(H):
        for x in range(W):
            r, g, b, a = px[x, y]
            if a <= 127: continue
            k = classify((r, g, b)); L = lum((r, g, b))
            if k == 'steel':
                t = ((smooth[(x, y)] - lo) / max(1, hi - lo)) ** gamma
                if L > hi - 4: t = 1.0
                c = CREAM if t > 0.985 else step(STEEL, t * 0.999)
            elif k == 'dark': c = rgb('s')
            elif k == 'rim': c = rgb('z')
            elif k == 'cloth': c = field[1] if L < 52 else (field[2] if L > 92 else field[0])
            elif k == 'leather': c = step(LEATHER, min(0.999, L / 150))
            else: c = step(GOLD, min(0.999, max(0, (L - 70) / 140)))
            op[x, y] = c + (255,)
    if plain:
        # The study's white cross and gold thread: anything held between
        # cloth across or down is cloth.
        for _ in range(2):
            for y in range(plain[1], plain[3]):
                for x in range(plain[0], plain[2]):
                    if kind.get((x, y)) not in ('steel', 'gold'):
                        continue
                    cl = lambda q: kind.get(q) == 'cloth'
                    if (cl((x - 1, y)) or cl((x - 2, y))) and (cl((x + 1, y)) or cl((x + 2, y))) \
                            or (cl((x, y - 1)) or cl((x, y - 2))) and (cl((x, y + 1)) or cl((x, y + 2))):
                        kind[(x, y)] = 'cloth'
                        op[x, y] = field[0] + (255,)
    x0, y0, x1, y1 = box
    # The old shield comes off, and the hero's goes on in its place.
    for y in range(y0, y1):
        for x in range(x0, x1):
            if px[x, y][3] > 127 and classify(px[x, y][:3]) in ('cloth', 'gold', 'leather', 'dark'):
                op[x, y] = (0, 0, 0, 0)
    if back:
        # Its back: the boards and the strap, inside the same rim.
        sh = heater(x1 - x0, y1 - y0, (rgb('P'), rgb('B')), rgb('b'), rim)
    else:
        sh = heater(x1 - x0, y1 - y0, field, charge, rim)
    out.alpha_composite(sh, (x0, y0))
    return out


def hero_studies():
    red = Image.open(a.output / 'knight-red.png')
    for seat in SEATS:
        restyle(red, seat, (1, 21, 13, 40), plain=(15, 19, 25, 37)).save(a.output / f'knight-{seat}.png')
    restyle(Image.open(a.output / 'knight-run.png'), 'red', (0, 19, 10, 36)).save(a.output / 'knight-run.png')
    restyle(Image.open(a.output / 'knight-back.png'), 'red', (4, 21, 14, 38), back=True).save(a.output / 'knight-back.png')
hero_studies()
def horse(r,g,b,x,y):
    return (r>g*1.15 and r>b*1.3 and r>47) or (min(r,g,b)>120 and max(r,g,b)-min(r,g,b)<70)
for name,source,box,size,key in [
    ('helios','04-fields-160-upgrade.png',(663,34,934,204),(110,69),horse),
    ('horse-brown','04-fields-160-upgrade.png',(382,548,575,666),(83,51),horse),
    ('horse-grey','04-fields-160-upgrade.png',(1067,153,1241,259),(76,46),lambda r,g,b,x,y:min(r,g,b)>66 and abs(r-g)<38 and abs(g-b)<45),
    ('horse-tan','04-fields-160-upgrade.png',(1100,361,1249,520),(70,74),horse),
    ('horse-black','02-ranch-upgrade.png',(252,420,464,605),(88,77),lambda r,g,b,x,y:max(r,g,b)<165 and abs(r-g)<30 and abs(g-b)<30 and max(r,g,b)>27),
    ('ripper','ripper-07-mid-rip-upgrade.png',(414,211,611,382),(86,74),lambda r,g,b,x,y:(g>r*0.86 and g>b*1.7 and g>55) or (r>b*1.8 and g>b*1.3 and r>85)),
]:crop(name,source,box,size,key)
# Boiler silhouette is bounded by an authored polygon; rail textures are not
# included. The animated legs/steam and exact danger rails are runtime layers.
crop('engine-boiler','engine-03-rails-pulse-upgrade.png',(714,277,972,483),(107,86),lambda r,g,b,x,y:max(r,g,b)>72 and not (b>r and b>g))

# Hall furniture and focal fixtures, reconstructed at their source footprints.
crop('hall-window','02-great-hall-upgrade.png',(173,232,303,300),(64,34))
crop('hall-hearth','02-great-hall-upgrade.png',(634,63,898,185),(132,61))
crop('hall-table','02-great-hall-upgrade.png',(208,490,558,638),(174,74))
crop('hall-bench','02-great-hall-upgrade.png',(228,650,543,691),(158,21))
crop('gas-core','ripper-07-mid-rip-upgrade.png',(624,420,700,493),(34,34),lambda r,g,b,x,y:((x-38)/36)**2+((y-36)/35)**2<1.0)

crop('autumn-canopy','sweep-college-upgrade.png',(170,278,303,405),(64,64),lambda r,g,b,x,y:((x-66)/66)**2+((y-60)/61)**2<1 and r>g*1.03 and r>b*1.6)
crop('grey-canopy','sweep-college-upgrade.png',(1395,362,1535,514),(62,68),lambda r,g,b,x,y:((x-70)/68)**2+((y-68)/68)**2<1 and abs(r-g)<30 and max(r,g,b)>45)
crop('water','sweep-college-upgrade.png',(689,229,753,293),(32,32))
crop('map-grass','sweep-college-upgrade.png',(414,404,543,478),(64,36))
crop('map-road','sweep-college-upgrade.png',(672,359,713,484),(24,64))
crop('college-facade','sweep-college-upgrade.png',(819,154,1315,625),(246,234),lambda r,g,b,x,y:not (140<x<193 and 414<y))
crop('gate-dome','sweep-college-upgrade.png',(376,480,552,625),(88,72),lambda r,g,b,x,y:((x-88)/87)**2+((y-92)/99)**2<1)
crop('barn','02-ranch-upgrade.png',(115,45,634,326),(260,140))
crop('stall','02-ranch-upgrade.png',(751,93,1075,270),(162,89))
crop('horse-bay','02-ranch-upgrade.png',(1100,450,1281,611),(86,76),horse)
crop('horse-ranch-grey','02-ranch-upgrade.png',(911,552,1098,703),(88,74),lambda r,g,b,x,y:min(r,g,b)>68 and abs(r-g)<33 and abs(g-b)<45)
crop('pen','02-ranch-upgrade.png',(280,649,441,748),(80,50))
crop('privy-house','12_Royal_24-upgrade.png',(837,358,1038,610),(86,108))
crop('privy-wall','12_Royal_24-upgrade.png',(73,543,217,656),(72,56))
crop('race-dirt','hippodrome_race-upgrade.png',(364,285,620,389),(128,52))
crop('cart-gold','hippodrome_race-upgrade.png',(742,258,796,335),(30,40),lambda r,g,b,x,y:(y<45 and 10<x<48) or ((x-23)/23)**2+((y-57)/19)**2<1)
crop('cart-blue','hippodrome_race-upgrade.png',(916,417,976,496),(32,40),lambda r,g,b,x,y:(y<48 and 10<x<49) or ((x-27)/26)**2+((y-62)/17)**2<1)
crop('cart-teal','hippodrome_race-upgrade.png',(729,365,781,446),(28,40),lambda r,g,b,x,y:(y<48 and 10<x<49) or ((x-23)/23)**2+((y-62)/17)**2<1)

crop('hall-stone','02-great-hall-upgrade.png',(0,27,83,111),(32,32))
crop('hall-pillar','02-great-hall-upgrade.png',(436,221,512,368),(38,74))

# Calm Helios keeps the forehead/fetlock markings while flame tips extinguish.
im=Image.open(a.output/'helios.png').convert('RGBA');px=im.load()
for y in range(im.height):
    for x in range(im.width):
        r,g,b,alpha=px[x,y]
        plume=x<32 or (y<20 and x<88) or (y<28 and x<77)
        if plume and r>155 and g>105: px[x,y]=(r,g,b,0)
im.save(a.output/'helios-calm.png')
