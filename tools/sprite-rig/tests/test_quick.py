"""Quick rig behavior: source fidelity, editable hierarchy, final pixel contract."""
import copy
import json

import numpy as np
from PIL import Image, ImageDraw, ImageSequence
import pytest

from spriterig import quick as Q
from spriterig.__main__ import main


@pytest.fixture(scope='module')
def pal():
    return Q._palette()


def humanoid(size=24, pal=None):
    """Asymmetric native sprite with separately visible hands/feet and a cape."""
    pal = pal or Q._palette()
    colors = list(dict.fromkeys(pal.values()))
    im = Image.new('RGBA', (24, 24))
    d = ImageDraw.Draw(im)
    def rect(box, i):
        d.rectangle(box, fill=(*colors[i], 255))
    rect((5, 7, 7, 19), 6)       # rear cape
    rect((8, 2, 14, 6), 12)      # head
    rect((14, 4, 16, 5), 13)     # facing-right nose
    rect((9, 7, 14, 13), 8)      # torso
    rect((7, 8, 8, 10), 10)      # rear upper arm
    rect((6, 11, 7, 13), 9)      # rear forearm
    rect((6, 14, 7, 16), 11)     # rear hand
    rect((15, 8, 16, 10), 10)    # front upper arm
    rect((16, 11, 17, 13), 9)    # front forearm
    rect((17, 14, 18, 16), 11)   # front hand
    rect((9, 14, 10, 20), 5)     # rear leg
    rect((12, 14, 14, 20), 7)    # front leg
    rect((8, 21, 11, 22), 8)     # rear foot
    rect((12, 21, 16, 22), 10)   # front foot
    return im if size == 24 else im.resize((size, size), Image.Resampling.NEAREST)


def array(frame):
    return np.array([list(row) for row in frame['rows']])


def rest_rig(im):
    rig = Q.fit(im)
    rig['clips'] = {'idle': [dict(ticks=6, angles={})]}
    return rig


def test_fit_disjoint_complete_segmented_json_and_no_resize(pal, tmp_path):
    im = humanoid(pal=pal)
    rig = Q.fit(im)
    assert set(rig['parts']) == set(Q.PARTS)
    assert {'head', 'torso', 'hand_f', 'hand_b', 'foot_f', 'foot_b', 'back'} <= set(Q.PARTS)
    masks = np.array([[list(r) for r in p['mask']] for p in rig['parts'].values()]) == '#'
    np.testing.assert_array_equal(masks.sum(0), np.asarray(im)[..., 3] > 0)
    assert sum(mask.any() for mask in masks) >= 12
    assert rig['source']['size'] == [24, 24]
    assert set(rig['clips']) == set(Q.CLIPS)
    assert set(rig['anchors']) == set(Q.ANCHORS)
    Q.save_rig(rig, tmp_path/'nested/rig.json')
    assert Q.load_rig(tmp_path/'nested/rig.json') == rig
    assert json.loads(json.dumps(rig)) == rig


@pytest.mark.parametrize('size', [16, 24, 32])
def test_all_clips_fixed_contract_source_fidelity_and_motion(size, pal):
    im = humanoid(size, pal)
    rig = Q.fit(im)
    before = copy.deepcopy(rig)
    game = Q.build(im, rig)
    assert rig == before
    assert set(game) == {'version', 'canvas', 'pivot', 'clips'}
    assert game['version'] == 1
    Q.validate_game(game)
    w, h = game['canvas']
    for name, frames in game['clips'].items():
        for frame in frames:
            assert set(frame) == {'ticks', 'rows', 'anchors'}
            assert len(frame['rows']) == h
            assert all(len(row) == w for row in frame['rows'])
            assert set(''.join(frame['rows'])) <= set(pal) | {'.'}
            assert frame['ticks'] > 0
            assert set(frame['anchors']) == set(Q.ANCHORS)
            assert all(type(v) is int for p in frame['anchors'].values() for v in p)
        assert len({tuple(f['rows']) for f in frames}) > 1, name
    # No extra pixels, recoloring, scale, accidental corner key, alpha halo or
    # duplicated silhouette at the zero pose, even with shared seam collars.
    expected = np.full((h, w), '.', dtype='<U1')
    ink = Q._inks(np.asarray(im), pal)
    ox, oy = np.array(rig['pivot'])-rig['origin']
    expected[oy:oy+size, ox:ox+size] = ink
    np.testing.assert_array_equal(array(game['clips']['idle'][0]), expected)
    # Intentional anticipation/action/recovery, not constant-speed waving.
    for name in ('slash', 'overhead', 'thrust', 'flail'):
        poses = rig['clips'][name]
        assert any(p['smear'] for p in poses)
        assert min(p['ticks'] for p in poses) < poses[0]['ticks']
        assert not poses[-1]['angles']
    # Death settles rather than snapping upright, and remains on one floor.
    die = [array(f) for f in game['clips']['die']]
    assert not np.array_equal(die[0], die[-1])
    assert all(np.nonzero(a != '.')[0].max() == rig['pivot'][1] for a in die)


