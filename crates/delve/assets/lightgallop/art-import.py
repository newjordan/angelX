#!/usr/bin/env python3
"""Lightgallop's art import: measured pieces, reviewed maps, the shared importer.

The wave-5 deliveries (/work/artifacts/art-prod-rt/slots/lightgallop.<slot>/v01)
came without a source-sheet.json, so the pieces are measured here from each
sheet's alpha (projection gaps), a few are merged or given a reviewed pivot, and
a copy of the delivery plus that metadata is laid in a prepared folder outside
the repository (the sheet's bytes are unchanged, so its SHA-256 still binds).
The shared importer (scripts/slot-art-import.py) then runs on the copy with
`--source-dir`. Nothing here edits a delivery, repaints a pixel or adds an
exposure change: the pixels are the importer's BOX reduction snapped to the
shared inks.

    python3 -I crates/delve/assets/lightgallop/art-import.py <slot> [--check]
    python3 -I crates/delve/assets/lightgallop/art-import.py --all [--check]

`--prepared` moves the prepared folders (default
/work/artifacts/monolith-private/art-import-lg/prepared).
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
from PIL import Image

ROOT = Path(__file__).resolve().parents[4]
ASSETS = ROOT / 'crates/delve/assets'
SLOTS_JSON = ASSETS / 'lightgallop/slots.json'
OUT = ASSETS / 'art-slots'
DELIVERIES = Path('/work/artifacts/art-prod-rt/slots')
PREPARED = Path('/work/artifacts/monolith-private/art-import-lg/prepared')
IMPORTER = ROOT / 'scripts/slot-art-import.py'

GEN = 'Luna (gpt-6-luna) via Codex CLI'

# Per slot: the sheet the importer binds, the canvas, how the pieces are found
# (`grid`: alpha projection gaps, with `merge` listing measured components that
# are one drawing, or `whole`: the full sheet is the one piece), each piece's
# source pivot in its own rectangle (None: bottom centre), registration and
# the reviewed notes. A piece's pivot is in the measured rectangle's pixels.
SPECS = {
    'lightgallop.layer-far': dict(
        sheet='source-sheet.png', size=(1774, 887), canvas=(192, 96), role='backdrop', whole=True, registration='shared-pivot',
        pivot=(887, 443.5), anchor=(96, 48), runtime=True,
        # The deep indigo sky sits between realm inks and the nearest-ink snap
        # stipples it in four. A reviewed ink grade (ink to ink, no pixel moves)
        # gathers the two darker stipple inks into the sky's own: the stepped
        # bands, the moon, the stars and the hills keep their pixels.
        grade={'192934': '22303a', '263b40': '22303a'},
        fit='native canvas 192x96, one frame, repeating horizontally; the whole opaque strip is the frame, scrolled slowest behind the road; stage 384x224.',
        basis='The delivered strip is one continuous 2:1 scenery picture (1774x887, opaque RGB), so the whole sheet is the single piece and it is reduced by 0.10823 to exactly 192x96 with no crop and no padding. The moon and the horizon band repeat every 192 px.',
        comments=['One piece, the whole sheet: the importer is given the full rectangle so the canvas is not cut to alpha bounds.']),
    'lightgallop.layer-mid': dict(
        sheet='source-sheet.png', size=(1774, 887), canvas=(192, 96), role='backdrop', whole=True, registration='shared-pivot',
        pivot=(887, 443.5), anchor=(96, 48), runtime=True,
        fit='native canvas 192x96, one frame, repeating horizontally; real alpha above the treeline; scrolled between far and near; stage 384x224.',
        basis='The first generation (source-sheet.png) is the kept strip: 1774x887 RGBA with real alpha above the treeline. The whole sheet is the single piece, reduced by 0.10823 to exactly 192x96. notes.md describes the second generation, which is not used.',
        comments=['One piece, the whole sheet, so the transparent sky above the treeline is kept in the frame.']),
    'lightgallop.layer-near': dict(
        sheet='source-sheet.png', size=(1774, 887), canvas=(192, 96), role='backdrop', whole=True, registration='shared-pivot',
        pivot=(887, 443.5), anchor=(96, 48), runtime=True,
        fit='native canvas 192x96, one frame, repeating horizontally; real alpha above the fence; scrolled fastest; stage 384x224.',
        basis='The first generation (source-sheet.png) is the kept strip: 1774x887 RGBA with real alpha above the fence. The whole sheet is the single piece, reduced by 0.10823 to exactly 192x96. notes.md describes the second generation, which is not used. It is the loudest layer; the stage dims it at draw time.',
        comments=['One piece, the whole sheet. The stage, not the import, dims it (no exposure change is imported).']),
    'lightgallop.road': dict(
        sheet='source-sheet-keyed.png', size=(1448, 1086), canvas=(64, 48), role='prop', whole=True, registration='shared-pivot',
        pivot=(724, 543), anchor=(32, 24), runtime=True,
        fit='native canvas 64x48, a tileable road surface; the lip sits near row 9, grass blades above it in rows 4 to 9; stage 384x224.',
        basis='The delivery was re-run on a flat magenta key; source-sheet-keyed.png (the delivery\'s own copy with the key at alpha 0) is the bound sheet. The whole 1448x1086 sheet is the single piece, reduced by 0.0442 to exactly 64x48.',
        comments=['Bound sheet is source-sheet-keyed.png; the raw magenta-key source-sheet.png is kept alongside it in the delivery.']),
    'lightgallop.bell': dict(
        sheet='source-sheet.png', size=(1536, 1024), canvas=(48, 72), role='prop', registration='shared-feet', runtime=True,
        grid=dict(gap=12), anchor=(24, 72),
        pivots=[(155.5, None), (155.0, None), (155.5, None), (155.0, None), (155.5, None), (156.0, None), (156.0, None)],
        fit='native canvas 48x72, anchor bottom-centre (24,72); stage 384x224; the bronze bell on its timber yoke.',
        basis='Seven drawings in a 4x2 grid (cell 8 empty), measured by alpha because the cell pitch is about 370 px, not 384. Reading order is the delivered one: rest, then ring-0..ring-5 (right 10, 22 and 25 degrees with four sparkles at the peak, then 2 sparkles, left 12 degrees, then centred with one sparkle).',
        comments=['The yoke is the same drawing in every cell: the pivot is the centre of its outer post edges (measured on the top 60 rows) and its foot, so the yoke registers to one pixel grid in all seven frames.',
                  'One shared scale from all seven frames: the sparkles of ring-2 reach 24 source px past the right post, so the bell is drawn at 0.133 (yoke 41 px wide, 52 px tall in the 48x72 canvas).']),
    'lightgallop.hay': dict(
        sheet='source-sheet.png', size=(1536, 1024), canvas=(36, 22), role='prop', registration='shared-feet', runtime=True,
        grid=dict(gap=12), anchor=(18, 22), scale_reference_pieces=[0],
        pivots=[(209.5, None), (219.0, None), (239.5, None), (214.5, None), (214.5, None)],
        fit='native canvas 36x22, anchor bottom-centre (18,22); stage 384x224; a round hay bale on its side.',
        basis='Five drawings in a 3x2 grid (cell 6 empty): rest, then sprung-0..sprung-3. Reading order is the delivered one.',
        comments=['Registered by the bale, not the burst: the pivot is the centre of the bale\'s base and its foot, and the rest bale alone sets the shared scale (0.084: 35 px wide, 22 tall).',
                  'The sprung frames reach above the 22 px canvas (the straw burst is up to 372 source px tall): their highest loose straw is cut by the canvas top. Reviewed: nothing is stretched, and the stage holds the first sprung frame.']),
    'lightgallop.post': dict(
        sheet='source-sheet.png', size=(1536, 1024), canvas=(26, 56), role='prop', registration='shared-feet', runtime=True,
        grid=dict(gap=12), anchor=(13, 56),
        pivots=[(104.0, None), (103.5, None), (103.5, None), (103.5, None), (103.5, None)],
        fit='native canvas 26x56, anchor bottom-centre (13,56); stage 384x224; a wrought-iron lantern post. The painted post is 0.475 wide:tall, so its canvas is 26 wide, not the first 20x56.',
        basis='Five drawings (cells 6-8 empty): dark-0, then lit-0..lit-3 (tall, left, right and narrow flames). Reading order is the delivered one.',
        comments=['Natural aspect: 207x437 source px reduced by 0.1256 to 26x55, no stretch. The canvas widens from 20x56 to 26x56 (slots.json), anchor bottom-centre of the whole post (pole, hook and lantern), so the pole sits about 5 px left of the post\'s column.']),
    'lightgallop.crate': dict(
        sheet='source-sheet.png', size=(1536, 1024), canvas=(24, 24), role='prop', registration='shared-feet', runtime=True,
        grid=dict(gap=12, merge=[[5, 6]]), anchor=(12, 24), scale_reference_pieces=[0, 1, 2],
        pivots=[(None, None), (None, None), (None, None), (None, None), (219.5, 367), (154.5, 306)],
        fit='native canvas 24x24, anchor bottom-centre (12,24); stage 384x224; a lantern crate, drawn as a 3/4 box.',
        basis='Six drawings: shield-0, sparks-0, spur-0, then the break: a cracked crate, a burst and scattered bits. The measured components of the scattered bits (two groups) are one drawing, merged.',
        comments=['The three intact crates set the shared scale (0.0714: 24 px wide, 23 tall). The cracked crate is a little larger and loses under half a pixel at the edge.',
                  'The burst and the scatter are registered by their centres on the canvas centre (pivots below their centres); the burst is wider than the canvas and loses about 4 px each side. The stage draws only the three intact states.']),
    'lightgallop.bramble': dict(
        sheet='source-sheet.png', size=(1774, 887), canvas=(32, 22), role='prop', registration='shared-feet', runtime=True,
        grid=dict(gap=12), anchor=(16, 22),
        pivots=[(414.0, None), (412.0, None)],
        fit='native canvas 32x22, anchor bottom-centre (16,22); stage 384x224; a low thorn tangle (a), and a taller one (b).',
        basis='Two drawings: a, the low tangle, and b, the taller one. Reading order is the delivered one.',
        comments=['One shared scale from both tangles (0.0298): the low one is 25 wide and 14 tall, the size of its 24x14 hitbox, and the tall one fits the 22 px canvas whole.',
                  'The stems are tan, not the near-black the brief asked for; no pixel is repainted.']),
    'lightgallop.sign': dict(
        sheet='source-sheet.png', size=(1774, 887), canvas=(56, 28), role='prop', registration='shared-feet', runtime=False,
        grid=dict(gap=12), anchor=(28, 28),
        pivots=[(413.0, None)],
        fit='native canvas 56x28, anchor bottom-centre (28,28); stage 384x224; a roadside plank on its post.',
        basis='One drawing: a timber board with an arrow-pointed end, one pale arrow symbol and a short post; no letters.',
        comments=['Imported and left off (runtime false): the stage prints the sign\'s own words ("A D RUN", "E JUMP"...) on the plank, and the painted board\'s face is one big arrow, so its words would sit on the arrow. The knob above the board and the thin outline are also lost at 56x28.']),
}


def runs(profile, gap):
    out, start, last, quiet = [], None, None, 0
    for i, v in enumerate(profile):
        if v > 0:
            if start is None:
                start = i
            last, quiet = i, 0
        elif start is not None:
            quiet += 1
            if quiet > gap:
                out.append((start, last))
                start = None
    if start is not None:
        out.append((start, last))
    return out


def measure(sheet_path, gap):
    """Alpha projections: rows of drawings, then each row's columns."""
    im = Image.open(sheet_path).convert('RGBA')
    mask = im.getchannel('A').point(lambda a: 255 if a >= 128 else 0)
    w, h = mask.size
    pieces = []
    for y0, y1 in runs(list(mask.resize((1, h), Image.Resampling.BOX).get_flattened_data()), gap):
        strip = mask.crop((0, y0, w, y1 + 1))
        for x0, x1 in runs(list(strip.resize((w, 1), Image.Resampling.BOX).get_flattened_data()), gap):
            b = mask.crop((x0, y0, x1 + 1, y1 + 1)).getbbox()
            pieces.append([x0 + b[0], y0 + b[1], b[2] - b[0], b[3] - b[1]])
    return mask, pieces


