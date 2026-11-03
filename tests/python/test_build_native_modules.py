"""Edit-to-load builds must be incremental and preserve running generations."""
import ctypes
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[2] / 'scripts/runtime/build-native-modules.py'
MODULES = ('skill_router', 'activity_classifier', 'verify_shell', 'moa_router')


@unittest.skipUnless(shutil.which('cc'), 'requires a host C compiler')
class NativeModuleBuildTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='angel-native-build-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / 'cockpit/native'
        self.source.mkdir(parents=True)
        for name in MODULES:
            (self.source / f'{name}.c').write_text('int answer(void) { return 1; }\n')
        self.env = dict(os.environ, CC=shutil.which('cc'), CFLAGS='', ANGEL_NATIVE_BUILD_JOBS='1')

    def build(self, **env):
        return subprocess.run([sys.executable, str(SCRIPT), str(self.root)],
                              env={**self.env, **env}, capture_output=True, text=True, timeout=20)

    def outputs(self):
        return {name: self.root / 'cockpit/target/native' / f'{name}.so' for name in MODULES}

    def test_unchanged_sources_do_not_compile_or_replace_any_library(self):
        first = self.build()
        self.assertEqual(first.returncode, 0, first.stderr)
        before = {name: path.stat().st_mtime_ns for name, path in self.outputs().items()}
        second = self.build()
        self.assertEqual(second.returncode, 0, second.stderr)
        self.assertIn('0 compiled', second.stderr)
        self.assertEqual(before, {name: path.stat().st_mtime_ns for name, path in self.outputs().items()})

    def test_only_changed_source_is_published_and_old_generation_stays_valid(self):
        self.assertEqual(self.build().returncode, 0)
        paths = self.outputs()
        old = ctypes.CDLL(str(paths['skill_router']))
        before = {name: path.stat().st_ino for name, path in paths.items()}
        (self.source / 'skill_router.c').write_text('int answer(void) { return 2; }\n')
        result = self.build()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('1 compiled', result.stderr)
        self.assertNotEqual(paths['skill_router'].stat().st_ino, before['skill_router'])
        for name in MODULES[1:]:
            self.assertEqual(paths[name].stat().st_ino, before[name])
        self.assertEqual(old.answer(), 1)
        copy = self.root / 'new-generation.so'
        shutil.copyfile(paths['skill_router'], copy)
        self.assertEqual(ctypes.CDLL(str(copy)).answer(), 2)

    def test_failed_build_keeps_last_good_library_and_cleans_temporary_output(self):
        self.assertEqual(self.build().returncode, 0)
        image = self.outputs()['skill_router']
        before = image.read_bytes()
        (self.source / 'skill_router.c').write_text('not valid C!\n')
        result = self.build()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(image.read_bytes(), before)
        self.assertFalse(list(image.parent.glob('*.tmp')))

    def test_compiler_flags_and_output_corruption_invalidate_cache(self):
        self.assertEqual(self.build().returncode, 0)
        result = self.build(CFLAGS='-DNATIVE_CHANGED=1')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('4 compiled', result.stderr)
        self.outputs()['verify_shell'].write_bytes(b'corrupt')
        result = self.build(CFLAGS='-DNATIVE_CHANGED=1')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('1 compiled', result.stderr)
        self.assertEqual(ctypes.CDLL(str(self.outputs()['verify_shell'])).answer(), 1)

    def compiler_probe(self):
        """Compiler events, not a speed threshold, prove overlap and reaping."""
        wrapper = self.root / 'compiler with spaces'
        wrapper.write_text(f'''#!{sys.executable}
import json, os, subprocess, sys, time
from pathlib import Path
name = next(Path(s).stem for s in sys.argv[1:] if s.endswith('.c'))
events = Path(os.environ['EVENTS'])
def record(kind):
    fd = os.open(events, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
    os.write(fd, (json.dumps([kind, name]) + '\\n').encode()); os.close(fd)
record('start')
if os.environ.get('REQUIRE_OVERLAP') == '1' and name == 'skill_router':
    until = time.monotonic() + 2
    while 'activity_classifier' not in events.read_text():
        if time.monotonic() > until:
            record('end'); sys.exit(93)
        time.sleep(.005)
time.sleep(.03)
code = 7 if os.environ.get('FAIL_MODULE') == name else subprocess.call([{shutil.which('cc')!r}, *sys.argv[1:]])
record('end')
sys.exit(code)
''')
        wrapper.chmod(0o755)
        events = self.root / 'compiler-events.jsonl'
        return wrapper, events

    def test_parallel_build_is_bounded_and_joins_every_compiler(self):
        wrapper, events = self.compiler_probe()
        result = self.build(CC=str(wrapper), EVENTS=str(events), REQUIRE_OVERLAP='1', ANGEL_NATIVE_BUILD_JOBS='2')
        self.assertEqual(result.returncode, 0, result.stderr)
        active = peak = 0
        rows = [json.loads(row) for row in events.read_text().splitlines()]
        for kind, _ in rows:
            active += 1 if kind == 'start' else -1
            peak = max(active, peak)
        self.assertEqual(active, 0, 'all compiler children must be reaped')
        self.assertEqual(peak, 2, 'honor the selected bound, but overlap independent compiles')

    def test_parallel_compile_failure_keeps_entire_previous_batch(self):
        wrapper, events = self.compiler_probe()
        options = dict(CC=str(wrapper), EVENTS=str(events), ANGEL_NATIVE_BUILD_JOBS='2')
        self.assertEqual(self.build(**options).returncode, 0)
        before = {name: (path.read_bytes(), path.stat().st_ino) for name, path in self.outputs().items()}
        for name in MODULES:
            (self.source / f'{name}.c').write_text('int answer(void) { return 2; }\n')
        events.unlink()
        result = self.build(**options, FAIL_MODULE='activity_classifier')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(before, {name: (path.read_bytes(), path.stat().st_ino) for name, path in self.outputs().items()})
        rows = [json.loads(row) for row in events.read_text().splitlines()]
        self.assertCountEqual([n for k, n in rows if k == 'start'], [n for k, n in rows if k == 'end'])
        self.assertFalse(list(next(iter(self.outputs().values())).parent.glob('*.tmp')))

    def test_invalid_job_budget_does_not_publish(self):
        for value in ('0', '-1', 'unbounded'):
            result = self.build(ANGEL_NATIVE_BUILD_JOBS=value)
            self.assertNotEqual(result.returncode, 0, value)
            self.assertFalse(any(path.exists() for path in self.outputs().values()))

    def test_parent_jobserver_keeps_one_implicit_slot(self):
        wrapper, events = self.compiler_probe()
        result = self.build(CC=str(wrapper), EVENTS=str(events), ANGEL_NATIVE_BUILD_JOBS='4',
                            CARGO_MAKEFLAGS='--jobserver-auth=3,4')
        self.assertEqual(result.returncode, 0, result.stderr)
        active = peak = 0
        for kind, _ in map(json.loads, events.read_text().splitlines()):
            active += 1 if kind == 'start' else -1
            peak = max(active, peak)
        self.assertEqual((active, peak), (0, 1))

    def test_changed_inputs_abort_the_whole_compiled_batch(self):
        self.assertEqual(self.build().returncode, 0)
        before = {name: path.read_bytes() for name, path in self.outputs().items()}
        wrapper = self.root / 'mutating compiler'
        changed = self.source / 'skill_router.c'
        wrapper.write_text(f'''#!{sys.executable}
import subprocess, sys
from pathlib import Path
code = subprocess.call([{shutil.which('cc')!r}, *sys.argv[1:]])
Path({str(changed)!r}).write_text('int answer(void) {{ return 3; }}\\n')
sys.exit(code)
''')
        wrapper.chmod(0o755)
        result = self.build(CC=str(wrapper), ANGEL_NATIVE_BUILD_JOBS='2')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('changed during compilation', result.stderr)
        self.assertEqual(before, {name: path.read_bytes() for name, path in self.outputs().items()})
        self.assertFalse(list(next(iter(self.outputs().values())).parent.glob('*.tmp')))


if __name__ == '__main__':
    unittest.main()
