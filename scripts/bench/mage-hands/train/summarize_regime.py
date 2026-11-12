"""Replay offline grounding receipts and check artifact consistency; never deploy.

This checks local records, not the truth of a remote execution or live-task
competence. Historical flag-only pilot output is intentionally rejected.
"""
import argparse
import hashlib
import importlib.util
import json
import math
from pathlib import Path

spec = importlib.util.spec_from_file_location('regime', Path(__file__).with_name('regime.py'))
regime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(regime)

CHECKPOINT_FILES = ('adapter_config.json', 'adapter_model.safetensors', 'merger.safetensors')
SOURCE_FILES = ('modal_regime.py', 'regime.py', 'regime_batch.py', 'summarize_regime.py')


def read_rows(path):
    return [json.loads(line) for line in Path(path).read_text().splitlines() if line.strip()]


def digest(path):
    hasher = hashlib.sha256()
    with Path(path).open('rb') as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b''):
            hasher.update(chunk)
    return hasher.hexdigest()


def required_json(path, label):
    try:
        value = json.loads(path.read_text())
    except (OSError, ValueError) as exc:
        raise ValueError(f'missing or invalid {label}: {path.name}') from exc
    if not isinstance(value, dict):
        raise ValueError(f'invalid {label}: expected object')
    return value


def check_digest(path, expected, label):
    if (not isinstance(expected, str) or len(expected) != 64
            or any(c not in '0123456789abcdef' for c in expected)):
        raise ValueError(f'invalid {label} SHA-256')
    try:
        actual = digest(path)
    except OSError as exc:
        raise ValueError(f'missing {label}: {path.name}') from exc
    if actual != expected:
        raise ValueError(f'{label} hash mismatch: {path.name}')


def paired_summary(baseline, candidate, expected_count):
    if type(expected_count) is not int or expected_count < 1:
        raise ValueError('expected count must be positive')
    if len(baseline) != expected_count or len(candidate) != expected_count:
        raise ValueError(f"incomplete pair: expected {expected_count}, have {len(baseline)}/{len(candidate)}")
    fields = ('split', 'family', 'screen', 'group', 'target', 'box')
    seen = set()
    for before, after in zip(baseline, candidate):
        if any(before.get(k) != after.get(k) for k in fields) or before.get('size') != after.get('size'):
            raise ValueError('paired target identities differ')
        key = tuple(json.dumps(before[k], sort_keys=True) for k in fields)
        if key in seen:
            raise ValueError('duplicate paired target')
        seen.add(key)
        if before.get('arm') != 'hands-v1' or after.get('arm') != 'candidate':
            raise ValueError('unexpected evaluation arm')
        for row in (before, after):
            if any(type(row.get(metric)) is not bool for metric in ('first', 'cascade')):
                raise ValueError('missing boolean grounding outcome')
            regime.verify_ground_record(row)
    result = {}
    for family in sorted({row['family'] for row in baseline}):
        pairs = [(x, y) for x, y in zip(baseline, candidate) if x['family'] == family]
        for metric in ('first', 'cascade'):
            result[f'{family}/{metric}'] = {
                'n': len(pairs), 'screens': len({x['screen'] for x, _ in pairs}),
                'groups': len({x['group'] for x, _ in pairs}),
                'v1': sum(x[metric] for x, _ in pairs),
                'candidate': sum(y[metric] for _, y in pairs),
                'gained': sum(not x[metric] and y[metric] for x, y in pairs),
                'lost': sum(x[metric] and not y[metric] for x, y in pairs),
            }
    return result


