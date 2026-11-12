"""Foundational, native-resolution humanoid cutout rig (numpy/Pillow only).

fit() returns JSON-editable source-space joints, disjoint #/. part masks,
attachment points and local-angle clips. No source pixels are rescaled or
invented at rest. build() inverse-samples rigid parts into ONE fixed canvas;
its rows and attachments are final facing-right pixels, ready for the runtime
contract (left-facing runtime views mirror x to width-1-x).

A single view cannot reveal hidden anatomy. Edit masks/joints in rig.json for
occluded limbs, unusual proportions, capes or an already-held weapon. Optional
source keying is explicit: an opaque corner is never silently deleted.
"""
import argparse
import json
import math
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

from .delve.source import palette
from .native import root_path
from .quick_motion import CLIPS, TICKS_PER_SECOND, defaults

# name, parent, source joint, draw order. Front means image-right, not anatomy.
_LAYOUT = (
    ('back', 'torso', 'neck', 0),
    ('upper_arm_b', 'torso', 'shoulder_b', 1),
    ('lower_arm_b', 'upper_arm_b', 'elbow_b', 2),
    ('hand_b', 'lower_arm_b', 'wrist_b', 3),
    ('upper_leg_b', None, 'hip_b', 4),
    ('lower_leg_b', 'upper_leg_b', 'knee_b', 5),
    ('foot_b', 'lower_leg_b', 'ankle_b', 6),
    ('upper_leg_f', None, 'hip_f', 7),
    ('lower_leg_f', 'upper_leg_f', 'knee_f', 8),
    ('foot_f', 'lower_leg_f', 'ankle_f', 9),
    ('torso', None, 'root', 10),
    ('head', 'torso', 'neck', 11),
    ('upper_arm_f', 'torso', 'shoulder_f', 12),
    ('lower_arm_f', 'upper_arm_f', 'elbow_f', 13),
    ('hand_f', 'lower_arm_f', 'wrist_f', 14),
)
PARTS = tuple(p[0] for p in _LAYOUT)
ANCHORS = ('grip', 'off_hand', 'head', 'back', 'feet', 'tip')
_MAX_SIDE = 1024  # source image, including an entire input sheet
_MAX_CANVAS = 128
_MAX_FRAMES = 128
_MAX_TOTAL_FRAMES = 1024
_MAX_TICKS = 3600
_MAX_JOINTS = 64
_MAX_OUTPUT_PIXELS = _MAX_CANVAS**2 * _MAX_FRAMES * len(CLIPS)
_MAX_BYTES = 8 * 1024 * 1024


def _integer(value, name, low=0, high=None):
    if type(value) is not int or value < low or (high is not None and value > high):
        raise ValueError(f'{name}: expected integer in {low}..{high or "unbounded"}')
    return value


def _point(value, name, integer=False):
    if not isinstance(value, (list, tuple)) or len(value) != 2:
        raise ValueError(f'{name}: expected [x, y]')
    if any(isinstance(v, bool) or not isinstance(v, (int, float)) or
           (isinstance(v, float) and not math.isfinite(v)) or
           (integer and type(v) is not int) for v in value):
        raise ValueError(f'{name}: expected two finite {"integers" if integer else "numbers"}')
    return value


def _size(value, name, high=_MAX_SIDE):
    _point(value, name, integer=True)
    for v in value:
        _integer(v, name, 1, high)
    return value


def _key(value):
    if value is None:
        return None
    if isinstance(value, str):
        try:
            if len(value.lstrip('#')) != 6:
                raise ValueError()
            value = list(bytes.fromhex(value.lstrip('#')))
        except ValueError as exc:
            raise ValueError('key_color: expected RRGGBB') from exc
    if not isinstance(value, (list, tuple)) or len(value) != 3:
        raise ValueError('key_color: expected three RGB bytes')
    return [_integer(v, 'key_color', 0, 255) for v in value]


