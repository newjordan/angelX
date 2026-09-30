"""Offline token-prefix accounting and CLI checks without tokenizer downloads."""

import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


SOURCE = Path(__file__).resolve().parents[2] / "scripts/bench/book-harness/tokens"


def message(text, role="user"):
    return {"role": role, "content": text}


def receipt(messages, tools=None, size=1):
    return {"body": {"messages": messages, "tools": [] if tools is None else tools}, "bytes": size}


class TokenAccountingTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="angel-token-prefix-", dir="/tmp")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for name in ("accounting.mjs", "analyze.mjs", "curve.mjs"):
            shutil.copy2(SOURCE / name, self.root / name)
        # These packages replace only the external tokenizer boundary. The
        # production counting/prefix/CLI code is used unchanged.
        for name in ("deepseek_v4", "gpt4o", "qwen3", "llama3_1"):
            package = self.root / "node_modules/@lenml" / ("tokenizer-" + name)
            package.mkdir(parents=True)
            (package / "package.json").write_text(json.dumps({"type": "module", "main": "index.js"}))
            (package / "index.js").write_text(
                "export function fromPreTrained() { return {encode: text => Array.from(text)}; }\n")

    def node(self, args, check=True):
        result = subprocess.run(["node", *args], cwd=self.root, text=True, capture_output=True, timeout=5)
        if check:
            self.assertEqual(result.returncode, 0, result.stderr)
        return result

    def profile(self, rows):
        code = "import {prepareRows,profile} from './accounting.mjs';\n" \
               + "console.log(JSON.stringify(profile(prepareRows(" + json.dumps(rows) \
               + "), text => Array.from(text).length)));"
        return json.loads(self.node(["--input-type=module", "-e", code]).stdout)

    def log(self, rows, name="capture.jsonl"):
        path = self.root / name
        path.write_text("".join(json.dumps(row) + "\n" for row in rows))
        return path

    def test_append_only_keeps_original_total_fresh_and_first_projection(self):
        rows = [receipt([message("a", "system")]),
                receipt([message("a", "system"), message("bb")])]
        result = self.profile(rows)
        self.assertEqual((result["total"], result["fresh"]), (8, 5))
        self.assertEqual(result["first"], {"input": 3, "tools": 2, "system": 1, "messages": 1})
        self.assertEqual(result["points"], [{"request": 1, "total": 3, "fresh": 3, "ctx": 3},
                                            {"request": 2, "total": 8, "fresh": 5, "ctx": 5}])
        self.assertEqual(result["coverage"], {"requests": 2, "prefixRewrites": 0,
                                               "shortenedContexts": 0, "toolSchemaChanges": 0})

    def test_same_index_rewrite_recounts_current_text_and_estimated_fresh(self):
        result = self.profile([receipt([message("a")]), receipt([message("bbbb")])])
        self.assertEqual((result["total"], result["fresh"], result["points"][-1]["ctx"]), (9, 7, 6))
        self.assertEqual(result["coverage"]["prefixRewrites"], 1)

    def test_middle_rewrite_invalidates_suffix_even_when_later_text_is_unchanged(self):
        result = self.profile([receipt([message("a"), message("b"), message("d")]),
                               receipt([message("a"), message("ccc"), message("d")])])
        self.assertEqual((result["total"], result["fresh"]), (12, 9))

    def test_shrink_and_reused_positions_do_not_reuse_stale_counts(self):
        result = self.profile([receipt([message("a"), message("bb")]), receipt([message("a")]),
                               receipt([message("a"), message("ccc")])])
        self.assertEqual((result["total"], result["fresh"]), (14, 8))
        self.assertEqual(result["coverage"]["shortenedContexts"], 1)
        self.assertEqual(result["points"][1]["fresh"], 5)

    def test_changed_tool_schemas_invalidate_the_represented_request(self):
        rows = [receipt([message("a")], [{"name": "x"}]),
                receipt([message("a"), message("bb")], [{"name": "longer"}])]
        result = self.profile(rows)
        self.assertEqual(result["fresh"], result["total"])
        self.assertEqual(result["coverage"]["toolSchemaChanges"], 1)

    def test_role_or_serialization_changes_do_not_count_as_an_unchanged_prefix(self):
        for changed in (message("a", "system"), {"content": "a", "role": "user"}):
            with self.subTest(changed=changed):
                result = self.profile([receipt([message("a")]), receipt([changed])])
                self.assertEqual((result["total"], result["fresh"]), (6, 4))
                self.assertEqual(result["coverage"]["prefixRewrites"], 1)

    def test_function_arguments_and_content_parts_keep_original_text_projection(self):
        code = "import {contentText} from './accounting.mjs'; console.log(JSON.stringify(contentText(" \
               + json.dumps({"content": [{"text": "a"}, {"text": "b"}], "tool_calls": [
                   {"function": {"name": "shell", "arguments": '{"x":1}'}}]}) + ")));"
        self.assertEqual(json.loads(self.node(["--input-type=module", "-e", code]).stdout), 'ab\nshell {"x":1}')
        first = {"role": "assistant", "content": None, "tool_calls": [{"function": {"name": "shell", "arguments": "x"}}]}
        second = {"role": "assistant", "content": None, "tool_calls": [{"function": {"name": "shell", "arguments": "xxxx"}}]}
        result = self.profile([receipt([first]), receipt([second])])
        self.assertEqual(result["fresh"], result["total"] - 2)

    def test_import_has_no_cli_execution_or_argv_requirement(self):
        code = "await import('./analyze.mjs'); await import('./curve.mjs'); console.log('imports complete');"
        for prefix in ("", "process.argv[1] = 'missing unrelated caller'; "):
            result = self.node(["--input-type=module", "-e", prefix + code])
            self.assertEqual(result.stdout, "imports complete\n")
            self.assertEqual(result.stderr, "")

    def test_both_clis_use_corrected_counts_and_expose_estimate_scope(self):
        path = self.log([receipt([message("a")])] * 6 + [receipt([message("bbbb")])])
        result = json.loads(self.node(["analyze.mjs", str(path)]).stdout)[0]
        for cell in result["tok"].values():
            self.assertEqual((cell["total"], cell["fresh"]), (24, 7))
        self.assertIn("Not observed provider cache usage", result["scope"]["fresh"])
        curve = self.node(["curve.mjs", "deepseek_v4", str(path)]).stdout
        self.assertIn("7: total 24 fresh 7 ctx 6", curve)
        self.assertIn("fresh=perfect-prefix-estimate", curve)
        self.assertIn("prefix_rewrites=1", curve)

    def test_empty_or_invalid_later_input_refuses_before_any_stdout(self):
        good = self.log([receipt([message("a")])])
        bad = self.root / "empty.jsonl"
        for data in ("", " \n", "not JSON\n", json.dumps({"body": {"input": []}}),
                     json.dumps(receipt([message({"invalid": True})]))):
            bad.write_text(data)
            for args in (["analyze.mjs", str(good), str(bad)],
                         ["curve.mjs", "deepseek_v4", str(good), str(bad)]):
                with self.subTest(data=data, cli=args[0]):
                    result = self.node(args, check=False)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertEqual(result.stdout, "")
                    self.assertIn("Token input rejected", result.stderr)

    def test_unknown_tokenizer_and_missing_arguments_refuse_cleanly(self):
        for args in (["curve.mjs", "unknown", "missing.jsonl"], ["curve.mjs"], ["analyze.mjs"]):
            result = self.node(args, check=False)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "")
            self.assertIn("Token input rejected", result.stderr)

    def test_unknown_bytes_remain_unknown_independently_of_text_counts(self):
        path = self.log([receipt([message("a")], size=0), receipt([message("a"), message("b")], size=None)])
        result = json.loads(self.node(["analyze.mjs", str(path)]).stdout)[0]
        self.assertEqual(result["bytesFirst"], 0)
        self.assertIsNone(result["bytesTotal"])
        self.assertEqual(result["bytesReportedSubtotal"], 0)
        self.assertEqual(result["bytesCoverage"], {"reported": 1, "requests": 2})
        self.assertEqual(result["tok"]["deepseek_v4"]["total"], 7)
        path = self.log([receipt([message("a")], size=None), receipt([message("a"), message("b")], size=None)])
        result = json.loads(self.node(["analyze.mjs", str(path)]).stdout)[0]
        self.assertIsNone(result["bytesFirst"])
        self.assertIsNone(result["bytesTotal"])
        self.assertIsNone(result["bytesReportedSubtotal"])
        self.assertEqual(result["bytesCoverage"], {"reported": 0, "requests": 2})
        self.assertEqual(result["tok"]["deepseek_v4"]["total"], 7)


if __name__ == "__main__":
    unittest.main()
