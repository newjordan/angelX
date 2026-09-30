"""Prompt counts retain request identities and failures without provider calls."""
import contextlib
import copy
import http.client
import importlib.util
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock
import urllib.error


HARNESS = Path(__file__).resolve().parents[2] / "scripts/bench/book-harness"


def load():
    spec = importlib.util.spec_from_file_location("count_prompts_test", HARNESS / "decide/count_prompts.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


counter = load()


class Response(io.BytesIO):
    def __init__(self, reply, status=200):
        super().__init__(json.dumps(reply).encode())
        self.status = status


def body():
    return {"messages": [{"role": "assistant", "content": "offline fixture"}], "tools": []}


def captured(size=12):
    return {"body": body(), "bytes": size}


def http_error(status):
    source = io.BytesIO(b"offline provider error marker")
    return urllib.error.HTTPError("https://offline.invalid", status, "fixture", {}, source), source


class CountPromptTests(unittest.TestCase):
    def test_import_has_no_capture_credential_or_cli_side_effects(self):
        with mock.patch.dict(os.environ, {}, clear=True), mock.patch.object(sys, "argv", ["count_prompts.py", "missing"]), \
                mock.patch.object(Path, "open", side_effect=AssertionError("capture read")), \
                mock.patch.object(counter.urllib.request, "urlopen", side_effect=AssertionError("provider call")), \
                contextlib.redirect_stdout(io.StringIO()) as stdout:
            load()
        self.assertEqual(stdout.getvalue(), "")

    def test_positive_distinct_selection_keeps_explicit_order(self):
        self.assertIsNone(counter.selection(""))
        self.assertEqual(counter.selection("213, 9,51"), [213, 9, 51])
        for value in ("0", "-1", "1,0", "9,9", "1,", ",1", "1,,2", "no", "1.5", " "):
            with self.subTest(value=value), self.assertRaises(ValueError):
                counter.selection(value)

    def test_invalid_configuration_preserves_destination_before_any_calls(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "fixture.jsonl").write_text(json.dumps(captured()) + "\n")
            destination = root / "tokcount-fixture.jsonl"
            destination.write_bytes(b"preserved destination")
            for value in ("0", "-1", "2", "1,1", "1,"):
                with self.subTest(value=value), mock.patch.dict(os.environ, {"COUNT_AT": value}), \
                        mock.patch.object(counter.urllib.request, "urlopen", side_effect=AssertionError("provider call")), \
                        contextlib.redirect_stderr(io.StringIO()):
                    with self.assertRaises(SystemExit) as error:
                        counter.main(["fixture", "--out-dir", str(root)])
                self.assertEqual(error.exception.code, 2)
                self.assertEqual(destination.read_bytes(), b"preserved destination")

    def test_later_label_range_failure_refuses_entire_configuration(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "first.jsonl").write_text((json.dumps(captured()) + "\n") * 2)
            (root / "second.jsonl").write_text(json.dumps(captured()) + "\n")
            with mock.patch.dict(os.environ, {"COUNT_AT": "2"}), \
                    mock.patch.object(counter.urllib.request, "urlopen", side_effect=AssertionError("provider call")), \
                    contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit):
                    counter.main(["first", "second", "--out-dir", str(root)])
            self.assertFalse((root / "tokcount-first.jsonl").exists())
            self.assertFalse((root / "tokcount-second.jsonl").exists())

    def test_success_closes_owned_response_and_preserves_payload_and_zeros(self):
        original = body()
        unchanged = copy.deepcopy(original)
        usage = {"prompt_tokens": 0, "prompt_cache_hit_tokens": 0}
        response = Response({"usage": usage})
        with mock.patch.object(counter.urllib.request, "urlopen", return_value=response) as opener:
            result = counter.count(original, "offline-key")
        self.assertTrue(response.closed)
        self.assertEqual(original, unchanged)
        self.assertEqual((result["prompt"], result["cached"], result["error"], result["status"]), (0, 0, None, 200))
        self.assertEqual(result["attempts"], [{"attempt": 1, "status": 200, "error_class": None, "usage": usage}])
        self.assertEqual(opener.call_args.kwargs["timeout"], 300)
        self.assertEqual(json.loads(opener.call_args.args[0].data), {"model": "deepseek-flash", "messages": [
            {"role": "assistant", "content": "offline fixture", "reasoning_content": ""}],
            "tools": [], "max_tokens": 1, "stream": False})

    def test_retry_then_success_keeps_each_receipt_and_closes_errors(self):
        error, source = http_error(503)
        response = Response({"usage": {"prompt_tokens": 20, "prompt_cache_hit_tokens": 12}})
        with mock.patch.object(counter.urllib.request, "urlopen", side_effect=[error, response]), \
                mock.patch.object(counter.time, "sleep") as sleep:
            result = counter.count(body(), "offline-key")
        self.assertTrue(source.closed and response.closed)
        self.assertEqual((result["prompt"], result["cached"], result["error"]), (20, 12, None))
        self.assertEqual([(a["status"], a["error_class"]) for a in result["attempts"]], [(503, "HTTPError"), (200, None)])
        sleep.assert_called_once_with(3)
        self.assertNotIn("offline provider error marker", json.dumps(result))

    def test_exact_five_http_retry_statuses_and_exhausted_backoffs(self):
        for status in (429, 500, 502, 503, 504):
            failures = [http_error(status) for _ in range(5)]
            with self.subTest(status=status), \
                    mock.patch.object(counter.urllib.request, "urlopen", side_effect=[e for e, _ in failures]) as opener, \
                    mock.patch.object(counter.time, "sleep") as sleep:
                result = counter.count(body(), "offline-key")
            self.assertEqual(opener.call_count, 5)
            self.assertEqual(sleep.call_args_list, [mock.call(3), mock.call(6), mock.call(9), mock.call(12), mock.call(15)])
            self.assertTrue(all(source.closed for _, source in failures))
            self.assertEqual([a["attempt"] for a in result["attempts"]], [1, 2, 3, 4, 5])
            self.assertEqual((result["prompt"], result["cached"], result["status"], result["error"]), (None, None, status, "HTTPError"))

    def test_nonretry_http_status_is_one_class_only_failure(self):
        error, source = http_error(401)
        with mock.patch.object(counter.urllib.request, "urlopen", side_effect=error) as opener, \
                mock.patch.object(counter.time, "sleep") as sleep:
            result = counter.count(body(), "offline-key")
        self.assertEqual(opener.call_count, 1)
        sleep.assert_not_called()
        self.assertTrue(source.closed)
        self.assertEqual((result["status"], result["error"], result["error_class"]), (401, "HTTPError", "HTTPError"))
        self.assertIsNone(result["prompt"])

    def test_network_and_protocol_failures_keep_existing_five_tries(self):
        failures = (urllib.error.URLError("offline secret marker"), http.client.BadStatusLine("offline secret marker"))
        for failure in failures:
            with self.subTest(failure=type(failure).__name__), \
                    mock.patch.object(counter.urllib.request, "urlopen", side_effect=failure) as opener, \
                    mock.patch.object(counter.time, "sleep") as sleep:
                result = counter.count(body(), "offline-key")
            self.assertEqual(opener.call_count, 5)
            self.assertEqual(sleep.call_count, 5)
            self.assertEqual(len(result["attempts"]), 5)
            self.assertIsNone(result["prompt"])
            self.assertNotIn("offline secret marker", json.dumps(result))

    def test_read_failures_preserve_received_status_close_and_retry(self):
        responses = [Response({"usage": {"prompt_tokens": 100}}) for _ in range(5)]
        with mock.patch.object(counter.urllib.request, "urlopen", side_effect=responses), \
                mock.patch.object(counter.json, "load", side_effect=http.client.IncompleteRead(b"fixture", 10)), \
                mock.patch.object(counter.time, "sleep"):
            result = counter.count(body(), "offline-key")
        self.assertTrue(all(response.closed for response in responses))
        self.assertEqual([(a["status"], a["error_class"], a["usage"]) for a in result["attempts"]],
                         [(200, "IncompleteRead", None)] * 5)

    def test_malformed_received_reply_remains_an_unknown_count(self):
        for malformed in ([], {"usage": None}):
            responses = [Response(malformed) for _ in range(5)]
            with self.subTest(malformed=malformed), \
                    mock.patch.object(counter.urllib.request, "urlopen", side_effect=responses), \
                    mock.patch.object(counter.time, "sleep"):
                result = counter.count(body(), "offline-key")
            self.assertTrue(all(response.closed for response in responses))
            self.assertIsNone(result["prompt"])
            self.assertEqual(result["error_class"], "ValueError")
            self.assertEqual(len(result["attempts"]), 5)
            json.dumps(result, allow_nan=False)

    def test_nonfinite_usage_does_not_create_new_retry_conditions(self):
        response = Response({"usage": {"prompt_tokens": 12, "prompt_cache_hit_tokens": float("nan")}})
        with mock.patch.object(counter.urllib.request, "urlopen", return_value=response) as opener, \
                mock.patch.object(counter.time, "sleep") as sleep:
            result = counter.count(body(), "offline-key")
        self.assertEqual(opener.call_count, 1)
        sleep.assert_not_called()
        self.assertTrue(response.closed)
        self.assertEqual((result["prompt"], result["cached"], result["error_class"]), (12, None, "ValueError"))
        self.assertEqual(result["attempts"], [{"attempt": 1, "status": 200, "error_class": "ValueError",
                                             "usage": None, "usage_invalid": True}])
        json.dumps(result, allow_nan=False)

    def test_duplicate_output_labels_are_refused_before_calls_or_writes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "fixture.jsonl").write_text(json.dumps(captured()) + "\n")
            with mock.patch.dict(os.environ, {"COUNT_AT": ""}), \
                    mock.patch.object(counter.urllib.request, "urlopen", side_effect=AssertionError("provider call")), \
                    contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit):
                    counter.main(["fixture", "fixture", "--out-dir", str(root)])
            self.assertFalse((root / "tokcount-fixture.jsonl").exists())

    def test_linked_output_cannot_overwrite_an_input_capture(self):
        for mode in ("symlink", "hardlink"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                source, destination = root / "fixture.jsonl", root / "tokcount-fixture.jsonl"
                original = (json.dumps(captured()) + "\n").encode()
                source.write_bytes(original)
                if mode == "symlink":
                    destination.symlink_to(source)
                else:
                    os.link(source, destination)
                with mock.patch.dict(os.environ, {"COUNT_AT": "1"}, clear=True), \
                        mock.patch.object(counter, "count", side_effect=AssertionError("count boundary called")) as boundary, \
                        mock.patch.object(counter.urllib.request, "urlopen", side_effect=AssertionError("provider call")) as opener, \
                        contextlib.redirect_stdout(io.StringIO()) as stdout, contextlib.redirect_stderr(io.StringIO()) as stderr:
                    with self.assertRaises(SystemExit) as error:
                        counter.main(["fixture", "--out-dir", str(root)])
                self.assertEqual(error.exception.code, 2)
                self.assertIn("must not alias selected capture", stderr.getvalue())
                self.assertEqual(stdout.getvalue(), "")
                boundary.assert_not_called()
                opener.assert_not_called()
                self.assertEqual(source.read_bytes(), original)
                self.assertEqual(destination.read_bytes(), original)

    def test_output_cannot_replace_a_later_selected_capture_with_an_ordinary_name(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            first, later = root / "fixture.jsonl", root / "tokcount-fixture.jsonl"
            original_first = (json.dumps(captured(12)) + "\n").encode()
            original_later = (json.dumps(captured(20)) + "\n").encode()
            first.write_bytes(original_first)
            later.write_bytes(original_later)
            with mock.patch.dict(os.environ, {"COUNT_AT": "1"}, clear=True), \
                    mock.patch.object(counter, "count", side_effect=AssertionError("count boundary called")) as boundary, \
                    mock.patch.object(counter.urllib.request, "urlopen", side_effect=AssertionError("provider call")) as opener, \
                    contextlib.redirect_stdout(io.StringIO()) as stdout, contextlib.redirect_stderr(io.StringIO()) as stderr:
                with self.assertRaises(SystemExit) as error:
                    counter.main(["fixture", "tokcount-fixture", "--out-dir", str(root)])
            self.assertEqual(error.exception.code, 2)
            self.assertIn("must not alias selected capture", stderr.getvalue())
            self.assertEqual(stdout.getvalue(), "")
            boundary.assert_not_called()
            opener.assert_not_called()
            self.assertEqual(first.read_bytes(), original_first)
            self.assertEqual(later.read_bytes(), original_later)
            self.assertFalse((root / "tokcount-tokcount-fixture.jsonl").exists())

    def test_existing_nonalias_count_outputs_can_be_rerun_without_changing_captures(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            original = (json.dumps(captured()) + "\n").encode()
            for label in ("first", "second"):
                (root / f"{label}.jsonl").write_bytes(original)
                (root / f"tokcount-{label}.jsonl").write_bytes(b"previous count evidence\n")
            with mock.patch.dict(os.environ, {"COUNT_AT": "1", "DEEPSEEK_API_KEY": "offline-key"}), \
                    mock.patch.object(counter.urllib.request, "urlopen", side_effect=lambda *a, **k: Response({"usage": {
                        "prompt_tokens": 0, "prompt_cache_hit_tokens": 0}})) as opener, \
                    contextlib.redirect_stdout(io.StringIO()):
                status = counter.main(["first", "second", "--out-dir", str(root)])
            self.assertEqual(status, 0)
            self.assertEqual(opener.call_count, 2)
            for label in ("first", "second"):
                self.assertEqual((root / f"{label}.jsonl").read_bytes(), original)
                row = json.loads((root / f"tokcount-{label}.jsonl").read_text())
                self.assertEqual((row["label"], row["i"], row["prompt"], row["cached"], row["error"]),
                                 (label, 1, 0, 0, None))

    def test_missing_invalid_and_independently_measured_fields_stay_visible(self):
        for usage in ({}, {"prompt_tokens": True}, {"prompt_tokens": -1}, {"prompt_cache_hit_tokens": 0}):
            response = Response({"usage": usage})
            with mock.patch.object(counter.urllib.request, "urlopen", return_value=response):
                result = counter.count(body(), "offline-key")
            self.assertIsNone(result["prompt"])
            self.assertEqual(len(result["attempts"]), 1)
            self.assertIsNone(result["error"])
        self.assertEqual(result["cached"], 0)
        with mock.patch.object(counter.urllib.request, "urlopen", return_value=Response({"usage": {
                "prompt_tokens": 20, "prompt_tokens_details": {"cached_tokens": 12}}})):
            self.assertEqual(counter.count(body(), "offline-key")["cached"], 12)

    def test_setup_failure_needs_no_credentials_and_keeps_unknown_fields(self):
        with mock.patch.dict(os.environ, {}, clear=True), \
                mock.patch.object(counter.urllib.request, "urlopen", side_effect=AssertionError("provider call")):
            malformed = counter.count({})
            missing_key = counter.count(body())
        self.assertEqual(malformed["error_class"], "ValueError")
        self.assertEqual(missing_key["error_class"], "KeyError")
        for row in (malformed, missing_key):
            self.assertTrue(row["setup_error"])
            self.assertEqual(row["attempts"], [])
            self.assertIsNone(row["prompt"])

    def test_main_persists_prior_success_and_each_malformed_logical_request(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "fixture.jsonl").write_text(json.dumps(captured()) + '\n{"body": {}, "bytes": 10}\ninvalid JSON\n')
            with mock.patch.dict(os.environ, {"COUNT_AT": "", "DEEPSEEK_API_KEY": "offline-key"}), \
                    mock.patch.object(counter.urllib.request, "urlopen", return_value=Response({"usage": {
                        "prompt_tokens": 12, "prompt_cache_hit_tokens": 0}})) as opener, \
                    contextlib.redirect_stdout(io.StringIO()):
                status = counter.main(["fixture", "--out-dir", str(root)])
            rows = [json.loads(line) for line in (root / "tokcount-fixture.jsonl").read_text().splitlines()]
        self.assertEqual(status, 1)
        self.assertEqual(opener.call_count, 1)
        self.assertEqual([row["i"] for row in rows], [1, 2, 3])
        self.assertEqual((rows[0]["prompt"], rows[0]["cached"], rows[0]["bytes"], rows[0]["error"]), (12, 0, 12, None))
        self.assertEqual([row["error_class"] for row in rows[1:]], ["ValueError", "JSONDecodeError"])
        self.assertTrue(all(row["setup_error"] and not row["attempts"] for row in rows[1:]))

    def test_selected_indices_and_legacy_success_projection_stay_exact(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "fixture.jsonl").write_text("\n".join(json.dumps(captured(size)) for size in (10, 20, 30)) + "\n")
            with mock.patch.dict(os.environ, {"COUNT_AT": "3,1", "DEEPSEEK_API_KEY": "offline-key"}), \
                    mock.patch.object(counter.urllib.request, "urlopen", side_effect=lambda *a, **k: Response({"usage": {
                        "prompt_tokens": 100, "prompt_cache_hit_tokens": 50}})), \
                    contextlib.redirect_stdout(io.StringIO()) as stdout:
                status = counter.main(["fixture", "--out-dir", str(root)])
            rows = [json.loads(line) for line in (root / "tokcount-fixture.jsonl").read_text().splitlines()]
        self.assertEqual(status, 0)
        legacy = [{key: row[key] for key in ("label", "i", "prompt", "cached", "bytes", "error")} for row in rows]
        self.assertEqual(legacy, [{"label": "fixture", "i": index, "prompt": 100, "cached": 50, "bytes": size, "error": None}
                                  for index, size in ((3, 30), (1, 10))])
        self.assertEqual(stdout.getvalue(), "fixture: 2 requests, final 100 prompt tokens, errors 0\n")

    def test_empty_missing_and_path_bearing_captures_do_not_fabricate_request_one(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "empty.jsonl").write_text("")
            with mock.patch.dict(os.environ, {"COUNT_AT": ""}), \
                    mock.patch.object(counter.urllib.request, "urlopen", side_effect=AssertionError("provider call")), \
                    contextlib.redirect_stdout(io.StringIO()):
                status = counter.main(["empty", "missing", "--out-dir", str(root)])
            for label in ("empty", "missing"):
                row = json.loads((root / f"tokcount-{label}.jsonl").read_text())
                self.assertIsNone(row["i"])
                self.assertTrue(row["setup_error"])
                self.assertIsNone(row["prompt"])
            with mock.patch.dict(os.environ, {"COUNT_AT": ""}), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit):
                    counter.main(["../outside", "--out-dir", str(root)])
        self.assertEqual(status, 1)


if __name__ == "__main__":
    unittest.main()