def _image(image, *, cell=None, cell_size=None, facing='right',
           alpha_threshold=128, key_color=None):
    _integer(alpha_threshold, 'alpha_threshold', 1, 255)
    if facing not in ('right', 'left'):
        raise ValueError('facing: expected right or left')
    def selection(size):
        # Inspect headers/shapes BEFORE conversion, decoding, slicing or copying.
        w, h = _size(size, 'source')
        if (cell is None) != (cell_size is None):
            raise ValueError('sheet: cell and cell_size must be supplied together')
        if cell is None:
            return (0, 0, w, h)
        _integer(cell, 'cell')
        cw, ch = _size(cell_size, 'cell_size')
        if w % cw or h % ch:
            raise ValueError('sheet: dimensions must be multiples of cell_size')
        cols, rows = w // cw, h // ch
        if cell >= cols * rows:
            raise ValueError('sheet: cell index out of range')
        x, y = (cell % cols) * cw, (cell // cols) * ch
        return (x, y, x+cw, y+ch)

    if isinstance(image, (str, Path)):
        if Path(image).stat().st_size > _MAX_BYTES:
            raise ValueError('source: PNG exceeds 8 MiB')
        with Image.open(image) as im:
            if im.format != 'PNG':
                raise ValueError('source: expected PNG')
            box = selection(im.size)
            rgba = np.array(im.crop(box).convert('RGBA'))
    elif isinstance(image, Image.Image):
        box = selection(image.size)
        rgba = np.array(image.crop(box).convert('RGBA'))
    elif isinstance(image, np.ndarray):
        if image.dtype != np.uint8 or image.ndim != 3 or image.shape[2] not in (3, 4):
            raise ValueError('source: expected uint8 HxWx3/4 array')
        x0, y0, x1, y1 = selection(image.shape[1::-1])
        rgba = np.array(Image.fromarray(image[y0:y1, x0:x1]).convert('RGBA'))
    else:
        raise ValueError('source: expected PNG path, Pillow image or uint8 array')
    color = _key(key_color)
    if color is not None:
        rgba[np.all(rgba[..., :3] == color, axis=-1), 3] = 0
    rgba[..., 3] = np.where(rgba[..., 3] >= alpha_threshold, 255, 0)
    rgba[rgba[..., 3] == 0] = 0  # canonical transparent RGBA, never hidden RGB
    if facing == 'left':
        rgba = rgba[:, ::-1].copy()
    if not np.any(rgba[..., 3]):
        raise ValueError('source: no visible pixels after alpha/key filtering')
    return rgba


def _palette(root=None):
    pal = palette(root_path(root))
    if not pal or '.' in pal or any(len(k) != 1 or not k.isascii() or k == '.' for k in pal):
        raise ValueError('palette: expected nonempty one-character inks, excluding dot')
    return pal


def _inks(rgba, pal):
    """Map opaque RGB to nearest real ink. Alpha is binary, never RGB-composited."""
    names = sorted(pal)  # stable tie break, independent of source table ordering
    colors = np.array([pal[k] for k in names], dtype=np.int32)
    out = np.full(rgba.shape[:2], '.', dtype='<U1')
    opaque = rgba[..., 3] != 0
    rgb, inverse = np.unique(rgba[opaque, :3], axis=0, return_inverse=True)
    # Chunk unique colours so generated/antialiased inputs don't allocate N*palette.
    nearest = []
    for i in range(0, len(rgb), 1024):
        d = rgb[i:i+1024].astype(np.int32)[:, None, :] - colors[None, :, :]
        nearest.extend(np.argmin((d*d).sum(-1), axis=1).tolist())
    out[opaque] = np.array(names)[np.array(nearest)[inverse]]
    return out


def _rows(array):
    return [''.join(row) for row in array]


def _contact(mask, target, *, bottom=False):
    """A real pixel in one owning mask; never a proportional/median point in air."""
    y, x = np.nonzero(mask)
    if not len(x):
        raise ValueError('contact: owning part has no pixels; edit the part masks')
    pixels = np.stack([x, y], -1)
    if bottom:
        pixels = pixels[y == y.max()]
    return pixels[np.argmin(((pixels-np.asarray(target))**2).sum(-1))].tolist()


def fit(image, *, root=None, cell=None, cell_size=None, facing='right',
        alpha_threshold=128, key_color=None, canvas=None, pivot=None):
    """Fit a native-size silhouette; return a JSON-serializable editable rig.

    cell is a zero-based row-major sheet index; cell_size is (width, height).
    Joints/masks/anchor points are in the selected cell AFTER facing normalization.
    canvas/pivot are final output pixels; default padding is one body-height on
    every side. Source floor maps to pivot. No automatic cropping or scaling.
    """
    _palette(root)  # fail early if the authoritative palette root is unavailable
    rgba = _image(image, cell=cell, cell_size=cell_size, facing=facing,
                  alpha_threshold=alpha_threshold, key_color=key_color)
    m = rgba[..., 3] != 0
    h, w = m.shape
    yy, xx = np.nonzero(m)
    y0, y1 = int(yy.min()), int(yy.max())
    height = y1-y0+1
    if height < 4 or xx.max()-xx.min() < 2:
        raise ValueError('source: silhouette is too small to fit a humanoid (need 3x4)')
    Y, X = np.mgrid[:h, :w]
    # Central torso mass, avoiding a wide hat or long feet when estimating x.
    band = m & (Y >= y0+height*.28) & (Y < y0+height*.57)
    cx = float(np.median(X[band])) if band.any() else float(np.median(xx))
    neck, hip = y0+height*.25, y0+height*.60
    elbow, wrist = y0+height*.44, y0+height*.60
    knee, ankle = y0+height*.79, y0+height*.92
    runs = []
    for y in range(int(neck), max(int(neck)+1, int(hip))):
        c = min(w-1, max(0, int(round(cx))))
        if m[y, c]:
            l = r = c
            while l > 0 and m[y, l-1]:
                l -= 1
            while r+1 < w and m[y, r+1]:
                r += 1
            runs.append((r-l+1)/2)
    thw = float(np.clip(np.median(runs) if runs else height*.14,
                        max(1, height*.08), max(1, height*.17)))
    labels = np.full((h, w), 'torso', dtype='<U16')
    labels[Y < neck] = 'head'
    low = Y >= hip
    for side, side_mask in (('b', X < cx), ('f', X >= cx)):
        leg = low & side_mask
        labels[leg] = 'upper_leg_'+side
        labels[leg & (Y >= knee)] = 'lower_leg_'+side
        labels[leg & (Y >= ankle)] = 'foot_'+side
        arm = side_mask & (np.abs(X-cx) > thw) & (Y >= neck) & (Y < hip+height*.12)
        labels[arm] = 'upper_arm_'+side
        labels[arm & (Y >= elbow)] = 'lower_arm_'+side
        labels[arm & (Y >= wrist)] = 'hand_'+side
    # Conservatively recognize a rear hanging strip; ordinary short arms are
    # NOT turned into a cape. Occluded capes can be painted into the back mask.
    tail = m & (X < cx-thw-1) & (Y >= hip+height*.06) & (Y < ankle)
    ty, tx = np.nonzero(tail)
    if len(ty) >= 3 and np.ptp(ty) >= height*.12:
        cape = m & (X <= np.max(tx)) & (Y >= neck) & (Y < ankle)
        labels[cape] = 'back'

    # A short/occluded limb can end before the proportional wrist/ankle.
    # Give its distal visible pixel to the hand/foot, not to an unrelated torso.
    for side in ('b', 'f'):
        for distal, chain in (('hand_', ('lower_arm_', 'upper_arm_')),
                              ('foot_', ('lower_leg_', 'upper_leg_'))):
            name = distal+side
            if not np.any(m & (labels == name)):
                candidates = m & np.isin(labels, [p+side for p in chain])
                if candidates.any():
                    x, y = _contact(candidates, (cx, y1), bottom=True)
                    labels[y, x] = name

    def point(x, y):
        return [round(float(np.clip(x, 0, w-1)), 3),
                round(float(np.clip(y, 0, h-1)), 3)]

    def mass(name, fallback, axis=0):
        y, x = np.nonzero(m & (labels == name))
        return float(np.median((x, y)[axis])) if len(x) else fallback

    joints = {'root': point(cx, hip), 'neck': point(cx, neck)}
    for s, sign in (('b', -1), ('f', 1)):
        arm_x = mass('upper_arm_'+s, cx+sign*(thw+.5))
        fore_x = mass('lower_arm_'+s, arm_x)
        hand_x = mass('hand_'+s, fore_x)
        leg_x = mass('upper_leg_'+s, cx+sign*height*.10)
        knee_x = mass('lower_leg_'+s, leg_x)
        joints.update({
            'shoulder_'+s: point(cx+sign*thw, neck+height*.04),
            'elbow_'+s: point((arm_x+fore_x)/2, elbow),
            'wrist_'+s: point(hand_x, wrist),
            'hip_'+s: point(leg_x, hip),
            'knee_'+s: point(knee_x, knee),
            'ankle_'+s: point(mass('foot_'+s, knee_x), ankle),
        })
    parts = {name: dict(parent=parent, joint=joint, z=z,
                        mask=_rows(np.where(m & (labels == name), '#', '.')))
             for name, parent, joint, z in _LAYOUT}
    floor = [int(round(cx)), y1]
    pad = height
    canvas = list(canvas) if canvas is not None else [w+2*pad, h+2*pad]
    pivot = list(pivot) if pivot is not None else [floor[0]+pad, floor[1]+pad]
    def contact(name, target, bottom=False):
        return _contact(m & (labels == name), target, bottom=bottom)

    grip = contact('hand_f', joints['wrist_f'])
    off_hand = contact('hand_b', joints['wrist_b'])
    head = contact('head', (cx, neck), bottom=True)
    foot = 'foot_f' if np.any(m & (labels == 'foot_f')) else 'foot_b'
    feet = contact(foot, floor, bottom=True)
    back = 'back' if np.any(m & (labels == 'back')) else 'torso'
    back_point = contact(back, (cx, (neck+hip)/2))
    rig = dict(version=1, source=dict(size=[w, h], cell=cell,
               cell_size=list(cell_size) if cell_size is not None else None,
               facing=facing, alpha_threshold=alpha_threshold, key_color=_key(key_color)),
               canvas=canvas, pivot=pivot, origin=floor, joints=joints, parts=parts,
               seam_radius=1 if height < 32 else 2,
               anchors={
                   'grip': dict(part='hand_f', point=grip),
                   'off_hand': dict(part='hand_b', point=off_hand),
                   'head': dict(part='head', point=head),
                   'back': dict(part=back, point=back_point),
                   'feet': dict(part=foot, point=feet),
                   'tip': dict(part='hand_f', point=[grip[0], round(grip[1]+height*.45, 3)]),
               }, clips=defaults(height))
    return validate_rig(rig)


def validate_rig(rig):
    """Validate structure and finite values, including hierarchy cycles and masks."""
    if not isinstance(rig, dict) or type(rig.get('version')) is not int or rig['version'] != 1:
        raise ValueError('rig: expected version 1')
    source = rig.get('source')
    if not isinstance(source, dict):
        raise ValueError('source: expected object')
    w, h = _size(source.get('size'), 'source.size')
    if source.get('facing') not in ('left', 'right'):
        raise ValueError('source.facing: expected left or right')
    _integer(source.get('alpha_threshold'), 'source.alpha_threshold', 1, 255)
    _key(source.get('key_color'))
    cell, size = source.get('cell'), source.get('cell_size')
    if (cell is None) != (size is None):
        raise ValueError('source: cell and cell_size must be supplied together')
    if cell is not None:
        _integer(cell, 'source.cell')
        _size(size, 'source.cell_size')
        if list(size) != [w, h]:
            raise ValueError('source.cell_size must match source.size')
    cw, ch = _size(rig.get('canvas'), 'canvas', _MAX_CANVAS)
    px, py = _point(rig.get('pivot'), 'pivot', integer=True)
    if not (0 <= px < cw and 0 <= py < ch):
        raise ValueError('pivot must be inside canvas')
    ox, oy = _point(rig.get('origin'), 'origin')
    if not (0 <= ox < w and 0 <= oy < h):
        raise ValueError('origin must be inside source')
    _integer(rig.get('seam_radius'), 'seam_radius', 0, 4)
    joints, parts = rig.get('joints'), rig.get('parts')
    if not isinstance(joints, dict) or 'root' not in joints:
        raise ValueError('joints: missing root')
    if len(joints) > _MAX_JOINTS:
        raise ValueError(f'joints: exceeds {_MAX_JOINTS} joints')
    for name, value in joints.items():
        x, y = _point(value, f'joints.{name}')
        if not (0 <= x < w and 0 <= y < h):
            raise ValueError(f'joints.{name}: joint must be inside source')
    if not isinstance(parts, dict) or set(parts) != set(PARTS):
        raise ValueError('parts: expected head, torso, back and segmented b/f arms/hands/legs/feet')
    for name, part in parts.items():
        if not isinstance(part, dict):
            raise ValueError(f'parts.{name}: expected object')
        if not isinstance(part.get('joint'), str) or part['joint'] not in joints:
            raise ValueError(f'parts.{name}: unknown joint')
        if part.get('parent') is not None and (not isinstance(part['parent'], str) or part['parent'] not in parts):
            raise ValueError(f'parts.{name}: unknown parent')
        _integer(part.get('z'), f'parts.{name}.z')
        rows = part.get('mask')
        if not isinstance(rows, list) or len(rows) != h or any(
                not isinstance(row, str) or len(row) != w or set(row)-set('.#') for row in rows):
            raise ValueError(f'parts.{name}.mask: expected {h} rows of {w} #/. pixels')
        visited, cur = set(), name
        while cur is not None:
            if cur in visited:
                raise ValueError('parts: hierarchy cycle')
            visited.add(cur)
            node = parts.get(cur)
            if not isinstance(node, dict):
                raise ValueError('parts: unknown parent')
            cur = node.get('parent')
            if cur is not None and not isinstance(cur, str):
                raise ValueError('parts: parent must be a part name or null')
    anchors = rig.get('anchors')
    if not isinstance(anchors, dict) or set(anchors) != set(ANCHORS):
        raise ValueError('anchors: expected '+', '.join(ANCHORS))
    for name, anchor in anchors.items():
        if not isinstance(anchor, dict) or (anchor.get('part') is not None and
                (not isinstance(anchor['part'], str) or anchor['part'] not in parts)):
            raise ValueError(f'anchors.{name}: unknown part')
        point = _point(anchor.get('point'), f'anchors.{name}.point')
        if any(abs(v) > 2*_MAX_SIDE for v in point):
            raise ValueError(f'anchors.{name}: exceeds source coordinate limit')
    contacts = {'grip': ('hand_f',), 'off_hand': ('hand_b',),
                'head': ('head',), 'feet': ('foot_f', 'foot_b'),
                'back': ('back', 'torso')}
    for name, owners in contacts.items():
        if anchors[name]['part'] not in owners:
            raise ValueError(f'anchors.{name}: contact must belong to {owners}')
        x, y = anchors[name]['point']
        if not (0 <= x < w and 0 <= y < h):
            raise ValueError(f'anchors.{name}: contact must be inside source')
        if not any('#' in row for row in parts[anchors[name]['part']]['mask']):
            raise ValueError(f'anchors.{name}: owning part has no pixels')
    clips = rig.get('clips')
    _validate_clips(clips, 'clips')
    for name, frames in clips.items():
        for frame in frames:
            if not isinstance(frame, dict):
                raise ValueError(f'clips.{name}: expected pose object')
            _integer(frame.get('ticks'), f'clips.{name}.ticks', 1, _MAX_TICKS)
            offset = _point(frame.get('offset', [0, 0]), f'clips.{name}.offset')
            if any(abs(v) > _MAX_CANVAS for v in offset):
                raise ValueError(f'clips.{name}.offset: exceeds canvas limit')
            angles = frame.get('angles', {})
            if not isinstance(angles, dict) or set(angles)-set(parts):
                raise ValueError(f'clips.{name}: unknown angle part')
            for value in [frame.get('rotation', 0), *angles.values()]:
                if (isinstance(value, bool) or not isinstance(value, (int, float)) or
                        (isinstance(value, float) and not math.isfinite(value))):
                    raise ValueError(f'clips.{name}: angles must be finite numbers')
            if type(frame.get('smear', False)) is not bool:
                raise ValueError(f'clips.{name}: smear must be boolean')
    return rig


def _validate_clips(clips, label):
    if not isinstance(clips, dict) or not clips:
        raise ValueError(f'{label}: expected nonempty object')
    if any(name not in CLIPS for name in clips):
        raise ValueError(f'{label}: unknown clip name; expected standard clip identifiers')
    if 'idle' not in clips:
        raise ValueError(f'{label}: idle is required')
    for name, frames in clips.items():
        if not isinstance(frames, list) or not 1 <= len(frames) <= _MAX_FRAMES:
            raise ValueError(f'{label}.{name}: expected 1..{_MAX_FRAMES} frames')
    if sum(map(len, clips.values())) > _MAX_TOTAL_FRAMES:
        raise ValueError(f'{label}: exceeds {_MAX_TOTAL_FRAMES} total frames')


def _json_text(data):
    text = json.dumps(data, indent=2, allow_nan=False)+'\n'
    if len(text.encode('utf-8')) > _MAX_BYTES:
        # Formatting must not make an otherwise runtime-loadable asset fail.
        text = json.dumps(data, separators=(',', ':'), allow_nan=False)+'\n'
        if len(text.encode('utf-8')) > _MAX_BYTES:
            raise ValueError('JSON exceeds 8 MiB')
    return text


def save_rig(rig, path):
    """Write reloadable fit/fixup JSON, not generated pixel frames."""
    validate_rig(rig)
    text = _json_text(rig)
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding='utf-8')


