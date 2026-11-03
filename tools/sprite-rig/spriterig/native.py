"""Authoritative Delve ink-frame export and static worn-asset authoring.

No body art is reconstructed here. Rust exports the actual native canvas and
resolved Body for every pose; knight-fit.json is the sole authoring fit table.
Coordinates and fits precede mirroring; exported rows/PNGs are final pixels.
"""
import argparse
import copy
import json
import os
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

from .delve.source import palette

PARTS = ('head', 'face', 'shoulders', 'body', 'back', 'left', 'right', 'feet', 'aura')
ALIASES = dict(helm='head', mask='face', cape='back', chest='body', centre='body',
               center='body', shield='left', hand='right', hands='right',
               weapon='right', legs='feet')
TICKS_PER_FRAME = 6
FIT_PATH = Path('crates/delve/assets/knight-fit.json')


def root_path(root=None):
    """Explicit root, environment, enclosing checkout. Never a fabricated table."""
    if root is not None:
        return Path(root).resolve()
    if os.environ.get('ANGELX_ROOT'):
        return Path(os.environ['ANGELX_ROOT']).resolve()
    for parent in Path(__file__).resolve().parents:
        if (parent / 'crates/delve').is_dir():
            return parent
    raise ValueError('cannot find angelX; supply --angelx or ANGELX_ROOT')


def read_json(path):
    with Path(path).open(encoding='utf-8') as stream:
        return json.load(stream)


def write_json(path, data):
    Path(path).write_text(json.dumps(data, indent=2) + '\n', encoding='utf-8')


def _ints(value, n, label):
    if not isinstance(value, list) or len(value) != n or any(type(v) is not int for v in value):
        raise ValueError(f'{label}: expected {n} integers')
    return value


def validate_fit(fit, label='fit'):
    if not isinstance(fit, dict):
        raise ValueError(f'{label}: expected a Body object (not nested parts)')
    bounds = _ints(fit.get('bounds'), 4, f'{label}.bounds')
    if min(bounds[2:]) < 1:
        raise ValueError(f'{label}: bounds must have positive width/height')
    for name in PARTS:
        part = fit.get(name)
        if not isinstance(part, dict):
            raise ValueError(f'{label}: missing part {name}')
        _ints(part.get('anchor'), 2, f'{label}.{name}.anchor')
        size = _ints(part.get('size'), 2, f'{label}.{name}.size')
        if min(size) < 1:
            raise ValueError(f'{label}.{name}: size must be positive')
        if 'stretch' in part and type(part['stretch']) is not bool:
            raise ValueError(f'{label}.{name}: stretch must be boolean')
        if any(type(part.get(k)) is not bool for k in ('bottom', 'under')):
            raise ValueError(f'{label}.{name}: bottom and under must be booleans')
    return fit


def load_fits(root=None):
    path = root_path(root) / FIT_PATH
    data = read_json(path)
    if not isinstance(data, dict):
        raise ValueError(f'{path}: expected a versioned fit-table object')
    if type(data.get('version')) is not int or data['version'] != 1:
        raise ValueError(f'{path}: unsupported fit-table version')
    profiles = data.get('profiles')
    if not isinstance(profiles, dict) or not profiles:
        raise ValueError(f'{path}: missing profiles')
    for name, fit in profiles.items():
        validate_fit(fit, f'profiles.{name}')
    return profiles


def slot_name(name):
    name = ALIASES.get(name, name)
    if name not in (*PARTS, 'whole'):
        raise ValueError(f'unknown worn slot {name!r}')
    return name


