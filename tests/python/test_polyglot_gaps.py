"""Request-gap diagnostics retain observed actions and incomplete clocks."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock


SOURCE = Path(__file__).resolve().parents[2] / "scripts/bench/book-harness/gaps.py"


def load():
    spec = importlib.util.spec_from_file_location("gap_tests", SOURCE)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


gap = load()


def chat_call(name):
    return {"role": "assistant", "tool_calls": [{"function": {"name": name}}]}


class GapTests(unittest.TestCase):
    def test_import_does_not_consume_cli_inputs_or_print(self):
        with mock.patch.dict(os.environ, {}, clear=True), mock.patch.object(sys, "argv", ["fixture", "/missing"]), \
                mock.patch.object(Path, "open", side_effect=AssertionError("input read")), \
                contextlib.redirect_stdout(io.StringIO()) as out:
            load()
        self.assertEqual(out.getvalue(), "")

    def test_responses_parallel_batch_uses_first_observed_call(self):
        rows = [{"t": 1, "body": {"input": [{"role": "user", "content": "task"}]}},
                {"t": 2, "body": {"input": [{"role": "user", "content": "task"},
                  {"type": "function_call", "name": "shell"},
                  {"type": "function_call", "name": "read_file"},
                  {"type": "function_call_output", "output": "result"}]}}]
        self.assertEqual(gap.analyze(rows)["groups"], {"shell": [1000]})

    def test_chat_answer_does_not_reuse_older_tool_action(self):
        rows = [{"body": {"messages": [chat_call("shell")]}},
                {"body": {"messages": [chat_call("shell"), {"role": "assistant", "content": "answer"}]}}]
        self.assertEqual(gap.action_of(rows, 0), "final")

    def test_unchanged_history_is_not_a_new_observed_reply(self):
        for key, history in (("messages", [chat_call("shell")]),
                             ("input", [{"type": "function_call", "name": "shell"}])):
            with self.subTest(wire=key):
                rows = [{"body": {key: history}}, {"body": {key: history + [{"role": "user", "content": "retry"}]}}]
                self.assertEqual(gap.action_of(rows, 0), gap.UNKNOWN)

    def test_missing_and_malformed_actions_remain_unknown(self):
        bodies = [{}, {"input": [None]}, {"messages": [{"role": "assistant"}]},
                  {"input": [{"type": "function_call", "name": 3}]},
                  {"messages": [{"role": "assistant", "tool_calls": "shell"}]}]
        for body in bodies:
            self.assertEqual(gap.action_of([{}, {"body": body}], 0), gap.UNKNOWN)
        for action in ("shell", {}, [None], [""]):
            self.assertEqual(gap.action_of([{"action": action}, {}], 0), gap.UNKNOWN)

    def test_explicit_stub_and_responses_answers_remain_final(self):
        for action in (None, []):
            self.assertEqual(gap.action_of([{"action": action}, {}], 0), "final")
        self.assertEqual(gap.action_of([{}, {"body": {"input": [
            {"role": "assistant", "content": [{"type": "output_text", "text": "answer"}]}]}}], 0), "final")

    def test_valid_zero_and_partial_clock_subtotals_are_distinct(self):
        rows = [{"t": 0, "action": ["shell", {}]}, {"t": 0, "action": ["shell", {}]}, {}]
        result = gap.analyze(rows)
        self.assertEqual(result["groups"], {"shell": [0, None]})
        self.assertIsNone(result["first_to_last_seconds"])
        rendered = gap.render("fixture", result)
        self.assertIn("clocked=1/2; reported_sum=0.0s", rendered)
        self.assertIn("median=      —", rendered)
        self.assertIn("sum=     —", rendered)
        self.assertIn("response latency", rendered)

    def test_invalid_regressing_and_overflowing_clocks_cannot_report_zero(self):
        for value in (None, True, "1", -1, float("nan"), float("inf")):
            result = gap.analyze([{"t": value}, {"t": 2}])
            self.assertEqual(result["groups"], {gap.UNKNOWN: [None]})
            self.assertIn("reported_sum=—", gap.render("fixture", result))
            json.dumps(result, allow_nan=False)
        self.assertEqual(gap.analyze([{"t": 2}, {"t": 1}])["groups"], {gap.UNKNOWN: [None]})
        overflow = gap.analyze([{"t": 0}, {"t": 1e308}])
        self.assertEqual(overflow["groups"], {gap.UNKNOWN: [None]})
        json.dumps(overflow, allow_nan=False)

    def test_empty_and_single_capture_have_no_measured_duration(self):
        for rows in ([], [{"t": 1}]):
            result = gap.analyze(rows)
            self.assertIsNone(result["first_to_last_seconds"])
            self.assertEqual(result["groups"], {})
            self.assertIn("— first→last", gap.render("fixture", result))

    def test_cli_audit_is_import_safe_and_preserves_source(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "capture.jsonl"
            data = b'{"t":1,"action":["shell",{}]}\n\n{"t":2}\n'
            path.write_bytes(data)
            with contextlib.redirect_stdout(io.StringIO()) as out:
                self.assertEqual(gap.main([str(path), "--audit-json"]), 0)
            audit = json.loads(out.getvalue())
            self.assertEqual(audit[0]["groups"], {"shell": [1000]})
            self.assertEqual(path.read_bytes(), data)

    def test_malformed_later_file_cannot_publish_partial_stdout(self):
        with tempfile.TemporaryDirectory() as tmp:
            valid, invalid = Path(tmp) / "valid", Path(tmp) / "invalid"
            valid.write_text('{"t":1}\n{"t":2}\n')
            for malformed in ("[1,2]\n", "invalid JSON\n"):
                invalid.write_text(malformed)
                with contextlib.redirect_stdout(io.StringIO()) as out, contextlib.redirect_stderr(io.StringIO()):
                    with self.assertRaises(SystemExit) as error:
                        gap.main([str(valid), str(invalid)])
                self.assertEqual(error.exception.code, 2)
                self.assertEqual(out.getvalue(), "")


if __name__ == "__main__":
    unittest.main()
