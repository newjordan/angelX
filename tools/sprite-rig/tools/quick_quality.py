"""Quick-rig review gate, not a proof of artistic quality.

API: review(game, native, out, *, rig=None, image=None, root=None) -> checks dict.
``game`` and ``rig`` accept dictionaries or JSON paths. The native manifest is
mandatory Rust-exported ink rows (realm preferred; legacy knight is supported).
Source + fixup are optional *together*: they enable exact rebuild/provenance and
counterfactual no-smear checks. Without them those checks are explicitly skipped.

Contact sheets crop transparent padding for layout ONLY. Neither subject is
resized at 1x; the native painted floor and global rig pivot share a groundline. Cyan weapon and magenta
offhand contacts are diagnostic overlays, never included in measured ink rows.
"""
import argparse
import copy
import json
import math
from pathlib import Path
import sys

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from spriterig import native as N
from spriterig import quick as Q

LOOPS = ('idle', 'walk', 'run')
SWINGS = ('slash', 'overhead', 'thrust', 'flail')
NOTE = ('Silhouette, anatomical proximity, motion distinctness and ink-expansion '
        'tests are conservative heuristics, not human proof of readability, '
        'anatomy, animation quality or aesthetics. Inspect the 1x sheets and GIFs.')


def _json(value):
    return N.read_json(value) if isinstance(value, (str, Path)) else value


def _mask(frame):
    return np.array([list(row) for row in frame['rows']]) != '.'


def _bounds(mask):
    y, x = np.nonzero(mask)
    return (int(x.min()), int(y.min()), int(x.max())+1, int(y.max())+1) if len(x) else None


def _distance(point, mask):
    y, x = np.nonzero(mask)
    return float(np.hypot(x-point[0], y-point[1]).min()) if len(x) else math.inf


def _silhouette(mask):
    """8-connected mass permits diagonal pixel art and small detached effects."""
    bounds = _bounds(mask)
    if bounds is None:
        return dict(pixels=0, components=0, largest_fraction=0., body_mass=0., density=0.)
    x0, y0, x1, y1 = bounds
    small = mask[y0:y1, x0:x1]
    seen = np.zeros(small.shape, bool)
    sizes = []
    for y, x in zip(*np.nonzero(small)):
        if seen[y, x]:
            continue
        todo = [(y, x)]
        seen[y, x] = True
        size = 0
        while todo:
            cy, cx = todo.pop()
            size += 1
            for dy in (-1, 0, 1):
                for dx in (-1, 0, 1):
                    ny, nx = cy+dy, cx+dx
                    if (0 <= ny < small.shape[0] and 0 <= nx < small.shape[1]
                            and small[ny, nx] and not seen[ny, nx]):
                        seen[ny, nx] = True
                        todo.append((ny, nx))
        sizes.append(size)
    # Fraction of ink in solid 2x2 cells: a line/scattered dots are not a body.
    solid = small[:-1, :-1] & small[1:, :-1] & small[:-1, 1:] & small[1:, 1:]
    mass = np.zeros(small.shape, bool)
    for dy in (0, 1):
        for dx in (0, 1):
            mass[dy:dy+solid.shape[0], dx:dx+solid.shape[1]] |= solid
    count = int(small.sum())
    return dict(pixels=count, components=len(sizes), largest_fraction=max(sizes)/count,
                body_mass=float(mass.sum()/count), density=float(small.mean()))


