"""Conservative quality gates: synthetic unit vectors, no fabricated native oracle."""
import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys

import numpy as np
from PIL import Image, ImageSequence
import pytest

from spriterig import quick as Q
from spriterig import native as N

SPEC = importlib.util.spec_from_file_location(
    'quick_quality', Path(__file__).parents[1] / 'tools/quick_quality.py')
K = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(K)
SAMPLE = Path(__file__).parents[1] / 'samples/quick/squire.png'


@pytest.fixture(scope='module')
def example():
    """An editable planted-foot animation, not a golden render of the generator.

    Default walk/run have moving painted bottoms; a quality gate must expose
    that, not quietly move frames to hide it. This deliberate fixup keeps shoes
    planted and animates arms, still exercising every authored clip and smear.
    """
    rig = Q.fit(SAMPLE)
    for name in K.LOOPS:
        for pose in rig['clips'][name]:
            pose['offset'] = [0, 0]
            for part in list(pose['angles']):
                if any(word in part for word in ('leg', 'foot')):
                    del pose['angles'][part]
    return rig, Q.build(SAMPLE, rig)


@pytest.fixture
def native(tmp_path):
    # Intentionally much larger padding than the sprite. Palette RGB must come
    # from Delve, but these tiny rows are synthetic Rust-manifest unit vectors.
    ink = next(iter(Q._palette()))
    w, h = 151, 113
    rows = ['.'*w for _ in range(h)]
    for y in range(72, 79):
        rows[y] = '.'*73+ink*5+'.'*(w-78)
    fit = dict(bounds=[73, 72, 5, 7])
    fit.update({p: dict(anchor=[75, 76], size=[5, 7], bottom=False, under=False) for p in N.PARTS})
    frames = [dict(kit=kit, view='right', pose='stand', tick=0, w=w, h=h,
                   rows=rows, fit=fit, mirror=False) for kit in ('percival', 'realm')]
    path = tmp_path/'native.json'
    path.write_text(json.dumps(dict(frames=frames)))
    return path


@pytest.fixture
def fast(monkeypatch):
    # Pixel-measurement tests don't repeatedly encode the exact same artifacts.
    monkeypatch.setattr(K, '_contacts', lambda *a, **kw: None)
    monkeypatch.setattr(K.Q, 'export', lambda *a, **kw: None)


def check(game, native, tmp_path, **kwargs):
    return K.review(game, native, tmp_path/'quality', **kwargs)


def failed(report, key):
    assert not report['ok'], report
    assert report['checks'][key]['status'] == 'fail', report['checks']


def test_planted_fixup_passes_all_checks_with_exact_provenance(example, native, tmp_path, fast):
    rig, game = example
    before = copy.deepcopy((rig, game))
    report = check(game, native, tmp_path, rig=rig, image=SAMPLE)
    assert report['ok'], {n: v for n, v in report['checks'].items() if v['status'] == 'fail'}
    assert all(c['status'] == 'pass' for c in report['checks'].values())
    assert report['checks']['native_baseline']['kit'] == 'realm'
    assert 'not human proof' in report['note']
    assert report['checks']['silhouette_1x']['heuristic']
    assert not report['checks']['source_rebuild']['heuristic']
    assert (rig, game) == before
    assert json.loads((tmp_path/'quality/checks.json').read_text()) == report


@pytest.mark.parametrize('damage', ['missing', 'json', 'empty', 'no_realm', 'bad_ink', 'bad_shape', 'blank'])
def test_required_native_never_silently_fabricated(example, native, tmp_path, fast, damage):
    if damage == 'missing':
        native.unlink()
    elif damage == 'json':
        native.write_text('{bad')
    else:
        data = json.loads(native.read_text())
        if damage == 'empty':
            data['frames'] = []
        elif damage == 'no_realm':
            data['frames'] = data['frames'][:1]
        else:
            f = data['frames'][1]
            if damage == 'bad_ink':
                f['rows'][0] = '?'*f['w']
            elif damage == 'bad_shape':
                f['rows'].pop()
            else:
                f['rows'] = ['.'*f['w'] for _ in range(f['h'])]
        native.write_text(json.dumps(data))
    report = check(example[1], native, tmp_path)
    failed(report, 'native_baseline')
    assert report['artifacts'] == []


