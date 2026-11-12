"""Read-only audit of the frozen hands-only dataset. No model or service imports.

This recomputes file/pixel hashes, crop geometry and split intersections from the
artifact, rather than trusting the assembler's counters. It is a project audit,
not independent model-performance evaluation.
"""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
import math
from pathlib import Path
import re
from urllib.parse import urlparse

SPLITS = ('train', 'validation', 'test')


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def inside(root, name):
    path = (root / name).resolve()
    if not path.is_relative_to(root.resolve()) or not path.is_file():
        raise ValueError(f'unsafe/missing artifact path: {name}')
    return path


def check_geometry(row):
    w, h = row['width'], row['height']
    if type(w) is not int or type(h) is not int or min(w, h) < 2:
        raise ValueError('invalid image dimensions')
    crop = row.get('crop')
    zoom = row.get('zoom', 1)
    if zoom not in (1, 2, 3):
        raise ValueError('invalid crop zoom')
    if crop is not None:
        if (not isinstance(crop, list) or len(crop) != 4
                or any(type(x) is not int for x in crop)):
            raise ValueError('invalid crop values')
        x, y, cw, ch = crop
        if (zoom == 1 or x < 0 or y < 0 or cw < 1 or ch < 1 or x+cw > w or y+ch > h
                or cw != w//zoom or ch != h//zoom):
            raise ValueError('invalid crop bounds or scale')
    elif zoom != 1:
        raise ValueError('crop missing at zoom > 1')
    box = row['box']
    if (len(box) != 4 or any(type(x) not in (float, int) or not math.isfinite(x) for x in box)
            or not (0 <= box[0] < box[2] <= w and 0 <= box[1] < box[3] <= h)):
        raise ValueError('invalid target box')
    if not isinstance(row['target'], str) or not row['target'].strip():
        raise ValueError('empty target')
    reply = row['answer']
    matches = re.findall(r'<tool_call>\s*(.*?)\s*</tool_call>', reply, re.S)
    if len(matches) != 1 or reply.count('<tool_call>') != 1:
        raise ValueError('answer must have one complete click call')
    call = json.loads(matches[0])
    args = call.get('arguments', {})
    if call.get('name') != 'computer_use' or args.get('action') != 'click' or args.get('button') != 'left':
        raise ValueError('answer is not a single left click')
    point = args.get('coordinate')
    if not isinstance(point, list) or len(point) != 2 or any(type(x) is not int or not 0 <= x <= 999 for x in point):
        raise ValueError('invalid answer coordinate')
    px, py = point[0]/999*(w-1), point[1]/999*(h-1)
    if crop is not None:
        px, py = x + px/w*cw, y + py/h*ch
    if not (box[0] <= px <= box[2] and box[1] <= py <= box[3]):
        raise ValueError('answer outside annotated target box')
    return [px, py]


def audit_dataset(directory):
    from PIL import Image
    root = Path(directory).resolve()
    manifest = json.loads((root/'manifest.json').read_text())
    for name, digest in manifest['files'].items():
        if sha(inside(root, name)) != digest:
            raise ValueError(f'file hash mismatch: {name}')
    if sha(root/'system.txt') != manifest['system_sha256']:
        raise ValueError('system hash mismatch')
    provenance = json.loads((root/'provenance.json').read_text())
    sources = {}
    for name, source in provenance.items():
        if sha(inside(root, name)) != source['sha256']:
            raise ValueError('original source receipt hash mismatch')
        if name.endswith('.json'):
            sources[name] = json.loads((root/name).read_text())
    frames = json.loads((root/'frames.json').read_text())
    frame_by_id = {f['id']: f for f in frames}
    if len(frame_by_id) != len(frames):
        raise ValueError('duplicate provenance frame id')
    frames_by_origin = defaultdict(list)
    group_entities = defaultdict(set)
    for frame in frames:
        origin = frame['origin']
        original = sources[origin['file']][origin['index']]
        if frame['url'] != original.get('url', ''):
            raise ValueError('frame URL disagrees with source annotation')
        if frame['family'] in ('steam-store', 'steam-navigation'):
            match = re.search(r'/app/(\d+)', original['url'])
            if match:
                group_entities[frame['group']].add('steam-app:'+match[1])
        if frame['family'] in ('replay-web', 'legacy-test-web'):
            host = urlparse(original['url']).hostname or ''
            group_entities[frame['group']].add('legacy-web-domain:'+'.'.join(host.split('.')[-2:]))
        frames_by_origin[(origin['file'], origin['index'])].append(frame)
    for frame in frames:
        if frame['entities'] != sorted(group_entities[frame['group']]):
            raise ValueError('declared frame entities disagree with source URLs')
    labels = [json.loads(x) for x in (root/'labels.jsonl').read_text().splitlines()]
    by_label = {x['id']: x for x in labels}
    if len(by_label) != len(labels):
        raise ValueError('duplicate label ids')
    for label in labels:
        origin = label['origin']
        original = sources[origin['file']][origin['index']]
        matching_frames = [f for f in frames_by_origin[(origin['file'], origin['index'])]
                           if f['group'] == label['group'] and f['split'] == label['split']
                           and 'screens/'+f['image']+'.png' == label['screen']]
        if not matching_frames:
            raise ValueError('label lacks matching original frame')
        if label['entities'] != sorted(group_entities[label['group']]):
            raise ValueError('label entities disagree with original frame URLs')
        if origin['kind'] == 'capture':
            element = original['elements'][origin['element_index']]
            if (element['target'] != label['target'] or original['split'] != label['split']
                    or original['group'] != label['group'] or original['family'] != label['family']):
                raise ValueError('capture label/split/group changed from source')
        elif origin['kind'] == 'replay':
            element = original['elements'][origin['element_index']]
            if label['split'] != 'train' or not label['family'].startswith('replay-'):
                raise ValueError('replay belongs only in training')
            name = (element.get('text') or element.get('label') or '').strip()
            # Check the exact source name, never a substring (Cart != Cartoon).
            kinds = ('button', 'link', 'dropdown', 'tab', 'menu', 'menu item', 'checkbox',
                     'option', 'text field', 'number field', 'column header', 'item')
            forms = {f'"{name}"', f'the {name} icon'}
            for kind in kinds:
                forms.update((f'the "{name}" {kind}', f'the {name} {kind}', f'the {name} {kind} in the toolbar'))
            if not name or label['target'] not in forms:
                raise ValueError('replay target phrase changed from original annotation')
            source_path = Path(provenance[origin['file']]['original'])
            sid = hashlib.sha1(f"{source_path.parent}/{original['shot']}".encode()).hexdigest()[:12]
            if label['group'] != 'legacy-dev:'+sid:
                raise ValueError('replay scenario changed')
            if label['family'] == 'replay-desk':
                if any(app in original['name'] for app in ('disks', 'printers', 'pinta', 'kdenlive', 'zenity', 'omacalc')):
                    raise ValueError('historical heldout desktop included in replay')
            else:
                host = urlparse(original['url']).hostname or ''
                domain = '.'.join(host.split('.')[-2:])
                if int(hashlib.sha1(domain.encode()).hexdigest(), 16) % 5 < 2:
                    raise ValueError('historical heldout web domain included in replay')
        elif origin['kind'] == 'legacy-eval':
            element = original
            target = original['instruction'].strip().removeprefix('Click ').rstrip('.')
            if (label['split'] != 'test' or original['split'] != 'test'
                    or label['target'] != target or label['group'] != 'legacy-test:'+original['page']):
                raise ValueError('historical test label changed')
        else:
            raise ValueError('unknown label origin')
        if element['box'] != label['box']:
            raise ValueError('box changed from original annotation')
    image_data = {}
    for name, digest in manifest['screens'].items():
        path = inside(root/'screens', name)
        if sha(path) != digest or name != digest+'.png':
            raise ValueError(f'image hash mismatch: {name}')
        with Image.open(path) as im:
            if im.format != 'PNG':
                raise ValueError('non-PNG screenshot')
            im.load()
            rgb = im.convert('RGB')
            pixel_hash = hashlib.sha256(f'{im.width}x{im.height}:RGB:'.encode()+rgb.tobytes()).hexdigest()
            # Reject empty/solid-color captures; not a substitute for visual review.
            if all(lo == hi for lo, hi in rgb.getextrema()):
                raise ValueError('solid-color screenshot')
            image_data['screens/'+name] = {'sha256': digest, 'pixels': pixel_hash, 'size': list(im.size)}
    all_rows = []
    rows_by_split = {}
    counts = {}
    intersections = {kind: defaultdict(set) for kind in ('image', 'pixels', 'group', 'entity')}
    used_images, used_labels = set(), set()
    unique_rows = set()
    for split in SPLITS:
        rows = [json.loads(x) for x in (root/(split+'.jsonl')).read_text().splitlines()]
        if not rows:
            raise ValueError('empty split: '+split)
        rows_by_split[split] = rows
        counts[split] = dict(sorted(Counter(r['family'] for r in rows).items()))
        if counts[split] != manifest['counts'][split]:
            raise ValueError('split counts disagree')
        for i, row in enumerate(rows):
            if row['split'] != split:
                raise ValueError('split field disagrees')
            try:
                check_geometry(row)
            except (KeyError, ValueError, TypeError) as exc:
                raise ValueError(f'{split}:{i}: {exc}') from exc
            image = image_data[row['screen']]
            if image['size'] != [row['width'], row['height']]:
                raise ValueError('image dimensions disagree with labels')
            label = by_label[row['label_id']]
            for k in ('screen', 'target', 'box', 'split', 'group', 'family', 'entities'):
                if label[k] != row[k]:
                    raise ValueError('row/annotation disagreement: '+k)
            if label['origin']['split'] != row['split']:
                raise ValueError('original holdout reassigned')
            key = (row['screen'], row['target'], tuple(row['crop'] or []))
            if key in unique_rows:
                raise ValueError('duplicate image/target/view training exposure')
            unique_rows.add(key)
            used_labels.add(row['label_id']); used_images.add(row['screen'])
            for kind, value in (('image', image['sha256']), ('pixels', image['pixels']), ('group', row['group'])):
                intersections[kind][value].add(split)
            for entity in row['entities']:
                intersections['entity'][entity].add(split)
        all_rows.extend(rows)
    for kind, values in intersections.items():
        bad = {key: sorted(splits) for key, splits in values.items() if len(splits) > 1}
        if bad:
            raise ValueError(f'cross-split {kind} leakage: {list(bad.items())[:4]}')
    if used_labels != set(by_label) or used_images != set(image_data):
        raise ValueError('unreferenced installed label or screenshot')
    required = ('shopping', 'steam-store', 'steam-synthetic', 'shopping-transition', 'steam-navigation')
    for split in SPLITS:
        for family in required:
            if counts[split].get(family, 0) < 1:
                raise ValueError(f'missing curriculum coverage: {family}/{split}')
    for family in ('replay-desk', 'replay-web'):
        if counts['train'].get(family, 0) < 1:
            raise ValueError('missing replay family '+family)
    for family in ('legacy-test-desk', 'legacy-test-web'):
        if counts['test'].get(family, 0) < 1:
            raise ValueError('missing legacy holdout '+family)
    frame_ids = {f['id'] for f in frames}
    transitions = [json.loads(x) for x in (root/'transitions.jsonl').read_text().splitlines()]
    eligible_edges = Counter()
    for t in transitions:
        if t['before'] not in frame_ids or t['after'] not in frame_ids:
            raise ValueError('transition references unknown frame')
        before, after = frame_by_id[t['before']], frame_by_id[t['after']]
        if any(f['group'] != t['group'] or f['split'] != t['split'] or f['family'] != t['family'] for f in (before, after)):
            raise ValueError('transition changes scenario or split')
        eligible = all(f['eligible'] and 'screens/'+f['image']+'.png' in used_images for f in (before, after))
        if t['eligible_pair'] != eligible:
            raise ValueError('transition eligibility disagrees with installed frames')
        original_before = sources[before['origin']['file']][before['origin']['index']]['shot']
        original_after = sources[after['origin']['file']][after['origin']['index']]['shot']
        if not any(e['group'] == t['group'] and e['label'] == t['label']
                   and e['before'] == original_before and e['after'] == original_after for e in sources[t['source']]):
            raise ValueError('transition absent from original capture')
        if t['eligible_pair']:
            eligible_edges[t['family']+'/'+t['split']] += 1
    for split in SPLITS:
        for family in ('steam-navigation', 'shopping-transition'):
            if eligible_edges[family+'/'+split] < 1:
                raise ValueError(f'no eligible actual transitions: {family}/{split}')
    exclusions = [json.loads(x) for x in (root/'exclusions.jsonl').read_text().splitlines()]
    return {'verdict': 'PASS', 'manifest_sha256': sha(root/'manifest.json'),
            'scope': 'offline hands-only grounding dataset; not autonomous/live-task success',
            'rows': {s: len(rows_by_split[s]) for s in SPLITS}, 'counts': counts,
            'installed_images': len(image_data), 'decoded_images': len(image_data),
            'unique_targets': len(labels), 'geometry_checked': len(all_rows),
            'original_annotation_labels_checked': len(labels),
            'boxed_replay_rows': sum(r['family'].startswith('replay-') for r in all_rows),
            'cross_split_intersections': {k: 0 for k in intersections},
            'groups': {s: len({r['group'] for r in rows_by_split[s]}) for s in SPLITS},
            'eligible_transition_pairs': dict(sorted(eligible_edges.items())),
            'exclusion_reasons': dict(sorted(Counter(x['reason'] for x in exclusions).items())),
            'native_steam_client': False, 'training_started': False, 'holdout_reassignment': False,
            'limitations': manifest['limitations']}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('dataset')
    parser.add_argument('--output')
    args = parser.parse_args()
    result = audit_dataset(args.dataset)
    text = json.dumps(result, indent=2)+'\n'
    if args.output:
        path = Path(args.output)
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open('x') as stream:
            stream.write(text)
    print(text)
