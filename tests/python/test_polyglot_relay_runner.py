"""The local manual relay owns fixtures and preserves the task's status."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


SOURCE = Path(__file__).resolve().parents[2] / "scripts/bench/book-harness"
TASK = """#!/usr/bin/env python3
import json, os, sys, urllib.request
with urllib.request.urlopen(os.environ['ANGEL_LOCAL_URL'] + '/models', timeout=2) as response:
    response.read()
work = sys.argv[sys.argv.index('--workspace') + 1]
print(json.dumps({'home': os.environ['HOME'], 'temporary': os.environ['TMPDIR'],
                  'url': os.environ['ANGEL_LOCAL_URL'], 'work': work}), flush=True)
raise SystemExit(17)
"""

# Run cancellation in a separate subreaper so the suite never adopts unrelated
# test children. The fixture's direct and inherited pipe holders are all reaped.
CANCEL = r"""
import ctypes, json, os, pathlib, signal, subprocess, sys, time, urllib.request
ctypes.CDLL(None).prctl(36, 1, 0, 0, 0)
script, binary, run, event = sys.argv[1:]
driver = subprocess.Popen(['bash', '-m', script, binary, 'fixture-helper', run],
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
receipt = None
try:
    deadline = time.monotonic() + 6
    while time.monotonic() < deadline:
        path = pathlib.Path(run) / 'result.json'
        if path.exists():
            try:
                receipt = json.loads(path.read_text())
                break
            except json.JSONDecodeError:
                pass
        if driver.poll() is not None:
            raise RuntimeError('driver exited before the task was ready')
        time.sleep(.01)
    if receipt is None:
        raise RuntimeError('task never became ready')
    if event != 'exit':
        os.kill(driver.pid, getattr(signal, event))
    stdout, stderr = driver.communicate(timeout=6)
    child = receipt['child']
    pid, status = os.waitpid(child, 0)
    assert pid == child and os.WIFSIGNALED(status) and os.WTERMSIG(status) == signal.SIGKILL
    try:
        urllib.request.urlopen(receipt['url'] + '/models', timeout=.5)
    except Exception:
        closed = True
    else:
        closed = False
    print(json.dumps({'status': driver.returncode, 'receipt': receipt, 'relay_closed': closed,
                      'stderr': stderr, 'child_reaped': True}))
finally:
    if driver.poll() is None:
        os.kill(driver.pid, signal.SIGTERM)
        try:
            driver.wait(timeout=2)
        except subprocess.TimeoutExpired:
            driver.kill()
            driver.wait()
    # Only our adopted fixture jobs qualify for fallback cleanup.
    children = pathlib.Path(f'/proc/self/task/{os.getpid()}/children').read_text().split()
    for raw in children:
        try:
            descriptor = os.pidfd_open(int(raw))
            try:
                signal.pidfd_send_signal(descriptor, signal.SIGKILL)
            finally:
                os.close(descriptor)
        except ProcessLookupError:
            pass
    while True:
        try:
            os.waitpid(-1, 0)
        except ChildProcessError:
            break
"""


class RelayRunnerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="angel-relay-fixture-", dir="/tmp")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "caller's fixture"
        self.root.mkdir()
        for name in ("relay_run.sh", "relay.py", "relay_files.py", "pending.py", "exec_session.py", "lifecycle.sh"):
            shutil.copy2(SOURCE / name, self.root / name)
        self.parent = self.root / "temporary parent"
        self.parent.mkdir()
        (self.parent / "retained").write_text("caller-owned")
        self.run = self.root / "manual run"
        self.binary = self.root / "angel-fixture"
        self.binary.write_text(TASK)
        self.binary.chmod(0o755)
        self.env = dict(os.environ, TMPDIR=str(self.parent))

    def execute(self, *args, env=None):
        return subprocess.run(["bash", str(self.root / "relay_run.sh"),
                               str(self.binary), "fixture-helper", str(self.run), *args],
                              cwd=self.root, env=env or self.env, text=True,
                              capture_output=True, timeout=10)

    def assert_clean(self, receipt=None):
        self.assertEqual([path.name for path in self.parent.iterdir()], ["retained"])
        if receipt:
            for key in ("home", "temporary", "work"):
                self.assertFalse(Path(receipt[key]).exists(), key)

    def test_task_failure_retains_receipts_and_removes_owned_fixtures(self):
        result = self.execute("fixture directive")
        self.assertEqual(result.returncode, 17, result.stderr)
        self.assertEqual((self.run / "rc").read_text(), "17\n")
        self.assertEqual((self.run / "verify").read_text(), "fail\n")
        self.assertEqual((self.run / "verify.rc").read_text(), "1\n")
        receipt = json.loads((self.run / "result.json").read_text())
        self.assertTrue(receipt["temporary"].startswith(str(self.parent / "angel-relay-run-")))
        self.assert_clean(receipt)
        probe = subprocess.run([sys.executable, "-c", "import urllib.request; "
                                "urllib.request.urlopen(__import__('sys').argv[1], timeout=.5)",
                                receipt["url"] + "/models"], capture_output=True, timeout=2)
        self.assertNotEqual(probe.returncode, 0)

    def test_existing_output_is_preserved_before_startup(self):
        self.run.mkdir()
        (self.run / "retained").write_text("previous request evidence")
        result = self.execute()
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertEqual((self.run / "retained").read_text(), "previous request evidence")
        self.assertEqual([path.name for path in self.run.iterdir()], ["retained"])
        self.assert_clean()

    def test_invalid_arguments_do_not_create_outputs(self):
        result = subprocess.run(["bash", str(self.root / "relay_run.sh")],
                                env=self.env, capture_output=True, text=True, timeout=2)
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertFalse(self.run.exists())
        self.assert_clean()

    def test_failed_relay_does_not_launch_task(self):
        (self.root / "relay.py").write_text("raise SystemExit(19)\n")
        result = self.execute()
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertIn("Relay failed to start", result.stderr)
        self.assertFalse((self.run / "result.json").exists())
        self.assertEqual((self.run / "rc").read_text(), "2\n")
        self.assert_clean()

    def test_temporary_setup_failure_publishes_completion(self):
        result = self.execute(env=dict(self.env, TMPDIR=str(self.root / "missing temporary parent")))
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.run / "rc").read_text(), f"{result.returncode}\n")
        observed = subprocess.run([sys.executable, str(self.root / "pending.py"), str(self.run)],
                                  capture_output=True, text=True, timeout=2)
        self.assertEqual(observed.returncode, 0, observed.stderr)
        self.assertEqual(observed.stdout, f"DONE {result.returncode}\n")
        self.assertFalse((self.run / "result.json").exists())
        self.assert_clean()

    def test_exited_run_has_no_pending_manual_reply(self):
        self.run.mkdir()
        (self.run / "rc").write_text("143\n")
        # The run has exited with an unanswered request, including one whose
        # bytes cannot be parsed. Its completion marker settles the status.
        (self.run / "1.req.json").write_text('{"body":')
        result = subprocess.run([sys.executable, str(self.root / "pending.py"), str(self.run)],
                                capture_output=True, text=True, timeout=2)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "DONE 143\n")

    def test_git_fixture_ignores_inherited_repository(self):
        inherited = self.root / "unrelated repository"
        inherited.mkdir()
        subprocess.run(["git", "init", "-q", str(inherited)], check=True, capture_output=True)
        result = self.execute(env=dict(self.env, GIT_DIR=str(inherited / ".git"),
                                       GIT_WORK_TREE=str(inherited), GIT_INDEX_FILE=str(inherited / "index")))
        self.assertEqual(result.returncode, 17, result.stderr)
        self.assertFalse((inherited / "index").exists())
        self.assertFalse((inherited / ".git/refs/heads/master").exists())
        self.assert_clean()

    def test_hanging_verifier_is_bounded_and_cannot_mark_pass(self):
        self.binary.write_text(TASK.replace("raise SystemExit(17)",
                              "__import__('pathlib').Path(work, 'test_stats.py').write_text('import time; time.sleep(30)\\n')\nraise SystemExit(0)"))
        result = self.execute(env=dict(self.env, VERIFY_TIMEOUT_SECS="0.1"))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.run / "verify").read_text(), "fail\n")
        self.assertEqual((self.run / "verify.rc").read_text(), "137\n")
        self.assert_clean(json.loads((self.run / "result.json").read_text()))

    @unittest.skipUnless(sys.platform == "linux", "isolated subreaper fixture requires Linux")
    def test_task_group_is_cleaned_on_pid_only_signal_and_normal_leader_exit(self):
        for event, status in (("SIGINT", 130), ("SIGTERM", 143), ("exit", 0)):
            with self.subTest(event=event):
                self.run = self.root / event
                self.binary.write_text(TASK.replace("raise SystemExit(17)", "")
                                       .replace("print(json.dumps", "import signal, time\nready = os.path.join(os.environ['HOME'], 'child.ready')\nchild = os.fork()\nif child == 0:\n    signal.signal(signal.SIGTERM, signal.SIG_IGN)\n    open(ready, 'w').write('ready')\n    time.sleep(30)\n    raise SystemExit(0)\nwhile not os.path.exists(ready):\n    time.sleep(.01)\nprint(json.dumps")
                                       .replace("'work': work", "'work': work, 'child': child")
                                       + ("raise SystemExit(0)\n" if event == "exit" else
                                          "signal.signal(signal.SIGTERM, signal.SIG_IGN)\ntime.sleep(30)\n"))
                result = subprocess.run([sys.executable, "-c", CANCEL, str(self.root / "relay_run.sh"),
                                         str(self.binary), str(self.run), event], env=self.env,
                                        text=True, capture_output=True, timeout=14)
                self.assertEqual(result.returncode, 0, result.stderr)
                evidence = json.loads(result.stdout)
                self.assertEqual(evidence["status"], status, evidence)
                self.assertEqual((self.run / "rc").read_text(), f"{status}\n")
                self.assertTrue(evidence["relay_closed"], evidence)
                self.assertTrue(evidence["child_reaped"], evidence)
                self.assert_clean(evidence["receipt"])


if __name__ == "__main__":
    unittest.main()
