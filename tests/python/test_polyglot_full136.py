"""Regression checks for full-cohort latency accounting and trace discovery."""
import importlib.util
import json
from pathlib import Path
import tempfile
import subprocess
import sys
import unittest

spec = importlib.util.spec_from_file_location('full136', Path(__file__).resolve().parents[2] / 'scripts/analyze-polyglot-full136.py')
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


def row(task, wall, solved):
    result = audit.history.extract_row({'task': {'data': {'name': task}}}, {}, 'run')
    result.update(task=task, wall_s=wall, solved=solved, score=int(solved), language=task.split('-')[0], line=1)
    return result


class FullCohortTests(unittest.TestCase):
    def test_timespan_uses_timestamps_not_absent_duration_property(self):
        self.assertEqual(audit.span({'start': 100, 'end': 700}), 600)
        self.assertIsNone(audit.span({'start': 100, 'end': 0}))
        self.assertIsNone(audit.span({'duration': 600}))
        self.assertEqual(audit.span({'start': 100, 'end': 100}), 0)

    def test_all_task_pairing_preserves_failed_timeout_tail(self):
        own = {'id': 'angel', 'rows': [row('js-a', 20, True), row('rust-b', 600, False)]}
        peer = {'id': 'peer', 'rows': [row('js-a', 30, True), row('rust-b', 10, True)]}
        all_tasks = audit.pair(own, peer)
        jointly_solved = audit.pair(own, peer, True)
        self.assertEqual(all_tasks['tasks'], 2)
        self.assertEqual(all_tasks['metrics']['wall_s']['total'], 580)
        self.assertEqual(all_tasks['failure_involved_delta_s'], 590)
        self.assertEqual(jointly_solved['tasks'], 1)
        self.assertEqual(jointly_solved['metrics']['wall_s']['total'], -10)

    def test_unknown_partitions_do_not_remove_wall_time(self):
        result = audit.summarize([row('cpp-a', 600, True)])
        self.assertEqual(result['metrics']['wall_s']['total'], 600)
        self.assertIsNone(result['metrics']['model_s']['total'])
        self.assertEqual(result['model_partition_unknown_wall_s'], 600)

    def test_unrecognized_directory_is_retained_for_review(self):
        cell = {'id': 'new-variant/qwen/angel/run', 'rows': [row('js-a', 1, True)]}
        self.assertEqual(audit.classify(cell, {'js-a'}), 'UNKNOWN: retained for review')
        cell['id'] = 'QUARANTINE-old/qwen/angel/run'
        self.assertEqual(audit.classify(cell, {'js-a'}), 'quarantined')

    def test_repeated_attempts_are_not_collapsed_into_a_pair(self):
        own = {'id': 'angel', 'rows': [row('js-a', 600, False), row('js-a', 10, True)]}
        peer = {'id': 'peer', 'rows': [row('js-a', 20, True)]}
        result = audit.pair(own, peer)
        self.assertEqual(result['tasks'], 0)
        self.assertEqual(result['ambiguous_tasks'], ['js-a'])
        self.assertEqual(result['duplicate_attempts']['angel'], {'js-a': 2})
        self.assertEqual(audit.summarize(own['rows'])['metrics']['wall_s']['total'], 610)

    def test_unobserved_response_nodes_and_partial_call_timing_are_audited(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path = root / 'runs/model/angelx/run/traces.jsonl'
            path.parent.mkdir(parents=True)
            record = {'task': {'data': {'name': 'js-a'}}, 'traces': [{
                'timing': {'agent': {'start': 10, 'end': 20, 'model': {'duration': 10}}},
                'nodes': [{'message': {'role': 'assistant', 'content': 'valid'}}],
                'calls': [
                    {'node': 0, 'time': {'start': 10, 'end': 12}},
                    {'node': -1, 'time': {'start': 12}},
                    {'node': 5, 'time': {'start': 12, 'end': 14}},
                    {'node': True},
                ],
            }]}
            path.write_text(json.dumps(record))
            result = audit.read_trace(root, path)
            attempt = result['rows'][0]
            self.assertEqual(attempt['content_chars'], 5)
            self.assertEqual(attempt['sampled_response_calls'], 1)
            self.assertEqual(attempt['call_time_reported_sum_s'], 4)
            self.assertIsNone(attempt['call_time_sum_s'])
            self.assertEqual(attempt['calls_missing_end'], 2)
            self.assertEqual(result['timing_audit']['invalid_response_node'], 3)
            self.assertNotIn('proxy_call_duration_mismatch', result['timing_audit'])

    def test_partial_inventory_does_not_require_unrelated_full_cohorts(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'tasks-polyglot-v1.json').write_text(json.dumps([{'name': 'js-a'}]))
            path = root / 'runs-new/deepseek/angelx/seed0-test/traces.jsonl'
            path.parent.mkdir(parents=True)
            path.write_text(json.dumps({'task': {'data': {'name': 'js-a'}}, 'traces': [{}]}))
            output = root / 'audit.json'
            result = subprocess.run([sys.executable, str(Path(audit.__file__)), '--root', str(root), '--output', str(output)],
                                    text=True, capture_output=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stderr)
            data = json.loads(output.read_text())
            self.assertEqual(len(data['inventory']), 1)
            self.assertEqual(data['full_cohorts'], [])
            self.assertEqual(data['pairings'], [])


if __name__ == '__main__':
    unittest.main()
