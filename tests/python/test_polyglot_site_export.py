"""The site renderer cannot distinguish missing measurements from numeric zero."""
import contextlib
import copy
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/build_polyglot_bench_data.py"


def load():
    spec = importlib.util.spec_from_file_location("polyglot_site_export_test", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


exporter = load()
METADATA = {"angelx": {"commit": "13c97bc", "cockpit_source_sha256": "a" * 64,
                       "executable_sha256": "b" * 64},
            "peers": {"omp": "recorded omp fixture", "opencode": "recorded OpenCode fixture"}}


def row(**changes):
    return {"model": "deepseek", "harness": "angelx", "run": "recorded-run", "task": "js-a",
            "reward": 1, "scored": True, "solved": True, "agent_wall_s": 2.345, "calls": 2,
            "prompt_tokens": 100, "cached_tokens": 20, "completion_tokens": 10, "reasoning_tokens": 0,
            "call_errors": [], "trace_errors": [], "isolation": "fresh-home", **changes}


def results(*rows):
    return {**copy.deepcopy(METADATA), "rows": list(rows)}


class SiteExportTests(unittest.TestCase):
    def test_import_has_no_reads_of_inputs_probes_or_output(self):
        with mock.patch.object(subprocess, "check_output", side_effect=AssertionError("live probe")):
            with mock.patch.object(Path, "read_bytes", side_effect=AssertionError("input read")):
                with mock.patch.object(Path, "write_text", side_effect=AssertionError("output write")):
                    with contextlib.redirect_stdout(io.StringIO()) as stdout:
                        load()
        self.assertEqual(stdout.getvalue(), "")

    def test_complete_zero_measurements_are_exported(self):
        zero = row(reward=0, solved=False, **dict.fromkeys(exporter.METRICS, 0))
        cell = exporter.build_data(results(zero))["cells"][0]
        self.assertEqual(cell["attempts"][0], {"task": "js-a", "run": 1, "solved": False,
                         "wall_s": 0, "calls": 0, "uncached_in": 0, "cached_in": 0, "out": 0, "reasoning": 0})
        self.assertEqual(cell["summary"]["cache_hit"], 0)
        self.assertEqual(cell["summary"]["wall"]["mean"], 0)

    def test_missing_fields_do_not_become_zero_and_have_coverage(self):
        partial = row()
        for key in exporter.METRICS:
            partial.pop(key)
        audit = exporter.audit_data(results(row(), partial))
        self.assertEqual(audit["attempts"], 2)
        for key in exporter.METRICS:
            self.assertIsNone(audit["rows"][1][key])
            self.assertIsNone(audit["metrics"][key])
            self.assertEqual(audit["metric_coverage"][key], {"reported": 1, "attempts": 2, "complete": False})
            self.assertEqual(audit["reported_subtotals"][key], row()[key])
        with self.assertRaisesRegex(ValueError, "row 2: missing/invalid calls"):
            exporter.build_data(results(row(), partial))

    def test_invalid_numbers_and_fractional_counts_are_unknown(self):
        for value in (None, True, -1, float("inf"), float("nan"), "0"):
            with self.subTest(value=value):
                audit = exporter.audit_data(results(row(prompt_tokens=value)))
                self.assertIsNone(audit["rows"][0]["prompt_tokens"])
                with self.assertRaises(ValueError):
                    exporter.build_data(results(row(prompt_tokens=value)))
        with self.assertRaisesRegex(ValueError, "invalid calls"):
            exporter.build_data(results(row(calls=1.5)))

    def test_unscored_and_inconsistent_score_do_not_become_unsolved(self):
        for changes in ({"reward": None}, {"reward": True}, {"scored": False},
                        {"scored": None}, {"reward": 0.5}, {"solved": False}):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                exporter.build_data(results(row(**changes)))

    def test_missing_identity_unknown_categories_and_nonobject_rows_are_retained(self):
        data = results(row(task=""), row(model="new-model"), row(harness="new-harness"), None)
        audit = exporter.audit_data(data)
        self.assertEqual(audit["attempts"], 4)
        self.assertEqual(audit["rows"][1]["model"], "new-model")
        self.assertTrue(any("unsupported harness" in issue for issue in audit["issues"]))
        with self.assertRaises(ValueError):
            exporter.build_data(data)

    def test_error_and_isolation_counts_are_measured(self):
        cell = exporter.build_data(results(row(call_errors=["call"], trace_errors=["trace", "trace"],
                                               isolation="shared-home")))["cells"][0]
        self.assertEqual((cell["summary"]["call_errors"], cell["summary"]["trace_errors"],
                          cell["summary"]["not_isolated"]), (1, 2, 1))
        for key in ("call_errors", "trace_errors", "isolation"):
            partial = row()
            partial.pop(key)
            with self.subTest(key=key), self.assertRaises(ValueError):
                exporter.build_data(results(partial))

    def test_empty_inputs_cannot_fabricate_empty_zero_statistics(self):
        with self.assertRaisesRegex(ValueError, "no observed attempts"):
            exporter.build_data(results())
        with self.assertRaises(ValueError):
            exporter.quantiles([])
        for bad in (None, {}, {"rows": {}}, {"rows": None}):
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                exporter.audit_data(bad)

    def test_archived_identity_does_not_probe_live_binary_or_git(self):
        with mock.patch.object(subprocess, "check_output", side_effect=AssertionError("live probe")):
            with mock.patch.dict(os.environ, {"BENCH_ANGEL_BIN": "/missing/current-binary"}):
                exported = exporter.build_data(results(row()))
        self.assertEqual(exported["angelx"], METADATA["angelx"])
        self.assertEqual(exported["peers"], METADATA["peers"])

    def test_missing_invalid_and_conflicting_recorded_metadata_refuse(self):
        for field in ("angelx", "peers"):
            data = results(row())
            del data[field]
            audit = exporter.audit_data(data)
            self.assertFalse(audit["exportable"])
            self.assertEqual(audit["metric_coverage"]["calls"]["reported"], 1)
            with self.assertRaisesRegex(ValueError, "recorded"):
                exporter.build_data(data)
        for field, value in (("commit", "current"), ("cockpit_source_sha256", ""), ("executable_sha256", None)):
            data = results(row())
            data["angelx"][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                exporter.build_data(data)
        conflicting = copy.deepcopy(METADATA)
        conflicting["angelx"]["commit"] = "0000000"
        with self.assertRaisesRegex(ValueError, "conflicts with results"):
            exporter.build_data(results(row()), conflicting)
        malformed = results(row())
        malformed["angelx"]["commit"] = float("nan")
        malformed["peers"] = float("nan")
        audit = exporter.audit_data(malformed)
        self.assertFalse(audit["exportable"])
        json.dumps(audit, allow_nan=False)

    def test_order_within_resumed_run_and_partial_cohort_are_preserved(self):
        exported = exporter.build_data(results(row(run="z", task="js-z", task_index=0),
                                               row(run="a", task="js-a", task_index=1),
                                               row(run="a", task="js-b", task_index=0)))
        self.assertEqual([(a["task"], a["run"]) for a in exported["cells"][0]["attempts"]],
                         [("js-a", 1), ("js-b", 1), ("js-z", 2)])
        self.assertEqual(exported["cells"][0]["summary"]["attempts"], 3)

    def test_cli_refusal_preserves_existing_destination(self):
        with tempfile.TemporaryDirectory() as tmp:
            source, target = Path(tmp) / "results.json", Path(tmp) / "bench.js"
            source.write_text(json.dumps(results(row(calls=None))))
            target.write_text("published destination fixture")
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as error:
                exporter.main(["--results", str(source), "--output", str(target)])
            self.assertEqual(error.exception.code, 1)
            self.assertEqual(target.read_text(), "published destination fixture")

    def test_audit_without_provenance_retains_partial_attempts_without_writes(self):
        with tempfile.TemporaryDirectory() as tmp:
            source, target = Path(tmp) / "results.json", Path(tmp) / "bench.js"
            source.write_text(json.dumps({"rows": [row(), row(prompt_tokens=None)]}))
            target.write_text("destination fixture")
            with contextlib.redirect_stdout(io.StringIO()) as stdout:
                exporter.main(["--results", str(source), "--output", str(target), "--audit-json"])
            audit = json.loads(stdout.getvalue())
            self.assertEqual(audit["attempts"], 2)
            self.assertIsNone(audit["metrics"]["prompt_tokens"])
            self.assertEqual(audit["reported_subtotals"]["prompt_tokens"], 100)
            self.assertEqual(target.read_text(), "destination fixture")

    def test_explicit_provenance_is_bound_to_exact_input_bytes(self):
        with tempfile.TemporaryDirectory() as tmp:
            source, metadata, target = (Path(tmp) / name for name in ("results.json", "metadata.json", "bench.js"))
            source.write_text(json.dumps({"rows": [row()]}))
            metadata.write_text(json.dumps({**METADATA, "results_sha256": hashlib.sha256(source.read_bytes()).hexdigest()}))
            with contextlib.redirect_stdout(io.StringIO()):
                exporter.main(["--results", str(source), "--provenance", str(metadata), "--output", str(target)])
            recorded_output = target.read_text()
            self.assertEqual(json.loads(recorded_output.removeprefix("window.BENCH = ").strip().removesuffix(";"))["angelx"],
                             METADATA["angelx"])
            source.write_text(source.read_text() + "\n")
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                exporter.main(["--results", str(source), "--provenance", str(metadata), "--output", str(target)])
            self.assertEqual(target.read_text(), recorded_output)

    def test_legacy_environment_paths_are_resolved_at_cli_time(self):
        with tempfile.TemporaryDirectory() as tmp:
            source, target = Path(tmp) / "results.json", Path(tmp) / "bench.js"
            source.write_text(json.dumps(results(row())))
            with mock.patch.dict(os.environ, {"BENCH_RESULTS_FILE": str(source), "BENCH_SITE_JS": str(target),
                                             "BENCH_ANGEL_BIN": "/unused/live/binary"}):
                with contextlib.redirect_stdout(io.StringIO()):
                    exporter.main([])
            self.assertTrue(target.read_text().startswith("window.BENCH = "))

    def test_complete_historical_chart_arrays_and_wall_statistics_are_compatible(self):
        data = results(*(row(task=task, agent_wall_s=wall) for task, wall in
                         (("js-a", 1), ("js-b", 2), ("js-c", 3), ("js-d", 100))))
        cell = exporter.build_data(data)["cells"][0]
        self.assertEqual(cell, {
            "model": "deepseek", "harness": "angelx", "harness_label": "angelX",
            "attempts": [
                {"task": "js-a", "run": 1, "solved": True, "wall_s": 1, "calls": 2,
                 "uncached_in": 100, "cached_in": 20, "out": 10, "reasoning": 0},
                {"task": "js-b", "run": 1, "solved": True, "wall_s": 2, "calls": 2,
                 "uncached_in": 100, "cached_in": 20, "out": 10, "reasoning": 0},
                {"task": "js-c", "run": 1, "solved": True, "wall_s": 3, "calls": 2,
                 "uncached_in": 100, "cached_in": 20, "out": 10, "reasoning": 0},
                {"task": "js-d", "run": 1, "solved": True, "wall_s": 100, "calls": 2,
                 "uncached_in": 100, "cached_in": 20, "out": 10, "reasoning": 0}],
            "summary": {"attempts": 4, "solved": 4,
                        "wall": {"min": 1, "q1": 1.75, "median": 2.5, "q3": 27.25,
                                 "max": 3, "mean": 26.5, "outliers": [100]},
                        "calls_mean": 2, "uncached_in_per_task": 100, "cached_in_per_task": 20,
                        "in_per_task": 120, "out_per_task": 10, "reasoning_per_task": 0,
                        "cache_hit": 1 / 6, "call_errors": 0, "trace_errors": 0, "not_isolated": 0}})


if __name__ == "__main__":
    unittest.main()
