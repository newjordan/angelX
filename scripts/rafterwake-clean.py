#!/usr/bin/env python3
"""Hand-cluster the hoist keeper; keep knights, props and rules byte-for-byte.
Usage: python scripts/rafterwake-clean.py REFERENCE.png REVIEW_DIRECTORY
"""
from pathlib import Path
from collections import Counter
import importlib.util
import json
import sys
from PIL import Image, ImageDraw


def module(name, file):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(file))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


art = module('rafter', 'rafterwake-art.py')
boss = art.boss
INK = art.INK
WARM, COOL = art.RAMPS
# Local recipe inks leave the global ink palette and other games unchanged.
DETAIL = {'a':(224,150,44), 'b':(60,90,102), 'c':(95,127,138),
          'd':(90,54,24), 'e':(217,163,90), 'f':(111,127,136),
          'g':(58,71,80), 'h':(42,26,16), 'j':(138,127,110),
          'l':(95,87,76)}
INK.update(DETAIL)
art.INKS.update(DETAIL)
WARM, COOL = "Idoe", "gbcf"
POSES = ['idle', 'walk-1', 'walk-2', 'drop-tell', 'drop', 'sweep-tell',
         'sweep', 'flinch', 'damage-1', 'damage-2', 'defeat', 'recover']


def clustered(rows, features=()):
    """Merge sampled/sliver islands without changing the authored silhouette.

    Every remaining material component must contain a solid native 4x4 block.
    Signature pixels are applied afterwards, never repaired into the materials.
    """
    protected = boss.linear_pixels(features, rows)
    cells = list(map(list, rows))
    while True:
        bad = list(boss.region_errors([''.join(r) for r in cells], [WARM, COOL], linear_features=features))
        if not bad:
            return [''.join(r) for r in cells]
        before = [''.join(r) for r in cells]
        for k, part in bad[:1]:
            part = part - protected
            ns = Counter(cells[y][x] for px, py in part for x, y in boss.neighbours(px, py)
                         if 0 <= x < 64 and 0 <= y < 80 and (x, y) not in part
                         and cells[y][x] != '.')
            ramp = WARM if k in WARM else COOL
            same = {c: n for c, n in ns.items() if c in ramp and c != k}
            choices = same or {c: n for c, n in ns.items() if c != k}
            assert choices, (k, part)
            use = max(choices, key=choices.get)
            for x, y in part:
                cells[y][x] = use
        assert before != [''.join(r) for r in cells], 'material repair stalled'


