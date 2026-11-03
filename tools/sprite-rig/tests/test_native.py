"""Unit vectors are deliberately synthetic; integration consumes Rust's export."""
import copy
import importlib.util
import os
from pathlib import Path
import subprocess
import sys

import numpy as np
from PIL import Image
import pytest

from spriterig import native as N
from spriterig.__main__ import main as default_main
from spriterig.delve import source

_spec = importlib.util.spec_from_file_location('compare_game', Path(__file__).parents[1] / 'tools/compare_game.py')
compare_game = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(compare_game)
PAL = {'h': (100, 110, 120), 'W': (240, 245, 250), 'k': (10, 10, 10)}


def synthetic_fit():
    # Not a replacement table: tiny geometry makes integer placement visible.
    fit = {'bounds': [0, 0, 5, 5]}
    fit.update({p: dict(anchor=[2, 2], size=[3, 3], bottom=False, under=False) for p in N.PARTS})
    fit['head'].update(anchor=[2, 1], bottom=True)
    fit['back']['under'] = True
    fit['feet'].update(anchor=[2, 4], bottom=True)
    return fit


def native_frame(mirror=False, tick=0):
    rows = ['.....', '.h...', '.hhh.', '..h..', '.h.h.']
    return dict(kit='knight', view='side_left' if mirror else 'side', pose='idle', tick=tick,
                w=5, h=5, rows=[r[::-1] for r in rows] if mirror else rows,
                fit=synthetic_fit(), mirror=mirror)


@pytest.fixture
def root(tmp_path):
    root = tmp_path / 'angelx'
    (root / N.FIT_PATH).parent.mkdir(parents=True)
    N.write_json(root / N.FIT_PATH, {'version': 1, 'profiles': {'knight': synthetic_fit()}})
    ink = root / source.OVERWORLD / 'ink.rs'
    ink.parent.mkdir(parents=True)
    ink.write_text("pub fn ink(ch: char) { match ch { 'h' => 0x646e78, 'W' => 0xf0f5fa, 'k' => 0x0a0a0a, _ => return None }}")
    return root


def test_canonical_table_validation_and_missing(root):
    assert N.load_fits(root)['knight']['head']['bottom']
    with pytest.raises(FileNotFoundError):
        N.load_fits(root / 'missing')
    path = root / N.FIT_PATH
    data = N.read_json(path)
    data['version'] = 2
    N.write_json(path, data)
    with pytest.raises(ValueError, match='version'):
        N.load_fits(root)
    data['version'] = 1
    del data['profiles']['knight']['feet']
    N.write_json(path, data)
    with pytest.raises(ValueError, match='feet'):
        N.load_fits(root)


def test_table_not_nested_parts(root):
    N.write_json(root / N.FIT_PATH, {'version': 1, 'profiles': {'knight': {'parts': synthetic_fit()}}})
    with pytest.raises(ValueError, match='bounds'):
        N.load_fits(root)


def test_root_resolution(root, monkeypatch):
    monkeypatch.setenv('ANGELX_ROOT', str(root))
    assert N.root_path() == root
    assert N.root_path(root / 'explicit') == root / 'explicit'


@pytest.mark.parametrize('alias,slot', list(N.ALIASES.items()) + [('whole', 'whole')])
def test_slot_aliases(alias, slot):
    assert N.slot_name(alias) == slot


def test_unknown_slot():
    with pytest.raises(ValueError, match='unknown worn slot'):
        N.slot_name('screen-left')


@pytest.mark.parametrize('size,maximum,expected', [
    ((3, 2), (10, 8), (9, 6)), ((7, 3), (4, 3), (4, 2)),
    ((4, 2), (3, 3), (3, 2)),
    ((6, 5), (3, 3), (3, 3)),  # Rust .round: 2.5 -> 3, not ties-even
    ((2, 4), (3, 3), (2, 3)), ((1, 99), (2, 3), (1, 3)),
    ((10, 10), (10, 10), (10, 10)),
])
def test_fit_size_rust_rounding(size, maximum, expected):
    assert N.fit_size(size, maximum) == expected


def test_union_crop_ragged_animation_and_edge_sampling():
    frames = [['..W.', '.h', ''], ['....', '...k']]
    assert N.ink_bounds(frames) == (1, 0, 3, 2)
    part = dict(size=[3, 2])
    assert N.fitted_frames(frames, part) == [['.W.', 'h..'], ['...', '..k']]
    # 3 -> 2 chooses source indices 0,1, not Pillow's 0,2.
    assert N.sample_frames([['Whk']], (0, 0, 3, 1), (2, 1)) == [['Wh']]


