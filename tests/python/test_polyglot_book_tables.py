"""Legacy book tables must retain request attempts and per-field coverage."""
import contextlib
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


HARNESS = Path(__file__).resolve().parents[2] / "scripts/bench/book-harness"


def load(name):
    spec = importlib.util.spec_from_file_location("book_table_test_" + name, HARNESS / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


e1 = load("e1_table")
e3 = load("e3_table")


def receipt(value, timestamp=1, **extra):
    return {"body": {"model": "fixture", "messages": []}, "usage": value,
            "t": timestamp, "done": timestamp + 1, **extra}


def chat(prompt=100, cached=20):
    return {"prompt_tokens": prompt, "prompt_tokens_details": {"cached_tokens": cached},
            "completion_tokens": 5, "completion_tokens_details": {"reasoning_tokens": 0}}


class LegacyTableTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="angel-book-table-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.out = self.root / "out"
        self.out.mkdir()
        self.heesch = self.root / "heesch"
        (self.heesch / "submission").mkdir(parents=True)
        (self.heesch / "submission/best.heesch").write_text("frontier fixture")

    def log(self, label, rows, *, run=True, candidate=False):
        (self.out / (label + ".jsonl")).write_text("\n".join(map(json.dumps, rows)) + ("\n" if rows else ""))
        if run:
            (self.out / (label + ".run")).write_text(label + " status=done solved=yes wall=2\n")
        if label.startswith("e3-"):
            state = self.out / (label + ".state")
            (state / "loops").mkdir(parents=True, exist_ok=True)
            (state / "loops/fixture.json").write_text(json.dumps({"iteration": 2, "directions_tried": ["fixture"]}))
            (state / "learning-fixture.json").write_text('{"loop-iteration": 1}\n')
            if candidate:
                (state / "submission").mkdir()
                (state / "submission/best.heesch").write_text("frontier fixture")

    def test_import_does_not_read_frontier_or_argv_or_emit_output(self):
        with mock.patch.dict(os.environ, {"HEESCH": "/missing/heesch"}):
            with mock.patch.object(sys, "argv", ["fixture", "/missing/out"]):
                with contextlib.redirect_stdout(io.StringIO()) as stdout:
                    load("e1_table")
                    load("e3_table")
        self.assertEqual(stdout.getvalue(), "")

    def test_e1_messages_total_includes_cache_read_and_creation(self):
        value = {"input_tokens": 100, "cache_read_input_tokens": 20,
                 "cache_creation_input_tokens": 10, "output_tokens": 5}
        self.log("e1-deepseek-flash-bare-1", [receipt(value)])
        row = e1.load_rows(out_dir=self.out)[("deepseek-flash", "bare")][0]
        self.assertEqual((row["prompt"], row["cached"], row["fresh"]), (130, 20, 110))
        text = e1.table(e1.load_rows(out_dir=self.out))
        self.assertIn("110 (110–110) | 130 (130–130)", text)

    def test_e1_failure_tail_remains_a_turn_and_unknown_tokens_have_coverage(self):
        self.log("e1-glm-flash-bare-1", [receipt(chat()), receipt(None, 2, proxy_error="URLError")])
        rows = e1.load_rows(out_dir=self.out)
        row = rows[("glm-flash", "bare")][0]
        self.assertEqual(row["turns"], 2)
        self.assertIsNone(row["prompt"])
        self.assertEqual(row["usage_reported_subtotals"]["prompt"], 100)
        text = e1.table(rows)
        self.assertIn("2 (2–2)", text)
        self.assertIn("— [runs 0/1; receipts 1/2]", text)

    def test_e1_median_uses_complete_runs_and_discloses_missing_runs(self):
        self.log("e1-glm-flash-bare-1", [receipt(chat())])
        self.log("e1-glm-flash-bare-2", [receipt(chat()), receipt(None, 2)])
        text = e1.table(e1.load_rows(out_dir=self.out))
        self.assertIn("80 (80–80) [runs 1/2; receipts 2/3]", text)
        self.assertIn("100 (100–100) [runs 1/2; receipts 2/3]", text)

    def test_e1_prefix_and_arm_order_remain_compatible(self):
        for arm in reversed(e1.ARM_ORDER):
            self.log("e5-glm-flash-" + arm + "-1", [receipt(chat())])
        text = e1.table(e1.load_rows("e5", self.out))
        displayed = [line.split(" | ")[1] for line in text.splitlines()[2:]]
        self.assertEqual(displayed, list(e1.ARM_ORDER))
        self.assertEqual(e1.table(e1.load_rows(out_dir=self.out)).count("glm-flash"), 0)

    def test_zero_receipts_and_wall_are_measured_but_empty_attempts_are_unknown(self):
        self.log("e1-glm-flash-bare-1", [receipt(chat(0, 0))])
        (self.out / "e1-glm-flash-bare-1.run").write_text("solved=yes wall=0\n")
        text = e1.table(e1.load_rows(out_dir=self.out))
        self.assertIn("0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0)", text)
        self.log("e1-glm-flash-intro-1", [])
        text = e1.table(e1.load_rows(out_dir=self.out))
        self.assertIn("— [runs 0/1; receipts 0/0]", text)

    def test_missing_run_receipt_retains_request_and_unknown_score_and_wall(self):
        self.log("e1-glm-flash-bare-1", [receipt(chat())], run=False)
        rows = e1.load_rows(out_dir=self.out)
        row = rows[("glm-flash", "bare")][0]
        self.assertIsNone(row["solved"])
        self.assertIsNone(row["wall"])
        self.assertIn("0 [0/1 scored]", e1.table(rows))
        self.assertIn("— [0/1 reported]", e1.table(rows))

    def test_e3_messages_prompt_cache_and_frontier_are_checked_lazily(self):
        value = {"input_tokens": 100, "cache_read_input_tokens": 20,
                 "cache_creation_input_tokens": 10, "output_tokens": 5}
        self.log("e3-deepseek-flash-intro-1", [receipt(value)], candidate=True)
        rows = e3.load_sessions(self.out, self.heesch)
        row = rows[("deepseek-flash", "intro")][0]
        self.assertEqual((row["prompt"], row["cached"]), (130, 20))
        self.assertEqual(row["hc"], "frontier")
        self.assertEqual(row["iterations"], 2)
        self.assertEqual(row["observed"], 1)
        self.assertIn("0.0M (15%)", e3.table(rows))
        self.assertIn("#1 obs=1 iters=2 dirs=1 status=done", e3.table(rows))

    def test_e3_partial_session_keeps_failures_and_reports_subtotals(self):
        self.log("e3-muse-bare-1", [receipt(chat()), receipt(None, 2, proxy_error="BrokenPipeError")])
        rows = e3.load_sessions(self.out, self.heesch)
        self.assertEqual(rows[("muse", "bare")][0]["requests"], 2)
        text = e3.table(rows)
        self.assertIn("prompt receipts 1/2, reported subtotal 100", text)
        self.assertIn("cached receipts 1/2, reported subtotal 20", text)
        self.assertNotIn("(20%)", text)

    def test_e3_responses_cache_and_invalid_cache_share(self):
        value = {"input_tokens": 100, "input_tokens_details": {"cached_tokens": 20}, "output_tokens": 5}
        self.log("e3-muse-bare-1", [receipt(value)])
        self.assertIn("0.0M (20%)", e3.table(e3.load_sessions(self.out, self.heesch)))
        self.log("e3-muse-bare-1", [receipt(chat(cached=120))])
        text = e3.table(e3.load_sessions(self.out, self.heesch))
        self.assertIn("invalid cache receipts 1", text)
        self.assertNotIn("(120%)", text)

    def test_e3_absent_run_file_does_not_drop_failed_session(self):
        self.log("e3-muse-bare-1", [receipt(None, proxy_error="URLError")], run=False)
        rows = e3.load_sessions(self.out, self.heesch)
        self.assertEqual(rows[("muse", "bare")][0]["status"], "status=unknown")
        self.assertIn("prompt receipts 0/1", e3.table(rows))

    def test_e3_route_and_token_counts_use_the_same_loaded_receipt_snapshot(self):
        rows = [receipt(chat()), receipt(chat(), 2)]
        rows[0]["body"]["messages"] = [{"role": "tool", "content": "⡪⠁"}]
        rows[1]["body"]["messages"] = [*rows[0]["body"]["messages"],
            {"role": "assistant", "tool_calls": [{"function": {"name": "apply_patch", "arguments": "{}"}}]}]
        self.log("e3-muse-bare-1", rows)
        read_receipts = e3.receipt.read_receipts

        def mutated_after_read(path):
            snapshot = read_receipts(path)
            Path(path).write_text("{changed wire after read\n")
            return snapshot

        with mock.patch.object(e3.receipt, "read_receipts", side_effect=mutated_after_read):
            sessions = e3.load_sessions(self.out, self.heesch)
        row = sessions[("muse", "bare")][0]
        self.assertEqual((row["requests"], row["prompt"]), (2, 200))
        self.assertEqual((row["raised"]["⡪⠁"], row["judged"]["⡪⠁"], row["matched"]["⡪⠁"]), (1, 1, 1))

    def test_e3_empty_output_needs_no_heesch_installation(self):
        self.assertEqual(e3.load_sessions(self.out, "/missing/heesch"), {})
        result = subprocess.run([sys.executable, str(HARNESS / "e3_table.py"), "--out-dir", str(self.out),
                                 "--heesch", "/missing/heesch"], capture_output=True, text=True, timeout=3)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("E4 (intro arm)", result.stdout)

    def test_e3_local_verifier_receives_candidate_and_original_source_is_untouched(self):
        candidate = self.root / "candidate.heesch"
        candidate.write_text("different fixture")
        with mock.patch.object(e3.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, '{"hc_verified": 5}', '')) as run:
            self.assertEqual(e3.hc_of(candidate, self.heesch), 5)
        self.assertEqual(run.call_args.args[0][-1], candidate)
        self.assertEqual(run.call_args.kwargs["cwd"], self.heesch)
        self.assertEqual((self.heesch / "submission/best.heesch").read_text(), "frontier fixture")

    def test_table_cli_refuses_malformed_receipt_without_partial_table(self):
        for name, label in (("e1_table.py", "e1-muse-bare-1"), ("e3_table.py", "e3-muse-bare-1")):
            path = self.out / (label + ".jsonl")
            path.write_text("{invalid\n")
            result = subprocess.run([sys.executable, str(HARNESS / name), "--out-dir", str(self.out)],
                                    capture_output=True, text=True, timeout=3)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stdout, "")
            self.assertIn("invalid receipt JSON", result.stderr)
            path.unlink()


if __name__ == "__main__":
    unittest.main()