def test_sheet_extraction_left_normalization_and_alpha(pal, tmp_path):
    im = humanoid(pal=pal)
    raw = np.asarray(im).copy()
    raw[0, 0] = [255, 0, 255, 127]
    raw[0, 1] = [*list(pal.values())[7], 128]
    selected = Image.fromarray(raw[:, ::-1])
    sheet = Image.new('RGBA', (48, 48), (255, 0, 0, 255))
    sheet.paste(selected, (24, 24))
    path = tmp_path/'sheet.png'
    sheet.save(path)
    rig = Q.fit(path, cell=3, cell_size=(24, 24), facing='left')
    rig['clips'] = {'idle': [dict(ticks=6)]}
    game = Q.build(path, rig)
    arr = array(game['clips']['idle'][0])
    dx, dy = np.array(rig['pivot'])-rig['origin']
    assert arr[dy, dx] == '.'
    assert arr[dy, dx+1] != '.'
    expected = Q._inks(Q._image(Image.fromarray(raw)), pal)
    np.testing.assert_array_equal(arr[dy:dy+24, dx:dx+24], expected)
    assert rig['source']['cell'] == 3
    # Palette mapping is Euclidean nearest and keeps alpha separate from RGB.
    opaque = Q._image(Image.fromarray(raw))
    opaque[4, 12] = [91, 132, 183, 255]
    ink = Q._inks(opaque, pal)[4, 12]
    assert sum((int(v)-p)**2 for v, p in zip(opaque[4, 12, :3], pal[ink])) == min(
        sum((int(v)-p)**2 for v, p in zip(opaque[4, 12, :3], rgb)) for rgb in pal.values())


def test_opaque_background_is_not_implicitly_keyed_and_key_is_exact():
    im = humanoid()
    opaque = Image.new('RGBA', im.size, '#ff00ff')
    opaque.alpha_composite(im)
    assert Q._image(opaque)[0, 0, 3] == 255
    rig = rest_rig(im)
    key_rig = Q.fit(opaque, key_color='#ff00ff')
    key_rig['clips'] = rig['clips']
    assert Q.build(opaque, key_rig) == Q.build(im, rig)


def test_manual_fixups_masks_joint_hierarchy_and_shared_anchors(tmp_path):
    im = humanoid()
    rig = rest_rig(im)
    # Force an anchor to a known pixel; its runtime location must use exactly
    # the same chained rigid transform as the source texture, not a heuristic.
    rig['seam_radius'] = 0
    rig['joints']['shoulder_f'] = [16, 8]
    rig['joints']['elbow_f'] = [16, 11]
    rig['joints']['wrist_f'] = [17, 14]
    rig['anchors']['grip'] = {'part': 'hand_f', 'point': [17, 15]}
    rig['anchors']['tip'] = {'part': 'hand_f', 'point': [17, 10]}
    rig['clips']['idle'][0]['angles'] = {'upper_arm_f': -90, 'lower_arm_f': 90}
    Q.save_rig(rig, tmp_path/'rig.json')
    rig = Q.load_rig(tmp_path/'rig.json')
    game = Q.build(im, rig)
    anchor = game['clips']['idle'][0]['anchors']['grip']
    # Upper arm rotates elbow [16,11] to [19,8], forearm's local +90
    # cancels parent -90, so [17,15] becomes [20,12].
    delta = np.array(rig['pivot'])-rig['origin']
    assert anchor == (np.array([20, 12])+delta).tolist()
    tip = game['clips']['idle'][0]['anchors']['tip']
    assert np.array_equal(np.array(tip)-anchor, [0, -5])
    # Paint a source head pixel into the front hand: source still identical at
    # rest, then this exact ink travels with the hand in the edited pose.
    changed = copy.deepcopy(rig)
    y, x = 4, 12
    for part in changed['parts'].values():
        line = list(part['mask'][y]); line[x] = '.'; part['mask'][y] = ''.join(line)
    line = list(changed['parts']['hand_f']['mask'][y]); line[x] = '#'
    changed['parts']['hand_f']['mask'][y] = ''.join(line)
    assert Q.build(im, changed)['clips']['idle'][0]['rows'] != game['clips']['idle'][0]['rows']


def test_inverse_sampling_solid_rotated_part_has_no_forward_scatter_holes():
    texture = np.full((15, 15), 'W', dtype='<U1')
    out = np.full((40, 40), '.', dtype='<U1')
    r = Q._rotation(31)
    t = np.array([20, 20])-r@np.array([7, 7])
    Q._stamp(out, texture, (r, t))
    # Every pixel with an inverse coordinate inside the source is occupied.
    Y, X = np.mgrid[:40, :40]
    source = np.floor((np.stack([X, Y], -1)-t)@r+.5).astype(int)
    expected = np.all((source >= 0) & (source < 15), axis=-1)
    np.testing.assert_array_equal(out != '.', expected)
    assert abs(np.count_nonzero(out != '.')-225) < 10


def test_smear_only_on_action_and_tip_trajectory():
    im = humanoid()
    rig = Q.fit(im)
    with_smear = Q.build(im, rig, clips='slash')['clips']['slash']
    plain = copy.deepcopy(rig)
    for pose in plain['clips']['slash']:
        pose['smear'] = False
    without = Q.build(im, plain, clips=['slash'])['clips']['slash']
    changed = [a['rows'] != b['rows'] for a, b in zip(with_smear, without)]
    assert changed[2] or changed[3]
    assert not any(changed[i] for i in (0, 1, 4, 5, 6))
    for a, b in zip(with_smear, without):
        assert a['anchors'] == b['anchors']
        assert np.count_nonzero(array(a) != '.') >= np.count_nonzero(array(b) != '.')


