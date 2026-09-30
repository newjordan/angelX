"""Offline checks for cohort integrity and honest pilot telemetry."""
import importlib.util
import ctypes
import json
import os
from pathlib import Path
import shutil
import subprocess
import signal
import sys
import tempfile
import time
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]


def load_script(name):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), ROOT / "scripts" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


runner = load_script("run-polyglot-loop")
summary = load_script("summarize-polyglot-loop")
history = load_script("analyze-polyglot-history")
shapes = load_script("analyze-qwen-traces")


class CohortTests(unittest.TestCase):
    def test_selection_balanced_stable_and_independent_of_outcomes(self):
        rows = [{"name": f"{lang}-{idx}", "prompt": "original", "reward": 0}
                for lang in runner.LANGUAGES for idx in range(5)]
        selected = runner.select_tasks(rows, 3, [])
        changed = [{**row, "prompt": "different", "reward": 1} for row in reversed(rows)]
        again = runner.select_tasks(changed, 3, [])
        self.assertEqual([r["name"] for r in selected], [r["name"] for r in again])
        self.assertEqual(len(selected), 12)
        for lang in runner.LANGUAGES:
            self.assertEqual(sum(r["name"].startswith(lang + "-") for r in selected), 3)
        self.assertEqual(selected[0]["prompt"], "original")

    def test_explicit_selection_rejects_missing_or_duplicate_tasks(self):
        rows = [{"name": "js-a"}, {"name": "py-a"}]
        for names in (["missing"], ["js-a", "js-a"]):
            with self.subTest(names=names), self.assertRaises(ValueError):
                runner.select_tasks(rows, 3, names)
        self.assertEqual(runner.select_tasks(rows, 3, ["py-a", "js-a"]), list(reversed(rows)))

    def test_fixture_snapshot_is_confined_and_independent(self):
        with tempfile.TemporaryDirectory() as tmp:
            bench, output = Path(tmp) / "old", Path(tmp) / "new"
            source = bench / "fixtures/js-a"
            source.mkdir(parents=True)
            (source / "solution.js").write_text("original")
            runner.snapshot_fixtures(bench, output, [{"name": "js-a", "fixture": "fixtures/js-a"}])
            target = output / "fixtures/js-a/solution.js"
            self.assertTrue(target.resolve().is_relative_to(output))
            target.write_text("modified")
            self.assertEqual((source / "solution.js").read_text(), "original")
            for fixture in ("../old/fixtures/js-a", str(source), "not-fixtures/js-a"):
                with self.subTest(fixture=fixture), self.assertRaises(ValueError):
                    runner.snapshot_fixtures(bench, output, [{"name": "js-a", "fixture": fixture}])

    def test_fixture_snapshot_rejects_empty_roots_and_external_symlinks(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            bench, output = root / "bench", root / "output"
            fixture = bench / "fixtures/js-a"
            fixture.mkdir(parents=True)
            outside = root / "private.txt"
            outside.write_text("not fixture content")
            for name in ("", ".", "fixtures"):
                with self.subTest(name=name), self.assertRaises(ValueError):
                    runner.snapshot_fixtures(bench, output, [{"name": "js-a", "fixture": name}])
            (fixture / "solution.js").symlink_to(outside)
            with self.assertRaisesRegex(ValueError, "symlink"):
                runner.snapshot_fixtures(bench, output, [{"name": "js-a", "fixture": "fixtures/js-a"}])
            self.assertFalse(output.exists())
            (fixture / "solution.js").unlink()
            (bench / "fixtures/linked").symlink_to(fixture, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "symlink"):
                runner.snapshot_fixtures(bench, output, [{"name": "js-a", "fixture": "fixtures/linked"}])

    def test_gold_is_pinned_outside_candidate_fixture_trees(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            bench, output = root / "bench", root / "output"
            fixture = bench / "fixtures/js-a"
            gold = bench / "gold/js-a"
            fixture.mkdir(parents=True)
            gold.mkdir(parents=True)
            (fixture / "solution.js").write_text("candidate")
            (gold / "solution.js").write_text("gold")
            selected = [{"name": "js-a", "fixture": "fixtures/js-a", "gold": "js-a"}]
            fixture_manifest = runner.snapshot_fixtures(bench, output, selected)
            private = output / "private-grading"
            gold_manifest = runner.snapshot_gold(bench, private, selected)
            (fixture / "solution.js").write_text("changed original candidate")
            (gold / "solution.js").write_text("changed original gold")
            self.assertEqual((output / "fixtures/js-a/solution.js").read_text(), "candidate")
            self.assertEqual((private / "gold/js-a/solution.js").read_text(), "gold")
            self.assertEqual(fixture_manifest["js-a"], runner.tree_manifest(output / "fixtures/js-a"))
            self.assertEqual(gold_manifest["js-a"], runner.tree_manifest(private / "gold/js-a"))
            self.assertFalse((output / "fixtures/js-a/gold").exists())

    def test_copy_snapshot_rejects_mutation_and_keeps_cache_rules_consistent(self):
        with tempfile.TemporaryDirectory() as tmp:
            source, target = Path(tmp) / "source", Path(tmp) / "target"
            source.mkdir()
            file = source / "tool.py"
            file.write_text("before")
            (source / "module.pyc").write_bytes(b"cache")
            original_copy = shutil.copytree

            def changing_copy(*args, **kwargs):
                result = original_copy(*args, **kwargs)
                file.write_text("after")
                return result

            with mock.patch.object(runner.shutil, "copytree", side_effect=changing_copy):
                with self.assertRaisesRegex(ValueError, "changed"):
                    runner.copy_tree_snapshot(source, target)
            self.assertFalse((target / "module.pyc").exists())
            other = Path(tmp) / "other"
            manifest = runner.copy_tree_snapshot(source, other)
            self.assertEqual(manifest, {"tool.py": runner.sha256(file)})

    def test_resource_source_binding_rejects_dirty_and_mismatched_checkouts(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp)
            files = {"cockpit/Cargo.toml": "cockpit", "vendor/dotmax/Cargo.toml": "dotmax",
                     "vendor/ureq/Cargo.toml": "ureq", "rust-toolchain.toml": "toolchain",
                     "docs/telemetry/model-calibration.toml": "models",
                     "docs/telemetry/store-caps.toml": "caps", "docs/telemetry/prices.toml": "prices"}
            for name, content in files.items():
                path = source / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
            script = source / "scripts/check/cockpit-source-digest.sh"
            script.parent.mkdir(parents=True)
            shutil.copyfile(ROOT / "scripts/check/cockpit-source-digest.sh", script)
            for args in (["init", "-q"], ["add", "."],
                         ["-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "-qm", "source pin"]):
                subprocess.run(["git", "-C", str(source), *args], check=True, capture_output=True)
            digest = subprocess.check_output(["bash", str(script)], text=True).strip()
            identity = runner.checked_resource_source(source, digest)
            self.assertEqual(identity["cockpit_source_sha256"], digest)
            self.assertEqual(identity["digest_script_sha256"], runner.sha256(ROOT / "scripts/check/cockpit-source-digest.sh"))
            nested_script = source / "nested/scripts/check/cockpit-source-digest.sh"
            nested_script.parent.mkdir(parents=True)
            shutil.copyfile(script, nested_script)
            with self.assertRaisesRegex(ValueError, "top-level"):
                runner.checked_resource_source(source / "nested", digest)
            with self.assertRaisesRegex(ValueError, "does not match"):
                runner.checked_resource_source(source, "0" * 64)
            (source / "cockpit/Cargo.toml").write_text("dirty")
            with self.assertRaisesRegex(ValueError, "not source-bound"):
                runner.checked_resource_source(source, digest)

    def test_resource_source_binding_rejects_forged_missing_and_unsupported_helpers(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp)
            subprocess.run(["git", "-C", str(source), "init", "-q"], check=True, capture_output=True)
            script = source / "scripts/check/cockpit-source-digest.sh"
            script.parent.mkdir(parents=True)
            script.write_text('#!/bin/bash\ntouch "$(dirname "$0")/executed"\nprintf "%s\\n" ' + "0" * 64 + "\n")
            with self.assertRaisesRegex(ValueError, "unsupported helper version"):
                runner.checked_resource_source(source, "0" * 64)
            self.assertFalse((script.parent / "executed").exists(), "untrusted helper must never execute")
            script.unlink()
            with self.assertRaisesRegex(ValueError, "no plain"):
                runner.checked_resource_source(source, "0" * 64)
            shutil.copyfile(ROOT / "scripts/check/cockpit-source-digest.sh", script)
            script.write_text(script.read_text() + "\n# unsupported version\n")
            with self.assertRaisesRegex(ValueError, "unsupported helper version"):
                runner.checked_resource_source(source, "0" * 64)
            shutil.copyfile(ROOT / "scripts/check/cockpit-source-digest.sh", script)
            (source / "scripts").rename(source / "saved-scripts")
            (source / "scripts").symlink_to(source / "saved-scripts", target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "symlink"):
                runner.checked_resource_source(source, "0" * 64)

    def test_resource_source_binding_rejects_helper_mutation_during_verification(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp)
            subprocess.run(["git", "-C", str(source), "init", "-q"], check=True, capture_output=True)
            script = source / "scripts/check/cockpit-source-digest.sh"
            script.parent.mkdir(parents=True)
            shutil.copyfile(ROOT / "scripts/check/cockpit-source-digest.sh", script)
            original_run = subprocess.run

            def changed_helper(*args, **kwargs):
                if args[0][0] != "bash":
                    return original_run(*args, **kwargs)
                script.write_text(script.read_text() + "\n# concurrently changed\n")
                return subprocess.CompletedProcess(args[0], 0, "0" * 64 + "\n", "")

            with mock.patch.object(runner.subprocess, "run", side_effect=changed_helper):
                with self.assertRaisesRegex(ValueError, "changed while verifying"):
                    runner.checked_resource_source(source, "0" * 64)

    def test_resource_snapshot_preserves_tracked_scripts_without_ignored_outputs(self):
        with tempfile.TemporaryDirectory() as tmp:
            source, target = Path(tmp) / "source", Path(tmp) / "resources"
            files = {".gitignore": "out/\nnode_modules/\n",
                     "scripts/runtime/worker.mjs": "runtime", "scripts/check/check.sh": "support tool",
                     "scripts/bench/run.py": "benchmark tool", "scripts/bench/out/wire.jsonl": "private output",
                     "scripts/bench/tokens/node_modules/package/index.js": "installed package",
                     "cockpit/Cargo.toml": "manifest", "cockpit/src/main.rs": "source"}
            for name, content in files.items():
                path = source / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
            subprocess.run(["git", "-C", str(source), "init", "-q"], check=True, capture_output=True)
            subprocess.run(["git", "-C", str(source), "add", "."], check=True, capture_output=True)
            manifest = runner.snapshot_resources(source, target)
            self.assertIn("scripts/runtime/worker.mjs", manifest)
            self.assertIn("scripts/check/check.sh", manifest)
            self.assertIn("scripts/bench/run.py", manifest)
            self.assertIn("cockpit/src/main.rs", manifest)
            self.assertFalse((target / "scripts/bench/out").exists())
            self.assertFalse((target / "scripts/bench/tokens/node_modules").exists())
            self.assertEqual(manifest, runner.tree_manifest(target))

    def test_environment_does_not_inherit_host_credentials_or_switches(self):
        env = runner.evaluator_environment(Path("/bench"), Path("/snapshot"), "gpt-6-luna",
                                           "medium", 600, Path("/private/auth"))
        self.assertEqual(env["OPENAI_API_KEY"], "unused-luna-direct")
        self.assertNotIn("ANGEL_YOLO", env)
        self.assertNotIn("ANGEL_RESOURCE_DIR", env)
        self.assertNotIn("META_API_KEY", env)
        self.assertEqual(env["PYTHONPATH"], "/snapshot")


@unittest.skipUnless(sys.platform.startswith("linux"), "Linux process-group cleanup")
class RunnerProcessTests(unittest.TestCase):
    def check_group_cleanup(self, leader_mode):
        # Adopt and reap the fixture grandchild even on hosts whose PID 1 does
        # not reap orphans. Restore this process-wide setting after each case.
        libc = ctypes.CDLL(None, use_errno=True)
        old_subreaper = ctypes.c_int()
        self.assertEqual(libc.prctl(37, ctypes.byref(old_subreaper), 0, 0, 0), 0)
        self.assertEqual(libc.prctl(36, 1, 0, 0, 0), 0)
        child_code = "import os,signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); print(os.getpid(),flush=True); time.sleep(60)"
        leader_code = (
            "import signal,subprocess,sys,time; "
            "signal.signal(signal.SIGTERM,signal.SIG_IGN) if sys.argv[2]=='ignore' else None; "
            "child=subprocess.Popen([sys.executable,'-c',sys.argv[1]],stdout=subprocess.PIPE,text=True); "
            "print(child.stdout.readline(),end='',flush=True); "
            "sys.exit(0) if sys.argv[2]=='exit' else time.sleep(60)"
        )
        leader, child = None, None
        try:
            leader = subprocess.Popen([sys.executable, "-c", leader_code, child_code, leader_mode],
                                      stdout=subprocess.PIPE, text=True, start_new_session=True)
            child = int(leader.stdout.readline())
            if leader_mode == "exit":
                self.assertEqual(runner.wait_evaluator_exit(leader, 5), 0)
                self.assertIsNone(leader.returncode, "exit observation must not release the leader PID")
            runner.terminate_group(leader, grace_secs=.05)
            self.assertIsNotNone(leader.returncode)
            status = None
            deadline = time.monotonic() + 3
            while time.monotonic() < deadline:
                observed, observed_status = os.waitpid(child, os.WNOHANG)
                if observed:
                    status = observed_status
                    break
                time.sleep(.01)
            self.assertIsNotNone(status, "evaluator child survived process-group cleanup")
            child = None
            self.assertTrue(os.WIFSIGNALED(status))
            self.assertEqual(os.WTERMSIG(status), signal.SIGKILL)
        finally:
            if leader is not None:
                try:
                    os.killpg(leader.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                leader.wait(timeout=5)
                leader.stdout.close()
            if child is not None:
                os.waitpid(child, 0)
            self.assertEqual(libc.prctl(36, old_subreaper.value, 0, 0, 0), 0)

    def test_child_ignoring_term_is_stopped_when_leader_exits_on_term(self):
        self.check_group_cleanup("normal")

    def test_term_ignoring_leader_and_child_are_stopped_after_grace(self):
        self.check_group_cleanup("ignore")

    def test_already_exited_leader_does_not_leave_its_child_running(self):
        self.check_group_cleanup("exit")

    def test_reaped_leader_does_not_authorize_signalling_its_old_group(self):
        process = subprocess.Popen([sys.executable, "-c", "raise SystemExit(7)"], start_new_session=True)
        try:
            self.assertEqual(runner.wait_evaluator_exit(process, 5), 7)
            self.assertIsNone(process.returncode)
            self.assertEqual(process.wait(timeout=5), 7)
            with mock.patch.object(runner.os, "killpg") as killpg:
                runner.terminate_group(process, grace_secs=.01)
                killpg.assert_not_called()
        finally:
            process.wait(timeout=5)

    def test_external_reaper_does_not_authorize_signalling_released_group(self):
        process = subprocess.Popen([sys.executable, "-c", "raise SystemExit(0)"], start_new_session=True)
        try:
            self.assertEqual(runner.wait_evaluator_exit(process, 5), 0)
            os.waitpid(process.pid, 0)
            self.assertIsNone(process.returncode)
            with mock.patch.object(runner.os, "killpg") as killpg:
                runner.terminate_group(process, grace_secs=.01)
                killpg.assert_not_called()
        finally:
            process.wait(timeout=5)


class SummaryTests(unittest.TestCase):
    def test_missing_telemetry_remains_unknown(self):
        row = summary.summarize_trace({"task": {"data": {"name": "js-a"}}}, {})
        self.assertIsNone(row["score"])
        for key in summary.METRICS:
            self.assertIsNone(row[key], key)
        result = summary.aggregate([row])
        self.assertEqual(result["scored"], 0)
        self.assertEqual(result["solved"], 0)
        self.assertIsNone(result["cache_hit_ratio"])
        self.assertIsNone(result["metrics"]["input_tokens"]["sum"])

    def test_zero_is_reported_and_agent_timing_is_distinct_from_verifier(self):
        trace = {
            "timing": {"agent": {"start": 10, "end": 12}},
            "rewards": {"technical_outcome": {"score": 1}},
            "info": {"angel": {"hops": 0, "usage": {
                "input": 100, "cache_read": 0, "output": 0, "core_complete": True,
            }}, "heldout_verification": {"verifier_elapsed_ms": 500}},
        }
        row = summary.summarize_trace({"task": {"data": {"name": "js-a"}}}, trace)
        result = summary.aggregate([row])
        self.assertEqual(row["task_wall_s"], 2)
        self.assertEqual(row["verifier_wall_s"], 0.5)
        self.assertEqual(row["hops"], 0)
        self.assertEqual(result["metrics"]["output_tokens"], {"reported": 1, "sum": 0, "median": 0})
        self.assertEqual(result["cache_hit_ratio"], 0)

    def test_partial_coverage_and_contract_mismatch_remain_visible(self):
        row = summary.summarize_trace({"task": {"data": {"name": "js-a"}}}, {})
        known = {**row, "task_wall_s": 2, "input_tokens": 100, "cached_input_tokens": 20}
        result = summary.aggregate([row, known])
        self.assertEqual(result["metrics"]["input_tokens"]["reported"], 1)
        self.assertEqual(result["cache_hit_reported_tasks"], 1)
        baseline = {"path": "a", "manifest": {"effort": "medium"}, "rows": [known]}
        candidate = {"path": "b", "manifest": {"effort": "high"},
                     "rows": [{**known, "task_wall_s": 1}]}
        comparison = summary.paired(baseline, candidate)
        self.assertEqual(comparison["contract_mismatches"], ["effort"])
        self.assertEqual(comparison["deltas_candidate_minus_baseline"][0]["task_wall_s"], -1)
        self.assertIsNone(comparison["deltas_candidate_minus_baseline"][0]["model_s"])

    def test_unscored_attempt_is_not_a_solve_gain_or_loss(self):
        unknown = summary.summarize_trace({"task": {"data": {"name": "js-a"}}}, {})
        solved = {**unknown, "score": 1, "solved": True}
        baseline = {"path": "a", "manifest": {}, "rows": [unknown]}
        candidate = {"path": "b", "manifest": {}, "rows": [solved]}
        for left, right in ((baseline, candidate), (candidate, baseline)):
            comparison = summary.paired(left, right)
            self.assertEqual(comparison["solve_gains"], [])
            self.assertEqual(comparison["solve_losses"], [])
            self.assertEqual(comparison["unscored_shared_tasks"], ["js-a"])
            self.assertEqual(comparison["jointly_scored_tasks"], 0)

    def test_duplicate_attempts_are_retained_but_not_silently_paired(self):
        unknown = summary.summarize_trace({"task": {"data": {"name": "js-a"}}}, {})
        failed = {**unknown, "score": 0, "solved": False, "task_wall_s": 600}
        solved = {**unknown, "score": 1, "solved": True, "task_wall_s": 10}
        baseline = {"path": "a", "manifest": {}, "rows": [failed, solved]}
        candidate = {"path": "b", "manifest": {}, "rows": [solved]}
        result = summary.aggregate(baseline["rows"])
        self.assertEqual(result["tasks"], 2)
        self.assertEqual(result["unique_tasks"], 1)
        self.assertEqual(result["duplicate_tasks"], {"js-a": 2})
        self.assertEqual(result["metrics"]["task_wall_s"]["sum"], 610)
        for left, right in ((baseline, candidate), (candidate, baseline)):
            comparison = summary.paired(left, right)
            self.assertEqual(comparison["ambiguous_tasks"], ["js-a"])
            self.assertEqual(comparison["shared_tasks"], 0)
            self.assertEqual(comparison["deltas_candidate_minus_baseline"], [])
            self.assertEqual(comparison["missing_from_candidate"], [])

    def test_contract_drift_is_symmetric_and_unknown_fields_are_explicit(self):
        baseline = {"path": "a", "manifest": {"effort": None, "model": "x"}, "rows": []}
        candidate = {"path": "b", "manifest": {"effort": "high", "model": "x", "wall_secs": 600}, "rows": []}
        for left, right in ((baseline, candidate), (candidate, baseline)):
            comparison = summary.paired(left, right)
            self.assertEqual(comparison["contract_mismatches"], ["effort", "wall_secs"])
            self.assertEqual(comparison["contract_unknown_fields"], ["effort", "wall_secs"])

    def test_manifest_coverage_includes_missing_empty_duplicate_and_unexpected_tasks(self):
        with tempfile.TemporaryDirectory() as tmp:
            cell = Path(tmp)
            (cell / "manifest.json").write_text(json.dumps({"task_names": ["js-a", "py-b"], "attempts": 1}))
            records = [
                {"task": {"data": {"name": "js-a"}}, "traces": [{}, {}]},
                {"task": {"data": {"name": "py-b"}}, "traces": []},
                {"task": {"data": {"name": "cpp-c"}}, "traces": [{}]},
            ]
            (cell / "traces.jsonl").write_text("\n".join(json.dumps(record) for record in records))
            result = summary.load_cell(cell)
            coverage = result["coverage"]
            self.assertFalse(coverage["cohort_complete"])
            self.assertEqual(coverage["missing_tasks"], ["py-b"])
            self.assertEqual(coverage["unexpected_tasks"], ["cpp-c"])
            self.assertEqual(coverage["attempt_count_mismatches"], {"js-a": 2, "py-b": 0})
            self.assertEqual(coverage["records_without_traces"], 1)
            self.assertEqual(coverage["expected_attempts"], 2)
            self.assertEqual([(row["line"], row["attempt"]) for row in result["rows"]], [(1, 1), (1, 2), (3, 1)])

    def test_manifest_coverage_supports_planned_repetitions_and_absent_traces(self):
        with tempfile.TemporaryDirectory() as tmp:
            cell = Path(tmp)
            (cell / "manifest.json").write_text(json.dumps({"task_names": ["js-a"], "attempts": 2}))
            result = summary.load_cell(cell)
            self.assertFalse(result["coverage"]["trace_file_present"])
            self.assertEqual(result["coverage"]["missing_tasks"], ["js-a"])
            record = {"task": {"data": {"name": "js-a"}}, "traces": [{}, {}]}
            (cell / "traces.jsonl").write_text(json.dumps(record))
            self.assertTrue(summary.load_cell(cell)["coverage"]["cohort_complete"])

    def test_nonfinite_and_reversed_timing_remain_unknown(self):
        for value in (float("nan"), float("inf"), float("-inf"), True):
            self.assertIsNone(summary.numeric(value))
        self.assertIsNone(summary.duration({"start": 10, "end": 9}))
        self.assertIsNone(summary.milliseconds(-1))

    def test_partial_envelope_counters_are_subtotals_not_complete_measurements(self):
        trace = {"info": {"angel": {"usage": {
            "attempts": 2, "input": 100, "output": 5, "cache_read": 20,
            "reported_attempts": {"input": 1, "output": 2, "cache_read": 1},
            "core_complete": False,
        }}}}
        row = summary.summarize_trace({}, trace)
        self.assertIsNone(row["input_tokens"])
        self.assertIsNone(row["cached_input_tokens"])
        self.assertEqual(row["output_tokens"], 5)
        self.assertEqual(row["usage_reported_subtotals"]["input_tokens"], 100)
        self.assertEqual(row["usage_metric_coverage"]["input_tokens"], {"reported": 1, "attempts": 2, "complete": False})
        result = summary.aggregate([row])
        self.assertIsNone(result["cache_hit_ratio"])
        self.assertEqual(result["usage_partial_tasks_by_metric"]["input_tokens"], 1)

    def test_incoherent_cache_ratio_is_not_reported(self):
        row = summary.summarize_trace({}, {"info": {"angel": {"usage": {"input": 10, "cache_read": 20}}}})
        result = summary.aggregate([row])
        self.assertIsNone(result["cache_hit_ratio"])
        self.assertEqual(result["cache_hit_invalid_tasks"], 1)


class HistoricalTraceTests(unittest.TestCase):
    def test_proxy_usage_is_uncached_plus_cached_and_absence_stays_unknown(self):
        trace = {"calls": [{"usage": {"prompt_tokens": 30, "cached_input_tokens": 70,
                                      "completion_tokens": 0}}]}
        row = history.extract_row({}, trace, "run")
        self.assertEqual((row["input"], row["uncached_input"], row["cached_input"], row["output"]), (100, 30, 70, 0))
        unknown = history.extract_row({}, {"calls": [{"usage": None}]}, "run")
        self.assertIsNone(unknown["input"])
        self.assertIsNone(unknown["output"])
        self.assertEqual(unknown["usage_calls_reported"], 0)

    def test_p90_interpolates_and_unknown_cache_has_no_percent_suffix(self):
        self.assertEqual(history.percentile([0, 10], .9), 9)
        row = history.extract_row({}, {}, "run")
        cell = {"id": "runs/qwen/angelx/test", "model_family": "qwen", "harness": "angelx",
                "contract": {}, "summary": history.aggregate([row])}
        rendered = history.markdown({"primary": [cell], "pairings": []})
        self.assertNotIn("—%", rendered)
        self.assertIn("Same recorded settings", rendered)

    def test_pairing_reports_duplicate_and_unknown_outcomes(self):
        unknown = history.extract_row({"task": {"data": {"name": "js-a"}}}, {}, "run")
        solved = {**unknown, "score": 1, "solved": True, "wall_s": 10}
        contract = {"model": "m", "sampling": {}, "wall_secs": 600}
        own = {"id": "a", "contract": contract, "rows": [solved, solved]}
        peer = {"id": "b", "contract": contract, "rows": [solved]}
        result = history.pair_cells(own, peer)
        self.assertEqual(result["ambiguous_tasks"], ["js-a"])
        self.assertEqual(result["shared"], 0)
        self.assertEqual(result["duplicate_attempts"]["angel"], {"js-a": 2})
        self.assertTrue(result["same_recorded_settings"])
        own["rows"] = [unknown]
        result = history.pair_cells(own, peer)
        self.assertEqual(result["peer_only_solved"], [])
        self.assertEqual(result["unscored_shared_tasks"], ["js-a"])
        peer["contract"] = {"model": "m", "sampling": {}}
        result = history.pair_cells(own, peer)
        self.assertFalse(result["same_recorded_settings"])
        self.assertEqual(result["contract_mismatches"], ["wall_secs"])
        self.assertEqual(result["contract_unknown_fields"], ["wall_secs"])

    def test_invalid_clock_does_not_become_negative_or_zero_overhead(self):
        self.assertIsNone(history.elapsed({"start": 10, "end": 9}))
        self.assertIsNone(history.number(float("nan")))
        trace = {"calls": [{"usage": {}}], "timing": {"agent": {
            "start": 10, "end": 12, "model": {"duration": 3}}}}
        result = history.extract_row({}, trace, "run")
        self.assertEqual(result["wall_s"], 2)
        self.assertEqual(result["model_s"], 3)
        self.assertIsNone(result["non_model_s"])

    def test_partial_proxy_usage_is_not_mistaken_for_full_task_usage(self):
        trace = {"calls": [
            {"usage": {"prompt_tokens": 30, "cached_input_tokens": 70, "completion_tokens": 0}},
            {"usage": {"prompt_tokens": 20, "completion_tokens": 5}},
        ]}
        row = history.extract_row({}, trace, "run")
        self.assertIsNone(row["input"])
        self.assertIsNone(row["cached_input"])
        self.assertEqual(row["uncached_input"], 50)
        self.assertEqual(row["output"], 5)
        self.assertEqual(row["usage_reported_subtotals"]["input"], 100)
        self.assertEqual(row["usage_metric_coverage"]["input"], {"reported": 1, "attempts": 2, "complete": False})
        result = history.aggregate([row])
        self.assertIsNone(result["cache_ratio"])
        self.assertEqual(result["usage_partial_tasks_by_metric"]["input"], 1)

    def test_direct_envelope_requires_each_metric_to_cover_all_attempts(self):
        usage = {"attempts": 2, "input": 100, "output": 0, "cache_read": 20,
                 "reported_attempts": {"input": 2, "output": 2, "cache_read": 1}}
        row = history.extract_row({}, {"info": {"angel": {"usage": usage}}}, "run")
        self.assertEqual(row["input"], 100)
        self.assertEqual(row["output"], 0)
        self.assertIsNone(row["cached_input"])
        self.assertEqual(row["usage_reported_subtotals"]["cached_input"], 20)
        usage["untracked_sources"] = True
        row = history.extract_row({}, {"info": {"angel": {"usage": usage}}}, "run")
        self.assertIsNone(row["input"])
        self.assertIsNone(row["output"])

    def test_inventory_finds_new_roots_and_excludes_runtime_caches(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            names = ["runs/deepseek/angelx/seed0-old/traces.jsonl",
                     "runs-legend/deepseek/angelx/seed0-new/traces.jsonl",
                     ".venv/cache/traces.jsonl", "node_modules/package/traces.jsonl"]
            for name in names:
                path = root / name
                path.parent.mkdir(parents=True)
                path.write_text("")
            self.assertEqual([str(path.relative_to(root)) for path in history.discover_traces(root)], names[:2])
            empty = root / "empty"
            empty.mkdir()
            self.assertEqual(history.discover_traces(empty), [])

    def test_new_roots_are_not_promoted_without_explicit_primary_selection(self):
        def cell(identity, n=136, unique=136):
            return {"id": identity, "model_family": "deepseek", "harness": "angelx",
                    "summary": {"n": n, "unique_tasks": unique, "unnamed_attempts": 0}}
        original = cell("runs/deepseek/angelx/seed0-old")
        new = cell("runs-legend/deepseek/angelx/seed0-new")
        selective = cell("runs/deepseek/angelx/smoke-new")
        repeated = cell("runs/deepseek/angelx/seed9-repeated", unique=135)
        cells = [original, new, selective, repeated]
        self.assertEqual(history.select_primary(cells), [original])
        self.assertEqual(history.select_primary(cells, ["runs-legend"]), [new])

    def test_response_shapes_count_produced_calls_not_replayed_nodes(self):
        trace = {"nodes": [
            {"message": {"role": "system", "content": "abc"}},
            {"parent": 0, "message": {"role": "assistant", "reasoning_content": "prior"}},
            {"parent": 1, "message": {"role": "tool", "content": "result"}},
            {"parent": 2, "message": {"role": "assistant", "content": "ok", "reasoning_content": "new",
                                      "tool_calls": [{"name": "read", "arguments": "{}"}]}}
        ], "calls": [{"node": 3, "time": {"start": 2, "end": 4}}, {"error": "missing response"}]}
        result, _, tools = shapes.trace_shapes(trace)
        self.assertEqual(result["attempted_calls"], 2)
        self.assertEqual(result["unobserved_response_calls"], 1)
        self.assertEqual(len(result["calls"]), 1)
        call = result["calls"][0]
        self.assertEqual(call["history_reasoning_chars"], 5)
        self.assertEqual(call["reasoning_chars"], 3)
        self.assertEqual(call["generation_chars"], 7)
        self.assertEqual(tools, {"read": 1})
        with self.assertRaises(ValueError):
            shapes.ancestors([{"parent": 0, "message": {}}], 0)


if __name__ == "__main__":
    unittest.main()