def _anatomy(frame, upright):
    """Spatial bands are deliberately loose; source provenance is the strong test.

    Tip is a virtual weapon endpoint, NOT a painted body attachment. Feet may
    sit between the shoes; hands can be a pixel outside their wrist/hand ink.
    Upright rest/walk/run get head/body/foot bands to reject anchors on the wrong
    painted region, even when every anchor is technically on some opaque pixel.
    """
    mask = _mask(frame)
    box = _bounds(mask)
    if box is None:
        return dict(ok=False, reason='empty frame', distances={})
    x0, y0, x1, y1 = box
    height, width = y1-y0, x1-x0
    Y, X = np.indices(mask.shape)
    tolerance = max(2., min(4., height*.12))
    regions = {name: mask.copy() for name in ('grip', 'off_hand', 'head', 'back', 'feet')}
    if upright:
        regions['head'] &= Y < y0+height*.36
        regions['back'] &= (Y >= y0+height*.18) & (Y < y0+height*.65)
        regions['feet'] &= Y >= y0+height*.78
        for name in ('grip', 'off_hand'):
            regions[name] &= (Y >= y0+height*.36) & (Y < y0+height*.84)
        # Final-facing-right convention: do not accept swapping painted hands.
        regions['grip'] &= X >= x0+width*.45
        regions['off_hand'] &= X <= x0+width*.60
    distances = {name: _distance(frame['anchors'][name], region)
                 for name, region in regions.items()}
    # A floor socket may legitimately sit in the empty gap BETWEEN the shoes.
    # Measure against their footprint envelope, not only one painted shoe.
    if upright and (foot_box := _bounds(regions['feet'])):
        fx, fy = frame['anchors']['feet']
        left, top, right, bottom = foot_box
        distances['feet'] = math.hypot(max(left-fx, 0, fx-(right-1)),
                                      max(top-fy, 0, fy-(bottom-1)))
    # JSON has no Infinity. A missing region is an explicit failure.
    return dict(ok=all(d <= tolerance for d in distances.values()), tolerance=tolerance,
                distances={k: round(v, 3) if math.isfinite(v) else None
                           for k, v in distances.items()})


def _native(path, pal):
    frames = N.load_native(path, pal)
    # Old Rust exports called the realm knight 'knight'. Never pick an arbitrary
    # first kit (e.g. percival), nor demand equality to today's bootstrap fits.
    kit = next((k for k in ('realm', 'knight') if any(f['kit'] == k for f in frames)), None)
    if kit is None:
        raise ValueError('native baseline has no realm (or legacy knight) frames')
    frames = [f for f in frames if f['kit'] == kit]
    if any(not any(c not in '. ' for r in f['rows'] for c in r) for f in frames):
        raise ValueError('native baseline contains an empty frame')
    return sorted(frames, key=lambda f: (
        ('right', 'side', 'front', 'back', 'left').index(f['view'])
        if f['view'] in ('right', 'side', 'front', 'back', 'left') else 5,
        f['pose'], f['tick']))


def _reference(refs, clip, index):
    poses = ('stand', 'idle') if clip == 'idle' else (
        ('walk', 'run') if clip in ('walk', 'run') else ('strike', 'attack'))
    matching = [f for f in refs if any(f['pose'].startswith(p) for p in poses)]
    candidates = matching or refs
    view = candidates[0]['view']
    candidates = [f for f in candidates if f['view'] == view]
    return candidates[index % len(candidates)]


def _schema(game, root):
    Q.validate_game(game, root=root)
    if set(game) != {'version', 'canvas', 'pivot', 'clips'}:
        raise ValueError('game: expected only version/canvas/pivot/clips; fixed global pivot')
    for name, frames in game['clips'].items():
        for i, frame in enumerate(frames):
            if set(frame) != {'ticks', 'rows', 'anchors'}:
                raise ValueError(f'{name}[{i}]: only ticks/rows/anchors; no per-frame pivot/canvas')


def _record(report, name, ok, *, heuristic=False, **data):
    report['checks'][name] = dict(status='pass' if ok else 'fail', heuristic=heuristic, **data)


def _skip(report, name, reason):
    report['checks'][name] = dict(status='skipped', heuristic=False, reason=reason)


