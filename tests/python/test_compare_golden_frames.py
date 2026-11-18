import contextlib

import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[2] / 'scripts/check/compare-golden-frames.py'
spec = importlib.util.spec_from_file_location('compare_golden_frames', SCRIPT)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def sheet():
    return {'width': 2, 'height': 1,
            'palette': [['White', 'Reset', 'NONE'], ['Red', 'Black', 'BOLD']],
            'cells': [['x', 0], [' ', 1]]}


def write_scene(directory, value=None, text='x\n', name='fixture'):
    directory = Path(directory)
    (directory / f'{name}.txt').write_text(text, encoding='utf-8')
    (directory / f'{name}.json').write_text(json.dumps(sheet() if value is None else value), encoding='utf-8')


class CompareGoldenFramesTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.a = Path(self.temporary.name) / 'a'
        self.b = Path(self.temporary.name) / 'b'
        self.a.mkdir()
        self.b.mkdir()

    def compare(self):
        stdout, stderr = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            status = module.main(['compare.py', str(self.a), str(self.b)])
        return status, stdout.getvalue(), stderr.getvalue()

    def test_valid_identical_scene(self):
        write_scene(self.a)
        write_scene(self.b)
        self.assertEqual(self.compare(), (0, 'all 1 scenes identical\n', ''))

    def test_palette_reordering_and_unused_entries_are_not_visual_changes(self):
        write_scene(self.a)
        value = sheet()
        value['palette'].reverse()
        for cell in value['cells']:
            cell[1] = 1 - cell[1]
        value['palette'].append(['Green', 'Reset', 'NONE'])
        write_scene(self.b, value)
        self.assertEqual(self.compare()[0], 0)

    def test_text_symbol_style_and_dimensions_each_cause_a_difference(self):
        for kind in ('text', 'symbol', 'style', 'dimensions'):
            with self.subTest(kind=kind):
                write_scene(self.a)
                value = sheet()
                text = 'x\n'
                if kind == 'text':
                    text = 'y\n'
                elif kind == 'symbol':
                    value['cells'][0][0] = 'y'
                elif kind == 'style':
                    value['cells'][0][1] = 1
                else:
                    value['width'], value['height'] = 1, 2
                write_scene(self.b, value, text=text)
                status, out, err = self.compare()
                self.assertEqual(status, 1)
                self.assertIn('1 of 1 scenes differ', out)
                self.assertEqual(err, '')

    def test_missing_counterpart_is_a_difference(self):
        write_scene(self.a)
        write_scene(self.b)
        (self.b / 'fixture.json').unlink()
        self.assertEqual(self.compare()[0], 1)

    def test_empty_captures_are_not_evidence_of_equality(self):
        (self.a / 'unrelated.log').write_text('no frames captured')
        status, out, err = self.compare()
        self.assertEqual(status, 2)
        self.assertNotIn('identical', out)
        self.assertIn('no scenes', err.lower())

    def test_truncated_or_extra_cells_are_invalid_on_either_side(self):
        for count in (0, 1, 3):
            for side in ('a', 'b', 'both'):
                with self.subTest(count=count, side=side):
                    value = sheet()
                    value['cells'] = (value['cells'] * 2)[:count]
                    write_scene(self.a, value if side in ('a', 'both') else sheet())
                    write_scene(self.b, value if side in ('b', 'both') else sheet())
                    status, out, err = self.compare()
                    self.assertEqual(status, 2)
                    self.assertNotIn('identical', out)
                    self.assertIn('cell count', err.lower())
                    self.assertIn('fixture.json', err)

    def test_malformed_sheets_have_controlled_errors(self):
        bad = [None, [], 3, {}, {'width': 2}]
        for key in ('width', 'height'):
            for value in (0, -1, True, 1.5, '2', None):
                candidate = sheet()
                candidate[key] = value
                bad.append(candidate)
        for palette in (None, {}, [], ['White'], [['White']],
                        [['White', 'Reset', 0]], [[True, False, None]]):
            candidate = sheet()
            candidate['palette'] = palette
            bad.append(candidate)
        for cells in (None, {}, [['x', -1], [' ', 1]], [['x', True], [' ', 1]],
                      [['x', 0.0], [' ', 1]], [['x', 2], [' ', 1]],
                      [[None, 0], [' ', 1]], [['x'], [' ', 1]], ['x', [' ', 1]]):
            candidate = sheet()
            candidate['cells'] = cells
            bad.append(candidate)
        for value in bad:
            with self.subTest(value=value):
                write_scene(self.a)
                write_scene(self.b)
                (self.b / 'fixture.json').write_text(json.dumps(value), encoding='utf-8')
                status, out, err = self.compare()
                self.assertEqual(status, 2)
                self.assertNotIn('identical', out)
                self.assertIn('fixture.json', err)

    def test_invalid_json_and_unreadable_paths_are_controlled_errors(self):
        write_scene(self.a)
        write_scene(self.b)
        (self.b / 'fixture.json').write_text('{', encoding='utf-8')
        self.assertEqual(self.compare()[0], 2)
        (self.b / 'fixture.json').unlink()
        (self.b / 'fixture.json').mkdir()
        self.assertEqual(self.compare()[0], 2)
        (self.b / 'fixture.json').rmdir()
        with contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(module.main(['compare.py', str(self.a), str(self.b / 'missing')]), 2)

    def test_invalid_text_encoding_is_not_comparable_evidence(self):
        write_scene(self.a)
        write_scene(self.b)
        (self.b / 'fixture.txt').write_bytes(b'\xff')
        status, out, err = self.compare()
        self.assertEqual(status, 2)
        self.assertNotIn('identical', out)
        self.assertIn('fixture', err)

    def test_cli_reports_invalid_evidence_without_a_traceback(self):
        write_scene(self.a)
        value = sheet()
        value['cells'].pop()
        write_scene(self.b, value)
        result = subprocess.run([sys.executable, str(SCRIPT), str(self.a), str(self.b)],
                                capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 2)
        self.assertNotIn('Traceback', result.stderr)
        self.assertNotIn('identical', result.stdout)

    def test_usage(self):
        with contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(module.main(['compare.py']), 2)


if __name__ == '__main__':
    unittest.main()
