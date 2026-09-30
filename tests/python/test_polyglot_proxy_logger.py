"""Offline failure receipts for the benchmark's loopback provider proxy."""

import importlib.util
import json
import os
from pathlib import Path
import socket
import struct
import tempfile
import threading
import time
import unittest
from unittest import mock
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


SCRIPT = Path(__file__).resolve().parents[2] / "scripts/bench/book-harness/proxy_log.py"


class ProxyTests(unittest.TestCase):
    def setUp(self):
        self.root = tempfile.TemporaryDirectory(prefix="angel-proxy-receipts-")
        self.addCleanup(self.root.cleanup)
        spec = importlib.util.spec_from_file_location("proxy_logger", SCRIPT)
        self.proxy = importlib.util.module_from_spec(spec)
        with mock.patch.dict(os.environ, {"PROXY_UPSTREAM": "http://127.0.0.1:9/v1",
                                         "PROXY_LOG": str(Path(self.root.name) / "calls.jsonl"),
                                         "PROXY_KEY": "", "PROXY_KEY_CMD": ""}):
            spec.loader.exec_module(self.proxy)
        self.server = self.serve(self.proxy.Handler)
        self.url = f"http://127.0.0.1:{self.server.server_port}/v1/chat/completions"

    def serve(self, handler):
        server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
        thread = threading.Thread(target=server.serve_forever, kwargs={"poll_interval": .01})
        thread.start()

        def cleanup():
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)
            self.assertFalse(thread.is_alive(), "fixture server did not stop")

        self.addCleanup(cleanup)
        return server

    def request(self):
        return urllib.request.Request(self.url, data=b'{"model":"fixture","messages":[]}',
                                      headers={"Content-Type": "application/json"})

    def receipts(self, count=1, suffix=""):
        path = Path(self.proxy.LOG + suffix)
        deadline = time.monotonic() + 2
        while time.monotonic() < deadline:
            try:
                records = [json.loads(line) for line in path.read_text().splitlines()]
                if len(records) >= count:
                    self.assertEqual(len(records), count, "one receipt per attempted request")
                    return records
            except (OSError, ValueError):
                pass
            time.sleep(.01)
        self.fail(f"proxy did not retain {count} receipt(s)")

    def test_refused_upstream_retains_post_and_probe_failures(self):
        # Holding a bound, non-listening port prevents another test from
        # acquiring it while still producing a real refused connection.
        refused = socket.socket()
        self.addCleanup(refused.close)
        refused.bind(("127.0.0.1", 0))
        self.proxy.UPSTREAM = f"http://127.0.0.1:{refused.getsockname()[1]}/v1"
        for request in (self.request(), self.url.replace("chat/completions", "models")):
            with self.assertRaises(urllib.error.HTTPError) as failure:
                urllib.request.urlopen(request, timeout=2)
            self.assertEqual(failure.exception.code, 502)
            failure.exception.close()
        post = self.receipts()[0]
        self.assertEqual(post["body"], {"model": "fixture", "messages": []})
        self.assertIsNone(post["status"])
        self.assertIsNone(post["usage"])
        self.assertEqual(post["proxy_error"], "URLError")
        self.assertGreaterEqual(post["done"], post["t"])
        probe = self.receipts(suffix=".get")[0]
        self.assertEqual(probe["path"], "/v1/models")
        self.assertIsNone(probe["status"])

    def test_failed_credential_command_retains_attempt_without_command_output(self):
        self.proxy.KEY_CMD = "python3 -c 'import sys; sys.stderr.write(\"fixture-private-sentinel\"); sys.exit(3)'"
        with self.assertRaises(urllib.error.HTTPError) as failure:
            urllib.request.urlopen(self.request(), timeout=2)
        self.assertEqual(failure.exception.code, 502)
        self.assertNotIn("fixture-private-sentinel", failure.exception.read().decode())
        failure.exception.close()
        receipt = self.receipts()[0]
        self.assertEqual(receipt["proxy_error"], "CalledProcessError")
        self.assertIsNone(receipt["usage"])
        self.assertNotIn("fixture-private-sentinel", Path(self.proxy.LOG).read_text())

    def test_upstream_http_error_is_forwarded_and_recorded_once(self):
        class Upstream(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_POST(self):
                self.rfile.read(int(self.headers["Content-Length"]))
                self.send_response(503)
                self.send_header("Content-Length", "2")
                self.end_headers()
                self.wfile.write(b"{}")

        upstream = self.serve(Upstream)
        self.proxy.UPSTREAM = f"http://127.0.0.1:{upstream.server_port}/v1"
        with self.assertRaises(urllib.error.HTTPError) as failure:
            urllib.request.urlopen(self.request(), timeout=2)
        self.assertEqual(failure.exception.code, 503)
        self.assertEqual(failure.exception.read(), b"{}")
        failure.exception.close()
        receipt = self.receipts()[0]
        self.assertEqual(receipt["status"], 503)
        self.assertIsNone(receipt["usage"])
        self.assertNotIn("proxy_error", receipt)

    def test_disconnected_client_retains_observed_upstream_status(self):
        arrived, release = threading.Event(), threading.Event()
        self.addCleanup(release.set)

        class Upstream(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_POST(self):
                self.rfile.read(int(self.headers["Content-Length"]))
                body = b'{"usage":{"prompt_tokens":100,"completion_tokens":10}}'
                self.send_response(200)
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                arrived.set()
                release.wait(2)
                self.wfile.write(body)

        upstream = self.serve(Upstream)
        self.proxy.UPSTREAM = f"http://127.0.0.1:{upstream.server_port}/v1"
        client = socket.create_connection(("127.0.0.1", self.server.server_port), timeout=2)
        self.addCleanup(client.close)
        raw = b'{"model":"fixture","messages":[]}'
        client.sendall(b"POST /v1/chat/completions HTTP/1.1\r\nHost: localhost\r\nContent-Length: "
                       + str(len(raw)).encode() + b"\r\n\r\n" + raw)
        self.assertTrue(arrived.wait(2), "proxy did not forward the request")
        client.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, struct.pack("ii", 1, 0))
        client.close()
        release.set()
        receipt = self.receipts()[0]
        self.assertEqual(receipt["status"], 200)
        self.assertIn(receipt["proxy_error"], ("BrokenPipeError", "ConnectionResetError"))
        self.assertIsNone(receipt["usage"], "interrupted responses cannot prove complete usage")

    def test_truncated_content_length_does_not_prove_complete_usage(self):
        body = b'{"usage":{"prompt_tokens":100,"completion_tokens":10}}'

        class Upstream(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_POST(self):
                self.rfile.read(int(self.headers.get("Content-Length", "0")))
                self.send_response(200)
                self.send_header("Content-Length", str(len(body) + 10))
                self.end_headers()
                self.wfile.write(body)
                self.close_connection = True

            do_GET = do_POST

        upstream = self.serve(Upstream)
        self.proxy.UPSTREAM = f"http://127.0.0.1:{upstream.server_port}/v1"
        for request, suffix in ((self.request(), ""),
                                (self.url.replace("chat/completions", "models"), ".get")):
            with self.subTest(suffix=suffix):
                # Headers/body may already have reached the client before the
                # bad EOF is discovered. The retained receipt must refuse it.
                with urllib.request.urlopen(request, timeout=2) as response:
                    self.assertEqual(response.read(), body)
                receipt = self.receipts(suffix=suffix)[0]
                self.assertEqual(receipt["status"], 200)
                self.assertEqual(receipt.get("proxy_error"), "IncompleteRead")
                if not suffix:
                    self.assertIsNone(receipt["usage"])

    def test_complete_content_length_keeps_exact_forwarded_body_and_usage(self):
        body = b'{"usage":{"prompt_tokens":100,"completion_tokens":0}}'

        class Upstream(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_POST(self):
                self.rfile.read(int(self.headers.get("Content-Length", "0")))
                self.send_response(200)
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

        upstream = self.serve(Upstream)
        self.proxy.UPSTREAM = f"http://127.0.0.1:{upstream.server_port}/v1"
        with urllib.request.urlopen(self.request(), timeout=2) as response:
            self.assertEqual(response.read(), body)
        receipt = self.receipts()[0]
        self.assertEqual(receipt["usage"], {"prompt_tokens": 100, "completion_tokens": 0})
        self.assertNotIn("proxy_error", receipt)

    def test_nonobject_events_do_not_discard_a_completed_response_receipt(self):
        self.assertIsNone(self.proxy.usage_of("[]"))
        self.assertIsNone(self.proxy.usage_of('"response"'))
        self.assertIsNone(self.proxy.usage_of('data: {"usage":42}\n'))
        stream = 'data: {"message":{"usage":{"input_tokens":10}}}\n' \
                 'data: {"usage":null}\n' \
                 'data: {"usage":{"output_tokens":0}}\n'
        self.assertEqual(self.proxy.usage_of(stream), {"input_tokens": 10, "output_tokens": 0})


if __name__ == "__main__":
    unittest.main()
