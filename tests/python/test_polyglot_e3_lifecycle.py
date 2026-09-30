"""Exercise E3 ownership with local tmux/proxy fixtures and no model calls."""

import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


HARNESS = Path(__file__).resolve().parents[2] / "scripts/bench/book-harness"
CONTROLLER = r'''
import ctypes, json, os, pathlib, signal, subprocess, sys, time
root, outcome = pathlib.Path(sys.argv[1]), sys.argv[2]
assert ctypes.CDLL(None).prctl(36, 1, 0, 0, 0) == 0
parent, source = root / 'caller temp', root / 'source'
env = dict(os.environ, PATH=str(root / 'bin') + os.pathsep + os.environ['PATH'],
           TMPDIR=str(parent), WORKROOT=str(parent), HEESCH=str(source),
           E3_FIXTURE_ROOT=str(root), E3_OUTCOME=outcome, ROUTE='local',
           ANGEL_LOCAL_MODEL='fixture', PROXY_UPSTREAM='http://127.0.0.1:9/v1',
           PROXY_KEY='', PROXY_KEY_CMD='')
if outcome == 'unknown-route':
    env['ROUTE'] = 'unavailable-fixture-route'
if outcome == 'missing-parent':
    env['WORKROOT'] = str(root / 'absent')
runner = subprocess.Popen(['bash', str(root / 'e3_session.sh'), str(root / 'bin'), 'fixture'],
                          cwd=root, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          start_new_session=True)
receipt = {}
try:
    if outcome in ('SIGINT', 'SIGTERM', 'copy-SIGINT', 'copy-SIGTERM'):
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            try:
                receipt = json.loads((root / ('copy.json' if outcome.startswith('copy-') else 'tmux.json')).read_text())
                if receipt.get('worker') or receipt.get('copier'):
                    break
            except (OSError, ValueError):
                pass
            assert runner.poll() is None, runner.communicate()
            time.sleep(.01)
        assert receipt.get('worker') or receipt.get('copier'), 'owned fixture never started'
        os.kill(runner.pid, getattr(signal, outcome.removeprefix('copy-')))
    stdout, stderr = runner.communicate(timeout=6)
    expected = {'SIGINT': 130, 'SIGTERM': 143}.get(outcome.removeprefix('copy-'), 2)
    assert runner.returncode == expected, (runner.returncode, stdout, stderr)
    assert sorted(p.name for p in parent.iterdir()) == ['retained']
    assert (source / 'input.txt').read_text() == 'source bytes\n'
    assert not (source / '.git').exists()
    if (root / 'copy.json').exists():
        copier = json.loads((root / 'copy.json').read_text())['copier']
        try:
            os.kill(copier, 0)
        except ProcessLookupError:
            pass
        else:
            raise AssertionError('owned copier survived')
    if (root / 'proxy.pid').exists():
        proxy = int((root / 'proxy.pid').read_text())
        try:
            os.kill(proxy, 0)
        except ProcessLookupError:
            pass
        else:
            raise AssertionError('owned proxy survived')
    if (root / 'tmux.json').exists():
        receipt = json.loads((root / 'tmux.json').read_text())
        assert receipt['session'].startswith('e3-fixture-')
        assert (root / 'tmux.killed').read_text() == '=' + receipt['session'] + ':'
        if receipt.get('worker'):
            command = json.loads((root / 'command.json').read_text())
            assert command['work'] == receipt['work']
            assert not pathlib.Path(command['home']).exists()
            assert not pathlib.Path(command['temporary']).exists()
            assert (root / 'out/fixture.stderr').read_text() == 'fixture command stderr\n'
            pid, status = os.waitpid(receipt['worker'], 0)
            assert pid == receipt['worker'] and os.WIFSIGNALED(status)
            try:
                os.kill(pid, 0)
            except ProcessLookupError:
                pass
            else:
                raise AssertionError('workspace worker survived')
    else:
        assert outcome in ('unknown-route', 'missing-parent', 'copy-failure', 'copy-SIGINT', 'copy-SIGTERM')
finally:
    groups = [runner.pid]
    if (root / 'proxy.pid').exists():
        groups.append(int((root / 'proxy.pid').read_text()))
    try:
        groups.append(json.loads((root / 'tmux.json').read_text()).get('worker'))
    except (OSError, ValueError):
        pass
    try:
        groups.append(json.loads((root / 'copy.json').read_text()).get('copier'))
    except (OSError, ValueError):
        pass
    for group in groups:
        if group:
            try:
                os.killpg(group, signal.SIGKILL)
            except ProcessLookupError:
                pass
    runner.wait(timeout=2)
    while True:
        try:
            os.waitpid(-1, 0)
        except ChildProcessError:
            break
'''


