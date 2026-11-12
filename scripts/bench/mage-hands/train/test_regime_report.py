"""Local receipt/collector tests: synthetic files, no SDK, model or network."""
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('reporter', HERE / 'summarize_regime.py')
reporter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(reporter)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def put_json(path, obj):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(obj) + '\n')


def put_rows(path, rows):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(''.join(json.dumps(row) + '\n' for row in rows))


class ReportTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name) / 'receipts'
        self.root.mkdir()
        r = self.root
        self.screen_hash = hashlib.sha256(b'synthetic image fixture, not a real evaluation').hexdigest()
        self.checkpoint = {}
        for name in ('adapter_config.json', 'adapter_model.safetensors', 'merger.safetensors'):
            path = r / 'final' / name
            path.parent.mkdir(exist_ok=True)
            path.write_bytes(('synthetic ' + name).encode())
            self.checkpoint[name] = sha(path)
        sources = {}
        for name in ('modal_regime.py', 'regime.py', 'regime_batch.py', 'summarize_regime.py'):
            path = r / ('executed-' + name)
            path.write_text('# synthetic source receipt: ' + name)
            sources[name] = sha(path)
        selections, summary, files = {}, {}, {}
        for split in ('test', 'validation'):
            row = {'family': 'shopping', 'screen': f'screens/{self.screen_hash}.png',
                   'group': 'g-' + split, 'target': 'Cart', 'box': [1, 2, 3, 4],
                   'width': 100, 'height': 80, 'split': split}
            selections[split] = [row]
            selection = 'eval-selection.json' if split == 'test' else 'validation-selection.json'
            put_json(r / selection, [row])
            path = r / 'evaluation-data' / (split + '.jsonl')
            put_rows(path, [row])
            files[split + '.jsonl'] = sha(path)
            arms = []
            for arm in ('hands-v1', 'candidate'):
                record = reporter.regime.ground_record(row, arm, split, (100, 80),
                    ['<tool_call>{"arguments":{"coordinate":[20,38]}}</tool_call>', 'no point'])
                record.update(image_sha256=self.screen_hash, checkpoint_sha256=self.checkpoint)
                arms.append([record])
                put_rows(r / f'eval-{split}-{arm}.jsonl', [record])
            summary.update({f'{split}/{k}': v for k, v in reporter.paired_summary(*arms, 1).items()})
        manifest = {'files': files, 'screens': {self.screen_hash + '.png': self.screen_hash},
                    'limitations': ['synthetic test fixture']}
        put_json(r / 'dataset-manifest.json', manifest)
        self.status = {'state': 'complete-not-deployed', 'run': 'test-fixture', 'gpu': 'none-test',
                       'revision': 'fixture', 'steps': 1, 'samples': 4, 'answer_tokens': 8,
                       'test_targets': 1, 'validation_targets': 1, 'final_model_reloaded': True,
                       'final_checkpoint_sha256': self.checkpoint, 'evaluation_scope': 'full-frozen-offline',
                       'dataset_sha256': sha(r / 'dataset-manifest.json'),
                       'compute_rate_usd_per_hour': 10., 'estimated_compute_usd': 0.}
        put_json(r / 'status.json', self.status)
        put_json(r / 'execution-configuration.json', {'dataset_sha256': self.status['dataset_sha256'],
                  'sources_sha256': sources, 'executed_source_sha256': sources['modal_regime.py'],
                  'baseline_checkpoint_sha256': self.checkpoint})
        self.smoke = {'passed': True, 'before': 1., 'after': .1, 'reloaded': .1,
                      'parameter_l1_delta': 1., 'fresh_model_reload': True}
        put_json(r / 'smoke.json', self.smoke)
        put_json(r / 'final-reload.json', {'fresh_model_reload': True, 'checkpoint_sha256': self.checkpoint})
        put_json(r / 'summary.json', summary)
        put_rows(r / 'training.jsonl', [{'step': 1, 'samples': 4, 'answer_tokens': 8,
                                       'window_samples': 4, 'window_answer_tokens': 8, 'loss': .1}])

    def summarize(self):
        with patch('builtins.print'):
            return reporter.summarize(self.root)

    def test_complete_artifact_set_is_replayed_without_claiming_live_or_gpu_verification(self):
        audit = self.summarize()
        self.assertTrue(audit['raw_reply_replay'])
        self.assertTrue(audit['checkpoint_files_match'])
        self.assertTrue(audit['full_frozen_evaluation'])
        self.assertFalse(audit['deployed'])
        text = (self.root / 'results.md').read_text()
        self.assertIn('not a live-task', text)
        self.assertIn('not production SGLang', text)
        self.assertNotIn('Five real Store test screens', text)

    def test_missing_final_reload_does_not_write_a_report(self):
        (self.root / 'final-reload.json').unlink()
        with self.assertRaisesRegex(ValueError, 'reload'):
            self.summarize()
        self.assertFalse((self.root / 'results.md').exists())

    def test_corrupt_final_checkpoint_is_rejected(self):
        (self.root / 'final' / 'merger.safetensors').write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'checkpoint'):
            self.summarize()

    def test_false_smoke_pass_flag_cannot_override_failed_numeric_gate(self):
        put_json(self.root / 'smoke.json', {**self.smoke, 'after': 2., 'reloaded': 2.})
        with self.assertRaisesRegex(ValueError, 'smoke'):
            self.summarize()

    def test_source_receipt_mismatch_is_rejected(self):
        (self.root / 'executed-modal_regime.py').write_text('# changed')
        with self.assertRaisesRegex(ValueError, 'source'):
            self.summarize()

    def test_frozen_validation_selection_is_checked_not_just_test(self):
        put_json(self.root / 'validation-selection.json', [])
        with self.assertRaisesRegex(ValueError, 'selection'):
            self.summarize()

    def test_declared_counts_cannot_hide_omitted_frozen_targets(self):
        path = self.root / 'evaluation-data' / 'test.jsonl'
        rows = reporter.read_rows(path)
        put_rows(path, rows + [{**rows[0], 'target': 'Checkout'}])
        manifest = json.loads((self.root / 'dataset-manifest.json').read_text())
        manifest['files']['test.jsonl'] = sha(path)
        put_json(self.root / 'dataset-manifest.json', manifest)
        self.status['dataset_sha256'] = sha(self.root / 'dataset-manifest.json')
        put_json(self.root / 'status.json', self.status)
        config = json.loads((self.root / 'execution-configuration.json').read_text())
        config['dataset_sha256'] = self.status['dataset_sha256']
        put_json(self.root / 'execution-configuration.json', config)
        with self.assertRaisesRegex(ValueError, 'selection|count|frozen'):
            self.summarize()

    def test_row_image_and_checkpoint_hashes_are_bound_to_frozen_inputs(self):
        path = self.root / 'eval-test-candidate.jsonl'
        rows = reporter.read_rows(path)
        original = copy.deepcopy(rows)
        for field in ('image_sha256', 'checkpoint_sha256'):
            rows = copy.deepcopy(original)
            rows[0][field] = 'unrelated'
            put_rows(path, rows)
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'image|checkpoint'):
                self.summarize()

    def test_collector_refuses_existing_destination_before_invoking_any_cloud_cli(self):
        marker = self.root / 'original-file'
        marker.write_text('preserve me')
        bin_dir = Path(self.tmp.name) / 'bin'
        bin_dir.mkdir()
        uv = bin_dir / 'uv'
        called = Path(self.tmp.name) / 'cloud-called'
        uv.write_text('#!/bin/sh\nprintf called > "$CALL_MARKER"\nexit 1\n')
        uv.chmod(0o755)
        run = subprocess.run(['bash', str(HERE / 'collect_regime.sh'), 'test-fixture', str(self.root)],
                             env={**os.environ, 'PATH': str(bin_dir) + ':' + os.environ['PATH'],
                                  'CALL_MARKER': str(called)}, capture_output=True, text=True)
        self.assertNotEqual(run.returncode, 0)
        self.assertFalse(called.exists(), 'existing report directory must be refused before cloud access')
        self.assertEqual(marker.read_text(), 'preserve me')


if __name__ == '__main__':
    unittest.main()
