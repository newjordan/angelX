"""Regression checks for full-cohort latency accounting and trace discovery."""
import importlib.util
from pathlib import Path
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


if __name__ == '__main__':
    unittest.main()
