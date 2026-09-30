"""Report cohorts require observed clocks and unambiguous E2 count identities."""

import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[2] / "scripts/bench/book-harness/report_data.py"
spec = importlib.util.spec_from_file_location("report_clock_tests", SCRIPT)
report = importlib.util.module_from_spec(spec)
spec.loader.exec_module(report)


def timing(value=0):
    return {key: value for key in ("wall_ms", "model_ms", "tool_ms", "tool_overhead_ms",
                                   "residual_ms", "other_ms", "startup_ms")}


def run(label, wall=0, clocks=None, solved=True):
    rows = [{"usage": {"prompt_tokens": 0, "prompt_tokens_details": {"cached_tokens": 0},
                       "completion_tokens": 0}, "body": {"messages": []}, "t": 1, "done": 2}]
    return {"label": label, "host": "atlas", "requests": 1, "solved": solved,
            "wall_s": wall, "timing": clocks, **report.receipt.usage_summary(rows)}


def e2(route="glm-flash"):
    return [{"route": route, "label": f"e2-{arm}-{cycles}", "prompt_tokens": 0}
            for arm in ("prebook", "bare", "intro") for cycles in (2, 16, 33, 70)]


class ReportClockTests(unittest.TestCase):
    def build(self, runs=None, counts=None):
        return report.build_report_data({"runs": runs or {}, "e2": counts or []}, "/missing/results.json")

    def test_missing_or_invalid_solved_wall_refuses_with_actual_coverage(self):
        first, second = "e1-glm-flash-bare-1", "e1-glm-flash-bare-2"
        for value in (None, True, "2", -1, float("nan"), float("inf")):
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, "wall_s clock coverage=1/2"):
                self.build({first: run(first), second: run(second, wall=value)})

    def test_unselected_unsolved_runs_do_not_change_the_established_solved_cohort(self):
        labels = ("e1-glm-flash-bare-1", "e1-glm-flash-bare-2", "e5-glm-flash-head-1", "e5-glm-flash-head-2")
        runs = {labels[0]: run(labels[0]), labels[1]: run(labels[1], wall=None, solved=False),
                labels[2]: run(labels[2], clocks=timing()), labels[3]: run(labels[3], clocks=None, solved=False)}
        result = self.build(runs)
        for cell, key in ((result["e1"][0], "wall"), (result["e5"][0], "wall_ms")):
            self.assertEqual((cell["solved"], cell["n"], cell[key]["n"]), (1, 2, 1))
            self.assertEqual(cell[key]["median"], 0)

    def test_missing_solved_timing_cannot_shrink_the_e5_cohort(self):
        first, second = "e5-glm-flash-head-1", "e5-glm-flash-head-2"
        with self.assertRaisesRegex(ValueError, "E5 glm-flash/head wall_ms clock coverage=1/2"):
            self.build({first: run(first, clocks=timing()), second: run(second)})

    def test_each_rendered_e5_clock_and_harness_component_requires_observation(self):
        label = "e5-glm-flash-head-1"
        for field in timing():
            clocks = timing()
            clocks.pop(field)
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, field + " clock coverage=0/1"):
                self.build({label: run(label, clocks=clocks)})

    def test_complete_zero_clocks_and_cohort_statistics_remain_measured(self):
        labels = ("e5-glm-flash-head-1", "e5-glm-flash-head-2")
        result = self.build({labels[0]: run(labels[0], clocks=timing()),
                             labels[1]: run(labels[1], clocks=timing(2))})["e5"][0]
        self.assertEqual(result["wall_ms"], {"median": 1, "min": 0, "max": 2, "n": 2})
        self.assertEqual(result["harness_ms"], {"median": 3, "min": 0, "max": 6, "n": 2})

    def test_stub_startup_and_x6_do_not_filter_missing_clock_runs(self):
        for prefix, field in (("base-", "wall_ms"), ("ttfr-base-", "startup_ms"),
                              ("x6-h-", "startup_ms"), ("x6-eng-", "wall_ms")):
            first, second = prefix + "1", prefix + "2"
            with self.subTest(prefix=prefix), self.assertRaisesRegex(ValueError, field + " clock coverage=1/2"):
                self.build({first: run(first, clocks=timing()), second: run(second, clocks=timing() | {field: None})})

    def test_dsab_wall_and_startup_require_all_previously_selected_runs(self):
        first, second = "dsab-chat-1", "dsab-chat-2"
        for wall, clocks, field in ((None, timing(), "wall_s"), (0, None, "startup_ms")):
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, field + " clock coverage=1/2"):
                self.build({first: run(first, clocks=timing()), second: run(second, wall=wall, clocks=clocks)})

    def test_derived_clock_overflow_refuses_before_json_export(self):
        label = "e5-glm-flash-head-1"
        with self.assertRaisesRegex(ValueError, "harness clock is missing or invalid"):
            self.build({label: run(label, clocks=timing(1e308))})

    def test_duplicate_e2_identity_refuses_instead_of_selecting_the_last_count(self):
        for duplicate in ({}, {"prompt_tokens": 999}, {"prompt_tokens": None, "error_class": "URLError"}):
            rows = e2()
            rows.append(rows[0] | duplicate)
            original = copy.deepcopy(rows)
            with self.subTest(duplicate=duplicate), self.assertRaisesRegex(ValueError, "duplicate E2 count identity glm-flash/e2-prebook-2"):
                self.build(counts=rows)
            self.assertEqual(rows, original)

    def test_same_label_on_distinct_routes_and_complete_zero_grid_stay_eligible(self):
        result = self.build(counts=e2() + e2("deepseek-flash"))["e2"]
        self.assertEqual([cell["model"] for cell in result], ["glm-flash", "deepseek-flash"])
        for cell in result:
            self.assertEqual([point["requests"] for point in cell["points"]], [9, 51, 102, 213])
            self.assertTrue(all(point[arm] == 0 for point in cell["points"] for arm in ("prebook", "bare", "intro")))

    def test_cli_refusal_cannot_emit_a_partial_report(self):
        label = "e1-glm-flash-bare-1"
        cases = ({"runs": {label: run(label, wall=None)}, "e2": []},
                 {"runs": {}, "e2": e2() + [e2()[0]]})
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "results.json"
            for data in cases:
                path.write_text(json.dumps(data))
                original = path.read_bytes()
                result = subprocess.run([sys.executable, str(SCRIPT), str(path)], text=True,
                                        capture_output=True, timeout=3)
                self.assertEqual(result.returncode, 2)
                self.assertEqual(result.stdout, "")
                self.assertIn("unsafe report export", result.stderr)
                self.assertEqual(path.read_bytes(), original)


if __name__ == "__main__":
    unittest.main()