def summarize(root):
    root = Path(root)
    status = required_json(root / 'status.json', 'status')
    smoke = required_json(root / 'smoke.json', 'smoke')
    if status.get('state') != 'complete-not-deployed':
        raise ValueError('campaign not complete')
    vals = [smoke.get(k) for k in ('before', 'after', 'reloaded', 'parameter_l1_delta')]
    if (smoke.get('passed') is not True or smoke.get('fresh_model_reload') is not True
            or any(type(v) not in (int, float) or not math.isfinite(v) for v in vals)):
        raise ValueError('learning smoke evidence missing or invalid')
    before_loss, after_loss, reload_loss, change = vals
    if not (before_loss > 0 and 0 <= after_loss < before_loss * .95 and reload_loss >= 0 and change > 0
            and abs(reload_loss - after_loss) <= max(1e-5, abs(after_loss) * .001)):
        raise ValueError('learning smoke numeric gate failed despite its pass flag')

    reload_receipt = required_json(root / 'final-reload.json', 'final reload')
    if status.get('final_model_reloaded') is not True or reload_receipt.get('fresh_model_reload') is not True:
        raise ValueError('final fresh-model reload evidence missing')
    checkpoint = reload_receipt.get('checkpoint_sha256')
    if (not isinstance(checkpoint, dict) or set(checkpoint) != set(CHECKPOINT_FILES)
            or status.get('final_checkpoint_sha256') != checkpoint):
        raise ValueError('final checkpoint identities differ')
    for name, expected in checkpoint.items():
        check_digest(root / 'final' / name, expected, 'final checkpoint')
    config = required_json(root / 'execution-configuration.json', 'execution configuration')
    sources = config.get('sources_sha256')
    if (not isinstance(sources, dict) or set(sources) != set(SOURCE_FILES)
            or config.get('executed_source_sha256') != sources['modal_regime.py']):
        raise ValueError('executed source identities missing or inconsistent')
    for name, expected in sources.items():
        check_digest(root / ('executed-' + name), expected, 'source')
    baseline_checkpoint = config.get('baseline_checkpoint_sha256')
    if not isinstance(baseline_checkpoint, dict) or set(baseline_checkpoint) != set(CHECKPOINT_FILES):
        raise ValueError('baseline checkpoint identities missing')
    manifest = required_json(root / 'dataset-manifest.json', 'dataset manifest')
    if config.get('dataset_sha256') != status.get('dataset_sha256'):
        raise ValueError('dataset manifest identities differ')
    check_digest(root / 'dataset-manifest.json', status.get('dataset_sha256'), 'dataset manifest')
    if status.get('evaluation_scope') != 'full-frozen-offline':
        raise ValueError('full frozen evaluation scope missing')

    result = {}
    identity = ('split', 'family', 'screen', 'group', 'target', 'box')
    for split in ('test', 'validation'):
        frozen_path = root / 'evaluation-data' / (split + '.jsonl')
        check_digest(frozen_path, manifest.get('files', {}).get(split + '.jsonl'), 'frozen evaluation')
        frozen = read_rows(frozen_path)
        selection_path = root / ('eval-selection.json' if split == 'test' else 'validation-selection.json')
        try:
            selection = json.loads(selection_path.read_text())
        except (OSError, ValueError) as exc:
            raise ValueError(f'missing {split} selection') from exc
        if selection != frozen:
            raise ValueError(f'{split} selection differs from full frozen evaluation file')
        n = status.get(f'{split}_targets')
        if type(n) is not int or n < 1 or n != len(frozen):
            raise ValueError(f'{split} count differs from full frozen evaluation')
        before = read_rows(root / f'eval-{split}-hands-v1.jsonl')
        after = read_rows(root / f'eval-{split}-candidate.jsonl')
        result.update({f'{split}/{key}': value for key, value in paired_summary(before, after, n).items()})
        for arm_rows, hashes in ((before, baseline_checkpoint), (after, checkpoint)):
            for row, target in zip(arm_rows, frozen):
                if (row.get('split') != split or any(row.get(k) != target.get(k) for k in identity)
                        or row.get('size') != [target.get('width'), target.get('height')]):
                    raise ValueError(f'{split} rows differ from frozen target identities or geometry')
                screen = Path(target['screen'])
                if screen.parent.as_posix() != 'screens':
                    raise ValueError('invalid frozen image path')
                expected_image = manifest.get('screens', {}).get(screen.name)
                if not expected_image or row.get('image_sha256') != expected_image:
                    raise ValueError('row image hash differs from frozen manifest')
                if row.get('checkpoint_sha256') != hashes:
                    raise ValueError('row checkpoint differs from reloaded checkpoint identity')
    remote = required_json(root / 'summary.json', 'remote summary')
    if remote != result:
        raise ValueError('remote/local aggregate mismatch')
    training = read_rows(root / 'training.jsonl')
    samples = tokens = 0
    if not training:
        raise ValueError('no training telemetry')
    for step, row in enumerate(training, 1):
        if (row.get('step') != step
                or any(type(row.get(k)) is not int or row[k] < 1 for k in ('window_samples', 'window_answer_tokens'))
                or type(row.get('loss')) not in (int, float) or not math.isfinite(row['loss']) or row['loss'] < 0):
            raise ValueError('invalid training telemetry')
        samples += row['window_samples']
        tokens += row['window_answer_tokens']
        if row.get('samples') != samples or row.get('answer_tokens') != tokens:
            raise ValueError('training telemetry totals differ')
    if (status.get('steps') != len(training) or status.get('samples') != samples
            or status.get('answer_tokens') != tokens):
        raise ValueError('training status differs from telemetry')
    legacy_regression = any(v['candidate'] < v['v1'] for k, v in result.items()
                            if k.startswith('test/legacy-test-') and k.endswith('/cascade'))
    cost = status.get('estimated_compute_usd')
    rate = status.get('compute_rate_usd_per_hour')
    cost_text = (f'Estimated compute: ${cost:.2f} at the supplied ${rate:.2f}/hour; not an invoice or storage total.'
                 if all(type(v) in (int, float) and math.isfinite(v) and v >= 0 for v in (cost, rate))
                 else 'No valid compute estimate supplied; no billing claim.')
    text = ['# Offline grounding — replayed artifact checks', '',
            '**Not deployed.** This is not a live-task or native Steam result.', '',
            f"Run: `{status['run']}`; recorded GPU: {status['gpu']}; model revision: `{status['revision']}`.",
            f'Learning smoke: {before_loss:.6f} → {after_loss:.6f}; reload {reload_loss:.6f}.',
            f"Training: {status['steps']} optimizer steps, {samples} samples, {tokens} answer tokens.",
            cost_text, '', '## Full frozen offline test', '',
            '| Family / metric | Targets / groups | v1 | Candidate | Gained / lost |',
            '|---|---:|---:|---:|---:|']
    for key, value in result.items():
        if key.startswith('test/'):
            n = value['n']
            text.append(f"| {key.removeprefix('test/')} | {n} / {value['groups']} | {value['v1']}/{n} | {value['candidate']}/{n} | {value['gained']} / {value['lost']} |")
    text += ['', '## What these checks establish', '',
             '- First-look and final coordinates, parse outcomes, crops and hit flags were recomputed from saved raw replies.',
             '- Both evaluation sets exactly match their frozen files and manifest hashes, with all paired targets accounted for.',
             '- The local final checkpoint files match the reload receipt and every candidate evaluation record.',
             '- Source file hashes, recorded smoke numbers and training counters are internally consistent.', '',
             '## What these checks do not establish', '',
             '- These are consistency checks on supplied artifacts, not independent attestation of remote execution.',
             '- Baseline checkpoint identity and screenshot hashes are recorded; this reporter does not load baseline weights or image bytes.',
             '- The generation backend is Transformers, not production SGLang. JPEG/Lanczos preprocessing is Pillow, not the Rust image crate.',
             '- Parser/geometry parity does not prove backend or pixel-level parity.',
             '- Grouped holdouts do not remove all layout/semantic similarity. No broad capability claim follows from these counts.',
             '- Synthetic library screens are not native Steam evidence. No live primary-driver gate has passed.',
             f'- Legacy cascade regression in this frozen test: **{legacy_regression}**.',
             '- No promotion decision is automated. Optimization/save/reload mechanics are not model-quality proof.', '']
    audit = {'complete_pairs': True, 'raw_reply_replay': True, 'checkpoint_files_match': True,
             'full_frozen_evaluation': True, 'independent_execution_attestation': False,
             'legacy_regression': legacy_regression, 'deployed': False, 'summary': result,
             'training_telemetry_records': len(training),
             'files_sha256': {str(p.relative_to(root)): digest(p) for p in sorted(root.rglob('*'))
                              if p.is_file() and p.name not in ('audit.json', 'results.md')}}
    # No report is emitted until all checks above have passed.
    (root / 'results.md').write_text('\n'.join(text))
    (root / 'audit.json').write_text(json.dumps(audit, indent=2) + '\n')
    print('\n'.join(text))
    return audit


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory')
    args = parser.parse_args()
    summarize(args.directory)
