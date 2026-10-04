import copy
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("slop_runner", ROOT / "cockpit/research/sloptomizer/runner.py")
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


def event(ident, verdict="failed", route="sol", receipt="red"):
    return dict(id=ident, check="check-a", receipt=receipt, tool="run_tests", route=route,
                verdict=verdict, hypothesis="boundary unchecked", expected="raise")


class LiveRelations(unittest.TestCase):
    def test_negative_and_positive_receipts_survive_model_change_without_fitness_credit(self):
        first = runner.execute({"action": "relate", "events": [event("1")]})
        result = runner.execute({"state": first["state"], "action": "relate",
                                 "events": [event("2", "passed", "astra", "green")]})
        row = result["advice"]["checks"][0]
        self.assertEqual((row["failed"], row["passed"]), ("1", "2"))
        self.assertEqual(result["advice"]["signals"][0]["kind"], "contrast")
        self.assertEqual(result["state"]["observations"], [])
        self.assertIsNone(result["state"]["learner"])
        self.assertEqual(result["advice"]["contrast_count"], 1)

    def test_idempotent_reload_conflict_and_inconclusive(self):
        first = runner.execute({"action": "relate", "events": [event("1", "inconclusive")]})
        before = copy.deepcopy(first["state"])
        replay = runner.execute({"state": before, "action": "relate", "events": [event("1", "inconclusive")]})
        self.assertFalse(replay["changed"])
        self.assertEqual(replay["advice"]["inconclusive_count"], 1)
        with self.assertRaisesRegex(ValueError, "conflicting"):
            runner.execute({"state": before, "action": "relate", "events": [event("1", "passed")]})
        self.assertEqual(before, first["state"])
        read = runner.execute({"state": before, "action": "context"})
        self.assertFalse(read["changed"])
        self.assertEqual(read["advice"]["signals"], [])

    def test_repetition_speaks_at_evidence_changes_and_never_restricts_exploration(self):
        state = None
        signalled = []
        for i in range(1, 18):
            result = runner.execute({"state": state, "action": "relate", "events": [event(str(i))]})
            state = result["state"]
            if result["advice"]["signals"]:
                signalled.append(i)
            self.assertTrue(result["advice"]["advisory"])
        self.assertEqual(signalled, [2, 4, 8, 16])
        self.assertEqual(len(state["relations"]), 17)
        self.assertFalse(any(k in result for k in ("stop", "budget", "cancel", "choose")))

    def test_changed_hypothesis_is_new_information_and_context_stays_small(self):
        events = [dict(event(str(i)), check=f"check-{i}") for i in range(200)]
        result = runner.execute({"action": "relate", "events": events})
        self.assertEqual(len(result["advice"]["checks"]), 6)
        self.assertEqual(result["advice"]["check_count"], 200)
        self.assertEqual(len(result["state"]["relations"]), 200)
        changed = dict(event("201"), check="check-199", hypothesis="new explanation")
        next_result = runner.execute({"state": result["state"], "action": "relate", "events": [changed]})
        self.assertEqual(next_result["advice"]["checks"][0]["repeat"], 1)

    def test_changing_failure_receipts_keep_a_distinct_verdict_run(self):
        state = None
        signalled = []
        for i in range(1, 18):
            result = runner.execute({"state": state, "action": "relate", "events": [
                event(str(i), receipt=f"different-output-{i}")
            ]})
            state = result["state"]
            self.assertEqual(result["advice"]["checks"][0]["repeat"], 1)
            for signal in result["advice"]["signals"]:
                signalled.append((i, signal["kind"]))
        self.assertEqual(signalled, [(4, "failure-run"), (8, "failure-run"), (16, "failure-run")])
        self.assertEqual(result["advice"]["checks"][0]["verdict_run"], 17)
        self.assertEqual(len({row["receipt"] for row in state["relations"]}), 17)
        for changed in (
            dict(event("next"), hypothesis="new explanation"),
            dict(event("next"), expected="new prediction"),
            event("next", route="glm"),
            event("next", verdict="inconclusive"),
        ):
            reset = runner.execute({"state": state, "action": "relate", "events": [changed]})
            self.assertEqual(reset["advice"]["checks"][0]["verdict_run"], 1)
            self.assertFalse(reset["advice"]["signals"])

    def test_changing_success_receipts_do_not_request_a_pivot(self):
        result = runner.execute({"action": "relate", "events": [
            event(str(i), verdict="passed", receipt=f"green-{i}") for i in range(20)
        ]})
        self.assertFalse(result["advice"]["signals"])

    def test_warning_keeps_its_check_when_later_quiet_checks_fill_a_batch(self):
        first = runner.execute({"action": "relate", "events": [event("first")]})
        events = [event("repeat")] + [dict(event(str(i)), check=f"quiet-{i}") for i in range(20)]
        result = runner.execute({"state": first["state"], "action": "relate", "events": events})
        self.assertEqual(result["advice"]["signals"][-1]["check"], "check-a")
        self.assertEqual(result["advice"]["checks"][0]["check"], "check-a")
        self.assertEqual(result["advice"]["checks"][0]["repeat"], 2)
        self.assertEqual(len(result["advice"]["checks"]), 6)

    def test_inconclusive_receipt_does_not_hide_a_later_conclusive_contrast(self):
        result = runner.execute({"action": "relate", "events": [
            event("red"), event("unknown", "inconclusive"), event("green", "passed")
        ]})
        self.assertEqual(result["advice"]["signals"][-1]["kind"], "contrast")
        row = result["advice"]["checks"][0]
        self.assertEqual((row["failed"], row["passed"]), ("red", "green"))
        self.assertEqual(result["advice"]["inconclusive_count"], 1)


if __name__ == "__main__":
    unittest.main()