def alpha_area(mask, rect):
    x, y, w, h = rect
    return sum(a >= 128 for a in mask.crop((x, y, x + w, y + h)).get_flattened_data())


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def js(data):
    return (json.dumps(data, indent=2, sort_keys=True) + '\n').encode()


def lightgallop_slot(slot):
    for s in json.loads(SLOTS_JSON.read_text()):
        if s['id'] == slot:
            return s
    raise KeyError(slot)


def pieces_for(slot, spec, sheet_path):
    sheet = Image.open(sheet_path)
    assert sheet.size == spec['size'], f'{slot}: sheet is {sheet.size}'
    mask = sheet.convert('RGBA').getchannel('A').point(lambda a: 255 if a >= 128 else 0)
    if spec.get('whole'):
        return mask, [[0, 0, spec['size'][0], spec['size'][1]]]
    mask, found = measure(sheet_path, spec['grid']['gap'])
    merges = spec['grid'].get('merge', [])
    merged_away = {i for group in merges for i in group[1:]}
    out = []
    for i, rect in enumerate(found):
        if i in merged_away:
            continue
        for group in merges:
            if group[0] == i:
                parts = [found[j] for j in group]
                x0 = min(p[0] for p in parts)
                y0 = min(p[1] for p in parts)
                x1 = max(p[0] + p[2] for p in parts)
                y1 = max(p[1] + p[3] for p in parts)
                rect = [x0, y0, x1 - x0, y1 - y0]
        out.append(rect)
    return mask, out