@unittest.skipUnless(sys.platform == "linux", "requires the isolated Linux subreaper fixture")
class E3LifecycleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="angel-e3-lifecycle-")
        self.addCleanup(self.temp.cleanup)
        # Spaces and an apostrophe also exercise the generated tmux command.
        self.root = Path(self.temp.name) / "fixture's harness"
        self.root.mkdir()
        self.parent = self.root / "caller temp"
        self.parent.mkdir()
        (self.parent / "retained").write_text("caller bytes\n")
        self.source = self.root / "source"
        self.source.mkdir()
        (self.source / "input.txt").write_text("source bytes\n")
        for name in ("e3_session.sh", "seat.sh", "copy_fixture.py", "exec_session.py", "lifecycle.sh"):
            shutil.copy2(HARNESS / name, self.root / name)
        (self.root / "proxy_log.py").write_text(
            "import os, time\nfrom pathlib import Path\n"
            "Path(os.environ['E3_FIXTURE_ROOT'], 'proxy.pid').write_text(str(os.getpid()))\n"
            "Path(os.environ['PROXY_LOG']).touch()\n"
            "Path(os.environ['PROXY_READY']).write_text('1\\n')\n"
            "time.sleep(30)\n")
        bindir = self.root / "bin"
        bindir.mkdir()
        self.executable(bindir / "angel", "#!/usr/bin/env python3\n"
                        "import json, os\nfrom pathlib import Path\n"
                        f"Path({str(self.root / 'command.json')!r}).write_text(json.dumps("
                        "{'home':os.environ['HOME'],'temporary':os.environ['TMPDIR'],'work':os.environ['ANGEL_WORKSPACE']}))\n"
                        "print('fixture command stderr', file=__import__('sys').stderr)\n")
        self.executable(bindir / "sleep", "#!/bin/sh\nexec /bin/sleep .05\n")
        self.executable(bindir / "tmux", r'''#!/usr/bin/env python3
import json, os, pathlib, subprocess, sys, time
root = pathlib.Path(os.environ['E3_FIXTURE_ROOT'])
if sys.argv[1] == 'new-session':
    session = sys.argv[sys.argv.index('-s') + 1]
    work = sys.argv[sys.argv.index('-c') + 1]
    receipt = {'session': session, 'work': work}
    if os.environ['E3_OUTCOME'] != 'tmux-failure':
        subprocess.run(['/bin/bash', '-c', sys.argv[-1]], cwd=work, check=True)
        child = subprocess.Popen([sys.executable, '-c',
            'import os,pathlib,signal,time; os.chdir(os.environ["E3_WORK"]); '
            'signal.signal(signal.SIGTERM,signal.SIG_IGN); '
            'pathlib.Path(os.environ["E3_FIXTURE_ROOT"],"worker.ready").touch(); time.sleep(30)'],
            env=dict(os.environ, E3_WORK=work), stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
        receipt['worker'] = child.pid
        deadline = time.monotonic() + 2
        while not (root / 'worker.ready').exists():
            assert child.poll() is None and time.monotonic() < deadline
            time.sleep(.005)
    (root / 'tmux.json').write_text(json.dumps(receipt))
    raise SystemExit(19 if os.environ['E3_OUTCOME'] == 'tmux-failure' else 0)
elif sys.argv[1] == 'kill-session':
    (root / 'tmux.killed').write_text(sys.argv[sys.argv.index('-t') + 1])
''')

    def executable(self, path, text):
        path.write_text(text)
        path.chmod(0o755)

    def check_outcome(self, outcome):
        result = subprocess.run([sys.executable, "-c", CONTROLLER, str(self.root), outcome],
                                text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=12)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_unknown_route_removes_private_snapshot_and_home(self):
        self.check_outcome("unknown-route")

    def test_temporary_parent_failure_stops_before_fixture_copy(self):
        self.check_outcome("missing-parent")

    def test_copy_failure_removes_partially_populated_snapshot(self):
        os.mkfifo(self.source / "unsupported-pipe")
        self.check_outcome("copy-failure")

    def test_tmux_startup_failure_stops_proxy_and_removes_private_snapshot(self):
        self.check_outcome("tmux-failure")

    def test_pid_only_interrupt_stops_owned_session_proxy_and_workspace_worker(self):
        self.check_outcome("SIGINT")

    def test_pid_only_termination_stops_owned_session_proxy_and_workspace_worker(self):
        self.check_outcome("SIGTERM")

    def slow_copy(self):
        (self.root / "copy_fixture.py").write_text(
            "import json, os, signal, time\nfrom pathlib import Path\n"
            "signal.signal(signal.SIGTERM, signal.SIG_IGN)\n"
            "Path(os.environ['E3_FIXTURE_ROOT'], 'copy.json').write_text(json.dumps({'copier': os.getpid()}))\n"
            "time.sleep(30)\n")

    def test_pid_only_interrupt_stops_fixture_copy_before_tmux_starts(self):
        self.slow_copy()
        self.check_outcome("copy-SIGINT")

    def test_pid_only_termination_stops_fixture_copy_before_tmux_starts(self):
        self.slow_copy()
        self.check_outcome("copy-SIGTERM")


if __name__ == "__main__":
    unittest.main()