def test_author_repair_does_not_enlarge():
    frames = [['.Wh.']]
    part = dict(size=[10, 8])
    assert N.fitted_frames(frames, part, repair=True) == [['Wh']]
    assert N.fitted_frames(frames, part) == [['WWWWWhhhhh' for _ in range(5)]]


def test_origin_odd_bottom_and_whole():
    fit = synthetic_fit()
    assert N.part_origin(fit['head'], (3, 3)) == (1, -1)
    assert N.part_origin(fit['body'], (3, 3)) == (1, 1)
    whole = N.body_part(fit, 'whole')
    assert whole == dict(anchor=[2, 2], size=[5, 5], bottom=False, under=False)


def test_native_body_preserved_exactly_and_no_mutation():
    frame = native_frame()
    frame['exporter_metadata'] = {'stand': [2, 4], 'padding': [0, 0]}
    original = copy.deepcopy(frame)
    assert N.dress(frame, []) == frame
    N.dress(frame, [dict(at='head', frames=[['W']])])
    assert frame == original
    assert N.rows_rgba(frame['rows'], PAL)[1, 1].tolist() == [100, 110, 120, 255]


def test_under_over_order_and_entire_composite_mirror():
    frame = native_frame()
    frame['fit']['head']['size'] = [1, 1]
    frame['fit']['body']['size'] = [1, 1]
    frame['fit']['back']['size'] = [1, 1]
    under = dict(at='back', frames=[['W']])
    over = dict(at='body', frames=[['k']])
    assert N.dress(frame, [under])['rows'][2][2] == 'h'
    assert N.dress(frame, [over, dict(at='body', frames=[['W']])])['rows'][2][2] == 'W'
    right = N.dress(frame, [over])
    left = copy.deepcopy(frame)
    left['mirror'] = True
    left['rows'] = [r[::-1] for r in left['rows']]
    assert N.dress(left, [over])['rows'] == [r[::-1] for r in right['rows']]


def test_symmetric_side_padding_top_only_floor_and_fit_shift():
    frame = native_frame()
    frame['fit']['right'].update(anchor=[6, 0], size=[3, 3])
    sprite = dict(at='weapon', frames=[['WWW', 'W.W', 'WWW']])
    dressed = N.dress(frame, [sprite])
    assert dressed['worn_padding'] == [3, 1]
    assert (dressed['w'], dressed['h']) == (11, 6)
    assert dressed['fit']['bounds'] == [3, 1, 5, 5]
    assert dressed['fit']['right']['anchor'] == [9, 1]
    frame['fit']['right']['anchor'] = [2, 8]
    dressed = N.dress(frame, [sprite])
    assert dressed['h'] == frame['h']
    assert dressed['rows'][-1][1:4] == 'WWW'


def test_tick_six_animation_and_stable_empty_frame():
    frame = native_frame(tick=6)
    frame['fit']['head']['size'] = [1, 1]
    item = dict(at='helm', frames=[['W'], ['k']])
    assert N.dress(frame, [item])['rows'][1][2] == 'k'
    item['frames'] = [['W'], ['.']]
    assert N.dress(frame, [item])['rows'] == frame['rows']


def test_unsupported_motion_fails_visibly():
    sprite = dict(frames=[['W']], at='head', moves=['bob'])
    with pytest.raises(ValueError, match='move='):
        N.validate_sprite(sprite, PAL)
    with pytest.raises(ValueError, match='move='):
        N.dress(native_frame(), [sprite])


def test_author_png_alpha_palette_union_and_roundtrip(tmp_path):
    # Two equal cells with different margins retain one union coordinate box.
    rgba = np.zeros((4, 8, 4), np.uint8)
    rgba[1, 1] = (239, 245, 250, 255)
    rgba[2, 6] = (100, 111, 120, 255)
    rgba[0, 0] = (255, 255, 255, 127)
    png = tmp_path / 'item.png'
    Image.fromarray(rgba).save(png)
    item = N.author_png(png, PAL, dict(size=[3, 3]), 'helm', 2)
    assert item == {'frames': [['W.', '..'], ['..', '.h']], 'at': 'head', 'pixel': True}
    assert N.validate_sprite(item, PAL) == item
    frame = native_frame(tick=6)
    N.export_frames([N.dress(frame, [item])], PAL, tmp_path / 'export', zoom=3)
    assert N.load_native(tmp_path / 'export/frames.json', PAL)[0]['rows']
    rgba[:] = 0
    Image.fromarray(rgba).save(png)
    with pytest.raises(ValueError, match='no ink'):
        N.author_png(png, PAL, dict(size=[3, 3]), 'head', 2)


