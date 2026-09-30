"""Offline checks for cohort integrity and honest pilot telemetry."""
import importlib.util
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


def load_script(name):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), ROOT / "scripts" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


runner = load_script("run-polyglot-loop")
summary = load_script("summarize-polyglot-loop")
history = load_script("analyze-polyglot-history")
shapes = load_script("analyze-qwen-traces")


class CohortTests(unittest.TestCase):
    def test_selection_balanced_stable_and_independent_of_outcomes(self):
        rows = [{"name": f"{lang}-{idx}", "prompt": "original", "reward": 0}
                for lang in runner.LANGUAGES for idx in range(5)]
        selected = runner.select_tasks(rows, 3, [])
        changed = [{**row, "prompt": "different", "reward": 1} for row in reversed(rows)]
        again = runner.select_tasks(changed, 3, [])
        self.assertEqual([r["name"] for r in selected], [r["name"] for r in again])
        self.assertEqual(len(selected), 12)
        for lang in runner.LANGUAGES:
            self.assertEqual(sum(r["name"].startswith(lang + "-") for r in selected), 3)
        self.assertEqual(selected[0]["prompt"], "original")

    def test_explicit_selection_rejects_missing_or_duplicate_tasks(self):
        rows = [{"name": "js-a"}, {"name": "py-a"}]
        for names in (["missing"], ["js-a", "js-a"]):
            with self.subTest(names=names), self.assertRaises(ValueError):
                runner.select_tasks(rows, 3, names)
        self.assertEqual(runner.select_tasks(rows, 3, ["py-a", "js-a"]), list(reversed(rows)))

    def test_fixture_snapshot_is_confined_and_independent(self):
        with tempfile.TemporaryDirectory() as tmp:
            bench, output = Path(tmp) / "old", Path(tmp) / "new"
            source = bench / "fixtures/js-a"
            source.mkdir(parents=True)
            (source / "solution.js").write_text("original")
            runner.snapshot_fixtures(bench, output, [{"name": "js-a", "fixture": "fixtures/js-a"}])
            target = output / "fixtures/js-a/solution.js"
            self.assertTrue(target.resolve().is_relative_to(output))
            target.write_text("modified")
            self.assertEqual((source / "solution.js").read_text(), "original")
            for fixture in ("../old/fixtures/js-a", str(source), "not-fixtures/js-a"):
                with self.subTest(fixture=fixture), self.assertRaises(ValueError):
                    runner.snapshot_fixtures(bench, output, [{"name": "js-a", "fixture": fixture}])

    def test_environment_does_not_inherit_host_credentials_or_switches(self):
        env = runner.evaluator_environment(Path("/bench"), Path("/snapshot"), "gpt-6-luna",
                                           "medium", 600, Path("/private/auth"))
        self.assertEqual(env["OPENAI_API_KEY"], "unused-luna-direct")
        self.assertNotIn("ANGEL_YOLO", env)
        self.assertNotIn("ANGEL_RESOURCE_DIR", env)
        self.assertNotIn("META_API_KEY", env)
        self.assertEqual(env["PYTHONPATH"], "/snapshot")


