"""Loop replay keeps unscored logical samples using offline response fixtures."""
import contextlib
import copy
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import threading
import unittest
from unittest import mock
import urllib.error


DIRECTORY = Path(__file__).resolve().parents[2] / "scripts/bench/book-harness/decide"


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


producer = load("loop_producer_test", DIRECTORY / "loop_replay.py")
table = load("loop_producer_table_test", DIRECTORY / "loop_table.py")


def tool(name="shell", arguments='{"cmd":"fixture 1"}'):
    return {"function": {"name": name, "arguments": arguments}}


def reply(calls=None, content="done", prompt=100, completion=10):
    return {"choices": [{"message": {"tool_calls": calls, "content": content}}],
            "usage": {"prompt_tokens": prompt, "completion_tokens": completion}}


def job(cue="raw", sample=0):
    return ("js-fixture", cue, sample, [{"role": "user", "content": "fixture"}], [],
            [producer.signature("shell", '{"cmd":"fixture 9"}')])


def one(response):
    with mock.patch.object(producer, "call", return_value=response):
        return producer.one(job(), "offline-key")


class Response(io.BytesIO):
    def __init__(self, response):
        super().__init__(json.dumps(response).encode())


class LoopProducerTests(unittest.TestCase):
    def test_each_valid_move_keeps_existing_scoring_and_signature_semantics(self):
        cases = [([tool()], "repeat"), ([tool("write_file", '{"path":"a","content":"b"}')], "edit"),
                 ([tool("read_file", '{"path":"a"}')], "other_call"), ([], "answer")]
        for calls, move in cases:
            with self.subTest(move=move):
                row = one(reply(calls))
                self.assertEqual(row, {"task": "js-fixture", "cue": "raw", "i": 0, "loop_at": producer.LOOP_AT,
                                       "move": move, "broke": move != "repeat", "prompt_tokens": 100,
                                       "completion_tokens": 10, "next": [producer.signature(c["function"]["name"],
                                                                                           c["function"]["arguments"]) for c in calls][:2],
                                       "text": "done"})
        self.assertEqual(producer.signature("shell", '{"b":24,"a":13}'),
                         producer.signature("shell", '{"a":1,"b":2}'))

    def test_malformed_received_replies_are_unscored_class_only_samples(self):
        for response in (None, [], {}, {"choices": []}, {"choices": [{"message": {}}]},
                         reply(None, None), reply([{"function": {"name": "shell"}}]),
                         reply([tool(arguments=None)]), reply(content={})):
            with self.subTest(response=response):
                row = one(response)
                self.assertEqual((row["task"], row["cue"], row["i"], row["loop_at"]),
                                 ("js-fixture", "raw", 0, producer.LOOP_AT))
                self.assertEqual((row["error"], row["error_class"], row["reply_error"]), ("ValueError", "ValueError", True))
                self.assertNotIn("move", row)
                self.assertNotIn("broke", row)
                self.assertNotIn("next", row)
                json.dumps(row, allow_nan=False)

    def test_independent_usage_survives_malformed_choice_without_fake_break(self):
        row = one({"choices": [], "usage": {"prompt_tokens": 0, "completion_tokens": 12}})
        self.assertEqual((row["prompt_tokens"], row["completion_tokens"]), (0, 12))
        cell = table.aggregate([row])[0]
        self.assertEqual((cell["attempts"], cell["replies"], cell["errors"], cell["no_reply"], cell["broke"]),
                         (1, 0, 1, 1, 0))

    def test_missing_invalid_and_zero_usage_keep_observation_separate(self):
        for value in (None, True, -1, float("nan"), float("inf"), "12"):
            row = one(reply([], prompt=value, completion=value))
            self.assertEqual((row["move"], row["broke"]), ("answer", True))
            self.assertIsNone(row["prompt_tokens"])
            self.assertIsNone(row["completion_tokens"])
            json.dumps(row, allow_nan=False)
        row = one(reply([], prompt=0, completion=0))
        self.assertEqual((row["prompt_tokens"], row["completion_tokens"]), (0, 0))

    def test_deliberate_empty_values_stay_observed(self):
        for response in (reply([], None), reply(None, "")):
            row = one(response)
            self.assertEqual((row["move"], row["broke"], row["text"]), ("answer", True, ""))
        row = one(reply([tool(arguments="")], None))
        self.assertEqual((row["move"], row["text"]), ("other_call", ""))

    def test_network_failure_has_one_class_only_logical_outcome_without_retry(self):
        with mock.patch.object(producer, "call", side_effect=urllib.error.URLError("offline secret marker")) as call:
            row = producer.one(job(cue="bare", sample=3), "offline-key")
        self.assertEqual(call.call_count, 1)
        self.assertEqual(row, {"task": "js-fixture", "cue": "bare", "i": 3, "loop_at": producer.LOOP_AT,
                               "prompt_tokens": None, "completion_tokens": None,
                               "error": "URLError", "error_class": "URLError", "call_error": True})
        self.assertNotIn("offline secret marker", json.dumps(row))

    def test_control_signals_keep_existing_propagation(self):
        for signal in (KeyboardInterrupt(), SystemExit(17)):
            with mock.patch.object(producer, "call", side_effect=signal), self.assertRaises(type(signal)):
                producer.one(job(), "offline-key")

    def test_response_closes_and_original_single_call_settings_stay_exact(self):
        response = Response(reply())
        messages = [{"role": "user", "content": "fixture"}]
        tools = [{"fixture": True}]
        with mock.patch.object(producer.urllib.request, "urlopen", return_value=response) as opener:
            observed = producer.call("offline-key", messages, tools)
        self.assertTrue(response.closed)
        self.assertEqual(observed, reply())
        self.assertEqual(opener.call_count, 1)
        self.assertEqual(opener.call_args.kwargs["timeout"], 600)
        self.assertEqual(json.loads(opener.call_args.args[0].data), {"model": "deepseek-flash", "messages": messages,
            "tools": tools, "temperature": 0, "max_tokens": 8192, "thinking": {"type": "disabled"}, "stream": False})

    def test_read_error_and_http_error_close_owned_response_without_retry(self):
        response = Response(reply())
        with mock.patch.object(producer.urllib.request, "urlopen", return_value=response) as opener, \
                mock.patch.object(producer.json, "load", side_effect=ValueError("offline secret marker")):
            row = producer.one(job(), "offline-key")
        self.assertTrue(response.closed)
        self.assertEqual(opener.call_count, 1)
        self.assertEqual(row["error_class"], "ValueError")
        source = io.BytesIO(b"offline secret marker")
        failure = urllib.error.HTTPError("https://offline.invalid", 503, "fixture", {}, source)
        with mock.patch.object(producer.urllib.request, "urlopen", side_effect=failure) as opener:
            row = producer.one(job(), "offline-key")
        self.assertTrue(source.closed)
        self.assertEqual(opener.call_count, 1)
        self.assertEqual(row["error_class"], "HTTPError")
        self.assertNotIn("offline secret marker", json.dumps(row))

    def test_pool_persists_malformed_and_later_completed_sample(self):
        with tempfile.TemporaryDirectory() as tmp:
            source, out = Path(tmp) / "traces.jsonl", Path(tmp) / "rows.jsonl"
            source.write_text(json.dumps({"traces": [{"task": {"data": {"name": "js-fixture"}}, "tools": []}]}) + "\n")
            barrier = threading.Barrier(2, timeout=2)
            completed = []
            def fake_call(key, messages, tools):
                cue = messages[0]["content"]
                completed.append(cue)
                barrier.wait()
                return {"choices": []} if cue == "raw" else reply([])
            with mock.patch.object(producer, "CUES", ("raw", "bare")), \
                    mock.patch.object(producer, "key", return_value="offline-key"), \
                    mock.patch.object(producer, "decision", return_value=([], ["shell fixture"], producer.LOOP_AT)), \
                    mock.patch.object(producer, "cued", side_effect=lambda messages, cue, k: [{"role": "user", "content": cue}]), \
                    mock.patch.object(producer, "call", side_effect=fake_call), \
                    mock.patch.object(producer.sys, "argv", ["loop_replay.py", str(source), str(out), "1"]), \
                    contextlib.redirect_stdout(io.StringIO()):
                producer.main()
            rows = [json.loads(line) for line in out.read_text().splitlines()]
        self.assertCountEqual(completed, ["raw", "bare"])
        self.assertEqual([row["cue"] for row in rows], ["raw", "bare"])
        self.assertTrue(rows[0]["reply_error"])
        self.assertEqual((rows[1]["move"], rows[1]["broke"]), ("answer", True))
        self.assertTrue(all(row["loop_at"] == producer.LOOP_AT for row in rows))

    def test_all_cue_positions_preserve_capture_and_original_route_bytes(self):
        messages = [{"role": "user", "content": "start"}, {"role": "tool", "tool_call_id": "fixture",
                    "content": "a\nb\nc\nd\ne\nf\n⠇\n"}]
        original = copy.deepcopy(messages)
        for cue in producer.ALL_CUES:
            cued = producer.cued(messages, cue, 5)
            self.assertEqual(messages, original)
            self.assertEqual(cued[0], original[0])
            if cue in ("perturb", "user_stamp", "user_act", "sys_act", "user_dont", "user_intro", "user_page"):
                self.assertEqual(cued[1]["content"], "a\nb\nc\nd\ne\nf")
                self.assertEqual(cued[-1]["role"], "system" if cue == "sys_act" else "user")
            else:
                self.assertEqual(len(cued), 2)
        self.assertEqual(producer.cued(messages, "bare", 5)[1]["content"], "a\nb\nc\nd\ne\nf\n⠇⠁")
        self.assertEqual(producer.cued(messages, "warn_tail", 5)[1]["content"], "⛔⠇⠁ ×5\n…\nb\nc\nd\ne\nf")

    def test_existing_repeat_depth_selection_and_task_identity_stay_exact(self):
        nodes = []
        for number in range(1, 7):
            nodes += [{"message": {"role": "assistant", "tool_calls": [
                {"name": "shell", "arguments": json.dumps({"cmd": f"fixture {number}"})}]}},
                      {"message": {"role": "tool", "content": "fixture"}}]
        with mock.patch.object(producer, "LOOP_AT", 5):
            messages, repeated, depth = producer.decision({"nodes": nodes})
        self.assertEqual(depth, 5)
        self.assertEqual(messages, [node["message"] for node in nodes[:10]])
        self.assertEqual(repeated, [producer.signature("shell", '{"cmd":"fixture 1"}')])


if __name__ == "__main__":
    unittest.main()
