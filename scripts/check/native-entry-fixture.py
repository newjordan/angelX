#!/usr/bin/env python3
"""Private native CLI evidence: no credentials, live provider or tool turn.

The native image is supplied by the fast gate, never resolved from installed PATH.
Only offline practice and local loopback fixture replies are allowed; ordinary
metadata endpoints are redirected to an unready fixture.
All terminal bytes stay in memory. Evidence prints binary hash and counts only.
"""
import fcntl
import hashlib
import http.server
import json
import os
from pathlib import Path
import pty
import select
import shutil
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time

class Metadata(http.server.BaseHTTPRequestHandler):
    posts = 0
    def do_GET(self):
        self.send_response(503)
        self.end_headers()
    def do_POST(self):
        Metadata.posts += 1
        self.send_response(500)
        self.end_headers()
    def log_message(self, *_):
        pass


class Api(http.server.BaseHTTPRequestHandler):
    bodies = []
    def do_GET(self):
        body = json.dumps({'data': [{'id': 'fixture-exact', 'context_length': 1000000,
            'supported_parameters': ['reasoning_effort', 'tools']}]}).encode()
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def do_POST(self):
        Api.bodies.append(json.loads(self.rfile.read(int(self.headers['Content-Length']))))
        body = b'data: {"choices":[{"delta":{"content":"fixture-ok"},"finish_reason":null}]}\n\ndata: [DONE]\n\n'
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def log_message(self, *_):
        pass