def keeper(pose, stage=0):
    im = Image.new('RGBA', (64, 80))
    d = ImageDraw.Draw(im)

    def poly(points, k):
        d.polygon(points, fill=INK[k])

    def rect(box, k):
        d.rectangle(box, fill=INK[k])

    def capsule(points, width, k):
        d.line(points, fill=INK[k], width=width)
        r = width // 2
        for x, y in points:
            d.ellipse((x-r, y-r, x+r, y+r), fill=INK[k])

    def arm(points):
        capsule(points, 14, WARM[0])
        capsule([(x-1, y-1) for x, y in points], 10, WARM[1])
        capsule([(x-2, y-2) for x, y in points], 6, WARM[2])
        x, y = points[-1]
        rect((x-4, y-4, x-1, y-1), WARM[3])

    crouch = 11 if pose.startswith('sweep') or pose == 'recover' else 0
    if pose.startswith('sweep-tell'):crouch -= stage*3
    lean = -3 if pose == 'flinch' else 0
    if pose == 'defeat':
        # The apron slumps to one side; the visor remains separate from the body.
        poly([(8,59),(15,49),(34,50),(44,63),(42,76),(8,76)], WARM[0])
        poly([(11,60),(17,53),(32,54),(38,65),(35,72),(11,72)], WARM[1])
        poly([(14,59),(18,55),(29,55),(33,64),(14,64)], WARM[2])
        rect((17,55,23,59), WARM[3])
        head = (28,43)
    else:
        # Wide boots, separate knees and a clean apron hem.
        stride = 4 if pose == 'walk-1' else -3 if pose == 'walk-2' else 0
        for x, yy in [(15-stride,65),(38+stride,65)]:
            poly([(x,yy),(x+14,yy),(x+16,77),(x-5,77),(x-5,72)], COOL[0])
            poly([(x+2,yy+1),(x+12,yy+1),(x+12,73),(x-2,73),(x-2,70)], COOL[1])
            rect((x+2,yy+2,x+8,yy+6), COOL[2])
            rect((x-2,yy+7,x+3,yy+10), COOL[3])
        # Rear arm: the hoist hand climbs through four distinct silhouettes.
        if pose.startswith('drop-tell'):
            n = stage
            paths = [[(45,44),(54,35),(52,24)], [(45,43),(54,27),(50,15)],
                     [(45,43),(53,21),(45,9)], [(45,42),(48,18),(36,7)]]
            arm(paths[n])
            # A broad iron counterweight crowns the hoist fist.
            xx, yy = paths[n][-1]
            poly([(xx-7,yy-5),(xx+5,yy-5),(xx+9,yy),(xx+5,yy+5),
                  (xx-7,yy+5),(xx-9,yy)], COOL[0])
            rect((xx-5,yy-4,xx+3,yy+1), COOL[2])
        elif pose == 'drop':
            arm([(45,43),(50,21),(36,7)])
            poly([(29,2),(41,2),(45,7),(41,12),(29,12),(27,7)], COOL[0])
            rect((30,3,38,8), COOL[2])
        elif pose.startswith('sweep-tell'):
            paths = [[(46,48),(52,53),(49,62)], [(46,51),(55,53),(53,61)],
                     [(46,53),(57,49),(57,57)], [(46,54),(58,43),(60,49)]]
            arm(paths[stage])
            xx, yy = paths[stage][-1]

        elif pose == 'sweep':
            arm([(45,52),(30,61),(12,65)])

        else:
            arm([(46,43+crouch),(52,52+crouch),(51,62+crouch)])
        top = 34 + crouch
        poly([(20+lean,top),(43+lean,top-1),(50,top+10),(49,66),(42,70),
              (22,69),(15,62),(15,top+12)], WARM[0])
        poly([(21+lean,top+3),(41,top+3),(46,top+11),(44,64),(23,64),
              (19,59),(18,top+13)], WARM[1])
        poly([(22+lean,top+5),(39,top+5),(43,top+12),(40,top+25),
              (24,top+25),(20,top+17)], WARM[2])
        rect((23+lean,top+6,30+lean,top+10), WARM[3])
        # Wide shaped shoulder and belt planes separate limbs from the apron.
        poly([(15,top),(23,top+1),(25,top+6),(19,top+11),(11,top+9),(10,top+4)], COOL[0])
        rect((14,top+2,20,top+6), COOL[2])
        if not pose.startswith('sweep'):
            poly([(20,62),(44,62),(43,67),(24,68),(20,66)], COOL[1])
            rect((29,63,35,66), COOL[2])
        # The bracing hand leaves an open elbow and readable torso.
        if pose == 'drop':
            arm([(18,44),(11,51),(18,58)])
        elif pose == 'flinch':
            arm([(18,40),(10,33),(9,25)])
        elif pose.startswith('sweep'):
            arm([(18,48),(12,60),(19,66)])
        else:
            arm([(18,42+crouch),(10,51+crouch),(12,62+crouch)])
        head = (13+lean, 15 if pose.startswith('drop') else 21-stage*3 if pose.startswith('sweep-tell') else 21 if pose=='sweep' else 12+crouch)
    hx, hy = head
    # Helmet: broad domed crown, overhanging visor and dark face plane.
    poly([(hx+6,hy),(hx+20,hy),(hx+28,hy+6),(hx+31,hy+20),
          (hx+26,hy+29),(hx+12,hy+31),(hx+3,hy+26),(hx,hy+17),
          (hx+3,hy+7)], COOL[0])
    poly([(hx+7,hy+3),(hx+19,hy+3),(hx+25,hy+8),(hx+26,hy+17),
          (hx+20,hy+23),(hx+5,hy+22),(hx+3,hy+15)], COOL[1])
    poly([(hx+8,hy+4),(hx+18,hy+4),(hx+22,hy+8),(hx+22,hy+13),
          (hx+6,hy+13),(hx+4,hy+11)], COOL[2])
    rect((hx+10,hy+4,hx+16,hy+7), COOL[3])
    # A flush rounded faceplate follows the dome, without a projecting brim.
    poly([(hx+5,hy+15),(hx+20,hy+14),(hx+25,hy+18),(hx+24,hy+25),
          (hx+18,hy+29),(hx+9,hy+28),(hx+4,hy+23)], COOL[0])
    poly([(hx+7,hy+16),(hx+19,hy+16),(hx+22,hy+19),(hx+21,hy+24),
          (hx+17,hy+26),(hx+8,hy+25)], COOL[1])
    _, rows = boss.snap(im, art.INKS)
    rows = clustered(rows)
    boss.region_gate(rows, [WARM, COOL])
    # Broad identity masses are clustered independently before thin strokes.
    cells = list(map(list, rows))
    def paste_mass(box, k):
        x0,y0,x1,y1=box
        for yy in range(max(0,y0),min(80,y1+1)):
            for xx in range(max(0,x0),min(64,x1+1)):
                cells[yy][xx]=k
    # Swept 6x15 crest, attached to the rear of the crown.
    cx,cy=hx+21,max(0,hy-12)
    crest=Image.new('1',(64,80))
    ImageDraw.Draw(crest).polygon([(cx+5,cy),(cx+10,cy),(cx+5,cy+14),(cx,cy+14)],fill=1)
    for yy in range(80):
        for xx in range(64):
            if crest.getpixel((xx,yy)):cells[yy][xx]='b'
    # Near shoulder cap, eight by six; the body remains a broad warm mass.
    shoulder_y = 32+crouch if pose.startswith('sweep') or pose=='recover' else 35
    if pose!='defeat':
        paste_mass((10,shoulder_y,17,shoulder_y+5),COOL[2])
    rows=clustered([''.join(r) for r in cells])
    features=[]
    def stroke(name,path,width,k):
        nonlocal rows
        mask=Image.new('1',(64,80));ImageDraw.Draw(mask).line(path,fill=1,width=width)
        points={(xx,yy) for yy in range(80) for xx in range(64) if mask.getpixel((xx,yy))}
        underlay=[[xx,yy,rows[yy][xx]] for xx,yy in sorted(points)]
        rows=[''.join(k if (xx,yy) in points else old for xx,old in enumerate(row)) for yy,row in enumerate(rows)]
        features.append(dict(tag='LINEAR',name=name,path=path,width=width,ink=k,pixels=sorted(points),underlay=underlay))
    stroke('crest swept light',[(cx+8,cy),(cx+3,cy+14)],3,'c')
    for offset in [2,5,8,11]:
        left=cx+5-offset//3
        stroke('crest rib',[(left,cy+offset),(left+5,cy+offset)],1,COOL[0])
    if pose!='defeat':
        stroke('pauldron lip',[(10,shoulder_y+4),(13,shoulder_y+5),(17,shoulder_y+4)],1,COOL[0])
        # Curved hoops follow the bulging apron, with a thin warm upper edge.
        shift=crouch//2 if pose.startswith('sweep') or pose=='recover' else 0
        for yy in [47+shift,58+shift]:
            stroke('belly hoop',[(20,yy),(25,yy+2),(37,yy+3),(46,yy)],2,'d')
            stroke('hoop warm edge',[(20,yy-1),(25,yy+1),(37,yy+2),(46,yy-1)],1,'e')
        # A small pulley, with a two-pixel rim rather than a rear brown hump.
        wx,wy=(52,37+shift)
        stroke('pulley rim',[(wx+1,wy),(wx+3,wy),(wx+5,wy+2),(wx+5,wy+5),
                            (wx+3,wy+7),(wx+1,wy+7),(wx,wy+5),(wx,wy+2),(wx+1,wy)],2,'g')
        stroke('pulley axle',[(wx+2,wy+1),(wx+2,wy+6)],2,'g')
        stroke('pulley rim light',[(wx+1,wy),(wx+3,wy),(wx+5,wy+2),(wx+5,wy+5)],1,'f')
        if not pose.startswith('sweep'):
            for yy in [52,59]:
                stroke('rear arm iron band',[(47,yy),(51,yy+2),(56,yy)],2,'g')
                stroke('rear arm band light',[(47,yy-1),(51,yy+1),(56,yy-1)],1,'f')
        # Narrow articulated banding survives as strokes instead of texture.
        stroke('near wrist band',[(7,57+min(shift,3)),(11,59+min(shift,3)),(15,57+min(shift,3))],2,'g')
        if pose.startswith('sweep-tell'):
            hookx,hooky=47+stage*4,64-stage*5
        elif pose=='sweep':
            hookx,hooky=9,65
        else:
            hookx,hooky=8,64+min(shift,3)
        # Each 3x2 link belongs to one continuous >=5px centreline.
        path=[(30,64),(27,66),(24,65),(21,67),(18,66),(15,68),(hookx+3,hooky)]
        if pose.startswith('sweep-tell'):
            path=[(39,64),(43,67),(47,66),(51,63),(hookx+3,hooky)]
        stroke('hanging chain shadow',path,2,'g')
        stroke('hanging chain links',path,1,'f')
        stroke('hand hook',[(hookx+1,hooky-1),(hookx+1,hooky+1),(hookx,hooky+3),
                            (hookx-1,hooky+3),(hookx-2,hooky+1)],2,'f')
    material_rows=rows
    x,y=hx+8,hy+19
    core={(x,y),(x+1,y),(x-1,y+1),(x,y+1),(x+1,y+1)}
    fall={q for p in core for q in boss.neighbours(*p)}-core
    signature={'name':'curved amber visor slit',
               'pixels':[[xx,yy,'a'] for xx,yy in sorted(core)],
               'falloff':[[xx,yy,COOL[0]] for xx,yy in sorted(fall)],
               'hues':{k:'#%02x%02x%02x'%INK[k] for k in ['a',COOL[0]]}}
    spec={'rows':rows,'ramps':[WARM,COOL,'bcdefgh'],
          'signature_lights':[signature], 'linear_features':features,
          'material_rows':material_rows,'material_min_side':4}
    spec['rows']=boss.signature_rows(spec)
    spec['helmet_origin']=[hx,hy]
    spec['damage_origin']=[22,40+min(shift,5)] if pose!='defeat' else [22,51]
    return spec


