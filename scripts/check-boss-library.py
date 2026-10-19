#!/usr/bin/env python3
"""Library display gates; original absolute-area gates remain for other bosses."""
import argparse,copy,json,math,runpy,statistics
from functools import lru_cache
from pathlib import Path
from PIL import Image
ROOT=Path(__file__).resolve().parents[1]
A=runpy.run_path(str(ROOT/'scripts/boss-art.py'))
I=runpy.run_path(str(ROOT/'scripts/boss-library-import.py'))
# Keep the established 15 L* body separation and 80% source-alpha coverage,
# but compare every sample with its own graded floor, rather than an old sprite.
BODY_DELTA = 15.0
VISIBLE_ALPHA_FLOOR = .80
EDGE_DELTA = 10.0
EDGE_COVERAGE = .50


def failures(median_delta, visible_fraction, edge_fraction, pose='rest'):
    result = []
    if median_delta + 1e-6 < BODY_DELTA:
        result.append('body median floor contrast')
    # The retained envelope gate samples rest, as it did before this change.
    # Both poses must independently pass local median and silhouette contrast.
    if pose == 'rest' and visible_fraction + 1e-6 < VISIBLE_ALPHA_FLOOR:
        result.append('body floor-contrast coverage')
    if edge_fraction + 1e-6 < EDGE_COVERAGE:
        result.append('silhouette floor-contrast coverage')
    return result


@lru_cache(maxsize=None)
def lightness(colour):
    return A['lightness'](colour)


def contrast_metrics(body, floor, points, edge):
    """Signed local body separation; dark contours and light rims both count."""
    delta = {p: lightness(tuple(body(p))) - lightness(tuple(floor(p))) for p in points}
    visible = sum(d + 1e-6 >= BODY_DELTA for d in delta.values())
    return dict(median_delta=statistics.median(delta.values()),
                visible_fraction=visible / len(points),
                body_pixels=len(points), visible_pixels=visible,
                presence_shortfall_pixels=max(0,math.ceil(VISIBLE_ALPHA_FLOOR*len(points))-visible),
                edge_fraction=sum(abs(delta[p]) + 1e-6 >= EDGE_DELTA for p in edge) / len(edge),
                floor_median=statistics.median(lightness(tuple(floor(p))) for p in points),
                floor_p95=sorted(lightness(tuple(floor(p))) for p in points)[int(.95*(len(points)-1))],
                body_median=statistics.median(lightness(tuple(body(p))) for p in points))


def silhouette_edge(alpha, radius):
    # The outer two logical pixels include both the contour and its inner rim.
    # Include transparency beyond the canvas so a crop edge cannot disappear.
    return {p for p in alpha if any((p[0]+dx,p[1]+dy) not in alpha
            for dy in range(-radius,radius+1) for dx in range(-radius,radius+1))}


