import json
import os
from pathlib import Path
import runpy
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]


class PrivateStoreBridgeTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="angel-private-adapter-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.runtime = self.root / "scripts/runtime"
        self.runtime.mkdir(parents=True)
        for name in ("private_store_io.py", "private-store-bridge.py", "trajectory-redact.py"):
            shutil.copyfile(ROOT / "scripts/runtime" / name, self.runtime / name)
        (self.root / ".angel.env").write_text("", encoding="utf-8")
        self.env = {"PATH": os.defpath, "PYTHONDONTWRITEBYTECODE": "1"}

    def request(self, value, *, instrument=False):
        if instrument:
            code = """
import pathlib, runpy, sys
original = runpy.run_path
count = 0
def tracked(path, *args, **kwargs):
    global count
    if pathlib.Path(path).name == 'trajectory-redact.py':
        count += 1
    return original(path, *args, **kwargs)
runpy.run_path = tracked
sys.argv = [sys.argv[1]]
original(sys.argv[0], run_name='__main__')
print(count, file=sys.stderr)
"""
            argv = [sys.executable, "-I", "-B", "-c", code, str(self.runtime / "private-store-bridge.py")]
        else:
            argv = [sys.executable, "-I", "-B", str(self.runtime / "private-store-bridge.py")]
        return subprocess.run(argv, input=json.dumps(value).encode(), stdout=subprocess.PIPE,
                              stderr=subprocess.PIPE, env=self.env, timeout=10)

    def test_copy_loads_scanner_once_and_preserves_clean_bytes(self):
        for index, body in enumerate((b"", b"hello\r\n", "h\u00e9llo \u96ea\n".encode(),
                                      b'{"answer":42}\n', b'\x00\xff\xfe\r\n')):
            with self.subTest(index=index):
                source = self.root / "source"
                destination = self.root / "destination"
                source.write_bytes(body)
                result = self.request({"operation": "copy", "source": str(source),
                                       "path": str(destination)}, instrument=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout, b"")
                self.assertEqual(result.stderr, b"1\n")
                self.assertEqual(destination.read_bytes(), body)
                self.assertEqual(stat.S_IMODE(destination.stat().st_mode), 0o600)

    def test_module_reuse_still_refreshes_known_values(self):
        with patch.dict(os.environ, self.env, clear=True):
            private = runpy.run_path(str(self.runtime / "private_store_io.py"))
            scanner = private["_get_redactor"]()
            self.assertIs(private["_get_redactor"](), scanner)
            value = "fixture-value-with-no-shape-9283"
            self.assertEqual(private["redact_text"](value), value)
            os.environ["FIXTURE_SECRET"] = value
            self.assertNotIn(value, private["redact_text"](value))
            del os.environ["FIXTURE_SECRET"]
            self.assertEqual(private["redact_text"](value), value)
            (self.root / ".angel.env").write_text("FIXTURE_SECRET=" + value + "\n")
            self.assertNotIn(value, private["redact_text"](value))
            records, dropped = private["redact_records"]([{"note": value}])
            self.assertEqual(dropped, 0)
            self.assertNotIn(value, json.dumps(records))
            (self.root / ".angel.env").write_text("")
            self.assertEqual(private["redact_text"](value), value)

    def test_secret_binary_and_sealed_copy_refusal_preserve_destination(self):
        value = "synthetic-private-fixture-value-9371"
        (self.root / ".angel.env").write_text("FIXTURE_SECRET=" + value + "\n")
        source, destination = self.root / "source", self.root / "destination"
        for body in (b"\x00" + value.encode(),
                     json.dumps({"sha256": "binding", "note": value}).encode()):
            with self.subTest(binary=body.startswith(b"\x00")):
                source.write_bytes(body)
                destination.write_bytes(b"old complete document")
                destination.chmod(0o600)
                result = self.request({"operation": "copy", "source": str(source),
                                       "path": str(destination)})
                self.assertEqual(result.returncode, 2)
                self.assertNotIn(value.encode(), result.stderr + result.stdout)
                self.assertEqual(destination.read_bytes(), b"old complete document")
                self.assertEqual(stat.S_IMODE(destination.stat().st_mode), 0o600)
        self.assertEqual(sorted(p.name for p in self.root.iterdir()),
                         [".angel.env", "destination", "scripts", "source"])

    def test_nested_credentials_are_redacted_and_sealed_copy_is_refused(self):
        source, destination = self.root / "source", self.root / "destination"
        value = "synthetic-nested-fixture-value"
        record = {"password": {"value": value}, "note": "ordinary note"}
        source.write_text(json.dumps(record))
        result = self.request({"operation": "copy", "source": str(source),
                               "path": str(destination)})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(destination.read_text()),
                         {"password": {"value": "«redacted»"}, "note": "ordinary note"})
        self.assertEqual(stat.S_IMODE(destination.stat().st_mode), 0o600)
        self.assertNotIn(value.encode(), result.stderr + result.stdout)

        record["sha256"] = "fixture-binding"
        source.write_text(json.dumps(record))
        original = destination.read_bytes()
        result = self.request({"operation": "copy", "source": str(source),
                               "path": str(destination)})
        self.assertEqual(result.returncode, 2)
        self.assertEqual(destination.read_bytes(), original)
        self.assertNotIn(value.encode(), result.stderr + result.stdout)

    def test_failed_publication_preserves_old_file_and_cleans_staging(self):
        with patch.dict(os.environ, self.env, clear=True):
            private = runpy.run_path(str(self.runtime / "private_store_io.py"))
            destination = self.root / "destination"
            destination.write_bytes(b"old complete document")
            destination.chmod(0o600)
            error = OSError("injected replace failure")
            with patch.object(os, "replace", side_effect=error):
                with self.assertRaises(OSError) as caught:
                    private["replace_text"](destination, "new complete document")
            self.assertIs(caught.exception, error)
            self.assertEqual(destination.read_bytes(), b"old complete document")
            self.assertEqual(stat.S_IMODE(destination.stat().st_mode), 0o600)
            self.assertEqual(sorted(p.name for p in self.root.iterdir()),
                             [".angel.env", "destination", "scripts"])

    def test_unsafe_copy_destinations_remain_refused_and_untouched(self):
        source = self.root / "source"
        source.write_bytes(b"clean input")
        sentinel = self.root / "sentinel"
        sentinel.write_bytes(b"keep me")
        sentinel.chmod(0o640)
        destination = self.root / "destination"
        factories = [lambda: destination.symlink_to(sentinel),
                     lambda: os.link(sentinel, destination),
                     lambda: destination.mkdir(),
                     lambda: os.mkfifo(destination)]
        for index, create in enumerate(factories):
            with self.subTest(index=index):
                create()
                try:
                    before = destination.lstat()
                    result = self.request({"operation": "copy", "source": str(source),
                                           "path": str(destination)})
                    self.assertNotEqual(result.returncode, 0)
                    self.assertEqual(sentinel.read_bytes(), b"keep me")
                    self.assertEqual(stat.S_IMODE(sentinel.stat().st_mode), 0o640)
                    self.assertEqual(destination.lstat().st_ino, before.st_ino)
                    self.assertEqual(destination.lstat().st_mode, before.st_mode)
                finally:
                    if destination.is_dir():
                        destination.rmdir()
                    else:
                        destination.unlink()


