#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import shlex
import sqlite3
import subprocess
import sys
import tempfile
import time
import unittest


SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "angel-machine-queue.py"
SPEC = importlib.util.spec_from_file_location("angel_machine_queue", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
QUEUE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(QUEUE)


class MachineQueueTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.db = str(Path(self.temp.name) / "queue.sqlite3")
        self.conn = QUEUE.connect_database(self.db)

    def tearDown(self) -> None:
        self.conn.close()
        self.temp.cleanup()

    def enqueue(self, owner: str, competition: str, now: float) -> str:
        return QUEUE.enqueue_request(
            self.conn, "mac-test", owner, competition, f"test {competition}", now=now
        )

    def release(self, lease: dict, now: float) -> None:
        self.assertTrue(
            QUEUE.finish_request(
                self.conn,
                lease["id"],
                lease["lease_token"],
                "done",
                0,
                "ok",
                now=now,
            )
        )

    def test_owners_alternate_even_when_one_owner_has_a_backlog(self) -> None:
        a1 = self.enqueue("angel-a", "competition-a-1", 100)
        a2 = self.enqueue("angel-a", "competition-a-2", 101)
        b1 = self.enqueue("angel-b", "competition-b", 102)

        lease_a1 = QUEUE.try_acquire(self.conn, a1, now=103)
        self.assertEqual(lease_a1["status"], "active")
        self.release(lease_a1, 104)

        waiting_a2 = QUEUE.try_acquire(self.conn, a2, now=105)
        self.assertEqual(waiting_a2["status"], "queued")
        self.assertEqual(waiting_a2["position"], 2)
        lease_b1 = QUEUE.try_acquire(self.conn, b1, now=105)
        self.assertEqual(lease_b1["status"], "active")
        self.release(lease_b1, 106)

        lease_a2 = QUEUE.try_acquire(self.conn, a2, now=107)
        self.assertEqual(lease_a2["status"], "active")

    def test_only_one_active_lease_and_stale_lease_recovers(self) -> None:
        a1 = self.enqueue("angel-a", "competition-a", 100)
        b1 = self.enqueue("angel-b", "competition-b", 101)
        lease_a1 = QUEUE.try_acquire(self.conn, a1, lease_seconds=15, now=102)
        self.assertEqual(lease_a1["status"], "active")

        waiting_b1 = QUEUE.try_acquire(self.conn, b1, lease_seconds=15, now=103)
        self.assertEqual(waiting_b1["status"], "queued")
        lease_b1 = QUEUE.try_acquire(self.conn, b1, lease_seconds=15, now=118)
        self.assertEqual(lease_b1["status"], "active")

        expired = self.conn.execute(
            "SELECT status FROM requests WHERE id = ?", (a1,)
        ).fetchone()
        self.assertEqual(expired["status"], "expired")

    def test_raw_command_is_not_persisted(self) -> None:
        secret_command = "run-sensitive-candidate --never-store-this"
        QUEUE.enqueue_request(
            self.conn,
            "mac-test",
            "angel-a",
            "competition-a",
            secret_command,
            now=100,
        )
        rows = self.conn.execute("SELECT * FROM requests").fetchall()
        rendered = repr([dict(row) for row in rows])
        self.assertNotIn(secret_command, rendered)
        self.assertIn("command_sha256", rendered)

    def test_terminal_receipts_are_bounded(self) -> None:
        original = QUEUE.DEFAULT_RETAINED_RECEIPTS
        try:
            QUEUE.DEFAULT_RETAINED_RECEIPTS = 3
            for index in range(5):
                request = self.enqueue(f"angel-{index}", f"competition-{index}", index + 1)
                lease = QUEUE.try_acquire(self.conn, request, now=index + 10)
                self.assertEqual(lease["status"], "active")
                # Exercise the actual pruning function with an explicit small cap;
                # the production default remains 2,048 receipts per resource.
                QUEUE.finish_request(
                    self.conn,
                    request,
                    lease["lease_token"],
                    "done",
                    0,
                    "ok",
                    now=index + 20,
                )
                self.conn.execute("BEGIN IMMEDIATE")
                QUEUE.prune_receipts_locked(self.conn, "mac-test", retained=3)
                self.conn.execute("COMMIT")
            count = self.conn.execute(
                "SELECT COUNT(*) AS n FROM requests WHERE status = 'done'"
            ).fetchone()["n"]
            self.assertEqual(count, 3)
        finally:
            QUEUE.DEFAULT_RETAINED_RECEIPTS = original

    def test_independent_processes_never_overlap_the_machine_lease(self) -> None:
        marker = Path(self.temp.name) / "active.marker"
        quoted_marker = shlex.quote(str(marker))
        command = (
            f"test ! -e {quoted_marker} && touch {quoted_marker} && sleep 1; "
            f"status=$?; rm -f {quoted_marker}; exit $status"
        )

        def launch(owner: str) -> subprocess.Popen[str]:
            payload = QUEUE.encode_payload(
                {
                    "db": self.db,
                    "resource": "mac-test",
                    "owner": owner,
                    "competition": owner,
                    "command": command,
                    "cwd": self.temp.name,
                    "lease_seconds": 15,
                    "max_run_seconds": 10,
                }
            )
            process = subprocess.Popen(
                [sys.executable, str(SCRIPT), "run-payload"],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )
            assert process.stdin is not None
            process.stdin.write(payload + "\n")
            process.stdin.close()
            return process

        first = launch("angel-a")
        assert first.stdout is not None
        first_prefix = []
        while True:
            line = first.stdout.readline()
            self.assertTrue(line, "first runner exited before acquiring")
            first_prefix.append(line)
            if json.loads(line)["event"] == "acquired":
                break

        second = launch("angel-b")
        assert second.stdout is not None
        second_prefix = []
        while True:
            line = second.stdout.readline()
            self.assertTrue(line, "second runner exited before reporting queue state")
            second_prefix.append(line)
            if json.loads(line)["event"] == "waiting":
                break

        first_output = "".join(first_prefix) + first.stdout.read()
        first_stderr = first.stderr.read() if first.stderr is not None else ""
        self.assertEqual(first.wait(timeout=10), 0, first_output + first_stderr)
        second_output = "".join(second_prefix) + second.stdout.read()
        second_stderr = second.stderr.read() if second.stderr is not None else ""
        self.assertEqual(second.wait(timeout=10), 0, second_output + second_stderr)
        for process in (first, second):
            if process.stdout is not None:
                process.stdout.close()
            if process.stderr is not None:
                process.stderr.close()
        self.assertIn('"event": "released"', first_output)
        self.assertIn('"event": "released"', second_output)
        self.assertFalse(marker.exists())

    def test_runtime_quantum_kills_a_term_ignoring_process_group(self) -> None:
        payload = QUEUE.encode_payload(
            {
                "db": self.db,
                "resource": "mac-test",
                "owner": "angel-a",
                "competition": "competition-a",
                "command": "trap '' TERM; sleep 30",
                "cwd": self.temp.name,
                "lease_seconds": 15,
                "max_run_seconds": 1,
            }
        )
        started = time.monotonic()
        completed = subprocess.run(
            [sys.executable, str(SCRIPT), "run-payload"],
            input=payload + "\n",
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
        elapsed = time.monotonic() - started
        self.assertNotEqual(completed.returncode, 0)
        self.assertLess(elapsed, 9)
        self.assertIn('"status": "failed"', completed.stdout)
        receipt = self.conn.execute(
            "SELECT status, message FROM requests ORDER BY enqueued_at DESC LIMIT 1"
        ).fetchone()
        self.assertEqual(receipt["status"], "failed")
        self.assertEqual(receipt["message"], "maximum test runtime exceeded")


if __name__ == "__main__":
    unittest.main()
