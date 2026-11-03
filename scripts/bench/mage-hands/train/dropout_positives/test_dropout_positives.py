"""Focused regression tests for the isolated element-dropout add-on.

These are intentionally small.  They exercise the fail-closed rules that keep
the generated rows positive, full-frame and target-preserving; they do not run
the full artifact audit (that is `verify.py` on the actual artifact).
"""
from __future__ import annotations

import sys
import unittest
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import build  # noqa: E402
import common  # noqa: E402


def synthetic_scene():
    image = Image.new("RGB", (200, 100), (245, 245, 245))
    draw = ImageDraw.Draw(image)
    draw.rectangle([20, 20, 60, 50], fill=(200, 0, 0))       # target
    draw.rectangle([70, 20, 120, 50], fill=(0, 0, 200))     # identifying context
    draw.rectangle([140, 60, 180, 90], fill=(0, 160, 0))    # distractor
    elements = [
        {"target": "the target", "box": [20, 20, 60, 50]},
        {"text": "Email address field", "box": [70, 20, 120, 50]},
        {"text": "Decorative note", "box": [140, 60, 180, 90]},
    ]
    return image, elements


class PreserveTargetTests(unittest.TestCase):
    def test_protected_target_and_context_are_not_candidates(self):
        image, elements = synthetic_scene()
        matched = common.find_target_element(elements, [20, 20, 60, 50], 200, 100)
        self.assertIsNotNone(matched)
        target_index, _target, _iou = matched
        candidates, protected = common.candidate_elements(
            elements, target_index, [20, 20, 60, 50], "the Email address field", 200, 100
        )
        candidate_indices = {c["element_index"] for c in candidates}
        self.assertEqual(candidate_indices, {2})
        protected_boxes = [tuple(region["box"]) for region in protected]

        def contains(outer, inner):
            return outer[0] <= inner[0] and outer[1] <= inner[1] and outer[2] >= inner[2] and outer[3] >= inner[3]

        for box in ([20, 20, 60, 50], [70, 20, 120, 50]):
            self.assertTrue(any(contains(outer, box) for outer in protected_boxes))
        self.assertFalse(any(contains(outer, [140, 60, 180, 90]) for outer in protected_boxes))

    def test_render_changes_only_dropped_pixels(self):
        image, elements = synthetic_scene()
        matched = common.find_target_element(elements, [20, 20, 60, 50], 200, 100)
        target_index = matched[0]
        candidates, _protected = common.candidate_elements(
            elements, target_index, [20, 20, 60, 50], "the Email address field", 200, 100
        )
        self.assertEqual(len(candidates), 1)
        drop = candidates[0]
        generated = image.copy()
        fill = build.median_ring_color(image, tuple(drop["box"]), 200, 100)
        ImageDraw.Draw(generated).rectangle(
            [drop["box"][0], drop["box"][1], drop["box"][2] - 1, drop["box"][3] - 1], fill=fill
        )
        for box in ([20, 20, 60, 50], [70, 20, 120, 50]):
            self.assertIsNone(ImageChops.difference(image.crop(box), generated.crop(box)).getbbox())
        self.assertIsNotNone(ImageChops.difference(
            image.crop(tuple(drop["box"])), generated.crop(tuple(drop["box"]))
        ).getbbox())
        changed, leak, count = build.diff_stats(image, generated, [tuple(drop["box"])])
        self.assertIsNone(leak.getbbox())
        self.assertGreater(count, 0)

    def test_target_overlapping_element_is_not_a_candidate(self):
        image, elements = synthetic_scene()
        elements.append({"text": "Overlapping target copy", "box": [50, 30, 70, 60]})
        matched = common.find_target_element(elements, [20, 20, 60, 50], 200, 100)
        self.assertIsNotNone(matched)
        candidates, _protected = common.candidate_elements(
            elements, matched[0], [20, 20, 60, 50], "the target", 200, 100
        )
        boxes = [tuple(c["box"]) for c in candidates]
        self.assertNotIn((50, 30, 70, 60), boxes)
        self.assertEqual(len(candidates), 1)

    def test_ambiguous_target_is_not_guessed(self):
        elements = [
            {"text": "same", "box": [10, 10, 50, 40]},
            {"text": "same", "box": [10, 10, 50, 40]},
        ]
        self.assertIsNone(common.find_target_element(elements, [10, 10, 50, 40], 200, 100))


class RejectUnsafeInputTests(unittest.TestCase):
    def test_validation_row_is_rejected(self):
        row = {
            "screen": "screens/whatever.png",
            "target": "the target",
            "box": [1, 1, 2, 2],
            "family": "shopping",
            "group": "g",
            "split": "validation",
            "width": 10,
            "height": 10,
            "crop": None,
            "zoom": 1,
            "answer": "x",
            "entities": [],
            "label_id": "x",
        }
        context, reason = build.validate_parent(row, "{}", Path("/nonexistent"), {}, None)
        self.assertIsNone(context)
        self.assertEqual(reason, "row_not_train")

    def test_crop_row_is_rejected(self):
        row = {
            "screen": "screens/whatever.png",
            "target": "the target",
            "box": [1, 1, 2, 2],
            "family": "shopping",
            "group": "g",
            "split": "train",
            "width": 10,
            "height": 10,
            "crop": [0, 0, 5, 5],
            "zoom": 2,
            "answer": "x",
            "entities": [],
            "label_id": "x",
        }
        context, reason = build.validate_parent(row, "{}", Path("/nonexistent"), {}, None)
        self.assertIsNone(context)
        self.assertEqual(reason, "not_full_frame")

    def test_non_click_answer_is_rejected(self):
        with self.assertRaises(ValueError):
            common.parse_click('Action: Click x.\n\n<tool_call>{"name":"computer_use","arguments":{"action":"move","coordinate":[1,2]}}</tool_call>')
        with self.assertRaises(ValueError):
            common.parse_click('Action: no click')

    def test_click_must_project_inside_box(self):
        row = {
            "width": 200,
            "height": 100,
            "crop": None,
            "box": [10, 10, 50, 40],
            "answer": 'Action: Click x.\n\n<tool_call>{"name":"computer_use","arguments":{"action":"click","button":"left","coordinate":[900,900]}}</tool_call>',
        }
        self.assertFalse(common.click_inside_box(row))


if __name__ == "__main__":
    unittest.main()
