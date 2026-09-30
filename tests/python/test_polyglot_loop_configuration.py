"""Replay labels and source files are validated before any provider work."""

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


SCRIPT = Path(__file__).resolve().parents[2] / "scripts/bench/book-harness/decide/loop_replay.py"


class LoopConfigurationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="angel-loop-config-", dir="/tmp")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / "trace.jsonl"
        self.output = self.root / "replay.jsonl"
        messages = []
        for i in range(2):
            messages.extend([{"role": "assistant", "content": None,
                              "tool_calls": [{"id": f"call{i}", "name": "shell", "arguments": '{"command":"same"}'}]},
                             {"role": "tool", "content": "same output", "tool_call_id": f"call{i}"}])
        trace = {"task": {"data": {"name": "fixture"}},
                 "nodes": [{"message": message} for message in messages],
                 "tools": [{"name": "shell", "parameters": {"type": "object"}}]}
        self.original = json.dumps({"traces": [trace]}) + "\n"
        self.source.write_text(self.original)
        spec = importlib.util.spec_from_file_location("loop_config_fixture", SCRIPT)
        self.module = importlib.util.module_from_spec(spec)
        with mock.patch.dict(os.environ, {"LOOP_AT": "1", "CUES": "raw,bare"}):
            spec.loader.exec_module(self.module)

    def invoke(self, count="1"):
        with mock.patch.object(sys, "argv", [str(SCRIPT), str(self.source), str(self.output), count]), \
                contextlib.redirect_stdout(io.StringIO()):
            self.module.main()

    def assert_refused(self, count="1"):
        with mock.patch.object(self.module, "key", side_effect=AssertionError("credentials read")) as key, \
                mock.patch.object(self.module, "call", side_effect=AssertionError("provider called")) as call:
            with self.assertRaises(SystemExit) as failure:
                self.invoke(count)
            self.assertNotEqual(failure.exception.code, 0)
            key.assert_not_called()
            call.assert_not_called()
        self.assertEqual(self.source.read_text(), self.original)

    def test_unknown_cue_refuses_instead_of_labelling_raw_payload(self):
        self.module.CUES = ("raw", "warn_typo")
        self.assert_refused()
        self.assertFalse(self.output.exists())

    def test_duplicate_and_empty_cues_refuse_before_changing_existing_output(self):
        self.output.write_text("existing replay evidence\n")
        for cues in (("raw", "raw"), ("raw", ""), ()):
            with self.subTest(cues=cues):
                self.module.CUES = cues
                self.assert_refused()
                self.assertEqual(self.output.read_text(), "existing replay evidence\n")

    def test_nonpositive_samples_and_loop_depth_refuse_before_credentials(self):
        for count in ("0", "-1"):
            with self.subTest(count=count):
                self.assert_refused(count)
        for depth in (0, -1):
            with self.subTest(depth=depth):
                self.module.LOOP_AT = depth
                self.assert_refused()
        self.assertFalse(self.output.exists())

    def test_output_cannot_append_receipts_to_its_trace_source(self):
        for mode in ("same", "symlink", "hardlink"):
            with self.subTest(mode=mode):
                self.output = self.root / (mode + ".jsonl")
                if mode == "same":
                    self.output = self.source
                elif mode == "symlink":
                    self.output.symlink_to(self.source)
                else:
                    os.link(self.source, self.output)
                self.assert_refused()

    def test_positive_selection_keeps_ordered_logical_sample_labels(self):
        reply = {"choices": [{"message": {"content": "observed answer"}, "finish_reason": "stop"}],
                 "usage": {"prompt_tokens": 12, "completion_tokens": 2}}
        with mock.patch.object(self.module, "key", return_value="fixture-key") as key, \
                mock.patch.object(self.module, "call", return_value=reply) as call:
            self.invoke("2")
        self.assertEqual(key.call_count, 1)
        self.assertEqual(call.call_count, 4)
        rows = [json.loads(line) for line in self.output.read_text().splitlines()]
        self.assertEqual([(row["cue"], row["i"], row["loop_at"]) for row in rows],
                         [("raw", 0, 1), ("raw", 1, 1), ("bare", 0, 1), ("bare", 1, 1)])
        self.assertTrue(all(row["move"] == "answer" and row["broke"] for row in rows))
        self.assertEqual(self.source.read_text(), self.original)


if __name__ == "__main__":
    unittest.main()
