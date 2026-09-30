"""E2 probe receipts are tested with owned in-memory responses only."""
import contextlib
import copy
import http.client
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
import urllib.error


HARNESS = Path(__file__).resolve().parents[2] / "scripts/bench/book-harness"


def load():
    spec = importlib.util.spec_from_file_location("polyglot_e2_count_test", HARNESS / "e2_count.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


e2 = load()


class Response(io.BytesIO):
    def __init__(self, reply, status=200):
        super().__init__(json.dumps(reply).encode())
        self.status = status


def http_error(status):
    body = io.BytesIO(b"provider error body fixture")
    return urllib.error.HTTPError("https://offline.invalid", status, "fixture", {}, body), body


def body():
    return {"messages": [{"role": "user", "content": "fixture"},
                         {"role": "assistant", "content": "fixture", "reasoning_content": "existing"}],
            "tools": [{"type": "function", "function": {"name": "fixture"}}]}


class E2ReceiptTests(unittest.TestCase):
    def test_import_does_not_read_credentials_inputs_or_run_subprocesses(self):
        with mock.patch.dict(os.environ, {}, clear=True):
            with mock.patch.object(sys, "argv", ["fixture", "grok", "/missing/capture"]):
                with mock.patch.object(Path, "open", side_effect=AssertionError("capture read")):
                    with mock.patch.object(subprocess, "run", side_effect=AssertionError("credential process")):
                        with contextlib.redirect_stdout(io.StringIO()) as out:
                            load()
        self.assertEqual(out.getvalue(), "")

    def test_success_keeps_usage_zero_status_and_closes_owned_response(self):
        usage = {"prompt_tokens": 0, "completion_tokens": 0, "prompt_tokens_details": {"cached_tokens": 0}}
        response = Response({"usage": usage})
        with mock.patch.object(e2.urllib.request, "urlopen", return_value=response) as opener:
            with mock.patch.object(e2.time, "sleep") as sleep:
                result = e2.request_usage("deepseek-flash", e2.payload_for("deepseek-flash", body()), "offline-key")
        self.assertTrue(response.closed)
        self.assertEqual((result["prompt_tokens"], result["usage"], result["status"], result["error_class"]), (0, usage, 200, None))
        self.assertEqual(result["attempts"], [{"attempt": 1, "status": 200, "error_class": None, "usage": usage}])
        self.assertEqual(opener.call_args.kwargs["timeout"], 300)
        self.assertEqual(opener.call_args.args[0].get_header("Authorization"), "Bearer offline-key")
        sleep.assert_not_called()

    def test_connection_failure_has_one_unknown_receipt_without_new_retries(self):
        for failure in (urllib.error.URLError("fixture"), TimeoutError("fixture"), ConnectionResetError("fixture")):
            with self.subTest(failure=type(failure).__name__):
                with mock.patch.object(e2.urllib.request, "urlopen", side_effect=failure) as opener:
                    with mock.patch.object(e2.time, "sleep") as sleep:
                        result = e2.request_usage("muse", e2.payload_for("muse", body()), "offline-key")
                self.assertEqual(opener.call_count, 1)
                sleep.assert_not_called()
                self.assertIsNone(result["usage"])
                self.assertIsNone(result["prompt_tokens"])
                self.assertEqual(result["attempts"], [{"attempt": 1, "status": None,
                                                     "error_class": type(failure).__name__, "usage": None}])

    def test_retry_then_success_keeps_every_outcome_and_original_retry_policy(self):
        error, error_body = http_error(503)
        response = Response({"usage": {"prompt_tokens": 123}})
        with mock.patch.object(e2.urllib.request, "urlopen", side_effect=[error, response]) as opener:
            with mock.patch.object(e2.time, "sleep") as sleep:
                result = e2.request_usage("glm-flash", e2.payload_for("glm-flash", body()), "offline-key")
        self.assertEqual(opener.call_count, 2)
        sleep.assert_called_once_with(5)
        self.assertTrue(error_body.closed and response.closed)
        self.assertEqual(result["prompt_tokens"], 123)
        self.assertIsNone(result["error_class"])
        self.assertEqual([(a["status"], a["error_class"], a["usage"]) for a in result["attempts"]],
                         [(503, "HTTPError", None), (200, None, {"prompt_tokens": 123})])
        self.assertNotIn("provider error body fixture", json.dumps(result))

    def test_exhausted_retry_keeps_three_failures_and_original_final_backoff(self):
        errors = [http_error(429) for _ in range(3)]
        with mock.patch.object(e2.urllib.request, "urlopen", side_effect=[item[0] for item in errors]) as opener:
            with mock.patch.object(e2.time, "sleep") as sleep:
                result = e2.request_usage("deepseek-flash", e2.payload_for("deepseek-flash", body()), "offline-key")
        self.assertEqual(opener.call_count, 3)
        self.assertEqual(sleep.call_args_list, [mock.call(5)] * 3)
        self.assertTrue(all(item[1].closed for item in errors))
        self.assertEqual([item["attempt"] for item in result["attempts"]], [1, 2, 3])
        self.assertEqual((result["error_class"], result["status"], result["usage"]), ("HTTPError", 429, None))

    def test_other_http_status_and_subclass_keep_existing_retry_semantics(self):
        for status in (401, 504):
            error, error_body = http_error(status)
            with mock.patch.object(e2.urllib.request, "urlopen", side_effect=error) as opener:
                with mock.patch.object(e2.time, "sleep") as sleep:
                    result = e2.request_usage("muse", e2.payload_for("muse", body()), "offline-key")
            self.assertEqual(opener.call_count, 1)
            sleep.assert_not_called()
            self.assertTrue(error_body.closed)
            self.assertEqual(result["status"], status)
        class ErrorSubclass(urllib.error.HTTPError):
            pass
        error = ErrorSubclass("https://offline.invalid", 500, "fixture", {}, io.BytesIO())
        with mock.patch.object(e2.urllib.request, "urlopen", side_effect=[error, Response({"usage": {"prompt_tokens": 1}})]):
            with mock.patch.object(e2.time, "sleep") as sleep:
                result = e2.request_usage("muse", e2.payload_for("muse", body()), "offline-key")
        self.assertEqual(result["attempts"][0]["error_class"], "ErrorSubclass")
        sleep.assert_called_once_with(5)

    def test_read_failure_retains_observed_status_and_closes_response(self):
        class BrokenResponse(Response):
            def read(self, *args):
                raise OSError("offline read failure")
        response = BrokenResponse({"usage": {"prompt_tokens": 100}}, status=200)
        with mock.patch.object(e2.urllib.request, "urlopen", return_value=response):
            result = e2.request_usage("muse", e2.payload_for("muse", body()), "offline-key")
        self.assertTrue(response.closed)
        self.assertEqual(result["attempts"], [{"attempt": 1, "status": 200, "error_class": "OSError", "usage": None}])

    def test_http_protocol_read_failures_keep_observed_status_and_receipt(self):
        for failure in (http.client.IncompleteRead(b"fixture", 10), http.client.BadStatusLine("fixture"),
                        http.client.HTTPException("fixture")):
            response = Response({"usage": {"prompt_tokens": 100}})
            with self.subTest(failure=type(failure).__name__):
                with mock.patch.object(e2.urllib.request, "urlopen", return_value=response):
                    with mock.patch.object(e2.json, "load", side_effect=failure):
                        result = e2.request_usage("muse", e2.payload_for("muse", body()), "offline-key")
                self.assertTrue(response.closed)
                self.assertEqual(result["attempts"], [{"attempt": 1, "status": 200,
                                                     "error_class": type(failure).__name__, "usage": None}])

    def test_http_protocol_open_failure_cannot_discard_logical_label(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            (out / "fixture.jsonl").write_text(json.dumps({"body": body()}) + "\n")
            with mock.patch.object(e2, "credential_for", return_value="offline-key"):
                with mock.patch.object(e2.urllib.request, "urlopen", side_effect=http.client.BadStatusLine("fixture")) as opener:
                    with mock.patch.object(e2.time, "sleep") as sleep:
                        result = e2.count_label("muse", "fixture", out)
            self.assertEqual(opener.call_count, 1)
            sleep.assert_not_called()
            self.assertEqual(result["label"], "fixture")
            self.assertEqual(result["attempts"], [{"attempt": 1, "status": None, "error_class": "BadStatusLine", "usage": None}])
            self.assertIsNone(result["prompt_tokens"])

    def test_invalid_reply_and_nonfinite_usage_cannot_drop_the_receipt(self):
        for reply in ([], {"usage": {"prompt_tokens": float("nan")}}):
            response = Response(reply)
            with mock.patch.object(e2.urllib.request, "urlopen", return_value=response):
                result = e2.request_usage("muse", e2.payload_for("muse", body()), "offline-key")
            self.assertTrue(response.closed)
            self.assertEqual((result["status"], result["error_class"], result["usage"]), (200, "ValueError", None))
            json.dumps(result, allow_nan=False)

    def test_missing_and_invalid_prompt_fields_stay_unknown(self):
        for usage in (None, {}, {"prompt_tokens": True}, {"prompt_tokens": -1}):
            response = Response({"usage": usage})
            with mock.patch.object(e2.urllib.request, "urlopen", return_value=response):
                result = e2.request_usage("muse", e2.payload_for("muse", body()), "offline-key")
            self.assertIsNone(result["prompt_tokens"])
            self.assertEqual(len(result["attempts"]), 1)

    def test_payload_seat_settings_and_original_capture_are_preserved(self):
        capture = body()
        capture["messages"].append({"role": "assistant", "content": "fixture"})
        original = copy.deepcopy(capture)
        payload = e2.payload_for("deepseek-flash", capture)
        self.assertEqual(payload, {"model": "deepseek-flash", "messages": [
            {"role": "user", "content": "fixture"},
            {"role": "assistant", "content": "fixture", "reasoning_content": "existing"},
            {"role": "assistant", "content": "fixture", "reasoning_content": ""}],
            "tools": original["tools"], "max_tokens": 16, "stream": False})
        self.assertEqual(capture, original)
        self.assertEqual(e2.payload_for("glm-flash", capture)["thinking"], {"type": "disabled"})
        self.assertEqual(e2.payload_for("grok", capture)["reasoning_effort"], "low")

    def test_cli_retains_all_labels_after_network_failure_and_reports_failure(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            for label in ("failed", "success"):
                (out / (label + ".jsonl")).write_text(json.dumps({"body": body()}) + "\n")
            with mock.patch.object(e2, "credential_for", return_value="offline-key"):
                with mock.patch.object(e2.urllib.request, "urlopen", side_effect=[urllib.error.URLError("fixture"),
                                                                               Response({"usage": {"prompt_tokens": 0}})]):
                    with contextlib.redirect_stdout(io.StringIO()) as stdout:
                        status = e2.main(["deepseek-flash", "failed", "success", "--out-dir", str(out)])
            rows = [json.loads(line) for line in stdout.getvalue().splitlines()]
        self.assertEqual(status, 1)
        self.assertEqual([row["label"] for row in rows], ["failed", "success"])
        self.assertEqual(rows[0]["error_class"], "URLError")
        self.assertEqual(rows[1]["prompt_tokens"], 0)
        self.assertEqual([len(row["attempts"]) for row in rows], [1, 1])

    def test_missing_or_invalid_captures_record_setup_failure_without_credentials_or_calls(self):
        with tempfile.TemporaryDirectory() as tmp:
            with mock.patch.object(e2, "credential_for", side_effect=AssertionError("credential read")):
                with mock.patch.object(e2.urllib.request, "urlopen", side_effect=AssertionError("network call")):
                    for label in ("missing", "../outside"):
                        result = e2.count_label("deepseek-flash", label, Path(tmp))
                        self.assertTrue(result["setup_error"])
                        self.assertEqual(result["attempts"], [])
                        self.assertIsNone(result["prompt_tokens"])

    def test_existing_relative_out_cli_and_successful_result_fields_remain_compatible(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "out"
            out.mkdir()
            (out / "fixture.jsonl").write_text(json.dumps({"body": body()}) + "\n")
            response_usage = {"prompt_tokens": 234, "completion_tokens": 1, "prompt_cache_hit_tokens": 123}
            previous = Path.cwd()
            try:
                os.chdir(tmp)
                with mock.patch.object(e2, "credential_for", return_value="offline-key"):
                    with mock.patch.object(e2.urllib.request, "urlopen", return_value=Response({"usage": response_usage})):
                        with contextlib.redirect_stdout(io.StringIO()) as stdout:
                            status = e2.main(["deepseek-flash", "fixture"])
            finally:
                os.chdir(previous)
        result = json.loads(stdout.getvalue())
        self.assertEqual(status, 0)
        self.assertEqual({key: result[key] for key in ("route", "label", "turns", "prompt_tokens", "usage")},
                         {"route": "deepseek-flash", "label": "fixture", "turns": 2,
                          "prompt_tokens": 234, "usage": response_usage})

    def test_collector_preserves_new_retry_evidence_and_exact_legacy_projection(self):
        spec = importlib.util.spec_from_file_location("e2_collector_test", HARNESS / "collect_results.py")
        collector = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(collector)
        legacy = {"route": "deepseek-flash", "label": "e2-bare-2", "turns": 9,
                  "prompt_tokens": 0, "usage": {"prompt_tokens": 0}}
        current = {"route": "deepseek-flash", "label": "e2-intro-2", "prompt_tokens": None,
                   "status": 503, "error_class": "HTTPError", "attempts": [
                       {"attempt": 1, "status": 503, "error_class": "HTTPError", "usage": None},
                       {"attempt": 2, "status": 503, "error_class": "HTTPError", "usage": None}],
                   "setup_error": False}
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / "e2-count-fixture.jsonl").write_text(json.dumps(legacy) + "\n" + json.dumps(current) + "\n")
            result = collector.collect([Path(tmp)])["e2"]
        self.assertEqual(result[0], {"route": "deepseek-flash", "label": "e2-bare-2", "prompt_tokens": 0})
        self.assertEqual(result[1], current)


if __name__ == "__main__":
    unittest.main()