def test_legacy_native_allowed_without_canonical_fit_parity(example, native, tmp_path, fast):
    data = json.loads(native.read_text())
    data['frames'] = data['frames'][1:]
    data['frames'][0]['kit'] = 'knight'
    data['frames'][0]['view'] = 'side'
    data['frames'][0]['pose'] = 'idle'
    native.write_text(json.dumps(data))
    report = check(example[1], native, tmp_path)
    assert report['ok']
    assert report['checks']['native_baseline']['kit'] == 'knight'
    assert report['checks']['source_rebuild']['status'] == 'skipped'
    assert report['checks']['smear_added_ink']['status'] == 'skipped'


@pytest.mark.parametrize('damage', ['ink', 'ticks', 'pivot', 'frame_pivot', 'dimensions', 'anchor_type'])
def test_runtime_schema_and_exact_ink_contract(example, native, tmp_path, fast, damage):
    game = copy.deepcopy(example[1])
    f = game['clips']['idle'][0]
    if damage == 'ink':
        f['rows'][0] = '?'+f['rows'][0][1:]
    elif damage == 'ticks':
        f['ticks'] = 0
    elif damage == 'pivot':
        game['pivot'] = [True, 43]
    elif damage == 'frame_pivot':
        f['pivot'] = game['pivot'][:]
    elif damage == 'dimensions':
        f['rows'][0] += '.'
    else:
        f['anchors']['head'][0] = 20.5
    report = check(game, native, tmp_path)
    failed(report, 'input')
    assert report['artifacts'] == []


@pytest.mark.parametrize('anchor', ['head', 'back', 'feet', 'grip', 'off_hand'])
def test_anchor_must_match_painted_region_not_just_exist(example, native, tmp_path, fast, anchor):
    game = copy.deepcopy(example[1])
    f = game['clips']['idle'][0]
    if anchor == 'head':
        target = f['anchors']['feet']
    elif anchor == 'feet':
        target = f['anchors']['head']
    elif anchor == 'back':
        target = f['anchors']['feet']
    else:
        target = f['anchors']['head']
    # Wrong but IN BOUNDS and on painted ink, not just an obvious empty corner.
    f['anchors'][anchor] = target[:]
    report = check(game, native, tmp_path)
    failed(report, 'painted_anchor_proximity')


def test_tip_is_virtual_not_required_to_touch_paint(example, native, tmp_path, fast):
    game = copy.deepcopy(example[1])
    assert K._distance(game['clips']['idle'][0]['anchors']['tip'], K._mask(game['clips']['idle'][0])) > 1
    assert check(game, native, tmp_path)['ok']


def test_one_pixel_floor_jitter_cannot_hide_behind_fixed_json_pivot(example, native, tmp_path, fast):
    game = copy.deepcopy(example[1])
    frame = game['clips']['walk'][2]
    frame['rows'] = ['.'*game['canvas'][0]] + frame['rows'][:-1]
    for point in frame['anchors'].values():
        point[1] += 1
    Q.validate_game(game)  # Schema alone cannot see the jitter.
    failed(check(game, native, tmp_path), 'fixed_floor')


def test_default_source_floor_jitter_is_reported_not_repaired(native, tmp_path, fast):
    rig = Q.fit(SAMPLE)
    game = Q.build(SAMPLE, rig)
    report = check(game, native, tmp_path, rig=rig, image=SAMPLE)
    failed(report, 'fixed_floor')
    assert report['checks']['source_rebuild']['status'] == 'pass'