def test_export_png_sheet_json_gif_pixels_transparency_and_ticks(tmp_path):
    im = humanoid()
    game = Q.build(im, Q.fit(im), clips=['idle', 'slash', 'die'])
    manifest = Q.export(game, tmp_path, scale=2)
    assert json.loads((tmp_path/'game.json').read_text()) == game
    assert json.loads((tmp_path/'sheet.json').read_text()) == manifest
    sheet = Image.open(tmp_path/'sheet.png').convert('RGBA')
    w, h = game['canvas']
    assert sheet.size == (max(map(len, game['clips'].values()))*w, len(game['clips'])*h)
    for name, frames in game['clips'].items():
        for frame, rect in zip(frames, manifest['clips'][name]['rects']):
            x, y, fw, fh = rect
            np.testing.assert_array_equal(np.asarray(sheet.crop((x, y, x+fw, y+fh))),
                                          np.asarray(Q.frame_image(frame)))
        with Image.open(tmp_path/f'{name}.gif') as gif:
            assert gif.size == (w*2, h*2)
            assert (gif.info.get('loop') == 0) == (name == 'idle')
            duration = 0
            for frame in ImageSequence.Iterator(gif):
                # Pillow combines identical consecutive frames; match the
                # original pose at this cumulative 30Hz/centisecond boundary.
                elapsed, index = 0, 0
                while index+1 < len(frames) and round((elapsed+frames[index]['ticks'])*100/30)*10 <= duration:
                    elapsed += frames[index]['ticks']
                    index += 1
                expected = Q.frame_image(frames[index]).resize(gif.size, Image.Resampling.NEAREST)
                np.testing.assert_array_equal(np.asarray(frame.convert('RGBA')), np.asarray(expected))
                duration += frame.info['duration']
            assert abs(duration-sum(f['ticks'] for f in frames)*1000/30) <= 10


def test_cli_fit_build_sheet_and_dispatch_preserves_other_paths(tmp_path, monkeypatch):
    image = tmp_path/'source.png'
    humanoid().save(image)
    main(['quick', 'fit', str(image), '--out', str(tmp_path/'rig.json')])
    main(['quick', 'build', str(image), '--rig', str(tmp_path/'rig.json'),
          '--out', str(tmp_path/'out'), '--clips', 'idle,walk'])
    assert (tmp_path/'out/rig.json').is_file()
    assert set(json.loads((tmp_path/'out/game.json').read_text())['clips']) == {'idle', 'walk'}
    import spriterig.__main__ as cli
    import spriterig.native as native
    calls = []
    monkeypatch.setattr(cli, 'creative_main', lambda args: calls.append(('creative', args)))
    monkeypatch.setattr(native, 'main', lambda args: calls.append(('native', args)))
    cli.main(['creative', 'test.png'])
    cli.main(['test.png'])
    cli.main(['--frames', 'native.json'])
    assert calls == [('creative', ['test.png']), ('creative', ['test.png']),
                     ('native', ['--frames', 'native.json'])]


@pytest.mark.parametrize('kwargs,match', [
    ({'cell': 0}, 'together'),
    ({'cell_size': [24, 24]}, 'together'),
    ({'cell': -1, 'cell_size': [24, 24]}, 'cell'),
    ({'cell': 1, 'cell_size': [24, 24]}, 'range'),
    ({'cell': 0, 'cell_size': [7, 24]}, 'multiples'),
    ({'cell': 0, 'cell_size': [0, 24]}, 'cell_size'),
    ({'alpha_threshold': 0}, 'alpha_threshold'),
    ({'alpha_threshold': True}, 'alpha_threshold'),
    ({'facing': 'up'}, 'facing'),
    ({'key_color': 'xyz'}, 'key_color'),
    ({'canvas': [1, 1]}, 'pivot'),
])
def test_fit_rejects_malformed_options(kwargs, match):
    with pytest.raises(ValueError, match=match):
        Q.fit(humanoid(), **kwargs)


def test_empty_wrong_dtype_bad_source_and_missing_palette_fail(tmp_path):
    for source in (Image.new('RGBA', (24, 24)), np.zeros((24, 24, 4), np.uint8)):
        with pytest.raises(ValueError, match='no visible'):
            Q.fit(source)
    with pytest.raises(ValueError, match='uint8'):
        Q.fit(np.zeros((24, 24, 4), float))
    with pytest.raises(ValueError, match='too small'):
        Q.fit(Image.new('RGBA', (2, 2), 'white'))
    jpg = tmp_path/'source.jpg'; humanoid().convert('RGB').save(jpg)
    with pytest.raises(ValueError, match='PNG'):
        Q.fit(jpg)
    broken = tmp_path/'broken.png'; broken.write_text('not png')
    with pytest.raises(OSError):
        Q.fit(broken)
    with pytest.raises(OSError):
        Q.fit(humanoid(), root=tmp_path)


