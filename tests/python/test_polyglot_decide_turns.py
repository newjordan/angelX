"""Exercise decision TUI ownership with local stub and tmux fixtures."""

import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

HARNESS = Path(__file__).resolve().parents[2] / 'scripts/bench/book-harness'
CONTROLLER = r'''
import ctypes, json, os, pathlib, select, signal, subprocess, sys, time
root, outcome = pathlib.Path(sys.argv[1]), sys.argv[2]
assert ctypes.CDLL(None).prctl(36, 1, 0, 0, 0) == 0
parent = root / 'caller temp'
env = dict(os.environ, PATH=str(root / 'bin') + os.pathsep + os.environ['PATH'],
           TMPDIR=str(parent), DECIDE_ROOT=str(root), DECIDE_OUTCOME=outcome,
           STUB_CYCLES='invalid' if outcome == 'stub-failure' else '0')
for name in ('STUB_SCRIPT', 'STUB_PROMPTS', 'STUB_READY'):
    env.pop(name, None)
scenario = 'missing' if outcome == 'scenario-failure' else 'fixture'
runner = subprocess.Popen(['bash', str(root / 'decide_turns.sh'), str(root / 'bin'), 'fixture', scenario],
                          cwd=root, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          start_new_session=True, text=True)
handles = [os.pidfd_open(runner.pid)]
receipt = {}
try:
    if outcome in ('SIGINT', 'SIGTERM'):
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            try:
                receipt = json.loads((root / 'tmux.json').read_text())
                if receipt.get('worker'):
                    break
            except (OSError, ValueError):
                pass
            assert runner.poll() is None, runner.communicate()
            time.sleep(.005)
        assert receipt.get('worker'), 'session fixture did not start'
        handles.append(os.pidfd_open(receipt['worker']))
        os.kill(runner.pid, getattr(signal, outcome))
    stdout, stderr = runner.communicate(timeout=6)
    expected = {'success': 0, 'idle-limit': 124, 'SIGINT': 130, 'SIGTERM': 143}.get(outcome, 2)
    assert runner.returncode == expected, (runner.returncode, stdout, stderr)
    assert sorted(p.name for p in parent.iterdir()) == ['retained']
    if (root / 'tmux.json').exists():
        receipt = json.loads((root / 'tmux.json').read_text())
        assert receipt['session'].startswith('decide-fixture-')
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
                raise AssertionError('private workspace child survived')
            import urllib.request
            try:
                urllib.request.urlopen(command['url'] + '/models', timeout=1)
            except OSError:
                pass
            else:
                raise AssertionError('owned stub survived')
    else:
        assert outcome in ('stub-failure', 'scenario-failure')
    if outcome == 'success':
        assert 'requests=1' in stdout
    if outcome == 'idle-limit':
        assert 'did not become idle' in stderr
finally:
    for handle in handles:
        try:
            signal.pidfd_send_signal(handle, signal.SIGKILL)
        except ProcessLookupError:
            pass
        os.close(handle)
    runner.wait(timeout=2)
    # Bound assertion-failure cleanup too: only this isolated subreaper's own
    # adopted children qualify, and every signal uses a stable handle.
    deadline = time.monotonic() + 3
    while time.monotonic() < deadline:
        children = pathlib.Path(f'/proc/{os.getpid()}/task/{os.getpid()}/children').read_text().split()
        for raw in children:
            pid = int(raw)
            try:
                handle = os.pidfd_open(pid)
            except ProcessLookupError:
                continue
            try:
                fields = pathlib.Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
                if int(fields[1]) == os.getpid():
                    signal.pidfd_send_signal(handle, signal.SIGKILL)
            except ProcessLookupError:
                pass
            finally:
                os.close(handle)
        while True:
            try:
                pid, _ = os.waitpid(-1, os.WNOHANG)
            except ChildProcessError:
                pid = 0
                break
            if pid == 0:
                break
        if not pathlib.Path(f'/proc/{os.getpid()}/task/{os.getpid()}/children').read_text().split():
            break
        time.sleep(.005)
'''


