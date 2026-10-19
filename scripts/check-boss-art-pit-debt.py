#!/usr/bin/env python3
"""Actual-cell silhouettes, four-stage geometry and protected art checks."""
import argparse
import hashlib
import json
import runpy
import copy
import contextlib
import io
import tempfile
import statistics
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
NAMES = ('red-pick-chief', 'the-foreman', 'late-fee-leviathan')
CELL = runpy.run_path(str(ROOT / 'scripts/check-boss-art-sendback.py'))


def impact_cell_gate(captures, names=NAMES, frozen=False):
    """Actual emitted glyph masks, excluding the same tick's effect-free room."""
    geometry = json.loads((captures / 'review-layout.json').read_text())['bosses']
    errors = []
    for name in names:
        for kit in (1, 2):
            masks = []
            for stage in range(5):
                stem = captures / f'{name}-effects-{stage}-v{kit}'
                raw = json.loads(Path(str(stem) + '-cells.json').read_text())
                bg = json.loads(Path(str(stem) + '-background-cells.json').read_text())
                masks.append(CELL['shape'](raw, bg, geometry[name]['impact']))
            if frozen:
                masks = [masks[0]] * 5
            for stage in range(1, 5):
                changed = sum(masks[stage][q] != masks[stage-1][q] for q in masks[stage])
                print(f'{name} v{kit}: impact {stage-1}->{stage} changes {changed} actual cell silhouettes')
                if not changed:
                    errors.append(f'{name} v{kit}: impact {stage-1}/{stage} repeats a cell silhouette')
    return errors


