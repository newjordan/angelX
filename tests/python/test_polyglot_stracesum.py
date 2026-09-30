"""Successful descriptor lifecycle accounting uses synthetic strace only."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock


SCRIPT = Path(__file__).resolve().parents[2] / "scripts/bench/book-harness/stracesum.py"


def load():
    spec = importlib.util.spec_from_file_location("stracesum_test", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


summary = load()


def analyze(*calls):
    return summary.analyze(f"1001 12:00:00.{index:06d} {call}\n" for index, call in enumerate(calls))


def opened(path="/tmp/file-a", fd=3):
    return f'openat(AT_FDCWD, "{path}", O_WRONLY) = {fd}'


class StraceSummaryTests(unittest.TestCase):
    def test_import_does_not_read_argv_or_inputs(self):
        with mock.patch.object(sys, "argv", ["stracesum.py", "/missing"]), \
                mock.patch.object(Path, "open", side_effect=AssertionError("input read")), \
                contextlib.redirect_stdout(io.StringIO()) as out:
            load()
        self.assertEqual(out.getvalue(), "")

    def test_closed_descriptor_reused_by_socket_has_explicit_unknown_target(self):
        result = analyze(opened(), "close(3) = 0", "socket(AF_INET, SOCK_STREAM, 0) = 3",
                         'write(3, "fixture", 7) = 7', "fsync(3) = -1 EINVAL (Invalid argument)")
        self.assertEqual(result["wbytes"]["/tmp/file-a"], 0)
        self.assertEqual(result["wbytes"]["fd3 [unknown]"], 7)
        self.assertEqual(result["coverage"]["unknown_write_bytes"], 7)
        self.assertEqual(result["coverage"]["unknown_sync_calls"], 1)
        self.assertIn("fd3 [unknown]", summary.render(result))

    def test_dup2_replacement_rebinds_write_and_sync_to_source_file(self):
        result = analyze(opened(), opened("/tmp/file-b", 8), "dup2(3, 8) = 8",
                         'write(8, "fixture", 7) = 7', "fdatasync(8) = 0")
        self.assertEqual(result["wbytes"], {"/tmp/file-a": 7})
        self.assertEqual(result["syncs"], {"/tmp/file-a": 1})
        self.assertEqual(result["coverage"]["successful_duplications"], 1)

    def test_alias_survives_original_close_and_ends_on_own_close(self):
        result = analyze(opened(), "dup(3) = 4", "close(3) = 0", 'write(4, "fixture", 7) = 7',
                         "close(4) = 0", 'write(4, "fixture", 7) = 7')
        self.assertEqual(result["wbytes"], {"/tmp/file-a": 7, "fd4 [unknown]": 7})
        self.assertEqual(result["coverage"]["successful_closes"], 2)

    def test_dup3_and_fcntl_duplication_keep_known_descriptor_path(self):
        for operation in ("dup3(3, 8, O_CLOEXEC) = 8", "fcntl(3, F_DUPFD, 8) = 8",
                          "fcntl64(3, F_DUPFD_CLOEXEC, 8) = 8"):
            with self.subTest(operation=operation):
                result = analyze(opened(), operation, 'pwrite64(8, "fixture", 7, 0) = 7')
                self.assertEqual(result["wbytes"], {"/tmp/file-a": 7})
                self.assertEqual(result["coverage"]["successful_duplications"], 1)
        self.assertIn("exec-close", result["scope"])

    def test_duplicate_unknown_source_clears_stale_target_path(self):
        result = analyze(opened("/tmp/file-b", 8), "dup2(3, 8) = 8", 'write(8, "fixture", 7) = 7')
        self.assertEqual(result["wbytes"], {"fd8 [unknown]": 7})
        self.assertEqual(result["coverage"]["unknown_write_calls"], 1)

    def test_failed_unrelated_close_or_dup_does_not_erase_known_paths(self):
        result = analyze(opened(), opened("/tmp/file-b", 8), "close(99) = -1 EBADF (Bad file descriptor)",
                         "dup2(99, 8) = -1 EBADF (Bad file descriptor)",
                         'write(3, "fixture", 7) = 7', 'write(8, "fixture", 7) = 7')
        self.assertEqual(result["wbytes"], {"/tmp/file-a": 7, "/tmp/file-b": 7})
        self.assertEqual(result["coverage"]["successful_closes"], 0)
        self.assertEqual(result["coverage"]["successful_duplications"], 0)

    def test_same_descriptor_dup2_preserves_binding(self):
        result = analyze(opened(), "dup2(3, 3) = 3", 'write(3, "fixture", 7) = 7')
        self.assertEqual(result["wbytes"], {"/tmp/file-a": 7})

    def test_descriptor_maps_are_isolated_by_recorded_pid(self):
        result = summary.analyze([
            '1001 12:00:00.000 openat(AT_FDCWD, "/tmp/file-a", O_WRONLY) = 3\n',
            '1002 12:00:00.001 close(3) = 0\n',
            '1002 12:00:00.002 dup2(3, 8) = 8\n',
            '1001 12:00:00.003 write(3, "fixture", 7) = 7\n',
            '1002 12:00:00.004 write(8, "fixture", 7) = 7\n'])
        self.assertEqual(result["wbytes"], {"/tmp/file-a": 7, "fd8 [unknown]": 7})

    def test_measured_zero_write_and_failed_write_keep_original_accounting(self):
        result = analyze(opened(), 'write(3, "", 0) = 0', 'write(3, "fixture", 7) = -1 EIO (Input/output error)')
        self.assertEqual(result["wbytes"], {"/tmp/file-a": 0})
        self.assertEqual(result["wcalls"], {"/tmp/file-a": 1})
        self.assertEqual(result["coverage"]["unknown_write_calls"], 0)

    def test_complete_known_descriptor_rendering_is_exactly_compatible(self):
        result = analyze(opened("/tmp/angel-bench-ws-AbCd/path123456"),
                         'write(3, "fixture", 7) = 7', 'pwrite64(3, "hi", 2, 0) = 2',
                         "fsync(3) = 0", 'execve("/usr/bin/python3", ["python3"], []) = 0',
                         'rename("/tmp/old", "/tmp/new123456") = 0')
        self.assertEqual(summary.render(result), "== bytes written by path (top 25)\n"
                         "           9      2 <ws>/pathN\n== opens (top 25)\n     1 <ws>/pathN\n"
                         "== execs\n     1 /usr/bin/python3\n== fsyncs\n     1 <ws>/pathN\n"
                         "== renames\n     1 /tmp/newN\n")

    def test_unfinished_and_resumed_syscalls_are_visible_as_unmodeled(self):
        result = analyze('openat(AT_FDCWD, "/tmp/file-a", O_WRONLY <unfinished ...>',
                         '<... openat resumed>) = 3', 'write(3, "fixture", 7) = 7')
        self.assertEqual(result["coverage"]["unfinished_or_resumed_lines"], 2)
        self.assertEqual(result["wbytes"], {"fd3 [unknown]": 7})
        self.assertIn("unfinished-syscall", result["scope"])

    def test_cli_audit_and_missing_input_are_import_safe_and_read_only(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "fixture.strace"
            path.write_text('1001 12:00:00.000 write(4, "fixture", 7) = 7\n')
            original = path.read_bytes()
            with contextlib.redirect_stdout(io.StringIO()) as out:
                self.assertEqual(summary.main([str(path), "--audit-json"]), 0)
            result = json.loads(out.getvalue())
            self.assertEqual(result["coverage"]["unknown_write_bytes"], 7)
            self.assertEqual(path.read_bytes(), original)
            with contextlib.redirect_stdout(io.StringIO()) as out, contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit):
                    summary.main([str(path) + "-missing"])
            self.assertEqual(out.getvalue(), "")


if __name__ == "__main__":
    unittest.main()
