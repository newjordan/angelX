from pathlib import Path
import json
import sys
import tempfile
from unittest.mock import patch

import numpy as np
from PIL import Image
import pytest

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import factory as F
from backend import generate, workflow


def spec(kind='prop', **extra):
    return dict(id='test-asset',name='Test Asset',brief='Grey test prop',kind=kind,**extra)


def image(w=16,h=16):
    a=Image.new('RGBA',(w,h)); a.paste((*F.STYLE['G'],255),(3,3,w-3,h))
    return a


def test_key_is_explicit_and_preserves_grey_and_binary_edges(tmp_path):
    p=tmp_path/'source.png'; a=Image.new('RGBA',(32,32),(170,20,100,255))
    a.paste((100,100,100,255),(10,5,22,29)); a.save(p)
    with_key,_=F.clean(p,'prop','body','aa1464')
    no_key,_=F.clean(p,'prop','body')
    assert np.array(with_key)[:,:,3].sum()<np.array(no_key)[:,:,3].sum()
    assert np.isin(np.array(with_key)[:,:,3],[0,255]).all()
    assert all(c in F.STYLE or c=='.' for row in F.rows(with_key) for c in row)


def test_empty_source_is_prerequisite_failure_not_quality_score(tmp_path):
    p=tmp_path/'source.png';Image.new('RGBA',(16,16)).save(p)
    r=F.make(spec(source=str(p)),tmp_path/'out',tmp_path/'catalog.json')
    assert r['status']=='failed' and 'empty source' in r['error']
    assert 'checks' not in r and not (tmp_path/'catalog.json').exists()


def test_armour_rejects_tall_figure_without_squashing():
    a=Image.new('RGBA',(12,6)); a.paste((*F.STYLE['G'],255),(5,0,7,6))
    e,_=F.fit_entry(spec('worn'),a)
    c={c['check']:c for c in F.checks(e,a)}
    assert not c['armour-not-figure']['passed']
    assert c['armour-not-figure']['fix']=='needs-armour-source'


def test_all_shared_clips_rebuild_and_gait_has_no_floor_jitter():
    fixed=json.loads((F.ROOT/'tools/asset-factory/fixups/cinder-knave.json').read_text())
    source=F.rgba(fixed['rows']); e,rig=F.fit_entry(spec('body'),source)
    assert e['rig']==F.quick.build(source,rig,root=str(F.ROOT))
    assert set(e['rig']['clips'])==set(F.CLIPS)
    assert all(c['passed'] for c in F.checks(e,source))
    for name in ('idle','walk','run'):
        assert len({max(y for y,row in enumerate(f['rows']) if set(row)!={'.'}) for f in e['rig']['clips'][name]})==1


def test_weapon_contacts_are_painted_and_attack_is_not_repeated_static():
    a=F.rgba(json.loads((F.ROOT/'tools/asset-factory/fixups/chain-flail.json').read_text())['rows'])
    e,_=F.fit_entry(spec('weapon',clip='flail',sound='swing'),a);e=F.animation(e)
    for x,y in e['anchors'].values(): assert e['frames'][0][y][x]!='.'
    assert len({json.dumps(f) for f in e['attack']['vfx']})==4
    assert e['attack']['sound']=='swing'


def test_failure_does_not_publish_and_review_records_reason(tmp_path):
    p=tmp_path/'source.png';image(12,6).save(p)
    with patch.object(F,'checks',return_value=[dict(check='style',passed=False,reason='wrong silhouette',fix=None)]):
        r=F.make(spec('worn',source=str(p)),tmp_path/'out',tmp_path/'catalog.json')
    assert r['status']=='failed' and not (tmp_path/'catalog.json').exists()
    assert 'wrong silhouette' in (tmp_path/'out/review/index.html').read_text()


def test_targeted_fix_not_regeneration(tmp_path):
    p=tmp_path/'source.png';a=image();a.putpixel((0,0),(*F.STYLE['G'],255));a.save(p)
    r=F.make(spec(source=str(p)),tmp_path/'out',tmp_path/'catalog.json')
    assert r['status']=='passed'
    assert json.load(open(tmp_path/'catalog.json'))['assets'][0]['id']=='test-asset'
    F.make(spec(source=str(p)),tmp_path/'out',tmp_path/'catalog.json')
    assert list((tmp_path/'out/jobs/test-asset/history').glob('*.json'))


def test_call_budget_reserves_before_network_and_workflow_is_qwen(tmp_path):
    log=tmp_path/'calls.jsonl';log.write_text(''.join(json.dumps(dict(event='requested'))+'\n' for _ in range(30)))
    with patch('backend.request') as request:
        with pytest.raises(ValueError,match='budget exhausted'):
            generate('brief','body',tmp_path/'image.png',log)
        request.assert_not_called()
    graph=workflow('grey knight',39)
    assert 'qwen_image_2.1' in graph['1']['inputs']['unet_name']
    assert all('grok' not in json.dumps(n).lower() for n in graph.values())


@pytest.mark.parametrize('extra,reason', [
    ({'name':'bad\nname'}, 'single lines'),
    ({'brief':'x'*2049}, 'single lines'),
    ({'clip':'unknown'}, 'melee clip'),
    ({'sound':'not-a-bundled-sound'}, 'sound pick'),
])
def test_bad_batch_metadata_fails_before_generation(tmp_path, extra, reason):
    metadata=spec('weapon'); metadata.update(extra)
    with patch('factory.generate') as generate_image:
        r=F.make(metadata,tmp_path/'out',tmp_path/'catalog.json')
    generate_image.assert_not_called()
    assert r['status']=='failed' and reason in r['error']
    assert not (tmp_path/'catalog.json').exists()
    assert reason in (tmp_path/'out/review/index.html').read_text()
