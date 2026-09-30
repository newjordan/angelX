"""Host-clock phase diagnostics distinguish measured, assumed and unknown time."""
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


HARNESS = Path(__file__).resolve().parents[2] / "scripts/bench/book-harness"


def load(name):
    spec = importlib.util.spec_from_file_location("phase_test_" + name, HARNESS / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


phase = load("phase_accounting")
harness = load("harness_time")
groups = load("phases")


def requests():
    return [{"t": 100, "done": 110}, {"t": 111, "done": 112}]


def event(timestamp, kind, name="shell"):
    return {"t": timestamp, "kind": kind, "name": name}


def events():
    return [event(110.1, "call"), event(110.6, "done")]


class PhaseAccountingTests(unittest.TestCase):
    def test_imports_do_not_read_inputs_credentials_or_argv(self):
        with mock.patch.dict(os.environ, {}, clear=True), mock.patch.object(sys, "argv", ["fixture", "/missing"]), \
                mock.patch.object(Path, "open", side_effect=AssertionError("input read")), \
                contextlib.redirect_stdout(io.StringIO()) as stdout:
            load("harness_time")
            load("phases")
        self.assertEqual(stdout.getvalue(), "")

    def test_response_latency_is_excluded_from_harness_reply_phase(self):
        result = phase.analyze(requests(), events())
        for key, expected in {"response_latency": 10, "reply": 0.1, "tool": 0.5, "post": 0.4, "harness": 0.5}.items():
            self.assertAlmostEqual(result["totals"][key], expected)
        self.assertEqual(result["hops"][0]["response_scope"], "recorded-response-completion")
        self.assertEqual(result["metric_coverage"]["harness"],
                         {"reported": 1, "attempts": 1, "complete": True, "observed": 1, "estimated": 0})

    def test_missing_response_clock_retains_independent_tool_and_post(self):
        rows = requests()
        del rows[0]["done"]
        result = phase.analyze(rows, events())
        self.assertIsNone(result["totals"]["response_latency"])
        self.assertIsNone(result["totals"]["reply"])
        self.assertIsNone(result["totals"]["harness"])
        self.assertAlmostEqual(result["totals"]["tool"], 0.5)
        self.assertAlmostEqual(result["totals"]["post"], 0.4)
        self.assertEqual(result["metric_coverage"]["harness"]["reported"], 0)
        self.assertIn("missing/invalid response completion clock", result["hops"][0]["issues"])

    def test_instant_stub_estimate_requires_explicit_flag_and_visible_scope(self):
        rows = requests()
        del rows[0]["done"]
        result = phase.analyze(rows, events(), assume_instant_stub=True)
        self.assertAlmostEqual(result["totals"]["harness"], 10.5)
        self.assertIsNone(result["totals"]["response_latency"])
        self.assertEqual(result["metric_coverage"]["harness"]["estimated"], 1)
        self.assertEqual(result["metric_coverage"]["harness"]["observed"], 0)
        self.assertIn("harness_estimate=", harness.summary("fixture", result))
        self.assertIn("instant-stub-assumption", harness.summary("fixture", result))
        self.assertEqual(phase.phase_groups(result)[0]["metric_coverage"]["reply"]["estimated"], 1)

    def test_recorded_invalid_or_failed_clock_cannot_be_replaced_by_assumption(self):
        for value in (None, True, 99, float("nan"), float("inf"), "110"):
            rows = requests()
            rows[0]["done"] = value
            with self.subTest(value=value):
                result = phase.analyze(rows, events(), assume_instant_stub=True)
                self.assertIsNone(result["totals"]["harness"])
                self.assertIsNone(result["totals"]["response_latency"])
                self.assertEqual(result["hops"][0]["response_scope"], "unknown")
                json.dumps(result, allow_nan=False)

    def test_completion_after_call_keeps_tool_span_but_unknown_reply(self):
        rows = requests()
        rows[0]["done"] = 110.2
        result = phase.analyze(rows, events())
        self.assertAlmostEqual(result["totals"]["response_latency"], 10.2)
        self.assertAlmostEqual(result["totals"]["tool"], 0.5)
        self.assertIsNone(result["totals"]["reply"])
        self.assertIsNone(result["totals"]["harness"])
        self.assertIn("response completion is after the first tool call", result["hops"][0]["issues"])

    def test_wrong_tool_or_missing_completion_cannot_be_a_complete_span(self):
        cases = [[event(110.1, "call"), event(110.6, "done", "read_file")],
                 [event(110.1, "call")], [event(110.6, "done")], []]
        for logged in cases:
            with self.subTest(events=logged):
                result = phase.analyze(requests(), logged)
                self.assertIsNone(result["totals"]["tool"])
                self.assertIsNone(result["totals"]["post"])
                self.assertIsNone(result["totals"]["harness"])
                self.assertTrue(result["hops"][0]["issues"])
        self.assertAlmostEqual(phase.analyze(requests(), cases[0])["totals"]["reply"], 0.1)

    def test_overlapping_and_sequential_tool_calls_preserve_elapsed_span(self):
        logged = [event(110.1, "call", "shell"), event(110.2, "call", "read_file"),
                  event(110.3, "done", "read_file"), event(110.6, "done", "shell")]
        result = phase.analyze(requests(), logged)
        self.assertAlmostEqual(result["totals"]["tool"], 0.5)
        self.assertAlmostEqual(result["totals"]["harness"], 0.5)
        self.assertIn("not invocation durations", result["tool_association"])
        sequential = [event(110.1, "call"), event(110.2, "done"), event(110.3, "call"), event(110.6, "done")]
        self.assertAlmostEqual(phase.analyze(requests(), sequential)["totals"]["tool"], 0.5)

    def test_invalid_or_regressing_clock_cannot_sort_into_valid_association(self):
        for logged in ([event(None, "call"), event(110.6, "done")],
                       [event(110.6, "done"), event(110.1, "call")]):
            result = phase.analyze(requests(), logged)
            self.assertIsNone(result["totals"]["tool"])
            self.assertIsNone(result["totals"]["harness"])
            self.assertEqual(result["totals"]["response_latency"], 10)
            self.assertTrue(result["clock_issues"])
        rows = [{"t": 100, "done": 100}, {"t": 111, "done": 111}, {"t": 105}]
        result = phase.analyze(rows, events())
        self.assertTrue(all(hop["tool"] is None for hop in result["hops"]))
        self.assertIn("request clocks regress in captured order", result["clock_issues"])

    def test_direct_event_clock_inputs_are_checked_without_mutating_evidence(self):
        for value in (True, -1, float("nan"), float("inf"), "110.1"):
            logged = [event(value, "call"), event(110.6, "done")]
            result = phase.analyze(requests(), logged)
            self.assertIsNone(result["totals"]["tool"])
            self.assertTrue(result["clock_issues"])
            self.assertIs(logged[0]["t"], value)
            json.dumps(result, allow_nan=False)

    def test_shared_boundary_event_cannot_be_counted_in_two_hops(self):
        rows = [{"t": 100, "done": 100}, {"t": 101, "done": 101}, {"t": 102}]
        logged = [event(100.5, "call"), event(101, "done"), event(101.5, "call"), event(101.6, "done")]
        result = phase.analyze(rows, logged)
        self.assertIsNone(result["totals"]["tool"])
        self.assertTrue(all("event lies on a shared request boundary" in hop["issues"] for hop in result["hops"]))

    def test_measured_zero_phases_survive_without_missing_clock_coercion(self):
        result = phase.analyze([{"t": 100, "done": 100}, {"t": 100}], [event(100, "call"), event(100, "done")])
        self.assertEqual(result["totals"], dict.fromkeys(phase.FIELDS, 0))
        self.assertTrue(all(value["complete"] for value in result["metric_coverage"].values()))

    def test_partial_hop_retains_subtotals_without_claiming_complete_total(self):
        rows = [{"t": 100, "done": 110}, {"t": 111}, {"t": 113}]
        logged = events() + [event(112, "call"), event(112.5, "done")]
        result = phase.analyze(rows, logged)
        self.assertIsNone(result["totals"]["harness"])
        self.assertAlmostEqual(result["reported_subtotals"]["harness"], 0.5)
        self.assertEqual(result["metric_coverage"]["harness"]["reported"], 1)
        self.assertEqual(result["metric_coverage"]["harness"]["attempts"], 2)
        grouped = phase.phase_groups(result)[0]
        self.assertIsNone(grouped["medians"]["reply"])
        self.assertAlmostEqual(grouped["reported_medians"]["reply"], 0.1)

    def test_empty_and_single_request_are_unknown_rather_than_measured_zero(self):
        for rows in ([], [{"t": 100, "done": 101}]):
            result = phase.analyze(rows, [])
            self.assertEqual(result["hops"], [])
            self.assertTrue(all(value is None for value in result["totals"].values()))
            self.assertTrue(all(not value["complete"] for value in result["metric_coverage"].values()))

    def test_event_reader_retains_invalid_and_incomplete_clock_evidence(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "fixture.stderr"
            path.write_text("110.1 [task-event] call shell: fixture\nNaN [task-event] done shell fixture\n"
                            "110.6 [task-event] call\n110.7 unrelated line\n")
            parsed = phase.read_events(path)
        self.assertEqual(len(parsed), 3)
        self.assertIsNone(parsed[1]["t"])
        self.assertTrue(parsed[2]["malformed"])
        result = phase.analyze(requests(), parsed)
        self.assertIn("incomplete task-event record prevents event association", result["clock_issues"])

    def test_cli_defaults_and_audit_retain_scope_and_unknown_coverage(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "fixture.jsonl").write_text("\n".join(json.dumps(row) for row in requests()) + "\n")
            (root / "fixture.stderr").write_text("110.100 [task-event] call shell\n110.600 [task-event] done shell\n")
            with contextlib.redirect_stdout(io.StringIO()) as out:
                self.assertEqual(harness.main(["fixture", "--out-dir", str(root)]), 0)
            self.assertIn("harness=  0.50s", out.getvalue())
            self.assertIn("response_latency= 10.00s", out.getvalue())
            with contextlib.redirect_stdout(io.StringIO()) as out:
                groups.main(["fixture", "--out-dir", str(root), "--audit-json"])
            audit = json.loads(out.getvalue())
            self.assertEqual(audit["metric_coverage"]["reply"]["observed"], 1)
            self.assertAlmostEqual(audit["groups"][0]["medians"]["reply"], 0.1)
            (root / "fixture.jsonl").write_text('{"t":100}\n{"t":111}\n')
            with contextlib.redirect_stdout(io.StringIO()) as out:
                harness.main(["fixture", "--out-dir", str(root)])
            self.assertIn("unknown_harness_hops=1", out.getvalue())
            self.assertIn("harness=     —", out.getvalue())

    def test_malformed_input_refuses_stdout_and_identifies_source_line(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "fixture.jsonl").write_text('{"t":100}\ninvalid JSON\n')
            (root / "fixture.stderr").write_text("")
            for module in (harness, groups):
                with contextlib.redirect_stdout(io.StringIO()) as out, contextlib.redirect_stderr(io.StringIO()) as err:
                    with self.assertRaises(SystemExit):
                        module.main(["fixture", "--out-dir", str(root)])
                self.assertEqual(out.getvalue(), "")
                self.assertIn("fixture.jsonl:2", err.getvalue())


if __name__ == "__main__":
    unittest.main()
