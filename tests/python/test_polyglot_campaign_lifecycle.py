"""Campaign cancellation forwards through route chains without stale PID signals."""

import importlib.util
import os
from pathlib import Path
import select
import shutil
import signal
import subprocess
import sys
import tempfile
import unittest


HARNESS = Path(__file__).resolve().parents[2] / "scripts/bench/book-harness"
spec = importlib.util.spec_from_file_location("campaign_signals", HARNESS / "signal_children.py")
signals = importlib.util.module_from_spec(spec)
spec.loader.exec_module(signals)
CONTROLLER = r'''
import ctypes,json,os,pathlib,signal,subprocess,sys,time
assert ctypes.CDLL(None).prctl(36,1,0,0,0) == 0
root,name,requested = pathlib.Path(sys.argv[1]),sys.argv[2],sys.argv[3]
arguments = ['2','working','other'] if name == 'e3.sh' else ['working','other']
runner = subprocess.Popen(['bash',str(root/name),*arguments],cwd=root,
                          stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
handles = [os.pidfd_open(runner.pid)]
rows = []
try:
    expected = 4 if name == 'e3.sh' else 2
    deadline = time.monotonic()+5
    while time.monotonic() < deadline:
        try:
            rows = [json.loads(path.read_text()) for path in (root/'out/attempts').glob('*.json')]
            if len(rows) == expected:
                break
        except (OSError,ValueError):
            pass
        assert runner.poll() is None,runner.communicate()
        time.sleep(.01)
    assert len(rows) == expected,rows
    for parent in {row['parent'] for row in rows} - {runner.pid}:
        descriptor = os.pidfd_open(parent)
        handles.append(descriptor)
        assert int(pathlib.Path(f'/proc/{parent}/stat').read_text().rsplit(')',1)[1].split()[1]) == runner.pid
    for row in rows:
        handles.extend(os.pidfd_open(pid) for pid in (row['pid'],row['child']))
    os.kill(runner.pid,getattr(signal,requested))
    stdout,stderr = runner.communicate(timeout=8)
    assert runner.returncode == {'SIGINT':130,'SIGTERM':143}[requested],(runner.returncode,stdout,stderr)
    assert sorted(path.name for path in (root/'scratch').iterdir()) == ['retained']
    for row in rows:
        for pid in (row['pid'],row['child']):
            try:
                os.kill(pid,0)
            except ProcessLookupError:
                pass
            else:
                raise AssertionError(f'owned fixture {pid} survived')
    assert len(list((root/'out/attempts').glob('*.json'))) == expected,'cancel launched more trials'
finally:
    for descriptor in handles:
        try:
            signal.pidfd_send_signal(descriptor,signal.SIGKILL)
        except ProcessLookupError:
            pass
        os.close(descriptor)
    runner.wait(timeout=3)
    deadline = time.monotonic()+3
    while True:
        # Failed assertions can leave route chains or later fixture trials.
        # This isolated subreaper owns only this fixture: stop each adopted
        # direct child through a stable handle before collecting its status.
        for word in pathlib.Path(f'/proc/self/task/{os.getpid()}/children').read_text().split():
            pid = int(word)
            try:
                descriptor = os.pidfd_open(pid)
            except ProcessLookupError:
                continue
            try:
                parent = int(pathlib.Path(f'/proc/{pid}/stat').read_text().rsplit(')',1)[1].split()[1])
                if parent == os.getpid():
                    signal.pidfd_send_signal(descriptor,signal.SIGKILL)
            except (FileNotFoundError,ProcessLookupError):
                pass
            finally:
                os.close(descriptor)
        try:
            waited,_ = os.waitpid(-1,os.WNOHANG)
        except ChildProcessError:
            break
        assert time.monotonic() < deadline,'fixture cleanup did not settle'
        if not waited:
            time.sleep(.01)
'''