def damage(spec, stage):
    import copy
    spec=copy.deepcopy(spec)
    rows=list(map(list,spec['material_rows']))
    ox,oy=spec['damage_origin']
    paths=[[(ox,oy),(ox+2,oy+4),(ox,oy+7),(ox+3,oy+10),(ox+1,oy+14)]]
    if stage==2:
        paths += [[(39,43),(36,47),(39,50),(35,54),(37,58),(33,63)],
                  [(39,50),(44,48),(47,51)],
                  [(37,58),(43,60),(50,64)]]
    for n,path in enumerate(paths):
        # Fracture shadows run along the jagged fault rather than across the belly.
        for width,k in [(3,'h'),(1,'a')]:
            mask=Image.new('1',(64,80));ImageDraw.Draw(mask).line(path,fill=1,width=width)
            points={(x,y) for y in range(80) for x in range(64) if mask.getpixel((x,y)) and rows[y][x]!='.'}
            underlay=[[x,y,rows[y][x]] for x,y in sorted(points)]
            for x,y in points:rows[y][x]=k
            spec['linear_features'].append(dict(tag='LINEAR',name='branching fracture' if n else 'shoulder fracture',path=path,width=width,ink=k,pixels=sorted(points),underlay=underlay))
    if stage==2:
        # Spring a three-pixel gap in the lower hoop, exposing original leather.
        under=list(map(list,spec['material_rows']))
        for feature in reversed(spec['linear_features']):
            for x,y,k in feature['underlay']:under[y][x]=k
        hoops=[f for f in spec['linear_features'] if f['name'] in ['belly hoop','hoop warm edge']]
        lower=max(f['path'][0][1] for f in hoops)-1
        for feature in hoops:
            if feature['path'][0][1]>=lower:
                for x,y in feature['pixels']:
                    if 29<=x<=31 and not any((x,y) in f['pixels'] for f in spec['linear_features'] if f['name'] not in ['belly hoop','hoop warm edge']):rows[y][x]=under[y][x]
        # A dark, bent rim makes the crown dent read without a mottled patch.
        hx,hy=spec['helmet_origin']
        path=[(hx+22,hy+3),(hx+22,hy+7),(hx+26,hy+10)]
        mask=Image.new('1',(64,80));ImageDraw.Draw(mask).line(path,fill=1,width=3)
        points={(x,y) for y in range(80) for x in range(64) if mask.getpixel((x,y)) and rows[y][x]!='.'}
        underlay=[[x,y,rows[y][x]] for x,y in sorted(points)]
        for x,y in points:rows[y][x]='g'
        spec['linear_features'].append(dict(tag='LINEAR',name='dented helm rim',path=path,width=3,ink='g',pixels=sorted(points),underlay=underlay))
    covered=set()
    for feature in reversed(spec['linear_features']):
        points={tuple(p) for p in feature['pixels']}
        keep={p for p in points if rows[p[1]][p[0]]==feature['ink'] or p in covered}
        feature['pixels']=sorted(keep)
        feature['underlay']=[p for p in feature['underlay'] if tuple(p[:2]) in keep]
        covered.update(keep)
    spec['linear_features']=[f for f in spec['linear_features'] if f['pixels']]
    spec['material_rows']=[''.join(r) for r in rows]
    spec['rows']=boss.signature_rows(dict(spec,rows=spec['material_rows']))
    return spec