def body_part(fit, slot):
    slot = slot_name(slot)
    if slot != 'whole':
        return fit[slot]
    x, y, w, h = fit['bounds']
    return dict(anchor=[x + w // 2, y + h // 2], size=[w, h], bottom=False, under=False)


def frame_key(frame):
    return tuple(frame[k] for k in ('kit', 'view', 'pose', 'tick'))


def validate_rows(rows, pal, label):
    if not isinstance(rows, list) or not rows or any(not isinstance(r, str) for r in rows):
        raise ValueError(f'{label}: expected nonempty ink rows')
    unknown = set(''.join(rows)) - set(pal) - {'.', ' '}
    if unknown:
        raise ValueError(f'{label}: unknown inks {sorted(unknown)}')


def load_native(path, pal):
    data = read_json(path)
    if not isinstance(data, dict):
        raise ValueError(f'{path}: expected a native-frame manifest object')
    frames = data.get('frames')
    if not isinstance(frames, list) or not frames:
        raise ValueError(f'{path}: no native frames')
    seen = set()
    for i, frame in enumerate(frames):
        label = f'{path}: frame {i}'
        if not isinstance(frame, dict):
            raise ValueError(f'{label}: expected a frame object')
        if any(not isinstance(frame.get(k), str) or not frame[k] for k in ('kit', 'view', 'pose')):
            raise ValueError(f'{label}: missing kit/view/pose')
        if type(frame.get('tick')) is not int or frame['tick'] < 0:
            raise ValueError(f'{label}: invalid tick')
        if any(type(frame.get(k)) is not int or frame[k] < 1 for k in ('w', 'h')):
            raise ValueError(f'{label}: invalid dimensions')
        validate_rows(frame.get('rows'), pal, label)
        if len(frame['rows']) != frame['h'] or any(len(r) != frame['w'] for r in frame['rows']):
            raise ValueError(f'{label}: rows do not match native canvas')
        validate_fit(frame.get('fit'), label)
        if type(frame.get('mirror')) is not bool:
            raise ValueError(f'{label}: mirror must be boolean')
        key = frame_key(frame)
        if key in seen:
            raise ValueError(f'{label}: duplicate frame {key}')
        seen.add(key)
    return frames


def select_frames(frames, kit=None, view=None, pose=None, tick=None):
    selected = [f for f in frames if all(v is None or f[k] == v for k, v in
                (('kit', kit), ('view', view), ('pose', pose), ('tick', tick)))]
    if not selected:
        raise ValueError('no native frames match kit/view/pose/tick selection')
    return selected


def rows_rgba(rows, pal):
    out = np.zeros((len(rows), max(map(len, rows)), 4), dtype=np.uint8)
    for y, row in enumerate(rows):
        for x, ink in enumerate(row):
            if ink not in '. ':
                out[y, x] = (*pal[ink], 255)
    return out


def ink_bounds(frames):
    """One union box across *all* animation frames, including ragged rows."""
    points = [(x, y) for rows in frames for y, row in enumerate(rows)
              for x, ink in enumerate(row) if ink not in '. ']
    if not points:
        raise ValueError('worn sprite has no ink')
    xs, ys = zip(*points)
    return min(xs), min(ys), max(xs) - min(xs) + 1, max(ys) - min(ys) + 1


def fit_size(size, maximum):
    """rules::looks::fit_size: f32, positive round-half-away, integer growth."""
    w, h = size
    mw, mh = maximum
    if min(w, h, mw, mh) <= 0:
        raise ValueError('fit sizes must be positive')
    if w <= mw and h <= mh:
        k = max(1, min(mw // w, mh // h))
        return w * k, h * k
    scale = min(np.float32(mw) / np.float32(w), np.float32(mh) / np.float32(h))
    # Convert the f32 *product* to float64 only for rounding. Adding .5 in
    # f32 would introduce a second rounding absent from Rust's f32::round.
    return tuple(min(m, max(1, int(np.floor(float(np.float32(n) * scale) + .5))))
                 for n, m in ((w, mw), (h, mh)))


def sample_frames(frames, box, size):
    """Edge-origin integer sampling, NOT Pillow's centre-sampled resize."""
    x, y, w, h = box
    tw, th = size
    return [[''.join(rows[sy][sx] if sy < len(rows) and sx < len(rows[sy]) else '.'
                     for sx in (x + tx * w // tw for tx in range(tw)))
             for sy in (y + ty * h // th for ty in range(th))] for rows in frames]


def fitted_frames(frames, part, repair=False):
    box = ink_bounds(frames)
    size = box[2:]
    # Authoring repair must not enlarge a drawing that already fits.
    target = size if repair and all(n <= m for n, m in zip(size, part['size'])) else (part['size'] if part.get('stretch', False) and not repair else fit_size(size, part['size']))
    return sample_frames(frames, box, target)


def part_origin(part, size):
    w, h = size
    ax, ay = part['anchor']
    return ax - w // 2, ay - (h - 1 if part['bottom'] else h // 2)


def validate_sprite(sprite, pal):
    if not isinstance(sprite, dict):
        raise ValueError('worn Sprite must be an object')
    frames = sprite.get('frames')
    if not isinstance(frames, list) or not 1 <= len(frames) <= 4:
        raise ValueError('worn Sprite requires 1..4 frames')
    for i, rows in enumerate(frames):
        validate_rows(rows, pal, f'worn frame {i}')
    ink_bounds(frames)
    if sprite.get('at') is not None and not isinstance(sprite['at'], str):
        raise ValueError('worn Sprite.at must be a slot name')
    if 'moves' in sprite and not isinstance(sprite['moves'], list):
        raise ValueError('worn Sprite.moves must be an array')
    slot_name(sprite.get('at') or 'whole')
    if sprite.get('moves'):
        raise ValueError('native preview supports static worn sprites only; move= requires a Rust-exported frame')
    return sprite


def author_png(path, pal, part, slot, count=1):
    """Horizontal equal-cell RGBA strip -> repaired, palette-pure Sprite JSON."""
    if not 1 <= count <= 4:
        raise ValueError('worn PNG requires 1..4 horizontal frames')
    with Image.open(path) as image:
        rgba = np.array(image.convert('RGBA'))
    h, width = rgba.shape[:2]
    if width % count:
        raise ValueError('PNG width must divide evenly by --frames')
    w = width // count
    inks = list(pal)
    colours = np.array([pal[c] for c in inks], dtype=np.int32)
    frames = []
    for i in range(count):
        cell = rgba[:, i * w:(i + 1) * w]
        # Bounded chunks avoid H*W*palette temporary memory on large inputs.
        rgb = cell[..., :3].reshape(-1, 3).astype(np.int32)
        indices = np.empty(len(rgb), dtype=np.intp)
        for start in range(0, len(rgb), 4096):
            diff = rgb[start:start + 4096, None] - colours
            indices[start:start + 4096] = (diff * diff).sum(2).argmin(1)
        chars = np.array(inks)[indices].reshape(h, w)
        chars[cell[..., 3] < 128] = '.'
        frames.append([''.join(row) for row in chars])
    frames = fitted_frames(frames, part, repair=True)
    if any(not any(c not in '. ' for row in rows for c in row) for rows in frames):
        raise ValueError('worn PNG has a blank frame or loses a frame silhouette when fitted; draw a clearer subject')
    return {'frames': frames, 'at': slot_name(slot), 'pixel': True}


def _stamp(canvas, rows, x, y):
    for sy, row in enumerate(rows):
        for sx, ink in enumerate(row):
            if ink not in '. ' and 0 <= y + sy < len(canvas) and 0 <= x + sx < len(canvas[0]):
                canvas[y + sy][x + sx] = ink


def dress(frame, sprites):
    """wear::dressed for static fallback inks; oldest-to-newest in each pass.

    Native rows are already final/mirrored; unmirror before attachments, then
    mirror the *whole* result. Bounds/anchors stay in pre-mirror coordinates.
    """
    if not sprites:
        return copy.deepcopy(frame)
    body = [r[::-1] for r in frame['rows']] if frame['mirror'] else frame['rows']
    pieces = []
    for sprite in sprites:
        if sprite.get('moves'):
            raise ValueError('move= requires a Rust-exported frame')
        part = body_part(frame['fit'], sprite.get('at') or 'whole')
        frames = fitted_frames(sprite['frames'], part)
        rows = frames[(frame['tick'] // TICKS_PER_FRAME) % len(frames)]
        w, h = len(rows[0]), len(rows)
        x, y = part_origin(part, (w, h))
        y -= max(0, y + h - frame['h'])  # no bottom padding / feet drift
        pieces.append((rows, x, y, part['under']))
    pad = max([0] + [-x for rows, x, y, under in pieces] +
              [x + len(rows[0]) - frame['w'] for rows, x, y, under in pieces])
    top = max([0] + [-y for rows, x, y, under in pieces])
    w, h = frame['w'] + 2 * pad, frame['h'] + top
    canvas = [['.'] * w for _ in range(h)]
    for under in (True, False):
        if not under:
            _stamp(canvas, body, pad, top)
        for rows, x, y, behind in pieces:
            if behind == under:
                _stamp(canvas, rows, x + pad, y + top)
    rows = [''.join(row) for row in canvas]
    if frame['mirror']:
        rows = [row[::-1] for row in rows]
    result = copy.deepcopy(frame)
    result.update(w=w, h=h, rows=rows, worn_padding=[pad, top])
    # Preserve any exporter-specific metadata verbatim; only known coordinate
    # fields (the Body) are translated. worn_padding records our additional shift.
    result['fit']['bounds'][0] += pad
    result['fit']['bounds'][1] += top
    for part in PARTS:
        result['fit'][part]['anchor'][0] += pad
        result['fit'][part]['anchor'][1] += top
    return result


def export_frames(frames, pal, out, zoom=8):
    """Uncropped native PNGs + reloadable manifest + one common-integer preview."""
    if zoom < 1:
        raise ValueError('zoom must be a positive integer')
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    records = []
    for i, frame in enumerate(frames):
        image = f'{i:04d}.png'
        Image.fromarray(rows_rgba(frame['rows'], pal)).save(out / image)
        records.append(dict(frame, image=image))
    write_json(out / 'frames.json', {'version': 1, 'frames': records})
    # Preview only the first 32; *all* selected frames are exported, at 1:1.
    shown = records[:32]
    cw = max(f['w'] for f in frames) * zoom + 16
    ch = max(f['h'] for f in frames) * zoom + 44
    cols = min(4, len(shown))
    sheet = Image.new('RGB', (cols * cw, ((len(shown) + cols - 1) // cols) * ch), (30, 30, 38))
    draw = ImageDraw.Draw(sheet)
    for i, frame in enumerate(shown):
        x, y = (i % cols) * cw, (i // cols) * ch
        draw.text((x + 4, y + 2), f"{frame['kit']} {frame['view']}", fill='white')
        draw.text((x + 4, y + 15), f"{frame['pose']} t={frame['tick']}", fill='white')
        im = Image.fromarray(rows_rgba(frame['rows'], pal))
        im = im.resize((im.width * zoom, im.height * zoom), Image.Resampling.NEAREST)
        sheet.paste(im, (x + 8, y + 36), im)
    sheet.save(out / 'preview.png')
    return records


def export_animation(frames, pal, out, fps=5):
    """Native pose strip and looping preview with one fixed standing point.

    Group per kit/view; align feet without scaling or recropping any pose.
    The JSON keeps the original resolved fit for each frame and its offset.
    """
    if not 1 <= fps <= 60:
        raise ValueError('animation fps must be in 1..60')
    out = Path(out)
    groups = {}
    for frame in frames:
        groups.setdefault((frame['kit'], frame['view']), []).append(frame)
    index = []
    for n, ((kit, view), poses) in enumerate(groups.items()):
        w, h = max(f['w'] for f in poses), max(f['h'] for f in poses)
        strip = Image.new('RGBA', (w * len(poses), h))
        cells, metadata = [], []
        for i, frame in enumerate(poses):
            x, y = (w-frame['w'])//2, h-frame['h']
            cell = Image.new('RGBA', (w,h))
            cell.alpha_composite(Image.fromarray(rows_rgba(frame['rows'],pal)),(x,y))
            strip.alpha_composite(cell,(i*w,0))
            cells.append(cell)
            metadata.append(dict(frame, offset=[x,y]))
        name = f'animation-{n:03d}'
        strip.save(out / (name+'.png'))
        # GIF is preview only; RGBA strip and JSON are the lossless assets.
        cells[0].save(out / (name+'.gif'), save_all=True, append_images=cells[1:],
                      duration=round(1000/fps), loop=0, disposal=2)
        index.append(dict(kit=kit,view=view,size=[w,h],fps=fps,loop=True,
                          sheet=name+'.png',preview=name+'.gif',frames=metadata))
    write_json(out / 'animations.json',dict(version=1,animations=index))
    return index


def card_text(sprite):
    """Learned-card drawing section, accepted by the existing game checker."""
    frames = sprite['frames']
    rows = ['|'.join(frame[y] for frame in frames) for y in range(len(frames[0]))]
    return 'draw worn at='+sprite['at']+' pixel\n'+'\n'.join(rows)+'\n'


def main(argv=None):
    ap = argparse.ArgumentParser(prog='spriterig.native', description=__doc__)
    ap.add_argument('--angelx', help='checkout owning the canonical fit table (or ANGELX_ROOT)')
    ap.add_argument('--native', required=True, help='authoritative Rust-exported frames JSON')
    ap.add_argument('--out', default='out/native')
    ap.add_argument('--animate', action='store_true', help='fixed-floor native PNG strips, JSON and GIF previews')
    ap.add_argument('--fps', type=int, default=5)
    ap.add_argument('--kit')
    ap.add_argument('--view')
    ap.add_argument('--pose')
    ap.add_argument('--tick', type=int)
    ap.add_argument('--zoom', type=int, default=8, help='common integer preview zoom; PNG frames stay 1:1')
    ap.add_argument('--worn', help='RGBA PNG or equal-cell horizontal strip to author')
    ap.add_argument('--slot', default='head', help='body part, including game aliases')
    ap.add_argument('--profile', default='knight', help='canonical authoring profile, not a doll substitute')
    ap.add_argument('--frames', type=int, default=1, help='PNG strip frame count (1..4; tick//6)')
    ap.add_argument('--sprite', action='append', default=[], help='existing static Sprite JSON; repeat oldest first')
    a = ap.parse_args(argv)
    try:
        root = root_path(a.angelx)
        profiles = load_fits(root)
        if a.profile not in profiles:
            raise ValueError(f'missing canonical fit profile {a.profile!r}')
        pal = palette(root)
        if not pal:
            raise ValueError('empty game ink palette')
        native = select_frames(load_native(a.native, pal), a.kit, a.view, a.pose, a.tick)
        sprites = [validate_sprite(read_json(p), pal) for p in a.sprite]
        authored = None
        if a.worn:
            slot = slot_name(a.slot)
            authored = author_png(a.worn, pal, body_part(profiles[a.profile], slot), slot, a.frames)
            sprites.append(authored)
        rendered = [dress(f, sprites) for f in native]
        export_frames(rendered, pal, a.out, a.zoom)
        if a.animate:
            export_animation(rendered, pal, a.out, a.fps)
        if authored:
            write_json(Path(a.out) / 'worn.json', authored)
            (Path(a.out) / 'worn.card.txt').write_text(card_text(authored),encoding='utf-8')
        write_json(Path(a.out) / 'workflow.json', {
            'native': str(Path(a.native).resolve()), 'fit_table': str(root / FIT_PATH),
            'profile': a.profile, 'ticks_per_frame': TICKS_PER_FRAME,
            'preview': 'native fallback inks only; no HD painting compositor',
            'sprites': sprites,
        })
    except (OSError, ValueError, KeyError, TypeError) as exc:
        ap.error(str(exc))
    print(f'{len(rendered)} native frames -> {a.out}; fallback inks, native pixels at 1:1')
    return rendered


if __name__ == '__main__':
    main()