def load_rig(path):
    def unique(pairs):
        result = {}
        for name, value in pairs:
            if name in result:
                raise ValueError(f'rig: duplicate JSON field {name}')
            result[name] = value
        return result

    with Path(path).open('rb') as stream:
        data = stream.read(_MAX_BYTES+1)
    if len(data) > _MAX_BYTES:
        raise ValueError('rig: JSON exceeds 8 MiB')
    return validate_rig(json.loads(data, object_pairs_hook=unique))


def _rotation(degrees):
    r = math.radians(degrees % 360)
    c, s = math.cos(r), math.sin(r)
    return np.array([[c, -s], [s, c]])


def _transforms(rig, pose):
    """(2x2 rotation, translation) per part, in final output pixel space."""
    root = _rotation(pose.get('rotation', 0))
    joint_root = np.array(rig['joints']['root'])
    origin = np.array(rig['origin'])
    delta = np.array(rig['pivot'])-origin + np.array(pose.get('offset', [0, 0]))
    base = (root, joint_root+delta-root@joint_root)
    result = {}

    def resolve(name):
        if name not in result:
            part = rig['parts'][name]
            pr, pt = resolve(part['parent']) if part['parent'] else base
            joint = np.array(rig['joints'][part['joint']])
            r = pr@_rotation(pose.get('angles', {}).get(name, 0))
            result[name] = (r, pr@joint+pt-r@joint)
        return result[name]

    for name in rig['parts']:
        resolve(name)
    return result, base