def validate(recipe):
    def keeper_gate(name,spec):
        details={(x,y) for light in spec.get('signature_lights',[])
                 for x,y,k in light['pixels']+light['falloff']}
        linear=boss.linear_pixels(spec['linear_features'],spec['material_rows'])
        # A stroke may bisect a broad material into a one-pixel remainder.
        # Protect only pixels whose matching neighbour was explicitly overlaid.
        under=list(map(list,spec['material_rows']))
        for feature in reversed(spec['linear_features']):
            for x,y,k in feature['underlay']:under[y][x]=k
        split={(x,y) for y,row in enumerate(spec['rows']) for x,k in enumerate(row)
               if any((nx,ny) in linear and 0<=nx<64 and 0<=ny<80 and under[ny][nx]==k
                      for nx,ny in boss.neighbours(x,y))}
        boss.gate(spec['rows'],{k:INK[k] for k in set(''.join(spec['rows']))-{'.'}},details|linear|split)
        assert not boss.signature_gate(spec,art.INKS),name
        assert all((x,y) in details or k==spec['material_rows'][y][x]
                   for y,row in enumerate(spec['rows']) for x,k in enumerate(row)),name
        try:
            boss.region_gate(spec['material_rows'],spec['ramps'],4,spec['linear_features'])
        except AssertionError as error:
            raise AssertionError(f'{name}: {error}') from error
    for name,spec in recipe['assets'].items():
        if name.startswith('keeper-'):
            keeper_gate(name,spec)
            for n,variant in enumerate(spec.get('damage_variants',[])):
                keeper_gate(name+str(n),variant)
        else:
            boss.gate(spec['rows'],{k:INK[k] for k in set(''.join(spec['rows']))-{'.'}})
    idle=recipe['assets']['keeper-idle']
    for name,size in [('pulley rim',(8,8)),('hand hook',(6,5))]:
        feature=next(f for f in idle['linear_features'] if f['name']==name)
        xs,ys=zip(*feature['pixels'])
        assert (max(xs)-min(xs)+1,max(ys)-min(ys)+1)==size,name
    # Explicit LINEAR strokes pass; identical untagged strokes remain mottling.
    rows=['aa']*8
    feature=dict(tag='LINEAR',name='negative-control hoop',path=[(0,0),(0,7)],width=2,ink='a',pixels=[[x,y] for y in range(8) for x in range(2)])
    assert not list(boss.region_errors(rows,['a'],linear_features=[feature]))
    # A valid tagged line must not shelter an unrelated 2x3 mottle patch.
    mottled=[r+'...bb' if y<3 else r+'.....' for y,r in enumerate(rows)]
    assert any(k=='b' for k,part in boss.region_errors(mottled,['ab'],linear_features=[feature]))
    assert list(boss.region_errors(rows,['a']))
    assert list(boss.region_errors(['aaa']*6,['a']))
    assert list(boss.region_errors(['aaa']*3,['a']))
    assert list(boss.region_errors(['a'],['a']))
    assert not list(boss.region_errors(['aaaa']*4,['a']))
    for fake in [dict(feature,path=[(0,0),(0,2)]),dict(feature,width=4),
                 dict(feature,pixels=[[0,0],[1,0],[0,1],[1,1]])]:
        try:
            boss.linear_pixels([fake],rows)
        except AssertionError:
            pass
        else:
            raise AssertionError('a short patch must never accept the LINEAR tag')


