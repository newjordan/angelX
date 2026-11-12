"""Silhouette-aware fit for the quick rig.

`quick.fit` cuts a sprite at fixed proportions (neck at 25% of the height, hips
at 60%, arms by distance from the centre line). That slices big heads through
the mouth, splits robes into two scissoring legs and mis-cuts wide arms. This
fit reads the drawing instead: the dark outline the sprite already carries is
the separator between parts.

  neck   the narrowest interior row between the head and the shoulders
  hips   the highest row from which a gap runs down between two legs;
         no gap at all means a robe (one lower mass, plan "robe")
  arms   interior masses beside the torso that the outline separates from it
  joints where the parts actually meet

It returns the same rig schema as `quick.fit`, so `quick.build` is unchanged.
A sprite with no readable outline falls back to `quick.fit`.
"""
import numpy as np

from . import quick as Q
from .quick_motion import defaults

OUTLINE = set('kKZXdnfs')
FAMILIES = ('kKZXgjGJhiH', 'fDFEelNmLAMyCY', 'nbIBpPrRoOtT', 'sxSuUvVW', 'qQz0123w', 'a4@56', '$c9', '87', 'd()[]^', '!?;%+')
FAMILY = {c: i for i, f in enumerate(FAMILIES) for c in f}


def _components(mask):
    """4-connected components of a boolean grid -> int labels (0 = none)."""
    lab = np.zeros(mask.shape, int)
    n = 0
    h, w = mask.shape
    for y in range(h):
        for x in range(w):
            if mask[y, x] and not lab[y, x]:
                n += 1
                stack = [(y, x)]
                lab[y, x] = n
                while stack:
                    cy, cx = stack.pop()
                    for ny, nx in ((cy-1, cx), (cy+1, cx), (cy, cx-1), (cy, cx+1)):
                        if 0 <= ny < h and 0 <= nx < w and mask[ny, nx] and not lab[ny, nx]:
                            lab[ny, nx] = n
                            stack.append((ny, nx))
    return lab, n


def _spread(labels, mask):
    """Give every unlabelled pixel of mask the label of its nearest labelled one."""
    labels = labels.copy()
    h, w = mask.shape
    todo = mask & (labels == '')
    while todo.any():
        grown = labels.copy()
        for y, x in zip(*np.nonzero(todo)):
            for ny, nx in ((y, x-1), (y, x+1), (y-1, x), (y+1, x),
                           (y-1, x-1), (y-1, x+1), (y+1, x-1), (y+1, x+1)):
                if 0 <= ny < h and 0 <= nx < w and labels[ny, nx] != '':
                    grown[y, x] = labels[ny, nx]
                    break
        if (grown == labels).all():
            labels[todo] = 'torso'
            break
        labels = grown
        todo = mask & (labels == '')
    return labels


