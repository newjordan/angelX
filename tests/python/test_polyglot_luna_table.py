"""Offline direct-seat table checks for coverage, clocks and repeated attempts."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/polyglot_luna_table.py"


def load_module():
    spec = importlib.util.spec_from_file_location("luna_table", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


luna = load_module()


def trace(score=1, wall=10):
    return {"timing": {"agent": {"start": 0, "end": wall}},
            "rewards": {"technical_outcome": {"score": score}},
            "info": {"angel": {"model": "fixture", "reasoning_effort": "medium", "hops": 2,
                "usage": {"attempts": 2, "input": 100, "cache_read": 20, "output": 10, "reasoning": 0,
                          "core_complete": True, "reported_attempts": {"input": 2, "cache_read": 2, "output": 2, "reasoning": 2}}}}}


def write_run(root, traces, name="py-a"):
    root.mkdir(parents=True, exist_ok=True)
    (root / "traces.jsonl").write_text(json.dumps({"task": {"data": {"name": name, "idx": 0}}, "traces": traces}) + "\n")
    return root


class LunaTableTests(unittest.TestCase):
    def test_import_does_not_select_a_live_run(self):
        with mock.patch.object(Path, "glob", side_effect=AssertionError("unexpected live run discovery")):
            module = load_module()
        self.assertTrue(callable(module.load_rows))

    def test_absent_task_wall_metric_uses_validated_agent_clock(self):
        with tempfile.TemporaryDirectory() as tmp:
            rows = luna.load_rows(write_run(Path(tmp), [trace(wall=12)]))
        self.assertEqual(rows[0]["wall_s"], 12)
        self.assertEqual(rows[0]["wall_source"], "agent-clock")
        self.assertEqual(luna.aggregate(rows)["metrics"]["wall_s"]["median"], 12)

    def test_observed_zero_wall_and_hops_are_preserved(self):
        row = trace(wall=12)
        row["metrics"] = {"wall_ms": 0, "hops": 0}
        with tempfile.TemporaryDirectory() as tmp:
            rows = luna.load_rows(write_run(Path(tmp), [row]))
        self.assertEqual((rows[0]["wall_s"], rows[0]["hops"]), (0, 0))
        self.assertEqual(rows[0]["wall_source"], "task-metric")

    def test_invalid_wall_metrics_do_not_turn_into_zero_or_hide_behind_fallback(self):
        for wall in (None, True, -1, float("nan"), float("inf")):
            with self.subTest(wall=wall), tempfile.TemporaryDirectory() as tmp:
                row = trace(wall=12)
                row["metrics"] = {"wall_ms": wall}
                rows = luna.load_rows(write_run(Path(tmp), [row]))
                self.assertIsNone(rows[0]["wall_s"])
                self.assertIsNone(luna.aggregate(rows)["metrics"]["wall_s"]["total"])
        row = trace()
        row["timing"]["agent"] = {"start": 12, "end": 0}
        with tempfile.TemporaryDirectory() as tmp:
            self.assertIsNone(luna.load_rows(write_run(Path(tmp), [row]))[0]["wall_s"])

    def test_partial_receipts_use_independent_fields_and_keep_reported_subtotals(self):
        row = trace()
        usage = row["info"]["angel"]["usage"]
        usage["reported_attempts"]["input"] = 1
        usage.pop("reasoning")
        with tempfile.TemporaryDirectory() as tmp:
            rows = luna.load_rows(write_run(Path(tmp), [row]))
        self.assertIsNone(rows[0]["input_tokens"])
        self.assertEqual(rows[0]["output_tokens"], 10)
        self.assertIsNone(rows[0]["reasoning_tokens"])
        self.assertEqual(rows[0]["usage_reported_subtotals"]["input_tokens"], 100)
        self.assertEqual(rows[0]["usage_metric_coverage"]["input_tokens"], {"reported": 1, "attempts": 2, "complete": False})
        summary = luna.aggregate(rows)
        self.assertEqual(summary["metrics"]["output_tokens"]["total"], 10)
        self.assertEqual(summary["metrics"]["input_tokens"]["reported_subtotal"], 100)
        self.assertEqual(summary["usage_complete_attempts"], 0)

    def test_legacy_core_flag_alone_cannot_prove_complete_usage(self):
        row = trace()
        row["info"]["angel"]["usage"].pop("reported_attempts")
        with tempfile.TemporaryDirectory() as tmp:
            rows = luna.load_rows(write_run(Path(tmp), [row]))
        self.assertTrue(rows[0]["core"])
        self.assertIsNone(rows[0]["input_tokens"])
        self.assertEqual(rows[0]["usage_reported_subtotals"]["input_tokens"], 100)

    def test_repeated_failed_attempts_keep_their_time_and_usage(self):
        with tempfile.TemporaryDirectory() as tmp:
            rows = luna.load_rows(write_run(Path(tmp), [trace(score=0, wall=600), trace(wall=10)]))
        summary = luna.aggregate(rows)
        self.assertEqual(summary["duplicate_tasks"], {"py-a": 2})
        self.assertEqual((summary["attempts"], summary["unique_tasks"], summary["solved"]), (2, 1, 1))
        self.assertEqual(summary["metrics"]["wall_s"]["total"], 610)
        self.assertEqual(summary["metrics"]["input_tokens"]["total"], 200)
        self.assertEqual([(r["line"], r["attempt_index"]) for r in rows], [(1, 1), (1, 2)])

    def test_missing_scores_and_invalid_cache_measurements_remain_explicit(self):
        row = trace(score=True)
        row["info"]["angel"]["usage"]["cache_read"] = 101
        with tempfile.TemporaryDirectory() as tmp:
            rows = luna.load_rows(write_run(Path(tmp), [row], name=None))
        summary = luna.aggregate(rows)
        self.assertEqual((summary["unscored"], summary["unnamed_attempts"], summary["solved"]), (1, 1, 0))
        self.assertEqual(summary["cache_invalid_attempts"], 1)
        self.assertIsNone(summary["cache_ratio"])

    def test_valid_complete_metrics_match_recorded_values(self):
        row = trace()
        row["metrics"] = {"wall_ms": 2300, "hops": 3}
        with tempfile.TemporaryDirectory() as tmp:
            summary = luna.aggregate(luna.load_rows(write_run(Path(tmp), [row])))
        self.assertEqual(summary["metrics"]["wall_s"]["total"], 2.3)
        self.assertEqual(summary["metrics"]["hops"]["mean"], 3)
        self.assertEqual(summary["metrics"]["input_tokens"]["total"], 100)
        self.assertEqual(summary["metrics"]["output_tokens"]["total"], 10)
        self.assertEqual(summary["metrics"]["reasoning_tokens"]["total"], 0)
        self.assertEqual(summary["cache_ratio"], .2)

    def test_cli_custom_run_shows_unknowns_and_all_attempt_coverage(self):
        row = trace(score=None)
        row.pop("timing")
        row["info"]["angel"]["usage"] = {"input": 100}
        with tempfile.TemporaryDirectory() as tmp:
            run = write_run(Path(tmp), [row])
            result = subprocess.run([sys.executable, str(SCRIPT), str(run)], capture_output=True, text=True, timeout=5)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("attempts=1 scored=0 unscored=1", result.stdout)
        self.assertIn("usage_tasks=0/1", result.stdout)
        self.assertIn("— (0/1)", result.stdout)
        self.assertIn("hops / attempt", result.stdout)


if __name__ == "__main__":
    unittest.main()
