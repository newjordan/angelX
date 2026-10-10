"""Cross-producer dashboard compatibility and pinned upstream provenance."""
import hashlib
import importlib.util
import json
import shutil
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ENGINE = ROOT / 'cockpit/research/labyrinth'


class LabyrinthEngineTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='angel-labyrinth-engine-')
        self.root = Path(self.temp.name)
        self.labdir = self.root / 'labyrinth'
        (self.labdir / 'dashboard').mkdir(parents=True)
        shutil.copy(ENGINE / 'dashboard.html', self.labdir / 'dashboard/template.html')
        spec = importlib.util.spec_from_file_location('angel_lab_under_test', ENGINE / 'lab.py')
        self.lab = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.lab)
        self.lab.ROOT = self.root
        self.lab.LAB = self.labdir
        self.write_knowledge([])

    def tearDown(self):
        self.temp.cleanup()

    def write_knowledge(self, nodes):
        (self.labdir / 'knowledge.json').write_text(json.dumps({'schema': 1, 'nodes': nodes}))

    def fragment(self, nodes, event=None):
        directory = self.labdir / 'angel/observations'
        directory.mkdir(parents=True, exist_ok=True)
        blob = json.dumps({'schema': 1, 'nodes': nodes, 'event': event or {
            'ts': '2026-10-08T00:00:00Z', 'type': 'proposed', 'summary': 'under review',
            'nodes': [node['id'] for node in nodes], 'evidence': [],
        }})
        path = directory / (hashlib.sha256(blob.encode()).hexdigest() + '.json')
        path.write_text(blob)
        return path

    def test_distinct_producers_share_a_task_without_replacing_its_links(self):
        for node in [
            {'id': 'k.deli', 'kind': 'conjecture', 'tier': 'T5', 'status': 'open',
             'title': 'reasoned direction', 'test': 'independent derivation', 'links': []},
            {'id': 'ev.rl', 'kind': 'evidence', 'tier': 'T4', 'status': 'observed',
             'title': 'measured failure', 'evidence': ['receipt'], 'links': []},
        ]:
            task = {'id': 'q.task', 'kind': 'question', 'title': 'task', 'status': 'open',
                    'links': [{'to': node['id'], 'rel': 'suggests'}]}
            self.fragment([task, node])
        self.assertEqual(self.lab.check(), [])
        self.lab.build()
        data = json.loads((self.labdir / 'dashboard/data.json').read_text())
        nodes = {node['id']: node for node in data['nodes']}
        self.assertEqual(len(nodes['q.task']['links']), 2)
        self.assertEqual(nodes['ev.rl']['tier'], 'T4')
        self.assertEqual(len(data['events']), 2)

    def test_curated_referee_result_takes_precedence_over_automatic_hunch(self):
        curated = {'id': 'h.same', 'kind': 'theorem', 'tier': 'T2', 'status': 'established',
                   'title': 'independently established', 'evidence': ['referee/report.md'],
                   'review': {'state': 'refereed', 'by': ['referee'], 'verdict': 'PROVED'}, 'links': []}
        self.write_knowledge([curated])
        self.fragment([{'id': 'h.same', 'kind': 'hunch', 'tier': 'T6', 'title': 'older guess', 'links': []}])
        nodes = self.lab.merged_nodes()
        self.assertEqual(len(nodes), 1)
        self.assertEqual(nodes[0]['review']['verdict'], 'PROVED')
        self.assertEqual(nodes[0]['tier'], 'T2')

    def test_research_text_cannot_close_the_dashboard_data_script(self):
        text = '</script><script>window.unwanted=true</script>'
        self.write_knowledge([{'id': 'q.text', 'kind': 'question', 'title': text, 'statement': text, 'links': []}])
        self.lab.build()
        html = (self.labdir / 'dashboard/index.html').read_text()
        data = json.loads((self.labdir / 'dashboard/data.json').read_text())
        self.assertNotIn(text, html)
        self.assertEqual(data['nodes'][0]['statement'], text)

    def test_malformed_observation_is_reported_instead_of_disappearing(self):
        path = self.fragment([])
        path.write_text('{broken')
        with self.assertRaises(json.JSONDecodeError):
            self.lab.merged_nodes()

    def test_observation_symlink_is_rejected(self):
        external = self.root / 'external.json'
        external.write_text('{"schema":1,"nodes":[]}')
        path = self.fragment([])
        path.unlink()
        path.symlink_to(external)
        with self.assertRaisesRegex(ValueError, 'invalid labyrinth observation'):
            self.lab.merged_nodes()

    def test_receipt_pins_every_shipped_resource_and_retained_license(self):
        receipt = json.loads((ENGINE / 'UPSTREAM.json').read_text())
        self.assertEqual(receipt['source_head'], '019a93da59313e697dc6f28bcfe1588a376c7565')
        self.assertEqual(receipt['license'], 'MIT')
        for entry in receipt['files']:
            actual = hashlib.sha256((ROOT / entry['path']).read_bytes()).hexdigest()
            self.assertEqual(actual, entry['sha256'], entry['path'])
            if not entry['adapted']:
                self.assertEqual(actual, entry['upstream_sha256'])
        license_text = (ROOT / 'third-party/labyrinth-exploration-LICENSE.txt').read_text()
        self.assertIn('Bartosz', license_text)

    def test_native_live_events_join_curated_and_observation_timelines(self):
        self.fragment([])
        event = {'ts': '2026-10-09T00:00:00Z', 'type': 'literature',
                 'summary': 'actual role started', 'nodes': [], 'evidence': ['report.md']}
        (self.labdir / 'angel/events.jsonl').write_text(json.dumps(event) + '\n')
        (self.labdir / 'events.jsonl').write_text(json.dumps({**event, 'type': 'reviewed'}) + '\n')
        self.assertEqual(len(self.lab.load_events()), 3)
        self.assertEqual(self.lab.check(), [])
        self.lab.build()
        data = json.loads((self.labdir / 'dashboard/data.json').read_text())
        self.assertEqual(len(data['events']), 3)

    def test_pending_or_mismatched_transactions_block_engine_readers(self):
        directory = self.labdir / 'angel/transactions'
        directory.mkdir(parents=True)
        token = 'a' * 64
        blob = json.dumps({'schema': 1, 'id': token, 'changes': []}).encode()
        (directory / f'{token}.prepared.json').write_bytes(blob)
        for read in [self.lab.merged_nodes, self.lab.check, self.lab.build]:
            with self.assertRaisesRegex(ValueError, 'integration pending'):
                read()
        self.assertFalse((self.labdir / 'dashboard/index.html').exists())
        marker = {'schema': 1, 'id': token, 'state': 'aborted',
                  'prepared_sha256': hashlib.sha256(blob).hexdigest()}
        marker_path = directory / f'{token}.aborted.json'
        marker_path.write_text(json.dumps(marker))
        self.assertEqual(self.lab.check(), [])
        marker_path.write_text(json.dumps({**marker, 'prepared_sha256': '0' * 64}))
        with self.assertRaisesRegex(ValueError, 'does not bind'):
            self.lab.check()

    def test_native_writer_lock_blocks_python_dashboard_reads(self):
        import os
        if os.name != 'posix':
            self.skipTest('descriptor locks require POSIX')
        import fcntl
        directory = self.labdir / 'angel'
        directory.mkdir(parents=True)
        with (directory / 'workflow.lock').open('w') as owner:
            fcntl.flock(owner, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with self.assertRaisesRegex(ValueError, 'coordinator is integrating'):
                self.lab.build()
        self.lab.build()
        self.assertTrue((self.labdir / 'dashboard/index.html').is_file())

    def test_full_pinned_workflow_contains_roles_and_reproducible_example(self):
        receipt = json.loads((ENGINE / 'UPSTREAM.json').read_text())
        paths = {entry['upstream_path'] for entry in receipt['files']
                 if '/upstream/' in entry['path']}
        self.assertEqual(len(paths), 46)
        required = {'SKILL.md', 'agents/openai.yaml', 'references/campaigns.md',
                    'references/codex.md', 'references/compute.md', 'references/saturation.md',
                    'references/lessons.md', 'references/loop.md', 'templates/briefs/attack.md',
                    'templates/briefs/referee.md', 'templates/briefs/writer.md',
                    'tests/test_skill.py', 'tests/test_lab.py',
                    'examples/triangle-counts/exhaustive_n8.py'}
        self.assertTrue(required.issubset(paths))


if __name__ == '__main__':
    unittest.main()