def _textures(rig, ink):
    textures = {}
    masks = {name: np.array([list(row) for row in part['mask']]) == '#'
             for name, part in rig['parts'].items()}
    covered = np.logical_or.reduce(list(masks.values()))
    if np.any((ink != '.') & ~covered):
        raise ValueError('parts: masks leave visible source pixels unassigned')
    Y, X = np.mgrid[:ink.shape[0], :ink.shape[1]]
    for name, part in rig['parts'].items():
        mask = masks[name].copy()
        # Borrow ONLY actual neighbouring parent pixels at the shared joint.
        # These overlapping collars seal rasterization seams without rest-pose
        # dilation, solid-colour discs, mesh holes or invented hidden body art.
        if part['parent'] and rig['seam_radius']:
            x, y = rig['joints'][part['joint']]
            collar = (X-x)**2+(Y-y)**2 <= rig['seam_radius']**2
            mask |= collar & masks[part['parent']]
        textures[name] = np.where(mask, ink, '.')
    return textures


def _stamp(out, texture, transform):
    """Inverse nearest sampling (one lookup per destination; no forward holes)."""
    y, x = np.nonzero(texture != '.')
    if not len(x):
        return
    r, t = transform
    corners = np.array([[x.min()-.5, y.min()-.5], [x.max()+.5, y.min()-.5],
                        [x.min()-.5, y.max()+.5], [x.max()+.5, y.max()+.5]])@r.T+t
    x0, y0 = np.floor(corners.min(0)).astype(int)
    x1, y1 = np.ceil(corners.max(0)).astype(int)+1
    # Sample the complete transformed box first; clipping opaque output is an
    # authoring error, not a silently truncated animation.
    if max(x1-x0, y1-y0) > 4*_MAX_SIDE:
        raise ValueError('render: transformed part exceeds supported bounds')
    Y, X = np.mgrid[y0:y1, x0:x1]
    xy = np.stack([X, Y], -1)
    source = np.floor((xy-t)@r + .5).astype(int)
    sx, sy = source[..., 0], source[..., 1]
    valid = (sx >= 0) & (sy >= 0) & (sx < texture.shape[1]) & (sy < texture.shape[0])
    sample = np.full(X.shape, '.', dtype='<U1')
    sample[valid] = texture[sy[valid], sx[valid]]
    opaque = sample != '.'
    inside = (X >= 0) & (Y >= 0) & (X < out.shape[1]) & (Y < out.shape[0])
    if np.any(opaque & ~inside):
        raise ValueError('render: canvas clips a part; enlarge canvas/reposition pivot in rig.json')
    k = opaque & inside
    out[Y[k], X[k]] = sample[k]


