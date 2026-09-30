"""Offline provider receipts and report-export guards, without model calls."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


HARNESS = Path(__file__).resolve().parents[2] / "scripts/bench/book-harness"


def load(name):
    spec = importlib.util.spec_from_file_location("book_test_" + name, HARNESS / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


usage = load("receipt_usage")
collector = load("collect_results")
summary = load("run_summary")
report = load("report_data")


def chat(prompt=100, cached=20, completion=5):
    return {"prompt_tokens": prompt, "prompt_tokens_details": {"cached_tokens": cached},
            "completion_tokens": completion, "completion_tokens_details": {"reasoning_tokens": 0}}


def receipt(value, timestamp=1, **extra):
    return {"body": {"model": "offline-fixture", "messages": []},
            "usage": value, "t": timestamp, "done": timestamp + 1, **extra}


def collected_run(label, rows, **extra):
    return {"label": label, "requests": len(rows), "host": "atlas", "solved": True,
            "wall_s": 2, **usage.usage_summary(rows), "per_request": usage.per_request(rows), **extra}


class ReceiptAccountingTests(unittest.TestCase):
    def test_failure_attempt_retains_partial_subtotal_and_unknown_per_request(self):
        rows = [receipt(chat()), receipt(None, 2, proxy_error="URLError")]
        result = usage.usage_summary(rows)
        self.assertIsNone(result["prompt"])
        self.assertIsNone(result["completion"])
        self.assertEqual(result["usage_reported_subtotals"]["prompt"], 100)
        self.assertEqual(result["usage_metric_coverage"]["prompt"],
                         {"reported": 1, "attempts": 2, "complete": False})
        self.assertEqual(result["proxy_errors"], 1)
        requests = usage.per_request(rows)
        self.assertEqual(len(requests), 2)
        self.assertEqual(requests[0]["cached"], 20)
        self.assertIsNone(requests[1]["prompt"])
        self.assertIsNone(requests[1]["cached"])
        self.assertEqual(requests[1]["proxy_error"], "URLError")

    def test_responses_cache_detail_is_kept_in_total_and_per_request(self):
        value = {"input_tokens": 100, "input_tokens_details": {"cached_tokens": 20},
                 "output_tokens": 5, "output_tokens_details": {"reasoning_tokens": 0}}
        result = usage.usage_summary([receipt(value)])
        self.assertEqual((result["prompt"], result["cached"], result["fresh"]), (100, 20, 80))
        self.assertEqual(usage.per_request([receipt(value)])[0]["cached"], 20)

    def test_messages_input_excludes_cache_read_and_creation(self):
        value = {"input_tokens": 100, "cache_read_input_tokens": 20,
                 "cache_creation_input_tokens": 10, "output_tokens": 5}
        result = usage.usage_summary([receipt(value)])
        self.assertEqual((result["prompt"], result["cached"], result["fresh"]), (130, 20, 110))
        self.assertEqual(result["completion"], 5)
        self.assertIsNone(result["reasoning"])

    def test_partial_fields_are_independent_and_missing_cache_is_unknown(self):
        value = {"prompt_tokens": 100, "completion_tokens": 5}
        result = usage.usage_summary([receipt(value)])
        self.assertEqual(result["prompt"], 100)
        self.assertEqual(result["completion"], 5)
        self.assertIsNone(result["cached"])
        self.assertIsNone(result["fresh"])
        self.assertFalse(result["usage_metric_coverage"]["cached"]["complete"])

    def test_measured_zero_does_not_fall_through_to_alternate_fields(self):
        value = {**chat(0, 0, 0), "input_tokens": 999, "prompt_cache_hit_tokens": 999, "output_tokens": 999}
        result = usage.usage_summary([receipt(value)])
        self.assertEqual([result[key] for key in usage.FIELDS], [0, 0, 0, 0, 0])
        self.assertTrue(all(field["complete"] for field in result["usage_metric_coverage"].values()))

    def test_invalid_numeric_receipts_cannot_become_complete(self):
        for invalid in (True, -1, float("nan"), float("inf"), "100"):
            with self.subTest(invalid=invalid):
                result = usage.usage_summary([receipt(chat(prompt=invalid))])
                self.assertIsNone(result["prompt"])
                self.assertIsNone(result["fresh"])
                self.assertEqual(result["usage_metric_coverage"]["prompt"]["reported"], 0)
        value = {**chat(cached=True), "prompt_cache_hit_tokens": 20}
        self.assertIsNone(usage.usage_summary([receipt(value)])["cached"])

    def test_cache_above_prompt_is_visible_and_fresh_remains_unknown(self):
        result = usage.usage_summary([receipt(chat(cached=120))])
        self.assertEqual(result["cache_invalid_receipts"], 1)
        self.assertIsNone(result["fresh"])
        run = collected_run("e3-deepseek-flash-bare-1", [receipt(chat(cached=120))])
        with self.assertRaisesRegex(ValueError, "cache counts above"):
            report.require_usage(run, ("prompt", "cached"), run["label"])

    def test_absent_receipts_and_empty_attempt_list_have_no_measured_totals(self):
        for rows in ([], [receipt(None), receipt({})]):
            result = usage.usage_summary(rows)
            self.assertTrue(all(result[key] is None for key in usage.FIELDS))
            self.assertTrue(all(value is None for value in result["usage_reported_subtotals"].values()))
            self.assertTrue(all(not field["complete"] for field in result["usage_metric_coverage"].values()))

    def test_analysis_modules_do_not_read_argv_or_print_on_import(self):
        with mock.patch.object(sys, "argv", ["fixture", "/missing/fixture.json"]):
            with contextlib.redirect_stdout(io.StringIO()) as stdout:
                for name in ("collect_results", "run_summary", "report_data"):
                    load(name)
        self.assertEqual(stdout.getvalue(), "")

    def test_collector_preserves_failure_attempts_and_later_root_precedence(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            first, second = root / "first", root / "second"
            first.mkdir(); second.mkdir()
            label = "e3-fixture-bare-1"
            for directory, rows in ((first, [receipt(chat())]),
                                    (second, [receipt(chat()), receipt(None, 2, proxy_error="URLError")])):
                (directory / (label + ".jsonl")).write_text("\n".join(map(json.dumps, rows)) + "\n")
            run = collector.collect([first, second])["runs"][label]
            self.assertEqual(run["requests"], 2)
            self.assertEqual(len(run["per_request"]), 2)
            self.assertIsNone(run["prompt"])
            self.assertEqual(run["usage_reported_subtotals"]["prompt"], 100)

    def test_summary_keeps_complete_text_and_exposes_failure_coverage(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "fixture.jsonl"
            path.write_text(json.dumps(receipt(chat())) + "\n")
            self.assertEqual(summary.summary(path),
                             f"{path}: requests=1 ledger_reads=0 prompt=100 cached=20 completion=5 wall=1.0s tools={{}}")
            path.write_text(path.read_text() + json.dumps(receipt(None, 2, proxy_error="BrokenPipeError")) + "\n")
            text = summary.summary(path)
            self.assertIn("requests=2", text)
            self.assertIn("prompt=unknown[reported_subtotal=100;coverage=1/2]", text)
            self.assertIn("completion=unknown[reported_subtotal=5;coverage=1/2]", text)

    def test_malformed_log_refuses_json_output_instead_of_dropping_attempt(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "e1-fixture-bare-1.jsonl"
            path.write_text(json.dumps(receipt(chat())) + "\n{invalid\n")
            result = subprocess.run([sys.executable, str(HARNESS / "collect_results.py"), tmp],
                                    text=True, capture_output=True, timeout=3)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stdout, "")
            self.assertIn("invalid receipt JSON", result.stderr)
            self.assertIn(":2", result.stderr)


class ReportExportTests(unittest.TestCase):
    def build(self, runs, e2=None):
        return report.build_report_data({"runs": runs, "e2": e2 or []}, "/missing/results.json")

    def test_partial_solved_cohort_and_legacy_missing_coverage_refuse_export(self):
        label = "e1-glm-flash-bare-1"
        run = collected_run(label, [receipt(chat()), receipt(None, 2)])
        with self.assertRaisesRegex(ValueError, "coverage=1/2"):
            self.build({label: run})
        run = collected_run(label, [receipt(chat())])
        run.pop("usage_metric_coverage")
        with self.assertRaisesRegex(ValueError, "complete receipt accounting"):
            self.build({label: run})

    def test_complete_report_arrays_have_expected_tokens_and_loop_series(self):
        label, loop = "e1-glm-flash-bare-1", "e3-deepseek-flash-bare-1"
        rows = [receipt(chat()), receipt(chat(), 61)]
        result = self.build({label: collected_run(label, rows), loop: collected_run(loop, rows)})
        self.assertEqual(result["e1"][0]["fresh"], {"median": 160, "min": 160, "max": 160, "n": 1})
        self.assertEqual(result["e1"][0]["prompt"]["median"], 200)
        self.assertEqual(result["loops"][0]["series"],
                         [{"i": 1, "minutes": 0.0, "prompt": 100, "cached": 20, "uncached_cum": 80},
                          {"i": 2, "minutes": 1.0, "prompt": 100, "cached": 20, "uncached_cum": 160}])

    def test_partial_loop_and_omitted_request_tail_refuse_export(self):
        label = "e3-deepseek-flash-bare-1"
        run = collected_run(label, [receipt(chat()), receipt(None, 2)])
        with self.assertRaisesRegex(ValueError, "coverage=1/2"):
            self.build({label: run})
        run = collected_run(label, [receipt(chat()), receipt(chat(), 2)])
        run["per_request"].pop()
        with self.assertRaisesRegex(ValueError, "request-series coverage=1/2"):
            self.build({label: run})

    def test_invalid_timestamps_and_boolean_coverage_refuse_export(self):
        label = "e3-deepseek-flash-bare-1"
        run = collected_run(label, [receipt(chat(), 2), receipt(chat(), 1)])
        with self.assertRaisesRegex(ValueError, "timestamps are reversed"):
            self.build({label: run})
        run = collected_run(label, [receipt(chat())])
        run["usage_metric_coverage"]["prompt"]["reported"] = True
        with self.assertRaisesRegex(ValueError, "coverage"):
            self.build({label: run})

    def test_stub_timing_remains_exportable_without_provider_usage(self):
        label = "base-1"
        run = collected_run(label, [receipt(None)], timing={"wall_ms": 10})
        result = self.build({label: run})
        self.assertEqual(result["stub"]["base"]["wall_ms"], {"median": 10, "min": 10, "max": 10, "n": 1})

    def test_e2_zero_is_observed_but_missing_grid_point_refuses_export(self):
        rows = [{"route": "glm-flash", "label": f"e2-{arm}-{cycles}", "prompt_tokens": 0}
                for arm in ("prebook", "bare", "intro") for cycles in (2, 16, 33, 70)]
        result = self.build({}, rows)
        self.assertEqual(result["e2"][0]["points"][0], {"requests": 9, "prebook": 0, "bare": 0, "intro": 0})
        with self.assertRaisesRegex(ValueError, "E2 .* missing or invalid"):
            self.build({}, rows[:-1])
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / "e2-count-fixture.jsonl").write_text(json.dumps(rows[0]) + "\n")
            self.assertEqual(collector.collect([tmp])["e2"][0]["prompt_tokens"], 0)

    def test_failed_final_followup_refuses_cache_fraction_export(self):
        run = collected_run("twoturn-ds", [receipt(chat()), receipt(None, 2)])
        with self.assertRaisesRegex(ValueError, "final request.*coverage=0/1"):
            self.build({"twoturn-ds": run})

    def test_cli_refusal_emits_no_partial_json(self):
        label = "e1-glm-flash-bare-1"
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "results.json"
            path.write_text(json.dumps({"runs": {label: collected_run(label, [receipt(chat()), receipt(None, 2)])}, "e2": []}))
            result = subprocess.run([sys.executable, str(HARNESS / "report_data.py"), str(path)],
                                    text=True, capture_output=True, timeout=3)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stdout, "")
            self.assertIn("coverage=1/2", result.stderr)


if __name__ == "__main__":
    unittest.main()
