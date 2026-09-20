import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('active_connections', (Path(__file__).resolve().parents[2] / "scripts" / 'check-active-connections.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ActiveConnectionsTest(unittest.TestCase):
    def test_live_module_and_include_anchors_with_missing_route(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'cockpit/src').mkdir(parents=True)
            (root / 'scripts').mkdir()
            (root / 'cockpit/Cargo.toml').write_text('[[bin]]\nname="fixture"\npath="src/main.rs"\n')
            (root / 'cockpit/src/main.rs').write_text('mod live;\ninclude!("table.rs");\n')
            (root / 'cockpit/src/live.rs').write_text('fn live() {}')
            (root / 'cockpit/src/table.rs').write_text('const VALUE: u8 = 1;')
            (root / 'cockpit/src/dead.rs').write_text('fn orphan() {}')
            (root / 'scripts/route.mjs').write_text('import { x } from "./absent.mjs";')
            report = module.audit(root)
            self.assertEqual(report['rust_files_without_anchor'], ['cockpit/src/dead.rs'])
            self.assertEqual(report['missing_literal_targets'], [{'source': 'scripts/route.mjs', 'target': './absent.mjs'}])
            (root / 'cockpit/src/dead.rs').unlink()
            (root / 'scripts/absent.mjs').write_text('export const x = new URL("..", import.meta.url);')
            self.assertTrue(module.audit(root)['ok'])

    def test_outside_and_hidden_targets_are_never_opened(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'scripts').mkdir()
            (root / 'scripts/leak.mjs').symlink_to('/etc/passwd')
            (root / 'scripts/route.mjs').write_text('import "./leak.mjs"; import "../.env";')
            report = module.audit(root)
            self.assertEqual(report['source_files_checked'], 1)
            self.assertEqual(len(report['missing_literal_targets']), 2)

    def test_symlinked_source_ancestor_is_not_even_enumerated(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / 'workspace'
            outside = Path(tmp) / 'outside'
            root.mkdir()
            (outside / 'src').mkdir(parents=True)
            (root / 'cockpit').symlink_to(outside, target_is_directory=True)
            with patch.object(module.os, 'walk', side_effect=AssertionError('unsafe enumeration')):
                self.assertEqual(module.audit(root)['source_files_checked'], 0)


if __name__ == '__main__':
    unittest.main()