@pytest.mark.parametrize('mutate,match', [
    (lambda r: r.update(version=2), 'version'),
    (lambda r: r.update(pivot=[-1, 0]), 'pivot'),
    (lambda r: r.update(canvas=[24.0, 24]), 'integers'),
    (lambda r: r['joints'].update(neck=[float('nan'), 2]), 'finite'),
    (lambda r: r['parts']['head'].update(parent='head'), 'cycle'),
    (lambda r: r['parts']['head'].update(parent='missing'), 'parent'),
    (lambda r: r['parts']['head'].update(joint='missing'), 'joint'),
    (lambda r: r['parts']['head'].update(mask=['bad']), 'mask'),
    (lambda r: r['anchors'].pop('grip'), 'anchors'),
    (lambda r: r['anchors']['tip'].update(part='missing'), 'part'),
    (lambda r: r.update(clips={}), 'clips'),
    (lambda r: r['clips']['idle'][0].update(ticks=0), 'ticks'),
    (lambda r: r['clips']['idle'][0].update(angles={'missing': 30}), 'angle'),
    (lambda r: r['clips']['idle'][0].update(rotation=float('inf')), 'finite'),
    (lambda r: r['clips']['idle'][0].update(smear=1), 'boolean'),
])
def test_rig_fixup_validation(mutate, match):
    rig = rest_rig(humanoid())
    mutate(rig)
    with pytest.raises(ValueError, match=match):
        Q.validate_rig(rig)


def test_build_rejects_missing_pixels_wrong_image_and_clipped_canvas():
    im = humanoid()
    rig = rest_rig(im)
    rig['parts']['head']['mask'] = ['.'*24]*24
    with pytest.raises(ValueError, match='unassigned|owning part has no pixels'):
        Q.build(im, rig)
    rig = rest_rig(im)
    with pytest.raises(ValueError, match='dimensions'):
        Q.build(humanoid(16), rig)
    with pytest.raises(ValueError, match='clips'):
        Q.build(im, rig, clips=['missing'])
    rig['canvas'] = [24, 24]; rig['pivot'] = [12, 12]
    with pytest.raises(ValueError, match='anchor outside|canvas clips'):
        Q.build(im, rig)


@pytest.mark.parametrize('mutate,match', [
    (lambda g: g.update(version=False), 'version'),
    (lambda g: g['clips']['idle'][0].update(rows=['bad']), 'rows'),
    (lambda g: g['clips']['idle'][0]['rows'].__setitem__(0, '?'*g['canvas'][0]), 'inks'),
    (lambda g: g['clips']['idle'][0]['anchors'].update(tip=[-1, 0]), 'outside'),
    (lambda g: g['clips']['idle'][0].update(ticks=0), 'ticks'),
    (lambda g: g.update(clips={'../escape': g['clips']['idle']}), 'identifiers'),
])
def test_export_rejects_malformed_contract_before_writing(tmp_path, mutate, match):
    game = Q.build(humanoid(), rest_rig(humanoid()))
    mutate(game)
    with pytest.raises(ValueError, match=match):
        Q.export(game, tmp_path/'should-not-exist')
    assert not (tmp_path/'should-not-exist').exists()


def test_palette_root_is_explicit_and_environment_is_respected(tmp_path, monkeypatch):
    ink = tmp_path/'crates/delve/src/render/overworld/ink.rs'
    ink.parent.mkdir(parents=True)
    ink.write_text("pub fn ink(ch: char) { 'A' => 0x123456, 'B' => 0xabcdef, _ => return None }")
    im = humanoid()
    monkeypatch.setenv('ANGELX_ROOT', str(tmp_path))
    rig = Q.fit(im)
    game = Q.build(im, rig)
    assert set(''.join(game['clips']['idle'][0]['rows'])) <= {'.', 'A', 'B'}
    Q.export(game, tmp_path/'out', root=tmp_path)
    assert (tmp_path/'out/idle.gif').is_file()


def test_committed_quick_sample_and_sheet_run_all_clips():
    from pathlib import Path
    samples = Path(__file__).resolve().parents[1]/'samples/quick'
    im = Image.open(samples/'squire.png')
    rig = Q.fit(im)
    game = Q.build(im, rig)
    assert len(game['clips']) == 13
    sheet = samples/'squire_sheet.png'
    sheet_rig = Q.fit(sheet, cell=1, cell_size=[24, 24])
    assert Q.build(sheet, sheet_rig) == game
    left_rig = Q.fit(sheet, cell=0, cell_size=[24, 24], facing='left')
    assert Q.build(sheet, left_rig) == game


@pytest.mark.parametrize('location', ['joint', 'parent', 'anchor'])
def test_non_string_part_references_fail_cleanly(location):
    rig = rest_rig(humanoid())
    if location == 'anchor':
        rig['anchors']['grip']['part'] = []
    else:
        rig['parts']['head'][location] = []
    with pytest.raises(ValueError, match='part|joint|parent'):
        Q.validate_rig(rig)


