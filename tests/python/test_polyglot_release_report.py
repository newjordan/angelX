"""Release exports require actual catalog coverage and observed score/timing."""
import contextlib
import copy
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tempfile
import types
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("polyglot_release_report_test", ROOT / "scripts/build_release_report.py")
report = importlib.util.module_from_spec(spec)
spec.loader.exec_module(report)


def record(task, score=1, wall=2, usage=None):
    usage = {"prompt_tokens": 100, "cached_input_tokens": 20,
             "completion_tokens": 10, "reasoning_tokens": 0} if usage is None else usage
    return {"task": {"data": {"name": task}}, "traces": [{
        "task": {"data": {"name": task}}, "calls": [{"usage": usage}],
        "timing": {"agent": {"start": 0, "end": wall}},
        "rewards": {"technical_outcome": {"score": score}},
        "info": {"agent_exit": {"exit_code": 0, "timed_out": False, "wall_cap": {"secs": 600}},
                 "agent_isolation": {"mode": "fresh-home"}, "heldout_verification": {"integrity_pass": True}}}]}


class ReleaseReportTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="angel-release-report-test-")
        self.addCleanup(self.temp.cleanup)
        self.bench = Path(self.temp.name)
        self.catalog = ["js-a", "py-b"]
        (self.bench / "tasks-polyglot-v1.json").write_text(json.dumps([{"name": name} for name in self.catalog]))
        self.run = self.bench / "runs/deepseek/angelx/seed0-fixture"
        (self.run / "configs/resolved").mkdir(parents=True)
        self.config = self.run / "configs/resolved/eval.json"
        self.config.write_text('{"num_rollouts": 1}')
        self.cell = {"model": "deepseek", "harness": "angelx", "run": "seed0-fixture", "status": "complete",
                     "config": {"model_id": "deepseek-flash", "reasoning": "off", "temperature": 0,
                                "bin_sha256": "a" * 64, "source_sha256": "b" * 64}, "metered": True,
                     "summary": {"solved": 99, "wall_total_s": 0}, "solved_wall_s": [0], "unsolved": []}
        self.board = types.SimpleNamespace(TASKS=2, HARNESS_NAMES={"angelx": "angelX"},
                                           collect=lambda model=None: [self.cell] if model is None else [])
        self.write([record("js-a"), record("py-b", score=0, wall=3)])

    def write(self, records):
        (self.run / "traces.jsonl").write_text("\n".join(map(json.dumps, records)) + "\n")

    def build(self):
        with mock.patch.object(report, "load_board", return_value=self.board):
            return report.build_data(self.bench)

    def test_valid_raw_snapshot_overrides_zero_filled_or_stale_board_summaries(self):
        data = self.build()
        model = data["models"][0]
        self.assertEqual((model["solved"], model["tasks"], model["wall_total_s"], model["wall_median_s"]), (1, 2, 5, 2.5))
        self.assertEqual(model["attempts"], [{"task": "js-a", "language": "javascript", "solved": True, "wall_s": 2},
                                             {"task": "py-b", "language": "python", "solved": False, "wall_s": 3}])
        self.assertEqual((model["input_tokens"], model["uncached_input_tokens"], model["output_tokens"],
                          model["reasoning_tokens"], model["cache_hit"], model["calls_per_task"]), (240, 200, 20, 0, 1 / 6, 1))
        self.assertEqual(model["wall_caps_s"], [600])
        self.assertEqual(model["isolation"], ["fresh-home"])
        self.assertEqual(data["suite"]["task_names"], self.catalog)
        self.assertEqual(model["trace_sha256"], hashlib.sha256((self.run / "traces.jsonl").read_bytes()).hexdigest())

    def test_duplicate_missing_unexpected_or_unnamed_attempts_refuse_complete_status(self):
        for records in ([record("js-a"), record("js-a")], [record("js-a")],
                        [record("js-a"), record("other")], [record("js-a"), record(None)]):
            self.write(records)
            with self.subTest(records=len(records)), self.assertRaisesRegex(ValueError, "unsafe cohort"):
                self.build()

    def test_catalog_and_board_count_conflicts_refuse(self):
        self.board.TASKS = 3
        with self.assertRaisesRegex(ValueError, "board task count conflicts"):
            self.build()
        for tasks in ([], [{"name": "js-a"}, {"name": "js-a"}], [{"name": None}], {}):
            (self.bench / "tasks-polyglot-v1.json").write_text(json.dumps(tasks))
            with self.subTest(tasks=tasks), self.assertRaisesRegex(ValueError, "catalog"):
                self.build()

    def test_unobserved_nonbinary_and_invalid_scores_refuse(self):
        for score in (None, True, float("nan"), float("inf"), "1", 0.5, -1):
            self.write([record("js-a", score=score), record("py-b")])
            with self.subTest(score=score), self.assertRaisesRegex(ValueError, "observed binary score"):
                self.build()

    def test_missing_reversed_and_invalid_agent_clocks_refuse(self):
        for clock in ({}, {"start": 0}, {"start": 3, "end": 2}, {"start": True, "end": 2},
                      {"start": 0, "end": float("nan")}):
            item = record("js-a")
            item["traces"][0]["timing"]["agent"] = clock
            self.write([item, record("py-b")])
            with self.subTest(clock=clock), self.assertRaisesRegex(ValueError, "observed wall"):
                self.build()

    def test_measured_score_wall_and_usage_zeros_remain_valid(self):
        zero = dict.fromkeys(("prompt_tokens", "cached_input_tokens", "completion_tokens", "reasoning_tokens"), 0)
        self.write([record("js-a", score=0, wall=0, usage=zero), record("py-b", score=0, wall=0, usage=zero)])
        model = self.build()["models"][0]
        self.assertEqual((model["solved"], model["wall_total_s"], model["input_tokens"], model["reasoning_tokens"]), (0, 0, 0, 0))
        self.assertIsNone(model["cache_hit"])
        self.assertTrue(all(field["complete"] for field in model["metric_coverage"].values()))

    def test_missing_usage_keeps_independent_coverage_and_subtotals(self):
        partial = record("py-b")
        partial["traces"][0]["calls"].append({"usage": None, "error": {"status_code": 503}})
        self.write([record("js-a"), partial])
        model = self.build()["models"][0]
        self.assertIsNone(model["input_tokens"])
        self.assertIsNone(model["reasoning_tokens"])
        self.assertEqual(model["reported_subtotals"]["input"], 240)
        self.assertEqual(model["metric_coverage"]["input"], {"reported": 1, "attempts": 2, "complete": False})
        self.assertEqual(model["usage_metric_coverage"]["input"], {"reported": 2, "attempts": 3, "complete": False})
        self.assertEqual(model["model_call_errors"], 1)

    def test_reasoning_absence_cannot_become_measured_zero(self):
        usage = {"prompt_tokens": 100, "cached_input_tokens": 20, "completion_tokens": 10}
        self.write([record("js-a", usage=usage), record("py-b", usage=usage)])
        model = self.build()["models"][0]
        self.assertEqual(model["input_tokens"], 240)
        self.assertIsNone(model["reasoning_tokens"])
        self.assertEqual(model["usage_metric_coverage"]["reasoning"]["reported"], 0)

    def test_declared_multiple_missing_or_boolean_rollout_settings_refuse(self):
        for setting in (None, True, 2, "1"):
            self.config.write_text(json.dumps({"num_rollouts": setting}))
            with self.subTest(setting=setting), self.assertRaisesRegex(ValueError, "num_rollouts=1"):
                self.build()

    def test_missing_attempts_malformed_records_and_conflicting_trace_identity_refuse(self):
        malformed = record("js-a")
        malformed["traces"][0]["task"]["data"]["name"] = "py-b"
        for records in ([{"task": {"data": {"name": "js-a"}}, "traces": []}, record("py-b")],
                        [malformed, record("py-b")], [None], [{"traces": [1]}]):
            self.write(records)
            with self.subTest(records=len(records)), self.assertRaises(ValueError):
                self.build()
        (self.run / "traces.jsonl").write_text('{broken\n')
        with self.assertRaisesRegex(ValueError, "invalid trace JSON"):
            self.build()

    def test_invalid_cohort_does_not_fetch_fonts_run_renderers_or_change_destination(self):
        self.write([record("js-a"), record("js-a")])
        out = self.bench / "published"
        out.mkdir()
        markers = {name: "recorded " + name for name in ("models.json", "report.html", "post.txt")}
        for name, text in markers.items():
            (out / name).write_text(text)
        with mock.patch.object(report, "load_board", return_value=self.board):
            with mock.patch.object(report, "font_css", side_effect=AssertionError("font fetch")):
                with mock.patch.object(subprocess, "run", side_effect=AssertionError("renderer process")):
                    with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as result:
                        report.main(["--bench-root", str(self.bench), "--out", str(out)])
        self.assertEqual(result.exception.code, 2)
        self.assertEqual({p.name: p.read_text() for p in out.iterdir()}, markers)

    def test_direct_media_report_and_post_cannot_bypass_catalog_score_wall_validation(self):
        data = self.build()
        for changes in ("wall", "duplicate", "score", "catalog", "total"):
            bad = copy.deepcopy(data)
            if changes == "wall": bad["models"][0]["attempts"][1]["wall_s"] = None
            if changes == "duplicate": bad["models"][0]["attempts"][1]["task"] = "js-a"
            if changes == "score": bad["models"][0]["attempts"][1]["solved"] = None
            if changes == "catalog": bad["suite"].pop("task_names")
            if changes == "total": bad["models"][0]["wall_total_s"] = 0
            for function, args in ((report.render_media, (bad, "", self.bench, None)),
                                   (report.render_report, (bad, "", None)), (report.render_post, (bad,))):
                with self.subTest(changes=changes, function=function.__name__):
                    with mock.patch.object(report.shutil, "which", side_effect=AssertionError("renderer probe")):
                        with self.assertRaises(ValueError):
                            function(*args)

    def test_complete_media_board_keeps_historical_attempt_arrays(self):
        data = self.build()
        captured = []
        def capture(args, **kwargs):
            text = Path(args[args.index("--html") + 1]).read_text()
            board = text.split("const BOARD = ", 1)[1].split(";\n", 1)[0]
            captured.append(json.loads(board))
        with mock.patch.object(report.shutil, "which", side_effect=lambda name: "/fixture/" + name):
            with mock.patch.object(report.subprocess, "run", side_effect=capture):
                report.render_media(data, "", self.bench, None)
        self.assertEqual(len(captured), 2)
        self.assertEqual(captured[0]["models"][0]["attempts"], [[1, 2], [0, 3]])
        self.assertEqual(captured[0]["models"][0]["total_s"], 5)
        self.assertEqual(captured[0], captured[1])


if __name__ == "__main__":
    unittest.main()
