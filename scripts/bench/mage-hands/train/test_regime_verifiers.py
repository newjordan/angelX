"""Offline verifier contracts; no models, services, or GPU jobs."""
import importlib.util
from pathlib import Path
import unittest


def load(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


live = load("score_live_regime")
paired = load("summarize_regime")


def evidence_row(row, arm, hit):
    # First point (1.98, 3.00) is inside the test box; failed zoom retains it.
    texts = ['<tool_call>{"arguments":{"coordinate":[20,38]}}</tool_call>', 'no point'] if hit else ['no point']
    return paired.regime.ground_record(row, arm, 'test', (100, 80), texts)


class VerifierTests(unittest.TestCase):
    def test_success_flags_without_raw_reply_evidence_are_rejected(self):
        row = {"split": "test", "family": "shopping", "screen": "a", "group": "g",
               "target": "Cart", "box": [1, 2, 3, 4], "first": True, "cascade": True,
               "point": [2, 3]}
        with self.assertRaisesRegex(ValueError, "evidence"):
            paired.paired_summary([{**row, "arm": "hands-v1"}],
                                  [{**row, "arm": "candidate"}], 1)

    def test_correct_completed_order_uses_order_not_cleared_cart(self):
        state = {"cart": {}, "orders": {"key": {"cart": {"blue-m-shirt": 2}, "total_cents": 3600, "shipping": "standard"}},
                 "view": "complete", "shipping": "standard"}
        expected = {"ordered_cart": {"blue-m-shirt": 2}, "order_count": 1, "total_cents": 3600, "final_cart": {}}
        self.assertTrue(live.score(state, expected)["pass"])
        state["orders"]["key"]["cart"] = {"blue-l-shirt": 2}
        self.assertFalse(live.score(state, expected)["pass"])
        state["orders"]["second"] = state["orders"]["key"]
        self.assertFalse(live.score(state, expected)["pass"])

    def test_exact_fake_address_is_required_when_requested(self):
        address = {"name": "Test Shopper", "street": "1 Fiction Lane", "postal": "00000"}
        state = {"cart": {}, "orders": {"key": {"cart": {"blue-m-shirt": 2},
                 "total_cents": 3600, "shipping": "standard", "address": dict(address)}},
                 "view": "complete", "shipping": "standard"}
        self.assertTrue(live.score(state, {"address": address})["pass"])
        state["orders"]["key"]["address"]["postal"] = "99999"
        self.assertFalse(live.score(state, {"address": address})["pass"])

    def test_cross_split_shared_navigation_frames_all_quarantined(self):
        nav = load("prepare_navigation")
        rows = [{"sha256": "shared", "split": "train"},
                {"sha256": "shared", "split": "test"},
                {"sha256": "unique", "split": "test"}]
        eligible, quarantined = nav.eligible_pages(rows)
        self.assertEqual(eligible, [rows[2]])
        self.assertEqual(quarantined, rows[:2])

    def test_infra_failure_is_not_task_success(self):
        self.assertEqual(live.score({}, {}, 1)["status"], "infra-blocked")
        self.assertEqual(live.score(None, {})["status"], "infra-blocked")

    def test_stopping_before_order_requires_review_state(self):
        state = {"cart": {"cedar-bag": 1}, "orders": {}, "view": "review", "shipping": "standard"}
        expected = {"cart": {"cedar-bag": 1}, "order_count": 0, "view": "review"}
        self.assertTrue(live.score(state, expected)["pass"])
        state["view"] = "cart"
        self.assertFalse(live.score(state, expected)["pass"])

    def test_incomplete_and_mismatched_pairs_rejected(self):
        row = {"split": "test", "family": "shopping", "screen": "a", "group": "g", "target": "Cart",
               "box": [1, 2, 3, 4], "first": True, "cascade": True}
        before = [evidence_row(row, "hands-v1", True)]
        after = [evidence_row(row, "candidate", True)]
        result = paired.paired_summary(before, after, 1)
        self.assertEqual(result["shopping/cascade"]["candidate"], 1)
        with self.assertRaisesRegex(ValueError, "incomplete"):
            paired.paired_summary(before, [], 1)
        after[0]["target"] = "Checkout"
        with self.assertRaisesRegex(ValueError, "identities"):
            paired.paired_summary(before, after, 1)

    def test_gained_and_lost_pairs_not_hidden_by_net_accuracy(self):
        row = {"split": "test", "family": "shopping", "screen": "a", "group": "g", "target": "Cart",
               "box": [1, 2, 3, 4]}
        before = [evidence_row(row, "hands-v1", True),
                  evidence_row({**row, "screen": "b"}, "hands-v1", False)]
        after = [evidence_row(row, "candidate", False),
                 evidence_row({**row, "screen": "b"}, "candidate", True)]
        result = paired.paired_summary(before, after, 2)["shopping/cascade"]
        self.assertEqual((result["gained"], result["lost"], result["v1"], result["candidate"]), (1, 1, 1, 1))


if __name__ == "__main__":
    unittest.main()