def test_autofit_contacts_are_real_source_pixels(pal):
    # Remove the distal half of each painted hand: a proportional wrist lies
    # in air, but the visible lower-arm end must still own a real hand contact.
    raw = np.asarray(humanoid(pal=pal)).copy()
    raw[15:17, :8] = 0
    raw[15:17, 17:] = 0
    rig = Q.fit(raw)
    for name in Q.ANCHORS:
        if name == 'tip':
            continue
        anchor = rig['anchors'][name]
        x, y = anchor['point']
        assert type(x) is int and type(y) is int
        assert rig['parts'][anchor['part']]['mask'][y][x] == '#'
    for name in ('head', 'feet'):
        anchor = rig['anchors'][name]
        rows = rig['parts'][anchor['part']]['mask']
        assert anchor['point'][1] == max(y for y, row in enumerate(rows) if '#' in row)
    assert rig['anchors']['feet']['part'] in ('foot_f', 'foot_b')


@pytest.mark.parametrize('size', [16, 24, 32])
def test_every_contact_hits_exact_transformed_owner_not_seam_or_other_limb(size):
    image = humanoid(size)
    rig = Q.fit(image)
    before = copy.deepcopy(rig)
    game = Q.build(image, rig)
    assert rig == before  # mutable clips/anchor data are not used as work buffers
    ink = Q._inks(Q._image(image), Q._palette())
    textures = Q._textures(rig, ink)
    w, h = rig['canvas']
    for clip, frames in game['clips'].items():
        for i, frame in enumerate(frames):
            transforms, base = Q._transforms(rig, rig['clips'][clip][i])
            if clip in ('roll', 'die'):
                transforms, base = Q._ground(rig, transforms, base, textures)
            for name, point in frame['anchors'].items():
                if name == 'tip':
                    continue
                anchor = rig['anchors'][name]
                part = anchor['part']
                # Replay inverse-nearest sampling of ONLY this original mask,
                # not expanded seam texture, whole-body ink or nearby pixels.
                mask = np.array([list(row) for row in rig['parts'][part]['mask']]) == '#'
                layer = np.full((h, w), '.', dtype='<U1')
                Q._stamp(layer, np.where(mask, ink, '.'), transforms[part])
                x, y = point
                assert layer[y, x] != '.', (size, clip, i, name, point)
                assert frame['rows'][y][x] != '.', (size, clip, i, name, point)
                if name == 'feet':
                    assert y == np.nonzero(layer != '.')[0].max()
                else:
                    r, t = transforms[part]
                    target = r@anchor['point']+t
                    yy, xx = np.nonzero(layer != '.')
                    best = ((np.stack([xx, yy], -1)-target)**2).sum(-1).min()
                    assert np.sum((np.array(point)-target)**2) == pytest.approx(best)


def tiny_game():
    ink = next(iter(Q._palette()))
    frame = dict(ticks=1, rows=[ink], anchors={n: [0, 0] for n in Q.ANCHORS})
    return dict(version=1, canvas=[1, 1], pivot=[0, 0], clips={'idle': [frame]})


@pytest.mark.parametrize('case', ['missing_idle', 'unknown_clip', '129_frames', '1025_frames',
                                  '129_canvas', '3601_ticks', 'extra_top', 'extra_frame',
                                  'empty_idle', 'bool_ticks', 'bool_version', 'unicode_rows'])
def test_runtime_unloadable_games_rejected(case):
    game = tiny_game()
    frame = game['clips']['idle'][0]
    if case == 'missing_idle':
        game['clips'] = {'walk': [frame]}
    elif case == 'unknown_clip':
        game['clips']['dance'] = [frame]
    elif case == '129_frames':
        game['clips']['idle'] *= 129
    elif case == '1025_frames':
        game['clips'] = {name: [frame]*128 for name in Q.CLIPS[:8]}
        game['clips']['thrust'] = [frame]
    elif case == '129_canvas':
        game['canvas'] = [129, 1]
        frame['rows'] = [frame['rows'][0]*129]
    elif case == '3601_ticks':
        frame['ticks'] = 3601
    elif case == 'extra_top':
        game['extra'] = 0
    elif case == 'extra_frame':
        frame['extra'] = 0
    elif case == 'empty_idle':
        frame['rows'] = ['.']
    elif case == 'bool_ticks':
        frame['ticks'] = True
    elif case == 'bool_version':
        game['version'] = True
    elif case == 'unicode_rows':
        frame['rows'] = ['é']
    with pytest.raises(ValueError):
        Q.validate_game(game)


def test_runtime_limits_inclusive_and_no_invented_total_tick_cap():
    game = tiny_game()
    frame = game['clips']['idle'][0]
    frame['ticks'] = 3600
    frame['rows'] = [frame['rows'][0]*128]*128
    game['canvas'] = [128, 128]
    assert Q.validate_game(game) is game
    # Rig::parse sums u64 ticks, with no 216000-total-tick restriction.
    game = tiny_game()
    frame = game['clips']['idle'][0]
    frame['ticks'] = 3600
    game['clips'] = {name: [frame]*128 for name in Q.CLIPS[:8]}
    assert sum(len(fs) for fs in game['clips'].values()) == 1024
    assert Q.validate_game(game) is game


@pytest.mark.parametrize('case', ['missing_idle', 'unknown_clip', '129_frames', '1025_frames',
                                  '3601_ticks', 'far_joint', 'far_origin', 'far_offset',
                                  'no_foot_owner', 'wrong_hand_owner', 'empty_hand'])
