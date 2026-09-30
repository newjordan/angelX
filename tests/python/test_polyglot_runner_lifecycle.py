"""Real fake-evaluator process groups, with no credentials or model calls."""
import ctypes
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/run-polyglot-loop.py"


@unittest.skipUnless(sys.platform == "linux", "fixture descendants require Linux subreaper support")
class EvaluatorLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.libc = ctypes.CDLL(None)
        self.previous_subreaper = ctypes.c_int()
        self.assertEqual(self.libc.prctl(37, ctypes.byref(self.previous_subreaper), 0, 0, 0), 0)
        self.assertEqual(self.libc.prctl(36, 1, 0, 0, 0), 0)
        self.addCleanup(lambda: self.libc.prctl(36, self.previous_subreaper.value, 0, 0, 0))

    def reap_child(self, pid):
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            child, status = os.waitpid(pid, os.WNOHANG)
            if child:
                self.assertEqual(child, pid)
                self.assertTrue(os.WIFSIGNALED(status), "the owned TERM-ignoring child must be stopped")
                return
            time.sleep(.01)
        self.fail("fixture child did not exit")

    def exercise(self, outcome, expected):
        with tempfile.TemporaryDirectory(prefix="angelx-evaluator-fixture-") as tmp:
            root = Path(tmp)
            receipt_path = root / "ready.json"
            evaluator = root / "eval.py"
            evaluator.write_text("""import json, os, signal, sys, time
read_ready, write_ready = os.pipe()
child = os.fork()
if child == 0:
    os.close(read_ready)
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    os.write(write_ready, b'ready')
    os.close(write_ready)
    while True:
        time.sleep(1)
os.close(write_ready)
os.read(read_ready, 5)
os.close(read_ready)
signal.signal(signal.SIGTERM, signal.SIG_IGN)
with open(sys.argv[1], 'w') as receipt:
    json.dump({'pid': os.getpid(), 'child': child, 'home': os.environ['LUNA_CODEX_HOME'],
               'blocked_signals': sorted(int(sig) for sig in signal.pthread_sigmask(signal.SIG_BLOCK, []))}, receipt)
if sys.argv[2] == 'exit':
    raise SystemExit(7)
while True:
    time.sleep(1)
""")
            driver = root / "runner.py"
            driver.write_text(f"""import importlib.util, json, os, pathlib, signal, sys, tempfile
spec = importlib.util.spec_from_file_location('runner', {json.dumps(str(SCRIPT))})
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)
root = pathlib.Path(sys.argv[1])
outcome = sys.argv[2]
previous = {{sig: signal.getsignal(sig) for sig in (signal.SIGINT, signal.SIGTERM)}}
mask = signal.pthread_sigmask(signal.SIG_BLOCK, [])
manifest = {{}}
group_signals = []
killpg = runner.os.killpg
def owned_killpg(pid, signum):
    # Every group signal must still have a waitable direct-child claim. A
    # zombie leader keeps its PID reserved even when all descendants exited.
    os.waitid(os.P_PID, pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
    group_signals.append(int(signum))
    return killpg(pid, signum)
runner.os.killpg = owned_killpg
directory = tempfile.TemporaryDirectory(prefix='codex-home-', dir=root)
cleanup_ignored = []
def cleanup():
    cleanup_ignored.append(all(signal.getsignal(sig) == signal.SIG_IGN for sig in previous))
    directory.cleanup()
with directory as home:
    pathlib.Path(home, 'fixture-private').write_text('fixture')
    env = {{'PATH': os.environ['PATH'], 'LUNA_CODEX_HOME': home}}
    with (root / 'eval.log').open('w') as log:
        status = runner.run_evaluator([sys.executable, str(root / 'eval.py'), str(root / 'ready.json'), outcome],
            env=env, cwd=root, log=log, timeout=1 if outcome == 'timeout' else 10,
            manifest=manifest, grace_secs=.05, cleanup=cleanup)
print(json.dumps({{'status': status, 'manifest': manifest, 'home': home,
    'group_signals': group_signals,
    'cleanup_signals_ignored': cleanup_ignored == [True],
    'handlers_restored': all(signal.getsignal(sig) == prior for sig, prior in previous.items()),
    'mask_restored': signal.pthread_sigmask(signal.SIG_BLOCK, []) == mask}}), flush=True)
raise SystemExit(status)
""")
            process = subprocess.Popen([sys.executable, str(driver), str(root), outcome],
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
            receipt = None
            reaped = False
            try:
                deadline = time.monotonic() + 3
                while time.monotonic() < deadline:
                    try:
                        receipt = json.loads(receipt_path.read_text())
                        break
                    except (OSError, ValueError):
                        if process.poll() is not None:
                            self.fail(f"fixture exited before readiness: {process.communicate()}")
                        time.sleep(.01)
                self.assertIsNotNone(receipt, "fake evaluator did not start")
                self.assertNotIn(int(signal.SIGTERM), receipt["blocked_signals"])
                self.assertNotIn(int(signal.SIGINT), receipt["blocked_signals"])
                if outcome in ("SIGTERM", "SIGINT"):
                    os.kill(process.pid, getattr(signal, outcome))
                stdout, stderr = process.communicate(timeout=5)
                self.assertEqual(process.returncode, expected, stderr)
                result = json.loads(stdout)
                self.assertEqual(result["status"], expected)
                self.assertTrue(result["handlers_restored"])
                self.assertTrue(result["mask_restored"])
                self.assertTrue(result["cleanup_signals_ignored"])
                self.assertEqual(result["group_signals"], [int(signal.SIGTERM), int(signal.SIGKILL)])
                self.assertFalse(Path(result["home"]).exists())
                self.assertEqual(result["home"], receipt["home"])
                self.reap_child(receipt["child"])
                reaped = True
                for pid in (receipt["pid"], receipt["child"]):
                    with self.assertRaises(ProcessLookupError):
                        os.kill(pid, 0)
                return result["manifest"]
            finally:
                if receipt:
                    try:
                        os.killpg(receipt["pid"], signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                if process.poll() is None:
                    process.kill()
                process.communicate(timeout=3)
                if receipt and not reaped:
                    try:
                        self.reap_child(receipt["child"])
                    except ChildProcessError:
                        pass

    def test_normal_nonzero_exit_preserves_status_and_stops_lingering_group(self):
        self.assertEqual(self.exercise("exit", 7), {})

    def test_pid_only_sigterm_stops_owned_group_and_removes_temporary_home(self):
        manifest = self.exercise("SIGTERM", 143)
        self.assertEqual(manifest["runner_terminated_signal"], "SIGTERM")
        self.assertNotIn("runner_timed_out", manifest)

    def test_pid_only_sigint_preserves_interrupt_status_and_cleanup(self):
        manifest = self.exercise("SIGINT", 130)
        self.assertEqual(manifest["runner_terminated_signal"], "SIGINT")
        self.assertTrue(manifest["runner_interrupted"])

    def test_timeout_preserves_timeout_status_and_cleanup(self):
        manifest = self.exercise("timeout", 124)
        self.assertTrue(manifest["runner_timed_out"])
        self.assertNotIn("runner_terminated_signal", manifest)


if __name__ == "__main__":
    unittest.main()