@pytest.mark.parametrize('damage', ['empty', 'dots', 'line', 'detached'])
def test_nonempty_connectivity_and_body_mass_are_real_checks(example, native, tmp_path, fast, damage):
    game = copy.deepcopy(example[1])
    w, h = game['canvas']
    pixels = np.full((h, w), '.', dtype='<U1')
    ink = next(iter(Q._palette()))
    if damage == 'dots':
        pixels[10:50:3, 10:50:3] = ink
    elif damage == 'line':
        pixels[15:43, 30] = ink
    elif damage == 'detached':
        for y in range(5, 55, 10):
            for x in range(5, 55, 10):
                pixels[y:y+3, x:x+3] = ink
    game['clips']['idle'][0]['rows'] = [''.join(row) for row in pixels]
    report = check(game, native, tmp_path)
    failed(report, 'silhouette_1x')
    metrics = report['checks']['silhouette_1x']['metrics']['idle'][0]
    if damage == 'line':
        assert metrics['largest_fraction'] == 1 and metrics['body_mass'] == 0
    elif damage == 'detached':
        assert metrics['body_mass'] > .9 and metrics['largest_fraction'] < .1


@pytest.mark.parametrize('damage', ['cloned', 'frozen', 'different_ticks_only'])
def test_distinct_clips_compare_pixels_not_metadata(example, native, tmp_path, fast, damage):
    game = copy.deepcopy(example[1])
    if damage == 'frozen':
        game['clips']['gun'] = [copy.deepcopy(game['clips']['gun'][0]) for _ in range(7)]
    else:
        game['clips']['gun'] = copy.deepcopy(game['clips']['bow'])
        if damage == 'different_ticks_only':
            for f in game['clips']['gun']:
                f['ticks'] += 1
    failed(check(game, native, tmp_path), 'distinct_clips')


@pytest.mark.parametrize('field', ['anchors', 'rows', 'ticks', 'frame_count', 'pivot'])
def test_rebuild_detects_small_drift_even_when_heuristics_allow_it(example, native, tmp_path, fast, field):
    rig, original = example
    game = copy.deepcopy(original)
    f = game['clips']['slash'][2]
    if field == 'anchors':
        f['anchors']['grip'][0] += 1
    elif field == 'rows':
        ink = next(iter(Q._palette()))
        y, x = np.argwhere(K._mask(f))[0]
        f['rows'][y] = f['rows'][y][:x]+ink+f['rows'][y][x+1:]
    elif field == 'ticks':
        f['ticks'] += 1
    elif field == 'frame_count':
        game['clips']['slash'].pop()
    else:
        game['pivot'][0] += 1
    report = check(game, native, tmp_path, rig=rig, image=SAMPLE)
    failed(report, 'source_rebuild')
    assert report['checks']['source_rebuild']['differences']


def test_self_consistently_wrong_source_anchor_does_not_pass_rebuild(example, native, tmp_path, fast):
    rig = copy.deepcopy(example[0])
    rig['anchors']['grip']['point'] = rig['anchors']['head']['point'][:]
    game = Q.build(SAMPLE, rig)
    report = check(game, native, tmp_path, rig=rig, image=SAMPLE)
    assert report['checks']['source_rebuild']['status'] == 'pass'
    failed(report, 'source_anchor_regions')
    assert 'grip' in report['checks']['source_anchor_regions']['failures']


def test_missing_smear_is_detected_by_actual_added_pixels(example, native, tmp_path, fast):
    rig, game = example
    broken = copy.deepcopy(rig)
    for pose in broken['clips']['slash']:
        pose['smear'] = False
    no_smear = Q.build(SAMPLE, broken)
    report = check(no_smear, native, tmp_path, rig=rig, image=SAMPLE)
    failed(report, 'smear_added_ink')
    assert all(f['pixels'] == 0 for f in report['checks']['smear_added_ink']['clips']['slash'])
    failed(report, 'source_rebuild')
    # Removing both metadata and pixels still does not make a swing smear exist.
    report = check(no_smear, native, tmp_path, rig=broken, image=SAMPLE)
    assert report['checks']['source_rebuild']['status'] == 'pass'
    failed(report, 'smear_added_ink')


@pytest.mark.parametrize('source_only', [True, False])
def test_source_pair_required(example, native, tmp_path, fast, source_only):
    kwargs = dict(image=SAMPLE) if source_only else dict(rig=example[0])
    report = check(example[1], native, tmp_path, **kwargs)
    failed(report, 'source_rebuild')
    assert 'together' in report['checks']['source_rebuild']['error']