def effect_brightness_gate(captures, dim_v2=False):
    geometry = json.loads((captures / 'review-layout.json').read_text())['bosses']
    errors = []
    art = CELL['ART']
    for name in NAMES:
        for role, count in [('projectile', 2), ('tell', 4), ('impact', 5)]:
            for stage in range(count):
                values = []
                for kit in (1, 2):
                    stem = captures / f'{name}-effects-{stage}-v{kit}'
                    raw = json.loads(Path(str(stem) + '-cells.json').read_text())
                    bg = json.loads(Path(str(stem) + '-background-cells.json').read_text())
                    if dim_v2 and kit == 2:
                        for cell in raw:
                            for channel in ('fg', 'bg'):
                                cell[channel] = [v // 2 for v in cell[channel]]
                    mask = CELL['shape'](raw, bg, geometry[name][role])
                    light = [art['lightness'](art['terminal_colour'](raw[y*96+x], dx, dy))
                             for (x,y), bits in mask.items() for dy in range(4) for dx in range(2)
                             if bits & (1 << (dy*2+dx))]
                    values.append(statistics.median(light) if light else 0)
                print(f'{name}: {role} {stage} cell median V1 {values[0]:.2f}, V2 {values[1]:.2f} L*')
                if values[1] + 1e-6 < values[0]:
                    errors.append(f'{name}: {role} {stage} V2 ink below V1 brightness')
    return errors


def negative_controls(output):
    entries = json.loads((ROOT / 'scripts/boss-art-pit-debt.json').read_text())
    frozen = copy.deepcopy(entries)
    for entry in frozen:
        entry['frames']['tell'] = [entry['frames']['tell'][0]] * 4
    with contextlib.redirect_stdout(io.StringIO()):
        errors = CELL['ART']['numeric_gates'](frozen)
    assert sum('identical authored tell' in error for error in errors) == 9
    print('Repeated authored stage: rejects all nine transitions')
    white = copy.deepcopy(entries)
    spec = white[0]['frames']['tell'][0]
    rows = list(spec['rows']); rows[5] = rows[5][:5] + 'tt' + rows[5][7:]; spec['rows'] = rows
    spec['pigments']['t'] = '#fafafa'
    with contextlib.redirect_stdout(io.StringIO()):
        errors = CELL['ART']['numeric_gates'](white)
    assert any('near-white cluster' in error for error in errors)
    print('Near-white connected core: rejects')
    geometry = json.loads((output / 'terminal/review-layout.json').read_text())['bosses']
    with tempfile.TemporaryDirectory(prefix='boss-art-frozen-', dir='/work/tmp') as temporary:
        target = Path(temporary)
        for path in (output / 'terminal').iterdir():
            (target / path.name).symlink_to(path)
        for name in NAMES:
            box = geometry[name]['tell']; ignore = geometry[name]['tell_ignore']
            for kit in (1, 2):
                base = json.loads((target / f'{name}-effects-0-v{kit}-cells.json').read_text())
                for stage in range(1, 4):
                    path = target / f'{name}-effects-{stage}-v{kit}-cells.json'
                    raw = json.loads(path.read_text()); path.unlink()
                    for y in range(box[1] // 16, (box[3] + 15) // 16):
                        for x in range(box[0] // 8, (box[2] + 7) // 8):
                            if not any(a // 8 <= x < (c + 7) // 8 and b // 16 <= y < (d + 15) // 16 for a, b, c, d in ignore):
                                raw[y * 96 + x] = base[y * 96 + x]
                    path.write_text(json.dumps(raw))
        with contextlib.redirect_stdout(io.StringIO()):
            errors, _ = CELL['measure'](target, ROOT / 'scripts/boss-art-pit-debt.json', NAMES)
        assert sum('tell ' in error and 'shape collapsed' in error for error in errors) == 18, errors
    print('Frozen cell warnings with live boss/HUD exclusions: rejects all eighteen transitions')
    with contextlib.redirect_stdout(io.StringIO()):
        errors = impact_cell_gate(output / 'terminal', frozen=True)
    assert len(errors) == 24, errors
    print('Frozen actual-cell impacts: rejects all twenty-four consecutive frame transitions')
    with contextlib.redirect_stdout(io.StringIO()):
        errors = effect_brightness_gate(output / 'terminal', dim_v2=True)
    assert len(errors) == 33, errors
    print('Dimmed V2 actual cells: rejects all thirty-three projectile/tell/impact frames')


def check(output):
    errors, metrics = CELL['measure'](output / 'terminal', ROOT / 'scripts/boss-art-pit-debt.json', NAMES)
    errors += impact_cell_gate(output / 'terminal')
    errors += effect_brightness_gate(output / 'terminal')
    geometry = json.loads((output / 'terminal/review-layout.json').read_text())['bosses']
    name = NAMES[2]
    g = geometry[name]
    for kit in (1, 2):
        masks = []
        for stage in range(4):
            stem = output / 'terminal' / f'{name}-effects-{stage}-v{kit}'
            raw = json.loads(Path(str(stem) + '-cells.json').read_text())
            bg = json.loads(Path(str(stem) + '-background-cells.json').read_text())
            masks.append(CELL['shape'](raw, bg, g['laser']))
        for stage in range(1, 4):
            active = [q for q in masks[stage] if masks[stage][q] or masks[stage - 1][q]]
            changed = sum(masks[stage][q] != masks[stage - 1][q] for q in active)
            ratio = changed / len(active) if active else 0
            print(f'{name} v{kit}: Laser {stage-1}->{stage} glyphs {changed}/{len(active)} = {ratio:.1%} (need >=30%)')
            if ratio < .30:
                errors.append(f'{name} v{kit}: Laser shape stage {stage} collapsed')
    protected = json.loads((output / 'protected-hashes.json').read_text())
    assigned = {f'cockpit/assets/dungeon/bosses/{name}.boss' for name in NAMES}
    for path, digest in protected.items():
        if path not in assigned and hashlib.sha256((ROOT / path).read_bytes()).hexdigest() != digest:
            errors.append(f'protected bytes changed: {path}')
    print(f'{len(protected)-len(assigned)} unassigned assets / recipes / master ink: exact bytes')
    entries = json.loads((ROOT / 'scripts/boss-art-pit-debt.json').read_text())
    for entry in entries:
        impact_masks = [tuple(k != '.' for row in frame['rows'] for k in row) for frame in entry['frames']['impact']]
        if len(set(impact_masks)) != 5:
            errors.append(f'{entry["boss"]}: impact repeats a native silhouette')
        for role, frames in entry['frames'].items():
            for stage, spec in enumerate(frames):
                colours = CELL['ART']['frame_inks'](spec, CELL['ART']['palette']())
                peak = max(CELL['ART']['lightness'](colours[k]) for row in spec['rows'] for k in row if k != '.')
                if peak >= 80:
                    errors.append(f'{entry["boss"]}: {role} {stage} brightest ink {peak:.2f} >=80 L*')
        path = Path(entry['boss'])
        before = (output / 'before/bosses' / path.name).read_text().split('\nart\n')
        after = (ROOT / path).read_text().split('\nart\n')
        assert before[0] == after[0], f'{path}: header changed'
        old, new = before[1].splitlines(), after[1].splitlines()
        assert (len(old), list(map(len, old))) == (len(new), list(map(len, new))), path
        assert [[k != '.' for k in row] for row in old] == [[k != '.' for k in row] for row in new], path
        for kit in (1, 2):
            im = Image.open(ROOT / entry['output'] / f'v{kit}.png').convert('RGBA')
            assert im.size == (len(old[0]) * kit, len(old) * kit)
            assert all(bool(im.getpixel((x, y))[3]) == (old[y // kit][x // kit] != '.') for y in range(im.height) for x in range(im.width))
        print(f'{path.stem}: grid, alpha, feet, header/numbers and doubled study mask exact')
    count = 0
    for before in (output / 'before/terminal').iterdir():
        if before.name.startswith(NAMES) or before.name in ('review-layout.json', 'room-fixtures.json'):
            continue
        if before.read_bytes() != (output / 'terminal' / before.name).read_bytes():
            errors.append(f'protected room/cell export changed: {before.name}')
        count += 1
    print(f'{count} approved room / cell exports: exact bytes')
    families = {}
    for name in (*NAMES, 'cinderjaw', 'the-index', 'mother-of-spores', 'waxen-warden', 'the-bone-choir', 'crypt-tithe-marshal', 'crypt-tax-keeper', 'crypt-relic-liege', 'iron-tusk-chief'):
        directory = ROOT / 'cockpit/assets/dungeon/bosses' / name
        families[name] = {role: [Image.open(directory / f'{role}{suffix}.png').convert('RGBA').getchannel('A').tobytes()
                                for suffix in (['', '-1'] if role == 'projectile' else ['', '-1', '-2', '-3'] if role == 'tell' else ['', '-1', '-2', '-3', '-4'])]
                          for role in ('projectile', 'tell', 'impact')}
    for name in NAMES:
        for other in families:
            if name == other:
                continue
            for role in families[name]:
                if families[name][role] == families[other][role]:
                    errors.append(f'{name}/{other}: shared {role} silhouette family')
    for error in errors:
        print('FAIL: ' + error)
    print(f'{len(errors)} pit/debt failure(s)')
    (output / 'cell-metrics.json').write_text(json.dumps(metrics, indent=2) + '\n')
    return errors


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--negative-controls', action='store_true')
    args = parser.parse_args()
    if args.negative_controls:
        negative_controls(args.output)
        raise SystemExit(0)
    raise SystemExit(bool(check(args.output)))