def fit(image, *, root=None, parts=None, **kw):
    """Fit by silhouette; `parts` (rows of part letters) overrides the reading."""
    base = Q.fit(image, root=root, **kw)
    rgba = Q._image(image, cell=kw.get('cell'), cell_size=kw.get('cell_size'),
                    facing=kw.get('facing', 'right'),
                    alpha_threshold=kw.get('alpha_threshold', 128),
                    key_color=kw.get('key_color'))
    pal = Q._palette(root)
    inks = Q._inks(rgba, pal)
    m = rgba[..., 3] != 0
    h, w = m.shape
    ink = np.array([list(r.ljust(w, '.')) for r in Q._rows(inks)]) if not isinstance(inks, np.ndarray) else inks
    dark = np.isin(ink, list(OUTLINE)) & m
    border = m & ~(np.roll(m, 1, 0) & np.roll(m, -1, 0) & np.roll(m, 1, 1) & np.roll(m, -1, 1))
    if border.sum() == 0 or (dark & border).sum() < 0.7 * border.sum():
        return base
    inner = m & ~dark
    yy, xx = np.nonzero(m)
    y0, y1 = int(yy.min()), int(yy.max())
    height = y1 - y0 + 1
    width = inner.sum(1)

    # Neck: where the material changes under the head (helm to plate, face to robe).
    cx = float(np.median(xx))
    half = max(2, int(round(height * .17)))
    c0, c1 = max(0, int(cx) - half), min(w, int(cx) + half + 1)

    def families(y):
        out = np.zeros(len(FAMILIES))
        for c in ink[y, c0:c1][inner[y, c0:c1]]:
            out[FAMILY[c]] += 1
        return out / out.sum() if out.sum() else None
    lo, hi = y0 + int(height * .2), y0 + int(height * .55)
    scores = {}
    for y in range(lo, hi + 1):
        below = families(y)
        up = y - 1
        while up > y0 and families(up) is None:
            up -= 1
        above = families(up)
        if below is not None and above is not None:
            scores[y] = float(np.abs(above - below).sum())
    # A hood or helm changes material at the face too: the neck is the lowest strong change.
    strong = [y for y, v in scores.items() if v >= .6 * max(scores.values()) and v > .5] if scores else []
    neck = max(strong) if strong else None
    if neck is None:  # one material head to toe (a skeleton, a slime): the neck is the narrowest row
        full = m.sum(1)
        rows_ = [y for y in range(lo, hi + 1) if full[y] > 0]
        if rows_:
            neck = min(rows_, key=lambda y: (full[y], y)) + 1
    if neck is None:
        return base

    # Hips: the highest row from which one gap between two legs runs to the floor.
    def gap(y, near, reach):
        row = inner[y]
        xs = np.nonzero(row)[0]
        if len(xs) < 2:
            return None
        hits = [x for x in range(xs.min() + 1, xs.max())
                if not row[x] and abs(x - near) <= reach]
        return min(hits, key=lambda x: abs(x - near)) if hits else None
    floor_row = y1 if width[y1] else y1 - 1
    hip, split = None, gap(floor_row, cx, max(2, height * .12))
    y = floor_row
    while split is not None and y > y0 + height * .45:
        g = gap(y, split, 1)
        if g is None:
            break
        hip = y
        y -= 1
    robe = hip is None or y1 - hip + 1 < max(2, height * .12)
    if robe:
        hip = y1  # only the hem row belongs to the feet
        split = int(round(cx))

    labels = np.full((h, w), '', dtype='<U16')
    rows = np.arange(h)[:, None]
    cols = np.arange(w)[None, :]
    labels[inner & (rows < neck)] = 'head'

    # Arms: interior masses beside the torso, cut off from it by the outline.
    arm, wings = {}, None
    arm_floor = min(y1, hip + int(height * .12)) if not robe else y0 + int(height * .75)
    for start in range(neck, min(neck + 4, arm_floor)):
        zone = inner & (rows >= start) & (rows <= arm_floor)
        lab, n = _components(zone)
        if n < 2:
            continue
        mid = [lab[yy_, int(round(cx))] for yy_ in range(start, arm_floor + 1) if lab[yy_, int(round(cx))]]
        if not mid:
            continue
        torso_id = max(set(mid), key=mid.count)
        found = {'b': [], 'f': []}
        for i in range(1, n + 1):
            if i == torso_id:
                continue
            ys, xs = np.nonzero(lab == i)
            if len(ys) < 2 or ys.min() > start + height * .2:
                continue  # a leg or a stray, not an arm
            found['b' if np.median(xs) < cx else 'f'].append((abs(float(np.median(xs)) - cx), i))
        if not (found['b'] and found['f']) and start < min(neck + 4, arm_floor) - 1:
            continue
        arm, wings = {}, np.zeros((h, w), bool)
        for side, items in found.items():
            items.sort()
            for rank, (_, i) in enumerate(items):
                piece = lab == i
                xs = np.nonzero(piece)[1]
                # the shoulder pixels above the cut belong to the piece too
                piece = piece | (inner & (rows >= neck) & (rows < start) & (cols >= xs.min()) & (cols <= xs.max()))
                if rank == 0:
                    arm[side] = piece  # the mass nearest the body is the arm
                else:
                    wings |= piece     # anything further out is worn on the back: wings, a cape
        break
    if wings is not None:
        labels[wings] = 'back'
    for side, piece in arm.items():
        ys = np.nonzero(piece)[0]
        top, bot = ys.min(), ys.max()
        length = bot - top + 1
        hand = 2 if length >= 6 else 1
        elbow = top + (length - hand + 1) // 2
        labels[piece] = 'upper_arm_' + side
        labels[piece & (rows >= elbow)] = 'lower_arm_' + side
        labels[piece & (rows > bot - hand)] = 'hand_' + side

    free = inner & (labels == '')
    if robe:
        hem = free & (rows == y1 if width[y1] else rows == y1 - 1)
        labels[hem & (cols < cx)] = 'foot_b'
        labels[hem & (cols >= cx)] = 'foot_f'
    else:
        leg = free & (rows >= hip)
        length = y1 - hip + 1
        foot = 2 if length >= 6 else 1
        knee = hip + (length - foot + 1) // 2
        for side, sm in (('b', cols < split), ('f', cols > split)):
            labels[leg & sm] = 'upper_leg_' + side
            labels[leg & sm & (rows >= knee)] = 'lower_leg_' + side
            labels[leg & sm & (rows > y1 - foot)] = 'foot_' + side
    labels[inner & (labels == '')] = 'torso'
    for side in ('b', 'f'):  # a side with no readable arm keeps the proportional cut
        if side not in arm:
            for name in ('upper_arm_', 'lower_arm_', 'hand_'):
                old = np.array([[c == '#' for c in r] for r in base['parts'][name + side]['mask']])
                labels[old & inner & (labels == 'torso')] = name + side
    labels = _spread(labels, m)
    if parts is not None:
        key = {'h': 'head', 't': 'torso', 'c': 'back',
               'a': 'upper_arm_b', 'b': 'lower_arm_b', 'd': 'hand_b',
               'A': 'upper_arm_f', 'B': 'lower_arm_f', 'D': 'hand_f',
               'l': 'upper_leg_b', 'm': 'lower_leg_b', 'f': 'foot_b',
               'L': 'upper_leg_f', 'M': 'lower_leg_f', 'F': 'foot_f'}
        for y, r in enumerate(parts):
            for x, c in enumerate(r):
                if c in key and m[y, x]:
                    labels[y, x] = key[c]

    # Every contact part needs at least one pixel: a hidden hand or foot takes
    # the end of its limb, or the body pixel nearest where it would be.
    for s_ in ('b', 'f'):
        for distal, chain, joint in (('hand_', ('lower_arm_', 'upper_arm_'), 'wrist_'),
                                     ('foot_', ('lower_leg_', 'upper_leg_'), 'ankle_')):
            if np.any(labels == distal + s_):
                continue
            pool = m & np.isin(labels, [c + s_ for c in chain])
            target = base['joints'][joint + s_]
            if not pool.any():
                pool = m & (labels == 'torso')
                side = (cols < cx) if s_ == 'b' else (cols >= cx)
                pool = pool & side if (pool & side).any() else pool
            x, y = Q._contact(pool, target, bottom=True)
            labels[y, x] = distal + s_

    def med(name, axis, fallback):
        ys, xs = np.nonzero(labels == name)
        return float(np.median((xs, ys)[axis])) if len(xs) else fallback

    def top(name, fallback):
        ys = np.nonzero(labels == name)[0]
        return float(ys.min()) if len(ys) else fallback

    joints = {'root': [round(cx, 3), float(hip if not robe else y0 + height * .6)],
              'neck': [round(cx, 3), float(neck)]}
    for s, sign in (('b', -1), ('f', 1)):
        old = base['joints']
        ax = med('upper_arm_' + s, 0, old['shoulder_' + s][0])
        ys_, xs_ = np.nonzero(labels == 'upper_arm_' + s)
        if len(xs_):  # pivot on the torso side of a thick arm, so it swings without leaving the body
            wide = xs_.max() - xs_.min() + 1
            ax_j = (xs_.max() - min(1, wide // 3)) if s == 'b' else (xs_.min() + min(1, wide // 3))
        else:
            ax_j = ax
        joints['shoulder_' + s] = [float(ax_j), top('upper_arm_' + s, old['shoulder_' + s][1]) + 1.0]
        joints['elbow_' + s] = [med('lower_arm_' + s, 0, ax), top('lower_arm_' + s, old['elbow_' + s][1])]
        joints['wrist_' + s] = [med('hand_' + s, 0, ax), top('hand_' + s, old['wrist_' + s][1])]
        lx = med('upper_leg_' + s, 0, med('foot_' + s, 0, old['hip_' + s][0]))
        joints['hip_' + s] = [lx, top('upper_leg_' + s, old['hip_' + s][1])]
        joints['knee_' + s] = [med('lower_leg_' + s, 0, lx), top('lower_leg_' + s, old['knee_' + s][1])]
        joints['ankle_' + s] = [med('foot_' + s, 0, lx), top('foot_' + s, old['ankle_' + s][1])]
    rig = dict(base)
    rig['joints'] = {k: [round(float(np.clip(v[0], 0, w-1)), 3), round(float(np.clip(v[1], 0, h-1)), 3)]
                     for k, v in joints.items()}
    rig['parts'] = {name: dict(parent=parent, joint=joint, z=z,
                               mask=Q._rows(np.where(m & (labels == name), '#', '.')))
                    for name, parent, joint, z in Q._LAYOUT}

    def contact(name, target, bottom=False):
        return Q._contact(m & (labels == name), target, bottom=bottom)
    j = rig['joints']
    has = lambda n: bool(np.any(labels == n))
    floor = base['origin']
    grip = contact('hand_f', j['wrist_f']) if has('hand_f') else base['anchors']['grip']['point']
    off = contact('hand_b', j['wrist_b']) if has('hand_b') else base['anchors']['off_hand']['point']
    foot = 'foot_f' if has('foot_f') else 'foot_b' if has('foot_b') else base['anchors']['feet']['part']
    back = 'back' if has('back') else 'torso'
    rig['anchors'] = {
        'grip': dict(part='hand_f', point=grip),
        'off_hand': dict(part='hand_b', point=off),
        'head': dict(part='head', point=contact('head', (cx, neck), bottom=True)),
        'back': dict(part=back, point=contact(back, (cx, (neck + j['root'][1]) / 2))),
        'feet': dict(part=foot, point=contact(foot, floor, bottom=True) if has(foot) else base['anchors']['feet']['point']),
        'tip': dict(part='hand_f', point=[grip[0], round(grip[1] + height * .45, 3)]),
    }
    rig['clips'] = defaults(height)
    rig['plan'] = 'robe' if robe else 'biped'
    return Q.validate_rig(rig)
