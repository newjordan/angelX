import importlib.util
from pathlib import Path
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("room_worker", Path(__file__).with_name("worker.py"))
worker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(worker)


class WorkerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.table = worker.Table(self.temp.name)

    def configure(self):
        for team, provider in (("v", "codex"), ("grok", "grok"), ("muse", "muse")):
            self.table.configure_worker(team, provider)

    def test_worker_has_distinct_identity_and_takes_over_pending_queue(self):
        self.table.join("grok", "Silverhand")
        self.table.send("A saved question", target="grok", passes=1)
        self.table.configure_worker("grok", "grok")
        self.assertIsNone(self.table.task("grok-lead"))
        task = self.table.task("grok:worker")
        self.assertEqual(task["question"], "A saved question")
        people = {a["id"]: a for a in self.table.state()["agents"]}
        self.assertEqual(people["grok-lead"]["name"], "Silverhand")
        self.assertFalse(people["grok-lead"]["leader"])
        self.assertTrue(people["grok:worker"]["leader"])
        self.assertIn("local worker", people["grok:worker"]["name"])
        self.assertEqual(self.table.join("grok", "Silverhand")["role"], "member")
        self.assertEqual(next(a["id"] for a in self.table.state()["agents"] if a["team"] == "grok" and a["leader"]), "grok:worker")

    def test_one_coordinator_can_ping_two_peers_without_ping_pong(self):
        self.configure()
        self.table.send("Discuss a decision", target="v", passes=1)
        root = self.table.task("v:worker")
        self.table.reply("v:worker", root["id"], "Here is the initial view.", [
            {"team": "grok", "question": "Find a counterargument"},
            {"team": "muse", "question": "Check the communication"}])
        grok = self.table.task("grok:worker")
        muse = self.table.task("muse:worker")
        self.assertTrue(grok["peer_turn"])
        self.assertEqual(muse["peer_request"], "Check the communication")
        self.table.reply("grok:worker", grok["id"], "A counterargument", [{"team": "v", "question": "Ping back"}])
        self.assertIsNone(self.table.task("v:worker"))
        self.table.reply("muse:worker", muse["id"], "A communication check")
        self.assertEqual(self.table.state()["rounds"][0]["state"], "complete")
        self.assertEqual(self.table.state()["pending"], 0)
        everyone = self.table.send("@all another question", target="v", passes=1)
        self.assertEqual(set(everyone["targets"]), {"v:worker", "grok:worker", "muse:worker"})

    def test_native_process_result_is_delivered_and_failure_does_not_retry(self):
        self.configure()
        self.table.send("A test question", target="v", passes=1)
        task = self.table.task("v:worker")
        route = {"agent": "v:worker", "provider": "codex"}
        script = 'import sys,json;sys.stdin.read();print(json.dumps({"result":json.dumps({"reply":"Real process result", "pings":[]})}))'
        with patch.object(worker, "command_for", return_value=[sys.executable, "-c", script]):
            worker.run_task(self.table, route, task)
        self.assertEqual(self.table.state()["messages"][-1]["text"], "Real process result")
        self.table.send("A failure case", target="v", passes=1)
        task = self.table.task("v:worker")
        with patch.object(worker, "command_for", return_value=[sys.executable, "-c", "import sys;sys.stdin.read();sys.exit(3)"]):
            worker.run_task(self.table, route, task)
        self.assertIsNone(self.table.task("v:worker"))
        self.assertEqual(self.table.state()["rounds"][0]["state"], "attention")

    def test_quota_limited_worker_is_excluded_from_available_broadcasts(self):
        self.configure()
        self.table.send("For Muse", target="muse", passes=1)
        task = self.table.task("muse:worker")
        self.table.worker_failed("muse:worker", task["id"], "Quota exhausted", time.time() + 3600)
        request = self.table.send("@all a new question", target="v", passes=1)
        self.assertEqual(set(request["targets"]), {"v:worker", "grok:worker"})
        direct = self.table.send("@muse when your quota returns", target="v", passes=1)
        self.assertEqual(direct["targets"], ["muse:worker"])

    def test_idle_loop_calls_no_model_and_processes_a_queued_message_once(self):
        self.configure()
        argv = ["worker.py", "--state-dir", self.temp.name, "--team", "v"]
        with patch.object(sys, "argv", argv), patch.object(worker.time, "sleep", side_effect=StopIteration), patch.object(worker, "run_task") as run:
            with self.assertRaises(StopIteration):
                worker.main()
            run.assert_not_called()
        self.table.send("A queued question", target="v", passes=1)
        def deliver(table, route, task):
            table.reply(route["agent"], task["id"], "A single reply")
        with patch.object(sys, "argv", argv), patch.object(worker.time, "sleep", side_effect=StopIteration), patch.object(worker, "run_task", side_effect=deliver) as run:
            with self.assertRaises(StopIteration):
                worker.main()
            self.assertEqual(run.call_count, 1)
        self.assertEqual(self.table.state()["pending"], 0)


if __name__ == "__main__":
    unittest.main()
