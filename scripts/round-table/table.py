#!/usr/bin/env python3
"""Local round table for existing agent chats. Python standard library only."""
import argparse
import contextlib
import html
import json
import os
import re
from pathlib import Path
import secrets
import sqlite3
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

HERE = Path(__file__).resolve().parent
DEFAULT_STATE = Path(os.environ.get("XDG_STATE_HOME", str(Path.home() / ".local/state"))) / "round-table"
TEAMS = ("dots", "grok", "v", "muse")
TEAM_NAMES = {"dots": "Codex / ChatGPT Dots", "grok": "Grok Bot", "v": "V", "muse": "Muse"}
LEASE_SECONDS = 600


class Table:
    def __init__(self, state_dir=DEFAULT_STATE):
        self.root = Path(state_dir)
        self.root.mkdir(parents=True, exist_ok=True, mode=0o700)
        self.path = self.root / "table.sqlite3"
        with self.db() as db:
            db.executescript("""
                CREATE TABLE IF NOT EXISTS agents (
                    id TEXT PRIMARY KEY, team TEXT NOT NULL, name TEXT NOT NULL,
                    leader INTEGER NOT NULL DEFAULT 0, joined INTEGER NOT NULL DEFAULT 0,
                    seen REAL NOT NULL DEFAULT 0);
                CREATE UNIQUE INDEX IF NOT EXISTS one_leader ON agents(team) WHERE leader=1;
                CREATE TABLE IF NOT EXISTS rounds (
                    id INTEGER PRIMARY KEY, question TEXT NOT NULL, passes INTEGER NOT NULL,
                    phase INTEGER NOT NULL DEFAULT 1, state TEXT NOT NULL DEFAULT 'active',
                    targets TEXT NOT NULL, created REAL NOT NULL);
                CREATE TABLE IF NOT EXISTS tasks (
                    id INTEGER PRIMARY KEY, round_id INTEGER NOT NULL, agent TEXT NOT NULL,
                    phase INTEGER NOT NULL, state TEXT NOT NULL DEFAULT 'queued',
                    lease REAL NOT NULL DEFAULT 0,
                    UNIQUE(round_id, agent, phase));
                CREATE TABLE IF NOT EXISTS messages (
                    id INTEGER PRIMARY KEY, author TEXT NOT NULL, name TEXT NOT NULL,
                    text TEXT NOT NULL, created REAL NOT NULL, round_id INTEGER,
                    task_id INTEGER UNIQUE);
                CREATE TABLE IF NOT EXISTS access_keys (role TEXT PRIMARY KEY, token TEXT UNIQUE NOT NULL);
                CREATE TABLE IF NOT EXISTS settings (name TEXT PRIMARY KEY, value TEXT NOT NULL);
                CREATE TABLE IF NOT EXISTS workers (
                    team TEXT PRIMARY KEY, agent TEXT UNIQUE NOT NULL, provider TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'starting', heartbeat REAL NOT NULL DEFAULT 0,
                    error TEXT NOT NULL DEFAULT '');
            """)
            columns = {r[1] for r in db.execute("PRAGMA table_info(tasks)")}
            for name, declaration in (("request", "TEXT NOT NULL DEFAULT ''"), ("parent", "INTEGER")):
                if name not in columns:
                    db.execute(f"ALTER TABLE tasks ADD COLUMN {name} {declaration}")
            if "retry_after" not in {r[1] for r in db.execute("PRAGMA table_info(workers)")}:
                db.execute("ALTER TABLE workers ADD COLUMN retry_after REAL NOT NULL DEFAULT 0")
            for team, name in (("dots", "Dots leader"), ("grok", "Grok leader"), ("v", "V"), ("muse", "Muse")):
                db.execute("INSERT OR IGNORE INTO agents(id,team,name,leader) VALUES(?,?,?,1)",
                           (team + "-lead", team, name))
            for role in ("host", *TEAMS):
                db.execute("INSERT OR IGNORE INTO access_keys(role,token) VALUES(?,?)", (role, secrets.token_urlsafe(32)))
        self.path.chmod(0o600)

    @contextlib.contextmanager
    def db(self):
        db = sqlite3.connect(self.path, timeout=10)
        db.row_factory = sqlite3.Row
        db.execute("PRAGMA journal_mode=WAL")
        try:
            with db:
                yield db
        finally:
            db.close()

    def join(self, team, name, member=None):
        if team not in TEAMS:
            raise ValueError("team must be dots, grok, v or muse")
        name = name.strip()
        if not name or len(name) > 100:
            raise ValueError("give a name of 1–100 characters")
        agent = team + "-lead" if member is None else team + ":" + member.strip()
        if member is not None and (not member.strip() or len(member) > 80):
            raise ValueError("member ID must be 1–80 characters")
        with self.db() as db:
            is_leader = member is None and not db.execute("SELECT 1 FROM workers WHERE team=?", (team,)).fetchone()
            db.execute("""INSERT INTO agents(id,team,name,leader,joined,seen) VALUES(?,?,?,?,1,?)
                ON CONFLICT(id) DO UPDATE SET name=excluded.name,joined=1,seen=excluded.seen""",
                       (agent, team, name, bool(is_leader), time.time()))
        return {"agent": agent, "team": team, "role": "leader" if is_leader else "member"}

    def send(self, text, target="both", passes=2):
        text = text.strip()
        if not text or len(text) > 50000:
            raise ValueError("message must contain 1–50,000 characters")
        if target not in ("all", "both", *TEAMS) or passes not in (1, 2, 3):
            raise ValueError("choose all, dots, grok, v or muse, and 1–3 discussion passes")
        with self.db() as db:
            db.execute("BEGIN IMMEDIATE")
            selected = TEAMS if target == "all" else TEAMS[:2] if target == "both" else (target,)
            mentions = re.findall(r"(?:^|\s)@(all|codex|dots|grok|v|muse)\b", text, re.I)
            if mentions:
                selected = TEAMS if "all" in [x.lower() for x in mentions] else tuple(dict.fromkeys(
                    "v" if x.lower() == "codex" else x.lower() for x in mentions))
            managed = {r[0] for r in db.execute("SELECT team FROM workers WHERE retry_after<=?", (time.time(),))}
            if selected == TEAMS and managed:
                selected = tuple(t for t in TEAMS if t in managed)
            agents = [db.execute("SELECT id FROM agents WHERE team=? AND leader=1", (t,)).fetchone()[0]
                      for t in selected]
            rid = db.execute("INSERT INTO rounds(question,passes,targets,created) VALUES(?,?,?,?)",
                             (text, passes, json.dumps(agents), time.time())).lastrowid
            db.execute("INSERT INTO messages(author,name,text,created,round_id) VALUES('human','You',?,?,?)",
                       (text, time.time(), rid))
            for agent in agents:
                db.execute("INSERT INTO tasks(round_id,agent,phase) VALUES(?,?,1)", (rid, agent))
        return {"round": rid, "targets": agents, "passes": passes}

    def task(self, agent):
        with self.db() as db:
            db.execute("BEGIN IMMEDIATE")
            person = db.execute("SELECT * FROM agents WHERE id=? AND joined=1", (agent,)).fetchone()
            if person is None:
                raise ValueError("join your seat first")
            now = time.time()
            db.execute("UPDATE agents SET seen=? WHERE id=?", (now, agent))
            # Return an existing lease as well: a lost CLI result never loses a turn.
            task = db.execute("""SELECT t.*,r.question,r.passes FROM tasks t JOIN rounds r ON r.id=t.round_id
                WHERE t.agent=? AND r.state='active' AND t.state IN ('queued','leased')
                ORDER BY t.id LIMIT 1""", (agent,)).fetchone()
            if task is None:
                return None
            db.execute("UPDATE tasks SET state='leased',lease=? WHERE id=?", (now + LEASE_SECONDS, task["id"]))
            # Recent room context plus the full current question; nothing is silently invented.
            rows = db.execute("SELECT * FROM messages ORDER BY id DESC LIMIT 80").fetchall()[::-1]
            transcript = [dict(row) for row in rows]
            budget = 60000
            kept = []
            for row in reversed(transcript):
                if kept and budget < len(row["text"]):
                    break
                kept.append(row)
                budget -= len(row["text"])
            instruction = ("Give your group's view on the human's question. Be specific and concise."
                           if task["phase"] == 1 else
                           "Read the other leader's latest contribution. Address agreements, disagreements, "
                           "missing evidence and a useful next step. Do not merely repeat your first reply.")
            return {"id": task["id"], "round": task["round_id"], "phase": task["phase"],
                    "passes": task["passes"], "question": task["question"], "instruction": instruction,
                    "team": person["team"], "transcript": kept[::-1],
                    "peer_request": task["request"], "peer_turn": task["parent"] is not None,
                    "context_note": "Recent room messages; the complete current question is above."}

    def wait(self, agent, seconds=25):
        deadline = time.monotonic() + max(0, min(50, seconds))
        while True:
            task = self.task(agent)
            if task is not None or time.monotonic() >= deadline:
                return {"task": task, "status": "reply needed" if task else "idle"}
            time.sleep(0.4)

    def reply(self, agent, task_id, text, pings=None):
        text = text.strip()
        if not text or len(text) > 50000:
            raise ValueError("reply must contain 1–50,000 characters")
        with self.db() as db:
            db.execute("BEGIN IMMEDIATE")
            task = db.execute("SELECT * FROM tasks WHERE id=? AND agent=?", (task_id, agent)).fetchone()
            if task is None:
                raise ValueError("this task belongs to another seat or does not exist")
            if task["state"] == "done":
                return {"ok": True, "duplicate": True, "task": task_id}
            if task["state"] == "cancelled":
                raise ValueError("the human stopped this discussion; do not send this reply")
            if task["state"] != "leased":
                raise ValueError("read the task with wait before replying")
            person = db.execute("SELECT * FROM agents WHERE id=? AND joined=1", (agent,)).fetchone()
            if person is None:
                raise ValueError("join your seat first")
            db.execute("INSERT INTO messages(author,name,text,created,round_id,task_id) VALUES(?,?,?,?,?,?)",
                       (agent, person["name"], text, time.time(), task["round_id"], task_id))
            db.execute("UPDATE tasks SET state='done' WHERE id=?", (task_id,))
            db.execute("UPDATE agents SET seen=? WHERE id=?", (time.time(), agent))
            if task["parent"] is None:
                ping_count = db.execute("SELECT COUNT(*) FROM tasks WHERE round_id=? AND parent IS NOT NULL",
                                        (task["round_id"],)).fetchone()[0]
                for ping in (pings or [])[:2]:
                    if ping_count >= 2 or not isinstance(ping, dict):
                        break
                    peer = db.execute("SELECT agent FROM workers WHERE team=? AND retry_after<=?", (ping.get("team"), time.time())).fetchone()
                    question = str(ping.get("question", "")).strip()[:2000]
                    if not peer or peer[0] == agent or not question:
                        continue
                    created = db.execute("INSERT OR IGNORE INTO tasks(round_id,agent,phase,request,parent) VALUES(?,?,?,?,?)",
                                         (task["round_id"], peer[0], task["phase"], question, task_id)).rowcount
                    ping_count += created
                    if created:
                        db.execute("INSERT INTO messages(author,name,text,created,round_id) VALUES('queue','Room queue',?,?,?)",
                                   (f"{person['name']} pings {TEAM_NAMES[ping['team']]}: {question}", time.time(), task["round_id"]))
            pending = db.execute("SELECT 1 FROM tasks WHERE round_id=? AND state IN ('queued','leased')",
                                 (task["round_id"],)).fetchone()
            if not pending:
                discussion = db.execute("SELECT * FROM rounds WHERE id=?", (task["round_id"],)).fetchone()
                failed = db.execute("SELECT 1 FROM tasks WHERE round_id=? AND state='failed'", (discussion["id"],)).fetchone()
                if failed:
                    db.execute("UPDATE rounds SET state='attention' WHERE id=?", (discussion["id"],))
                elif discussion["phase"] < discussion["passes"]:
                    phase = discussion["phase"] + 1
                    db.execute("UPDATE rounds SET phase=? WHERE id=?", (phase, discussion["id"]))
                    for who in json.loads(discussion["targets"]):
                        db.execute("INSERT INTO tasks(round_id,agent,phase) VALUES(?,?,?)",
                                   (discussion["id"], who, phase))
                else:
                    db.execute("UPDATE rounds SET state='complete' WHERE id=?", (discussion["id"],))
        return {"ok": True, "task": task_id}

    def configure_worker(self, team, provider):
        if team not in TEAMS or provider not in ("codex", "grok", "muse"):
            raise ValueError("Unknown worker route")
        agent = team + ":worker"
        name = {"v": "V / Codex", "grok": "Grok CLI", "muse": "Muse CLI"}.get(team, TEAM_NAMES[team]) + " [local worker]"
        with self.db() as db:
            db.execute("BEGIN IMMEDIATE")
            db.execute("UPDATE agents SET leader=0 WHERE team=?", (team,))
            db.execute("INSERT INTO agents(id,team,name,leader,joined,seen) VALUES(?,?,?,1,1,0) ON CONFLICT(id) DO UPDATE SET name=excluded.name,leader=1,joined=1",
                       (agent, team, name))
            db.execute("INSERT INTO workers(team,agent,provider) VALUES(?,?,?) ON CONFLICT(team) DO UPDATE SET provider=excluded.provider", (team, agent, provider))
            db.execute("UPDATE tasks SET agent=? WHERE agent=? AND state IN ('queued','leased')", (agent, team + "-lead"))
            db.execute("UPDATE rounds SET passes=1 WHERE state='active'")
        return agent

    def worker_health(self, agent, status=None, error=""):
        with self.db() as db:
            db.execute("UPDATE workers SET heartbeat=? WHERE agent=?", (time.time(), agent))
            if status is not None:
                db.execute("UPDATE workers SET status=?,error=? WHERE agent=?", (status, error[:500], agent))
            db.execute("UPDATE agents SET seen=? WHERE id=?", (time.time(), agent))

    def worker_failed(self, agent, task_id, error, retry_after=0):
        with self.db() as db:
            db.execute("UPDATE tasks SET state='failed' WHERE id=? AND agent=? AND state IN ('queued','leased')", (task_id, agent))
            db.execute("UPDATE rounds SET state='attention' WHERE id=(SELECT round_id FROM tasks WHERE id=?) AND NOT EXISTS (SELECT 1 FROM tasks t WHERE t.round_id=rounds.id AND t.state IN ('queued','leased'))", (task_id,))
        self.worker_health(agent, "error", error)
        if retry_after:
            with self.db() as db:
                db.execute("UPDATE workers SET retry_after=?,status='cooldown' WHERE agent=?", (retry_after, agent))

    def say(self, agent, text):
        text = text.strip()
        if not text or len(text) > 50000:
            raise ValueError("message must contain 1–50,000 characters")
        with self.db() as db:
            person = db.execute("SELECT * FROM agents WHERE id=? AND joined=1", (agent,)).fetchone()
            if person is None:
                raise ValueError("join your seat first")
            db.execute("INSERT INTO messages(author,name,text,created) VALUES(?,?,?,?)",
                       (agent, person["name"], text, time.time()))
            db.execute("UPDATE agents SET seen=? WHERE id=?", (time.time(), agent))
        return {"ok": True}

    def stop(self):
        with self.db() as db:
            db.execute("BEGIN IMMEDIATE")
            count = db.execute("UPDATE tasks SET state='cancelled' WHERE state IN ('queued','leased')").rowcount
            db.execute("UPDATE rounds SET state='stopped' WHERE state='active'")
        return {"ok": True, "cancelled": count}

    def leave(self, agent):
        with self.db() as db:
            db.execute("UPDATE agents SET joined=0,seen=0 WHERE id=?", (agent,))
        return {"ok": True}

    def role(self, token):
        if not token:
            return None
        with self.db() as db:
            row = db.execute("SELECT role FROM access_keys WHERE token=?", (token,)).fetchone()
            return row[0] if row else None

    def key(self, role):
        with self.db() as db:
            return db.execute("SELECT token FROM access_keys WHERE role=?", (role,)).fetchone()[0]

    def csrf(self):
        with self.db() as db:
            db.execute("INSERT OR IGNORE INTO settings(name,value) VALUES('http_csrf',?)", (secrets.token_urlsafe(32),))
            return db.execute("SELECT value FROM settings WHERE name='http_csrf'").fetchone()[0]

    def public_url(self):
        with self.db() as db:
            row = db.execute("SELECT value FROM settings WHERE name='public_url'").fetchone()
            return row[0] if row else "http://127.0.0.1:18791"

    def set_public_url(self, url):
        parsed = urlparse(url)
        if parsed.scheme not in ("http", "https") or not parsed.netloc or parsed.path not in ("", "/"):
            raise ValueError("use the room's base HTTP(S) address")
        with self.db() as db:
            db.execute("INSERT OR REPLACE INTO settings(name,value) VALUES('public_url',?)", (url.rstrip('/'),))

    def links(self, base=None):
        base = (base or self.public_url()).rstrip('/')
        return {role: (f"{base}/#key={self.key(role)}" if role == "host" else
                       f"{base}/join/{self.key(role)}") for role in ("host", *TEAMS)}

    def record_open(self, role):
        if role in TEAMS:
            with self.db() as db:
                db.execute("INSERT OR REPLACE INTO settings(name,value) VALUES(?,?)",
                           ("opened:" + role, str(time.time())))

    def state(self):
        with self.db() as db:
            agents = [dict(row) for row in db.execute("SELECT * FROM agents ORDER BY team,leader DESC,name")]
            tasks = [dict(row) for row in db.execute("SELECT * FROM tasks WHERE state IN ('queued','leased')")]
            for person in agents:
                worker = db.execute("SELECT * FROM workers WHERE agent=?", (person["id"],)).fetchone()
                person["worker"] = dict(worker) if worker else None
                opened = db.execute("SELECT value FROM settings WHERE name=?", ("opened:" + person["team"],)).fetchone()
                person["opened"] = float(opened[0]) if opened else 0
                own = [t for t in tasks if t["agent"] == person["id"]]
                person["pending"] = len(own)
                recent = person["joined"] and time.time() - person["seen"] < 90
                person["status"] = ("awaiting connection" if not person["joined"] else
                                    "check chat / reconnect" if not recent else
                                    "reply in progress" if any(t["state"] == "leased" for t in own) else
                                    "connected")
                if worker and worker["status"] in ("error", "cooldown"):
                    person["status"] = "worker error"
            return {"agents": agents, "messages": [dict(r) for r in db.execute("SELECT * FROM messages ORDER BY id")],
                    "rounds": [dict(r) for r in db.execute("SELECT * FROM rounds ORDER BY id DESC LIMIT 20")],
                    "pending": len(tasks), "client_version": str((HERE / "index.html").stat().st_mtime_ns)}

    def invite(self, team):
        if team not in TEAMS:
            raise ValueError("team must be dots, grok, v or muse")
        return (f"Join my Round Table as {TEAM_NAMES[team]}. Open this link in your browser, "
                f"enter your screen name, and click Join Room: {self.links()[team]}\n"
                "Read the shared chat and respond using its message box. Keep the page open "
                "for follow-ups. You can participate entirely through the webpage.")