def test_editable_rig_limits(case):
    rig = rest_rig(humanoid())
    pose = rig['clips']['idle'][0]
    if case == 'missing_idle':
        rig['clips'] = {'walk': [pose]}
    elif case == 'unknown_clip':
        rig['clips']['dance'] = [pose]
    elif case == '129_frames':
        rig['clips']['idle'] *= 129
    elif case == '1025_frames':
        rig['clips'] = {name: [pose]*128 for name in Q.CLIPS[:8]}
        rig['clips']['thrust'] = [pose]
    elif case == '3601_ticks':
        pose['ticks'] = 3601
    elif case == 'far_joint':
        rig['joints']['root'] = [24, 0]
    elif case == 'far_origin':
        rig['origin'] = [-1, 0]
    elif case == 'far_offset':
        pose['offset'] = [1e30, 0]
    elif case == 'no_foot_owner':
        rig['anchors']['feet']['part'] = None
    elif case == 'wrong_hand_owner':
        rig['anchors']['grip']['part'] = 'torso'
    elif case == 'empty_hand':
        rig['parts']['hand_f']['mask'] = ['.'*24]*24
    with pytest.raises(ValueError):
        Q.validate_rig(rig)


def test_clip_subset_includes_idle_without_mutating_request(tmp_path):
    image = humanoid()
    rig = Q.fit(image)
    names = ['walk', 'slash']
    assert list(Q.build(image, rig, clips=names)['clips']) == ['idle', 'walk', 'slash']
    assert names == ['walk', 'slash']
    path = tmp_path/'source.png'
    image.save(path)
    rig_path = tmp_path/'rig.json'
    Q.save_rig(rig, rig_path)
    Q.main(['build', str(path), '--rig', str(rig_path),
            '--out', str(tmp_path/'out'), '--clips', 'walk'])
    assert set(json.loads((tmp_path/'out/game.json').read_text())['clips']) == {'idle', 'walk'}


def test_default_pose_mutations_are_independent():
    from spriterig.quick_motion import defaults
    clips = defaults(20)
    before = copy.deepcopy(clips)
    clips['idle'][0]['angles']['head'] = 44
    clips['idle'][0]['offset'][0] = 19
    assert clips['idle'][1:] == before['idle'][1:]
    assert clips['cast'] == before['cast']
    assert defaults(20) == before


@pytest.mark.parametrize('kind', ['path', 'pil', 'array', 'sheet'])
def test_oversized_source_rejected_before_decode_convert_or_copy(kind, tmp_path, monkeypatch):
    image = Image.new('RGBA', (1025, 1))
    if kind in ('path', 'sheet'):
        source = tmp_path/'wide.png'
        image.save(source)
    elif kind == 'array':
        source = np.asarray(image)
    else:
        source = image
    def forbidden(*args, **kwargs):
        pytest.fail('oversized source was decoded/converted/copied before checking header/shape')
    monkeypatch.setattr(Image.Image, 'convert', forbidden)
    monkeypatch.setattr(Image.Image, 'crop', forbidden)
    monkeypatch.setattr(Image.Image, 'load', forbidden)
    monkeypatch.setattr(Image, 'fromarray', forbidden)
    args = dict(cell=0, cell_size=[1, 1]) if kind == 'sheet' else {}
    with pytest.raises(ValueError, match='source'):
        Q._image(source, **args)


def test_maximum_source_size_and_transparent_rgba_are_canonical(pal):
    image = np.zeros((1024, 1024, 4), np.uint8)
    image[0, :3] = [[1, 2, 3, 0], [4, 5, 6, 127], [7, 8, 9, 128]]
    rgba = Q._image(image)
    assert rgba[0, 0].tolist() == rgba[0, 1].tolist() == [0, 0, 0, 0]
    assert rgba[0, 2].tolist() == [7, 8, 9, 255]
    frame = tiny_game()['clips']['idle'][0]
    frame['rows'] = ['.'+next(iter(pal))]
    pixels = np.asarray(Q.frame_image(frame))
    assert pixels[0, 0].tolist() == [0, 0, 0, 0]
    assert pixels[0, 1].tolist() == [*pal[frame['rows'][0][1]], 255]


def test_bounded_file_inputs_and_duplicate_json_fields(tmp_path, monkeypatch):
    path = tmp_path/'huge.json'
    with path.open('wb') as stream:
        stream.truncate(Q._MAX_BYTES+1)
    with pytest.raises(ValueError, match='8 MiB'):
        Q.load_rig(path)
    with pytest.raises(ValueError, match='8 MiB'):
        Q._image(path)
    path.write_text('{"version":1,"version":1}')
    with pytest.raises(ValueError, match='duplicate'):
        Q.load_rig(path)
    # Oversized frame/clip counts are rejected before a raster or sheet allocation.
    game = tiny_game()
    game['clips']['idle'] *= 129
    def forbidden(*args, **kwargs):
        pytest.fail('invalid game allocated an image')
    monkeypatch.setattr(Image, 'new', forbidden)
    with pytest.raises(ValueError, match='frames'):
        Q.export(game, tmp_path/'out')


