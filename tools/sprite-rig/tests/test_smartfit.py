"""The silhouette fit reads each bestiary sprite the way an artist would cut it."""
from pathlib import Path

import numpy as np
import pytest
from PIL import Image

from spriterig import quick as Q, smartfit as S
from spriterig.delve.source import palette

HERE = Path(__file__).resolve().parents[1] / 'samples' / 'bestiary'
NAMES = sorted(p.stem for p in HERE.glob('*.txt'))
ROBES = {'mage', 'witch', 'ghost'}


def sprite(name, tmp_path):
    pal = palette()
    rows = [l.rstrip('\n') for l in (HERE / f'{name}.txt').read_text().splitlines() if l.strip()]
    w = max(map(len, rows))
    im = Image.new('RGBA', (w, len(rows)))
    for y, r in enumerate(rows):
        for x, c in enumerate(r):
            if c != '.':
                im.putpixel((x, y), pal[c] + (255,))
    path = tmp_path / f'{name}.png'
    im.save(path)
    return path, rows


def mask(rig, part):
    return np.array([[c == '#' for c in r] for r in rig['parts'][part]['mask']])


def test_there_is_a_bestiary():
    assert len(NAMES) >= 12


@pytest.mark.parametrize('name', NAMES)
def test_every_pixel_belongs_to_exactly_one_part(name, tmp_path):
    path, rows = sprite(name, tmp_path)
    rig = S.fit(path)
    total = sum(mask(rig, p).astype(int) for p in Q.PARTS)
    ink = np.array([[c != '.' for c in r.ljust(total.shape[1], '.')] for r in rows])
    assert (total == ink).all()


@pytest.mark.parametrize('name', NAMES)
def test_plan_is_read_from_the_legs(name, tmp_path):
    path, _ = sprite(name, tmp_path)
    assert S.fit(path)['plan'] == ('robe' if name in ROBES else 'biped')


@pytest.mark.parametrize('name', NAMES)
def test_the_head_is_whole_and_above_the_torso(name, tmp_path):
    path, _ = sprite(name, tmp_path)
    rig = S.fit(path)
    head, torso = mask(rig, 'head'), mask(rig, 'torso')
    assert head.sum() >= 12 and torso.sum() >= 6
    assert np.nonzero(head)[0].mean() < np.nonzero(torso)[0].mean()
    # the eyes (the top third of the figure) are never cut off the head
    ys = np.nonzero(head | torso)[0]
    top_third = ys.min() + (ys.max() - ys.min()) // 4
    assert not torso[:top_third].any()


@pytest.mark.parametrize('name', NAMES)
def test_both_arms_are_found_beside_the_body(name, tmp_path):
    path, _ = sprite(name, tmp_path)
    rig = S.fit(path)
    cx = rig['joints']['neck'][0]
    for side, sign in (('b', -1), ('f', 1)):
        arm = mask(rig, 'upper_arm_' + side) | mask(rig, 'lower_arm_' + side) | mask(rig, 'hand_' + side)
        assert arm.any(), side
        assert sign * (np.nonzero(arm)[1].mean() - cx) > 0, side


def test_wings_go_on_the_back_not_the_arms(tmp_path):
    path, _ = sprite('imp', tmp_path)
    rig = S.fit(path)
    assert mask(rig, 'back').sum() >= 20
    assert mask(rig, 'upper_arm_b').sum() <= 6


@pytest.mark.parametrize('name', NAMES)
def test_it_builds_every_clip_and_idle_starts_as_drawn(name, tmp_path):
    path, rows = sprite(name, tmp_path)
    rig = S.fit(path)
    game = Q.build(path, rig)
    Q.validate_game(game)
    assert set(game['clips']) == set(Q.CLIPS)
    first = [r.strip('.') for r in game['clips']['idle'][0]['rows'] if r.strip('.')]
    drawn = [r.strip('.') for r in rows if r.strip('.')]
    assert first == drawn


def test_a_sprite_with_no_outline_falls_back_to_the_proportional_fit():
    squire = HERE.parent / 'quick' / 'squire.png'  # flat colour, no dark outline
    assert S.fit(squire) == Q.fit(squire)
