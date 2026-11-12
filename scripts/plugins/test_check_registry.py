#!/usr/bin/env python3
"""Tests for check-registry.py: python3 scripts/plugins/test_check_registry.py"""

import importlib.util
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("check_registry", HERE / "check-registry.py")
cr = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cr)

GIT = ["git", "-c", "user.name=Tester", "-c", "user.email=tester@example.com",
       "-c", "init.defaultBranch=main", "-c", "commit.gpgsign=false"]

CARTRIDGE = """id = "hello"
label = "Hello"

[plugin]
version = "0.1.0"
description = "Says hello."
authors = ["Tester"]
license = "MIT"

[[commands]]
name = "hello"
usage = "<who>"
skills = ["hello"]

[[tools]]
name = "hello_hi"
description = "Greets."
run = "bin/hello"
"""


def entry(**changes):
    base = {
        "label": "Hello", "version": "0.1.0", "description": "Says hello.",
        "repository": "https://github.com/tester/hello", "commit": "a" * 40,
        "path": "plugins/hello", "sha256": "b" * 64, "license": "MIT", "authors": ["Tester"],
    }
    base.update(changes)
    return base


class Digest(unittest.TestCase):
    def test_spec_vector(self):
        # The same vector cockpit's plugins tests hold the Rust digest to.
        files = [("link", "l", b"a.txt"), ("bin/run", "x", b"#!/bin/sh\necho hi\n"), ("a.txt", "f", b"hello\n")]
        self.assertEqual(cr.digest(files), "d2f3d9d9350d8e1555cc17cf052c71c89c0216c7b813d2407fc69b1a063754f5")
        self.assertEqual(cr.digest([]), "887dd9868d358ebe96f1be0d915f3540bf64d61384c4dc84e7ba4adb0b50d006")

    def test_paths_and_links(self):
        self.assertTrue(cr.safe_path("bin/run"))
        for bad in ("", "/etc", "a/../b", "./a", ".git/config", "a//b"):
            self.assertFalse(cr.safe_path(bad), bad)
        self.assertTrue(cr.link_inside("bin/x", "../README.md"))
        self.assertFalse(cr.link_inside("bin/x", "../../x"))
        self.assertFalse(cr.link_inside("x", "/etc/passwd"))


class Entries(unittest.TestCase):
    def test_a_good_entry_passes(self):
        self.assertEqual(cr.entry_problems("hello", entry()), [])

    def test_the_rules(self):
        cases = {
            "repository": "http://example.com/x",
            "commit": "abc123",
            "sha256": "B" * 64,
            "version": "1.0",
            "description": "two\nlines",
            "authors": [],
            "path": "../up",
        }
        for key, value in cases.items():
            self.assertTrue(cr.entry_problems("hello", entry(**{key: value})), key)
        self.assertTrue(cr.entry_problems("Hello", entry()))
        self.assertTrue(cr.entry_problems("hello", entry(mystery=1)))
        self.assertTrue(cr.entry_problems("hello", entry(repository="https://me:pw@github.com/a/b")))


class Fetched(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        self.repo = root / "repo"
        plugin = self.repo / "plugins" / "hello"
        (plugin / "skills" / "hello").mkdir(parents=True)
        (plugin / "bin").mkdir()
        (plugin / "cartridge.toml").write_text(CARTRIDGE)
        (plugin / "skills" / "hello" / "SKILL.md").write_text("---\nname: hello\ndescription: hi\n---\nbody\n")
        (plugin / "bin" / "hello").write_text("#!/bin/sh\necho hi\n")
        os.chmod(plugin / "bin" / "hello", 0o755)
        (self.repo / "other.txt").write_text("not the plugin\n")
        subprocess.run([*GIT, "init", "-q"], cwd=self.repo, check=True)
        subprocess.run([*GIT, "add", "-A"], cwd=self.repo, check=True)
        subprocess.run([*GIT, "commit", "-q", "-m", "one"], cwd=self.repo, check=True)
        self.commit = subprocess.run([*GIT, "rev-parse", "HEAD"], cwd=self.repo, check=True,
                                     capture_output=True, text=True).stdout.strip()
        self.root = root

    def tearDown(self):
        self.tmp.cleanup()

    def fetch(self, name="scratch"):
        scratch = self.root / name
        scratch.mkdir()
        return cr.fetch(f"file://{self.repo}", self.commit, "plugins/hello", scratch)

    def test_fetch_takes_the_folder_and_the_plugin_agrees_with_its_entry(self):
        files = self.fetch()
        self.assertEqual(sorted(path for path, _, _ in files), ["bin/hello", "cartridge.toml", "skills/hello/SKILL.md"])
        self.assertEqual(dict((p, k) for p, k, _ in files)["bin/hello"], "x")
        good = entry(commit=self.commit, sha256=cr.digest(files))
        problems, cart = cr.plugin_problems("hello", good, files)
        self.assertEqual(problems, [])
        self.assertEqual(cart["id"], "hello")

    def test_disagreements_are_named(self):
        files = self.fetch()
        problems, _ = cr.plugin_problems("hello", entry(commit=self.commit, sha256="0" * 64, version="0.2.0"), files)
        text = "\n".join(problems)
        self.assertIn("digest to", text)
        self.assertIn("version", text)
        # A tool program that lost its mode, and a missing skill.
        broken = [(p, "f" if p == "bin/hello" else k, d) for p, k, d in files if p != "skills/hello/SKILL.md"]
        problems, _ = cr.plugin_problems("hello", entry(commit=self.commit, sha256=cr.digest(broken)), broken)
        text = "\n".join(problems)
        self.assertIn("not executable", text)
        self.assertIn("skill `hello` is not in skills/", text)

    def test_a_missing_commit_fails(self):
        scratch = self.root / "missing"
        scratch.mkdir()
        with self.assertRaises(RuntimeError):
            cr.fetch(f"file://{self.repo}", "0" * 40, "plugins/hello", scratch)


class Repository(unittest.TestCase):
    def test_this_repositorys_registry_passes_offline(self):
        registry = HERE.parent.parent / "plugins" / "registry.toml"
        run = subprocess.run([sys.executable, str(HERE / "check-registry.py"), str(registry), "--offline"],
                             capture_output=True, text=True)
        self.assertEqual(run.returncode, 0, run.stdout + run.stderr)


if __name__ == "__main__":
    unittest.main()
