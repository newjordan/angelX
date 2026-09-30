"""Offline checks that Grok's legacy table shares conservative proxy accounting."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/polyglot_grok47_table.py"


def load_module():
    spec = importlib.util.spec_from_file_location("grok_table", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


grok = load_module()


def trace(score=1, wall=10):
    return {"timing": {"agent": {"start": 0, "end": wall}},
            "metrics": {"input_tokens": 100, "output_tokens": 10, "reasoning_tokens": 0},
            "rewards": {"technical_outcome": {"score": score}},
            "info": {"angel": {"model": "wrapper-model", "reasoning_effort": "medium"}},
            "calls": [{"model": "fixture-grok", "sampling": {"reasoning_effort": "low"},
                       "usage": {"prompt_tokens": 80, "cached_input_tokens": 20,
                                 "completion_tokens": 10, "reasoning_tokens": 0}}]}


def write_run(root, traces):
    root.mkdir(parents=True, exist_ok=True)
    (root / "traces.jsonl").write_text(json.dumps({"task": {"data": {"name": "js-a"}}, "traces": traces}) + "\n")
    return root


class GrokTableTests(unittest.TestCase):
    def test_import_does_not_select_a_live_run(self):
        with mock.patch.object(Path, "glob", side_effect=AssertionError("unexpected live run discovery")):
            module = load_module()
        self.assertTrue(callable(module.load_rows))

    def test_proxy_metadata_and_complete_zero_reasoning_are_preserved(self):
        with tempfile.TemporaryDirectory() as tmp:
            row = grok.load_rows(write_run(Path(tmp), [trace()]))[0]
        self.assertEqual((row["model"], row["effort"]), ("fixture-grok", "low"))
        self.assertEqual((row["input_tokens"], row["cached_tokens"], row["output_tokens"], row["reasoning_tokens"]), (100, 20, 10, 0))
        self.assertEqual((row["wall_s"], row["hops"]), (10, 1))

    def test_partial_proxy_metrics_cannot_claim_complete_task_tokens(self):
        data = trace()
        data["calls"].append({"usage": None, "error": "fixture failure"})
        with tempfile.TemporaryDirectory() as tmp:
            rows = grok.load_rows(write_run(Path(tmp), [data]))
        self.assertIsNone(rows[0]["input_tokens"])
        self.assertIsNone(rows[0]["output_tokens"])
        self.assertEqual(rows[0]["usage_reported_subtotals"]["input_tokens"], 100)
        self.assertEqual(rows[0]["usage_metric_coverage"]["input_tokens"], {"reported": 1, "attempts": 2, "complete": False})
        self.assertEqual(grok.table.aggregate(rows)["usage_complete_attempts"], 0)

    def test_repeated_tasks_retain_failed_time_and_attempts(self):
        with tempfile.TemporaryDirectory() as tmp:
            rows = grok.load_rows(write_run(Path(tmp), [trace(score=0, wall=600), trace(wall=10)]))
        summary = grok.table.aggregate(rows)
        self.assertEqual(summary["duplicate_tasks"], {"js-a": 2})
        self.assertEqual(summary["metrics"]["wall_s"]["total"], 610)
        self.assertEqual(summary["metrics"]["input_tokens"]["total"], 200)

    def test_timeout_zero_placeholders_do_not_erase_proxy_measurements(self):
        data = trace(score=0, wall=600)
        data["metrics"] = {"wall_ms": 0, "hops": 0, "input_tokens": 0, "output_tokens": 0, "reasoning_tokens": 0}
        data["info"]["agent_exit"] = {"timed_out": True, "exit_code": 124}
        with tempfile.TemporaryDirectory() as tmp:
            row = grok.load_rows(write_run(Path(tmp), [data]))[0]
        self.assertEqual(row["wall_s"], 600)
        self.assertEqual(row["wall_source"], "agent-clock-timeout")
        self.assertEqual(row["hops"], 1)
        self.assertEqual(row["input_tokens"], 100)
        self.assertEqual(row["output_tokens"], 10)
        self.assertFalse(row["solved"])

    def test_default_main_still_uses_grok_discovery_and_recorded_metadata(self):
        with tempfile.TemporaryDirectory() as tmp:
            run = write_run(Path(tmp), [trace()])
            output = io.StringIO()
            with mock.patch.object(grok, "newest", return_value=run) as discover, contextlib.redirect_stdout(output):
                grok.main([])
            discover.assert_called_once_with()
        self.assertIn("model=fixture-grok effort=low", output.getvalue())
        self.assertIn("| fixture-grok, reasoning low |", output.getvalue())

    def test_partial_cli_reports_coverage_instead_of_false_zero_tokens(self):
        data = trace()
        data["calls"][0]["usage"] = {"prompt_tokens": 80}
        with tempfile.TemporaryDirectory() as tmp:
            run = write_run(Path(tmp), [data])
            result = subprocess.run([sys.executable, str(SCRIPT), str(run)], capture_output=True, text=True, timeout=5)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("usage_tasks=0/1", result.stdout)
        self.assertIn("— (0/1)", result.stdout)
        self.assertIn("model=fixture-grok effort=low", result.stdout)


if __name__ == "__main__":
    unittest.main()