def floor_contrast_gates(selected, captures):
    errors, records, seen = [], [], set()
    fixtures = json.loads((captures/'floor-contrast-fixtures.json').read_text())
    for fixture in fixtures:
        boss, kit, pose = (fixture[k] for k in ('boss','kit','pose'))
        if boss not in selected:
            continue
        key = (boss,kit,pose)
        assert key not in seen, f'duplicate contrast fixture {key}'
        seen.add(key)
        assert fixture['players'] == 4 and fixture['room_kind'] == 'stairs'
        assert fixture['pack'] == fixture['room']['pack']
        room = fixture['room']['rooms'][fixture['room_index']]
        assert room['kind'] == 'stairs' and room['cols'] == 36 and room['rows'] == 20
        definition = (ROOT/'cockpit/assets/dungeon/bosses'/f'{boss}.boss').read_text()
        pack_name = {'cavern':'mines'}.get(fixture['pack'],fixture['pack'])
        assert f"where {pack_name}\n" in definition, 'wrong boss pack'
        stem = captures/fixture['name']
        rgb = Image.open(str(stem)+'-rgb.png').convert('RGB')
        floor = Image.open(str(stem)+'-floor-rgb.png').convert('RGB')
        raw = json.loads(Path(str(stem)+'-cells.json').read_text())
        floor_raw = json.loads(Path(str(stem)+'-floor-cells.json').read_text())
        im = Image.open(ROOT/'cockpit/assets/dungeon/bosses'/boss/f'library-v{kit}-{pose}.png').convert('RGBA')
        assert list(im.size) == fixture['size'] == selected[boss][f'v{kit}']
        x0,y0 = fixture['origin']
        points = {(x0+x,y0+y) for y in range(im.height) for x in range(im.width) if im.getpixel((x,y))[3]}
        assert all(0 <= x < rgb.width and 0 <= y < rgb.height for x,y in points), 'clipped body'
        edge = silhouette_edge(points,2*kit)
        def decoder(cells):
            cw,ch = 4*kit,8*kit
            return lambda p: A['terminal_colour'](cells[p[1]//ch*96+p[0]//cw],p[0]%cw*2//cw,p[1]%ch*4//ch)
        for path,body,background in [('ROOM',rgb.getpixel,floor.getpixel),('CELLS',decoder(raw),decoder(floor_raw))]:
            metrics = contrast_metrics(body,background,points,edge)
            errors.extend(f'{boss} v{kit} {pose} {path}: {error}' for error in failures(
                metrics['median_delta'],metrics['visible_fraction'],metrics['edge_fraction'],pose))
            records.append(dict(boss=boss,kit=kit,pose=pose,path=path,**metrics))
            presence_rule = f'>= {VISIBLE_ALPHA_FLOOR}' if pose == 'rest' else '(diagnostic)'
            print(f"{boss} v{kit} {pose} {path}: floor {metrics['floor_median']:.2f}; body {metrics['body_median']:.2f}; "
                  f"median delta {metrics['median_delta']:.2f} >= {BODY_DELTA}; "
                  f"body coverage {metrics['visible_fraction']:.3f} {presence_rule}; "
                  f"edge {metrics['edge_fraction']:.3f} >= {EDGE_COVERAGE}")
    expected = {(boss,kit,pose) for boss in selected for kit in (1,2) for pose in ('rest','windup')}
    assert seen == expected, f'missing contrast fixtures: {expected-seen}'
    return errors,records

def point_stage_gates(captures,before):
    """Retain the pre-pilot point-mark phase checks and known v1 limitation."""
    errors=[];box=(304,224,336,256)
    for boss in ('cinderjaw','mother-of-spores'):
        for kit in (1,2):
            divisor=2 if kit==1 else 1
            frames=[]
            for stage in range(4):
                stem=captures/f'{boss}-effects-{stage}-v{kit}'
                rgb=Image.open(str(stem)+'-rgb.png').convert('RGB')
                raw=json.loads(Path(str(stem)+'-cells.json').read_text())
                points=[(x,y)for y in range(box[1],box[3])for x in range(box[0],box[2])]
                frames.append(([rgb.getpixel((x//divisor,y//divisor))for x,y in points],
                               [raw[y//16*96+x//8]for x,y in points]))
            for stage in range(1,4):
                changed=[sum(a!=b for a,b in zip(frames[stage-1][i],frames[stage][i]))for i in (0,1)]
                print(f'{boss} v{kit} point tell {stage-1}->{stage}: RGB/cell changed samples {changed}')
                if not changed[0]:
                    errors.append(f'{boss} v{kit}: point-tell RGB stage collapse {stage}')
                if not changed[1]:
                    old=[json.loads((before/f'{boss}-effects-{s}-v{kit}-cells.json').read_text())for s in (stage-1,stage)]
                    old_diff=sum(old[0][y//16*96+x//8]!=old[1][y//16*96+x//8]for x,y in points)
                    if old_diff:errors.append(f'{boss} v{kit}: new point-tell stage collapse {stage}')
                    else:print(f'{boss} v{kit}: pre-existing cell-stage collapse; source effects unchanged')
    return errors

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('captures',type=Path);p.add_argument('--before',type=Path,help='original effect/room fixtures, required only with room-fixtures.json');p.add_argument('--identity',type=Path,required=True);p.add_argument('--metrics',type=Path)
    p.add_argument('--contrast',type=Path,help='paired generated guardian-room bodies and graded floors (defaults to captures)')
    p.add_argument('--include-inactive',action='store_true',help='gate rejected candidates in a review-feature study; never activates them')
    args=p.parse_args()
    active={boss:config for boss,config in I['BOSSES'].items() if config.get('active',True)}
    selected=I['BOSSES'] if args.include_inactive else active
    entries=[e for f in ['boss-art-pilots.json','boss-art-crypt.json','boss-art-reliquary.json','boss-art-pit-debt.json'] for e in json.loads((ROOT/'scripts'/f).read_text()) if Path(e['boss']).stem in selected]
    errors=[]
    for entry in entries:
        boss=Path(entry['boss']).stem
        # Earlier body recipes are not provenance for library pixels. Their
        # unchanged effects still pass every native palette/shape/timing gate.
        errors+=A['numeric_gates']([copy.deepcopy(entry)])
        for role in ('projectile','tell','impact'):
            for i,frame in enumerate(entry['frames'][role]):
                path=ROOT/entry['output']/(role+(f'-{i}' if i else '')+'.png')
                expected=A['render'](A['signature_rows'](frame),A['frame_inks'](frame,A['palette']()))
                assert Image.open(path).convert('RGBA').tobytes()==expected.tobytes(),f'changed effects {path}'
        I['run'](boss,check=True)
    failures_,records=floor_contrast_gates(selected,args.contrast or args.captures);errors+=failures_
    identity=json.loads(args.identity.read_text())
    for boss in selected.keys()-active.keys():
        print(f'{boss}: inactive candidate; no identity approval or production activation')
    for boss,config in active.items():
        for kit in ('v1','v2'):
            review=identity[boss][kit]
            assert review['props']==config['props'] and review['inspected_cells_2x'] is True
            for evidence in review['evidence']:
                path=Path(evidence['path']);assert I['digest'](path)==evidence['sha256'],'stale identity review'
            assert review['visible_props']==config['cell_props'],f'{boss}/{kit}: unresolved identity prop'
            print(f'{boss}/{kit}: inspected 2x cell identity: '+', '.join(review['visible_props']))
    if (args.captures/'room-fixtures.json').exists():
        assert args.before, '--before is required for the unchanged effect/room regressions'
        assert json.loads((args.captures/'room-fixtures.json').read_text())==json.loads((args.before/'room-fixtures.json').read_text()),'room changed'
        S=runpy.run_path(str(ROOT/'scripts/check-boss-art-sendback.py'))
        errors+=S['measure'](args.captures,display=I['BOSSES'])[0]
        errors+=point_stage_gates(args.captures,args.before)
    if args.metrics:args.metrics.write_text(json.dumps(records,indent=2)+'\n')
    for e in errors:print('FAIL:',e)
    print(f'{len(errors)} library display failure(s)')
    return bool(errors)

if __name__=='__main__':raise SystemExit(main())