def frame_names(slot):
    s = lightgallop_slot(slot)
    names = []
    for state, ticks in s['states'].items():
        names += [(state, i, f'{state}-{i}', ticks[i]) for i in range(len(ticks))]
    return s, names


def request_for(slot, spec):
    s, names = frame_names(slot)
    groups = []
    for state, ticks in s['states'].items():
        groups.append(dict(kind='poses', names=[f'{state}-{i}' for i in range(len(ticks))], source_pose_count=len(ticks), state=state))
    timing = '; '.join(f'{state} {len(t)}x{t[0]} ticks' if len(t) > 1 else f'{state} {t[0]} ticks' for state, t in s['states'].items())
    return dict(
        fit=spec['fit'], frame_groups=groups, game='lightgallop', id=slot, role=spec['role'],
        geometry='The road, every collision box and every timing belong to the rules (rules/lightgallop); art never sets a hitbox or the terrain. The stage reads durations from assets/lightgallop/slots.json.',
        timing=f'{timing} (30 Hz). Art never sets a hitbox.',
    )


def prepare(slot, prepared):
    spec = SPECS[slot]
    delivery = DELIVERIES / slot / 'v01'
    out = prepared / slot
    out.mkdir(parents=True, exist_ok=True)
    sheet_src = delivery / spec['sheet']
    mask, rects = pieces_for(slot, spec, sheet_src)
    s, names = frame_names(slot)
    assert len(rects) == len(names), f'{slot}: measured {len(rects)} pieces, {len(names)} frames'
    (out / 'source-sheet.png').write_bytes(sheet_src.read_bytes())
    (out / 'notes.md').write_bytes((delivery / 'notes.md').read_bytes())
    meta = dict(
        file=str(out / 'source-sheet.png'), size_wh=list(spec['size']), alpha_threshold=128,
        measured_by='crates/delve/assets/lightgallop/art-import.py: alpha projections' + (', whole sheet as one piece' if spec.get('whole') else ''),
        warnings=[], pieces=[dict(source_rect_xywh=r, alpha_area=alpha_area(mask, r), index=i) for i, r in enumerate(rects)])
    (out / 'source-sheet.json').write_bytes(js(meta))
    return spec, out, rects, names


