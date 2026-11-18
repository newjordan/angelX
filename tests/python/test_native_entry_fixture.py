import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

if sys.platform == 'linux':
    SCRIPT = Path(__file__).resolve().parents[2] / 'scripts/check/native-entry-fixture.py'
    spec = importlib.util.spec_from_file_location('native_entry', SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)


@unittest.skipUnless(sys.platform == 'linux', 'Linux native-entry fixture')
class NativeEntryFixtureTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='angel-native-pty-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.fds = []
        original = module.pty.openpty

        def tracked():
            pair = original()
            self.fds.extend(pair)
            return pair

        self.mock = patch.object(module.pty, 'openpty', side_effect=tracked)
        self.mock.start()
        self.addCleanup(self.mock.stop)

    def assert_closed(self):
        self.assertEqual(len(self.fds), 2)
        for fd in self.fds:
            with self.assertRaises(OSError):
                os.fstat(fd)

    def test_spawn_failure_closes_both_descriptors(self):
        with self.assertRaises(FileNotFoundError):
            with module.terminal_process([str(self.root / 'absent')], env={}, cwd=self.root):
                self.fail('spawn failure must not yield')
        self.assert_closed()

    def test_terminal_setup_failure_closes_both_descriptors(self):
        error = OSError('injected terminal setup failure')
        with patch.object(module.fcntl, 'ioctl', side_effect=error):
            with self.assertRaises(OSError) as caught:
                with module.terminal_process([sys.executable], env={}, cwd=self.root):
                    self.fail('setup failure must not yield')
        self.assertIs(caught.exception, error)
        self.assert_closed()

    def test_normal_child_exit_closes_descriptors(self):
        with module.terminal_process([sys.executable, '-c', 'print("fixture")'],
                                     env={}, cwd=self.root) as (master, proc):
            self.assertEqual(proc.wait(timeout=5), 0)
            self.assertIn(b'fixture', os.read(master, 4096))
        self.assert_closed()

    def test_body_failure_terminates_and_reaps_child(self):
        error = RuntimeError('injected body failure')
        with self.assertRaises(RuntimeError) as caught:
            with module.terminal_process([sys.executable, '-c', 'import time; time.sleep(60)'],
                                         env={}, cwd=self.root) as (_, proc):
                raise error
        self.assertIs(caught.exception, error)
        self.assertIsNotNone(proc.returncode)
        with self.assertRaises(ProcessLookupError):
            os.kill(proc.pid, 0)
        self.assert_closed()


if __name__ == '__main__':
    unittest.main()