def test_all_frames_native_scale_common_floor_overlays_and_nearest_x6(example, native, tmp_path):
    # Full artifact verifier once; all clips and GIF timings remain native-size.
    rig, game = example
    report = check(game, native, tmp_path, rig=rig, image=SAMPLE)
    assert report['ok']
    out = tmp_path/'quality'
    with Image.open(out/'contacts_1x.png') as sheet, Image.open(out/'contacts_6x.png') as large:
        expected = sheet.resize((sheet.width*6, sheet.height*6), Image.Resampling.NEAREST)
        np.testing.assert_array_equal(large, expected)
        assert sheet.width < 2*151*max(map(len, game['clips'].values()))
        # The native reference block is exactly 5x7 at 1x, NOT scaled to rig size.
        ink_rgb = Q._palette()[next(iter(Q._palette()))]
        a = np.array(sheet)
        color = (a == ink_rgb).all(-1)
        assert color[20:80, :92].sum() == 35
        ys, xs = np.nonzero(color[20:80, :92])
        assert (xs.max()-xs.min()+1, ys.max()-ys.min()+1) == (5, 7)
        # Both painted subjects terminate on the same y, no padding-based shrink.
        first = game['clips']['idle'][0]
        floor = K._bounds(K._mask(first))[3]-1
        native_bottom = int(ys.max()+20)
        rig_pixels = a[native_bottom, 92:184]
        assert any(tuple(rgb) in set(Q._palette().values()) for rgb in rig_pixels)
        assert floor == game['pivot'][1]
    with Image.open(out/'contacts_overlay_1x.png') as overlay, Image.open(out/'contacts_1x.png') as clean:
        assert overlay.size != clean.size or np.any(np.array(overlay) != np.array(clean))
        pixels = np.array(overlay)
        assert (pixels == (50, 235, 255)).all(-1).any()
        assert (pixels == (255, 80, 210)).all(-1).any()
    manifest = json.loads((out/'clips/sheet.json').read_text())
    assert set(manifest['clips']) == set(game['clips'])
    for name, frames in game['clips'].items():
        with Image.open(out/f'clips/{name}.gif') as gif:
            assert gif.size == tuple(game['canvas'])
            assert sum(f.info['duration'] for f in ImageSequence.Iterator(gif)) == pytest.approx(
                round(sum(f['ticks'] for f in frames)*100/30)*10, abs=10)
        assert manifest['clips'][name]['ticks'] == [f['ticks'] for f in frames]
    assert len(report['artifacts']) == len(game['clips'])+4


def test_native_frame_selection_prefers_realm_and_matching_pose(native):
    data = json.loads(native.read_text())
    f = copy.deepcopy(data['frames'][1])
    f.update(pose='walk0', tick=3)
    data['frames'].append(f)
    native.write_text(json.dumps(data))
    refs = K._native(native, Q._palette())
    assert all(f['kit'] == 'realm' for f in refs)
    assert K._reference(refs, 'walk', 0)['pose'] == 'walk0'
    assert K._reference(refs, 'idle', 0)['pose'] == 'stand'


def test_cli_from_other_cwd_paths_exit_failure_and_machine_report(example, native, tmp_path):
    game_path = tmp_path/'game.json'
    game_path.write_text(json.dumps(example[1]))
    script = Path(K.__file__).resolve()
    # Missing baseline must be a visible nonzero exit with a report, not a fake.
    proc = subprocess.run([sys.executable, str(script), '--game', str(game_path),
                           '--native', str(tmp_path/'missing.json'), '--out', str(tmp_path/'cli')],
                          cwd=tmp_path, text=True, capture_output=True)
    assert proc.returncode == 1
    assert 'FAIL' in proc.stdout and 'native_baseline' in proc.stdout
    assert 'missing.json' in proc.stderr
    assert not json.loads((tmp_path/'cli/checks.json').read_text())['ok']


