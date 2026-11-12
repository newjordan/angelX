"""Offline regressions: no GPU, desktop input, network or provider tokens used.
Run: python3 -I scripts/bench/mage-hands/bench/test_live_bench.py
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
import io
import socket
import sys
from types import SimpleNamespace
from unittest.mock import patch

HERE = Path(__file__).resolve().parent


class LiveScoreTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        (self.root / "desk").mkdir()
        (self.root / "desk/pages.json").write_text(json.dumps([
            {"name": "omacalc", "elements": [
                {"name": "8", "role": "push button", "box": [10, 10, 30, 30]}
            ]}
        ]))

    def run_fixture(self, name="calc-8008-r1", rc=0, task=None, log=None, verdict=True):
        run = self.root / "angel-live-tuned" / name
        run.mkdir(parents=True)
        if verdict:
            (run / "verdict.json").write_text(json.dumps({
                "task": "calc-8008", "pass": True, "observed": "8008",
                "angel_rc": rc, "wall_s": 1.0,
            }))
        if task is not None:
            (run / "task.json").write_text(json.dumps(task))
        (run / "task.err").write_text(log or "")
        return run

    def score(self):
        result = subprocess.run(
            ["python3", "-I", str(HERE / "live_score.py"), str(self.root)],
            env={**os.environ, "LIVE_DIR": "angel-live-tuned", "LIVE_REPS": "1", "LIVE_TASKS": "calc-8008"},
            capture_output=True, text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        return json.loads((self.root / "live_analysis-angel-live-tuned.json").read_text())

    def test_missing_binary_is_infrastructure_not_a_model_failure(self):
        self.run_fixture(rc=127, log="env: angel: No such file or directory\n")
        score = self.score()
        self.assertEqual(score["overall"]["tasks_n"], 0)
        self.assertEqual(score["overall"]["infra_errors"], 1)
        self.assertEqual(score["infra_runs"][0]["reason"], "launcher_exit_127")

    def test_app_success_without_mage_points_is_not_a_mage_pass(self):
        self.run_fixture(task={"status": "completed", "stop_reason": "end_turn", "hops": 3})
        score = self.score()
        self.assertEqual(score["overall"]["tasks_n"], 0)
        self.assertEqual(score["overall"]["invalid_runs"], 1)
        self.assertEqual(score["invalid_runs"][0]["reason"], "no_successful_ground")

    def test_http_500_ground_fallback_is_infrastructure_not_a_pass(self):
        self.run_fixture(task={"status": "completed", "hops": 3}, log=(
            "[task-event] call desktop: action=ground, target=8\n"
            "[task-event] error desktop: HTTP 500 Internal Server Error\n"
        ))
        score = self.score()
        self.assertEqual(score["overall"]["tasks_n"], 0)
        self.assertEqual(score["overall"]["infra_errors"], 1)

    def test_valid_mage_run_scores_and_has_no_missing_runs(self):
        self.run_fixture(task={"status": "completed", "hops": 3}, log=(
            "[task-event] call desktop: action=ground, target=8\n"
            "[task-event] done desktop (1s): 8 is at (20,20)\n"
            "[task-event] call desktop: action=click, x=20, y=20\n"
        ))
        score = self.score()
        self.assertEqual(score["overall"]["tasks_pass"], 1)
        self.assertEqual(score["overall"]["tasks_n"], 1)
        self.assertEqual(score["overall"]["ground_hit"], 1)
        self.assertEqual(score["overall"]["missing_runs"], 0)
        self.assertEqual(score["overall"]["status"], "complete")

    def test_unfinished_run_is_missing_not_silently_dropped(self):
        self.run_fixture(verdict=False)
        score = self.score()
        self.assertEqual(score["overall"]["tasks_n"], 0)
        self.assertEqual(score["overall"]["missing_runs"], 1)
        self.assertEqual(score["overall"]["status"], "incomplete")


class LivePreflightTests(unittest.TestCase):
    def run_preflight(self, binary="/bin/true", approved=""):
        # A preflight must reject before it touches Hyprland or the network.
        return subprocess.run(
            ["bash", str(HERE / "live_bench.sh"), "--preflight"],
            env={**os.environ, "ANGEL_BENCH_BIN": binary, "MAGE_ALLOW_MUSE": approved},
            capture_output=True, text=True, timeout=5,
        )

    def test_muse_requires_explicit_per_run_opt_in(self):
        result = self.run_preflight()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("MAGE_ALLOW_MUSE=1", result.stderr)

    def test_missing_binary_rejected_before_seat_start(self):
        result = self.run_preflight("/does/not/exist", "1")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("angel binary is not executable", result.stderr)

    def test_grok_brain_is_explicit_and_needs_no_muse_approval(self):
        command = ('source "$1"; mage_check_launch && mage_brain_env && '
                   'printf "%s\\n" "${BRAIN_ENV[@]}"')
        result = subprocess.run(
            ["bash", "-c", command, "test", str(HERE / "preflight.sh")],
            env={**os.environ, "MAGE_BRAIN": "grok-4.7", "MAGE_ALLOW_GROK": "1",
                 "MAGE_ALLOW_MUSE": "", "ANGEL_BENCH_BIN": "/bin/true"},
            text=True, capture_output=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("ANGEL_DRIVER=grok\n", result.stdout)
        self.assertIn("ANGEL_GROK_MODEL=grok-4.7\n", result.stdout)
        self.assertIn("ANGEL_VISION_SIDECAR=0\n", result.stdout)
        self.assertIn("ANGEL_VISION_TOOLS=1\n", result.stdout)
        self.assertIn("ANGEL_VISION_MODEL=grok-4.7\n", result.stdout)
        self.assertIn("ANGEL_YOLO=0\n", result.stdout)
        self.assertIn("ANGEL_YOLO_SMART=0\n", result.stdout)
        self.assertNotIn("ANGEL_META_", result.stdout)
        self.assertNotIn("ANGEL_KIMI_", result.stdout)

    def test_grok_and_unknown_brains_fail_closed(self):
        for brain in ("grok-4.7", "grok-4.6", "unknown"):
            result = subprocess.run(
                ["bash", "-c", 'source "$1"; mage_check_launch', "test", str(HERE / "preflight.sh")],
                env={**os.environ, "MAGE_BRAIN": brain, "MAGE_ALLOW_GROK": "",
                     "MAGE_ALLOW_MUSE": "1", "ANGEL_BENCH_BIN": "/bin/true"},
                text=True, capture_output=True,
            )
            self.assertNotEqual(result.returncode, 0, brain)


class LiveTaskLimitsTests(unittest.TestCase):
    def test_task_launch_has_no_wall_clock_or_hop_limit(self):
        source = (HERE / "live_task.sh").read_text()
        launch = source[source.index('( cd "$R/work"'):source.index('\nANGEL_RC=$?')]
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "work").mkdir()
            binary = root / "fake-angel"
            binary.write_text(
                '#!/usr/bin/python3\n'
                'import json, os, sys\n'
                'print(json.dumps({"argv": sys.argv[1:], '
                '"deadline": os.environ.get("ANGEL_TURN_DEADLINE_SECS"), '
                '"hops": os.environ.get("ANGEL_MAX_HOPS")}))\n'
            )
            binary.chmod(0o755)
            command = ('R=$1; BIN=$2; SOCK=fixture; SIG=fixture; PROMPT=fixture; '
                       'BRAIN_UNSET=(); BRAIN_ENV=(); ' + launch)
            result = subprocess.run(
                ["bash", "-c", command, "test", str(root), str(binary)],
                env={**os.environ, "ANGEL_TURN_DEADLINE_SECS": "1", "ANGEL_MAX_HOPS": "1"},
                capture_output=True, text=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            observed = json.loads((root / "task.json").read_text())
            self.assertEqual(observed["deadline"], "0")
            self.assertEqual(observed["hops"], "0")
            for flag in ("--max-hops", "--deadline-secs"):
                self.assertIn(flag, observed["argv"])
                self.assertEqual(observed["argv"][observed["argv"].index(flag) + 1], "0")
            self.assertNotIn("timeout ", launch)


class WarmupTests(unittest.TestCase):
    def test_warmup_uses_mage_template_and_normalized_json_coordinates(self):
        # Execute the actual heredoc with HTTP and PIL replaced by deterministic
        # fixtures. No real socket, GPU, model or provider request is used.
        source = (HERE / "preflight.sh").read_text().split("<<'PY'\n", 1)[1].split("\nPY", 1)[0]
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "items.json").write_text(json.dumps([{
                "shot": "desk/shots/example.png", "instruction": 'Click the "8" button.'
            }]))
            prompt_dir = root / "cockpit/src/agent/tools"
            prompt_dir.mkdir(parents=True)
            (prompt_dir / "hands_ground_prompt.txt").write_text("trained system prompt")
            image = SimpleNamespace(convert=lambda _: image, save=lambda b, **kw: b.write(b"jpeg"))
            image_module = SimpleNamespace(open=lambda p: image)
            requests = []
            def response(req, timeout=None):
                requests.append(req)
                if isinstance(req, str):
                    return io.BytesIO(b'{"model_path":"fixture"}')
                return io.BytesIO(b'{"text":"{\\"coordinate\\": [500, 500]}"}')
            argv = ["-", str(root), "http://local-fixture.invalid", str(root)]
            with patch.dict(sys.modules, {"PIL": SimpleNamespace(Image=image_module)}), \
                 patch("urllib.request.urlopen", side_effect=response), patch.object(sys, "argv", argv):
                exec(compile(source, "preflight-warmup", "exec"), {})
            payload = json.loads(next(r for r in requests if not isinstance(r, str)).data)
            self.assertIn('Instruction: Click the "8" button. The target is visible', payload["text"])
            self.assertIn("trained system prompt", payload["text"])
            self.assertEqual(payload["sampling_params"], {"max_new_tokens": 1024, "temperature": 0.0})
            self.assertTrue((root / "preflight-model-info.json").exists())


class SeatIdentityTests(unittest.TestCase):
    def test_exact_nested_pid_is_required_without_system_bus_access(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            sock = socket.socket(socket.AF_UNIX)
            self.addCleanup(sock.close)
            sock.bind(str(root / "wayland-test"))
            registry = [{"pid": 100, "wl_socket": "user", "instance": "user-signature"},
                        {"pid": 200, "wl_socket": "wayland-test", "instance": "private-signature"}]
            command = ('source "$1"; '
                       'hyprctl() { printf "%s" "$REGISTRY"; }; '
                       'systemctl() { echo "must not use private session bus" >&2; return 99; }; '
                       'mage_check_seat "$2" "$3"')
            env = {**os.environ, "REGISTRY": json.dumps(registry),
                   "XDG_RUNTIME_DIR": str(root), "MAGE_SEAT_PID": "200"}
            for sig, success in [("private-signature", True), ("user-signature", False)]:
                result = subprocess.run(["bash", "-c", command, "test", str(HERE / "preflight.sh"),
                                         "wayland-test", sig], env=env, text=True, capture_output=True)
                self.assertEqual(result.returncode == 0, success, result.stderr)
                self.assertNotIn("must not use", result.stderr)


if __name__ == "__main__":
    unittest.main(verbosity=2)
