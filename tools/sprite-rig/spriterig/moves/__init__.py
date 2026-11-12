"""Move grammar, library and animator's-rule checker for the quick rig.

A move is plain text a language model can write and a game can parse. Angles
are degrees, clockwise in image space (y down), relative to the rest pose and
to each part's parent. They do not scale. Offsets are pixels at a 24px sprite
and scale by ``source height / 24`` when applied. See ``dump`` for the
canonical text form.
"""

from __future__ import annotations

import copy
from pathlib import Path

from ..quick import PARTS

ATTACK_SLOTS = ('slash', 'overhead', 'thrust', 'flail', 'cast')
# Sheet and reel order, not filename order.
ORDER = (
    'spin_slash', 'uppercut', 'ground_pound', 'leap_strike', 'double_stab',
    'shield_bash', 'heel_kick', 'headbutt', 'tail_whip', 'wing_buffet',
    'bite_lunge', 'hammer_wind_up', 'flurry', 'rising_cast', 'slam_cast', 'taunt',
)
FLAGS = ('weapon', 'air')
# Canonical dump order. torso/head/back lead so a short line reads as a pose.
PART_ORDER = (
    'torso', 'head', 'back',
    'upper_arm_f', 'lower_arm_f', 'hand_f',
    'upper_arm_b', 'lower_arm_b', 'hand_b',
    'upper_leg_f', 'lower_leg_f', 'foot_f',
    'upper_leg_b', 'lower_leg_b', 'foot_b',
)
assert set(PART_ORDER) == set(PARTS)

MIN_FRAMES, MAX_FRAMES = 2, 12
MIN_TICKS, MAX_TICKS = 1, 30
MAX_ANGLE = 180
MAX_OFFSET = 12
MAX_TOTAL_TICKS = 90
MAX_LIFT = 6  # upward grammar-pixels unless tagged air
RECOVERY_DEGREES = 15
RECOVERY_TICKS = 4
ANTICIPATION_TICKS = 3

LIBRARY_DIR = Path(__file__).resolve().parent


class MoveError(ValueError):
    """Parse or rule failure. ``line`` is 1-based in the move text when known."""

    def __init__(self, line, message):
        self.line = line
        super().__init__(f'line {line}: {message}')


def _integer(token, line, what):
    if token is None or not _is_int(token):
        raise MoveError(line, f'{what} must be an integer')
    return int(token)


def _is_int(token):
    if token in ('', '-', '+') or token[0] == '+':
        return False
    body = token[1:] if token[0] == '-' else token
    return body.isdigit()


def parse(text):
    """Parse move text into ``{name, slot, weapon, air, frames}``.

    Raises ``MoveError`` (a ``ValueError``) whose message starts with
    ``line N:`` for unknown keys, unknown parts and limit failures.
    """
    if not isinstance(text, str):
        raise MoveError(1, 'expected text')
    raw_lines = text.splitlines()
    if text.endswith('\n'):
        # splitlines drops the trailing newline; a blank last line is real.
        pass
    lines = [(i + 1, line) for i, line in enumerate(raw_lines) if line.strip() != '']
    # Blank lines are not part of the canonical grammar.
    for n, line in enumerate(raw_lines, 1):
        if line.strip() == '':
            raise MoveError(n, 'blank line')
        if line != line.strip() or '  ' in line:
            raise MoveError(n, 'expected single spaces and no surrounding space')
    if not lines:
        raise MoveError(1, 'expected a move header')
    header_no, header = lines[0]
    move = _parse_header(header_no, header)
    frames = []
    total = 0
    for n, line in lines[1:]:
        frame = _parse_frame(n, line)
        total += frame['ticks']
        if total > MAX_TOTAL_TICKS:
            raise MoveError(n, f'total ticks {total} exceed {MAX_TOTAL_TICKS}')
        frames.append(frame)
    if not MIN_FRAMES <= len(frames) <= MAX_FRAMES:
        # Header is lines[0]; the first frame past the cap is the error site.
        where = lines[MAX_FRAMES + 1][0] if len(frames) > MAX_FRAMES else header_no
        raise MoveError(where, f'expected {MIN_FRAMES}..{MAX_FRAMES} frames, found {len(frames)}')
    move['frames'] = frames
    return move