def main():
    source = Path(sys.argv[1]).resolve(strict=True)
    parent = Path(os.environ['TMPDIR'])
    with tempfile.TemporaryDirectory(prefix='native-entry-image-', dir=parent) as temporary:
        root = Path(temporary)
        image = root / 'angel'
        shutil.copy2(source, image)
        digest = hashlib.sha256(image.read_bytes()).hexdigest()
        server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Metadata)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        api = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Api)
        threading.Thread(target=api.serve_forever, daemon=True).start()
        try:
            def env_for(name):
                case = root / name
                case.mkdir()
                env = {key: os.environ[key] for key in ('PATH', 'CARGO_HOME', 'RUSTUP_HOME') if key in os.environ}
                for key, directory in [('HOME', 'home'), ('CODEX_HOME', 'codex'),
                    ('XDG_CONFIG_HOME', 'config'), ('XDG_DATA_HOME', 'data'),
                    ('XDG_STATE_HOME', 'state'), ('XDG_CACHE_HOME', 'cache'),
                    ('TMPDIR', 'tmp'), ('ANGEL_SESSION_DIR', 'sessions')]:
                    path = case / directory
                    path.mkdir()
                    env[key] = str(path)
                workspace = case / 'workspace'
                workspace.mkdir()
                env.update(USER='fixture', LOGNAME='fixture', CARGO_NET_OFFLINE='true',
                    LANG='C.UTF-8', TERM='xterm-256color', ANGEL_API_CLUBS='none',
                    ANGEL_LOCAL_URL=f'http://127.0.0.1:{server.server_port}', ANGEL_PROBE='0', ANGEL_BAG_PROBE='0',
                    ANGEL_HYDRA_DISCOVER='0', ANGEL_TAILNET_RESOLVE='0', ANGEL_LSP='0',
                    ANGEL_ATLAS='0', ANGEL_BACKPLANE='off', ANGEL_IMAGE_PROTOCOL='halfblocks',
                    ANGEL_YOLO='0', ANGEL_YOLO_SMART='0', ANGEL_DRIVER='practice',
                    ANGEL_WORKSPACE=str(case))
                return env, workspace

            env, workspace = env_for('error')
            result = subprocess.run([image, '--workspace', str(workspace), '--driver', 'openai',
                '--model', 'private-fixture-ID', '--prompt', 'private-fixture-text'], env=env,
                cwd=root, capture_output=True, timeout=30)
            assert result.returncode != 0 and b'\x1b' not in result.stdout
            assert b'private-fixture' not in result.stdout + result.stderr
            assert not list(Path(env['ANGEL_SESSION_DIR']).iterdir())

            def tui(name, option, text, state_error=False, api_turn=False):
                env, workspace = env_for(name)
                extra = ['--driver', 'practice']
                if state_error:
                    blocker = Path(env['HOME']).parent / 'session-blocker'
                    blocker.write_text('fixture')
                    env['ANGEL_SESSION_DIR'] = str(blocker)
                if api_turn:
                    env.update(ANGEL_API_CLUBS='openai', ANGEL_OPENAI_KEY='fixture-only',
                        ANGEL_OPENAI_API_URL=f'http://127.0.0.1:{api.server_port}/v1',
                        ANGEL_OPENAI_API_TRANSPORT='chat', ANGEL_OPENAI_API_MODEL='wrong-ambient-model',
                        ANGEL_OPENAI_API_REASONING_EFFORT='high', ANGEL_REASONING_EFFORT='high',
                        ANGEL_NO_AUTOCOMPACT='1')
                    memory = Path(env['HOME']).parent / 'route-memory.json'
                    memory.write_text(json.dumps({'v': 1, 'agent': 'sota', 'driver': 'openai-api',
                        'model': 'fixture-exact', 'reasoning_effort': 'high'}))
                    env['ANGEL_ROUTE_MEMORY_FILE'] = str(memory)
                    extra = ['--driver', 'openai-api', '--model', 'fixture-exact', '--effort', 'low']
                master, slave = pty.openpty()
                fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 40, 120, 0, 0))
                def terminal_child():
                    os.setsid()
                    fcntl.ioctl(0, termios.TIOCSCTTY, 0)
                proc = subprocess.Popen([image, '--workspace', str(workspace), *extra, option, text],
                    env=env, cwd=root, stdin=slave, stdout=slave, stderr=slave, preexec_fn=terminal_child)
                os.close(slave)
                output = bytearray()
                def pump(seconds):
                    until = time.monotonic() + seconds
                    while time.monotonic() < until:
                        if select.select([master], [], [], min(.1, max(0, until-time.monotonic())))[0]:
                            try:
                                chunk = os.read(master, 65536)
                                output.extend(chunk)
                                if b'\x1b[6n' in chunk:
                                    os.write(master, b'\x1b[1;1R')
                            except OSError:
                                return
                try:
                    deadline = time.monotonic() + 25
                    while b'RUN: entering event loop' not in output and time.monotonic() < deadline and proc.poll() is None:
                        pump(.2)
                    pump(.2)
                    if state_error:
                        assert proc.poll() not in (None, 0), 'failed startup state must reject'
                        assert b'initial turn startup session state unavailable' in output
                        assert text.encode() not in output
                        assert blocker.read_text() == 'fixture'
                        assert b'\x1b[?1049l' in output, 'terminal teardown missing on startup failure'
                        return
                    causes = [cause.decode() for cause in [b'requested launch route', b'No such file', b'No such device', b'Operation not permitted', b'Input/output error', b'Bad file descriptor', b'resource', b'machine mode', b'GRAPHICS:', b'InvalidInput', b'cursor position'] if cause in output]
                    assert b'RUN: entering event loop' in output, f'native TUI did not start (exit={proc.poll()}, diagnostics={causes})'
                    assert os.readlink(f'/proc/{proc.pid}/cwd') == str(workspace.resolve()), 'process cwd/workspace mismatch'
                    pump(2)
                    if option == '--draft':
                        assert b'practice: you said:' not in output, 'draft started a reply'
                        # Ordinary fixture exit; initial text itself is never PTY-typed.
                        os.write(master, b'\x15')
                    else:
                        deadline = time.monotonic() + 15
                        replied = False
                        roles = []
                        while time.monotonic() < deadline and proc.poll() is None:
                            snapshots = [json.loads(path.read_text()) for path in Path(env['ANGEL_SESSION_DIR']).glob('*.json')]
                            history = snapshots[0]['history'] if snapshots else []
                            roles = [entry['role'] for entry in history]
                            replied = any(entry['role'].lower() == 'assistant' and entry['content'].startswith('fixture-ok' if api_turn else 'practice: you said:') for entry in history)
                            if replied:
                                break
                            pump(.2)
                        assert replied, f'offline native reply missing (roles={roles}, exit={proc.poll()})'
                    os.write(master, b'/quit\r')
                    deadline = time.monotonic() + 15
                    while proc.poll() is None and time.monotonic() < deadline:
                        pump(.1)
                    assert proc.poll() == 0, 'native fixture did not exit cleanly'
                    snapshots = [json.loads(path.read_text()) for path in Path(env['ANGEL_SESSION_DIR']).glob('*.json')]
                    assert len(snapshots) == 1, 'one invocation must bind one session'
                    history = snapshots[0]['history']
                    users = [entry['content'] for entry in history if entry['role'].lower() == 'user']
                    assert users == ([] if option == '--draft' else [text]), 'literal enqueue count/text mismatch'
                    assert snapshots[0]['workspace'] == str(workspace.resolve()), 'session workspace mismatch'
                    assert b'\x1b[?1049l' in output, 'terminal teardown missing'
                finally:
                    if proc.poll() is None:
                        proc.terminate()
                        try:
                            proc.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            proc.kill()
                            proc.wait()
                    os.close(master)
            tui('state-error', '--prompt', 'private-state-fixture', state_error=True)
            tui('draft', '--draft', '  /yolo on\nα --doctor  ')
            tui('prompt', '--prompt', '  /quit\nα --doctor  ')
            tui('opaque-prompt', '--prompt', '--doctor')
            literal = '  /yolo on\nultrathink --doctor α  '
            tui('api-prompt', '--prompt', literal, api_turn=True)
            assert len(Api.bodies) == 1, 'one local fixture request expected'
            assert Api.bodies[0]['model'] == 'fixture-exact', 'CLI exact model lost'
            assert Api.bodies[0]['reasoning_effort'] == 'low', 'CLI effort lost to env/ultrathink'
            users = [message['content'] for message in Api.bodies[0]['messages'] if message['role'] == 'user']
            assert users.count(literal) == 1, 'native wire must contain one exact literal user message'
            assert Metadata.posts == 0, 'fixture attempted a non-fixture HTTP turn'
            print(f'native entry fixture: 6 passed; binary_sha256={digest}; local_api_posts=1; other_posts=0')
        finally:
            server.shutdown()
            server.server_close()
            api.shutdown()
            api.server_close()

if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        # Do not print traceback source lines containing fixture prompt literals.
        print(f'native entry fixture failed: {type(error).__name__}: {error}', file=sys.stderr)
        sys.exit(1)
