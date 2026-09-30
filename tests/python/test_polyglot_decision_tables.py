"""Offline decision tables retain failed attempts and unambiguous cost pairs."""
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


DIRECTORY = Path(__file__).resolve().parents[2] / "scripts/bench/book-harness/decide"


def load(name):
    spec = importlib.util.spec_from_file_location("decision_table_test_" + name, DIRECTORY / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


decide = load("decide_table")
loop = load("loop_table")


def decision(**changes):
    return {"scenario": "batch", "model": "glm-flash", "arm": "intro", "i": 0,
            "error": None, "match": True, "calls": ["read_file"], "args": ["{}"],
            "prompt": 100, "completion": 10, "reasoning": 0, "secs": 1, **changes}


def replay(**changes):
    return {"task": "js-a", "loop_at": 5, "i": 0, "cue": "raw", "move": "repeat",
            "broke": False, "prompt_tokens": 100, **changes}


class DecisionTableTests(unittest.TestCase):
    def test_imports_do_not_read_argv_or_emit_output(self):
        with mock.patch.object(sys, "argv", ["fixture", "/missing/log.jsonl"]):
            with mock.patch.object(Path, "open", side_effect=AssertionError("input read")):
                with contextlib.redirect_stdout(io.StringIO()) as out, contextlib.redirect_stderr(io.StringIO()) as err:
                    load("decide_table")
                    load("loop_table")
        self.assertEqual((out.getvalue(), err.getvalue()), ("", ""))

    def test_error_only_decision_cell_reports_unknown_interval_without_crashing(self):
        cell = decide.aggregate([{"scenario": "batch", "model": "glm-flash", "arm": "intro",
                                  "i": 0, "secs": 0, "error": "URLError"}])[0]
        self.assertEqual((cell["attempts"], cell["replies"], cell["no_reply"], cell["errors"], cell["n"]),
                         (1, 0, 1, 1, 0))
        self.assertIsNone(cell["rate"])
        self.assertIsNone(cell["lo"])
        self.assertIsNone(cell["prompt"])
        self.assertEqual(cell["secs"], 0)
        self.assertIn("[—,—]", decide.summary(cell))
        self.assertIn("attempts=1; replies=0; errors=1; no_reply=1", decide.summary(cell))

    def test_failed_tail_does_not_turn_partial_tokens_into_a_complete_median(self):
        failure = {"scenario": "batch", "model": "glm-flash", "arm": "intro", "error": "URLError"}
        cell = decide.aggregate([decision(), failure])[0]
        self.assertEqual((cell["n"], cell["attempts"]), (1, 2))
        self.assertIsNone(cell["prompt"])
        self.assertEqual(cell["reported_medians"]["prompt"], 100)
        self.assertEqual(cell["reported_subtotals"]["prompt"], 100)
        self.assertEqual(cell["metric_coverage"]["prompt"], {"reported": 1, "attempts": 2, "complete": False})
        self.assertIn("prompt 1/2 reported median=100", decide.summary(cell))

    def test_missing_reply_is_not_an_answer_and_missing_score_is_not_false(self):
        cell = decide.aggregate([decision(calls=None), decision(match=None), decision(match="yes")])[0]
        self.assertEqual((cell["attempts"], cell["replies"], cell["n"], cell["no_reply"], cell["unscored_replies"]),
                         (3, 2, 0, 1, 2))
        self.assertEqual(cell["moves"], [("read_file", 2)])
        self.assertIsNone(cell["rate"])
        self.assertIsNone(cell["hi"])

    def test_malformed_call_arguments_do_not_fabricate_decoded_or_answer_moves(self):
        for changes in ({"calls": []}, {"calls": [None]}, {"args": "ledger://fixture"},
                        {"args": [None]}, {"args": []}):
            with self.subTest(changes=changes):
                cell = decide.aggregate([decision(**changes)])[0]
                self.assertEqual((cell["replies"], cell["decoded"], cell["n"]), (0, 0, 0))
        cell = decide.aggregate([decision(calls=[], args=[])])[0]
        self.assertEqual((cell["replies"], cell["n"], cell["moves"]), (1, 1, [("answer", 1)]))

    def test_numeric_invalids_are_unknown_and_measured_zeros_survive(self):
        for invalid in (True, -1, float("nan"), float("inf"), "0"):
            with self.subTest(invalid=invalid):
                cell = decide.aggregate([decision(prompt=invalid)])[0]
                self.assertIsNone(cell["prompt"])
                self.assertEqual(cell["metric_coverage"]["prompt"]["reported"], 0)
                json.dumps(cell, allow_nan=False)
        cell = decide.aggregate([decision(prompt=0, completion=0, reasoning=0, secs=0)])[0]
        self.assertEqual([cell[k] for k in ("prompt", "completion", "reasoning", "secs")], [0, 0, 0, 0])

    def test_complete_decision_values_and_order_are_compatible(self):
        rows = [decision(arm="intro"), decision(arm="bare", match=False, calls=[], args=[]),
                decision(arm="intro", i=1, prompt=200, completion=20, secs=3,
                         calls=["read_file", "read_file"], args=["ledger://fixture", "ledger://fixture"])]
        cells = decide.aggregate(rows)
        self.assertEqual([cell["arm"] for cell in cells], ["bare", "intro"])
        cell = cells[1]
        self.assertEqual((cell["n"], cell["match"], cell["rate"], cell["decoded"]), (2, 2, 1, 1))
        self.assertEqual((cell["prompt"], cell["completion"], cell["reasoning"], cell["secs"]), (150, 15, 0, 2))
        self.assertEqual(cell["moves"], [("read_file", 1), ("read ledger ×2", 1)])
        self.assertEqual((cell["lo"], cell["hi"]), decide.wilson(2, 2))
        self.assertNotIn("attempts=", decide.summary(cell))

    def test_unknown_decision_cohorts_are_visible_after_known_cells(self):
        cells = decide.aggregate([decision(model="new-model"), decision(), decision(scenario=None)])
        self.assertEqual([cell["model"] for cell in cells], ["glm-flash", "new-model", "glm-flash"])
        self.assertEqual(cells[-1]["scenario"], "unknown")
        self.assertEqual(sum(cell["attempts"] for cell in cells), 3)

    def test_loop_failed_tail_retains_attempts_and_unknown_cost(self):
        rows = [replay(), replay(cue="bare", move="edit", broke=True, prompt_tokens=110),
                {"task": "js-a", "loop_at": 5, "cue": "bare", "i": 1, "error": "URLError"}]
        cell = loop.aggregate(rows)[1]
        self.assertEqual((cell["attempts"], cell["replies"], cell["errors"], cell["no_reply"]), (2, 1, 1, 1))
        self.assertIsNone(cell["cost"])
        self.assertEqual(cell["reported_cost"], 10)
        self.assertEqual(cell["cost_coverage"], {"reported": 1, "attempts": 2, "complete": False})
        self.assertIn("attempts=2; replies=1; errors=1", loop.table(loop.aggregate(rows)))

    def test_loop_missing_or_invalid_prompt_cannot_be_cost_zero(self):
        for value in (None, True, -1, float("nan"), float("inf")):
            for changes in ({"prompt_tokens": value}, {"cue": "bare", "prompt_tokens": value}):
                with self.subTest(changes=changes):
                    rows = [replay(), replay(**changes)] if changes.get("cue") == "bare" else [replay(**changes)]
                    cell = loop.aggregate(rows)[-1]
                    self.assertIsNone(cell["cost"])
                    self.assertEqual(cell["cost_coverage"]["reported"], 0)
                    self.assertEqual(cell["issues"][0]["reason"], "missing/invalid prompt measurement")
        cells = loop.aggregate([replay(prompt_tokens=0), replay(cue="bare", prompt_tokens=0)])
        self.assertEqual([cell["cost"] for cell in cells], [0, 0])

    def test_loop_duplicate_raw_or_cue_pairing_is_diagnosed(self):
        for rows in ([replay(), replay(), replay(cue="bare", prompt_tokens=110)],
                     [replay(), replay(cue="bare", prompt_tokens=110), replay(cue="bare", prompt_tokens=120)]):
            cell = loop.aggregate(rows)[1]
            self.assertIsNone(cell["cost"])
            self.assertEqual(cell["cost_coverage"]["reported"], 0)
            self.assertTrue(all(issue["reason"] == "ambiguous raw/cue sample identity" for issue in cell["issues"]))
            self.assertIn("ambiguous raw/cue sample identity", loop.table(loop.aggregate(rows)))

    def test_loop_missing_identity_or_baseline_does_not_create_a_pair(self):
        for changes in ({"task": None}, {"i": True}, {"i": -1}, {"loop_at": True}, {"task": "js-other"}):
            with self.subTest(changes=changes):
                cells = loop.aggregate([replay(), replay(cue="bare", **changes)])
                cell = next(cell for cell in cells if cell["cue"] == "bare")
                self.assertIsNone(cell["cost"])
                self.assertEqual(cell["cost_coverage"]["reported"], 0)

    def test_loop_unknown_or_conflicting_reply_does_not_become_broken_loop(self):
        cells = loop.aggregate([replay(cue="bare", move=None, broke=True),
                                replay(cue="bare", i=1, move="repeat", broke=True),
                                replay(cue="bare", i=2, move="edit", broke=None)])
        self.assertEqual((cells[0]["attempts"], cells[0]["replies"], cells[0]["broke"], cells[0]["moves"]), (3, 0, 0, {}))

    def test_complete_loop_text_and_even_upper_middle_cost_remain_compatible(self):
        rows = [replay(), replay(i=1), replay(cue="bare", prompt_tokens=110, move="edit", broke=True),
                replay(cue="bare", i=1, prompt_tokens=120, move="other_call", broke=True)]
        self.assertEqual(loop.table(loop.aggregate(rows)),
                         "\nafter repeat 5\ncue              broke  edit  other  answer  cost (tok)\n"
                         "raw              0/2       0      0       0           0\n"
                         "bare             2/2       1      1       0          20\n")

    def test_cli_error_cell_and_loop_audit_complete_without_network(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "fixture.jsonl"
            path.write_text(json.dumps({"scenario": "batch", "model": "glm-flash", "arm": "intro", "error": "URLError"}) + "\n")
            result = subprocess.run([sys.executable, str(DIRECTORY / "decide_table.py"), str(path)],
                                    capture_output=True, text=True, timeout=3)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout)[0]["attempts"], 1)
            path.write_text(json.dumps({"task": "js-a", "cue": "bare", "i": 0, "error": "URLError"}) + "\n")
            result = subprocess.run([sys.executable, str(DIRECTORY / "loop_table.py"), "--audit-json", str(path)],
                                    capture_output=True, text=True, timeout=3)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout)[0]["errors"], 1)

    def test_malformed_json_refuses_output_and_identifies_line(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "fixture.jsonl"
            path.write_text(json.dumps(decision()) + "\n{broken\n")
            for name in ("decide_table", "loop_table"):
                result = subprocess.run([sys.executable, str(DIRECTORY / (name + ".py")), str(path)],
                                        capture_output=True, text=True, timeout=3)
                self.assertEqual(result.returncode, 2)
                self.assertEqual(result.stdout, "")
                self.assertIn(str(path) + ":2", result.stderr)


if __name__ == "__main__":
    unittest.main()