def patch_map(slot, spec, rects, names):
    path = OUT / slot / f'{slot}.map.json'
    m = json.loads(path.read_text())
    m['review'] = 'reviewed'
    m['registration'] = spec['registration']
    m['anchor_xy'] = list(spec['anchor'])
    m['anchor_canvas_wh'] = list(spec['canvas'])
    m['sizes'] = {'v1': list(spec['canvas']), 'v2': list(spec['canvas'])}
    m['mapping_basis'] = spec['basis']
    m['comments'] = spec['comments']
    m['runtime'] = bool(spec['runtime'])
    m['embed_kits'] = ['v1']
    if spec.get('scale_reference_pieces'):
        m['scale_reference_pieces'] = spec['scale_reference_pieces']
    for i, p in enumerate(m['pieces']):
        state, frame, name, ticks = names[i]
        assert (p['piece'], p['state'], p['frame'], p['name']) == (i, state, frame, name), (slot, i, p['piece'], p['state'], p['name'])
        x, y, w, h = rects[i]
        if spec.get('whole'):
            pivot = list(spec['pivot'])
        else:
            px, py = spec['pivots'][i]
            pivot = [w / 2 if px is None else px, h if py is None else py]
        p['source_pivot_xy'] = pivot
        if spec.get('grade'):
            p['reviewed_processing'] = {'reviewed': True, 'grade': spec['grade']}
        p['approved'] = True
        p['runtime'] = bool(spec['runtime'])
        p['ticks'] = ticks
        p['label'] = f'{name}: delivered drawing {i + 1} in reading order.'
    path.write_bytes(js(m))