def test_author_rejects_frame_lost_by_repair(tmp_path):
    rgba = np.zeros((1, 6, 4), np.uint8)
    rgba[0, 0] = (*PAL['W'], 255)
    rgba[0, 5] = (*PAL['k'], 255)
    png = tmp_path / 'too_sparse.png'
    Image.fromarray(rgba).save(png)
    with pytest.raises(ValueError, match='loses a frame silhouette'):
        N.author_png(png, PAL, dict(size=[1, 1]), 'head', count=2)


@pytest.mark.parametrize('bad', [None, [], 'invalid'])
def test_non_object_json_inputs(root, tmp_path, bad):
    N.write_json(root / N.FIT_PATH, bad)
    with pytest.raises(ValueError, match='object'):
        N.load_fits(root)
    path = tmp_path / 'invalid.json'
    N.write_json(path, bad)
    with pytest.raises(ValueError, match='object'):
        N.load_native(path, PAL)
    with pytest.raises(ValueError, match='object'):
        N.validate_sprite(bad, PAL)


def test_native_validation(tmp_path):
    path = tmp_path / 'native.json'
    N.write_json(path, dict(frames=[native_frame()]))
    assert len(N.load_native(path, PAL)) == 1
    frame = native_frame()
    for key, bad, message in [('w', 6, 'canvas'), ('mirror', None, 'mirror'),
                              ('rows', ['?'] * 5, 'unknown inks'), ('fit', {}, 'bounds')]:
        invalid = dict(frame, **{key: bad})
        N.write_json(path, dict(frames=[invalid]))
        with pytest.raises(ValueError, match=message):
            N.load_native(path, PAL)
    N.write_json(path, dict(frames=[frame, frame]))
    with pytest.raises(ValueError, match='duplicate'):
        N.load_native(path, PAL)


def test_default_cli_canonical_anchor_changes_preview(root, tmp_path):
    native = tmp_path / 'native.json'
    N.write_json(native, {'frames': [native_frame()]})
    png = tmp_path / 'hat.png'
    Image.fromarray(np.full((1, 1, 4), [240, 245, 250, 255], dtype=np.uint8)).save(png)
    out = tmp_path / 'out'
    args = ['--angelx', str(root), '--native', str(native), '--worn', str(png), '--out', str(out)]
    default_main(args)
    assert (out / 'worn.json').exists() and (out / 'preview.png').exists()
    assert not N.read_json(out / 'worn.json').get('paint')
    before = N.read_json(out / 'frames.json')['frames'][0]['rows']
    # Native per-frame fit drives placement (not the authoring reference).
    frame = native_frame()
    frame['fit']['head']['anchor'][1] += 2
    N.write_json(native, {'frames': [frame]})
    default_main(args)
    assert N.read_json(out / 'frames.json')['frames'][0]['rows'] != before
    # The canonical table drives authoring repair. No parallel tool table.
    table = N.read_json(root / N.FIT_PATH)
    table['profiles']['knight']['head']['size'] = [1, 1]
    N.write_json(root / N.FIT_PATH, table)
    Image.fromarray(np.full((3, 3, 4), [240, 245, 250, 255], dtype=np.uint8)).save(png)
    default_main(args)
    assert N.read_json(out / 'worn.json')['frames'] == [['W']]
    with pytest.raises(SystemExit):
        default_main(args + ['--profile', 'not_a_profile'])


def test_compare_exact_pixels_and_missing_inputs(root, tmp_path):
    native = tmp_path / 'native.json'
    frames = [native_frame(), native_frame(True), native_frame(tick=6)]
    N.write_json(native, {'frames': frames})
    out = tmp_path / 'out'
    N.export_frames(frames, PAL, out)
    report = compare_game.compare(native, out, root, tmp_path / 'compare.png', zoom=3)
    assert report['parity'] and report['compared'] == 3 and report['zoom'] == 3
    extra = dict(native_frame(), tick=999)
    N.write_json(out / 'frames.json', {'frames': N.read_json(out / 'frames.json')['frames'] + [extra]})
    with pytest.raises(ValueError, match='unexpected actual native frames'):
        compare_game.compare(native, out, root, tmp_path / 'extra.png')
    N.export_frames(frames, PAL, out)
    with Image.open(out / '0000.png') as image:
        pixels = np.array(image)
    pixels[1, 1] = (0, 0, 0, 255)
    Image.fromarray(pixels).save(out / '0000.png')
    with pytest.raises(ValueError, match='frames differ'):
        compare_game.compare(native, out, root, tmp_path / 'different.png')
    (out / '0000.png').unlink()
    with pytest.raises(FileNotFoundError):
        compare_game.compare(native, out, root, tmp_path / 'missing.png')
    with pytest.raises(FileNotFoundError):
        compare_game.compare(tmp_path / 'absent.json', out, root, tmp_path / 'missing.png')
    with pytest.raises(ValueError, match='no native frames match'):
        compare_game.compare(native, out, root, tmp_path / 'wrong-kit.png', kit='arcade')
    N.write_json(out / 'frames.json', {'frames': [frames[1]]})
    with pytest.raises(ValueError, match='missing actual native frame'):
        compare_game.compare(native, out, root, tmp_path / 'omitted.png')


