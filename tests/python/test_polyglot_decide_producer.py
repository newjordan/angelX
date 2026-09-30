"""Decision sample retention uses in-memory replies and isolated output only."""
import contextlib
import copy
import http.client
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


HARNESS = Path(__file__).resolve().parents[2] / "scripts/bench/book-harness"


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


producer = load("decision_producer_test", HARNESS / "decide/decide.py")
table = load("decision_producer_table_test", HARNESS / "decide/decide_table.py")


def reply(calls=None, content="done", usage=None):
    return {"choices": [{"message": {"tool_calls": calls, "content": content}, "finish_reason": "stop"}],
            "usage": usage}


def tool(name="shell", arguments="make test"):
    return {"function": {"name": name, "arguments": arguments}}


def sample(response, scenario="finish", secs=0.25):
    with mock.patch.object(producer, "call", return_value=(response, secs, None)):
        return producer.one("deepseek-flash", scenario, "intro", 0, {"messages": []}, 8192)


class Response(io.BytesIO):
    def __init__(self, response):
        super().__init__(json.dumps(response).encode())
        self.status = 200


def http_error(status):
    body = io.BytesIO(b"offline fixture error")
    return urllib.error.HTTPError("https://offline.invalid", status, "fixture", {}, body), body


class DecisionProducerTests(unittest.TestCase):
    def test_complete_ordinary_projection_and_scoring_stay_unchanged(self):
        scenarios = [
            ("finish", [], "done", True),
            ("finish", [tool()], "", False),
            ("batch", [tool("read_file", "a"), tool("read_file", "b")], "", True),
            ("batch2", [tool("code_mode", "fixture")], "", True),
            ("batch", [tool()], "", False),
            ("untested", [tool()], "", True),
            ("untested", [], "I have not run the tests", True),
        ]
        for scenario, calls, text, matched in scenarios:
            with self.subTest(scenario=scenario, calls=calls):
                row = sample(reply(calls, text, {"prompt_tokens": 12, "completion_tokens": 3,
                                               "completion_tokens_details": {"reasoning_tokens": 1}}), scenario)
                self.assertEqual(row, {"model": "deepseek-flash", "scenario": scenario, "arm": "intro", "i": 0,
                                       "secs": 0.25, "error": None, "match": matched,
                                       "calls": [call["function"]["name"] for call in calls],
                                       "args": [call["function"]["arguments"] for call in calls],
                                       "text": text, "finish": "stop", "prompt": 12, "completion": 3, "reasoning": 1})

    def test_content_parts_null_calls_and_measured_zeros_are_valid(self):
        row = sample(reply(None, [{"type": "text", "text": "done"}, {"type": "image_url", "image_url": {}}],
                           {"prompt_tokens": 0, "completion_tokens": 0,
                            "completion_tokens_details": {"reasoning_tokens": 0}}), secs=0)
        self.assertEqual((row["text"], row["calls"], row["match"]), ("done", [], True))
        self.assertEqual((row["secs"], row["prompt"], row["completion"], row["reasoning"]), (0, 0, 0, 0))
        missing_usage = sample(reply())
        self.assertTrue(missing_usage["match"])
        self.assertTrue(all(missing_usage[key] is None for key in ("prompt", "completion", "reasoning")))

    def test_malformed_reply_shapes_are_explicit_unscored_samples(self):
        malformed = [None, [], {}, {"choices": []}, {"choices": {}}, {"choices": [None]},
                     {"choices": [{}]}, {"choices": [{"message": []}]}, {"choices": [{"message": {}}]},
                     {"choices": [{"message": {"role": "user", "content": "done"}}]},
                     reply("not a list"), reply([None]), reply([{}]), reply([{"function": {}}]),
                     reply([tool(name="")]), reply([tool(arguments=None)]),
                     reply(content={}), reply(content=[None]), reply(content=[{"text": None}]),
                     {"choices": [{"message": {}, "finish_reason": 1}]}]
        for response in malformed:
            with self.subTest(response=response):
                row = sample(response)
                self.assertEqual((row["error"], row["error_class"], row["reply_error"]), ("ValueError", "ValueError", True))
                self.assertNotIn("match", row)
                self.assertNotIn("calls", row)
                self.assertNotIn("args", row)
                self.assertEqual(row["secs"], 0.25)
                json.dumps(row, allow_nan=False)

    def test_malformed_choice_keeps_independent_observed_usage(self):
        row = sample({"choices": [], "usage": {"prompt_tokens": 15, "completion_tokens": 0}})
        self.assertEqual((row["prompt"], row["completion"], row["reasoning"]), (15, 0, None))
        cell = table.aggregate([row])[0]
        self.assertEqual((cell["attempts"], cell["no_reply"], cell["errors"], cell["n"]), (1, 1, 1, 0))

    def test_missing_observations_are_distinct_from_deliberate_empty_values(self):
        for response in (reply(None, None), reply([{"function": {"name": "shell"}}], "")):
            row = sample(response)
            self.assertTrue(row["reply_error"])
            self.assertNotIn("match", row)
        for response in (reply(None, ""), reply([], None)):
            row = sample(response)
            self.assertTrue(row["match"])
            self.assertEqual((row["calls"], row["text"]), ([], ""))
        row = sample(reply([tool("run_tests", "")], None), scenario="untested")
        self.assertTrue(row["match"])
        self.assertEqual(row["args"], [""])

    def test_invalid_metrics_stay_unknown_without_discarding_observed_move(self):
        for value in (True, -1, float("nan"), float("inf"), "12"):
            row = sample(reply(usage={"prompt_tokens": value, "completion_tokens": value,
                                      "completion_tokens_details": {"reasoning_tokens": value}}), secs=value)
            self.assertTrue(row["match"])
            self.assertTrue(all(row[key] is None for key in ("secs", "prompt", "completion", "reasoning")))
            json.dumps(row, allow_nan=False)
        row = sample(reply(usage="invalid usage"))
        self.assertTrue(row["match"])
        self.assertIsNone(row["prompt"])

    def test_worker_failure_keeps_logical_identity_without_error_text(self):
        with mock.patch.object(producer, "call", side_effect=KeyError("offline secret marker")):
            row = producer.one("muse", "finish", "bare", 7, {"messages": []}, 16)
        self.assertEqual(row, {"model": "muse", "scenario": "finish", "arm": "bare", "i": 7,
                               "secs": None, "error": "KeyError", "error_class": "KeyError", "call_error": True})
        self.assertNotIn("offline secret marker", json.dumps(row))

    def test_control_signals_keep_existing_propagation(self):
        for signal in (KeyboardInterrupt(), SystemExit(17)):
            with self.subTest(signal=type(signal).__name__):
                with mock.patch.object(producer, "call", side_effect=signal):
                    with self.assertRaises(type(signal)):
                        producer.one("muse", "finish", "bare", 0, {"messages": []}, 16)

    def test_pool_persists_malformed_and_later_completed_samples(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "out").mkdir()
            barrier = threading.Barrier(2, timeout=2)
            completed = []
            def fake_call(model, body, max_tokens):
                completed.append(body["arm"])
                barrier.wait()
                return ({"choices": []} if body["arm"] == "intro" else reply(), 0.25, None)
            with mock.patch.object(producer, "ROOT", str(root)), mock.patch.object(producer, "ARMS", ("intro", "bare")), \
                    mock.patch.object(producer, "decision_body", side_effect=lambda scenario, arm: {"messages": [], "arm": arm}), \
                    mock.patch.object(producer, "call", side_effect=fake_call), \
                    mock.patch.object(producer.sys, "argv", ["decide.py", "deepseek-flash", "1", "finish"]), \
                    mock.patch.dict(os.environ, {"DECIDE_THREADS": "2"}), contextlib.redirect_stdout(io.StringIO()) as stdout:
                producer.main()
            rows = [json.loads(line) for line in (root / "out/decide-deepseek-flash.jsonl").read_text().splitlines()]
        self.assertCountEqual(completed, ["intro", "bare"])
        self.assertEqual([row["arm"] for row in rows], ["intro", "bare"])
        self.assertEqual(rows[0]["error"], "ValueError")
        self.assertTrue(rows[1]["match"])
        self.assertEqual(len(stdout.getvalue().splitlines()), 2)
        cells = table.aggregate(rows)
        self.assertEqual(sum(cell["attempts"] for cell in cells), 2)
        self.assertEqual(sum(cell["errors"] for cell in cells), 1)

    def test_success_closes_owned_response_and_preserves_seat_payload(self):
        body = {"messages": [{"role": "assistant", "content": "fixture"}], "tools": [{"fixture": True}]}
        original = copy.deepcopy(body)
        response = Response(reply())
        with mock.patch.dict(os.environ, {"DEEPSEEK_API_KEY": "offline-key"}), \
                mock.patch.object(producer.urllib.request, "urlopen", return_value=response) as opener:
            observed, _, error = producer.call("deepseek-flash", body, 8192)
        self.assertTrue(response.closed)
        self.assertEqual(observed, reply())
        self.assertIsNone(error)
        self.assertEqual(body, original)
        self.assertEqual(json.loads(opener.call_args.args[0].data), {"model": "deepseek-flash", "messages": [
            {"role": "assistant", "content": "fixture", "reasoning_content": ""}],
            "tools": original["tools"], "max_tokens": 8192, "stream": False})
        self.assertEqual(opener.call_args.kwargs["timeout"], 600)

    def test_http_status_retry_conditions_and_backoffs_stay_unchanged(self):
        for status in (429, 500, 502, 503, 504):
            errors = [http_error(status) for _ in range(4)]
            with mock.patch.dict(os.environ, {"DEEPSEEK_API_KEY": "offline-key"}), \
                    mock.patch.object(producer.urllib.request, "urlopen", side_effect=[e for e, _ in errors]) as opener, \
                    mock.patch.object(producer.time, "sleep") as sleep:
                response, seconds, error = producer.call("deepseek-flash", {"messages": []}, 16)
            self.assertIsNone(response)
            self.assertEqual(seconds, 0)  # Preserve the existing exhausted-call contract.
            self.assertEqual(opener.call_count, 4)
            self.assertEqual(sleep.call_args_list, [mock.call(5), mock.call(10), mock.call(15), mock.call(20)])
            self.assertTrue(error.startswith(str(status)))
            self.assertTrue(all(body.closed for _, body in errors))
        error, body = http_error(401)
        with mock.patch.dict(os.environ, {"DEEPSEEK_API_KEY": "offline-key"}), \
                mock.patch.object(producer.urllib.request, "urlopen", side_effect=error) as opener, \
                mock.patch.object(producer.time, "sleep") as sleep:
            producer.call("deepseek-flash", {"messages": []}, 16)
        self.assertEqual(opener.call_count, 1)
        sleep.assert_not_called()
        self.assertTrue(body.closed)

    def test_protocol_read_failures_close_responses_and_keep_existing_retries(self):
        responses = [Response(reply()) for _ in range(4)]
        with mock.patch.dict(os.environ, {"DEEPSEEK_API_KEY": "offline-key"}), \
                mock.patch.object(producer.urllib.request, "urlopen", side_effect=responses) as opener, \
                mock.patch.object(producer.json, "load", side_effect=http.client.IncompleteRead(b"fixture", 10)), \
                mock.patch.object(producer.time, "sleep") as sleep:
            response, _, error = producer.call("deepseek-flash", {"messages": []}, 16)
        self.assertIsNone(response)
        self.assertIn("IncompleteRead", error)
        self.assertEqual(opener.call_count, 4)
        self.assertEqual(sleep.call_count, 4)
        self.assertTrue(all(response.closed for response in responses))

    def test_http_error_read_failure_still_retains_logical_sample_and_closes(self):
        class BrokenBody(io.BytesIO):
            def read(self, *args):
                raise OSError("offline body failure")
        body = BrokenBody()
        failure = urllib.error.HTTPError("https://offline.invalid", 503, "fixture", {}, body)
        with mock.patch.dict(os.environ, {"DEEPSEEK_API_KEY": "offline-key"}), \
                mock.patch.object(producer.urllib.request, "urlopen", side_effect=failure), \
                mock.patch.object(producer.time, "sleep") as sleep:
            row = producer.one("deepseek-flash", "finish", "intro", 0, {"messages": []}, 16)
        self.assertTrue(body.closed)
        self.assertEqual(row["error_class"], "OSError")
        self.assertTrue(row["call_error"])
        self.assertIsNone(row["secs"])
        sleep.assert_not_called()  # This failure escaped before retry in the old producer too.


if __name__ == "__main__":
    unittest.main()