class NestedCredentialTests(unittest.TestCase):
    def setUp(self):
        self.redactor = runpy.run_path(str(ROOT / "scripts/runtime/trajectory-redact.py"))

    def test_scanner_inherits_credential_context_through_objects(self):
        for payload in (
            {"value": "fixture-credential"},
            {"wrapper": {"value": "fixture-credential"}},
            [{"value": "fixture-credential"}],
            {"values": ["fixture-credential"]},
            {"value": json.dumps({"nested": "fixture-credential"})},
        ):
            with self.subTest(payload=payload):
                fixture = {"password": payload, "note": "ordinary note"}
                literal, shapes = self.redactor["scan_text"](json.dumps(fixture), [])
                self.assertEqual(literal, {})
                self.assertGreater(shapes.get("credential-field", 0), 0)

    def test_redactor_inherits_context_and_preserves_types_and_siblings(self):
        fixture = {
            "password": {"value": "fixture-credential", "empty": "", "count": 3,
                         "enabled": True, "missing": None,
                         "nested": [{"value": "fixture-second-credential"}]},
            "note": {"value": "ordinary note"},
        }
        before = json.loads(json.dumps(fixture))
        clean, hits = self.redactor["redact_record"](fixture, [])
        self.assertEqual(hits, 2)
        self.assertEqual(clean, {
            "password": {"value": self.redactor["REDACTED"], "empty": "", "count": 3,
                         "enabled": True, "missing": None,
                         "nested": [{"value": self.redactor["REDACTED"]}]},
            "note": {"value": "ordinary note"},
        })
        self.assertEqual(fixture, before, "the input record must not be mutated")
        self.assertEqual(self.redactor["scan_text"](json.dumps(clean), []), ({}, {}))
        self.assertEqual(self.redactor["redact_record"](clean, []), (clean, 0))

    def test_credential_context_survives_json_string_decoding(self):
        fixture = {"password": {"wrapper": json.dumps({"value": "fixture-credential"})}}
        clean, hits = self.redactor["redact_record"](fixture, [])
        self.assertEqual(hits, 1)
        self.assertEqual(clean["password"]["wrapper"], self.redactor["REDACTED"])
        literal, shapes = self.redactor["scan_text"](json.dumps(fixture), [])
        self.assertEqual(literal, {})
        self.assertGreater(shapes.get("credential-field", 0), 0)

    def test_sealed_nested_credentials_are_rejected_not_rewritten(self):
        record = {"sha256": "fixture-binding", "password": {"value": "fixture-credential"}}
        body = json.dumps(record) + "\n"
        with self.assertRaisesRegex(ValueError, "integrity-bound"):
            self.redactor["redact_document"](body, [], jsonl=True)
        rejections = {}
        clean, hits = self.redactor["redact_document"](body, [], jsonl=True, rejections=rejections)
        self.assertEqual((clean, hits), ("", 0))
        self.assertEqual(rejections, {"sealed_secret": 1})

    def test_noncredential_objects_and_existing_redactions_stay_unchanged(self):
        fixture = {"note": {"value": "ordinary note"},
                   "password": {"value": "«redacted»", "count": 2}}
        body = json.dumps(fixture, ensure_ascii=False, indent=2) + "\n"
        self.assertEqual(self.redactor["scan_text"](body, []), ({}, {}))
        self.assertEqual(self.redactor["redact_document"](body, [], jsonl=False), (body, 0))


if __name__ == "__main__":
    unittest.main()