def handler(table, port):
    csrf = table.csrf()

    def reader_view(role, key):
        person = next(a for a in table.state()["agents"] if a["id"] == role + "-lead")
        task = table.task(person["id"]) if person["joined"] else None
        state = table.state()
        esc = html.escape
        messages = "".join(f"<p><b>{esc(m['name'])}:</b> {esc(m['text'])}</p>" for m in state["messages"])
        people = "".join(f"<li>{esc(a['name'])}: {esc(a['status'])}</li>" for a in state["agents"] if a["leader"])
        action = f"/join/{key}"
        hidden = f'<input type="hidden" name="csrf" value="{csrf}">'
        if person["joined"]:
            prompt = f"<p><b>Your turn:</b> {esc(task['question'])}<br>{esc(task['instruction'])}</p>" if task else "<p>No assigned turn. You may say hello or respond to the chat.</p>"
            task_input = f'<input type="hidden" name="task" value="{task["id"] if task else ""}">'
            form = f'<form method="post" action="{action}">{hidden}{task_input}<input type="hidden" name="operation" value="chat"><label>Your message:<textarea name="text" required maxlength="50000" style="display:block;width:100%"></textarea></label><button type="submit">Send to room</button></form>'
        else:
            prompt = "<p>Your invitation has opened, but you have not joined yet.</p>"
            form = f'<form method="post" action="{action}">{hidden}<input type="hidden" name="operation" value="join"><label>Screen name: <input name="name" required maxlength="100"></label><button type="submit">Join Room</button></form>'
        return f'''<section id="readerFallback" class="window" style="padding:16px"><h1>Round Table — {esc(TEAM_NAMES[role])}</h1>
<p>This view works without JavaScript. You are joining as yourself; the room does not run your agent for you. Keep your agent session active to participate.</p>
<h2>People</h2><ul>{people}</ul><h2>Conversation</h2>{messages or '<p>No messages yet.</p>'}{prompt}{form}
<p><a href="{action}">Refresh room / check for the next turn</a></p>
<details><summary>For agents using HTTP tools</summary><p>Your seat is {esc(role)}. Use this invitation key as a Bearer token with /api/session, /api/state, /api/task, /api/join, /api/say and /api/reply. POST requests also require the X-Table-CSRF value from this page's form. You can alternatively submit the ordinary forms above. If you cannot browse or make HTTP requests, tell the user; reading an invitation alone does not join the room.</p></details></section>'''

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_args):
            pass

        def respond(self, status, body, kind="application/json; charset=utf-8"):
            if not isinstance(body, bytes):
                body = json.dumps(body).encode()
            self.send_response(status)
            self.send_header("Content-Type", kind)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.send_header("Referrer-Policy", "no-referrer")
            self.send_header("X-Content-Type-Options", "nosniff")
            self.send_header("Content-Security-Policy", "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; frame-ancestors 'none'")
            self.end_headers()
            self.wfile.write(body)

        def allowed(self):
            return self.headers.get("Host") in (
                f"127.0.0.1:{port}", f"localhost:{port}", urlparse(table.public_url()).netloc)

        def role(self):
            auth = self.headers.get("Authorization", "")
            return table.role(auth[7:] if auth.startswith("Bearer ") else "")

        def do_GET(self):
            if not self.allowed():
                return self.respond(403, {"error": "unknown room address"})
            path = urlparse(self.path).path
            if path == "/" or path.startswith("/join/"):
                key = path.removeprefix("/join/") if path.startswith("/join/") else ""
                role = table.role(key) if key else None
                if key and role not in TEAMS:
                    return self.respond(401, {"error": "Invalid seat invitation."})
                if role:
                    table.record_open(role)
                body = ((HERE / "index.html").read_text().replace("__CSRF__", csrf)
                        .replace("__INVITE_KEY__", json.dumps(key))
                        .replace("<!--READER_VIEW-->", reader_view(role, key) if role else "")
                        .replace("__CLIENT_VERSION__", str((HERE / "index.html").stat().st_mtime_ns))
                        .replace("__LIVE_CLASS__", "hidden" if role else ""))
                return self.respond(200, body.encode(), "text/html; charset=utf-8")
            role = self.role()
            if not role:
                return self.respond(401, {"error": "Open your room invitation link to join."})
            if path == "/api/session":
                table.record_open(role)
                return self.respond(200, {"role": role, "host": role == "host",
                    "agent": None if role == "host" else role + "-lead",
                    "team_name": TEAM_NAMES.get(role, "Room host")})
            if path == "/api/state":
                return self.respond(200, table.state())
            if path == "/api/task" and role != "host":
                try:
                    return self.respond(200, {"task": table.task(role + "-lead")})
                except ValueError as exc:
                    return self.respond(400, {"error": str(exc)})
            if path.startswith("/api/invite/"):
                if role != "host":
                    return self.respond(403, {"error": "Only the room host shares invitations."})
                team = path.rsplit("/", 1)[1]
                if team not in TEAMS:
                    return self.respond(400, {"error": "Unknown seat"})
                link = table.links()[team]
                return self.respond(200, {"link": link, "text":
                    f"Join my Round Table as {TEAM_NAMES[team]}. Open this link in your browser, enter your screen name and click Join Room: {link}\n"
                    "Read the shared conversation and reply in the chat box. Keep the page open to receive follow-ups. "
                    "Use your own identity; this invitation is for your seat."})
            if path == "/export":
                state = table.state()
                body = "# Round Table\n\n" + "\n\n".join(f"## {m['name']}\n\n{m['text']}" for m in state["messages"])
                return self.respond(200, body.encode(), "text/markdown; charset=utf-8")
            return self.respond(404, {"error": "not found"})

        def do_POST(self):
            if self.path.startswith("/join/"):
                return self.reader_post()
            if not self.allowed() or self.headers.get("X-Table-CSRF") != csrf:
                return self.respond(403, {"error": "reload the room before sending"})
            role = self.role()
            if not role:
                return self.respond(401, {"error": "Open your room invitation link to join."})
            origin = self.headers.get("Origin")
            allowed_origins = (f"http://127.0.0.1:{port}", f"http://localhost:{port}", table.public_url())
            if origin and origin not in allowed_origins:
                return self.respond(403, {"error": "Use the room's own page."})
            host_paths = ("/api/send", "/api/stop")
            agent_paths = ("/api/join", "/api/reply", "/api/say", "/api/leave")
            if (self.path in host_paths and role != "host") or (self.path in agent_paths and role == "host"):
                return self.respond(403, {"error": "That control belongs to another seat."})
            try:
                length = int(self.headers.get("Content-Length", "0"))
                if not 0 < length <= 250000:
                    raise ValueError("invalid message size")
                data = json.loads(self.rfile.read(length))
                if not isinstance(data, dict):
                    raise ValueError("expected an object")
                agent = role + "-lead"
                if self.path == "/api/send":
                    result = table.send(str(data.get("text", "")), data.get("target", "all"), data.get("passes", 1))
                elif self.path == "/api/stop":
                    result = table.stop()
                elif self.path == "/api/join":
                    result = table.join(role, str(data.get("name", "")))
                elif self.path == "/api/reply":
                    result = table.reply(agent, int(data["task"]), str(data.get("text", "")))
                elif self.path == "/api/say":
                    result = table.say(agent, str(data.get("text", "")))
                elif self.path == "/api/leave":
                    result = table.leave(agent)
                else:
                    return self.respond(404, {"error": "not found"})
                self.respond(200, result)
            except (ValueError, TypeError, KeyError) as exc:
                self.respond(400, {"error": str(exc)})

        def reader_post(self):
            role = table.role(self.path.removeprefix("/join/"))
            if not self.allowed() or role not in TEAMS:
                return self.respond(403, {"error": "Invalid seat invitation."})
            origin = self.headers.get("Origin")
            if origin and origin not in (f"http://127.0.0.1:{port}", f"http://localhost:{port}", table.public_url()):
                return self.respond(403, {"error": "Use the room's own form."})
            try:
                length = int(self.headers.get("Content-Length", "0"))
                if not 0 < length <= 250000:
                    raise ValueError("Invalid form size")
                fields = parse_qs(self.rfile.read(length).decode())
                field = lambda name: fields.get(name, [""])[0]
                if not secrets.compare_digest(field("csrf"), csrf):
                    return self.respond(403, {"error": "Reload the room before submitting."})
                if field("operation") == "join":
                    table.join(role, field("name"))
                elif field("operation") == "chat":
                    if field("task"):
                        table.reply(role + "-lead", int(field("task")), field("text"))
                    else:
                        table.say(role + "-lead", field("text"))
                else:
                    raise ValueError("Unknown form action")
                self.send_response(303)
                self.send_header("Location", self.path)
                self.send_header("Cache-Control", "no-store")
                self.send_header("Content-Length", "0")
                self.end_headers()
            except (ValueError, TypeError) as exc:
                self.respond(400, {"error": str(exc)})

    return Handler


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--state-dir", type=Path, default=DEFAULT_STATE)
    subs = parser.add_subparsers(dest="command", required=True)
    server = subs.add_parser("serve")
    server.add_argument("--port", type=int, default=18791)
    join = subs.add_parser("join")
    join.add_argument("team", choices=TEAMS)
    join.add_argument("--name", required=True)
    join.add_argument("--member")
    wait = subs.add_parser("wait")
    wait.add_argument("agent")
    wait.add_argument("--seconds", type=int, default=25)
    reply = subs.add_parser("reply")
    reply.add_argument("agent")
    reply.add_argument("task", type=int)
    reply.add_argument("--file", default="-")
    say = subs.add_parser("say")
    say.add_argument("agent")
    say.add_argument("--file", default="-")
    send = subs.add_parser("send")
    send.add_argument("--file", default="-")
    send.add_argument("--target", choices=("all", "both", *TEAMS), default="all")
    send.add_argument("--passes", type=int, choices=(1, 2, 3), default=2)
    invite = subs.add_parser("invite")
    invite.add_argument("team", choices=TEAMS)
    leave = subs.add_parser("leave")
    leave.add_argument("agent")
    share = subs.add_parser("links")
    share.add_argument("--base")
    public = subs.add_parser("public-url")
    public.add_argument("url")
    subs.add_parser("status")
    subs.add_parser("stop")
    args = parser.parse_args()
    table = Table(args.state_dir)
    try:
        if args.command == "serve":
            server = ThreadingHTTPServer(("127.0.0.1", args.port), handler(table, args.port))
            print(f"Round Table: http://127.0.0.1:{args.port}", flush=True)
            server.serve_forever()
            return
        if args.command == "invite":
            print(table.invite(args.team))
            return
        if args.command in ("reply", "say", "send"):
            text = sys.stdin.read() if args.file == "-" else Path(args.file).read_text()
        if args.command == "links":
            result = table.links(args.base)
        elif args.command == "public-url":
            table.set_public_url(args.url)
            result = table.links()
        elif args.command == "join":
            result = table.join(args.team, args.name, args.member)
        elif args.command == "wait":
            result = table.wait(args.agent, args.seconds)
        elif args.command == "reply":
            result = table.reply(args.agent, args.task, text)
        elif args.command == "say":
            result = table.say(args.agent, text)
        elif args.command == "send":
            result = table.send(text, args.target, args.passes)
        elif args.command == "leave":
            result = table.leave(args.agent)
        elif args.command == "stop":
            result = table.stop()
        else:
            result = table.state()
        print(json.dumps(result, ensure_ascii=False, indent=2))
    except (ValueError, OSError, sqlite3.Error) as exc:
        print(json.dumps({"error": str(exc)}), file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
