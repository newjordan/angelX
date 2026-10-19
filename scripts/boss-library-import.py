#!/usr/bin/env -S uv run --no-project --with Pillow python
"""Import checked library crops without touching their read-only source.

UV_CACHE_DIR=/work/tmp/uv-cache uv run --no-project --with Pillow \
    scripts/boss-library-import.py [boss-id] [--check] [--study OUTPUT]
The local crops and provenance suffice; no live tree or network is needed.
"""
import argparse
import hashlib
import json
import re
from collections import Counter
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'crates/delve/assets/boss-library-src'
OUTPUT = ROOT / 'cockpit/assets/dungeon/bosses'
BOSSES = json.loads((ROOT / 'scripts/boss-library-registry.json').read_text())
LIMITS = {'silhouette_iou': .98, 'key_colour_agreement': .90, 'edge_agreement': .85}
FILTERS = {n: getattr(Image.Resampling, n.upper()) for n in ('nearest', 'box', 'lanczos')}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def palette():
    text = (ROOT / 'crates/delve/src/render/overworld/ink.rs').read_text()
    bank = text.split('pub const PALETTE: [u32; 75] = [')[1].split('];')[0]
    # Signal ivory and black cannot become decorative cores or dead contours.
    return [tuple(bytes.fromhex(h)) for h in re.findall(r'0x([0-9a-f]{6})', bank)
            if h != '000000' and min(bytes.fromhex(h)) < 240]


def distance(a, b):
    return sum((x-y)**2 for x, y in zip(a[:3], b[:3]))


def source(boss, camera, pose):
    folder = SOURCE / boss
    record = json.loads((folder / 'provenance.json').read_text())
    assert digest(OUTPUT / (boss + '.boss')) == record['definition_sha256'], 'definition changed'
    frame = next(f for f in record['frames'] if f['id'] == f'{camera}/{pose}')
    path = folder / frame['path']
    assert digest(path) == frame['sha256'], f'crop hash changed: {path}'
    im = Image.open(path).convert('RGBA')
    # Clear RGB under transparent paper; retain original visible alpha until sampling.
    im.putdata([c if c[3] else (0, 0, 0, 0) for c in im.get_flattened_data()])
    return im


