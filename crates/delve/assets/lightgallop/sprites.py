#!/usr/bin/env python3
"""Lightgallop's drawn slots: the rider, the snuffer, the floater, the warden, the spark and the pad.

The rider, the snuffer and the wisps are reduced from the art wave's concept
sheets (rider-gallop, rider-actions, foes: read-only references that stay
outside the repository), area-averaged to the native canvas, lifted a little
out of the shadow and snapped to the realm's inks. The spark, the pad and the
tuck-roll wheel are drawn here in pixels. The red cloth of the rider is also
written as a seat mask (each pixel's red is one of four bands) so the stage
can dress every seat in its own colour; nothing here repaints a delivered
pixel beyond the reduction and the ink snap.

    python3 -I crates/delve/assets/lightgallop/sprites.py <concept-dir>

writes crates/delve/assets/art-slots/lightgallop.<slot>/v1/<state>-<n>.png,
the seat masks, and each slot's map and provenance record.
"""
import colorsys
import hashlib
import json
import math
import re
import sys
from pathlib import Path

from PIL import Image, ImageFilter

ROOT = Path(__file__).resolve().parents[4]
ASSETS = ROOT / 'crates/delve/assets'
OUT = ASSETS / 'art-slots'
INK_RS = ROOT / 'crates/delve/src/render/overworld/ink.rs'

RAW = Path(sys.argv[1]) if len(sys.argv) > 1 else Path.home() / 'angelX_minigames/games/lightgallop/assets/raw'

INKS = {m.group(1): tuple(int(m.group(2)[i:i + 2], 16) for i in (0, 2, 4))
        for m in re.finditer(r"'(.)' => 0x([0-9a-f]{6})", INK_RS.read_text())}
# The pools each drawing is snapped to: no cold blue-blacks in the horse's
# mane, no white outside the steel.
RIDER = {c: INKS[c] for c in 'nbIBpPrRoOtTa78!?4@56kKZXgjGJhiH'}
SMOKE = {c: INKS[c] for c in 'kKZXgjGdc(h)[]bIBpPrRoOtTaHiwW9c7x48!?@56'}
TEAL = {c: INKS[c] for c in 'kKZXgjGJhiHsxSuUvVWqQz0123BpPrRoOtTa4@56'}

GAMMA = 0.80


def tone(rgb):
    return tuple(min(255.0, 255.0 * ((v / 255.0) ** GAMMA)) for v in rgb)


def with_gamma(g, fn, *a, **k):
    global GAMMA
    old, GAMMA = GAMMA, g
    try:
        return fn(*a, **k)
    finally:
        GAMMA = old


def near(rgb, pool):
    best, bd = None, 1e18
    r, g, b = rgb
    for c, (r2, g2, b2) in pool.items():
        rm = (r + r2) / 2
        d = (2 + rm / 256) * (r - r2) ** 2 + 4 * (g - g2) ** 2 + (2 + (255 - rm) / 256) * (b - b2) ** 2
        if d < bd:
            best, bd = c, d
    return best


def is_cloth(r, g, b):
    """Saturated red: the caparison, the surcoat and the plume."""
    mx, mn = max(r, g, b), min(r, g, b)
    if mx < 60 or (mx - mn) < 0.45 * mx:
        return False
    return r >= mx and g < 0.55 * r and b < 0.55 * r


