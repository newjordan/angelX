"""Pinned Blaze setup must be reusable offline without trusting stale/corrupt images."""
import contextlib
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[2] / "scripts/runtime/install-blaze.py"
IMAGES = ("blaze", "cargo-blaze", "ra-jit", "ra-jit-rustc")
TOOLCHAIN = "nightly-2026-10-27"
HOST = "x86_64-unknown-linux-gnu"


class BlazeInstallTests(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location("install_blaze", SCRIPT)
        self.install = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.install)
        self.temp = tempfile.TemporaryDirectory(prefix="angel-blaze-install-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.tools = self.root / "tools"
        self.source = self.tools / "blaze-src"
        self.destination = self.tools / "blaze"
        self.version = self.destination / self.install.REVISION
        self.sysroot = self.root / "toolchain"
        for key, value in (("TOOLS", self.tools), ("SOURCE", self.source),
                           ("DESTINATION", self.destination)):
            setattr(self.install, key, value)
        self.calls = []
        self.head = self.install.REVISION
        self.dirty = ""
        self.provision()
        self.make_source()
        self.make_install()
        self.install.run = self.fake_run

    def provision(self):
        for name in ("cargo", "rustc"):
            path = self.sysroot / "bin" / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("#!/bin/sh\nexit 0\n")
            path.chmod(0o755)
        source_files, wasm_files, host_files = [], [], []
        for name in ("core", "alloc", "std"):
            for relative, paths in (
                (f"lib/rustlib/src/rust/library/{name}/src/lib.rs", source_files),
                (f"lib/rustlib/wasm32-unknown-unknown/lib/lib{name}-fixture.rlib", wasm_files),
                (f"lib/rustlib/{HOST}/lib/lib{name}-fixture.rlib", host_files),
            ):
                path = self.sysroot / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"installed component fixture")
                paths.append(path)
        rustlib = self.sysroot / "lib/rustlib"
        components = {"rust-src": source_files, "rust-std-wasm32-unknown-unknown": wasm_files,
                      f"rust-std-{HOST}": host_files}
        (rustlib / "components").write_text("\n".join(components) + "\n")
        for name, paths in components.items():
            (rustlib / f"manifest-{name}").write_text("".join(
                f"file:{path.relative_to(self.sysroot).as_posix()}\n" for path in paths))

    def make_source(self):
        harness = self.source / "harness/toolchain"
        harness.parent.mkdir(parents=True, exist_ok=True)
        harness.write_text(TOOLCHAIN + "\n")
        for name in IMAGES:
            path = self.source / "target" / HOST / "release" / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(f"#!/bin/sh\n# {name}\nexit 0\n")
            path.chmod(0o755)

    def make_install(self):
        self.version.mkdir(parents=True, exist_ok=True)
        hashes = {}
        for name in IMAGES:
            path = self.version / name
            path.write_bytes((self.source / "target" / HOST / "release" / name).read_bytes())
            path.chmod(0o755)
            hashes[name] = hashlib.sha256(path.read_bytes()).hexdigest()
        self.receipt = {"repository": self.install.REPOSITORY,
                        "revision": self.install.REVISION, "toolchain": TOOLCHAIN,
                        "sha256": hashes}
        self.save_receipt()
        (self.destination / "selected").write_text(self.install.REVISION + "\n")

    def save_receipt(self):
        (self.version / "install.json").write_text(json.dumps(self.receipt) + "\n")

    def fake_run(self, *args, **kwargs):
        args = tuple(str(a) for a in args)
        self.calls.append((args, kwargs))
        if args[:1] == ("git",):
            if args[-2:] == ("rev-parse", "HEAD"):
                return subprocess.CompletedProcess(args, 0, self.head + "\n")
            if "status" in args:
                self.assertIn("--untracked-files=all", args)
                self.assertIn("--ignore-submodules=none", args)
                return subprocess.CompletedProcess(args, 0, self.dirty)
            raise AssertionError(f"unexpected git/network call {args}")
        if args[:2] == ("rustup", "which"):
            path = self.sysroot / "bin" / args[-1]
            if not path.exists():
                raise subprocess.CalledProcessError(1, args)
            return subprocess.CompletedProcess(args, 0, str(path) + "\n")
        if args[:3] == ("rustup", "toolchain", "install"):
            # Network-free fixtures opt in to this only in explicit repair tests.
            if not getattr(self, "allow_provision", False):
                raise AssertionError("unnecessary toolchain provisioning")
            self.provision()
            return subprocess.CompletedProcess(args, 0, "")
        if args == (str(self.sysroot / "bin/rustc"), "--print", "sysroot"):
            return subprocess.CompletedProcess(args, 0, str(self.sysroot) + "\n")
        if args == (str(self.sysroot / "bin/rustc"), "--print", "host-tuple"):
            return subprocess.CompletedProcess(args, 0, HOST + "\n")
        if args[:2] == (str(self.sysroot / "bin/cargo"), "build"):
            return subprocess.CompletedProcess(args, 0, "")
        raise AssertionError(f"unexpected subprocess {args}")

    def invoke(self, *args):
        output = io.StringIO()
        with patch.object(sys, "argv", [str(SCRIPT), *args]), contextlib.redirect_stdout(output):
            self.install.main()
        return output.getvalue()

    def assert_no_build_or_install(self):
        self.assertFalse(any(a[:3] == ("rustup", "toolchain", "install") or
                             a[:2] == (str(self.sysroot / "bin/cargo"), "build")
                             for a, _ in self.calls), self.calls)

    def test_reuse_verifies_images_without_build_provision_or_selector_rewrite(self):
        selector = self.destination / "selected"
        before = (selector.stat().st_ino, selector.stat().st_mtime_ns)
        self.assertIn("reused", self.invoke())
        self.assert_no_build_or_install()
        self.assertEqual(before, (selector.stat().st_ino, selector.stat().st_mtime_ns))

    def test_offline_reuse_needs_no_source_checkout(self):
        import shutil
        shutil.rmtree(self.source)
        self.assertIn("reused", self.invoke("--offline"))
        self.assert_no_build_or_install()
        self.assertFalse(self.source.exists())
        self.assertFalse(any(a[0] == "git" for a, _ in self.calls))

    def test_same_length_corruption_is_refused_before_build_or_selection(self):
        image = self.version / "ra-jit"
        data = image.read_bytes()
        image.write_bytes(data[:-1] + bytes([data[-1] ^ 1]))
        selector = self.destination / "selected"
        selector.write_text("previous-install\n")
        with self.assertRaisesRegex(SystemExit, "ra-jit.*(hash|SHA|digest)"):
            self.invoke()
        self.assertEqual(selector.read_text(), "previous-install\n")
        self.assert_no_build_or_install()

    def test_incomplete_install_is_not_trusted_or_silently_replaced(self):
        (self.version / "ra-jit-rustc").unlink()
        with self.assertRaisesRegex(SystemExit, "ra-jit-rustc"):
            self.invoke()
        self.assert_no_build_or_install()

    def test_non_executable_or_symlink_image_is_refused(self):
        image = self.version / "blaze"
        image.chmod(0o644)
        with self.assertRaisesRegex(SystemExit, "executable"):
            self.invoke()
        image.unlink()
        image.symlink_to(self.source / "target" / HOST / "release/blaze")
        with self.assertRaisesRegex(SystemExit, "regular|symlink"):
            self.invoke()
        self.assert_no_build_or_install()

    def test_malformed_or_oversized_receipt_is_refused(self):
        receipt = self.version / "install.json"
        for data in ("{", "[]", " " * 20000):
            with self.subTest(size=len(data)):
                receipt.write_text(data)
                with self.assertRaisesRegex(SystemExit, "receipt"):
                    self.invoke()
        self.assert_no_build_or_install()

    def test_receipt_identity_and_image_inventory_must_match_pin(self):
        for key, value in (("repository", "https://example.invalid/not-blaze"),
                           ("revision", "0" * 40), ("toolchain", "nightly-2000-01-01"),
                           ("sha256", {})):
            with self.subTest(key=key):
                original = self.receipt[key]
                self.receipt[key] = value
                self.save_receipt()
                with self.assertRaises(SystemExit):
                    self.invoke()
                self.receipt[key] = original
        self.assert_no_build_or_install()

    def test_existing_source_edits_or_wrong_revision_are_still_refused(self):
        self.dirty = " M crates/ra-jit/src/main.rs\n"
        with self.assertRaisesRegex(SystemExit, "local edits"):
            self.invoke()
        self.dirty = ""
        self.head = "0" * 40
        with self.assertRaisesRegex(SystemExit, "expected"):
            self.invoke()
        self.assert_no_build_or_install()

    def test_source_toolchain_must_agree_with_pinned_installer(self):
        (self.source / "harness/toolchain").write_text("nightly-2000-01-01\n")
        with self.assertRaisesRegex(SystemExit, "toolchain"):
            self.invoke()
        self.assert_no_build_or_install()

    def test_offline_missing_component_fails_without_network_or_selector_change(self):
        (self.sysroot / "lib/rustlib/src/rust/library/core/src/lib.rs").unlink()
        selector = self.destination / "selected"
        selector.write_text("previous\n")
        with self.assertRaisesRegex(SystemExit, "offline.*toolchain|toolchain.*offline"):
            self.invoke("--offline")
        self.assertEqual(selector.read_text(), "previous\n")
        self.assert_no_build_or_install()

    def test_partial_toolchain_is_repaired_only_when_not_offline(self):
        (self.sysroot / "lib/rustlib/wasm32-unknown-unknown/lib/libstd-fixture.rlib").unlink()
        self.allow_provision = True
        self.assertIn("reused", self.invoke())
        self.assertEqual(sum(a[:3] == ("rustup", "toolchain", "install")
                             for a, _ in self.calls), 1)
        self.assertFalse(any(a[:2] == (str(self.sysroot / "bin/cargo"), "build")
                             for a, _ in self.calls))

    def test_missing_rustc_offline_is_actionable_and_does_not_install(self):
        (self.sysroot / "bin/rustc").unlink()
        with self.assertRaisesRegex(SystemExit, "offline.*toolchain|toolchain.*offline"):
            self.invoke("--offline")
        self.assert_no_build_or_install()

    def test_selector_can_be_restored_without_rebuilding_valid_install(self):
        (self.destination / "selected").unlink()
        self.assertIn("reused", self.invoke("--offline"))
        self.assertEqual((self.destination / "selected").read_text(), self.install.REVISION + "\n")
        self.assert_no_build_or_install()

    def test_cold_offline_build_retains_wrapper_isolation_and_writes_valid_receipt(self):
        import shutil
        shutil.rmtree(self.version)
        (self.destination / "selected").unlink()
        removed = ("CARGO_TARGET_DIR", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTFLAGS")
        overridden = ("CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET_DIR",
                      "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_TARGET")
        pollution = {name: "must-not-propagate" for name in (*removed, *overridden)}
        with patch.dict(os.environ, pollution):
            self.assertIn("installed", self.invoke("--offline"))
        calls = [(a, kw) for a, kw in self.calls if len(a) > 1 and a[1] == "build"]
        self.assertEqual(len(calls), 1)
        args, kw = calls[0]
        self.assertIn("--offline", args)
        self.assertIn("--locked", args)
        self.assertTrue(all(name not in kw["env"] for name in removed))
        self.assertEqual(kw["env"]["CARGO_BUILD_TARGET_DIR"], str(self.source / "target"))
        self.assertEqual(kw["env"]["CARGO_ENCODED_RUSTFLAGS"], "")
        self.assertEqual(kw["env"]["CARGO_BUILD_RUSTC_WRAPPER"], "")
        self.assertEqual(kw["env"]["CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"], "")
        self.assertIn('build.rustc-wrapper=""', args)
        self.assertIn('build.rustc-workspace-wrapper=""', args)
        self.assertEqual(args[args.index("--target-dir") + 1], str(self.source / "target"))
        self.assertEqual(args[args.index("--target") + 1], HOST)
        self.assertEqual(kw["env"]["RUSTC"], str(self.sysroot / "bin/rustc"))
        self.assertEqual(json.loads((self.version / "install.json").read_text()), self.receipt)
        self.calls.clear()
        self.assertIn("reused", self.invoke("--offline"))
        self.assert_no_build_or_install()

    def test_failed_build_preserves_previous_selector_and_publishes_nothing(self):
        import shutil
        shutil.rmtree(self.version)
        selector = self.destination / "selected"
        selector.write_text("previous\n")
        base_run = self.fake_run

        def fail_build(*args, **kwargs):
            if len(args) > 1 and args[1] == "build":
                raise subprocess.CalledProcessError(2, args)
            return base_run(*args, **kwargs)

        self.install.run = fail_build
        with self.assertRaises(subprocess.CalledProcessError):
            self.invoke("--offline")
        self.assertFalse(self.version.exists())
        self.assertEqual(selector.read_text(), "previous\n")
        self.assertFalse(list(self.destination.glob("install-*")))

    def test_cold_offline_without_source_does_not_fetch(self):
        import shutil
        shutil.rmtree(self.source)
        shutil.rmtree(self.version)
        with self.assertRaisesRegex(SystemExit, "offline.*source|source.*offline"):
            self.invoke("--offline")
        self.assertEqual(self.calls, [])

    def test_component_inventory_detects_missing_alloc_host_or_registered_file(self):
        for relative in ("lib/rustlib/src/rust/library/alloc/src/lib.rs",
                         f"lib/rustlib/{HOST}/lib/libcore-fixture.rlib",
                         "lib/rustlib/wasm32-unknown-unknown/lib/liballoc-fixture.rlib"):
            with self.subTest(relative=relative):
                path = self.sysroot / relative
                content = path.read_bytes()
                path.unlink()
                with self.assertRaisesRegex(SystemExit, "offline.*toolchain|toolchain.*offline"):
                    self.invoke("--offline")
                path.write_bytes(content)
        self.assert_no_build_or_install()

    def test_install_lock_rejects_concurrent_setup_and_releases_on_error(self):
        lock = self.tools / ".blaze-install.lock"
        lock.mkdir()
        with self.assertRaisesRegex(SystemExit, "already running"):
            self.invoke("--offline")
        self.assertTrue(lock.is_dir())
        self.assertEqual(self.calls, [])
        lock.rmdir()
        (self.version / "blaze").unlink()
        with self.assertRaises(SystemExit):
            self.invoke("--offline")
        self.assertFalse(lock.exists())

    @unittest.skipUnless(__import__("shutil").which("git"), "requires Git")
    def test_real_git_config_cannot_hide_untracked_source_edits(self):
        git = ["git", "-C", str(self.source)]
        subprocess.run([*git, "init", "-q"], check=True, capture_output=True)
        subprocess.run([*git, "add", "harness/toolchain"], check=True, capture_output=True)
        subprocess.run([*git, "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                        "commit", "-qm", "fixture"], check=True, capture_output=True)
        subprocess.run([*git, "config", "status.showUntrackedFiles", "no"], check=True)
        subprocess.run([*git, "config", "core.excludesFile", "/dev/null"], check=True)
        revision = subprocess.check_output([*git, "rev-parse", "HEAD"], text=True).strip()
        (self.source / ".git/info/exclude").write_text("target/\n")
        self.install.REVISION = revision
        self.install.run = lambda *a, **kw: subprocess.run(a, check=True, text=True, **kw)
        self.install.check_source()
        (self.source / "local.rs").write_text("// added source\n")
        with self.assertRaisesRegex(SystemExit, "local edits"):
            self.install.check_source()


if __name__ == "__main__":
    unittest.main()
