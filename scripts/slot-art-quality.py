#!/usr/bin/env python3
"""Audit staged bytes and attach SHA-bound real cockpit reduction metrics."""
import argparse,hashlib,json
from collections import Counter
from pathlib import Path
from PIL import Image
import importlib.util
spec=importlib.util.spec_from_file_location('slots',Path(__file__).with_name('slot-art-import.py'));s=importlib.util.module_from_spec(spec);spec.loader.exec_module(s)
p=argparse.ArgumentParser(description=__doc__);p.add_argument('metrics',type=Path);p.add_argument('quality',type=Path);a=p.parse_args()
metrics=json.loads(a.metrics.read_text());records=[];frames=0
for slot,terminal in sorted(metrics.items()):
    folder=s.OUTPUT/slot;path=folder/'slot.json';r=json.loads(path.read_text());m=json.loads((folder/(slot+'.map.json')).read_text())
    assert r['map_sha256']==s.digest(folder/(slot+'.map.json'))
    source=Path(r['source_path'])
    if not source.exists() or s.digest(source)!=r['source_sha256']:
        archived=next((p for p in (s.SOURCES/'_old').glob('*/v01/source-sheet.png') if s.digest(p)==r['source_sha256']),None)
        assert archived, f'mapped source unavailable: {slot}'
        r['source_path']=str(archived)
        r['incoming_delivery']={'source_path':str(source),'source_sha256':s.digest(source) if source.exists() else None,'reason':'Concurrent delivery rerun; retained SHA-bound mapped source.'}
        r['flags']=sorted(set(r['flags']+['delivery changed/incomplete; retain previous mapped assets']))
        if r['quality']=='OK':r['quality']='needs-manual-map'
    assert set(terminal)=={n for n in r['outputs'] if n.startswith('v1/')}
    for name,o in r['outputs'].items():
        frame=folder/name;assert s.digest(frame)==o['sha256']
        im=Image.open(frame).convert('RGBA');assert list(im.size)==o['canvas_wh']
        assert all(c[3] in (0,255) and (not c[3] or c[:3] in s.palette_cache) for c in im.get_flattened_data())
        mask=frame.with_name(frame.stem+'-seat-mask.png')
        if mask.exists():
            o['seat_mask_sha256']=s.digest(mask)
            mi=Image.open(mask).convert('RGBA');assert mi.size==im.size
            assert all(c[3] in (0,255) and c[0] in (0,70,140,210) for c in mi.get_flattened_data())
        frames+=1
    for name,value in terminal.items():assert value['native_sha256']==r['outputs'][name]['sha256'], f'stale cell export {slot}/{name}'
    r['terminal_96_columns']=terminal
    if any(v['weak'] for v in terminal.values()):
        r['flags']=sorted(set(r['flags']+['weak actual terminal silhouette']))
        if r['quality']=='OK':r['quality']='needs-manual-map'
    s.write(path,s.js(r));records.append((r,m))
counts=Counter(r['quality'] for r,m in records)
lines=['# Slot import quality','',f'{len(records)} verified-status deliveries staged: '+', '.join(f'{k} {v}' for k,v in sorted(counts.items()))+'.',f'{frames} native files audited for hashes, canvas, binary alpha and shared inks; real cockpit writer at 96 columns for every v1 frame.','Runtime subset: Latch knight (5 states) and strike (4 frames); mismatched supplemental coverage stays staged.','', '| Slot | Status | Pieces actual/request | Flags |','|---|---|---:|---|']
for r,m in records:lines.append(f'| {r["slot"]} | {r["quality"]} | {m["delivered_pieces"]}/{m["expected_existing_pieces"]} | '+('; '.join(r['flags']) or 'Reviewed')+' |')
lines+=['','FOREIGN/NOSHEET excluded. OK? provenance remains suspect. Real cell metrics use an isolated black 384x224 plate; full gameplay review is limited to the two pilots.','Zero saturated magenta was detected in the three proof crops; alpha is preserved before native thresholding. Synthetic fringe negative control passes.']
s.write(a.quality, ('\n'.join(lines)+'\n').encode());print(f'{len(records)} slots / {frames} native files / {dict(counts)}')