def _measure(game, report):
    missing = sorted(set(Q.CLIPS)-set(game['clips']))
    _record(report, 'clip_coverage', not missing, missing=missing,
            note='Full quick-rig review requires all standard clips; extra clips are also reviewed.')
    silhouettes, anatomy, floors = {}, {}, {}
    for name, frames in game['clips'].items():
        silhouettes[name] = [_silhouette(_mask(f)) for f in frames]
        anatomy[name] = [_anatomy(f, name in LOOPS) for f in frames]
        if name in LOOPS:
            floors[name] = [(_bounds(_mask(f)) or [0, 0, 0, 0])[3]-1 for f in frames]
    bad_silhouettes = [f'{name}[{i}]' for name, values in silhouettes.items()
                       for i, m in enumerate(values) if not (
                           m['pixels'] >= 12 and m['largest_fraction'] >= .50
                           and m['body_mass'] >= .25 and m['density'] >= .08)]
    _record(report, 'silhouette_1x', not bad_silhouettes, heuristic=True,
            failures=bad_silhouettes, metrics=silhouettes,
            thresholds=dict(min_pixels=12, largest_fraction=.50, body_mass=.25, density=.08))
    _record(report, 'painted_anchor_proximity', all(m['ok'] for a in anatomy.values() for m in a),
            heuristic=True, frames=anatomy,
            note='Loose painted-region proximity; tip is virtual. Rebuild is stronger.')
    _record(report, 'fixed_floor', all(ys and set(ys) == {game['pivot'][1]} for ys in floors.values()),
            floors=floors, pivot_y=game['pivot'][1],
            note='All present idle/walk/run painted bottoms must equal the global pivot y.')
    # Include complete frame sequences, not just the first frame (often rest).
    # Compare distinct pixel states independently of ticks, anchors and repeats.
    signatures = {n: frozenset(tuple(f['rows']) for f in fs) for n, fs in game['clips'].items()}
    duplicates = [[a, b] for i, a in enumerate(signatures) for b in list(signatures)[i+1:]
                  if signatures[a] == signatures[b]]
    frozen = [n for n, states in signatures.items() if len(states) < 2]
    _record(report, 'distinct_clips', not duplicates and not frozen, heuristic=True,
            duplicate_pairs=duplicates, frozen_clips=frozen,
            unique_frames={n: len(s) for n, s in signatures.items()})
    # Without source, an approximate expansion check can still catch dead effects.
    expansion = {}
    for name in SWINGS:
        if name in game['clips']:
            counts = [int(_mask(f).sum()) for f in game['clips'][name]]
            expansion[name] = dict(min_pixels=min(counts), max_pixels=max(counts),
                                   expands=max(counts) >= min(counts)+max(2, math.ceil(min(counts)*.02)))
    if expansion:
        _record(report, 'swing_ink_expansion', all(m['expands'] for m in expansion.values()),
                heuristic=True, clips=expansion,
                note='Ink-area variation only; no-smear source counterfactual proves added ink.')
    else:
        _skip(report, 'swing_ink_expansion', 'No swing clips supplied.')


def _provenance(game, rig, image, root, report):
    if rig is None and image is None:
        for name in ('source_rebuild', 'source_anchor_regions', 'smear_added_ink'):
            _skip(report, name, 'Supply --rig and --input together for source transform provenance.')
        return
    if rig is None or image is None:
        raise ValueError('--rig and --input must be supplied together')
    rig = _json(rig)
    rebuilt = Q.build(image, rig, root=root, clips=list(game['clips']))
    differences = []
    if game['canvas'] != rebuilt['canvas'] or game['pivot'] != rebuilt['pivot']:
        differences.append('canvas/pivot')
    for name, frames in game['clips'].items():
        expected = rebuilt['clips'][name]
        if len(frames) != len(expected):
            differences.append(f'{name}: frame count')
        for i, (a, b) in enumerate(zip(frames, expected)):
            for key in ('rows', 'anchors', 'ticks'):
                if a[key] != b[key]:
                    differences.append(f'{name}[{i}].{key}')
    _record(report, 'source_rebuild', not differences, differences=differences,
            note='Exact Q.build comparison of every frame: pixels, ticks, anchors, canvas, pivot.')
    # A self-consistently broken source socket must not pass merely because its
    # broken game matches a rebuild. Check the source socket against its named
    # anatomical mask, not against arbitrary ink elsewhere on the body.
    expected_parts = dict(grip=('hand_f',), off_hand=('hand_b',), head=('head',),
                          back=('back', 'torso'), feet=('foot_f', 'foot_b'))
    source = rig['source']
    rgba = Q._image(image, **{k: source.get(k) for k in
                    ('cell', 'cell_size', 'facing', 'alpha_threshold', 'key_color')})
    painted = rgba[..., 3] != 0
    distances, bad = {}, []
    tolerance = max(2., min(4., rig['source']['size'][1]*.12))
    for name, parts in expected_parts.items():
        mask = np.zeros(tuple(reversed(rig['source']['size'])), bool)
        for part in parts:
            mask |= np.array([list(r) for r in rig['parts'][part]['mask']]) == '#'
        d = _distance(rig['anchors'][name]['point'], mask & painted)
        distances[name] = round(d, 3) if math.isfinite(d) else None
        allowed = (None, *parts) if name == 'feet' else parts
        if d > tolerance or rig['anchors'][name]['part'] not in allowed:
            bad.append(name)
    _record(report, 'source_anchor_regions', not bad, heuristic=True,
            failures=bad, distances=distances, tolerance=tolerance)
    no_smear = copy.deepcopy(rig)
    for poses in no_smear['clips'].values():
        for pose in poses:
            pose['smear'] = False
    plain = Q.build(image, no_smear, root=root, clips=list(game['clips']))
    effects = {}
    for name in SWINGS:
        if name not in game['clips']:
            continue
        added = []
        for i, (frame, base) in enumerate(zip(game['clips'][name], plain['clips'][name])):
            if i < len(rig['clips'][name]) and rig['clips'][name][i].get('smear'):
                added.append(dict(frame=i, pixels=int((_mask(frame) & ~_mask(base)).sum())))
        effects[name] = added
    if effects:
        _record(report, 'smear_added_ink', all(any(f['pixels'] > 0 for f in fs) for fs in effects.values()),
                clips=effects, note='Actual occupied pixels outside a rebuilt no-smear silhouette, not metadata.')
    else:
        _skip(report, 'smear_added_ink', 'No swing clips supplied.')


