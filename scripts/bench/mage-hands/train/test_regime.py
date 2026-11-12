"""Offline contracts; no providers or GPU calls."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("regime", Path(__file__).with_name("regime.py"))
regime = importlib.util.module_from_spec(SPEC) if SPEC else None
if SPEC and SPEC.loader and Path(SPEC.origin).exists():
    SPEC.loader.exec_module(regime)


class RegimeTests(unittest.TestCase):
    def test_split_groups_do_not_depend_on_frame_or_element(self):
        self.assertTrue(hasattr(regime, "split_for"), "mixed-regime group splitter not implemented")
        groups = [regime.split_for(f"scenario-{i}") for i in range(100)]
        self.assertEqual(set(groups), {"train", "validation", "test"})
        self.assertEqual(groups, [regime.split_for(f"scenario-{i}") for i in range(100)])

    def test_crop_coordinate_round_trip(self):
        self.assertTrue(hasattr(regime, "point_in_view"), "resolution-aware coordinate transform not implemented")
        self.assertEqual(regime.point_in_view([100, 200, 300, 400], [0, 100, 800, 600], 1600, 900), [250, 333])
        self.assertEqual(regime.point_in_view([0, 0, 1919, 1079], None, 1920, 1080), [500, 500])

    def test_overlapping_capture_is_rejected(self):
        self.assertTrue(hasattr(regime, "audit_pages"), "cross-split leakage checker not implemented")
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / "a.png"
            p.write_bytes(b"same capture")
            pages = [{"shot": str(p), "split": split, "group": split, "family": "shopping"} for split in ("train", "test")]
            with self.assertRaisesRegex(ValueError, "image.*split"):
                regime.audit_pages(pages)

    def test_group_crossing_split_is_rejected(self):
        self.assertTrue(hasattr(regime, "audit_pages"), "cross-split leakage checker not implemented")
        with tempfile.TemporaryDirectory() as d:
            pages = []
            for i, split in enumerate(("train", "test")):
                p = Path(d) / f"{i}.png"
                p.write_bytes(str(i).encode())
                pages.append({"shot": str(p), "split": split, "group": "same", "family": "steam-synthetic"})
            with self.assertRaisesRegex(ValueError, "group.*split"):
                regime.audit_pages(pages)

    def test_mage_answer_is_click_only(self):
        self.assertTrue(hasattr(regime, "answer"), "hands-only answer formatter not implemented")
        text = regime.answer('the "Add to cart" button for Cedar Backpack', [123, 456])
        call = json.loads(text.split("<tool_call>\n")[1].split("\n</tool_call>")[0])
        self.assertEqual(call, {"name": "computer_use", "arguments": {"action": "click", "coordinate": [123, 456], "button": "left"}})


if __name__ == "__main__":
    unittest.main()
