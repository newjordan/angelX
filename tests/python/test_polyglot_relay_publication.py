"""A manual reader never observes a half-written relay request or reply."""

import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest


SOURCE = Path(__file__).resolve().parents[2] / "scripts/bench/book-harness"
WRITER = r"""
import importlib.util, json, os, pathlib, runpy, sys, time
root, mode = pathlib.Path(sys.argv[1]), sys.argv[2]
sys.path.insert(0, str(root))
ready, release = root / 'writing.ready', root / 'writing.release'
original = json.dump
def interrupted_dump(payload, target, *args, **kwargs):
    # json.dump streams its bytes. Hold a real writer after its first bytes,
    # letting the consumer run while the producer still owns the file.
    target.write('{"unfinished":')
    target.flush()
    ready.write_text('ready')
    deadline = time.monotonic() + 6
    while not release.exists():
        if time.monotonic() >= deadline:
            raise RuntimeError('fixture writer was never released')
        time.sleep(.01)
    target.seek(0)
    target.truncate()
    return original(payload, target, *args, **kwargs)
json.dump = interrupted_dump
if mode == 'request':
    os.environ.update(RELAY_DIR=str(root / 'run'), RELAY_PORT='0')
    spec = importlib.util.spec_from_file_location('fixture_relay', root / 'relay.py')
    relay = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(relay)
    with relay.ThreadingHTTPServer(('127.0.0.1', 0), relay.Handler) as server:
        (root / 'port').write_text(str(server.server_port))
        server.serve_forever()
else:
    sys.argv = [str(root / 'answer.py'), str(root / 'run'), str(root / 'reply.json')]
    runpy.run_path(sys.argv[0], run_name='__main__')
"""

CLIENT = r"""
import json, sys, urllib.request
request = urllib.request.Request(sys.argv[1] + '/chat/completions',
    data=json.dumps({'messages': [], 'stream': False}).encode(),
    headers={'Content-Type': 'application/json'})
with urllib.request.urlopen(request, timeout=5) as response:
    print(response.read().decode())
"""


class RelayPublicationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="angel-relay-publication-", dir="/tmp")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for name in ("relay.py", "pending.py", "answer.py", "relay_files.py"):
            shutil.copy2(SOURCE / name, self.root / name)
        self.run = self.root / "run"
        self.run.mkdir()
        self.jobs = []
        self.addCleanup(self.retire)

    def retire(self):
        # Release every fixture writer before stopping its directly owned job.
        (self.root / "writing.release").touch()
        for job in reversed(self.jobs):
            if job.poll() is None:
                job.terminate()
            try:
                job.communicate(timeout=2)
            except subprocess.TimeoutExpired:
                job.kill()
                job.communicate(timeout=2)

    def start(self, args):
        job = subprocess.Popen(args, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.jobs.append(job)
        return job

    def wait_for(self, name):
        path = self.root / name
        deadline = time.monotonic() + 4
        while not path.exists() and time.monotonic() < deadline:
            time.sleep(.01)
        self.assertTrue(path.exists(), name)
        return path

    def pending(self):
        return subprocess.run([sys.executable, str(self.root / "pending.py"), str(self.run)],
                              text=True, capture_output=True, timeout=2)

    def test_request_becomes_visible_only_after_its_complete_json(self):
        self.start([sys.executable, "-c", WRITER, str(self.root), "request"])
        port = self.wait_for("port").read_text()
        client = self.start([sys.executable, "-c", CLIENT, "http://127.0.0.1:" + port])
        self.wait_for("writing.ready")
        observed = self.pending()
        self.assertEqual(observed.returncode, 0, observed.stderr)
        self.assertEqual(observed.stdout, "WAIT\n")
        self.assertFalse((self.run / "1.req.json").exists())
        (self.root / "writing.release").touch()
        self.wait_for("run/1.req.json")
        request = json.loads((self.run / "1.req.json").read_text())
        self.assertEqual(request["body"], {"messages": [], "stream": False})
        (self.run / "1.resp.json").write_text(json.dumps({"content": "complete reply"}))
        stdout, stderr = client.communicate(timeout=3)
        self.assertEqual(client.returncode, 0, stderr)
        self.assertEqual(json.loads(stdout)["choices"][0]["message"]["content"], "complete reply")
        self.assertEqual(sorted(p.name for p in self.run.iterdir()), ["1.req.json", "1.resp.json"])

    def test_answer_becomes_visible_only_after_its_complete_json(self):
        (self.run / "1.req.json").write_text(json.dumps({"body": {"messages": [], "tools": []}}))
        (self.root / "reply.json").write_text(json.dumps({"content": "complete reply"}))
        writer = self.start([sys.executable, "-c", WRITER, str(self.root), "answer"])
        self.wait_for("writing.ready")
        self.assertFalse((self.run / "1.resp.json").exists())
        observed = self.pending()
        self.assertEqual(observed.returncode, 0, observed.stderr)
        self.assertTrue(observed.stdout.startswith("REQUEST 1\n"), observed.stdout)
        (self.root / "writing.release").touch()
        self.wait_for("run/1.resp.json")
        self.assertEqual(json.loads((self.run / "1.resp.json").read_text()), {"content": "complete reply"})
        (self.run / "rc").write_text("0\n")
        stdout, stderr = writer.communicate(timeout=3)
        self.assertEqual(writer.returncode, 0, stderr)
        self.assertIn("DONE 0", stdout)
        self.assertEqual(sorted(p.name for p in self.run.iterdir()), ["1.req.json", "1.resp.json", "rc"])


if __name__ == "__main__":
    unittest.main()
