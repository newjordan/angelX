#!/usr/bin/env python3
"""Offline regression checks for benchmark evidence coverage."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


spec = importlib.util.spec_from_file_location(
    "sloptomizer_summary", Path(__file__).with_name("summarize_sloptomizer.py")
)
summary = importlib.util.module_from_spec(spec)
spec.loader.exec_module(summary)


def trace_fixture():
    return {
        "nodes": [
            {"sampled": False, "message": {"role": "user", "content": "⚠⡫⠁ context"}},
            {"sampled": True, "parent": 0,
             "message": {"role": "assistant", "content": "⚠⡫⠃ quoted, not an input card"}},
            {"sampled": False, "message": {"role": "user", "content": "⚠⡫⠛ orphan, never sent"}},
        ],
        "calls": [{"node": 1, "model": "glm-5.3-flash", "sampling": {
            "temperature": 0.0, "reasoning_effort": "low", "max_tokens": 8192,
        }}],
        "info": {
            "angel": {"timing": {"model_calls": 1}, "tools": [], "usage": {
                "attempts": 1, "input": 100, "cache_read": 60, "output": 20, "reasoning": 10,
                "reported_attempts": {"input": 1, "cache_read": 1, "output": 1, "reasoning": 1},
                "cache_convention_attempts": {"included": 1},
                "reasoning_convention_attempts": {"included": 1}, "core_complete": True,
            }},
            "heldout_verification": {"integrity_pass": True},
            "agent_exit": {"exit_code": 0, "timed_out": False},
        },
        "rewards": {"technical_outcome": {"score": 1}},
        "is_completed": True, "ok": True,
    }


class DeliveryTests(unittest.TestCase):
    def test_only_actual_call_ancestors_count_as_exposure(self):
        got = summary.delivery(trace_fixture())
        self.assertTrue(got["input_call_coverage_complete"])
        self.assertEqual(got["unique_cards"], 1)
        self.assertEqual(got["card_routes"], {"context": 1})

    def test_broken_ancestry_or_missing_calls_is_unknown_not_zero(self):
        variants = []
        missing = trace_fixture()
        missing.pop("calls")
        variants.append(missing)
        cycle = trace_fixture()
        cycle["nodes"][0]["parent"] = 1
        variants.append(cycle)
        unobserved_call = trace_fixture()
        unobserved_call["info"]["angel"]["timing"]["model_calls"] = 2
        variants.append(unobserved_call)
        for trace in variants:
            with self.subTest(trace=variants.index(trace)):
                got = summary.delivery(trace)
                self.assertFalse(got["input_call_coverage_complete"])
                self.assertIsNone(got["unique_cards"])

    def test_token_derivation_requires_complete_included_receipts(self):
        trace = trace_fixture()
        self.assertEqual(summary.delivery(trace)["derived_uncached_input_tokens"], 40)
        self.assertTrue(summary.delivery(trace)["output_includes_reasoning"])
        trace["info"]["angel"]["usage"]["reported_attempts"]["cache_read"] = 0
        self.assertIsNone(summary.delivery(trace)["derived_uncached_input_tokens"])
        trace["info"]["angel"]["usage"]["reported_attempts"]["cache_read"] = 1
        trace["info"]["angel"]["usage"]["cache_convention_attempts"] = {"unknown": 1}
        self.assertIsNone(summary.delivery(trace)["derived_uncached_input_tokens"])


class CohortTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        manifest = {k: "pin" for k in summary.CONTRACT}
        manifest.update({
            "order": list(summary.ORDER), "mode": "execute", "model": "glm-5.3-flash",
            "effort": "low", "max_tokens": 8192, "task_names": ["task"], "attempts": 1,
            "binary": {"toolchain": {"profile": "dev"}, "video_decode": False},
            "adapter_files": {}, "fixture_files": {}, "gold_files": {},
        })
        (self.root / "manifest.json").write_text(json.dumps(manifest))
        for name in summary.ORDER:
            cell = self.root / name
            (cell / "run").mkdir(parents=True)
            cm = {**manifest, "exit_code": 0, "harness_env": {
                "ANGEL_SLOPTOMIZER_LIVE": "1" if name.endswith("-on") else "0",
                "ANGEL_WIRE_LOG_DIR": str(cell / "run/wire"), "COMMON": "held equal",
            }}
            (cell / "manifest.json").write_text(json.dumps(cm))
            trace = trace_fixture()
            if name.endswith("-off"):
                trace["nodes"][0]["message"]["content"] = "ordinary input"
            self.put_trace(name, trace)

    def put_trace(self, name, trace):
        (self.root / name / "run/traces.jsonl").write_text(json.dumps({
            "task": {"data": {"name": "task"}}, "traces": [trace],
        }) + "\n")

    def test_failed_timed_out_attempt_stays_in_denominator(self):
        trace = copy.deepcopy(trace_fixture())
        trace["rewards"]["technical_outcome"]["score"] = 0
        trace["info"]["agent_exit"] = {"exit_code": 124, "timed_out": True}
        self.put_trace("r1-on", trace)
        got = summary.summarize(self.root)
        self.assertTrue(got["complete"])
        self.assertEqual(got["arms"]["on"]["scored"], 2)
        self.assertEqual(got["arms"]["on"]["solved"], 1)
        self.assertEqual(got["arms"]["on"]["outcomes"]["timed_out"], 1)
        self.assertEqual(got["comparisons"][0]["solve_losses"], ["task"])
        self.assertTrue(got["comparisons"][0]["only_expected_contract_differences"])

    def test_sampling_drift_is_reported_without_removing_attempt(self):
        trace = trace_fixture()
        trace["calls"][0]["sampling"]["reasoning_effort"] = "high"
        self.put_trace("r1-on", trace)
        got = summary.summarize(self.root)
        self.assertFalse(got["complete"])
        self.assertTrue(any("observed model/sampling" in issue for issue in got["issues"]))
        self.assertEqual(got["arms"]["on"]["tasks"], 2)

    def test_missing_grading_integrity_is_not_validated_as_success(self):
        trace = trace_fixture()
        trace["info"].pop("heldout_verification")
        self.put_trace("r1-on", trace)
        got = summary.summarize(self.root)
        self.assertFalse(got["complete"])
        self.assertEqual(got["arms"]["on"]["outcomes"]["integrity_unknown"], 1)

    def test_card_delivered_to_disabled_arm_invalidates_the_comparison(self):
        self.put_trace("r1-off", trace_fixture())
        got = summary.summarize(self.root)
        self.assertFalse(got["complete"])
        self.assertTrue(any("disabled arm" in issue for issue in got["issues"]))
        self.assertEqual(got["arms"]["off"]["tasks"], 2)


if __name__ == "__main__":
    unittest.main()
