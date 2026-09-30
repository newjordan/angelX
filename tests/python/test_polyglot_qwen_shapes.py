"""Offline response graph integrity checks for the Qwen character-size audit."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("qwen_shapes", ROOT / "scripts/analyze-qwen-traces.py")
shapes = importlib.util.module_from_spec(spec)
spec.loader.exec_module(shapes)


def trace(index=1, parent=0):
    return {"nodes": [{"message": {"role": "system", "content": "old reasoning"}},
                      {"parent": parent, "message": {"role": "assistant", "content": "new",
                         "tool_calls": [{"name": "read", "arguments": "{}"}]}}],
            "calls": [{"node": index, "sampling": {"temperature": 0}, "time": {"start": 0, "end": 1}}]}


class ReferenceTests(unittest.TestCase):
    def test_omitted_and_null_responses_are_unobserved(self):
        data = trace()
        data["calls"] = [{}, {"node": None}]
        row, settings, tools = shapes.trace_shapes(data)
        self.assertEqual(row["attempted_calls"], 2)
        self.assertEqual(row["missing_response_calls"], 2)
        self.assertEqual(row["unobserved_response_calls"], 2)
        self.assertEqual(row["calls"], [])
        self.assertEqual(sum(settings.values()), 2)
        self.assertEqual(tools, {})

    def test_invalid_response_indices_cannot_select_replayed_nodes(self):
        for index in (-1, True, False, 2, 1.5, "1"):
            with self.subTest(index=index):
                row, _, tools = shapes.trace_shapes(trace(index=index))
                self.assertEqual(row["invalid_response_calls"], 1)
                self.assertEqual(row["unobserved_response_calls"], 1)
                self.assertEqual(row["calls"], [])
                self.assertEqual(tools, {})

    def test_response_reference_must_point_to_an_assistant_message(self):
        for message in (None, "raw", {"role": "user", "content": "replayed input"}):
            with self.subTest(message=message):
                data = trace()
                data["nodes"][1]["message"] = message
                row, _, _ = shapes.trace_shapes(data)
                self.assertEqual(row["invalid_response_calls"], 1)
                self.assertEqual(row["calls"], [])
        data = trace()
        data["nodes"][1] = None
        self.assertEqual(shapes.trace_shapes(data)[0]["invalid_response_calls"], 1)

    def test_invalid_ancestor_references_and_cycles_are_audited(self):
        for parent in (-1, True, 2, "0", 1):
            with self.subTest(parent=parent):
                row, _, tools = shapes.trace_shapes(trace(parent=parent))
                self.assertEqual(row["invalid_ancestry_calls"], 1)
                self.assertEqual(row["unobserved_response_calls"], 1)
                self.assertEqual(row["calls"], [])
                self.assertEqual(tools, {})
                with self.assertRaises(ValueError):
                    shapes.ancestors(trace(parent=parent)["nodes"], parent)

    def test_malformed_ancestor_messages_are_excluded_from_generation(self):
        for node in (None, {"message": None}, {"message": "old response"}):
            with self.subTest(node=node):
                data = trace()
                data["nodes"][0] = node
                row, _, tools = shapes.trace_shapes(data)
                self.assertEqual(row["invalid_ancestry_calls"], 1)
                self.assertEqual(row["calls"], [])
                self.assertEqual(tools, {})

    def test_valid_root_response_needs_no_ancestors(self):
        row, _, tools = shapes.trace_shapes(trace(parent=None))
        self.assertEqual(row["calls"][0]["generation_chars"], 5)
        self.assertEqual(row["calls"][0]["request_message_chars"], 0)
        self.assertEqual(tools, {"read": 1})
        self.assertEqual(row["unobserved_response_calls"], 0)

    def test_mixed_cohort_preserves_valid_metrics_and_reference_coverage(self):
        valid, bad, omitted = trace(), trace(index=-1), trace(index=None)
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "traces.jsonl"
            path.write_text("\n".join(json.dumps({"task": {"data": {"name": f"js-{index}"}}, "traces": [data]})
                                      for index, data in enumerate((valid, bad, omitted))) + "\n")
            result = shapes.read_cohort([path])
        summary = result["summary"]
        self.assertEqual((summary["tasks"], summary["attempted_calls"], summary["observed_response_calls"]), (3, 3, 1))
        self.assertEqual((summary["missing_response_calls"], summary["invalid_response_calls"], summary["invalid_ancestry_calls"]), (1, 1, 0))
        self.assertEqual(summary["metrics"]["generation_chars"]["sum"], 5)
        self.assertEqual(summary["metrics"]["wall_s"]["sum"], 1)
        self.assertEqual(result["tool_call_counts"], {"read": 1})
        self.assertEqual(sum(result["sampling_counts"].values()), 3)


if __name__ == "__main__":
    unittest.main()