def _anchors(rig, transforms, base, contacts):
    result = {}
    for name, anchor in rig['anchors'].items():
        r, t = transforms[anchor['part']] if anchor['part'] else base
        target = r@np.array(anchor['point'])+t
        if name == 'tip':
            # Direction/weapon reach, deliberately not a body contact.
            result[name] = np.floor(target+.5).astype(int).tolist()
        elif not np.any(contacts[anchor['part']]):
            # A part can be wholly hidden in a pose (a one-pixel hand behind the
            # body mid-roll). The socket then sits where the part would be.
            result[name] = np.floor(target+.5).astype(int).tolist()
        else:
            result[name] = _contact(contacts[anchor['part']], target,
                                    bottom=name == 'feet')
    return result


def _ground(rig, transforms, base, textures):
    """Ground roll/death by their actual transformed pixels, without canvas crop."""
    bottom = -math.inf
    for name, texture in textures.items():
        y, x = np.nonzero(texture != '.')
        if len(x):
            r, t = transforms[name]
            bottom = max(bottom, float((np.stack([x, y], -1)@r.T+t)[:, 1].max()))
    delta = np.array([0, rig['pivot'][1]-math.floor(bottom+.5)])
    return {n: (r, t+delta) for n, (r, t) in transforms.items()}, (base[0], base[1]+delta)