def components(name, q=2, grow=1, minpix=300):
    im = Image.open(RAW / f'{name}.png').convert('RGBA')
    al = im.split()[3].point(lambda v: 255 if v > 100 else 0)
    small = al.resize((im.width // q, im.height // q), Image.BOX).point(lambda v: 255 if v > 40 else 0)
    if grow:
        small = small.filter(ImageFilter.MaxFilter(2 * grow + 1))
    w, h = small.size
    px = small.load()
    seen = [[False] * w for _ in range(h)]
    comps = []
    for y in range(h):
        for x in range(w):
            if px[x, y] and not seen[y][x]:
                st, n = [(x, y)], 0
                seen[y][x] = True
                x0 = x1 = x
                y0 = y1 = y
                while st:
                    cx, cy = st.pop()
                    n += 1
                    x0, x1, y0, y1 = min(x0, cx), max(x1, cx), min(y0, cy), max(y1, cy)
                    for dx in (-1, 0, 1):
                        for dy in (-1, 0, 1):
                            nx, ny = cx + dx, cy + dy
                            if 0 <= nx < w and 0 <= ny < h and px[nx, ny] and not seen[ny][nx]:
                                seen[ny][nx] = True
                                st.append((nx, ny))
                if n >= minpix:
                    comps.append((max(0, x0 * q - 4), max(0, y0 * q - 4), (x1 + 1) * q + 4, (y1 + 1) * q + 4))
    comps.sort(key=lambda b: (0 if (b[1] + b[3]) / 2 < im.height / 2 else 1, b[0]))
    return im, comps


def grid_cells(name):
    """A sheet whose figures keep to a four by two grid: each cell's opaque box."""
    im = Image.open(RAW / f'{name}.png').convert('RGBA')
    cw, ch = im.width // 4, im.height // 2
    out = []
    for r in range(2):
        for c in range(4):
            al = im.crop((c * cw, r * ch, (c + 1) * cw, (r + 1) * ch)).split()[3].point(lambda v: 255 if v > 100 else 0)
            b = al.getbbox()
            out.append((c * cw + max(0, b[0] - 4), r * ch + max(0, b[1] - 4), c * cw + b[2] + 4, r * ch + b[3] + 4))
    return im, out


def centroid(im, box):
    crop = im.crop(box).split()[3].point(lambda v: 255 if v >= 128 else 0)
    px = crop.load()
    w, h = crop.size
    sx = sy = n = 0
    for y in range(0, h, 2):
        for x in range(0, w, 2):
            if px[x, y]:
                sx += x
                sy += y
                n += 1
    return box[0] + sx / n, box[1] + sy / n


class Frame:
    """A native frame: ink letters by position, and the red cloth's bands."""

    def __init__(self, w, h):
        self.w, self.h = w, h
        self.ink = {}
        self.cloth = {}

    def shifted(self, dx, dy):
        f = Frame(self.w, self.h)
        f.ink = {(x + dx, y + dy): c for (x, y), c in self.ink.items() if 0 <= x + dx < self.w and 0 <= y + dy < self.h}
        f.cloth = {(x + dx, y + dy): c for (x, y), c in self.cloth.items() if 0 <= x + dx < self.w and 0 <= y + dy < self.h}
        return f


RED = ['8', 'p', '7', 'R']
BANDS = (45, 57, 67)


def reduce(im, box, s, canvas, at, pool, hue=None):
    """Area-reduce `box` of `im` by `s` onto a canvas with the crop's top-left at `at`."""
    crop = im.crop(box)
    w, h = crop.size
    px = crop.load()
    f = Frame(*canvas)
    for oy in range(canvas[1]):
        for ox in range(canvas[0]):
            x0, x1 = max(0, int((ox - at[0]) / s)), min(w, int((ox + 1 - at[0]) / s + 0.999))
            y0, y1 = max(0, int((oy - at[1]) / s)), min(h, int((oy + 1 - at[1]) / s + 0.999))
            if x1 <= x0 or y1 <= y0:
                continue
            n = a = cn = 0
            sr = sg = sb = csum = 0
            for yy in range(y0, y1):
                for xx in range(x0, x1):
                    r, g, b, al = px[xx, yy]
                    n += 1
                    if al >= 128:
                        a += 1
                        sr, sg, sb = sr + r, sg + g, sb + b
                        if pool is RIDER and is_cloth(r, g, b):
                            cn += 1
                            csum += (r * 3 + g * 6 + b) / 10
            if n == 0 or a / n < 0.45:
                continue
            rgb = tone((sr / a, sg / a, sb / a))
            if hue:
                rgb = hue(rgb)
            if pool is RIDER and cn / a > 0.5:
                lum = csum / cn
                band = sum(lum >= t for t in BANDS)
                f.cloth[(ox, oy)] = band
                f.ink[(ox, oy)] = RED[band]
            else:
                f.ink[(ox, oy)] = near(rgb, pool)
    return f


def place(im, box, s, src, dst, canvas, pool, hue=None):
    at = (dst[0] - (src[0] - box[0]) * s, dst[1] - (src[1] - box[1]) * s)
    return reduce(im, box, s, canvas, at, pool, hue)


# --- the rider ---------------------------------------------------------------------

RIDER_CANVAS = (48, 40)


def rider_frames(s=0.0955):
    gim, gc = components('rider-gallop', grow=0)
    aim, ac = components('rider-actions', grow=1)
    assert len(gc) == 8 and len(ac) == 8, (len(gc), len(ac))
    base = [max(b[3] for b in gc[:4]), max(b[3] for b in gc[4:])]
    ycs = []
    for i, b in enumerate(gc):
        _, cy = centroid(gim, b)
        ycs.append(38.6 - (base[i // 4] - cy) * s)
    ycg = sum(ycs) / len(ycs)
    out = {}
    for i, b in enumerate(gc):
        cx, _ = centroid(gim, b)
        out[f'gallop-{i}'] = place(gim, b, s, (cx, base[i // 4]), (24.0, 38.6), RIDER_CANVAS, RIDER)
    names = ['jump-rise', 'jump-fall', 'spur', 'lunge-0', 'lunge-1', 'hurt', 'crouch', 'victory']
    for name, b in zip(names, ac):
        cx, cy = centroid(aim, b)
        if name in ('spur', 'crouch', 'hurt', 'victory'):
            out[name] = place(aim, b, s, (cx, b[3]), (24.0, 38.6), RIDER_CANVAS, RIDER)
        else:
            out[name] = place(aim, b, s, (cx, cy), (24.0, ycg), RIDER_CANVAS, RIDER)
    return out


def wheel(rot):
    """The tuck roll: the knight curled behind his sun shield, a wheel of steel,
    red cloth and a gold eight-point sun."""
    f = Frame(*RIDER_CANVAS)
    cx = cy = 24
    for y in range(cy - 11, cy + 11):
        for x in range(cx - 11, cx + 11):
            dx, dy = x - cx + 0.5, y - cy + 0.5
            r = math.hypot(dx, dy)
            if r > 10.3:
                continue
            a = math.atan2(dy, dx) - rot
            if r > 9.0:
                f.ink[(x, y)] = 'k'
            elif r > 7.4:
                f.ink[(x, y)] = 'h' if (dx + dy) < 0 else 'G'
            else:
                ray = abs(math.cos(a * 4)) ** 6
                if r < 2.0 or r < 6.4 * ray:
                    f.ink[(x, y)] = '6' if r < 1.6 else '5' if r < 4.2 else '4'
                else:
                    shade = 3 if (dx + dy) < -3 else 2 if (dx + dy) < 2 else 1 if r < 6.4 else 0
                    f.ink[(x, y)] = RED[shade]
                    f.cloth[(x, y)] = shade
    return f


def rider_set(frames):
    f = frames
    st = {}
    st['gallop'] = [f[f'gallop-{i}'] for i in range(8)]
    st['idle'] = [f['gallop-0'], f['gallop-0'].shifted(0, 1)]
    st['jump-rise'] = [f['jump-rise']]
    st['jump-fall'] = [f['jump-fall']]
    st['roll'] = [wheel(k * math.pi / 8) for k in range(4)]
    st['crouch'] = [f['crouch']]
    st['charge'] = [f['crouch'], f['crouch'].shifted(0, 1)]
    st['spur'] = [f['spur'], f['gallop-6'], f['spur'].shifted(0, 1), f['gallop-6']]
    st['lunge'] = [f['lunge-0'], f['lunge-1']]
    st['skid'] = [f['crouch']]
    st['hurt'] = [f['hurt'], f['hurt'].shifted(0, 1)]
    st['victory'] = [f['victory'], f['victory'].shifted(0, -1), f['victory']]
    return st


# --- the wisps ----------------------------------------------------------------------

def to_teal(rgb):
    r, g, b = (v / 255 for v in rgb)
    h, s, v = colorsys.rgb_to_hsv(r, g, b)
    if 0.60 <= h <= 0.88:  # the violet smoke only; the bronze cap keeps its colour
        h = 0.52
        s = min(1.0, s * 1.5 + 0.18)
        v = min(1.0, v * 1.15)
    r, g, b = colorsys.hsv_to_rgb(h, s, v)
    return (r * 255, g * 255, b * 255)


def dither_fade(f, k):
    """Drop pixels through an ordered dither: k 0 whole, 1 gone."""
    bay = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]
    g = Frame(f.w, f.h)
    for (x, y), c in f.ink.items():
        if bay[y % 4][x % 4] / 16.0 >= k:
            g.ink[(x, y)] = c
    return g


def wisp_set(canvas, s, pool, hue=None, anchor=None, bottom=None):
    fim, fc = grid_cells('foes')
    idle_a, idle_b, hurt, pop, _, _, alert_a, alert_b = fc

    def put(b, scale=s):
        cx = (b[0] + b[2]) / 2
        if bottom is not None:
            return place(fim, b, scale, (cx, b[3]), (canvas[0] / 2, bottom), canvas, pool, hue)
        cy = (b[1] + b[3]) / 2
        return place(fim, b, scale, (cx, cy), (canvas[0] / 2, canvas[1] / 2), canvas, pool, hue)

    pops = []
    for k, grow in enumerate((0.55, 0.8, 1.0, 1.1)):
        f = put(pop, s * 0.8 * grow)
        pops.append(dither_fade(f, 0.0 if k < 3 else 0.5))
    return {
        'idle': [put(idle_a), put(idle_b)],
        'alert': [put(alert_a), put(alert_b)],
        'hurt': [put(hurt)],
        'pop': pops,
    }


def warden_set():
    st = with_gamma(1.0, wisp_set, (64, 112), 0.19, SMOKE, bottom=110.0)
    def eyes_red(f):
        g = Frame(f.w, f.h)
        g.ink = {p: ('7' if c in 'wWc9V' else c) for p, c in f.ink.items()}
        return g
    out = {'idle': [eyes_red(f) for f in st['idle']], 'hit': [eyes_red(st['hurt'][0])],
           'roar': [eyes_red(st['alert'][0])]}
    out['defeat'] = [dither_fade(eyes_red(st['hurt'][0]), k) for k in (0.0, 0.25, 0.5, 0.75)]
    return out


# --- the spark and the pad ----------------------------------------------------------

def spark_set():
    def star(arm, core, inner, outer):
        f = Frame(10, 10)
        c = 4.5
        for y in range(10):
            for x in range(10):
                dx, dy = abs(x - c), abs(y - c)
                d = dx + dy
                if dx < 0.9 and dy <= arm or dy < 0.9 and dx <= arm:
                    f.ink[(x, y)] = outer
                if d <= core + 0.6:
                    f.ink[(x, y)] = inner
                if d <= 1.1:
                    f.ink[(x, y)] = '6'
                if d <= 0.6:
                    f.ink[(x, y)] = '9'
        return f
    idle = [star(4, 2.4, '5', '4'), star(3, 2.0, '5', '4'), star(4, 2.8, '6', '5'), star(3, 2.0, '5', '4')]
    loose = [star(3, 2.0, '6', '5'), star(2, 1.4, '5', '4')]
    return {'idle': idle, 'loose': loose}


def pad_set():
    frames = []
    for k in range(4):
        f = Frame(40, 8)
        for x in range(40):
            for y in range(1, 8):
                edge = y == 1 or y == 7 or x in (0, 39)
                f.ink[(x, y)] = 'j' if edge else ('X' if (x + y) % 2 else 'g')
        f.ink[(0, 7)] = f.ink[(39, 7)] = None
        for n in range(4):
            x0 = 4 + n * 9 + (k * 2) % 9
            lit = '6' if n == (k % 4) else '4'
            for d in range(3):
                for yy, off in ((2 + d, d), (6 - d, d)):
                    f.ink[(x0 + off, yy)] = lit
        f.ink = {p: c for p, c in f.ink.items() if c}
        frames.append(f)
    return {'idle': frames}


# --- writing ------------------------------------------------------------------------

def save(slot, state_frames, canvas, anchor, provenance):
    d = OUT / f'lightgallop.{slot}'
    v1 = d / 'v1'
    v1.mkdir(parents=True, exist_ok=True)
    for old in v1.glob('*.png'):
        old.unlink()
    pieces = []
    for state, frames in state_frames.items():
        for i, f in enumerate(frames):
            name = f'{state}-{i}'
            im = Image.new('RGBA', canvas, (0, 0, 0, 0))
            for (x, y), c in f.ink.items():
                im.putpixel((x, y), INKS[c] + (255,))
            im.save(v1 / f'{name}.png')
            if f.cloth:
                mask = Image.new('RGBA', canvas, (0, 0, 0, 0))
                for (x, y), band in f.cloth.items():
                    mask.putpixel((x, y), (band * 70, 0, 0, 255))
                mask.save(v1 / f'{name}-seat-mask.png')
            pieces.append({'name': name, 'state': state, 'frame': i, 'runtime': True, 'approved': True,
                           'label': f'{state}-{i}: reduced to the native canvas and snapped to the realm inks.'})
    (d / f'lightgallop.{slot}.map.json').write_text(json.dumps({
        'anchor_canvas_wh': list(canvas), 'anchor_xy': list(anchor), 'embed_kits': ['v1'],
        'runtime': True, 'pieces': pieces,
        'geometry': 'The road, every collision box and every timing belong to the rules (rules/lightgallop); art never sets a hitbox or the terrain.',
    }, indent=1, sort_keys=True) + '\n')
    (d / 'provenance.json').write_text(json.dumps(provenance, indent=1, sort_keys=True) + '\n')


def main():
    sheets = {n: hashlib.sha256((RAW / f'{n}.png').read_bytes()).hexdigest() for n in ('rider-gallop', 'rider-actions', 'foes')}
    note = ('Reduced by crates/delve/assets/lightgallop/sprites.py from the art wave concept sheets '
            '(Luna via Codex CLI, kept outside the repository, bound here by SHA-256), area-averaged, '
            'lifted out of the shadow (gamma 0.8) and snapped to the realm inks.')
    rider = rider_set(rider_frames())
    save('rider', rider, RIDER_CANVAS, (24, 40), {
        'generator': 'Luna (gpt-6-luna) via Codex CLI, reduced by sprites.py', 'note': note,
        'sheets': {k: sheets[k] for k in ('rider-gallop', 'rider-actions')},
        'seat_mask': 'v1/<frame>-seat-mask.png: the red cloth, its red value in four bands (value/70), re-inked per seat by the stage',
        'wheel': 'the tuck roll is drawn in code (a steel wheel with the sun shield), not reduced'})
    snuffer = with_gamma(1.0, wisp_set, (28, 30), 0.075, SMOKE, bottom=29.6)
    save('snuffer', snuffer, (28, 30), (14, 30), {'generator': 'Luna, reduced by sprites.py', 'note': note, 'sheets': {'foes': sheets['foes']}})
    floater = with_gamma(0.9, wisp_set, (28, 28), 0.075, TEAL, hue=to_teal)
    save('floater', floater, (28, 28), (14, 28), {'generator': 'Luna, reduced by sprites.py', 'note': note + ' The violet smoke is turned to teal.', 'sheets': {'foes': sheets['foes']}})
    save('warden', warden_set(), (64, 112), (32, 112), {'generator': 'Luna, reduced by sprites.py', 'note': note + ' The snuffer drawn large, its eyes red.', 'sheets': {'foes': sheets['foes']}})
    save('spark', spark_set(), (10, 10), (5, 5), {'generator': 'drawn in sprites.py', 'note': 'Drawn in pixels.'})
    save('pad', pad_set(), (40, 8), (20, 8), {'generator': 'drawn in sprites.py', 'note': 'Drawn in pixels.'})
    print('wrote the slots')


if __name__ == '__main__':
    main()
