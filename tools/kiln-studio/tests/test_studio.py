"""kiln-studio's own tests: the eyes, the vfx realm guard, the MCP protocol,
the library and the review votes. No model is called.

    python3 -m unittest discover -s tools/kiln-studio/tests
"""
import json
import os
import subprocess
import sys
import tempfile
import threading
import unittest
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(HERE))
_HOME = tempfile.TemporaryDirectory()
os.environ["KILN_HOME"] = _HOME.name  # jobs, the library and lessons go here, not to ~/.angelX/kiln

from studio import realm  # noqa: E402
from studio.backends import BackendError  # noqa: E402
from studio.backends.vfx import realmed  # noqa: E402


class Eyes(unittest.TestCase):
    def test_realm_has_74_inks(self):
        pal = realm.realm()
        self.assertEqual(len(pal.keys), 74)
        self.assertEqual(pal.hex("k"), "0b0a08")

    def test_reduce_keeps_the_outline_on_the_edge(self):
        g = realm.Grid(["....", ".kkk", ".kUU", ".kUU"])
        self.assertEqual(realm.reduce(g, 4).rows, ["k"])
        solid = realm.Grid(["UUUU"] * 4)
        self.assertEqual(realm.reduce(solid, 2).rows, ["UU", "UU"])

    def test_lint_finds_off_realm_and_lone_pixels(self):
        g = realm.Grid(["........", ".kkkkkk.", ".kUUUUk.", ".kUVUUk.", ".kUUUWk.", ".kkkkkk."])
        g.off = realm.Counter({"ff00ff": 3})
        rules = {f["rule"] for f in realm.lint(g, grain=1, rules={"lone_pixels_max": 1})}
        self.assertIn("palette", rules)
        self.assertIn("lone", rules)

    def test_pieces(self):
        self.assertEqual(realm.pieces(realm.Grid(["k.k", "...", "kk."])), 3)


class VfxGuard(unittest.TestCase):
    def test_ink_ramp_resolves_to_realm_hex(self):
        r = realmed({"layers": [{"type": "ring", "ramp": "ink:0123w"}]})
        slot = r["layers"][0]["ramp"][1:]
        self.assertEqual(r["ramps"][slot], ["12414f", "2079a6", "50c6b3", "8fe8dc", "e4fff8"])

    def test_cartridge_colours_are_refused(self):
        for bad in ({"layers": [{"type": "ring", "ramp": "fire"}]}, {"hue": 30, "layers": []},
                    {"ramps": {"x": ["ff0000", "00ff00"]}, "layers": [{"type": "ring", "ramp": "$x"}]}):
            with self.assertRaises(BackendError):
                realmed(bad)


class Mcp(unittest.TestCase):
    def test_initialize_list_and_a_refused_path(self):
        with tempfile.TemporaryDirectory() as d:
            msgs = [{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}},
                    {"jsonrpc": "2.0", "method": "notifications/initialized"},
                    {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
                    {"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "check", "arguments": {"source": "/etc/passwd.px.js"}}},
                    {"jsonrpc": "2.0", "id": 4, "method": "nope"}]
            p = subprocess.run([sys.executable, str(HERE / "kiln-studio"), "mcp", "--root", d, "--tools", "shot,check"],
                               input="".join(json.dumps(m) + "\n" for m in msgs), capture_output=True, text=True, timeout=60)
            out = [json.loads(l) for l in p.stdout.splitlines()]
            self.assertEqual([m["id"] for m in out], [1, 2, 3, 4])
            self.assertEqual(out[0]["result"]["protocolVersion"], "2025-06-18")
            self.assertEqual([t["name"] for t in out[1]["result"]["tools"]], ["shot", "check"])
            self.assertTrue(out[2]["result"]["isError"])
            self.assertIn("outside the workspace", out[2]["result"]["content"][0]["text"])
            self.assertEqual(out[3]["error"]["code"], -32601)


class Review(unittest.TestCase):
    def test_a_keep_goes_to_the_library_and_a_note_to_the_lessons(self):
        from studio import jobs, paths, review
        job = jobs.new("a test skull", variants=1, agents=["claude"], budget="1m", critics=["claude"])
        (job.dir / "director.json").write_text(json.dumps({"name": "test-skull", "source": "test-skull.px.js", "kind": "figure"}))
        d = job.variant_dir(1)
        (d / "shots").mkdir(parents=True)
        (d / "test-skull.px.js").write_text('use("realm.hex")\n')
        realm.to_image(realm.Grid(["kk", "kU"]), 4).save(d / "shots" / "test-skull.png")
        (d / "result.json").write_text(json.dumps({"agent": "claude", "state": "done", "source": "test-skull.px.js",
                                                   "shot": "variants/1/shots/test-skull.png"}))
        out = review.vote(job, {"variant": 1, "vote": "keep", "note": "the jaw reads", "voter": "tester"})
        kept = Path(out["library"])
        self.assertTrue((kept / "test-skull.px.js").exists())
        self.assertTrue((kept / "shot.png").exists())
        self.assertEqual(json.loads((kept / "notes.json").read_text())["votes"][0]["note"], "the jaw reads")
        self.assertIn("the jaw reads", paths.LESSONS.read_text())
        with self.assertRaises(ValueError):
            review.vote(job, {"variant": 2, "vote": "keep"})
        with self.assertRaises(ValueError):
            review.vote(job, {"variant": 1, "vote": "maybe"})
        self.assertEqual(len(review.job_detail(job)["votes"]), 1)


class Lighter(unittest.TestCase):
    def test_the_maker_serves_pixel_art_lighter(self):
        from http.server import ThreadingHTTPServer
        from studio import review

        page = review.lighter_page()
        self.assertIn(b"MIT License", page)
        self.assertIn(b"Pixel Art Lighter v 3.5.22", page)
        self.assertIn(b"Red Hat Sprite", page)
        self.assertNotIn(b"cloudflareinsights", page)
        # Up is positive Y, written in the green channel. That is the game's green-up map.
        self.assertIn(
            b'(direction.includes("up") ? s : direction.includes("down") ? -s : 0)',
            page,
        )
        self.assertIn('href="/lighter"', review.PAGE)

        httpd = ThreadingHTTPServer(("127.0.0.1", 0), review.LighterHandler)
        thread = threading.Thread(target=httpd.serve_forever, daemon=True)
        thread.start()
        try:
            port = httpd.server_address[1]
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/", timeout=5) as res:
                body = res.read()
                self.assertEqual(res.status, 200)
                self.assertIn("text/html", res.headers.get("Content-Type", ""))
            self.assertIn(b"Pixel Art Lighter v 3.5.22", body)
            review_httpd = ThreadingHTTPServer(("127.0.0.1", 0), review.Handler)
            review_thread = threading.Thread(target=review_httpd.serve_forever, daemon=True)
            review_thread.start()
            try:
                rport = review_httpd.server_address[1]
                with urllib.request.urlopen(f"http://127.0.0.1:{rport}/lighter", timeout=5) as res:
                    self.assertEqual(res.status, 200)
                    self.assertIn(b"Pixel Art Lighter", res.read())
            finally:
                review_httpd.shutdown()
                review_httpd.server_close()
        finally:
            httpd.shutdown()
            httpd.server_close()


if __name__ == "__main__":
    unittest.main()
