"""Local assembly regression tests; full-data acceptance is a separate artifact audit."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

HERE = Path(__file__).resolve().parent

def load(name):
    spec = importlib.util.spec_from_file_location(name, HERE / (name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

class RecoveryTests(unittest.TestCase):
    def assembly(self):
        self.assertTrue((HERE / 'assemble_recovery.py').exists(), 'dataset assembler not implemented')
        return load('assemble_recovery')

    def test_same_app_under_different_names_quarantines_whole_groups_without_reassignment(self):
        mod = self.assembly()
        rows = [dict(group='store:400', split='train', entities=['steam-app:400'], image='a'),
                dict(group='search:portal', split='test', entities=['steam-app:400'], image='b'),
                dict(group='search:portal', split='test', entities=[], image='c'),
                dict(group='clean', split='train', entities=['steam-app:500'], image='d')]
        kept, excluded = mod.partition(rows)
        self.assertEqual(kept, rows[-1:])
        self.assertEqual(len(excluded), 3)
        self.assertTrue(all('cross-split entity' in r['reason'] for r in excluded))
        self.assertEqual(rows[0]['split'], 'train')
        self.assertEqual(rows[1]['split'], 'test')

    def test_identical_pixels_in_all_splits_are_quarantined_even_if_png_bytes_differ(self):
        mod = self.assembly()
        rows = [dict(group='a', split='train', entities=[], image='x', pixels='same'),
                dict(group='b', split='test', entities=[], image='y', pixels='same'),
                dict(group='c', split='validation', entities=[], image='z', pixels='different')]
        kept, excluded = mod.partition(rows)
        self.assertEqual(kept, rows[2:])
        self.assertEqual(len(excluded), 2)

    def test_replay_requires_exact_label_match_and_coordinate_in_recovered_box(self):
        mod = self.assembly()
        row = {'target': 'the "Cart" link', 'crop': None, 'width': 1000, 'height': 1000,
               'answer': '<tool_call>{"arguments":{"coordinate":[25,35]}}</tool_call>'}
        page = {'elements':[{'text':'Cart','tag':'a','box':[20,30,40,50]},
                            {'text':'Cartoon','tag':'a','box':[60,60,90,90]}]}
        self.assertEqual(mod.recover_replay(row, page), [20,30,40,50])
        row['answer']='<tool_call>{"arguments":{"coordinate":[70,70]}}</tool_call>'
        with self.assertRaisesRegex(ValueError, 'replay'):
            mod.recover_replay(row,page)

    def test_audit_checks_coordinate_geometry_without_trusting_annotation_flag(self):
        self.assertTrue((HERE / 'audit_recovery.py').exists(), 'dataset auditor not implemented')
        audit = load('audit_recovery')
        row = {'width':1000,'height':1000,'box':[20,30,40,50],'crop':None,'zoom':1,
               'target':'Cart','answer':'Action: Click Cart.\n\n<tool_call>\n{"name":"computer_use","arguments":{"action":"click","coordinate":[25,35],"button":"left"}}\n</tool_call>'}
        audit.check_geometry(row)
        row['answer']=row['answer'].replace('[25,35]','[70,70]')
        with self.assertRaisesRegex(ValueError, 'outside'):
            audit.check_geometry(row)
        row['crop']=[-1,0,500,500]
        with self.assertRaisesRegex(ValueError, 'crop'):
            audit.check_geometry(row)

    def test_source_dataset_split_hashes_are_checked_before_building(self):
        mod = self.assembly()
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            (root/'manifest.json').write_text(json.dumps({'files':{'train.jsonl':'0'*64},'screens':{},'system_sha256':'0'*64}))
            (root/'train.jsonl').write_text('{}\n')
            with self.assertRaisesRegex(ValueError,'hash'):
                mod.verify_base(root)

if __name__=='__main__': unittest.main()