def _parse_header(line_no, line):
    parts = line.split(' ')
    if len(parts) < 3 or parts[0] != 'move':
        raise MoveError(line_no, 'expected "move NAME slot=SLOT"')
    name = parts[1]
    if not name or not (name[0].isalpha() or name[0] == '_') or not all(c.isalnum() or c == '_' for c in name):
        raise MoveError(line_no, f'invalid move name {name!r}')
    slot = None
    flags = []
    for token in parts[2:]:
        if token in FLAGS:
            if token in flags:
                raise MoveError(line_no, f'duplicate flag {token}')
            flags.append(token)
            continue
        if '=' not in token:
            raise MoveError(line_no, f'unknown key {token!r}')
        key, value = token.split('=', 1)
        if key != 'slot' or not value:
            raise MoveError(line_no, f'unknown key {key!r}')
        if slot is not None:
            raise MoveError(line_no, 'duplicate key slot')
        slot = value
    if slot is None:
        raise MoveError(line_no, 'missing slot')
    if slot not in ATTACK_SLOTS:
        raise MoveError(line_no, f'unknown slot {slot!r}')
    return dict(name=name, slot=slot, weapon='weapon' in flags, air='air' in flags, frames=[])


def _parse_frame(line_no, line):
    parts = line.split(' ')
    if parts[0] != 'f':
        raise MoveError(line_no, 'expected a frame line starting with f')
    ticks = None
    rotation = 0
    offset = [0, 0]
    smear = False
    angles = {}
    seen = set()
    for token in parts[1:]:
        if token == 'smear':
            if smear:
                raise MoveError(line_no, 'duplicate key smear')
            smear = True
            continue
        if '=' not in token:
            raise MoveError(line_no, f'unknown key {token!r}')
        key, value = token.split('=', 1)
        if key in seen:
            raise MoveError(line_no, f'duplicate key {key}')
        seen.add(key)
        if key == 'ticks':
            ticks = _integer(value, line_no, 'ticks')
            if not MIN_TICKS <= ticks <= MAX_TICKS:
                raise MoveError(line_no, f'ticks {ticks} outside {MIN_TICKS}..{MAX_TICKS}')
        elif key == 'rotation':
            rotation = _integer(value, line_no, 'rotation')
            if abs(rotation) > MAX_ANGLE:
                raise MoveError(line_no, f'rotation {rotation} exceeds {MAX_ANGLE}')
        elif key == 'off':
            bits = value.split(',')
            if len(bits) != 2:
                raise MoveError(line_no, 'off expects X,Y')
            x = _integer(bits[0], line_no, 'offset')
            y = _integer(bits[1], line_no, 'offset')
            if abs(x) > MAX_OFFSET or abs(y) > MAX_OFFSET:
                raise MoveError(line_no, f'offset {[x, y]} exceeds {MAX_OFFSET}')
            offset = [x, y]
        elif key in PARTS:
            angle = _integer(value, line_no, key)
            if abs(angle) > MAX_ANGLE:
                raise MoveError(line_no, f'{key} {angle} exceeds {MAX_ANGLE}')
            angles[key] = angle
        else:
            raise MoveError(line_no, f'unknown key {key!r}')
    if ticks is None:
        raise MoveError(line_no, 'missing ticks')
    return dict(ticks=ticks, angles=angles, offset=offset, rotation=rotation, smear=smear)


def dump(move):
    """Canonical text. ``parse(dump(move))`` equals ``move`` and the text matches."""
    flags = [flag for flag in FLAGS if move.get(flag)]
    header = f"move {move['name']} slot={move['slot']}"
    if flags:
        header += ' ' + ' '.join(flags)
    lines = [header]
    for frame in move['frames']:
        bits = ['f', f"ticks={int(frame['ticks'])}"]
        rotation = int(frame.get('rotation', 0))
        if rotation:
            bits.append(f'rotation={rotation}')
        angles = frame.get('angles') or {}
        for part in PART_ORDER:
            if part in angles and int(angles[part]) != 0:
                bits.append(f'{part}={int(angles[part])}')
        off = frame.get('offset') or [0, 0]
        if int(off[0]) or int(off[1]):
            bits.append(f'off={int(off[0])},{int(off[1])}')
        if frame.get('smear'):
            bits.append('smear')
        lines.append(' '.join(bits))
    return '\n'.join(lines) + '\n'


def _angle(frame, part):
    return int(frame['angles'].get(part, 0))


def _file_line(frame_index):
    """1-based text line of a frame, assuming a single header line and no blanks."""
    return frame_index + 2