def main(reference, review):
    review.mkdir(parents=True, exist_ok=True)
    ref = boss.extract(Image.open(reference), 'neutral', 80)
    ref = ref.crop(ref.getbbox())
    ref.thumbnail((60,74), Image.Resampling.NEAREST)
    ref, _ = boss.snap(ref, art.INKS)
    ref.save(review/'reference-native.png')
    ref.resize((ref.width*3,ref.height*3), Image.Resampling.NEAREST).save(review/'reference-3x.png')
    recipe = json.loads((art.OUT/'native.json').read_text())
    recipe['inks'].update(DETAIL)
    for pose in POSES:
        spec = keeper(pose, 3 if pose.endswith('tell') else 0)
        if pose in ['damage-1','damage-2']:
            spec=damage(spec,1 if pose=='damage-1' else 2)
        elif pose!='defeat':
            spec['damage_variants']=[damage(spec,n) for n in [1,2]]
        recipe['assets']['keeper-'+pose] = spec
    for attack in ['drop', 'sweep']:
        for stage in range(4):
            spec=keeper(attack+'-tell',stage)
            spec['damage_variants']=[damage(spec,n) for n in [1,2]]
            recipe['assets'][f'keeper-{attack}-tell-{stage}']=spec
    for frame in range(4):
        im = Image.new('RGBA', (48,32));d = ImageDraw.Draw(im)
        # Two separated low puffs. The last frame falls back into the deck.
        h = [8,13,16,7][frame]
        for x in [2-frame//2,30+frame//2]:
            d.rectangle((x,34-h,x+14,30),fill=INK['l'])
            for dx,drop in [(0,3),(4,0),(9,2)]:
                d.ellipse((x+dx,31-h+drop,x+dx+6,39-h+drop),fill=INK['j'])
            d.rectangle((x+3,min(37-h,28),x+11,28),fill=INK['j'])
        _, rows = boss.snap(im, art.INKS)
        recipe['assets'][f'fx-dust-{frame}'] = {'rows':rows, 'ramps':['lj']}
        assert im.getbbox()[3]-im.getbbox()[1] <= 17
        assert len(set(''.join(rows))-set('.')) == 2
    validate(recipe)
    for name, spec in recipe['assets'].items():
        boss.render(spec['rows'], art.INKS).save(art.OUT/(name+'.png'))
    (art.OUT/'native.json').write_text(json.dumps(recipe,separators=(',',':'))+'\n')
    for scale in [1,3]:
        art.sheet(['keeper-'+p for p in POSES], review/f'keeper-{scale}x.png', scale,4)
        art.sheet([f'keeper-{p}-tell-{n}' for p in ['drop','sweep'] for n in range(4)], review/f'tells-{scale}x.png',scale,4)
        art.sheet([f'fx-dust-{n}' for n in range(4)],review/f'dust-{scale}x.png',scale,4)
    (review/'art-gates.txt').write_text('Broad masses keep 4x4 blocks; recipe-tagged LINEAR strokes alone are exempt.\n'
        'Curved 3x2 #e0962c slit and 1px falloff pass shared signature gate.\n'
        'All assets pass <=10-ink palette, stroke-aware singleton and untagged-mottle controls.\n'
        'Dust: two cool grey-ochre lobed puffs, two tones, <=17px.\n')


if __name__ == '__main__':
    if sys.argv[1:] == ['--check']:
        validate(json.loads((art.OUT/'native.json').read_text()))
        print('Palette, singleton, 4x4 material, explicit LINEAR, signature, provenance and negative controls pass.')
    else:
        main(Path(sys.argv[1]), Path(sys.argv[2]))