def test_incomplete_game_does_not_claim_all_clips_reviewed(example, native, tmp_path, fast):
    game = copy.deepcopy(example[1])
    del game['clips']['cast']
    report = check(game, native, tmp_path)
    failed(report, 'clip_coverage')
    assert report['checks']['clip_coverage']['missing'] == ['cast']


def test_palette_missing_is_visible_input_failure(example, native, tmp_path, fast):
    report = check(example[1], native, tmp_path, root=tmp_path/'missing-root')
    failed(report, 'input')
    assert 'ink.rs' in report['checks']['input']['error']


def test_source_mask_membership_must_be_painted_not_transparent(example, native, tmp_path, fast):
    rig = copy.deepcopy(example[0])
    # An author can paint a mask over transparent source pixels. That is NOT a
    # painted hand, even when the anchor is inside this counterfeit mask.
    rig['anchors']['grip']['point'] = [0, 0]
    rows = rig['parts']['hand_f']['mask']
    rows[0] = '#'+rows[0][1:]
    game = Q.build(SAMPLE, rig)
    report = check(game, native, tmp_path, rig=rig, image=SAMPLE)
    assert report['checks']['source_rebuild']['status'] == 'pass'
    failed(report, 'source_anchor_regions')


def test_contact_layout_does_not_hide_floor_jitter(example, native, tmp_path):
    game = copy.deepcopy(example[1])
    first = game['clips']['idle'][0]
    shifted = copy.deepcopy(first)
    shifted['rows'] = ['.'*game['canvas'][0]] + shifted['rows'][:-1]
    game['clips'] = {'idle': [first, shifted]}
    refs = K._native(native, Q._palette())
    K._contacts(game, refs, tmp_path, Q._palette())
    with Image.open(tmp_path/'contacts_1x.png') as im:
        pixels = np.array(im)
        is_ink = np.zeros(pixels.shape[:2], bool)
        for rgb in set(Q._palette().values()):
            is_ink |= (pixels == rgb).all(-1)
        # Native/rig pairs use the same pivot origin, NOT re-grounded bounds.
        bottom0 = np.nonzero(is_ink[20:, 92:184])[0].max()
        bottom1 = np.nonzero(is_ink[20:, 276:368])[0].max()
        assert bottom1 == bottom0+1


def test_cli_success_all_flags_from_other_cwd(example, native, tmp_path):
    game_path, rig_path = tmp_path/'game.json', tmp_path/'fixup.json'
    game_path.write_text(json.dumps(example[1]))
    rig_path.write_text(json.dumps(example[0]))
    out = tmp_path/'cli-success'
    proc = subprocess.run([sys.executable, str(Path(K.__file__).resolve()),
                           '--game', str(game_path), '--rig', str(rig_path),
                           '--input', str(SAMPLE.resolve()), '--native', str(native), '--out', str(out)],
                          cwd=tmp_path, text=True, capture_output=True)
    assert proc.returncode == 0, proc.stdout+proc.stderr
    assert 'PASS' in proc.stdout and 'heuristics' in proc.stdout
    report = json.loads((out/'checks.json').read_text())
    assert report['ok']
    assert report['checks']['source_rebuild']['status'] == 'pass'
    assert all((out/name).is_file() for name in report['artifacts'])


@pytest.mark.parametrize('size', [16, 32])
def test_conservative_metrics_accept_other_native_sprite_sizes(example, native, tmp_path, fast, size):
    with Image.open(SAMPLE) as source:
        image = source.resize((size, size), Image.Resampling.NEAREST)
    rig = Q.fit(image)
    # Same explicit planted-foot fixup; no quality-side normalization.
    for name in K.LOOPS:
        for pose in rig['clips'][name]:
            pose['offset'] = [0, 0]
            pose['angles'] = {p: a for p, a in pose['angles'].items()
                              if not any(word in p for word in ('leg', 'foot'))}
    report = check(Q.build(image, rig), native, tmp_path, rig=rig, image=image)
    assert report['ok'], {n: c for n, c in report['checks'].items() if c['status'] == 'fail'}
