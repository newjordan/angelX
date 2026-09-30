"""Offline regressions for all-attempt seat telemetry and safe chart exports."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/bench/seat_compare.py"
spec = importlib.util.spec_from_file_location("seat_compare", SCRIPT)
seat = importlib.util.module_from_spec(spec)
spec.loader.exec_module(seat)
EXTRACTOR = SCRIPT.parent / "charts/extract_tokens_time.py"


def trace(name="js-a", score=1, start=0, end=10, usage=None):
    return {"task": {"data": {"name": name}}, "rewards": {"technical_outcome": {"score": score}},
            "timing": {"agent": {"start": start, "end": end}},
            "info": {"codex_usage": {"turns": 1, "input_tokens": 100, "cached_input_tokens": 20,
                                      "output_tokens": 10, "reasoning_output_tokens": 0} if usage is None else usage}}


def record(*traces, name="js-a"):
    return {"task": {"data": {"name": name}}, "traces": list(traces)}


def write_run(root, records):
    root.mkdir(parents=True, exist_ok=True)
    (root / "traces.jsonl").write_text("\n".join(json.dumps(row) for row in records) + "\n")
    return root


def loaded(root, records, order=("js-a",)):
    cell = seat.load(write_run(root, records))
    cell["summary"] = seat.aggregate(cell, order)
    return cell


class UsageTests(unittest.TestCase):
    def test_codex_one_turn_measures_present_fields_and_preserves_zero(self):
        usage = seat.usage_of(trace(usage={"turns": 1, "input_tokens": 100, "output_tokens": 0}))
        self.assertEqual(usage["fields"], {"input": 100, "cached_input": None, "output": 0, "reasoning": None})
        self.assertTrue(usage["metric_coverage"]["input"]["complete"])
        self.assertFalse(usage["metric_coverage"]["reasoning"]["complete"])

    def test_codex_aggregated_turns_require_each_fields_coverage(self):
        receipt = {"turns": 2, "input_tokens": 100, "cached_input_tokens": 20, "output_tokens": 10}
        usage = seat.usage_of(trace(usage=receipt))
        self.assertIsNone(usage["fields"]["input"])
        self.assertEqual(usage["reported_subtotals"]["input"], 100)
        covered = seat.usage_of(trace(usage={**receipt, "reported_attempts": {"input_tokens": 2, "output_tokens": 1}}))
        self.assertEqual(covered["fields"]["input"], 100)
        self.assertIsNone(covered["fields"]["output"])
        self.assertEqual(covered["metric_coverage"]["output"]["reported"], 1)

    def test_codex_invalid_attempts_receipts_and_counters_stay_unknown(self):
        for turns in (None, 0, -1, True, 1.5):
            with self.subTest(turns=turns):
                self.assertIsNone(seat.usage_of(trace(usage={"turns": turns, "input_tokens": 100}))["fields"]["input"])
        for value in (True, -1, float("inf"), float("nan")):
            with self.subTest(value=value):
                usage = seat.usage_of(trace(usage={"turns": 1, "input_tokens": value}))
                self.assertIsNone(usage["fields"]["input"])
                self.assertIsNone(usage["reported_subtotals"]["input"])
        for counter in (True, -1, 2):
            with self.subTest(counter=counter):
                self.assertIsNone(seat.usage_of(trace(usage={"turns": 1, "input_tokens": 100,
                    "reported_attempts": {"input_tokens": counter}}))["fields"]["input"])
        for flag in ("untracked_sources", "overflowed", "inconsistent_attempts"):
            with self.subTest(flag=flag):
                self.assertIsNone(seat.usage_of(trace(usage={"turns": 1, "input_tokens": 100, flag: True}))["fields"]["input"])

    def test_proxy_partial_calls_keep_subtotals_instead_of_task_totals(self):
        usage = seat.usage_of({"calls": [{"usage": {"prompt_tokens": 100, "cached_input_tokens": 20,
                                                  "completion_tokens": 10}}, {}]})
        self.assertEqual(usage["source"], "proxy")
        self.assertEqual(usage["reported_subtotals"]["input"], 120)
        self.assertIsNone(usage["fields"]["input"])
        self.assertEqual(usage["metric_coverage"]["input"], {"reported": 1, "attempts": 2, "complete": False})
        self.assertIsNone(usage["fields"]["reasoning"])

    def test_proxy_missing_cache_is_not_zero_or_replaced_by_angel(self):
        usage = seat.usage_of({"calls": [{"usage": {"prompt_tokens": 100, "completion_tokens": 0}}],
                               "info": {"angel": {"usage": {"input": 999}}}})
        self.assertIsNone(usage["fields"]["input"])
        self.assertIsNone(usage["fields"]["cached_input"])
        self.assertEqual(usage["fields"]["output"], 0)
        self.assertEqual(usage["source"], "proxy")

    def test_angel_coverage_requires_coherent_attempt_counters(self):
        receipt = {"attempts": 2, "input": 200, "cache_read": 0, "output": 20,
                   "reported_attempts": {"input": 2, "cache_read": 2, "output": 1}}
        usage = seat.usage_of({"info": {"angel": {"usage": receipt}}})
        self.assertEqual(usage["fields"]["input"], 200)
        self.assertEqual(usage["fields"]["cached_input"], 0)
        self.assertIsNone(usage["fields"]["output"])
        self.assertEqual(usage["reported_subtotals"]["output"], 20)
        legacy = seat.usage_of({"info": {"angel": {"usage": {"input": 200}}}})
        self.assertIsNone(legacy["fields"]["input"])
        self.assertEqual(legacy["reported_subtotals"]["input"], 200)


class AttemptTests(unittest.TestCase):
    def test_repeated_tasks_retain_failed_tails_and_attempt_provenance(self):
        with tempfile.TemporaryDirectory() as tmp:
            cell = loaded(Path(tmp), [record(trace(score=0, end=600), trace(end=10)), record(trace(end=20))])
        s = cell["summary"]
        self.assertEqual((s["attempts"], s["unique_tasks"], s["solved"]), (3, 1, 2))
        self.assertEqual(s["duplicate_tasks"], {"js-a": 3})
        self.assertEqual(s["metrics"]["agent_s"]["total"], 630)
        self.assertEqual(s["metrics"]["input"]["total"], 300)
        self.assertEqual([(row["line"], row["attempt_index"]) for row in cell["rows"]], [(1, 1), (1, 2), (2, 1)])
        with self.assertRaisesRegex(ValueError, "one named attempt"):
            seat.chart_series(cell, ["js-a"])

    def test_conflicting_and_missing_task_names_are_not_selected(self):
        with tempfile.TemporaryDirectory() as tmp:
            cell = loaded(Path(tmp), [record(trace("js-b")), record(trace(None), name=None)])
        self.assertEqual(cell["summary"]["unnamed_attempts"], 2)
        self.assertEqual(cell["summary"]["identity_conflicts"], 1)
        self.assertEqual(cell["rows"][0]["identity_conflict"], {"record": "js-a", "trace": "js-b"})
        self.assertEqual(cell["summary"]["missing_tasks"], ["js-a"])
        self.assertIsNone(cell["rows"][0]["task"])

    def test_empty_records_and_unexpected_tasks_prevent_complete_catalog(self):
        with tempfile.TemporaryDirectory() as tmp:
            cell = loaded(Path(tmp), [record(), record(trace("js-b"), name="js-b")])
        self.assertEqual(cell["records_without_traces"], [{"line": 1, "task": "js-a"}])
        self.assertEqual(cell["summary"]["unexpected_tasks"], ["js-b"])
        self.assertFalse(cell["summary"]["complete_catalog"])
        empty = {"rows": [], "records_without_traces": []}
        empty["summary"] = seat.aggregate(empty, ["js-a"])
        self.assertIsNone(empty["summary"]["metrics"]["agent_s"]["total"])
        self.assertIn("—", seat.markdown({"empty": empty}))

    def test_partial_metrics_and_invalid_cache_remain_explicit(self):
        with tempfile.TemporaryDirectory() as tmp:
            cell = loaded(Path(tmp), [record(trace(usage={"turns": 2, "input_tokens": 100})),
                 record(trace("js-b", usage={"turns": 1, "input_tokens": 10, "cached_input_tokens": 20}), name="js-b")],
                 ["js-a", "js-b"])
        s = cell["summary"]
        self.assertEqual(s["metrics"]["input"]["total"], 10)
        self.assertEqual(s["metrics"]["input"]["reported_subtotal"], 110)
        self.assertEqual(s["metrics"]["input"]["reported"], 1)
        self.assertEqual(s["cache_invalid_attempts"], 1)
        self.assertIsNone(s["cache_ratio"])
        self.assertIn("(1/2)", seat.markdown({"partial": cell}))
        with self.assertRaisesRegex(ValueError, "complete input token coverage"):
            seat.chart_series(cell, ["js-a", "js-b"])

    def test_invalid_clocks_and_unobserved_scores_cannot_be_charted(self):
        for kwargs in ({"start": 10, "end": 5}, {"end": True}, {"end": float("nan")}, {"score": None}, {"score": True}):
            with self.subTest(kwargs=kwargs), tempfile.TemporaryDirectory() as tmp:
                cell = loaded(Path(tmp), [record(trace(**kwargs))])
                with self.assertRaisesRegex(ValueError, "observed scores"):
                    seat.chart_series(cell, ["js-a"])

    def test_complete_legacy_chart_shape_order_and_zero_values_are_preserved(self):
        with tempfile.TemporaryDirectory() as tmp:
            cell = loaded(Path(tmp), [record(trace("js-b", score=0, end=12.345), name="js-b"),
                       record(trace(end=0, usage={"turns": 1, "input_tokens": 0}))], ["js-a", "js-b"])
        self.assertEqual(seat.chart_series(cell, ["js-a", "js-b"]),
                         [["js-a", True, 0, 0], ["js-b", False, 12.35, 100]])


class CliTests(unittest.TestCase):
    def run_cli(self, root, records, *extra, catalog=None):
        run = write_run(root / "run", records)
        catalog_path = root / "catalog.json"
        catalog_path.write_text(json.dumps(catalog if catalog is not None else [{"name": "js-a"}]))
        return subprocess.run([sys.executable, str(SCRIPT), f"seat={run}", "--catalog", str(catalog_path), *extra],
                              capture_output=True, text=True, timeout=5)

    def test_audit_retains_unknowns_and_bodies_never_enter_exports(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            chart, audit = root / "chart.json", root / "audit.json"
            chart.write_text("previous chart")
            row = trace(usage={"turns": 1, "input_tokens": 100})
            row["messages"] = [{"content": "private body sentinel"}]
            row["info"]["raw_output"] = "private body sentinel"
            result = self.run_cli(root, [record(row)], "--audit-json", str(audit), "--json", str(chart))
            # Input is complete even when other fields are missing: only the
            # measurements the legacy renderer reads constrain chart export.
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(chart.read_text())["seat"], [["js-a", True, 10, 100]])
            data = json.loads(audit.read_text())
            self.assertIsNone(data["runs"]["seat"]["rows"][0]["usage"]["fields"]["output"])
            self.assertNotIn("private body sentinel", audit.read_text())

    def test_incomplete_chart_refusal_preserves_existing_file_and_still_writes_audit(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            chart, audit = root / "chart.json", root / "audit.json"
            chart.write_text("previous chart")
            result = self.run_cli(root, [record(trace(usage={"input_tokens": 100}))],
                                  "--audit-json", str(audit), "--json", str(chart))
            self.assertEqual(result.returncode, 2)
            self.assertIn("complete input token coverage", result.stderr)
            self.assertEqual(chart.read_text(), "previous chart")
            self.assertEqual(json.loads(audit.read_text())["runs"]["seat"]["summary"]["metrics"]["input"]["reported_subtotal"], 100)

    def test_invalid_catalog_names_fail_without_export(self):
        for catalog in ([], [{"name": ""}], [{"name": "js-a"}, {"name": "js-a"}], [{"name": None}]):
            with self.subTest(catalog=catalog), tempfile.TemporaryDirectory() as tmp:
                result = self.run_cli(Path(tmp), [record(trace())], catalog=catalog)
                self.assertEqual(result.returncode, 2)
                self.assertIn("unique, nonempty task names", result.stderr)

    def test_duplicate_cli_labels_fail_instead_of_overwriting_an_arm(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            result = self.run_cli(root, [record(trace())], f"seat={root / 'run'}")
            self.assertEqual(result.returncode, 2)
            self.assertIn("labels must be nonempty and unique", result.stderr)


class ExtractorTests(unittest.TestCase):
    def test_import_does_not_read_the_external_catalog_or_arguments(self):
        spec = importlib.util.spec_from_file_location("extract_tokens_time", EXTRACTOR)
        extractor = importlib.util.module_from_spec(spec)
        with mock.patch.object(Path, "read_text", side_effect=AssertionError("unexpected catalog read")):
            spec.loader.exec_module(extractor)
        self.assertTrue(callable(extractor.per_task))

    def run_extractor(self, root, records, *, second_records=None, rerun=False):
        run = write_run(root / "run", records)
        second = write_run(root / "peer", records if second_records is None else second_records)
        catalog = root / "catalog.json"
        catalog.write_text(json.dumps([{"name": "js-a"}]))
        chart = root / "chart.json"
        chart.write_text("previous chart")
        command = [sys.executable, str(EXTRACTOR), str(run), str(second), str(chart)]
        if rerun:
            command.append(str(run))
        result = subprocess.run([*command, "--catalog", str(catalog)], capture_output=True, text=True, timeout=5)
        return result, chart

    def test_complete_arrays_and_optional_rerun_preserve_the_existing_shape(self):
        row = trace(score=0, end=12.345)
        row.pop("info")
        row["calls"] = [{"usage": {"prompt_tokens": 80, "cached_input_tokens": 20}}]
        with tempfile.TemporaryDirectory() as tmp:
            result, chart = self.run_extractor(Path(tmp), [record(row)], rerun=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(chart.read_text()),
                             {label: [["js-a", False, 12.35, 100]] for label in ("angelx", "opencode", "angelx2")})

    def test_duplicate_attempt_export_refusal_preserves_the_existing_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            result, chart = self.run_extractor(Path(tmp), [record(trace(score=0, end=600), trace(end=10))])
            self.assertEqual(result.returncode, 2)
            self.assertIn("one named attempt", result.stderr)
            self.assertEqual(chart.read_text(), "previous chart")

    def test_incomplete_second_arm_does_not_write_a_partial_export(self):
        row = trace()
        row.pop("info")
        row["calls"] = [{"usage": {"prompt_tokens": 100}}]
        with tempfile.TemporaryDirectory() as tmp:
            result, chart = self.run_extractor(Path(tmp), [record(trace())], second_records=[record(row)])
            self.assertEqual(result.returncode, 2)
            self.assertIn("complete input token coverage", result.stderr)
            self.assertEqual(chart.read_text(), "previous chart")


if __name__ == "__main__":
    unittest.main()