def test_compare_cli_requires_both_inputs():
    result = subprocess.run([sys.executable, str(Path(compare_game.__file__))], capture_output=True, text=True)
    assert result.returncode != 0 and '--native' in result.stderr and '--actual' in result.stderr


def test_authoritative_export_integration(tmp_path):
    """Opt-in integration gate; no hardcoded shared-workspace path in the tool."""
    native = os.environ.get('SPRITERIG_NATIVE')
    if not native:
        pytest.skip('set SPRITERIG_NATIVE to Rust native.json for cross-language parity')
    root = N.root_path()
    fits = N.load_fits(root)
    pal = source.palette(root)
    frames = N.load_native(native, pal)
    assert len(frames) > 10
    N.export_frames(frames, pal, tmp_path / 'all', zoom=2)
    report = compare_game.compare(native, tmp_path / 'all', root, tmp_path / 'parity.png', zoom=4)
    assert report['parity']
    # Every native pose's resolved anchors, not nominal procedural sockets.
    for frame in frames:
        assert frame['fit']['head']['size'][0] > 0
        N.validate_fit(frame['fit'])
        for slot in (*N.PARTS, 'whole'):
            item = dict(at=slot, frames=[['Wk', '.h'], ['.W', 'kh']])
            dressed = N.dress(frame, [item])
            assert dressed['h'] >= frame['h']
            assert dressed['w'] >= frame['w']
    assert {'knight', 'realm', 'skeleton', 'golem', 'mage_front', 'mage_side'} <= fits.keys()


def test_removed_mesh_entrypoints_are_not_advertised():
    from spriterig.skel import __main__ as skel_main, dances
    for module in (skel_main, dances):
        result = subprocess.run([sys.executable, '-m', module.__name__, '--help'],
                                capture_output=True, text=True)
        assert result.returncode == 0
        assert '--mesh' not in result.stdout and '--experimental-3d' not in result.stdout


def test_clothing_fills_slot_but_weapon_keeps_aspect():
    frames = [['.W.', '.W.', '.W.', '.W.']]
    assert N.fitted_frames(frames, dict(size=[6, 3], stretch=True)) == [['W' * 6] * 3]
    assert N.fitted_frames(frames, dict(size=[6, 3], stretch=False)) == [['W'] * 3]


def test_real_saved_worn_export_parity():
    """Opt-in Rust-vs-Python composition, not just a native PNG roundtrip."""
    manifest = os.environ.get('SPRITERIG_NATIVE')
    items_path = os.environ.get('SPRITERIG_ITEMS')
    if not (manifest and items_path):
        pytest.skip('set SPRITERIG_NATIVE and SPRITERIG_ITEMS for real worn composition parity')
    pal = source.palette(N.root_path())
    refs = N.load_native(manifest, pal)
    by_key = {(f['kit'], f['view'], f['pose']): f for f in refs}
    items = {i['name']: i['sprite'] for i in N.read_json(items_path)['items']}
    dressed = N.read_json(Path(manifest).with_name('dressed.json'))
    assert len(dressed) >= len(refs)
    for expected in dressed:
        frame = by_key[expected['kit'], expected['view'], expected['pose']]
        got = N.dress(frame, [items[expected['item']]])
        assert (got['w'], got['h'], got['rows']) == (expected['w'], expected['h'], expected['rows'])


def test_native_animation_strip_has_fixed_floor_and_fit_metadata(tmp_path):
    frames = [native_frame(), native_frame(tick=6)]
    frames[1]['rows'] = ['.....'] + frames[1]['rows']
    frames[1]['h'] += 1
    index = N.export_animation(frames, PAL, tmp_path, fps=5)
    entry = index[0]
    assert entry['size'] == [5, 6]
    assert entry['frames'][0]['offset'] == [0, 1]
    assert entry['frames'][1]['offset'] == [0, 0]
    assert entry['frames'][0]['fit'] == frames[0]['fit']
    assert Image.open(tmp_path / entry['sheet']).size == (10, 6)
    assert (tmp_path / entry['preview']).is_file()
    assert N.card_text(dict(at='head', frames=[['W.', '.h'], ['.W', 'h.']])) == 'draw worn at=head pixel\nW.|.W\n.h|h.\n'
    with pytest.raises(ValueError, match='fps'):
        N.export_animation(frames, PAL, tmp_path, fps=0)