class SummaryTests(unittest.TestCase):
    def test_missing_telemetry_remains_unknown(self):
        row = summary.summarize_trace({"task": {"data": {"name": "js-a"}}}, {})
        self.assertIsNone(row["score"])
        for key in summary.METRICS:
            self.assertIsNone(row[key], key)
        result = summary.aggregate([row])
        self.assertEqual(result["scored"], 0)
        self.assertEqual(result["solved"], 0)
        self.assertIsNone(result["cache_hit_ratio"])
        self.assertIsNone(result["metrics"]["input_tokens"]["sum"])

    def test_zero_is_reported_and_agent_timing_is_distinct_from_verifier(self):
        trace = {
            "timing": {"agent": {"start": 10, "end": 12}},
            "rewards": {"technical_outcome": {"score": 1}},
            "info": {"angel": {"hops": 0, "usage": {
                "input": 100, "cache_read": 0, "output": 0, "core_complete": True,
            }}, "heldout_verification": {"verifier_elapsed_ms": 500}},
        }
        row = summary.summarize_trace({"task": {"data": {"name": "js-a"}}}, trace)
        result = summary.aggregate([row])
        self.assertEqual(row["task_wall_s"], 2)
        self.assertEqual(row["verifier_wall_s"], 0.5)
        self.assertEqual(row["hops"], 0)
        self.assertEqual(result["metrics"]["output_tokens"], {"reported": 1, "sum": 0, "median": 0})
        self.assertEqual(result["cache_hit_ratio"], 0)

    def test_partial_coverage_and_contract_mismatch_remain_visible(self):
        row = summary.summarize_trace({"task": {"data": {"name": "js-a"}}}, {})
        known = {**row, "task_wall_s": 2, "input_tokens": 100, "cached_input_tokens": 20}
        result = summary.aggregate([row, known])
        self.assertEqual(result["metrics"]["input_tokens"]["reported"], 1)
        self.assertEqual(result["cache_hit_reported_tasks"], 1)
        baseline = {"path": "a", "manifest": {"effort": "medium"}, "rows": [known]}
        candidate = {"path": "b", "manifest": {"effort": "high"},
                     "rows": [{**known, "task_wall_s": 1}]}
        comparison = summary.paired(baseline, candidate)
        self.assertEqual(comparison["contract_mismatches"], ["effort"])
        self.assertEqual(comparison["deltas_candidate_minus_baseline"][0]["task_wall_s"], -1)
        self.assertIsNone(comparison["deltas_candidate_minus_baseline"][0]["model_s"])


class HistoricalTraceTests(unittest.TestCase):
    def test_proxy_usage_is_uncached_plus_cached_and_absence_stays_unknown(self):
        trace = {"calls": [{"usage": {"prompt_tokens": 30, "cached_input_tokens": 70,
                                      "completion_tokens": 0}}]}
        row = history.extract_row({}, trace, "run")
        self.assertEqual((row["input"], row["uncached_input"], row["cached_input"], row["output"]), (100, 30, 70, 0))
        unknown = history.extract_row({}, {"calls": [{"usage": None}]}, "run")
        self.assertIsNone(unknown["input"])
        self.assertIsNone(unknown["output"])
        self.assertEqual(unknown["usage_calls_reported"], 0)

    def test_p90_interpolates_and_unknown_cache_has_no_percent_suffix(self):
        self.assertEqual(history.percentile([0, 10], .9), 9)
        row = history.extract_row({}, {}, "run")
        cell = {"id": "runs/qwen/angelx/test", "model_family": "qwen", "harness": "angelx",
                "contract": {}, "summary": history.aggregate([row])}
        rendered = history.markdown({"primary": [cell], "pairings": []})
        self.assertNotIn("—%", rendered)
        self.assertIn("Same recorded settings", rendered)

    def test_response_shapes_count_produced_calls_not_replayed_nodes(self):
        trace = {"nodes": [
            {"message": {"role": "system", "content": "abc"}},
            {"parent": 0, "message": {"role": "assistant", "reasoning_content": "prior"}},
            {"parent": 1, "message": {"role": "tool", "content": "result"}},
            {"parent": 2, "message": {"role": "assistant", "content": "ok", "reasoning_content": "new",
                                      "tool_calls": [{"name": "read", "arguments": "{}"}]}}
        ], "calls": [{"node": 3, "time": {"start": 2, "end": 4}}, {"error": "missing response"}]}
        result, _, tools = shapes.trace_shapes(trace)
        self.assertEqual(result["attempted_calls"], 2)
        self.assertEqual(result["unobserved_response_calls"], 1)
        self.assertEqual(len(result["calls"]), 1)
        call = result["calls"][0]
        self.assertEqual(call["history_reasoning_chars"], 5)
        self.assertEqual(call["reasoning_chars"], 3)
        self.assertEqual(call["generation_chars"], 7)
        self.assertEqual(tools, {"read": 1})
        with self.assertRaises(ValueError):
            shapes.ancestors([{"parent": 0, "message": {}}], 0)


if __name__ == "__main__":
    unittest.main()
