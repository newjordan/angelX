#!/usr/bin/env python3
"""Fair, crash-recoverable leases for a scarce remote test machine.

The queue database lives on the scarce machine.  Independent angel0 clients
invoke this program over SSH, so there is one transactional source of truth
without exposing another network service.  Commands are carried in a one-shot
stdin payload and are never persisted; the ledger retains only a SHA-256
digest and bounded scheduling metadata.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import shlex
import signal
import sqlite3
import subprocess
import sys
import threading
import time
from typing import Any


DEFAULT_DB = "~/.angel0/machine-queue.sqlite3"
DEFAULT_REMOTE_SCRIPT = ".local/bin/angel-machine-queue.py"
DEFAULT_RESOURCE = "mac-test"
DEFAULT_LEASE_SECONDS = 90
DEFAULT_QUEUE_TTL_SECONDS = 120
DEFAULT_RETAINED_RECEIPTS = 2_048
POLL_SECONDS = 1.0
IDENTIFIER = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.:@/+ -]{0,127}$")
TERMINAL_STATES = ("done", "failed", "cancelled", "expired")


def now_seconds() -> float:
    return time.time()


def checked_identifier(label: str, value: str) -> str:
    value = value.strip()
    if not IDENTIFIER.fullmatch(value):
        raise ValueError(f"{label} must be 1-128 ordinary identifier characters")
    return value


def bounded_text(value: str, limit: int = 400) -> str:
    return value.replace("\x00", "").strip()[:limit]


def connect_database(path: str) -> sqlite3.Connection:
    db_path = Path(path).expanduser().resolve()
    db_path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    try:
        db_path.parent.chmod(0o700)
    except OSError:
        pass
    conn = sqlite3.connect(str(db_path), timeout=30, isolation_level=None)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA journal_mode=WAL")
    conn.execute("PRAGMA synchronous=NORMAL")
    conn.execute("PRAGMA busy_timeout=30000")
    conn.executescript(
        """
        CREATE TABLE IF NOT EXISTS resources (
            name TEXT PRIMARY KEY,
            grant_seq INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS owners (
            resource TEXT NOT NULL,
            owner TEXT NOT NULL,
            last_grant_seq INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (resource, owner)
        );
        CREATE TABLE IF NOT EXISTS requests (
            id TEXT PRIMARY KEY,
            resource TEXT NOT NULL,
            owner TEXT NOT NULL,
            competition TEXT NOT NULL,
            command_sha256 TEXT NOT NULL,
            enqueued_at REAL NOT NULL,
            waiter_seen_at REAL NOT NULL,
            status TEXT NOT NULL,
            lease_token TEXT,
            lease_expires_at REAL,
            heartbeat_at REAL,
            started_at REAL,
            finished_at REAL,
            exit_code INTEGER,
            message TEXT
        );
        CREATE INDEX IF NOT EXISTS requests_resource_status
            ON requests(resource, status, enqueued_at, id);
        CREATE UNIQUE INDEX IF NOT EXISTS one_active_lease_per_resource
            ON requests(resource) WHERE status = 'active';
        """
    )
    try:
        db_path.chmod(0o600)
    except OSError:
        pass
    return conn


def begin_immediate(conn: sqlite3.Connection) -> None:
    conn.execute("BEGIN IMMEDIATE")


def commit(conn: sqlite3.Connection) -> None:
    conn.execute("COMMIT")


def rollback(conn: sqlite3.Connection) -> None:
    try:
        conn.execute("ROLLBACK")
    except sqlite3.OperationalError:
        pass


def expire_stale_locked(
    conn: sqlite3.Connection,
    resource: str,
    now: float,
    queue_ttl_seconds: int,
) -> None:
    conn.execute(
        """
        UPDATE requests
           SET status = 'expired', finished_at = ?, lease_token = NULL,
               lease_expires_at = NULL, message = 'active lease heartbeat expired'
         WHERE resource = ? AND status = 'active'
           AND lease_expires_at IS NOT NULL AND lease_expires_at <= ?
        """,
        (now, resource, now),
    )
    conn.execute(
        """
        UPDATE requests
           SET status = 'expired', finished_at = ?,
               message = 'queue waiter disappeared'
         WHERE resource = ? AND status = 'queued' AND waiter_seen_at <= ?
        """,
        (now, resource, now - max(10, queue_ttl_seconds)),
    )


def prune_receipts_locked(
    conn: sqlite3.Connection, resource: str, retained: int = DEFAULT_RETAINED_RECEIPTS
) -> None:
    conn.execute(
        """
        DELETE FROM requests
         WHERE resource = ? AND status IN ('done', 'failed', 'cancelled', 'expired')
           AND id NOT IN (
               SELECT id FROM requests
                WHERE resource = ?
                  AND status IN ('done', 'failed', 'cancelled', 'expired')
                ORDER BY finished_at DESC, id DESC LIMIT ?
           )
        """,
        (resource, resource, retained),
    )


def enqueue_request(
    conn: sqlite3.Connection,
    resource: str,
    owner: str,
    competition: str,
    command: str,
    now: float | None = None,
) -> str:
    resource = checked_identifier("resource", resource)
    owner = checked_identifier("owner", owner)
    competition = checked_identifier("competition", competition)
    now = now_seconds() if now is None else now
    request_id = secrets.token_hex(12)
    command_digest = hashlib.sha256(command.encode("utf-8")).hexdigest()
    begin_immediate(conn)
    try:
        conn.execute(
            "INSERT OR IGNORE INTO resources(name, grant_seq) VALUES (?, 0)",
            (resource,),
        )
        conn.execute(
            """
            INSERT INTO requests(
                id, resource, owner, competition, command_sha256,
                enqueued_at, waiter_seen_at, status
            ) VALUES (?, ?, ?, ?, ?, ?, ?, 'queued')
            """,
            (request_id, resource, owner, competition, command_digest, now, now),
        )
        commit(conn)
    except BaseException:
        rollback(conn)
        raise
    return request_id


def selected_request_locked(conn: sqlite3.Connection, resource: str) -> sqlite3.Row | None:
    # Owners take turns.  Within one owner's turn, its oldest request wins.
    # A newly seen owner begins at sequence zero, so an established owner
    # cannot monopolize the machine by filling the queue.
    return conn.execute(
        """
        SELECT r.*
          FROM requests r
          LEFT JOIN owners o ON o.resource = r.resource AND o.owner = r.owner
         WHERE r.resource = ? AND r.status = 'queued'
           AND r.id = (
               SELECT r2.id FROM requests r2
                WHERE r2.resource = r.resource AND r2.status = 'queued'
                  AND r2.owner = r.owner
                ORDER BY r2.enqueued_at, r2.id LIMIT 1
           )
         ORDER BY COALESCE(o.last_grant_seq, 0), r.enqueued_at, r.id
         LIMIT 1
        """,
        (resource,),
    ).fetchone()


def queue_position_locked(conn: sqlite3.Connection, request_id: str, resource: str) -> int | None:
    rows = conn.execute(
        """
        SELECT r.id, r.owner, r.enqueued_at, COALESCE(o.last_grant_seq, 0) AS owner_seq
          FROM requests r
          LEFT JOIN owners o ON o.resource = r.resource AND o.owner = r.owner
         WHERE r.resource = ? AND r.status = 'queued'
         ORDER BY owner_seq, r.enqueued_at, r.id
        """,
        (resource,),
    ).fetchall()
    # Collapse each owner's requests into consecutive future turns.  This is an
    # estimate for display only; acquisition always re-evaluates transactionally.
    pending = list(rows)
    position = 0
    while pending:
        owner_order: list[str] = []
        for row in pending:
            if row["owner"] not in owner_order:
                owner_order.append(row["owner"])
        next_pending: list[sqlite3.Row] = []
        served: set[str] = set()
        for row in pending:
            if row["owner"] in served:
                next_pending.append(row)
                continue
            served.add(row["owner"])
            position += 1
            if row["id"] == request_id:
                return position
        pending = next_pending
    return None


def try_acquire(
    conn: sqlite3.Connection,
    request_id: str,
    lease_seconds: int = DEFAULT_LEASE_SECONDS,
    queue_ttl_seconds: int = DEFAULT_QUEUE_TTL_SECONDS,
    now: float | None = None,
) -> dict[str, Any]:
    now = now_seconds() if now is None else now
    lease_seconds = max(15, lease_seconds)
    begin_immediate(conn)
    try:
        request = conn.execute("SELECT * FROM requests WHERE id = ?", (request_id,)).fetchone()
        if request is None:
            result = {"status": "missing", "request_id": request_id}
            commit(conn)
            return result
        resource = request["resource"]
        if request["status"] != "queued":
            result = dict(request)
            commit(conn)
            return result
        conn.execute(
            "UPDATE requests SET waiter_seen_at = ? WHERE id = ? AND status = 'queued'",
            (now, request_id),
        )
        expire_stale_locked(conn, resource, now, queue_ttl_seconds)
        active = conn.execute(
            "SELECT id FROM requests WHERE resource = ? AND status = 'active' LIMIT 1",
            (resource,),
        ).fetchone()
        selected = selected_request_locked(conn, resource)
        if active is not None or selected is None or selected["id"] != request_id:
            refreshed = conn.execute(
                "SELECT * FROM requests WHERE id = ?", (request_id,)
            ).fetchone()
            result = dict(refreshed) if refreshed is not None else {"status": "missing"}
            result["position"] = queue_position_locked(conn, request_id, resource)
            commit(conn)
            return result

        grant_seq = conn.execute(
            "SELECT grant_seq FROM resources WHERE name = ?", (resource,)
        ).fetchone()["grant_seq"] + 1
        lease_token = secrets.token_hex(24)
        conn.execute(
            "UPDATE resources SET grant_seq = ? WHERE name = ?", (grant_seq, resource)
        )
        conn.execute(
            """
            INSERT INTO owners(resource, owner, last_grant_seq) VALUES (?, ?, ?)
            ON CONFLICT(resource, owner)
            DO UPDATE SET last_grant_seq = excluded.last_grant_seq
            """,
            (resource, request["owner"], grant_seq),
        )
        conn.execute(
            """
            UPDATE requests
               SET status = 'active', lease_token = ?, lease_expires_at = ?,
                   heartbeat_at = ?, started_at = ?, message = NULL
             WHERE id = ? AND status = 'queued'
            """,
            (lease_token, now + lease_seconds, now, now, request_id),
        )
        result = dict(
            conn.execute("SELECT * FROM requests WHERE id = ?", (request_id,)).fetchone()
        )
        commit(conn)
        return result
    except BaseException:
        rollback(conn)
        raise


def heartbeat(
    conn: sqlite3.Connection,
    request_id: str,
    lease_token: str,
    lease_seconds: int = DEFAULT_LEASE_SECONDS,
    now: float | None = None,
) -> bool:
    now = now_seconds() if now is None else now
    changed = conn.execute(
        """
        UPDATE requests SET heartbeat_at = ?, lease_expires_at = ?
         WHERE id = ? AND status = 'active' AND lease_token = ?
        """,
        (now, now + max(15, lease_seconds), request_id, lease_token),
    ).rowcount
    return changed == 1


def finish_request(
    conn: sqlite3.Connection,
    request_id: str,
    lease_token: str | None,
    status: str,
    exit_code: int | None,
    message: str,
    now: float | None = None,
) -> bool:
    if status not in TERMINAL_STATES:
        raise ValueError(f"invalid terminal status: {status}")
    now = now_seconds() if now is None else now
    begin_immediate(conn)
    try:
        request = conn.execute("SELECT * FROM requests WHERE id = ?", (request_id,)).fetchone()
        if request is None:
            commit(conn)
            return False
        allowed = request["status"] == "queued" and lease_token is None
        allowed = allowed or (
            request["status"] == "active" and request["lease_token"] == lease_token
        )
        if not allowed:
            commit(conn)
            return False
        conn.execute(
            """
            UPDATE requests
               SET status = ?, finished_at = ?, exit_code = ?, message = ?,
                   lease_token = NULL, lease_expires_at = NULL
             WHERE id = ?
            """,
            (status, now, exit_code, bounded_text(message), request_id),
        )
        prune_receipts_locked(conn, request["resource"])
        commit(conn)
        return True
    except BaseException:
        rollback(conn)
        raise


def queue_status(
    conn: sqlite3.Connection,
    resource: str,
    queue_ttl_seconds: int = DEFAULT_QUEUE_TTL_SECONDS,
) -> dict[str, Any]:
    resource = checked_identifier("resource", resource)
    now = now_seconds()
    begin_immediate(conn)
    try:
        expire_stale_locked(conn, resource, now, queue_ttl_seconds)
        prune_receipts_locked(conn, resource)
        active = conn.execute(
            """
            SELECT id, owner, competition, started_at, heartbeat_at, lease_expires_at
              FROM requests WHERE resource = ? AND status = 'active'
             ORDER BY started_at LIMIT 1
            """,
            (resource,),
        ).fetchone()
        queued = conn.execute(
            """
            SELECT id, owner, competition, enqueued_at, waiter_seen_at
              FROM requests WHERE resource = ? AND status = 'queued'
             ORDER BY enqueued_at, id LIMIT 256
            """,
            (resource,),
        ).fetchall()
        completed = conn.execute(
            """
            SELECT id, owner, competition, status, finished_at, exit_code, message
              FROM requests
             WHERE resource = ? AND status IN ('done', 'failed', 'cancelled', 'expired')
             ORDER BY finished_at DESC, id DESC LIMIT 20
            """,
            (resource,),
        ).fetchall()
        result = {
            "resource": resource,
            "active": dict(active) if active is not None else None,
            "queued": [dict(row) for row in queued],
            "recent": [dict(row) for row in completed],
        }
        commit(conn)
        return result
    except BaseException:
        rollback(conn)
        raise


class HeartbeatThread:
    def __init__(
        self, db_path: str, request_id: str, lease_token: str, lease_seconds: int
    ) -> None:
        self.db_path = db_path
        self.request_id = request_id
        self.lease_token = lease_token
        self.lease_seconds = lease_seconds
        self.stop_event = threading.Event()
        self.lost = threading.Event()
        self.thread = threading.Thread(target=self._run, name="machine-lease-heartbeat", daemon=True)

    def start(self) -> None:
        self.thread.start()

    def stop(self) -> None:
        self.stop_event.set()
        self.thread.join(timeout=5)

    def _run(self) -> None:
        interval = max(2.0, min(30.0, self.lease_seconds / 3))
        conn = connect_database(self.db_path)
        try:
            while not self.stop_event.wait(interval):
                if not heartbeat(
                    conn,
                    self.request_id,
                    self.lease_token,
                    self.lease_seconds,
                ):
                    self.lost.set()
                    return
        finally:
            conn.close()


def run_command_with_lease(payload: dict[str, Any]) -> int:
    resource = checked_identifier("resource", str(payload.get("resource", DEFAULT_RESOURCE)))
    owner = checked_identifier("owner", str(payload["owner"]))
    competition = checked_identifier("competition", str(payload["competition"]))
    command = str(payload["command"])
    if not command.strip():
        raise ValueError("command cannot be empty")
    db_path = str(payload.get("db", DEFAULT_DB))
    cwd = Path(str(payload.get("cwd", "."))).expanduser().resolve()
    wait_seconds = max(0, int(payload.get("wait_seconds", 0)))
    lease_seconds = max(15, int(payload.get("lease_seconds", DEFAULT_LEASE_SECONDS)))
    max_run_seconds = max(0, int(payload.get("max_run_seconds", 0)))
    queue_ttl_seconds = max(
        10, int(payload.get("queue_ttl_seconds", DEFAULT_QUEUE_TTL_SECONDS))
    )
    if not cwd.is_dir():
        raise ValueError(f"remote working directory does not exist: {cwd}")

    conn = connect_database(db_path)
    request_id = enqueue_request(conn, resource, owner, competition, command)
    print(
        json.dumps(
            {
                "event": "queued",
                "request_id": request_id,
                "resource": resource,
                "owner": owner,
                "competition": competition,
            },
            sort_keys=True,
        ),
        flush=True,
    )
    wait_started = time.monotonic()
    last_position: int | None = None
    last_notice = 0.0
    lease: dict[str, Any] | None = None
    initial_handlers: dict[int, Any] = {}

    def interrupt_queue_wait(_signum: int, _frame: Any) -> None:
        raise KeyboardInterrupt

    for signum in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        initial_handlers[signum] = signal.signal(signum, interrupt_queue_wait)
    try:
        while lease is None:
            candidate = try_acquire(
                conn, request_id, lease_seconds, queue_ttl_seconds=queue_ttl_seconds
            )
            if candidate.get("status") == "active":
                lease = candidate
                break
            if candidate.get("status") != "queued":
                raise RuntimeError(
                    f"queue request ended before acquisition: {candidate.get('status')}"
                )
            position = candidate.get("position")
            elapsed = time.monotonic() - wait_started
            if position != last_position or elapsed - last_notice >= 30:
                print(
                    json.dumps(
                        {
                            "event": "waiting",
                            "request_id": request_id,
                            "position": position,
                            "waited_seconds": round(elapsed, 1),
                        },
                        sort_keys=True,
                    ),
                    flush=True,
                )
                last_position = position
                last_notice = elapsed
            if wait_seconds and elapsed >= wait_seconds:
                finish_request(conn, request_id, None, "cancelled", None, "queue wait timeout")
                print(json.dumps({"event": "queue_timeout", "request_id": request_id}), flush=True)
                for signum, previous in initial_handlers.items():
                    signal.signal(signum, previous)
                conn.close()
                return 124
            time.sleep(POLL_SECONDS)
    except KeyboardInterrupt:
        finish_request(conn, request_id, None, "cancelled", None, "queue waiter interrupted")
        for signum, previous in initial_handlers.items():
            signal.signal(signum, previous)
        conn.close()
        print(json.dumps({"event": "queue_cancelled", "request_id": request_id}), flush=True)
        return 130
    except BaseException:
        finish_request(conn, request_id, None, "cancelled", None, "queue waiter interrupted")
        for signum, previous in initial_handlers.items():
            signal.signal(signum, previous)
        conn.close()
        raise

    lease_token = str(lease["lease_token"])
    print(
        json.dumps(
            {
                "event": "acquired",
                "request_id": request_id,
                "waited_seconds": round(time.monotonic() - wait_started, 1),
            },
            sort_keys=True,
        ),
        flush=True,
    )
    heartbeat_thread = HeartbeatThread(db_path, request_id, lease_token, lease_seconds)
    heartbeat_thread.start()
    child: subprocess.Popen[bytes] | None = None
    termination_reason: str | None = None
    termination_started: float | None = None

    def request_child_stop(reason: str) -> None:
        nonlocal termination_reason, termination_started
        if termination_reason is None:
            termination_reason = reason
            termination_started = time.monotonic()
        if child is not None and child.poll() is None:
            try:
                os.killpg(child.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass

    def terminate_child(_signum: int, _frame: Any) -> None:
        request_child_stop("operator")

    for signum in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(signum, terminate_child)

    run_started = time.monotonic()
    exit_code = 1
    status = "failed"
    message = "test command failed"
    try:
        child = subprocess.Popen(
            ["/bin/sh", "-lc", command], cwd=str(cwd), start_new_session=True
        )
        if termination_reason is not None:
            try:
                os.killpg(child.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
        while child.poll() is None:
            if heartbeat_thread.lost.is_set():
                request_child_stop("lease_lost")
            if max_run_seconds and time.monotonic() - run_started >= max_run_seconds:
                request_child_stop("quantum_expired")
            if (
                termination_started is not None
                and time.monotonic() - termination_started >= 5
                and child.poll() is None
            ):
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            try:
                child.wait(timeout=1)
            except subprocess.TimeoutExpired:
                continue
        exit_code = int(child.returncode or 0)
        if termination_reason == "operator":
            status = "cancelled"
            message = "test interrupted"
        elif termination_reason == "lease_lost":
            status = "failed"
            message = "machine lease was lost while the test was running"
        elif termination_reason == "quantum_expired":
            status = "failed"
            message = "maximum test runtime exceeded"
        elif exit_code == 0:
            status = "done"
            message = "test completed"
        else:
            status = "failed"
            message = message if message == "maximum test runtime exceeded" else "test command failed"
    finally:
        heartbeat_thread.stop()
        for signum, previous in initial_handlers.items():
            signal.signal(signum, previous)
        finish_request(conn, request_id, lease_token, status, exit_code, message)
        conn.execute("PRAGMA wal_checkpoint(PASSIVE)")
        conn.close()

    print(
        json.dumps(
            {
                "event": "released",
                "request_id": request_id,
                "status": status,
                "exit_code": exit_code,
                "run_seconds": round(time.monotonic() - run_started, 1),
            },
            sort_keys=True,
        ),
        flush=True,
    )
    return exit_code if status in ("done", "failed") else 130


def encode_payload(payload: dict[str, Any]) -> str:
    raw = json.dumps(payload, separators=(",", ":")).encode("utf-8")
    return base64.urlsafe_b64encode(raw).decode("ascii")


def decode_payload(encoded: str) -> dict[str, Any]:
    return json.loads(base64.urlsafe_b64decode(encoded.encode("ascii")).decode("utf-8"))


def remote_program(remote_script: str) -> str:
    if remote_script.startswith("/"):
        return shlex.quote(remote_script)
    clean = remote_script.removeprefix("~/").lstrip("/")
    if not clean or "\x00" in clean:
        raise ValueError("invalid remote script path")
    return f'"$HOME"/{shlex.quote(clean)}'


def ssh_run_payload(host: str, remote_script: str, payload: dict[str, Any]) -> int:
    host = checked_identifier("host", host)
    command = f"python3 {remote_program(remote_script)} run-payload"
    completed = subprocess.run(
        ["ssh", host, command],
        input=(encode_payload(payload) + "\n").encode("ascii"),
        check=False,
    )
    return completed.returncode


def ssh_status(host: str, remote_script: str, resource: str, db_path: str) -> int:
    host = checked_identifier("host", host)
    resource = checked_identifier("resource", resource)
    payload = encode_payload({"resource": resource, "db": db_path})
    command = f"python3 {remote_program(remote_script)} status-payload"
    return subprocess.run(
        ["ssh", host, command], input=(payload + "\n").encode("ascii"), check=False
    ).returncode


def install_remote(host: str, remote_script: str) -> int:
    host = checked_identifier("host", host)
    local_script = Path(__file__).resolve()
    if remote_script.startswith("/"):
        remote_target = remote_script
        parent = str(Path(remote_script).parent)
        mkdir_command = f"mkdir -p {shlex.quote(parent)}"
        chmod_command = f"chmod 700 {shlex.quote(remote_script)}"
    else:
        clean = remote_script.removeprefix("~/").lstrip("/")
        if not clean or "\x00" in clean:
            raise ValueError("invalid remote script path")
        remote_target = clean
        parent = str(Path(clean).parent)
        mkdir_command = f'mkdir -p "$HOME"/{shlex.quote(parent)}'
        chmod_command = f'chmod 700 "$HOME"/{shlex.quote(clean)}'
    if subprocess.run(["ssh", host, mkdir_command], check=False).returncode != 0:
        return 1
    if subprocess.run(["scp", str(local_script), f"{host}:{remote_target}"], check=False).returncode != 0:
        return 1
    if subprocess.run(["ssh", host, chmod_command], check=False).returncode != 0:
        return 1
    verify_command = f"python3 {remote_program(remote_script)} --help >/dev/null"
    return subprocess.run(["ssh", host, verify_command], check=False).returncode


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    sub = root.add_subparsers(dest="action", required=True)

    sub.add_parser("run-payload", help="run a base64 JSON job read from stdin")
    sub.add_parser("status-payload", help="show status from a base64 JSON request on stdin")

    status = sub.add_parser("status", help="show one local resource queue")
    status.add_argument("--resource", default=DEFAULT_RESOURCE)
    status.add_argument("--db", default=DEFAULT_DB)

    client = sub.add_parser("client-run", help="queue and run a command on an SSH host")
    client.add_argument("--host", required=True)
    client.add_argument("--remote-script", default=DEFAULT_REMOTE_SCRIPT)
    client.add_argument("--remote-db", default=DEFAULT_DB)
    client.add_argument("--resource", default=DEFAULT_RESOURCE)
    client.add_argument("--owner", required=True)
    client.add_argument("--competition", required=True)
    client.add_argument("--command", required=True)
    client.add_argument("--cwd", default=".")
    client.add_argument("--wait-seconds", type=int, default=0)
    client.add_argument("--lease-seconds", type=int, default=DEFAULT_LEASE_SECONDS)
    client.add_argument("--max-run-seconds", type=int, default=0)

    client_status = sub.add_parser("client-status", help="show a remote resource queue")
    client_status.add_argument("--host", required=True)
    client_status.add_argument("--remote-script", default=DEFAULT_REMOTE_SCRIPT)
    client_status.add_argument("--remote-db", default=DEFAULT_DB)
    client_status.add_argument("--resource", default=DEFAULT_RESOURCE)

    install = sub.add_parser("client-install", help="install this broker script over SSH")
    install.add_argument("--host", required=True)
    install.add_argument("--remote-script", default=DEFAULT_REMOTE_SCRIPT)
    return root


def main() -> int:
    args = parser().parse_args()
    if args.action == "run-payload":
        payload_text = sys.stdin.readline().strip()
        if not payload_text:
            raise ValueError("missing stdin payload")
        return run_command_with_lease(decode_payload(payload_text))
    if args.action == "status-payload":
        payload_text = sys.stdin.readline().strip()
        if not payload_text:
            raise ValueError("missing stdin payload")
        payload = decode_payload(payload_text)
        conn = connect_database(str(payload.get("db", DEFAULT_DB)))
        try:
            print(json.dumps(queue_status(conn, str(payload["resource"])), indent=2, sort_keys=True))
        finally:
            conn.close()
        return 0
    if args.action == "status":
        conn = connect_database(args.db)
        try:
            print(json.dumps(queue_status(conn, args.resource), indent=2, sort_keys=True))
        finally:
            conn.close()
        return 0
    if args.action == "client-run":
        return ssh_run_payload(
            args.host,
            args.remote_script,
            {
                "db": args.remote_db,
                "resource": args.resource,
                "owner": args.owner,
                "competition": args.competition,
                "command": args.command,
                "cwd": args.cwd,
                "wait_seconds": args.wait_seconds,
                "lease_seconds": args.lease_seconds,
                "max_run_seconds": args.max_run_seconds,
            },
        )
    if args.action == "client-status":
        return ssh_status(
            args.host, args.remote_script, args.resource, args.remote_db
        )
    if args.action == "client-install":
        return install_remote(args.host, args.remote_script)
    raise AssertionError(args.action)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, RuntimeError, sqlite3.Error) as error:
        print(f"machine queue: {error}", file=sys.stderr)
        raise SystemExit(2)