def test_joint_and_tip_coordinate_limits_handle_huge_integers():
    rig = rest_rig(humanoid())
    rig['joints'].update({f'unused{i}': [0, 0] for i in range(Q._MAX_JOINTS)})
    with pytest.raises(ValueError, match='joints'):
        Q.validate_rig(rig)
    for target in ('joint', 'tip', 'game'):
        rig = rest_rig(humanoid())
        if target == 'joint':
            rig['joints']['root'] = [10**1000, 0]
            validate = lambda: Q.validate_rig(rig)
        elif target == 'tip':
            rig['anchors']['tip']['point'] = [10**1000, 0]
            validate = lambda: Q.validate_rig(rig)
        else:
            game = tiny_game()
            game['pivot'] = [10**1000, 0]
            validate = lambda: Q.validate_game(game)
        with pytest.raises(ValueError):
            validate()


def test_json_size_bound_and_compact_fallback(monkeypatch):
    game = tiny_game()
    pretty = json.dumps(game, indent=2)+'\n'
    compact = json.dumps(game, separators=(',', ':'))+'\n'
    monkeypatch.setattr(Q, '_MAX_BYTES', len(compact))
    assert len(pretty) > Q._MAX_BYTES
    assert Q._json_text(game) == compact
    assert Q.validate_game(game) is game
    monkeypatch.setattr(Q, '_MAX_BYTES', len(compact)-1)
    with pytest.raises(ValueError, match='8 MiB'):
        Q.validate_game(game)


def test_output_pixel_budget_checked_before_allocation(tmp_path, monkeypatch):
    game = tiny_game()
    game['clips']['idle'] *= 2
    monkeypatch.setattr(Q, '_MAX_OUTPUT_PIXELS', 1)
    def forbidden(*args, **kwargs):
        pytest.fail('export exceeded pixel budget before image allocation')
    monkeypatch.setattr(Image, 'new', forbidden)
    with pytest.raises(ValueError, match='pixel budget'):
        Q.export(game, tmp_path/'out')
    game['clips']['idle'] = game['clips']['idle'][:1]
    with pytest.raises(ValueError, match='pixel budget'):
        Q.export(game, tmp_path/'out', scale=2)


def test_feet_follow_own_contact_chain_and_tip_is_not_snapped():
    image = humanoid()
    rig = rest_rig(image)
    rig['clips']['idle'].append(dict(ticks=6, angles={'lower_leg_f': -35}))
    game = Q.build(image, rig)
    a, b = game['clips']['idle']
    assert a['anchors']['feet'] != b['anchors']['feet']
    assert a['anchors']['grip'] == b['anchors']['grip']
    for pose, frame in zip(rig['clips']['idle'], game['clips']['idle']):
        ts, base = Q._transforms(rig, pose)
        tip = rig['anchors']['tip']
        r, t = ts[tip['part']]
        assert frame['anchors']['tip'] == np.floor(r@tip['point']+t+.5).astype(int).tolist()
    x, y = a['anchors']['tip']
    assert a['rows'][y][x] == '.'


@pytest.fixture(scope='module')
def native_knight():
    """The production 16px source, not a resized or synthetic humanoid."""
    from pathlib import Path
    sample = Path(Q.__file__).parents[1]/'samples/quick/knight'
    image = sample/'source.png'
    auto = Q.fit(image)
    manual = Q.load_rig(sample/'rig.json')
    return image, auto, manual


def _owned_pixels(rig, pose, ink, parts):
    """Render original owning masks only: no borrowed seams or other body ink."""
    transforms, _ = Q._transforms(rig, pose)
    layer = np.full(tuple(reversed(rig['canvas'])), '.', dtype='<U1')
    for name in parts:
        mask = np.array([list(row) for row in rig['parts'][name]['mask']]) == '#'
        Q._stamp(layer, np.where(mask, ink, '.'), transforms[name])
    return layer