@unittest.skipUnless(sys.platform == 'linux', 'requires an isolated Linux subreaper')
class DecideTurnsTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='angel-decide-turns-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "fixture's harness"
        self.root.mkdir()
        parent = self.root / 'caller temp'
        parent.mkdir()
        (parent / 'retained').write_text('caller bytes\n')
        for name in ('decide_turns.sh', 'stub_model.py', 'lifecycle.sh'):
            shutil.copy2(HARNESS / name, self.root / name)
        (self.root / 'decide').mkdir()
        (self.root / 'decide/scenarios.json').write_text(
            '{"fixture":{"files":{"calc.py":"pass\\n"},"prompts":["fixture prompt"],"turns":[[null]]}}\n')
        bindir = self.root / 'bin'
        bindir.mkdir()
        self.executable(bindir / 'angel', '#!/usr/bin/env python3\n'
                        'import json, os, sys, urllib.request\nfrom pathlib import Path\n'
                        f'root = Path({str(self.root)!r})\n'
                        "url = os.environ['ANGEL_LOCAL_URL']\n"
                        "if not (root / 'no-request').exists():\n"
                        "    request = urllib.request.Request(url + '/chat/completions', data=b'{\"messages\": []}', headers={'Content-Type': 'application/json'})\n"
                        '    with urllib.request.urlopen(request, timeout=2) as response: response.read()\n'
                        "(root / 'command.json').write_text(json.dumps({'home':os.environ['HOME'],'temporary':os.environ['TMPDIR'],'work':os.environ['ANGEL_WORKSPACE'],'url':url}))\n"
                        "print('fixture command stderr', file=sys.stderr)\n")
        self.executable(bindir / 'sleep', '#!/bin/sh\n'
                        'if [ "$1" = 4 ] && { [ "$DECIDE_OUTCOME" = SIGINT ] || [ "$DECIDE_OUTCOME" = SIGTERM ]; }; then exec /bin/sleep .3; fi\n'
                        'if [ "$1" = 0.1 ]; then exec /bin/sleep .025; fi\nexec /bin/sleep .005\n')
        self.executable(bindir / 'tmux', r'''#!/usr/bin/env python3
import json, os, pathlib, subprocess, sys, time
root = pathlib.Path(os.environ['DECIDE_ROOT'])
if sys.argv[1] == 'new-session':
    session = sys.argv[sys.argv.index('-s') + 1]
    work = sys.argv[sys.argv.index('-c') + 1]
    receipt = {'session':session, 'work':work}
    if os.environ['DECIDE_OUTCOME'] != 'tmux-failure':
        subprocess.run(['/bin/bash', '-c', sys.argv[-1]], cwd=work, check=True)
        child = subprocess.Popen([sys.executable, '-c',
            'import os,pathlib,signal,time; os.chdir(os.environ["DECIDE_WORK"]); '
            'signal.signal(signal.SIGTERM,signal.SIG_IGN); '
            'pathlib.Path(os.environ["DECIDE_ROOT"],"worker.ready").touch(); time.sleep(30)'],
            env=dict(os.environ, DECIDE_WORK=work), stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
        receipt['worker'] = child.pid
        deadline = time.monotonic() + 2
        while not (root / 'worker.ready').exists():
            assert child.poll() is None and time.monotonic() < deadline
            time.sleep(.005)
    (root / 'tmux.json').write_text(json.dumps(receipt))
    raise SystemExit(19 if os.environ['DECIDE_OUTCOME'] == 'tmux-failure' else 0)
elif sys.argv[1] == 'kill-session':
    (root / 'tmux.killed').write_text(sys.argv[sys.argv.index('-t') + 1])
''')

    def executable(self, path, body):
        path.write_text(body)
        path.chmod(0o755)

    def check_outcome(self, outcome):
        result = subprocess.run([sys.executable, '-c', CONTROLLER, str(self.root), outcome],
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=12)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_normal_completion_closes_stub_and_private_session(self):
        self.check_outcome('success')

    def test_no_request_idle_limit_is_reported_and_cleaned(self):
        (self.root / 'no-request').touch()
        self.check_outcome('idle-limit')

    def test_scenario_failure_cleans_private_fixtures(self):
        self.check_outcome('scenario-failure')

    def test_stub_failure_stops_before_tmux(self):
        self.check_outcome('stub-failure')

    def test_tmux_failure_closes_stub_and_owned_session_attempt(self):
        self.check_outcome('tmux-failure')

    def test_pid_only_interrupt_closes_session_and_private_workspace_jobs(self):
        self.check_outcome('SIGINT')

    def test_pid_only_termination_closes_session_and_private_workspace_jobs(self):
        self.check_outcome('SIGTERM')


@unittest.skipUnless(shutil.which('tmux'), 'requires local tmux')
class ExactTmuxTargetTests(unittest.TestCase):
    def test_dotted_runner_name_reaches_and_retires_only_its_exact_session(self):
        with tempfile.TemporaryDirectory(prefix='angel-tmux-target-') as tmp:
            base = ['tmux', '-S', str(Path(tmp) / 'socket')]
            name = 'decide-fixture.1-ABC'
            target = '=' + name + ':'
            unrelated = name + '-other'
            env = dict(os.environ, HOME=tmp, SHELL='/bin/sh')

            def tmux(*args, check=True):
                return subprocess.run([*base, *args], env=env, capture_output=True,
                                      text=True, timeout=3, check=check)

            try:
                tmux('new-session', '-d', '-s', name, 'cat')
                tmux('new-session', '-d', '-s', unrelated, 'cat')
                tmux('has-session', '-t', target)
                tmux('send-keys', '-t', target, '-l', 'exact-session-fixture')
                tmux('send-keys', '-t', target, 'Enter')
                deadline = time.monotonic() + 2
                while True:
                    screen = tmux('capture-pane', '-p', '-t', target).stdout
                    if 'exact-session-fixture' in screen:
                        break
                    self.assertLess(time.monotonic(), deadline)
                    time.sleep(.01)
                other = tmux('capture-pane', '-p', '-t', '=' + unrelated + ':').stdout
                self.assertNotIn('exact-session-fixture', other)
                tmux('kill-session', '-t', target)
                self.assertNotEqual(tmux('has-session', '-t', target, check=False).returncode, 0)
                tmux('has-session', '-t', '=' + unrelated + ':')
            finally:
                tmux('kill-server', check=False)


if __name__ == '__main__':
    unittest.main()
