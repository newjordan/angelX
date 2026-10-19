#!/usr/bin/env python3
"""Reproduce ART-18 entrance rooms and vertical doors through the shared importer."""
import argparse
from collections import deque
import hashlib
import json
from pathlib import Path
import runpy
from types import SimpleNamespace
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
IMPORT = runpy.run_path(str(ROOT / 'scripts/slot-art-import.py'))
BANK = ROOT / 'crates/delve/assets/art-slots'
SOURCES = Path('/work/artifacts/art-prod/slots')
ROOMS = ['cinder', 'latch', 'mossbell', 'brasswake', 'rafterwake', 'pennon']
PACKS = ['crypt', 'mines', 'keep', 'fungal', 'unknown']

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def write(path, value, check=False):
    IMPORT['write'](path, IMPORT['js'](value), check)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=Path('/work/tmp/int-4-prepared'))
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--slots', nargs='+', choices=ROOMS + PACKS,
                        help='Reimport only the named rooms or door packs.')
    args = parser.parse_args()
    acceptance = json.loads(Path('/work/artifacts/delve-int-20261015/art-18/acceptance.json').read_text())
    records = []
    for name, room in [(n, True) for n in ROOMS] + [(n, False) for n in PACKS]:
        if args.slots and name not in args.slots:
            continue
        detailed_room = room and name in ('latch', 'rafterwake', 'pennon')
        delivery = f'{name}.entrance.room-1' if room else f'dgn.sidedoor.{name}.vertical-1'
        slot = f'{name}.entrance' if room else f'dgn.sidedoor-{name}'
        source = SOURCES / delivery / 'v01'
        original = source / 'source-sheet.png'
        assert acceptance[delivery]['accepted_file'] == str(original)
        assert sha(original) == acceptance[delivery]['source_sha256']
        meta = json.loads((source / 'source-sheet.json').read_text())
        sheet = Image.open(original).convert('RGBA')
        if room:
            meta['pieces'] = [dict(index=0, source_rect_xywh=[0, 0, sheet.width, sheet.height], alpha_area=sheet.width*sheet.height)]
            meta['warnings'] = ['Opaque room: explicit full-sheet measurement; original metadata retained in provenance.']
        # Threshold before reduction; retain the largest connected alpha body
        # in each measured door crop, dropping detached chips and source fringe.
        pixels = list(sheet.get_flattened_data())
        pixels = [c[:3] + (255,) if c[3] >= 128 else (0, 0, 0, 0) for c in pixels]
        sheet.putdata(pixels)
        removed = 0
        if not room:
            for piece in meta['pieces']:
                x, y, w, h = piece['source_rect_xywh']
                crop = sheet.crop((x, y, x+w, y+h))
                opaque = {(xx, yy) for yy in range(h) for xx in range(w) if crop.getpixel((xx, yy))[3]}
                components = []
                while opaque:
                    start = opaque.pop(); component = {start}; queue = deque([start])
                    while queue:
                        xx, yy = queue.popleft()
                        for p in [(xx-1, yy), (xx+1, yy), (xx, yy-1), (xx, yy+1)]:
                            if p in opaque:
                                opaque.remove(p); component.add(p); queue.append(p)
                    components.append(component)
                keep = max(components, key=len)
                for component in components:
                    if component is not keep:
                        removed += len(component)
                        for xx, yy in component:
                            sheet.putpixel((x+xx, y+yy), (0, 0, 0, 0))
        prepared = args.output / slot
        prepared.mkdir(parents=True, exist_ok=True)
        sheet.save(prepared / 'source-sheet.png')
        (prepared / 'source-sheet.json').write_bytes(IMPORT['js'](meta))
        (prepared / 'notes.md').write_bytes((source / 'notes.md').read_bytes())
        sizes = {'v1': [384, 224], 'v2': [768, 448]} if room else {'v1': [16, 32], 'v2': [32, 64]}
        req = dict(id=slot, game=name if room else 'dungeon', role='backdrop' if room else 'prop',
                   fit='384x224; v2 768x448' if room else '16x32; v2 32x64',
                   timing='Static; existing room entry and door state only.',
                   geometry='Original collision tiles, exits and world anchors remain authoritative.',
                   frame_groups=[dict(state='room' if room else 'door', names=['room'] if room else ['west-closed', 'west-open', 'west-locked', 'east-closed', 'east-open', 'east-locked'], source_pose_count=1 if room else 12)])
        dest = BANK / slot
        dest.mkdir(parents=True, exist_ok=True)
        write(dest / 'request.json', req, args.check)
        pieces = []
        for i, piece in enumerate(meta['pieces']):
            rect = piece['source_rect_xywh']
            label = 'room' if room else ['west-closed', 'west-open', 'west-locked', 'east-closed', 'east-open', 'east-locked'][i % 6] + ('-worn' if i >= 6 else '')
            pieces.append(dict(piece=piece['index'], source_rect_xywh=rect,
                               source_pivot_xy=[rect[2]/2, rect[3]/2], name=label, state='room' if room else 'door',
                               frame=i, approved=True, runtime=room or i<6, label=label,
                               sampling='cover' if room else 'contain',
                               grade={'value': .55 if detailed_room else .62 if room else .80, 'saturation': .65 if room else .72},
                               **({'median_filter': 3, 'reviewed_processing': {'reviewed': True, 'grade': {'0d1a0c': '322b2b', '182414': '322b2b', '263b1c': '322b2b'}}} if room and not detailed_room else {})))
        mapping = dict(schema_version=1, slot=slot, review='reviewed', mapping_basis='ART-18 accepted source; measured reading order reviewed against notes',
                       source_sha256=sha(prepared/'source-sheet.png'), notes_sha256=sha(prepared/'notes.md'),
                       registration='own-bounds' if room else 'shared-pivot', size_basis=req['fit'], sizes=sizes,
                       anchor_xy=[v/2 for v in sizes['v1']], anchor_canvas_wh=sizes['v1'],
                       expected_existing_pieces=1 if room else 12, delivered_pieces=len(pieces),
                       timing=req['timing'], geometry=req['geometry'], pieces=pieces, runtime=True, embed_kits=['v1', 'v2'],
                       comments=['Exact native BOX conversion; binary alpha 128; existing shared palette.',
                                 'Original master interior retained; actual exits, collision furniture and complete title plaques preserved by the room painter.' if detailed_room else
                                 'Walkable perimeter, actual exits and collision furniture are masked by the room painter.' if room else 'First six states active; worn alternatives retained as proposals. Fixed shared centre/scale across all states.'])
        write(dest / f'{slot}.map.json', mapping, args.check)
        provenance = dict(delivery=delivery, source_path=str(original), source_sha256=sha(original), metadata_sha256=sha(source/'source-sheet.json'),
                          prepared_sha256=sha(prepared/'source-sheet.png'), detached_alpha_pixels_removed=removed,
                          recipe_sha256=sha(Path(__file__)), native_contract=sizes)
        write(dest / 'provenance.json', provenance, args.check)
        options = SimpleNamespace(source_dir=prepared, v1_height=None, v2_height=None, check=args.check, terminal_metrics=None)
        record = IMPORT['run'](slot, req, options, 'OK', None)
        records.append({'slot': slot, 'source_sha256': provenance['source_sha256'], 'native_outputs': len(record['outputs']), 'flags': record['flags']})
        print(slot, record['flags'])
    write(args.output / 'imports.json', records)

if __name__ == '__main__':
    main()