@pytest.mark.parametrize('kind', ['auto', 'manual'])
def test_native_16px_idle_and_ranged_body_signal(native_knight, kind):
    image, auto, manual = native_knight
    rig = auto if kind == 'auto' else manual
    assert rig['source']['size'] == [16, 16]
    game = Q.build(image, rig)
    ink = Q._inks(Q._image(image), Q._palette())
    clips = game['clips']

    # Idle must move actual helmet pixels, not just a virtual tip or a cuff.
    rest = _owned_pixels(rig, rig['clips']['idle'][0], ink, ('head',))
    breath = _owned_pixels(rig, rig['clips']['idle'][2], ink, ('head',))
    assert np.count_nonzero(rest != breath) >= 8
    assert np.count_nonzero((rest != '.') != (breath != '.')) >= 4
    assert clips['idle'][0]['rows'] != clips['idle'][2]['rows']
    np.testing.assert_array_equal(array(clips['idle'][0])[27:], array(clips['idle'][2])[27:])

    assert {tuple(f['rows']) for f in clips['bow']} != {tuple(f['rows']) for f in clips['gun']}
    for name, minimum in (('bow', 30), ('gun', 60)):
        aim = _owned_pixels(rig, rig['clips'][name][1], ink, ('head', 'torso'))
        action = _owned_pixels(rig, rig['clips'][name][2], ink, ('head', 'torso'))
        assert np.count_nonzero(aim != action) >= minimum, name

    bow = clips['bow']
    def cheek_distance(frame):
        return np.linalg.norm(np.array(frame['anchors']['off_hand'])-frame['anchors']['head'])
    assert cheek_distance(bow[2]) <= cheek_distance(bow[1])-1
    assert bow[2]['anchors']['off_hand'][0] <= bow[1]['anchors']['off_hand'][0]-3
    assert bow[3]['anchors']['off_hand'][0] > bow[2]['anchors']['off_hand'][0]
    assert cheek_distance(bow[4]) > cheek_distance(bow[2])

    gun = clips['gun']
    # Whole-body kick includes the planted leg chain, not merely an arm twitch.
    for name, dx in (('feet', 2), ('back', 2), ('head', 2)):
        assert gun[2]['anchors'][name][0] <= gun[1]['anchors'][name][0]-dx
    assert rig['clips']['gun'][2]['offset'][0] == -2
    assert gun[2]['anchors']['grip'][1] < gun[1]['anchors']['grip'][1]
    muzzle = np.array(gun[2]['anchors']['tip'])-gun[2]['anchors']['grip']
    assert muzzle[0] > 0 and muzzle[1] < 0
    assert gun[1]['rows'] == gun[4]['rows']

    hurt = clips['hurt']
    assert hurt[0]['anchors']['head'][0] <= hurt[-1]['anchors']['head'][0]-3
    for frame in clips['cast'][1:5]:
        assert frame['anchors']['grip'][1] < frame['anchors']['head'][1]
        assert frame['anchors']['off_hand'][1] < frame['anchors']['head'][1]
    assert not any(p.get('smear') for name in ('idle', 'bow', 'gun', 'hurt', 'cast')
                   for p in rig['clips'][name])


def test_production_native_rebuild_floor_and_owning_contacts(native_knight):
    from pathlib import Path
    image, auto, rig = native_knight
    game = Q.build(image, rig)
    production = Path(Q.__file__).parents[3]/'crates/delve/assets/quick-rig/knight.json'
    assert json.loads(production.read_text()) == game
    assert game['canvas'] == auto['canvas'] == [46, 46]
    assert game['pivot'] == auto['pivot'] == [23, 30]
    ink = Q._inks(Q._image(image), Q._palette())
    for name in ('idle', 'walk', 'run'):
        for frame in game['clips'][name]:
            assert np.nonzero(array(frame) != '.')[0].max() == game['pivot'][1]
    for name, poses in rig['clips'].items():
        for pose, frame in zip(poses, game['clips'][name]):
            # roll/die additionally ground their transforms inside build().
            ts, base = Q._transforms(rig, pose)
            if name in ('roll', 'die'):
                ts, base = Q._ground(rig, ts, base, Q._textures(rig, ink))
            for contact, anchor in rig['anchors'].items():
                if contact == 'tip':
                    continue  # deliberately virtual weapon direction
                x, y = anchor['point']
                assert rig['parts'][anchor['part']]['mask'][y][x] == '#'
                layer = np.full(tuple(reversed(rig['canvas'])), '.', dtype='<U1')
                mask = np.array([list(row) for row in rig['parts'][anchor['part']]['mask']]) == '#'
                Q._stamp(layer, np.where(mask, ink, '.'), ts[anchor['part']])
                x, y = frame['anchors'][contact]
                assert layer[y, x] != '.', (name, contact)

    # Each improved frame remains one hard-pixel mass, without detached limbs.
    for name in ('idle', 'bow', 'gun', 'hurt', 'cast'):
        for frame in game['clips'][name]:
            pixels = set(zip(*np.nonzero(array(frame) != '.')))
            todo = [next(iter(pixels))]
            seen = set(todo)
            while todo:
                y, x = todo.pop()
                for dy in (-1, 0, 1):
                    for dx in (-1, 0, 1):
                        neighbor = (y+dy, x+dx)
                        if neighbor in pixels and neighbor not in seen:
                            seen.add(neighbor)
                            todo.append(neighbor)
            assert seen == pixels, name


@pytest.mark.parametrize('ticks,expected_ms', [([6, 6], [200, 200]),
                                              ([1]*6, [30, 40, 30, 30, 40, 30]),
                                              ([2, 3, 4], [70, 100, 130])])
def test_gif_game_clock_30_ticks_per_second(tmp_path, ticks, expected_ms):
    # Independent expected boundaries: 6 Delve ticks == 200ms, never 100ms.
    game = tiny_game()
    game['canvas'] = [2, 1]
    first = game['clips']['idle'][0]
    a = next(c for c in first['rows'][0] if c != '.')
    b = next(c for c in Q._palette() if c != a)
    frames = [dict(ticks=t, rows=[a+b if i % 2 else b+a],
                   anchors=copy.deepcopy(first['anchors'])) for i, t in enumerate(ticks)]
    game['clips'] = {'idle': frames}
    manifest = Q.export(game, tmp_path)
    with Image.open(tmp_path/'idle.gif') as gif:
        assert [f.info['duration'] for f in ImageSequence.Iterator(gif)] == expected_ms
    assert manifest['clips']['idle']['ticks'] == ticks  # JSON keeps game ticks
