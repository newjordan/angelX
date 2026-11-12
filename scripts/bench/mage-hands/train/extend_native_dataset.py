"""Append click-verified native grounding data without rewriting historical rows.

No inference, training or remote service. Native labels must bind to raw AT-SPI
before/after trees and a target-specific effect; source success flags alone do
not count. Publish a new destination only after the existing whole-data audit.
"""
import argparse
from collections import Counter, defaultdict
import hashlib
import importlib.util
import json
from pathlib import Path
import random
import shutil

HERE = Path(__file__).resolve().parent


def load(name):
    spec = importlib.util.spec_from_file_location(name, HERE / (name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


assembly = load('assemble_recovery')
auditor = load('audit_recovery')
regime = load('regime')
sha, stable, read_rows = assembly.sha, assembly.stable, assembly.read_rows


def verified_element(edge, before, after, before_nodes, after_nodes):
    matches = [(i, e) for i, e in enumerate(before['elements']) if e['target'] == edge['target']]
    if len(matches) != 1:
        raise ValueError('native target not unique in source frame')
    index, element = matches[0]
    box, point = element['box'], edge['coordinate']
    if (edge.get('verified') is not True or edge['box'] != box
            or not (box[0] <= point[0] < box[2] and box[1] <= point[1] < box[3])
            or before['sha256'] == after['sha256']):
        raise ValueError('invalid native click evidence')
    nodes = [n for n in before_nodes if n['path'] == element['node_path']]
    if (len(nodes) != 1 or nodes[0]['box'] != box or nodes[0]['name'] != element['name']
            or nodes[0]['role'] != element['role'] or nodes[0]['states'] != element['states']
            or element['label_method'] != 'native-atspi-window-extents+x11-anchor'):
        raise ValueError('native label does not match original accessibility node')
    oracle = edge['oracle']
    if oracle['kind'] == 'calculator-digit':
        texts = lambda ns: [n['text'] for n in ns if n['role'] in ('text', 'entry') and n['text']]
        a, b = texts(before_nodes), texts(after_nodes)
        if not (element['name'].isdigit() and a and b and b[0] == a[0] + element['name']
                and oracle['before'] == a and oracle['after'] == b and oracle['expected'] == b[0]):
            raise ValueError('calculator result oracle failed')
    elif oracle['kind'] == 'named-toggle-opens':
        matching = [n for n in after_nodes if n['path'] == element['node_path'] and n['name'] == element['name']]
        opened = bool(matching and any(s in matching[0]['states'] and s not in nodes[0]['states']
                                      for s in ('checked', 'expanded', 'pressed')))
        existing = {(n['role'], n['name']) for n in before_nodes if 'showing' in n['states']}
        added = [[n['role'], n['name']] for n in after_nodes if 'showing' in n['states']
                 and (n['role'], n['name']) not in existing]
        if not opened or not added or oracle['added_controls'] != added:
            raise ValueError('named native menu oracle failed')
    else:
        raise ValueError('unsupported native effect oracle')
    return index, element


def build(base, captures, output):
    from PIL import Image
    base, output = Path(base).resolve(), Path(output).resolve()
    staging = output.with_name(output.name + '.building')
    if output.exists() or staging.exists():
        raise ValueError('new destination and staging paths required')
    old_manifest = assembly.verify_base(base)
    shutil.copytree(base, staging, copy_function=shutil.copyfile)
    # copyfile does not copy the frozen source mode; originals are never edited.
    rows = {s: read_rows(base / (s + '.jsonl')) for s in regime.SPLITS}
    original_rows = {s: len(rs) for s, rs in rows.items()}
    labels = {r['id']: r for r in read_rows(base / 'labels.jsonl')}
    frames = json.loads((base / 'frames.json').read_text())
    transitions = read_rows(base / 'transitions.jsonl')
    exclusions = read_rows(base / 'exclusions.jsonl')
    provenance = json.loads((base / 'provenance.json').read_text())
    source_cache = {}

    def snapshot(path):
        path = Path(path).resolve()
        if path not in source_cache:
            digest = sha(path)
            name = 'sources/' + digest + '-' + path.name
            shutil.copyfile(path, staging / name)
            provenance[name] = {'original': str(path), 'sha256': digest}
            source_cache[path] = name
        return source_cache[path]

    snapshot(base / 'manifest.json')
    for filename in ('extend_native_dataset.py', 'capture_native_atspi.py', 'native_x11.py', 'audit_recovery.py'):
        snapshot(HERE / filename)
    new_frames, frame_by_key, sources, candidates, new_edges = [], {}, {}, [], []
    seen_groups = set()
    expected_groups = set()
    raw_count = 0
    for capture in captures:
        capture = Path(capture).resolve()
        receipt = json.loads((capture / 'receipt.json').read_text())
        if (receipt.get('complete') is not True or receipt.get('failures') != 0
                or receipt.get('apps_stopped') is not True or receipt.get('model_calls') is not False
                or json.loads((capture / 'failures.json').read_text())):
            raise ValueError('native capture incomplete or not stopped')
        pages = json.loads((capture / 'pages.json').read_text())
        edges = json.loads((capture / 'transitions.json').read_text())
        plan = json.loads((capture / 'plan.json').read_text())
        assignments = {s['group']: s for s in plan['scenarios']}
        if len(pages) != receipt['pages'] or len(edges) != receipt['transitions']:
            raise ValueError('native source count mismatch')
        groups = {p['group'] for p in pages}
        if groups & seen_groups or groups != set(assignments):
            raise ValueError('native scenario missing or repeated')
        if {e['group'] for e in edges} != groups:
            raise ValueError('native scenario has no verified clicks')
        seen_groups.update(groups)
        expected_groups.update(groups)
        for name in ('receipt.json', 'plan.json', 'failures.json', 'attempts.json'):
            snapshot(capture / name)
        page_source, edge_source = snapshot(capture / 'pages.json'), snapshot(capture / 'transitions.json')
        by_shot = {}
        node_cache = {}
        for i, page in enumerate(pages):
            spec = assignments[page['group']]
            if spec['split'] != page['split'] or spec['family'] != page['family']:
                raise ValueError('native split changed from predeclared plan')
            shot, tree = auditor.inside(capture, page['shot']), auditor.inside(capture, page['tree'])
            if sha(shot) != page['sha256'] or sha(tree) != page['tree_sha256']:
                raise ValueError('native screenshot/tree hash changed')
            snapshot(tree)
            node_cache[page['shot']] = json.loads(tree.read_text())
            with Image.open(shot) as im:
                im.load()
                if list(im.size) != [page['width'], page['height']] or im.format != 'PNG':
                    raise ValueError('native screenshot geometry changed')
                pixels = hashlib.sha256(f'{im.width}x{im.height}:RGB:'.encode() + im.convert('RGB').tobytes()).hexdigest()
            frame = {'id': stable([page_source, i]), 'image': page['sha256'], 'pixels': pixels,
                     'width': page['width'], 'height': page['height'], 'path': str(shot),
                     'group': page['group'], 'split': page['split'], 'family': page['family'],
                     'state': page['state'], 'url': page['url'], 'origin': {'file': page_source, 'index': i},
                     'entities': [], 'eligible': False, 'exclusion': None}
            new_frames.append(frame)
            frame_by_key[frame['id']] = frame
            by_shot[page['shot']] = (page, frame)
            sources[frame['image']] = shot
        for edge in edges:
            before, bf = by_shot[edge['before']]
            after, af = by_shot[edge['after']]
            if any(before[k] != after[k] for k in ('group', 'split', 'family')) or edge['group'] != before['group']:
                raise ValueError('native transition changes scenario')
            index, element = verified_element(edge, before, after, node_cache[edge['before']], node_cache[edge['after']])
            candidates.append((bf, index, element))
            new_edges.append({'group': bf['group'], 'split': bf['split'], 'family': bf['family'],
                              'before': bf['id'], 'after': af['id'], 'label': edge['label'],
                              'source': edge_source, 'scripted': True, 'eligible_pair': False,
                              'click_oracle_checked': True})
            raw_count += 1
    # Compare to all historical frames, including quarantined holdout frames.
    # Never alter those frames/rows: exclude only conflicting new material.
    kept, rejected = assembly.partition(frames + new_frames)
    keep_ids = {f['id'] for f in kept}
    for f in rejected:
        if f['id'] in frame_by_key:
            frame_by_key[f['id']]['exclusion'] = f['reason']
            exclusions.append({'kind': 'native-frame', 'frame_id': f['id'], 'group': f['group'],
                               'split': f['split'], 'reason': f['reason']})
    seen = {(r['screen'], r['target'], tuple(r['crop'] or [])) for rs in rows.values() for r in rs}
    new_labels = set()
    for frame, index, element in candidates:
        if frame['id'] not in keep_ids:
            continue
        screen = 'screens/' + frame['image'] + '.png'
        origin = {**frame['origin'], 'kind': 'capture', 'element_index': index, 'split': frame['split']}
        label = dict(screen=screen, target=element['target'], box=element['box'], family=frame['family'],
                     split=frame['split'], group=frame['group'], entities=[], origin=origin)
        label['id'] = stable(label)
        for zoom in ((1, 2, 3) if frame['split'] == 'train' else (1,)):
            rng = random.Random(int(stable([frame['id'], index, zoom])[:16], 16))
            crop = regime.crop_for(element['box'], frame['width'], frame['height'], zoom, rng)
            key = (screen, element['target'], tuple(crop or []))
            if key in seen:
                continue
            seen.add(key)
            point = regime.point_in_view(element['box'], crop, frame['width'], frame['height'])
            row = {k: v for k, v in label.items() if k not in ('id', 'origin')}
            row.update(width=frame['width'], height=frame['height'], crop=crop, zoom=zoom,
                       answer=regime.answer(element['target'], point), source=frame['url'], label_id=label['id'])
            auditor.check_geometry(row)
            rows[frame['split']].append(row)
            labels[label['id']] = label
            new_labels.add(label['id'])
        destination = staging / screen
        if not destination.exists():
            shutil.copyfile(sources[frame['image']], destination)
    represented = {labels[x]['group'] for x in new_labels}
    if represented != expected_groups:
        raise ValueError('native scenarios lost all verified labels: ' + str(expected_groups - represented))
    installed = {(r['screen'], r['split']) for rs in rows.values() for r in rs}
    for f in new_frames:
        f['eligible'] = f['id'] in keep_ids and ('screens/' + f['image'] + '.png', f['split']) in installed
    for edge in new_edges:
        edge['eligible_pair'] = all(frame_by_key[edge[k]]['eligible'] for k in ('before', 'after'))
    # Historical lines stay byte-for-byte as the prefix, including their ordering.
    for split, rs in rows.items():
        with (staging / (split + '.jsonl')).open('a') as stream:
            for row in rs[original_rows[split]:]:
                stream.write(json.dumps(row) + '\n')
        if (staging / (split + '.jsonl')).read_bytes()[:(base / (split + '.jsonl')).stat().st_size] != (base / (split + '.jsonl')).read_bytes():
            raise ValueError('historical split modified')
    regime.write_rows(staging / 'labels.jsonl', list(labels.values()))
    regime.write_rows(staging / 'transitions.jsonl', transitions + new_edges)
    regime.write_rows(staging / 'exclusions.jsonl', exclusions)
    (staging / 'frames.json').write_text(json.dumps(frames + new_frames, indent=2) + '\n')
    (staging / 'provenance.json').write_text(json.dumps(provenance, indent=2) + '\n')
    native = {'raw_verified_clicks': raw_count, 'installed_verified_labels': len(new_labels),
              'added_rows': {s: len(rows[s]) - original_rows[s] for s in regime.SPLITS},
              'groups': sorted(represented), 'old_split_prefixes_unchanged': True,
              'model_calls': False, 'training_started': False,
              'scope': 'native calculator, editor and files; original raw effects rechecked'}
    (staging / 'NATIVE.json').write_text(json.dumps(native, indent=2) + '\n')
    (staging / 'AUDIT.json').unlink()
    manifest = {**old_manifest, 'version': 'recovery-dataset-native-v1',
                'scope': 'offline-browser-store-fake-shop-and-native-general',
                'base_manifest_sha256': sha(base / 'manifest.json'),
                'counts': {s: dict(sorted(Counter(r['family'] for r in rows[s]).items())) for s in regime.SPLITS},
                'screens': {p.name: sha(p) for p in sorted((staging / 'screens').glob('*.png'))},
                'native_extension': native,
                'limitations': old_manifest['limitations'] + [
                    'Native additions are verified scripted point targets, not autonomous tasks or model results.',
                    'Native splits use scenarios within the same apps; this is not held-out application generalization.',
                    'Only clicked before-frame targets enter supervision; after frames are provenance unless separately labelled.']}
    manifest['files'] = {str(p.relative_to(staging)): sha(p) for p in sorted(staging.rglob('*'))
                         if p.is_file() and not p.is_relative_to(staging / 'screens')
                         and p.name not in ('manifest.json', 'system.txt', 'AUDIT.json')}
    (staging / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    audit = auditor.audit_dataset(staging)
    (staging / 'AUDIT.json').write_text(json.dumps(audit, indent=2) + '\n')
    staging.rename(output)
    for path in output.rglob('*'):
        if path.is_file():
            path.chmod(0o444)
    result = {'dataset': str(output), 'native': native, 'audit': audit}
    print(json.dumps(result, indent=2), flush=True)
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base', required=True)
    parser.add_argument('--capture', action='append', required=True)
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    build(args.base, args.capture, args.output)