def _smear(out, previous, current, ink, width):
    # A short bright curved tip ribbon, behind the body; not a whole-frame
    # translucent ghost. The hand and tip are the SAME attachment transforms
    # emitted to the runtime, so a held item can use this exact trajectory.
    a, b = np.array(previous['tip'], float), np.array(current['tip'], float)
    grip = (np.array(previous['grip'])+np.array(current['grip']))/2
    if np.linalg.norm(b-a) < 1:
        return
    mid = (a+b)/2
    radial = mid-grip
    if np.linalg.norm(radial) > .01:
        radius = (np.linalg.norm(a-grip)+np.linalg.norm(b-grip))/2
        mid = grip+radial/np.linalg.norm(radial)*radius
    points = []
    for f in np.linspace(0, 1, 9):
        p = (1-f)**2*a+2*(1-f)*f*mid+f*f*b
        points.append(tuple(np.floor(p+.5).astype(int)))
    if any(x < 0 or y < 0 or x >= out.shape[1] or y >= out.shape[0] for x, y in points):
        raise ValueError('render: canvas clips swing smear; enlarge canvas')
    mask = Image.new('L', (out.shape[1], out.shape[0]))
    ImageDraw.Draw(mask).line(points, fill=255, width=width)
    out[np.array(mask) != 0] = ink


