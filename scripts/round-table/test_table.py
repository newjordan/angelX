import concurrent.futures
import importlib.util
import json
import re
from pathlib import Path
import tempfile
import threading
import unittest
from http.server import ThreadingHTTPServer
from urllib.error import HTTPError
from urllib.request import Request, urlopen
from urllib.parse import urlencode

SPEC = importlib.util.spec_from_file_location("round_table", Path(__file__).with_name("table.py"))
module = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(module)


class RoundTableTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.table = module.Table(self.temp.name)

    def seat_leaders(self):
        self.table.join("dots", "Real Dot")
        self.table.join("grok", "Real Grok")

    def test_offline_queue_restart_cross_group_discussion_and_idempotency(self):
        self.table.send("Help decide the next step", passes=2)
        self.assertTrue(all(a["status"] == "awaiting connection" for a in self.table.state()["agents"]))
        self.table = module.Table(self.temp.name)
        self.seat_leaders()
        dot = self.table.wait("dots-lead", 0)["task"]
        grok = self.table.wait("grok-lead", 0)["task"]
        self.table.reply("dots-lead", dot["id"], "Test the assumption first.")
        self.assertIsNone(self.table.task("dots-lead"), "second pass waits for both groups")
        self.table.reply("grok-lead", grok["id"], "Which assumption is most important?")
        self.assertTrue(self.table.reply("grok-lead", grok["id"], "retry")["duplicate"])
        dot2 = self.table.task("dots-lead")
        grok2 = self.table.task("grok-lead")
        self.assertEqual(dot2["phase"], 2)
        self.assertIn("Which assumption", dot2["transcript"][-1]["text"])
        self.assertTrue(any("Test the assumption" in m["text"] for m in grok2["transcript"]))
        self.table.reply("dots-lead", dot2["id"], "The delivery assumption.")
        self.table.reply("grok-lead", grok2["id"], "Agreed: test delivery.")
        state = self.table.state()
        self.assertEqual(state["rounds"][0]["state"], "complete")
        self.assertEqual(len(state["messages"]), 5)
        self.assertEqual(state["pending"], 0)

    def test_parallel_replies_create_exactly_one_next_phase(self):
        self.seat_leaders()
        self.table.send("Discuss", passes=3)
        for phase in range(1, 4):
            tasks = [self.table.task(agent) for agent in ("dots-lead", "grok-lead")]
            self.assertTrue(all(t["phase"] == phase for t in tasks))
            with concurrent.futures.ThreadPoolExecutor() as pool:
                futures = [pool.submit(self.table.reply, agent, task["id"], "A real response")
                           for agent, task in zip(("dots-lead", "grok-lead"), tasks)]
                for f in futures:
                    f.result()
        self.assertEqual(len(self.table.state()["messages"]), 7)
        self.assertEqual(self.table.state()["rounds"][0]["state"], "complete")

    def test_stop_rejects_late_reply_and_new_topic_can_start(self):
        self.seat_leaders()
        self.table.send("Old topic")
        task = self.table.task("dots-lead")
        followup = self.table.send("A follow-up can be queued before the first reply")
        self.assertGreater(followup["round"], 1)
        self.table.stop()
        with self.assertRaisesRegex(ValueError, "stopped"):
            self.table.reply("dots-lead", task["id"], "Too late")
        self.table.send("New topic", target="grok", passes=1)
        self.assertIsNone(self.table.task("dots-lead"))
        self.assertEqual(self.table.task("grok-lead")["question"], "New topic")

    def test_members_do_not_take_leader_seat_and_tasks_stay_with_owner(self):
        self.seat_leaders()
        self.table.join("dots", "Researcher", member="research")
        self.table.send("Discuss", passes=1)
        self.assertIsNone(self.table.task("dots:research"))
        task = self.table.task("dots-lead")
        with self.assertRaises(ValueError):
            self.table.reply("grok-lead", task["id"], "Wrong identity")
        self.table.say("dots:research", "Member contribution")
        self.table.leave("dots-lead")
        self.assertEqual(next(a for a in self.table.state()["agents"] if a["id"] == "dots-lead")["status"],
                         "awaiting connection")
        self.table.join("dots", "Real Dot")
        self.assertEqual(self.table.task("dots-lead")["id"], task["id"])

    def test_http_room_requires_local_host_and_csrf_and_retains_literal_text(self):
        server = ThreadingHTTPServer(("127.0.0.1", 0), module.handler(self.table, 0))
        port = server.server_port
        server.RequestHandlerClass = module.handler(self.table, port)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        base = f"http://127.0.0.1:{port}"
        html = urlopen(base).read().decode()
        token = html.split("const csrf='")[1].split("'")[0]
        body = json.dumps({"text": "<script>alert('text only')</script>", "passes": 1}).encode()
        with self.assertRaises(HTTPError) as caught:
            urlopen(Request(base + "/api/send", data=body, headers={"Content-Type": "application/json"}))
        self.assertEqual(caught.exception.code, 403)
        caught.exception.close()
        request = Request(base + "/api/send", data=body,
                          headers={"Content-Type": "application/json", "X-Table-CSRF": token,
                                   "Authorization": "Bearer " + self.table.key("host")})
        self.assertEqual(urlopen(request).status, 200)
        state = json.load(urlopen(Request(base + "/api/state", headers={
            "Authorization": "Bearer " + self.table.key("host")})))
        self.assertEqual(state["messages"][0]["text"], "<script>alert('text only')</script>")
        with self.assertRaises(HTTPError) as wrong_host:
            urlopen(Request(base + "/api/state", headers={"Host": "remote.example"}))
        wrong_host.exception.close()

    def test_browser_invites_are_scoped_and_all_seats_share_the_room(self):
        self.table.set_public_url("https://room.example")
        server = ThreadingHTTPServer(("127.0.0.1", 0), module.handler(self.table, 0))
        server.RequestHandlerClass = module.handler(self.table, server.server_port)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        base = f"http://127.0.0.1:{server.server_port}"
        html = urlopen(base).read().decode()
        csrf = html.split("const csrf='")[1].split("'")[0]
        self.assertNotIn(self.table.key("host"), html)

        def call(path, role=None, body=None):
            headers = {"Host": "room.example", "X-Table-CSRF": csrf,
                       "Origin": "https://room.example", "Content-Type": "application/json"}
            if role:
                headers["Authorization"] = "Bearer " + self.table.key(role)
            req = Request(base + path, headers=headers,
                          data=None if body is None else json.dumps(body).encode())
            try:
                with urlopen(req) as response:
                    return response.status, json.load(response)
            except HTTPError as exc:
                status = exc.code
                exc.close()
                return status, None

        self.assertEqual(call("/api/state")[0], 401)
        self.assertEqual(call("/api/invite/dots", "grok")[0], 403)
        self.assertEqual(call("/api/stop", "grok", {})[0], 403)
        for role in module.TEAMS:
            self.assertEqual(call("/api/join", role, {"name": role.title()})[0], 200)
        self.assertEqual(call("/api/send", "grok", {"text": "Impersonation"})[0], 403)
        self.assertEqual(call("/api/send", "host", {"text": "Everyone, weigh in", "passes": 1})[0], 200)
        tasks = {role: call("/api/task", role)[1]["task"] for role in module.TEAMS}
        self.assertEqual(call("/api/reply", "grok", {"task": tasks["dots"]["id"], "text": "wrong seat"})[0], 400)
        for role in module.TEAMS:
            status, _ = call("/api/reply", role, {"task": tasks[role]["id"], "text": role + " replies"})
            self.assertEqual(status, 200)
        state = call("/api/state", "v")[1]
        self.assertEqual(state["rounds"][0]["state"], "complete")
        self.assertEqual({m["author"] for m in state["messages"]}, {"human", *(team + "-lead" for team in module.TEAMS)})
        self.assertEqual(module.Table(self.temp.name).key("grok"), self.table.key("grok"))
        self.assertEqual(module.Table(self.temp.name).key("muse"), self.table.key("muse"))
        self.assertEqual(module.Table(self.temp.name).csrf(), csrf)

    def test_invitation_forms_work_without_javascript_and_do_not_auto_join(self):
        server = ThreadingHTTPServer(("127.0.0.1", 0), module.handler(self.table, 0))
        server.RequestHandlerClass = module.handler(self.table, server.server_port)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        base = f"http://127.0.0.1:{server.server_port}"
        invite = self.table.links(base)["muse"]
        with urlopen(invite) as response:
            page = response.read().decode()
        self.assertIn("This view works without JavaScript", page)
        person = next(a for a in self.table.state()["agents"] if a["team"] == "muse")
        self.assertFalse(person["joined"])
        self.assertGreater(person["opened"], 0)
        csrf = re.search('name="csrf" value="([^"]+)"', page).group(1)

        def submit(fields):
            body = urlencode({"csrf": csrf, **fields}).encode()
            with urlopen(Request(invite, data=body)) as response:
                return response.read().decode()

        submit({"operation": "join", "name": "Test Muse"})
        self.assertTrue(next(a for a in self.table.state()["agents"] if a["team"] == "muse")["joined"])
        self.table.send("A question for Muse", target="muse", passes=1)
        with urlopen(invite) as response:
            page = response.read().decode()
        self.assertIn("A question for Muse", page)
        task = re.search(r'name="task" value="(\d+)"', page).group(1)
        submit({"operation": "chat", "task": task, "text": "<b>Literal answer</b>"})
        with urlopen(invite) as response:
            self.assertIn("&lt;b&gt;Literal answer&lt;/b&gt;", response.read().decode())
        self.assertEqual(self.table.state()["rounds"][0]["state"], "complete")


if __name__ == "__main__":
    unittest.main()
