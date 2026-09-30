"""Private benchmark copies must not share source files or Git state."""

import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock


SCRIPT = Path(__file__).resolve().parents[2] / "scripts/bench/book-harness/copy_fixture.py"
spec = importlib.util.spec_from_file_location("fixture_snapshot", SCRIPT)
snapshot = importlib.util.module_from_spec(spec)
spec.loader.exec_module(snapshot)


class SnapshotTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="angel-fixture-snapshot-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source, self.destination = self.root / "source", self.root / "copy"
        self.source.mkdir()
        self.destination.mkdir()
        self.env = {"PATH": os.environ["PATH"], "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_CONFIG_NOSYSTEM": "1"}

    def git(self, root, *args):
        return subprocess.check_output(["git", "-C", str(root), "-c", "user.name=fixture",
                                        "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false", *args],
                                       env=self.env, text=True, stderr=subprocess.DEVNULL).strip()

    def repository(self):
        self.git(self.source, "-c", "init.templateDir=/dev/null", "init", "-q")
        (self.source / "input.txt").write_text("source bytes\n")
        self.git(self.source, "add", "input.txt")
        self.git(self.source, "commit", "-qm", "fixture")
        self.git(self.source, "remote", "add", "origin", "https://fixture.invalid/unused")

    def test_linked_worktree_copy_has_private_metadata_and_preserves_source_remote(self):
        self.repository()
        linked = self.root / "linked"
        self.git(self.source, "worktree", "add", "-q", "-b", "experiment", str(linked))
        with mock.patch.dict(os.environ, {"GIT_DIR": str(self.source / ".git"),
                                         "GIT_WORK_TREE": str(self.source), "GIT_INDEX_FILE": str(self.source / ".git/index")}):
            snapshot.snapshot_fixture(linked, self.destination)
        self.assertTrue((self.destination / ".git").is_dir())
        self.assertEqual(self.git(self.destination, "remote"), "")
        self.assertEqual(self.git(self.destination, "status", "--porcelain"), "")
        self.git(self.destination, "remote", "add", "origin", "https://copy.invalid/unused")
        self.git(self.destination, "remote", "remove", "origin")
        (self.destination / "input.txt").write_text("experiment changed\n")
        self.assertEqual(self.git(self.source, "remote", "get-url", "origin"), "https://fixture.invalid/unused")
        self.assertEqual((self.source / "input.txt").read_text(), "source bytes\n")
        self.assertEqual(self.git(self.source, "status", "--porcelain"), "")

    def test_file_and_directory_links_become_private_bytes_with_executable_mode(self):
        external = self.root / "external"
        external.mkdir()
        script = external / "tool"
        script.write_text("#!/bin/sh\nexit 0\n")
        script.chmod(0o755)
        (self.source / "tool").symlink_to(script)
        (self.source / "dependencies").symlink_to(external, target_is_directory=True)
        os.link(script, self.source / "hardlink")
        snapshot.snapshot_fixture(self.source, self.destination)
        for name in ("tool", "hardlink", "dependencies/tool"):
            path = self.destination / name
            self.assertFalse(path.is_symlink())
            self.assertNotEqual(path.stat().st_ino, script.stat().st_ino)
            self.assertEqual(path.stat().st_mode & 0o777, 0o755)
            path.write_text("copy changed\n")
        self.assertEqual(script.read_text(), "#!/bin/sh\nexit 0\n")

    def test_tracked_ignored_files_remain_observable_without_indexing_dependencies(self):
        self.repository()
        (self.source / ".gitignore").write_text("input.txt\n.venv/\nweird*\n")
        dependency = self.source / ".venv/tool"
        dependency.parent.mkdir()
        dependency.write_text("dependency bytes\n")
        special = self.source / "weird[1]\nfile"
        special.write_text("tracked bytes\n")
        self.git(self.source, "add", ".gitignore")
        self.git(self.source, "--literal-pathspecs", "add", "-f", special.name)
        self.git(self.source, "commit", "-qm", "ignored fixture input")
        snapshot.snapshot_fixture(self.source, self.destination)
        self.assertEqual(self.git(self.destination, "ls-files", "input.txt"), "input.txt")
        self.assertEqual(self.git(self.destination, "ls-files", ".venv"), "")
        self.assertEqual((self.destination / ".venv/tool").read_text(), "dependency bytes\n")
        self.assertEqual(self.git(self.destination, "status", "--porcelain"), "")
        (self.destination / "input.txt").write_text("experiment changed\n")
        self.assertIn("input.txt", self.git(self.destination, "status", "--porcelain"))
        self.assertEqual(self.git(self.source, "status", "--porcelain"), "")

    def test_nested_git_pointers_and_metadata_links_are_excluded(self):
        self.repository()
        nested = self.source / "nested"
        nested.mkdir()
        (nested / ".git").write_text(f"gitdir: {self.source / '.git'}\n")
        (nested / "data").write_text("nested bytes\n")
        another = self.source / "another"
        another.mkdir()
        (another / ".git").symlink_to(self.source / ".git", target_is_directory=True)
        snapshot.snapshot_fixture(self.source, self.destination)
        self.assertFalse((self.destination / "nested/.git").exists())
        self.assertFalse((self.destination / "another/.git").exists())
        self.assertEqual((self.destination / "nested/data").read_text(), "nested bytes\n")
        self.assertTrue((self.source / ".git").is_dir())

    def test_directory_cycles_and_links_into_destination_fail_without_git_initialization(self):
        (self.source / "cycle").symlink_to(self.source, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "cycle"):
            snapshot.snapshot_fixture(self.source, self.destination)
        self.assertFalse((self.destination / ".git").exists())
        (self.source / "cycle").unlink()
        (self.source / "output").symlink_to(self.destination, target_is_directory=True)
        other = self.root / "other-copy"
        other.mkdir()
        # A source link reaching its actual output is forbidden as well.
        (self.source / "output").unlink()
        (self.source / "output").symlink_to(other, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "destination"):
            snapshot.snapshot_fixture(self.source, other)

    def test_nonempty_or_nested_destination_is_rejected_without_touching_caller_files(self):
        retained = self.destination / "retained"
        retained.write_text("caller bytes\n")
        with self.assertRaisesRegex(ValueError, "empty"):
            snapshot.snapshot_fixture(self.source, self.destination)
        self.assertEqual(retained.read_text(), "caller bytes\n")
        nested = self.source / "nested-output"
        nested.mkdir()
        with self.assertRaisesRegex(ValueError, "outside"):
            snapshot.snapshot_fixture(self.source, nested)
        with self.assertRaisesRegex(ValueError, "outside"):
            snapshot.snapshot_fixture(self.source, self.source)

    @unittest.skipUnless(hasattr(os, "mkfifo"), "requires a POSIX fixture")
    def test_special_files_fail_instead_of_blocking_copy(self):
        os.mkfifo(self.source / "pipe")
        with self.assertRaisesRegex(ValueError, "unsupported file type"):
            snapshot.snapshot_fixture(self.source, self.destination)
        self.assertFalse((self.destination / ".git").exists())


if __name__ == "__main__":
    unittest.main()