def sample(im, size, method='box'):
    """One scale per camera bank; shared canvas keeps both feet registered."""
    scale = min(size[0] / im.width, size[1] / im.height)
    dims = (max(1, round(im.width * scale)), max(1, round(im.height * scale)))
    scaled = im.resize(dims, FILTERS[method])
    out = Image.new('RGBA', size)
    out.paste(scaled, ((size[0]-scaled.width)//2, size[1]-scaled.height))
    return out


def convert(reference):
    inks = palette()
    out = Image.new('RGBA', reference.size)
    pixels = out.load()
    for y in range(out.height):
        for x in range(out.width):
            c = reference.getpixel((x, y))
            if c[3] < 128:
                continue
            # Source contour and glints enter the same nearest-ink search;
            # no posterization, dithering, smoothing or global exposure gain.
            pixels[x, y] = min(inks, key=lambda ink: distance(c, ink)) + (255,)
    # Merge only isolated interior tone flecks. Keep source silhouette, dark
    # contour, bright glints and all >=3px linear seam features verbatim.
    seen = set()
    for y in range(out.height):
        for x in range(out.width):
            if (x, y) in seen or not pixels[x, y][3]:
                continue
            colour = pixels[x, y]
            todo = [(x, y)]; group = set()
            while todo:
                q = todo.pop()
                if q in group:
                    continue
                group.add(q); seen.add(q)
                for n in neighbours(*q, out.size):
                    if pixels[n] == colour and n not in group:
                        todo.append(n)
            if len(group) > 3 or max(colour[:3]) >= 190 or max(colour[:3]) < 80:
                continue
            if len(group) >= 3 and (len({p[0] for p in group}) == 1 or len({p[1] for p in group}) == 1):
                continue
            border = [pixels[n] for q in group for n in neighbours(*q, out.size) if n not in group]
            if not border or any(not c[3] for c in border):
                continue
            adjacent = Counter(border)
            replacement = min(adjacent, key=lambda c: distance(c, colour) - 36 * adjacent[c])
            if distance(replacement, colour) <= 2400:
                for q in group:
                    pixels[q] = replacement
    return out


def neighbours(x, y, size):
    return [(a, b) for a, b in ((x-1,y), (x+1,y), (x,y-1), (x,y+1))
            if 0 <= a < size[0] and 0 <= b < size[1]]


def fidelity(reference, native):
    assert reference.size == native.size
    a = {(x,y) for y in range(reference.height) for x in range(reference.width)
         if reference.getpixel((x,y))[3] >= 128}
    b = {(x,y) for y in range(native.height) for x in range(native.width)
         if native.getpixel((x,y))[3] >= 128}
    common = a & b
    edge = {p for p in a if any(n not in a for n in neighbours(*p, reference.size))}
    agree = lambda points: sum(distance(reference.getpixel(p), native.getpixel(p)) <= 3600
                               for p in points) / max(1, len(points))
    return {'silhouette_iou': len(common)/max(1,len(a|b)),
            'key_colour_agreement': agree(a), 'edge_agreement': agree(edge),
            'colours': len(set(c[:3] for c in native.get_flattened_data() if c[3])),
            'opaque_pixels': len(b)}


def bank(boss, camera):
    """Pad crops at their measured foot line without independently stretching poses."""
    poses = [source(boss, camera, p) for p in ('idle', 'anticipate')]
    size = (max(p.width for p in poses), max(p.height for p in poses))
    out = []
    for im in poses:
        canvas = Image.new('RGBA', size)
        canvas.paste(im, ((size[0]-im.width)//2, size[1]-im.height))
        out.append(canvas)
    return out


def write(path, data, check):
    if check:
        assert path.read_bytes() == data, f'stale output: {path}'
    elif not path.exists() or path.read_bytes() != data:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)


def png_bytes(im):
    import io
    buf = io.BytesIO(); im.save(buf, format='PNG', optimize=False)
    return buf.getvalue()


def run(boss, check=False, study=None):
    config = BOSSES[boss]; rows = bank(boss, 'overhead'); metrics = {}; rejected = []
    for kit in ('v1', 'v2'):
      sizes = [('', config[kit])] + [(f'-study-{i+1}', size) for i, size in enumerate(config['study'][kit])]
      for suffix, size in sizes:
        for pose, im in zip(('rest', 'windup'), rows):
            ref = sample(im, size); native = convert(ref)
            score = fidelity(ref, native)
            for name, threshold in LIMITS.items():
                if score[name] < threshold:
                    failure = f'{boss}/{kit}/{pose}: {name} {score[name]:.4f} < {threshold}'
                    assert not config.get('active', True), failure
                    rejected.append(failure)
            assert all(min(c[:3]) < 240 for c in native.get_flattened_data() if c[3])
            name = f'library-{kit}{suffix}-{pose}.png'
            write(OUTPUT / boss / name, png_bytes(native), check)
            metrics[name] = dict(score, size=size, sha256=hashlib.sha256(png_bytes(native)).hexdigest())
    record = {'boss': boss, 'camera': 'overhead', 'resample': 'box', 'thresholds': LIMITS,
              'display_policy': config,
              'identity_props': config['props'], 'definition_sha256': digest(OUTPUT/(boss+'.boss')),
              'outputs': metrics}
    if rejected: record['gate_failures'] = rejected
    write(OUTPUT/boss/'library-import.json', (json.dumps(record,indent=2,sort_keys=True)+'\n').encode(), check)
    if study:
        study.mkdir(parents=True, exist_ok=True)
        # Nine images per sheet; each is the actual sampled grain at 3x.
        sheet = Image.new('RGB', (1510, 1260), (8,11,14)); d = ImageDraw.Draw(sheet)
        for i, method in enumerate(FILTERS):
            for j, height in enumerate((72,96,128)):
                size = (round(rows[0].width*height/rows[0].height), height)
                ref = sample(rows[0], size, method); im = convert(ref)
                q = im.resize((im.width*3, im.height*3), Image.Resampling.NEAREST)
                sheet.paste(q,(j*500+10,i*420+30),q)
                score = fidelity(ref, im)
                d.text((j*500+10,i*420+10),f'{boss} {method} {size} / edge {score["edge_agreement"]:.3f}',fill='wheat')
        sheet.save(study/(boss+'-resampling.png'))
    return record


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('boss', nargs='?', choices=list(BOSSES))
    p.add_argument('--check', action='store_true')
    p.add_argument('--study', type=Path)
    a = p.parse_args()
    for boss in ([a.boss] if a.boss else BOSSES):
        print(json.dumps(run(boss,a.check,a.study),sort_keys=True))

if __name__ == '__main__':
    main()
