import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/check/cockpit-source-digest.sh"
OLD_INPUTS = [
    "cockpit/input", "crates/input", "vendor/dotmax/input", "vendor/ureq/input",
    "rust-toolchain.toml", "docs/telemetry/model-calibration.toml",
    "docs/telemetry/store-caps.toml", "docs/telemetry/prices.toml",
]
RUNTIME_INPUTS = [
    "scripts/runtime/repo-recon.js", "scripts/runtime/test-runner-report.mjs",
    "lib/test-runner/report.mjs", "lib/club/moa-preferences.json",
    "lib/tools/definitions.json",
]


class SourceDigestTests(unittest.TestCase):
    def test_runtime_inputs_change_identity_and_old_revisions_keep_v3(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            env = {**os.environ, "GIT_AUTHOR_NAME": "Test", "GIT_AUTHOR_EMAIL": "test@example.invalid",
                   "GIT_COMMITTER_NAME": "Test", "GIT_COMMITTER_EMAIL": "test@example.invalid"}

            def git(*args):
                return subprocess.check_output(["git", *args], cwd=root, env=env, text=True).strip()

            def write(path, content):
                target = root / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text(content, encoding="utf-8")

            def digest(*args):
                return subprocess.check_output(["bash", str(root / "scripts/check/cockpit-source-digest.sh"), *args], cwd=root, env=env, text=True).strip()

            git("init", "-q")
            for path in OLD_INPUTS:
                if path == "crates/input":
                    continue
                write(path, "baseline\n")
            destination = root / "scripts/check/cockpit-source-digest.sh"
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_text(SCRIPT.read_text().replace(
                "# Source identity inventory: angel-source-v8\n", ""), encoding="utf-8")
            git("add", ".")
            git("commit", "-qm", "v2 inputs")
            old_v2 = git("rev-parse", "HEAD")
            self.assertEqual(digest("--scheme"), "angel-source-v2")
            v2_digest = digest("--commit", old_v2)
            write("crates/input", "baseline\n")
            git("add", ".")
            git("commit", "-qm", "old inputs")
            old = git("rev-parse", "HEAD")
            self.assertEqual(digest("--scheme"), "angel-source-v3")
            old_digest = digest("--commit", old)
            old_paths = ["cockpit", "crates", "vendor/dotmax", "vendor/ureq", "rust-toolchain.toml",
                         "docs/telemetry/model-calibration.toml", "docs/telemetry/store-caps.toml",
                         "docs/telemetry/prices.toml"]
            object_ids = git("rev-parse", *(f"{old}:{path}" for path in old_paths))
            expected = hashlib.sha256(f"angel-source-v3\n{object_ids}\n".encode()).hexdigest()
            self.assertEqual(old_digest, expected)

            write(".gitignore", RUNTIME_INPUTS[0] + "\n")
            write(RUNTIME_INPUTS[0], "ignored\n")
            self.assertIn(RUNTIME_INPUTS[0], digest("--dirty"))
            self.assertNotEqual(subprocess.run(
                ["bash", str(destination)], cwd=root, env=env,
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            ).returncode, 0)
            (root / RUNTIME_INPUTS[0]).unlink()
            (root / ".gitignore").unlink()

            for path in RUNTIME_INPUTS:
                write(path, "first\n")
            self.assertTrue(all(path in digest("--dirty") for path in RUNTIME_INPUTS))
            git("add", ".")
            git("commit", "-qm", "embedded modules")
            self.assertEqual(digest("--scheme"), "angel-source-v4")
            self.assertEqual(digest("--commit", old), old_digest)
            self.assertEqual(digest("--commit", old_v2), v2_digest)
            v4 = git("rev-parse", "HEAD")
            v4_digest = digest("--commit", v4)
            help_path = "lib/control/slash-help.json"
            write(help_path, "first\n")
            self.assertIn(help_path, digest("--dirty"))
            git("add", help_path)
            git("commit", "-qm", "slash help catalog")
            self.assertEqual(digest("--scheme"), "angel-source-v5")
            self.assertEqual(digest("--commit", v4), v4_digest)
            v5 = git("rev-parse", "HEAD")
            v5_digest = digest("--commit", v5)
            new_paths = ["lib/knowledge/lesson-routing.json"]
            for path in new_paths:
                write(path, "first\n")
                self.assertIn(path, digest("--dirty"))
            git("add", *new_paths)
            git("commit", "-qm", "lesson routing module")
            self.assertEqual(digest("--scheme"), "angel-source-v6")
            self.assertEqual(digest("--commit", v5), v5_digest)
            v6 = git("rev-parse", "HEAD")
            v6_digest = digest("--commit", v6)
            native_path = "lib/native/native_module.rs"
            write(native_path, "first\n")
            self.assertIn(native_path, digest("--dirty"))
            git("add", native_path)
            git("commit", "-qm", "shared native loader")
            self.assertEqual(digest("--scheme"), "angel-source-v7")
            self.assertEqual(digest("--commit", v6), v6_digest)
            catalog_path = "delve/kingdom/frontier.json"
            write(catalog_path, "first\n")
            git("add", catalog_path)
            git("commit", "-qm", "historical catalogue before inventory upgrade")
            v7 = git("rev-parse", "HEAD")
            v7_digest = digest()
            self.assertEqual(digest("--scheme"), "angel-source-v7")
            shutil.copyfile(SCRIPT, destination)
            git("add", str(destination.relative_to(root)))
            git("commit", "-qm", "catalogue source inventory", "--allow-empty")
            self.assertEqual(digest("--scheme"), "angel-source-v8")
            self.assertEqual(digest("--commit", v7), v7_digest)
            before = digest()
            for path in [*RUNTIME_INPUTS, help_path, *new_paths, native_path, catalog_path]:
                write(path, "changed\n")
                self.assertIn(path, digest("--dirty"))
                git("add", path)
                git("commit", "-qm", "change embedded module")
                after = digest()
                self.assertNotEqual(before, after, path)
                before = after

            write("docs/unrelated.md", "does not affect the binary\n")
            git("add", "docs/unrelated.md")
            git("commit", "-qm", "unrelated docs")
            self.assertEqual(digest(), before)
            for path in [help_path, native_path]:
                content = (root / path).read_text()
                git("rm", "-q", path)
                git("commit", "-qm", "remove previous scheme prerequisite")
                self.assertEqual(digest("--scheme"), "angel-source-v8")
                missing = subprocess.run(["bash", str(destination)], cwd=root, env=env,
                                         capture_output=True, text=True)
                self.assertEqual(missing.returncode, 128)
                self.assertEqual(missing.stdout, "")
                write(path, content)
                git("add", path)
                git("commit", "-qm", "restore prerequisite")
                self.assertEqual(digest(), before)
            git("rm", "-q", catalog_path)
            git("commit", "-qm", "remove required catalogue")
            missing = subprocess.run(["bash", str(destination)], cwd=root, env=env,
                                     capture_output=True, text=True)
            self.assertNotEqual(missing.returncode, 0)
            self.assertEqual(missing.stdout, "")
            write(".gitignore", catalog_path + "\n")
            git("add", ".gitignore")
            git("commit", "-qm", "ignored catalogue")
            write(catalog_path, "ignored\n")
            self.assertIn(catalog_path, digest("--dirty"))
            ignored = subprocess.run(["bash", str(destination)], cwd=root, env=env,
                                     capture_output=True, text=True)
            self.assertEqual(ignored.returncode, 3)
            self.assertEqual(ignored.stdout, "")


if __name__ == "__main__":
    unittest.main()