def check(move):
    """Enforce anticipation, contact, follow-through, recovery, body and feet.

    Returns the move when it passes. Raises ``MoveError``.
    """
    frames = move['frames']
    if not MIN_FRAMES <= len(frames) <= MAX_FRAMES:
        raise MoveError(1, f'expected {MIN_FRAMES}..{MAX_FRAMES} frames')
    changes = []
    for i in range(1, len(frames)):
        delta = {part: _angle(frames[i], part) - _angle(frames[i - 1], part) for part in PARTS}
        magnitude = max(abs(v) for v in delta.values())
        changes.append((magnitude, delta))
    if not changes or max(m for m, _ in changes) < 1:
        raise MoveError(1, 'no striking change')
    fastest = max(m for m, _ in changes)
    contact = next(i for i, (m, _) in enumerate(changes, 1) if m == fastest)
    for i, (m, _) in enumerate(changes, 1):
        if m == fastest and frames[i]['ticks'] > 3:
            raise MoveError(_file_line(i), f'fastest change uses {frames[i]["ticks"]} ticks; contact must be at most 3')
    _, delta = changes[contact - 1]
    striking = [part for part in PART_ORDER if abs(delta[part]) == fastest]
    antic_ticks = sum(frame['ticks'] for frame in frames[:contact])
    if antic_ticks < ANTICIPATION_TICKS:
        raise MoveError(2, f'anticipation lasts {antic_ticks} ticks; need at least {ANTICIPATION_TICKS}')
    for part in striking:
        opening = _angle(frames[0], part)
        wound = _angle(frames[contact - 1], part)
        if opening * delta[part] >= 0 or wound * delta[part] >= 0:
            raise MoveError(2, f'{part} does not anticipate opposite the strike')
    # Every striking part overshoots on some frame after contact and before recovery.
    for part in striking:
        if not any(
                contact < j < len(frames) - 1 and
                (_angle(frames[j], part) - _angle(frames[contact], part)) * delta[part] > 0
                for j in range(contact + 1, len(frames) - 1)):
            raise MoveError(_file_line(len(frames) - 1), f'{part} does not overshoot after contact')
    last = frames[-1]
    last_line = _file_line(len(frames) - 1)
    if last['ticks'] < RECOVERY_TICKS:
        raise MoveError(last_line, f'recovery lasts {last["ticks"]} ticks; need at least {RECOVERY_TICKS}')
    for part in PARTS:
        if abs(_angle(last, part)) > RECOVERY_DEGREES:
            raise MoveError(last_line, f'recovery {part} is {_angle(last, part)} degrees from rest')
    if abs(int(last.get('rotation', 0))) > RECOVERY_DEGREES:
        raise MoveError(last_line, 'recovery rotation is not near rest')
    if not any(_angle(frame, 'torso') or frame['offset'][0] or frame['offset'][1] for frame in frames):
        raise MoveError(1, 'body never moves')
    if not move.get('air'):
        for i, frame in enumerate(frames):
            if frame['offset'][1] < -MAX_LIFT:
                raise MoveError(_file_line(i), f'upward offset {frame["offset"][1]} exceeds {MAX_LIFT}px without air')
    if move.get('weapon'):
        nxt = frames[contact + 1]['smear'] if contact + 1 < len(frames) else False
        if not frames[contact]['smear'] and not nxt:
            raise MoveError(_file_line(contact), 'weapon contact needs smear on the contact frame or the next')
    return move


def apply(rig, move):
    """Return a copy of ``rig`` whose ``clips[slot]`` is this move.

    Angles are copied as authored. Offsets scale by sprite height / 24.
    """
    if move['slot'] not in rig.get('clips', {}):
        raise MoveError(1, f'rig has no {move["slot"]} clip')
    height = rig['source']['size'][1]
    scale = height / 24
    frames = []
    for frame in move['frames']:
        ox, oy = frame['offset']
        frames.append(dict(
            ticks=int(frame['ticks']),
            angles={part: int(angle) for part, angle in frame['angles'].items() if int(angle)},
            offset=[ox * scale, oy * scale],
            rotation=int(frame.get('rotation', 0)),
            smear=bool(frame.get('smear')),
        ))
    out = copy.deepcopy(rig)
    out['clips'][move['slot']] = frames
    return out


def library_dir():
    return LIBRARY_DIR


def library_files():
    return sorted(LIBRARY_DIR.glob('*.move'))


def load(path):
    path = Path(path)
    return parse(path.read_text(encoding='utf-8'))


def library():
    """Named moves shipped in ``spriterig/moves/*.move``, in ``ORDER``."""
    by_name = {}
    for path in library_files():
        move = load(path)
        if f'{move["name"]}.move' != path.name:
            raise MoveError(1, f'{path.name} declares {move["name"]}')
        by_name[move['name']] = move
    missing = [name for name in ORDER if name not in by_name]
    extra = sorted(set(by_name) - set(ORDER))
    if missing or extra or len(by_name) < 16:
        raise MoveError(1, f'library mismatch missing={missing} extra={extra}')
    return [by_name[name] for name in ORDER]


def resolve(name):
    """A library move name, or a path to move text."""
    path = LIBRARY_DIR / f'{name}.move'
    if path.is_file():
        return load(path)
    other = Path(name)
    if other.is_file():
        return load(other)
    known = ', '.join(m['name'] for m in library())
    raise MoveError(1, f'unknown move {name!r}; library: {known}')