def build(image, rig, *, root=None, clips=None):
    """Return version/canvas/pivot/clips game JSON; never mutate the input rig.

    clips: optional iterable (or comma-separated string) of saved clip names.
    idle is included automatically, as required by the runtime contract.
    All attachments are integer final-facing-right pixels, not source-space
    sockets. Only the runtime mirrors for left-facing display.
    """
    validate_rig(rig)
    source = rig['source']
    rgba = _image(image, **{k: source.get(k) for k in
                  ('cell', 'cell_size', 'facing', 'alpha_threshold', 'key_color')})
    if list(rgba.shape[1::-1]) != source['size']:
        raise ValueError('source: selected dimensions do not match rig source.size')
    pal = _palette(root)
    ink = _inks(rgba, pal)
    textures = _textures(rig, ink)
    names = clips.split(',') if isinstance(clips, str) else list(clips) if clips is not None else list(rig['clips'])
    if not names or any(not isinstance(n, str) for n in names) or len(set(names)) != len(names) or any(n not in rig['clips'] for n in names):
        raise ValueError('clips: select distinct existing clip names')
    if 'idle' not in names:
        names.insert(0, 'idle')  # every export must be loadable by Rig::parse
    game = dict(version=1, canvas=rig['canvas'][:], pivot=rig['pivot'][:], clips={})
    ordered = sorted(rig['parts'], key=lambda n: (rig['parts'][n]['z'], n))
    w, h = rig['canvas']
    contact_textures = {n: np.where(np.array([list(row) for row in rig['parts'][n]['mask']]) == '#',
                                    ink, '.')
                        for n in {a['part'] for k, a in rig['anchors'].items() if k != 'tip'}}
    # Use a source highlight for smears; never introduce a fabricated ink bank.
    visible = sorted(set(''.join(_rows(ink)))-{'.'})
    smear_ink = max(visible, key=lambda c: sum(pal[c]))
    for name in names:
        frames, previous = [], None
        for i, pose in enumerate(rig['clips'][name]):
            transforms, base = _transforms(rig, pose)
            if name in ('roll', 'die'):
                transforms, base = _ground(rig, transforms, base, textures)
            out = np.full((h, w), '.', dtype='<U1')
            try:
                # Occupancy must come from the owning mask's inverse-sampled
                # pixels, not a rounded socket, borrowed collar or whole body.
                contacts = {}
                for part, texture in contact_textures.items():
                    layer = np.full((h, w), '.', dtype='<U1')
                    _stamp(layer, texture, transforms[part])
                    contacts[part] = layer != '.'
                anchors = _anchors(rig, transforms, base, contacts)
                if any(not (0 <= p[0] < w and 0 <= p[1] < h) for p in anchors.values()):
                    raise ValueError('anchor outside canvas; enlarge canvas/reposition pivot')
                if pose.get('smear') and previous:
                    _smear(out, previous, anchors, smear_ink, max(1, (rgba.shape[0]+8)//16))
                for part in ordered:
                    _stamp(out, textures[part], transforms[part])
            except ValueError as exc:
                raise ValueError(f'clips.{name}[{i}]: {exc}') from exc
            frames.append(dict(ticks=pose['ticks'], rows=_rows(out), anchors=anchors))
            previous = anchors
        game['clips'][name] = frames
    return validate_game(game, root=root)


def validate_game(game, *, root=None):
    if not isinstance(game, dict) or type(game.get('version')) is not int or game['version'] != 1:
        raise ValueError('game: expected version 1')
    if set(game) != {'version', 'canvas', 'pivot', 'clips'}:
        raise ValueError('game: unknown or missing fields')
    w, h = _size(game.get('canvas'), 'game.canvas', _MAX_CANVAS)
    px, py = _point(game.get('pivot'), 'game.pivot', integer=True)
    if not (0 <= px < w and 0 <= py < h):
        raise ValueError('game.pivot outside canvas')
    pal = _palette(root)
    clips = game.get('clips')
    _validate_clips(clips, 'game.clips')
    idle_ink = False
    for name, frames in clips.items():
        for frame in frames:
            if not isinstance(frame, dict):
                raise ValueError('game frame: expected object')
            if set(frame) != {'ticks', 'rows', 'anchors'}:
                raise ValueError('game frame: unknown or missing fields')
            _integer(frame.get('ticks'), 'game.ticks', 1, _MAX_TICKS)
            rows = frame.get('rows')
            if not isinstance(rows, list) or len(rows) != h or any(
                    not isinstance(r, str) or not r.isascii() or len(r) != w or set(r)-set(pal)-{'.'} for r in rows):
                raise ValueError('game.rows: invalid dimensions or unknown inks')
            if name == 'idle' and any(c != '.' for row in rows for c in row):
                idle_ink = True
            if not isinstance(frame.get('anchors'), dict) or set(frame['anchors']) != set(ANCHORS):
                raise ValueError('game.anchors: missing attachments')
            for point in frame['anchors'].values():
                x, y = _point(point, 'game.anchor', integer=True)
                if not (0 <= x < w and 0 <= y < h):
                    raise ValueError('game.anchor outside canvas')
    if not idle_ink:
        raise ValueError('game: idle body has no ink')
    _json_text(game)  # same 8 MiB bound as Rust, including exported formatting
    return game


def frame_image(frame, *, root=None):
    """Decode final ink rows as RGBA, with fully transparent dot pixels."""
    pal = _palette(root)
    rows = frame['rows']
    if not rows or len({len(r) for r in rows}) != 1 or not rows[0]:
        raise ValueError('frame.rows: expected rectangular nonempty rows')
    _size([len(rows[0]), len(rows)], 'frame.rows', _MAX_CANVAS)
    rgba = np.zeros((len(rows), len(rows[0]), 4), np.uint8)
    for y, row in enumerate(rows):
        for x, c in enumerate(row):
            if c != '.':
                if c not in pal:
                    raise ValueError(f'frame.rows: unknown ink {c!r}')
                rgba[y, x] = (*pal[c], 255)
    return Image.fromarray(rgba)


def export(game, out, *, root=None, scale=1):
    """Write game.json, sheet.png (one clip per row), manifest and each clip GIF.

    JSON and the lossless PNG sheet always remain native-size. scale affects
    GIF previews only (nearest); Delve runs at 30 game ticks/second (not 60Hz).
    Round cumulative tick boundaries to GIF centiseconds to avoid per-frame
    drift, with disposal=2 and a reserved transparent palette index.
    Returns the sheet manifest with each clip's pixel rectangles and timings.
    """
    validate_game(game, root=root)
    _integer(scale, 'scale', 1, 32)
    pal = _palette(root)
    names = sorted(pal)
    if len(names) > 255:
        raise ValueError('GIF palette: more than 255 opaque inks')
    gif_palette = [0, 0, 0]+[v for c in names for v in pal[c]]
    gif_palette += [0]*(768-len(gif_palette))
    indices = {c: i+1 for i, c in enumerate(names)}
    w, h = game['canvas']
    sheet_size = (w*max(map(len, game['clips'].values())), h*len(game['clips']))
    if sheet_size[0]*sheet_size[1] > _MAX_OUTPUT_PIXELS:
        raise ValueError('sheet: padded layout exceeds output pixel budget')
    if w*h*scale**2*max(map(len, game['clips'].values())) > _MAX_OUTPUT_PIXELS:
        raise ValueError('GIF: scaled clip exceeds output pixel budget')
    sheet = Image.new('RGBA', sheet_size)
    manifest = dict(version=1, canvas=game['canvas'][:], pivot=game['pivot'][:],
                    sheet='sheet.png', clips={})
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    for row, (name, frames) in enumerate(game['clips'].items()):
        previews, durations, rects = [], [], []
        elapsed, previous_cs = 0, 0
        for col, frame in enumerate(frames):
            sheet.paste(frame_image(frame, root=root), (col*w, row*h))
            rects.append([col*w, row*h, w, h])
            pixels = np.array([[0 if c == '.' else indices[c] for c in line]
                               for line in frame['rows']], np.uint8)
            im = Image.fromarray(pixels).convert('P')
            im.putpalette(gif_palette)
            if scale != 1:
                im = im.resize((w*scale, h*scale), Image.Resampling.NEAREST)
            previews.append(im)
            elapsed += frame['ticks']
            cs = (elapsed*100 + TICKS_PER_SECOND//2)//TICKS_PER_SECOND
            durations.append(max(1, cs-previous_cs)*10)
            previous_cs = cs
        options = dict(save_all=True, append_images=previews[1:], duration=durations,
                       transparency=0, background=0, disposal=2, optimize=False)
        if name in ('idle', 'walk', 'run'):
            options['loop'] = 0
        previews[0].save(out/f'{name}.gif', **options)
        manifest['clips'][name] = dict(rects=rects, ticks=[f['ticks'] for f in frames],
                                       gif=f'{name}.gif')
    sheet.save(out/'sheet.png')
    for filename, data in (('game.json', game), ('sheet.json', manifest)):
        (out/filename).write_text(_json_text(data), encoding='utf-8')
    return manifest


def main(argv=None):
    ap = argparse.ArgumentParser(prog='spriterig quick', description=__doc__)
    sub = ap.add_subparsers(dest='command', required=True)
    fit_ap = sub.add_parser('fit', help='auto-fit a PNG/cell into editable rig JSON')
    fit_ap.add_argument('input')
    fit_ap.add_argument('--out', required=True, help='rig.json path')
    fit_ap.add_argument('--angelx', help='checkout containing the real Delve palette')
    fit_ap.add_argument('--cell', type=int, help='zero-based row-major sheet cell')
    fit_ap.add_argument('--cell-size', type=int, nargs=2, metavar=('W', 'H'))
    fit_ap.add_argument('--facing', choices=('right', 'left'), default='right')
    fit_ap.add_argument('--alpha-threshold', type=int, default=128)
    fit_ap.add_argument('--key-color', help='optional exact background RRGGBB')
    fit_ap.add_argument('--canvas', type=int, nargs=2, metavar=('W', 'H'))
    fit_ap.add_argument('--pivot', type=int, nargs=2, metavar=('X', 'Y'))
    build_ap = sub.add_parser('build', help='render a corrected rig and export game/sheet/GIFs')
    build_ap.add_argument('input')
    build_ap.add_argument('--rig', required=True)
    build_ap.add_argument('--out', required=True, help='output directory')
    build_ap.add_argument('--angelx')
    build_ap.add_argument('--clips', help='comma-separated subset (idle always included; default all saved clips)')
    build_ap.add_argument('--scale', type=int, default=1, help='GIF preview scale only')
    a = ap.parse_args(argv)
    try:
        if a.command == 'fit':
            rig = fit(a.input, root=a.angelx, cell=a.cell, cell_size=a.cell_size,
                      facing=a.facing, alpha_threshold=a.alpha_threshold,
                      key_color=a.key_color, canvas=a.canvas, pivot=a.pivot)
            save_rig(rig, a.out)
            print(f'Wrote {a.out}; edit joints, #/. masks, anchors or clip angles, then quick build.')
        else:
            rig = load_rig(a.rig)
            game = build(a.input, rig, root=a.angelx, clips=a.clips)
            export(game, a.out, root=a.angelx, scale=a.scale)
            save_rig(rig, Path(a.out)/'rig.json')
            print(f'Wrote {a.out}/game.json, sheet.png, sheet.json, rig.json and {len(game["clips"])} GIFs')
    except (ValueError, OSError) as exc:
        ap.error(str(exc))


if __name__ == '__main__':
    main()
