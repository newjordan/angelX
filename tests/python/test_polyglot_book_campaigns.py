"""Campaigns retain planned attempts and report failed child runs."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


HARNESS = Path(__file__).resolve().parents[2] / "scripts/bench/book-harness"


class CampaignTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="angel-book-campaign-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for name in ("e1.sh", "e3.sh", "e5.sh", "campaign_lifecycle.sh", "signal_children.py"):
            shutil.copy2(HARNESS / name, self.root / name)
        runner = '''#!/usr/bin/env python3
import json, os, pathlib, sys
root = pathlib.Path(__file__).resolve().parent
out = root / 'out/attempts'
out.mkdir(exist_ok=True)
label = sys.argv[2]
(out / (label + '.json')).write_text(json.dumps({
    'label': label, 'args': sys.argv[1:], 'route': os.environ['ROUTE'],
    'intro': os.environ.get('ANGEL_BOOK_INTRO')}))
raise SystemExit(17 if os.environ['ROUTE'] == 'broken' else 0)
'''
        for name in ("task_run.sh", "e3_session.sh"):
            path = self.root / name
            path.write_text(runner)
            path.chmod(0o755)
        self.env = dict(os.environ)
        self.env.pop("ANGEL_BOOK_INTRO", None)
        self.env.pop("LABEL_TAG", None)

    def run_campaign(self, name, args, expected, count):
        result = subprocess.run(["bash", str(self.root / name), *args], cwd=self.root,
                                env=self.env, text=True, capture_output=True, timeout=10)
        self.assertEqual(result.returncode, expected, result.stderr)
        rows = [json.loads(path.read_text()) for path in (self.root / "out/attempts").glob("*.json")]
        self.assertEqual(len(rows), count)
        self.assertEqual({row['route'] for row in rows}, {"working", "broken"} if expected else {"working", "other"})
        if name == "e1.sh":
            self.assertEqual({row['label'].split('-')[-2] for row in rows}, {"intro", "bare", "prebook"})
            for row in rows:
                arm = row['label'].split('-')[-2]
                self.assertEqual(row['intro'], {"intro": "1", "bare": "0", "prebook": None}[arm])
        elif name == "e5.sh":
            self.assertEqual({row['label'].split('-')[-2] for row in rows}, {"base", "head"})
            self.assertTrue(all(row['intro'] == "1" for row in rows))
        else:
            self.assertEqual({row['label'].split('-')[-2] for row in rows}, {"intro", "bare"})
            self.assertEqual(result.stdout.count("status=" + str(expected)), 2)

    def test_e1_creates_output_and_preserves_all_arms(self):
        self.run_campaign("e1.sh", ["working", "other"], 0, 30)

    def test_e5_creates_output_and_preserves_both_builds(self):
        self.run_campaign("e5.sh", ["working", "other"], 0, 20)

    def test_e3_creates_output_and_waits_for_each_round(self):
        self.run_campaign("e3.sh", ["2", "working", "other"], 0, 8)

    def test_e1_preserves_failed_exit_while_finishing_planned_attempts(self):
        self.run_campaign("e1.sh", ["working", "broken"], 17, 30)

    def test_e5_preserves_failed_exit_while_finishing_planned_attempts(self):
        self.run_campaign("e5.sh", ["working", "broken"], 17, 20)

    def test_e3_preserves_failed_exit_across_rounds(self):
        self.run_campaign("e3.sh", ["2", "working", "broken"], 17, 8)

    def test_invalid_arguments_fail_before_creating_output(self):
        cases = [(name, []) for name in ("e1.sh", "e3.sh", "e5.sh")]
        cases += [(name, ["../escape"]) for name in ("e1.sh", "e5.sh")]
        cases += [("e3.sh", [rounds, "working"]) for rounds in ("0", "-1", "abc")]
        cases += [("e3.sh", ["1", "../escape"])]
        cases += [(name, ["working", "working"]) for name in ("e1.sh", "e5.sh")]
        cases += [("e3.sh", ["2", "working", "working"])]
        for name, args in cases:
            with self.subTest(name=name, args=args):
                result = subprocess.run(["bash", str(self.root / name), *args], env=self.env,
                                        text=True, capture_output=True, timeout=3)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertFalse((self.root / "out").exists())
        result = subprocess.run(["bash", str(self.root / "e3.sh"), "1", "working"],
                                env=dict(self.env, LABEL_TAG="../../escape"),
                                text=True, capture_output=True, timeout=3)
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertFalse((self.root / "out").exists())

    def test_failed_or_invalid_shuffle_does_not_report_an_empty_success(self):
        tools = self.root / "tools"
        tools.mkdir()
        shuffle = tools / "shuf"
        for script, name, expected in [
                ("#!/bin/sh\nexit 23\n", "e1.sh", 23),
                ("#!/bin/sh\nexit 23\n", "e5.sh", 23),
                ("#!/bin/sh\necho intro\n", "e1.sh", 2),
                ("#!/bin/sh\necho base\n", "e5.sh", 2)]:
            with self.subTest(name=name, script=script):
                shuffle.write_text(script)
                shuffle.chmod(0o755)
                result = subprocess.run(["bash", str(self.root / name), "working"],
                                        env=dict(self.env, PATH=str(tools) + os.pathsep + self.env['PATH']),
                                        text=True, capture_output=True, timeout=3)
                self.assertEqual(result.returncode, expected, result.stderr)
                self.assertFalse((self.root / "out/attempts").exists())

    def test_failed_round_generator_does_not_report_an_empty_success(self):
        tools = self.root / "tools"
        tools.mkdir()
        generator = tools / "seq"
        generator.write_text("#!/bin/sh\nexit 23\n")
        generator.chmod(0o755)
        result = subprocess.run(["bash", str(self.root / "e3.sh"), "2", "working"],
                                env=dict(self.env, PATH=str(tools) + os.pathsep + self.env['PATH']),
                                text=True, capture_output=True, timeout=3)
        self.assertEqual(result.returncode, 23, result.stderr)
        self.assertFalse((self.root / "out/attempts").exists())


if __name__ == "__main__":
    unittest.main()