def provenance(slot, spec, out):
    delivery = DELIVERIES / slot / 'v01'
    gen = json.loads((delivery / 'generation.json').read_text())
    prompt = (delivery / 'prompt.txt')
    record = dict(
        delivery=f'{slot}/v01',
        delivery_sheet_path=str(delivery / spec['sheet']),
        delivery_sheet_sha256=digest(delivery / spec['sheet']),
        generation_sha256=digest(delivery / 'generation.json'),
        generations=gen.get('number_of_generations'),
        generator=GEN,
        measured_pieces=f'The delivery has no source-sheet.json. crates/delve/assets/lightgallop/art-import.py measured the pieces from the sheet\'s alpha and wrote source-sheet.json beside a byte-identical copy of the sheet in the prepared folder; source_path, source_sha256 and metadata_sha256 in slot.json are that copy.',
        native_contract=f'Native {spec["canvas"][0]}x{spec["canvas"][1]}.',
        notes_sha256=digest(delivery / 'notes.md'),
        prompt='prompt.txt',
        prompt_sha256=digest(prompt),
        rich_sheets='external, bound by absolute path and SHA-256; only native PNGs, the map, this record and the review sheet are in the repository',
        source_path=str(out / 'source-sheet.png'),
        source_sha256=digest(out / 'source-sheet.png'),
        metadata_sha256=digest(out / 'source-sheet.json'),
        producer_note='PRODUCER-NOTE.md in the delivery (accepted; its cautions are repeated in the map comments)',
    )
    if spec['sheet'] != 'source-sheet.png':
        record['selected_sheet'] = f'{spec["sheet"]} (the delivery\'s own keyed copy of source-sheet.png; the raw magenta-key sheet is not bound)'
    else:
        record['selected_sheet'] = 'source-sheet.png (the first generation; any source-sheet-2.png in the delivery is kept outside the repo and not used)'
    return record


def run_importer(slot, out, check):
    cmd = [sys.executable, '-I', str(IMPORTER), slot, '--source-dir', str(out)]
    if check:
        cmd.append('--check')
    done = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
    sys.stdout.write(done.stdout)
    if done.returncode:
        sys.stderr.write(done.stderr)
        raise SystemExit(f'{slot}: the importer failed')


def do(slot, prepared, check):
    spec, out, rects, names = prepare(slot, prepared)
    dest = OUT / slot
    dest.mkdir(parents=True, exist_ok=True)
    request = js(request_for(slot, spec))
    if check:
        assert (dest / 'request.json').read_bytes() == request, f'stale: {slot}/request.json'
    else:
        (dest / 'request.json').write_bytes(request)
        (dest / 'prompt.txt').write_bytes((DELIVERIES / slot / 'v01' / 'prompt.txt').read_bytes())
    map_path = dest / f'{slot}.map.json'
    if not check:
        if not map_path.exists():
            run_importer(slot, out, False)
        patch_map(slot, spec, rects, names)
    record = js(provenance(slot, spec, out))
    if check:
        assert (dest / 'provenance.json').read_bytes() == record, f'stale: {slot}/provenance.json'
    else:
        (dest / 'provenance.json').write_bytes(record)
    run_importer(slot, out, check)


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('slot', nargs='?')
    p.add_argument('--all', action='store_true')
    p.add_argument('--check', action='store_true')
    p.add_argument('--prepared', type=Path, default=PREPARED)
    a = p.parse_args()
    assert bool(a.slot) != a.all, 'choose a slot or --all'
    for slot in (SPECS if a.all else [a.slot]):
        do(slot, a.prepared, a.check)


if __name__ == '__main__':
    main()
