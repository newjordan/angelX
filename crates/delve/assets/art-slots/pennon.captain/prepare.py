"""Freeze the reviewed Captain rerun and append standing Sentry damage crops."""
import argparse
import hashlib
import json
from pathlib import Path
from PIL import Image

SOURCE = Path('/work/artifacts/art-prod/slots')
EXPECTED = {
    'pennon.captain': '02f5b64c2df71643c7a9c57668af282f16367be2923a2630fb262c06361fd067',
    'pennon.sentry': '4a0dc90339b6b402d19caa3e6c4ac9be0027aa84b975207f2afcb06f61387067',
    'pennon.sentry-damaged': '5387992d1f31736d95571eb759d1f6cb8af8054f0b8a3a36cbf4ffa0f841c778',
}

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def prepare(output):
    originals = {}
    for slot, expected in EXPECTED.items():
        path = SOURCE / slot / 'v01/source-sheet.png'
        actual = sha(path)
        if expected is not None:
            assert actual == expected, f'{slot}: source changed; review before adopting'
        originals[slot] = {'path': str(path), 'sha256': actual}
    for slot in ['pennon.captain', 'pennon.sentry']:
        dest = output / slot
        dest.mkdir(parents=True, exist_ok=True)
        im = Image.open(originals[slot]['path']).convert('RGBA')
        meta = json.loads((SOURCE / slot / 'v01/source-sheet.json').read_text())
        if slot == 'pennon.sentry':
            damage = Image.open(originals['pennon.sentry-damaged']['path']).convert('RGBA')
            combined = Image.new('RGBA', (max(im.width, damage.width), im.height + damage.height))
            combined.paste(im, (0, 0))
            combined.paste(damage, (0, im.height))
            extra = json.loads((SOURCE / 'pennon.sentry-damaged/v01/source-sheet.json').read_text())
            for p in extra['pieces']:
                p['index'] += len(meta['pieces'])
                p['source_rect_xywh'][1] += im.height
            meta['pieces'] += extra['pieces']
            im = combined
        im.save(dest / 'source-sheet.png')
        meta['size_wh'] = list(im.size)
        meta['file'] = str(dest / 'source-sheet.png')
        meta['piece_count'] = len(meta['pieces'])
        (dest / 'source-sheet.json').write_text(json.dumps(meta, indent=2) + '\n')
        names = ['Idle', 'Hook windup', 'Hook action', 'Hook recovery', 'Banner windup', 'Banner slam', 'Walk A', 'Walk B', 'Hit flinch', 'Standing damage 1', 'Standing damage 2', 'Defeat'] if slot == 'pennon.captain' else [f'Original pose {i+1}' for i in range(12)] + [f'Standing damage pose {i+1}' for i in range(12)]
        (dest / 'notes.md').write_text('\n'.join(f'{i+1}. {n}' for i, n in enumerate(names)) + '\n')
        (dest / 'provenance.json').write_text(json.dumps(originals, indent=2) + '\n')

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    prepare(parser.parse_args().output)
