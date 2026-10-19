#!/usr/bin/env -S uv run --no-project --with Pillow python
"""Retain measured, lossless source crops from the read-only boss library."""
import argparse
import hashlib
import json
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_LIBRARY = Path('./cockpit/assets/boss-library/existing')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def retain(boss, library):
    folder = library / boss / 'v01'
    metadata = json.loads((folder / 'source-sheet.json').read_text())
    sheet = folder / metadata['image']
    assert digest(sheet) == metadata['sha256'], 'source sheet hash changed'
    image = Image.open(sheet).convert('RGBA')
    assert list(image.size) == metadata['size']
    plan = json.loads((library / 'production-plan.json').read_text())
    design = next(e for e in plan['entries'] if e['id'] == boss)
    definition = ROOT / 'cockpit/assets/dungeon/bosses' / (boss + '.boss')
    header = definition.read_text().split('\nart\n')[0]
    assert header == design['definition_header'], 'source rules/header changed'
    output = ROOT / 'crates/delve/assets/boss-library-src' / boss
    output.mkdir(parents=True, exist_ok=True)
    frames = []
    for frame in metadata['frames']:
        assert frame['foreign_core_pixels_in_crop'] == 0
        x, y, w, h = frame['source_rect_xywh']
        assert 0 <= x < x+w <= image.width and 0 <= y < y+h <= image.height
        path = output / (frame['id'].replace('/', '-') + '.png')
        image.crop((x, y, x+w, y+h)).save(path)
        frames.append(dict(frame, path=path.name, sha256=digest(path)))
    record = dict(id=boss, source_path=str(sheet), source_sha256=digest(sheet),
                  source_size=list(image.size), identity=design['identity'],
                  definition_header=header,
                  source_definition_sha256=design['definition_sha256'],
                  definition_sha256=digest(definition), frames=frames,
                  runtime_mapping={
                      'idle': 'overhead/idle plus existing one-sample breath',
                      'windup': ('overhead/anticipate during existing signature INHALE/TELL or ring/arms warning'
                                 if boss in ('grinwick','the-big-ripper') else
                                 'overhead/anticipate during ring_warning or arms_warning only'),
                      'attack': 'existing idle/breath; no new attack animation or timing',
                      'hurt': 'existing two cached hit grades on the selected pose',
                      'rage': 'existing heat flash; anticipation only during real warnings',
                      'unwired': 'side keyposes and all detached parts await camera/socket registration',
                  })
    (output / 'provenance.json').write_text(json.dumps(record, indent=2) + '\n')
    print(f'{boss}: retained {len(frames)} exact crops; sheet {digest(sheet)}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('boss', nargs='+')
    parser.add_argument('--library', type=Path, default=DEFAULT_LIBRARY)
    args = parser.parse_args()
    for boss in args.boss:
        retain(boss, args.library)