def _contacts(game, refs, out, pal, overlays=False):
    """A pair per frame, all clips; layout crop does not change pixel scale."""
    pairs = []
    for name, frames in game['clips'].items():
        row = []
        for i, frame in enumerate(frames):
            ref = _reference(refs, name, i)
            a = Image.fromarray(N.rows_rgba(ref['rows'], pal))
            b = Image.fromarray(N.rows_rgba(frame['rows'], pal))
            if overlays:
                draw = ImageDraw.Draw(b)
                grip, tip = frame['anchors']['grip'], frame['anchors']['tip']
                draw.line([tuple(grip), tuple(tip)], fill=(50, 235, 255, 255), width=1)
                for anchor, color in (('grip', (50, 235, 255, 255)),
                                      ('off_hand', (255, 80, 210, 255))):
                    x, y = frame['anchors'][anchor]
                    draw.line((x-2, y, x+2, y), fill=color)
                    draw.line((x, y-2, x, y+2), fill=color)
            # Ground is always measured from the RAW painted rows, not overlays.
            box = a.getbbox()
            fa = ((box[0]+box[2]-1)//2, box[3]-1)
            fb = tuple(game['pivot'])
            row.append((a, fa, b, fb, f'{name}[{i}] {ref["view"]}/{ref["pose"]}:{ref["tick"]}'))
        pairs.append(row)
    subjects = [(im.getbbox(), origin) for row in pairs for a, fa, b, fb, _ in row
                for im, origin in ((a, fa), (b, fb))]
    # One common origin-relative crop across ALL frames. The rig's global pivot
    # remains on the groundline: never conceal floor jitter by re-grounding each
    # frame. Native exports have no global pivot, so use their painted floor.
    left = max(0, *(origin[0]-box[0] for box, origin in subjects))
    right = max(0, *(box[2]-origin[0] for box, origin in subjects))
    above = max(0, *(origin[1]-box[1] for box, origin in subjects))
    below = max(0, *(box[3]-1-origin[1] for box, origin in subjects))
    cell_w, cell_h = max(92, left+right+8), above+below+29
    sheet = Image.new('RGB', (2*cell_w*max(map(len, pairs)), cell_h*len(pairs)+20), (24, 24, 32))
    draw = ImageDraw.Draw(sheet)
    draw.text((4, 3), '1x realm native | rig' + ('; cyan weapon / magenta offhand' if overlays else ''), fill='white')
    for r, row in enumerate(pairs):
        for c, (a, fa, b, fb, label) in enumerate(row):
            top = 20+r*cell_h
            draw.text((2*c*cell_w+3, top), label, fill='white')
            floor_y = top+18+above
            for col, (im, origin) in enumerate(((a, fa), (b, fb))):
                box = im.getbbox()
                x = (2*c+col)*cell_w+(cell_w-left-right)//2+left-(origin[0]-box[0])
                y = floor_y-(origin[1]-box[1])
                draw.line(((2*c+col)*cell_w, floor_y+1, (2*c+col+1)*cell_w-1, floor_y+1), fill=(65, 65, 75))
                sheet.paste(im.crop(box), (x, y), im.crop(box))
    stem = 'contacts_overlay' if overlays else 'contacts'
    sheet.save(out/f'{stem}_1x.png')
    sheet.resize((sheet.width*6, sheet.height*6), Image.Resampling.NEAREST).save(out/f'{stem}_6x.png')


def review(game, native, out, *, rig=None, image=None, root=None):
    """Write checks.json and (for valid nonempty inputs) previews; return report.

    Bad inputs and failed gates are reported with ``ok=False``. A skipped source
    check is not evidence of provenance. Baseline errors never trigger a fallback
    fabricated sprite. No exact art parity or size normalization is attempted.
    """
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    report = dict(version=1, ok=False, note=NOTE, checks={}, artifacts=[])
    try:
        root = N.root_path(root)
        pal = Q._palette(root)
        game = _json(game)
        try:
            refs = _native(native, pal)
            _record(report, 'native_baseline', True, path=str(native), kit=refs[0]['kit'],
                    frames=len(refs), note='Mandatory visual reference, not exact parity or a quality oracle.')
        except (OSError, ValueError, KeyError, TypeError) as exc:
            _record(report, 'native_baseline', False, error=str(exc))
            refs = None
        _schema(game, root)
        _record(report, 'runtime_schema', True, canvas=game['canvas'], pivot=game['pivot'])
        used = sorted(set(''.join(r for fs in game['clips'].values() for f in fs for r in f['rows']))-{'.'})
        _record(report, 'delve_palette', True, inks=used,
                note='Only authoritative Delve ink symbols; dot is transparent. No invented RGB palette.')
        _measure(game, report)
        try:
            _provenance(game, rig, image, root, report)
        except (OSError, ValueError, KeyError, TypeError) as exc:
            _record(report, 'source_rebuild', False, error=str(exc))
        if refs is not None and all(_bounds(_mask(f)) for fs in game['clips'].values() for f in fs):
            _contacts(game, refs, out, pal)
            _contacts(game, refs, out, pal, overlays=True)
            Q.export(game, out/'clips', root=root)
            report['artifacts'] = ['contacts_1x.png', 'contacts_6x.png', 'contacts_overlay_1x.png',
                                   'contacts_overlay_6x.png'] + [f'clips/{n}.gif' for n in game['clips']]
        report['ok'] = all(c['status'] != 'fail' for c in report['checks'].values())
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as exc:
        _record(report, 'input', False, error=str(exc))
    (out/'checks.json').write_text(json.dumps(report, indent=2, allow_nan=False)+'\n', encoding='utf-8')
    return report


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--game', required=True, help='Q.build game JSON')
    parser.add_argument('--rig', help='version 1 source fixup JSON; requires --input')
    parser.add_argument('--input', help='source PNG; requires --rig')
    parser.add_argument('--native', required=True, help='Rust native.json (realm or legacy knight)')
    parser.add_argument('--out', required=True, help='review output directory')
    args = parser.parse_args(argv)
    report = review(args.game, args.native, args.out, rig=args.rig, image=args.input)
    failures = [n for n, c in report['checks'].items() if c['status'] == 'fail']
    print(f'{"PASS" if report["ok"] else "FAIL"}: {Path(args.out)/"checks.json"}' +
          (f' ({", ".join(failures)})' if failures else ''))
    for name in failures:
        if 'error' in report['checks'][name]:
            print(f'  {name}: {report["checks"][name]["error"]}', file=sys.stderr)
    print(NOTE)
    return 0 if report['ok'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
