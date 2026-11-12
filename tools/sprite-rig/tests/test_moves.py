"""Move grammar, library rules and builds on the seven showcase sprites."""
from pathlib import Path

import pytest

from spriterig import quick as Q
from spriterig import smartfit as S
from spriterig.moves import MoveError, apply, check, dump, library, parse

ROOT = Path(__file__).resolve().parents[3]
SPRITES = sorted((ROOT / '.we-out' / 'src').glob('*.png'))


def test_grammar_round_trip():
    assert len(list((Path(__file__).resolve().parents[1] / 'spriterig' / 'moves').glob('*.move'))) >= 16
    for move in library():
        text = dump(move)
        assert parse(text) == move
        assert dump(parse(text)) == text
        path = Path(__file__).resolve().parents[1] / 'spriterig' / 'moves' / f'{move["name"]}.move'
        assert path.read_text(encoding='utf-8') == text


def test_library_moves_pass_check():
    moves = library()
    assert len(moves) >= 16
    names = {m['name'] for m in moves}
    assert {
        'spin_slash', 'uppercut', 'ground_pound', 'leap_strike', 'double_stab',
        'shield_bash', 'heel_kick', 'headbutt', 'tail_whip', 'wing_buffet',
        'bite_lunge', 'hammer_wind_up', 'flurry', 'rising_cast', 'slam_cast', 'taunt',
    } <= names
    assert any('back' in frame['angles'] for frame in next(m for m in moves if m['name'] == 'tail_whip')['frames'])
    assert any('back' in frame['angles'] for frame in next(m for m in moves if m['name'] == 'wing_buffet')['frames'])
    for move in moves:
        assert check(move) == move
        assert move['slot'] in ('slash', 'overhead', 'thrust', 'flail', 'cast')


@pytest.mark.parametrize('sprite', SPRITES, ids=lambda p: p.stem)
def test_library_builds_on_every_sprite(sprite):
    rig = S.fit(str(sprite), root=ROOT)
    for move in library():
        applied = apply(rig, move)
        assert applied is not rig
        assert applied['clips'][move['slot']] != rig['clips'][move['slot']]
        height = rig['source']['size'][1]
        authored = next(frame['offset'] for frame in move['frames'] if frame['offset'] != [0, 0])
        scaled = next(frame['offset'] for frame in applied['clips'][move['slot']] if frame['offset'] != [0, 0])
        assert scaled[0] == pytest.approx(authored[0] * height / 24)
        assert scaled[1] == pytest.approx(authored[1] * height / 24)
        game = Q.build(str(sprite), applied, root=ROOT, clips=['idle', move['slot']])
        Q.validate_game(game, root=ROOT)
        assert move['slot'] in game['clips']
        assert [f['ticks'] for f in game['clips'][move['slot']]] == [f['ticks'] for f in move['frames']]


def _boom(text):
    with pytest.raises(MoveError) as caught:
        parse(text)
    return str(caught.value)


def test_limits_rejected_with_line_numbers():
    header = 'move bad slot=slash\n'
    # Unknown part and unknown key name the offending line.
    assert 'line 2:' in _boom(header + 'f ticks=4 nose=10\nf ticks=4 torso=1\n')
    assert 'line 2:' in _boom(header + 'f ticks=4 bounce=1\nf ticks=4 torso=1\n')
    assert 'line 2:' in _boom(header + 'f ticks=0 torso=10\nf ticks=4 torso=1\n')
    assert 'line 2:' in _boom(header + 'f ticks=31 torso=10\nf ticks=4 torso=-10\n')
    assert 'line 2:' in _boom(header + 'f ticks=4 upper_arm_f=181\nf ticks=4 torso=1\n')
    assert 'line 2:' in _boom(header + 'f ticks=4 upper_arm_f=-181\nf ticks=4 torso=1\n')
    assert 'line 3:' in _boom(header + 'f ticks=4 torso=10\nf ticks=4 off=0,13\n')
    assert 'line 3:' in _boom(header + 'f ticks=4 torso=10\nf ticks=4 off=-13,0\n')
    assert 'line 1:' in _boom('move bad slot=walk\nf ticks=4 torso=10\nf ticks=4 torso=-10\n')
    one = _boom(header + 'f ticks=4 upper_arm_f=20\n')
    assert 'line 1:' in one and '2..12' in one
    many = header + ''.join(f'f ticks=2 torso={i}\n' for i in range(1, 14))
    assert 'line 14:' in _boom(many)
    long = header + 'f ticks=30 torso=10\n' * 3 + 'f ticks=1 torso=-10\n'
    assert 'line 5:' in _boom(long)
    # A key that is not a part and not a frame field.
    assert 'line 1:' in _boom('move bad slot=slash mystery=1\nf ticks=4 torso=1\nf ticks=4 torso=-1\n')