@unittest.skipUnless(sys.platform == "linux" and hasattr(os, "pidfd_open"), "requires Linux process handles")
class CampaignLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="angel-campaign-lifecycle-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for name in ("e1.sh", "e3.sh", "e5.sh", "campaign_lifecycle.sh", "signal_children.py"):
            shutil.copy2(HARNESS / name, self.root / name)
        (self.root / "scratch").mkdir()
        (self.root / "scratch/retained").write_text("caller bytes\n")
        trial = r'''#!/usr/bin/env python3
import json,os,pathlib,shutil,signal,subprocess,sys,time
root = pathlib.Path(__file__).resolve().parent
label = sys.argv[2]
scratch = root/'scratch'/label
scratch.mkdir()
child = subprocess.Popen([sys.executable,'-c',
    'import signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); print("ready",flush=True); time.sleep(30)'],
    stdout=subprocess.PIPE,start_new_session=True,text=True)
assert child.stdout.readline().strip() == 'ready'
def stop(signum,frame):
    child.kill()
    child.wait()
    shutil.rmtree(scratch)
    raise SystemExit(0)
signal.signal(signal.SIGTERM,stop)
(root/'out/attempts').mkdir(exist_ok=True)
(root/'out/attempts'/f'{label}.json').write_text(json.dumps({'pid':os.getpid(),'parent':os.getppid(),'child':child.pid}))
while True:
    time.sleep(.1)
'''
        for name in ("task_run.sh", "e3_session.sh"):
            path = self.root / name
            path.write_text(trial)
            path.chmod(0o755)

    def check_cancel(self, name, requested):
        result = subprocess.run([sys.executable, "-c", CONTROLLER, str(self.root), name, requested],
                                text=True, capture_output=True, timeout=14)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_e1_pid_only_interrupt_retires_both_route_chains(self):
        self.check_cancel("e1.sh", "SIGINT")

    def test_e1_pid_only_termination_retires_both_route_chains(self):
        self.check_cancel("e1.sh", "SIGTERM")

    def test_e5_pid_only_interrupt_retires_both_route_chains(self):
        self.check_cancel("e5.sh", "SIGINT")

    def test_e5_pid_only_termination_retires_both_route_chains(self):
        self.check_cancel("e5.sh", "SIGTERM")

    def test_e3_pid_only_interrupt_retires_every_active_session(self):
        self.check_cancel("e3.sh", "SIGINT")

    def test_e3_pid_only_termination_retires_every_active_session(self):
        self.check_cancel("e3.sh", "SIGTERM")

    def owned_peer(self, ignore=False):
        code = "import signal,time; "
        if ignore:
            code += "signal.signal(signal.SIGTERM,signal.SIG_IGN); "
        code += "print('ready',flush=True); time.sleep(30)"
        peer = subprocess.Popen([sys.executable, "-c", code], stdout=subprocess.PIPE, text=True)
        def cleanup():
            if peer.poll() is None:
                peer.kill()
            peer.wait(timeout=2)
            peer.stdout.close()
        self.addCleanup(cleanup)
        self.assertTrue(select.select([peer.stdout], [], [], 2)[0])
        self.assertEqual(peer.stdout.readline().strip(), "ready")
        return peer

    def test_signal_helper_refuses_a_process_outside_the_requested_parent(self):
        peer = self.owned_peer()
        try:
            signals.stop_children(os.getpid() + 1, [peer.pid], grace=.05)
            self.assertIsNone(peer.poll())
        finally:
            peer.kill()
            peer.wait(timeout=2)

    def test_signal_helper_escalates_a_term_ignoring_direct_child(self):
        peer = self.owned_peer(ignore=True)
        signals.stop_children(os.getpid(), [peer.pid], grace=.05)
        self.assertEqual(peer.wait(timeout=2), -signal.SIGKILL)

    def test_signal_helper_refuses_its_own_recycled_process_identity(self):
        code = (f"import importlib.util,os; spec=importlib.util.spec_from_file_location('signals',{str(HARNESS / 'signal_children.py')!r}); "
                "module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module); "
                "module.stop_children(os.getppid(),[os.getpid()],grace=.01)")
        result = subprocess.run([sys.executable, "-c", code], capture_output=True, timeout=2)
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
