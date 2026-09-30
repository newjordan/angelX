import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import test from 'node:test'

const source = fileURLToPath(new URL('../../scripts/bench/book-harness/', import.meta.url))

function runnerEnv(parent, overrides = {}) {
  const env = { ...process.env, TMPDIR: parent, STUB_CYCLES: '0', ...overrides }
  delete env.STUB_SCRIPT
  delete env.STUB_PROMPTS
  return env
}

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'angel-stub-runner-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const parent = join(root, 'caller temp')
  mkdirSync(parent)
  writeFileSync(join(parent, 'retained'), 'caller-owned')
  for (const name of [
    'run_one.sh',
    'run_ts.sh',
    'run_phase.sh',
    'stub_model.py',
    'ts.py',
    'exec_session.py',
    'lifecycle.sh',
    'task_run.sh',
    'seat.sh',
    'proxy_log.py',
    'decide_capture.sh',
  ]) {
    copyFileSync(join(source, name), join(root, name))
  }
  mkdirSync(join(root, 'decide'))
  copyFileSync(join(source, 'decide/scenarios.json'), join(root, 'decide/scenarios.json'))
  const binary = join(root, 'angel-fixture')
  writeFileSync(
    binary,
    `#!/usr/bin/env python3
import json, os, sys, urllib.request
url = os.environ['ANGEL_LOCAL_URL']
request = urllib.request.Request(url + '/chat/completions', data=json.dumps({'model': 'stub', 'messages': []}).encode(), headers={'Content-Type': 'application/json'})
with urllib.request.urlopen(request, timeout=2) as response:
    response.read()
print(json.dumps({'home': os.environ['HOME'], 'temporary': os.environ['TMPDIR'], 'url': url, 'pid': os.getpid(), 'group': os.getpgrp()}))
print('fixture stderr complete', file=sys.stderr, flush=True)
raise SystemExit(int(os.environ.get('FIXTURE_EXIT', '0')))
`,
    { mode: 0o755 },
  )
  return { root, parent, binary }
}

for (const name of ['run_one.sh', 'run_ts.sh', 'run_phase.sh']) {
  test(`${name} owns its fixtures and ephemeral stub server`, (t) => {
    const { root, parent, binary } = fixture(t)
    const result = spawnSync(
      'bash',
      [
        ...(name === 'run_one.sh' ? ['-m'] : []),
        join(root, name),
        binary,
        'fixture-helper',
        'smoke',
        'plain',
      ],
      {
        cwd: root,
        encoding: 'utf8',
        timeout: 10_000,
        env: runnerEnv(parent),
      },
    )
    assert.equal(result.status, 0, result.stderr)
    assert.match(result.stdout, /rc=0 .*requests=1/)
    const receipt = JSON.parse(readFileSync(join(root, 'out/smoke.result.json'), 'utf8'))
    assert.equal(
      receipt.group,
      receipt.pid,
      'the task must own its recorded group with or without job control',
    )
    assert.ok(receipt.temporary.startsWith(join(parent, 'angel-bench-run-')))
    assert.equal(existsSync(receipt.temporary), false)
    assert.equal(existsSync(receipt.home), false)
    assert.deepEqual(readdirSync(parent), ['retained'])
    assert.match(readFileSync(join(root, 'out/smoke.stderr'), 'utf8'), /fixture stderr complete/)
    const probe = spawnSync(
      'python3',
      [
        '-c',
        `import urllib.request; urllib.request.urlopen(${JSON.stringify(receipt.url + '/models')}, timeout=1)`,
      ],
      { encoding: 'utf8', timeout: 3000 },
    )
    assert.notEqual(probe.status, 0, 'the owned stub must be stopped')
  })
}

test('decision capture owns its private scenario and ephemeral stub', (t) => {
  const { root, parent, binary } = fixture(t)
  const bindir = join(root, 'bin')
  mkdirSync(bindir)
  copyFileSync(binary, join(bindir, 'angel'))
  const result = spawnSync(
    'bash',
    ['-m', join(root, 'decide_capture.sh'), bindir, 'smoke', 'batch'],
    {
      cwd: root,
      encoding: 'utf8',
      timeout: 10_000,
      env: runnerEnv(parent),
    },
  )
  assert.equal(result.status, 0, result.stderr)
  assert.match(result.stdout, /rc=0 requests=1/)
  const receipt = JSON.parse(readFileSync(join(root, 'out/smoke.result.json'), 'utf8'))
  assert.equal(receipt.pid, receipt.group)
  assert.ok(receipt.temporary.startsWith(join(parent, 'angel-decide-run-')))
  assert.equal(existsSync(receipt.temporary), false)
  assert.equal(existsSync(receipt.home), false)
  assert.deepEqual(readdirSync(parent), ['retained'])
  const probe = spawnSync(
    'python3',
    [
      '-c',
      `import urllib.request; urllib.request.urlopen(${JSON.stringify(receipt.url + '/models')}, timeout=1)`,
    ],
    { encoding: 'utf8', timeout: 3000 },
  )
  assert.notEqual(probe.status, 0)
})

for (const scenario of ['missing', 'batch2']) {
  test(`decision capture rejects unusable scenario ${scenario} and cleans its fixtures`, (t) => {
    const { root, parent } = fixture(t)
    const result = spawnSync('bash', [join(root, 'decide_capture.sh'), root, 'bad', scenario], {
      encoding: 'utf8',
      timeout: 10_000,
      env: runnerEnv(parent),
    })
    assert.equal(result.status, 2, result.stderr)
    assert.equal(existsSync(join(root, 'out/bad.result.json')), false)
    assert.deepEqual(readdirSync(parent), ['retained'])
  })
}

for (const startupFailure of [false, true]) {
  test(`decision capture preserves ${startupFailure ? 'stub startup' : 'task'} failure`, (t) => {
    const { root, parent } = fixture(t)
    const binary = join(root, 'angel')
    writeFileSync(binary, '#!/bin/sh\nexit 17\n', { mode: 0o755 })
    const result = spawnSync('bash', [join(root, 'decide_capture.sh'), root, 'failure', 'batch'], {
      encoding: 'utf8',
      timeout: 10_000,
      env: runnerEnv(parent, startupFailure ? { STUB_CYCLES: 'invalid' } : {}),
    })
    assert.equal(result.status, startupFailure ? 2 : 17, result.stderr)
    if (startupFailure) {
      assert.match(result.stderr, /Stub failed to start/)
      assert.equal(existsSync(join(root, 'out/failure.result.json')), false)
    } else {
      assert.match(result.stdout, /rc=17 requests=unknown/)
    }
    assert.deepEqual(readdirSync(parent), ['retained'])
  })
}

test('stub runner reports startup failure without invoking the binary or leaking fixtures', (t) => {
  const { root, parent, binary } = fixture(t)
  const result = spawnSync(
    'bash',
    [join(root, 'run_one.sh'), binary, 'fixture-helper', 'bad', 'plain'],
    {
      cwd: root,
      encoding: 'utf8',
      timeout: 10_000,
      env: runnerEnv(parent, { STUB_CYCLES: 'invalid' }),
    },
  )
  assert.equal(result.status, 2, result.stderr)
  assert.match(result.stderr, /Stub failed to start/)
  assert.equal(existsSync(join(root, 'out/bad.result.json')), false)
  assert.deepEqual(readdirSync(parent), ['retained'])
})

for (const name of [
  'run_one.sh',
  'run_ts.sh',
  'run_phase.sh',
  'task_run.sh',
  'decide_capture.sh',
]) {
  for (const signal of ['SIGTERM', 'SIGINT', 'exit']) {
    const event = signal === 'exit' ? 'its leader exits' : `PID-only ${signal}`
    test(`${name} cleans its task group after ${event}`, (t) => {
      const { root, parent, binary } = fixture(t)
      writeFileSync(
        binary,
        `#!/usr/bin/env python3
import json, os, signal, sys, time
ready = os.path.join(os.environ['HOME'], 'child.ready')
child = os.fork()
if child == 0:
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    with open(ready, 'w') as receipt:
        receipt.write('ready')
    while True:
        time.sleep(1)
while not os.path.exists(ready):
    time.sleep(.01)
print('cancellation fixture ready', file=sys.stderr, flush=True)
print(json.dumps({'pid': os.getpid(), 'child': child, 'home': os.environ['HOME'], 'temporary': os.environ['TMPDIR'], 'url': os.environ['ANGEL_LOCAL_URL']}), flush=True)
${signal === 'exit' ? 'raise SystemExit(0)' : ''}
signal.signal(signal.SIGTERM, signal.SIG_IGN)
while True:
    time.sleep(1)
`,
        { mode: 0o755 },
      )
      const result = spawnSync(
        'python3',
        [
          '-c',
          `import ctypes, json, os, pathlib, shutil, signal, subprocess, sys, time, urllib.request
if sys.platform == 'linux':
    # Reap the fixture descendants here rather than leaving zombies to PID 1.
    assert ctypes.CDLL(None).prctl(36, 1, 0, 0, 0) == 0
root, binary, parent, script, requested = sys.argv[1:]
env = dict(os.environ, TMPDIR=parent, STUB_CYCLES='0', ROUTE='local', PROXY_UPSTREAM='http://127.0.0.1:9/v1', PROXY_KEY='', PROXY_KEY_CMD='', ANGEL_LOCAL_MODEL='fixture')
env.pop('STUB_SCRIPT', None)
env.pop('STUB_PROMPTS', None)
args = [binary, 'fixture-helper', 'cancel', 'plain']
if script in ('task_run.sh', 'decide_capture.sh'):
    bindir = pathlib.Path(root) / 'bin'
    bindir.mkdir()
    shutil.copy2(binary, bindir / 'angel')
    args = [str(bindir), 'cancel', 'onebug' if script == 'task_run.sh' else 'batch']
runner = subprocess.Popen(['bash', str(pathlib.Path(root) / script), *args], cwd=root, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
receipt = None
try:
    output = pathlib.Path(root) / 'out/cancel.result.json'
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        try:
            receipt = json.loads(output.read_text())
            break
        except (OSError, ValueError):
            if runner.poll() is not None:
                raise AssertionError(runner.communicate())
            time.sleep(.01)
    assert receipt, 'fixture did not start'
    if requested != 'exit':
        os.kill(runner.pid, getattr(signal, requested))
    stdout, stderr = runner.communicate(timeout=4)
    expected = {'SIGTERM': 143, 'SIGINT': 130, 'exit': 0}[requested]
    assert runner.returncode == expected, (runner.returncode, stdout, stderr)
    assert not pathlib.Path(receipt['temporary']).exists(), receipt
    assert sorted(os.listdir(parent)) == ['retained']
    if sys.platform == 'linux':
        pid, status = os.waitpid(receipt['child'], 0)
        assert pid == receipt['child'] and os.WIFSIGNALED(status)
    for pid in (receipt['pid'], receipt['child']):
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            pass
        else:
            raise AssertionError(f'fixture process {pid} survived')
    try:
        urllib.request.urlopen(receipt['url'] + '/models', timeout=1)
    except OSError:
        pass
    else:
        raise AssertionError('stub survived runner cancellation')
    assert 'cancellation fixture ready' in (pathlib.Path(root) / 'out/cancel.stderr').read_text()
finally:
    for group in (runner.pid, receipt['pid'] if receipt else None):
        if group:
            try:
                os.killpg(group, signal.SIGKILL)
            except ProcessLookupError:
                pass
    runner.wait(timeout=3)
    if sys.platform == 'linux':
        while True:
            try:
                os.waitpid(-1, 0)
            except ChildProcessError:
                break
`,
          root,
          binary,
          parent,
          name,
          signal,
        ],
        { encoding: 'utf8', timeout: 15_000 },
      )
      assert.equal(result.status, 0, result.stderr)
    })
  }
}

test('stub runner preserves a failing binary exit status and cleans its fixtures', (t) => {
  const { root, parent, binary } = fixture(t)
  writeFileSync(binary, '#!/bin/sh\nexit 17\n', { mode: 0o755 })
  const result = spawnSync(
    'bash',
    [join(root, 'run_one.sh'), binary, 'fixture-helper', 'failure', 'plain'],
    {
      cwd: root,
      encoding: 'utf8',
      timeout: 10_000,
      env: runnerEnv(parent),
    },
  )
  assert.equal(result.status, 17, result.stderr)
  assert.match(result.stdout, /rc=17 .*requests=unknown/)
  assert.deepEqual(readdirSync(parent), ['retained'])
})

test('stub runner rejects labels that could escape the output directory', (t) => {
  const { root, parent, binary } = fixture(t)
  const result = spawnSync(
    'bash',
    [join(root, 'run_one.sh'), binary, 'fixture-helper', '../escape', 'plain'],
    {
      cwd: root,
      encoding: 'utf8',
      timeout: 10_000,
      env: runnerEnv(parent),
    },
  )
  assert.equal(result.status, 2, result.stderr)
  assert.match(result.stderr, /Invalid benchmark label/)
  assert.equal(existsSync(join(root, 'out')), false)
  assert.deepEqual(readdirSync(parent), ['retained'])
})

for (const outcome of ['success', 'failure', 'unknown-route', 'proxy-startup-failure']) {
  test(`task_run.sh owns its loopback proxy and reports ${outcome}`, (t) => {
    const { root, parent, binary } = fixture(t)
    const bindir = join(root, 'bin')
    mkdirSync(bindir)
    writeFileSync(
      join(bindir, 'angel'),
      readFileSync(binary, 'utf8').replace(
        "int(os.environ.get('FIXTURE_EXIT', '0'))",
        outcome === 'failure' ? '17' : '0',
      ),
      { mode: 0o755 },
    )
    const result = spawnSync(
      'python3',
      [
        '-c',
        `import json, os, pathlib, subprocess, sys, threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
root, bindir, parent, outcome = sys.argv[1:]
requests = []
class Upstream(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass
    def do_POST(self):
        requests.append(json.loads(self.rfile.read(int(self.headers['Content-Length']))))
        body = b'{"choices": [], "usage": {"prompt_tokens": 1, "completion_tokens": 0}}'
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)
server = ThreadingHTTPServer(('127.0.0.1', 0), Upstream)
thread = threading.Thread(target=server.serve_forever)
thread.start()
# An inherited PID is caller data, not an owned proxy handle.
innocent = subprocess.Popen(['sleep', '30'])
env = dict(os.environ, TMPDIR=parent, ROUTE='local', PROXY_UPSTREAM=f'http://127.0.0.1:{server.server_port}/v1', PROXY_KEY='', PROXY_KEY_CMD='', ANGEL_LOCAL_MODEL='fixture', PROXY=str(innocent.pid))
if outcome == 'unknown-route':
    env['ROUTE'] = 'unknown-fixture-route'
if outcome == 'proxy-startup-failure':
    env.pop('PROXY_UPSTREAM')
try:
    result = subprocess.run(['bash', str(pathlib.Path(root) / 'task_run.sh'), bindir, 'proxy', 'onebug'], cwd=root, env=env, capture_output=True, text=True, timeout=8)
    assert innocent.poll() is None, 'cleanup killed an unrelated inherited PID'
    print(json.dumps({'status': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr, 'requests': len(requests)}))
finally:
    innocent.terminate()
    innocent.wait(timeout=2)
    server.shutdown()
    server.server_close()
    thread.join(timeout=2)
`,
        root,
        bindir,
        parent,
        outcome,
      ],
      { encoding: 'utf8', timeout: 12_000 },
    )
    assert.equal(result.status, 0, result.stderr)
    const report = JSON.parse(result.stdout)
    const expected = { success: 0, failure: 17, 'unknown-route': 2, 'proxy-startup-failure': 2 }[
      outcome
    ]
    assert.equal(report.status, expected, report.stderr)
    assert.deepEqual(readdirSync(parent), ['retained'])
    if (outcome === 'success' || outcome === 'failure') {
      assert.match(report.stdout, new RegExp(`rc=${expected} solved=no .*requests=1`))
      assert.equal(report.requests, 1)
      const receipt = JSON.parse(readFileSync(join(root, 'out/proxy.result.json'), 'utf8'))
      assert.equal(existsSync(receipt.temporary), false)
      assert.match(readFileSync(join(root, 'out/proxy.stderr'), 'utf8'), /fixture stderr complete/)
      const wire = JSON.parse(readFileSync(join(root, 'out/proxy.jsonl'), 'utf8'))
      assert.equal(wire.status, 200)
      assert.equal(wire.body.model, 'stub')
      assert.deepEqual(wire.usage, { prompt_tokens: 1, completion_tokens: 0 })
      const probe = spawnSync(
        'python3',
        [
          '-c',
          `import urllib.request; urllib.request.urlopen(${JSON.stringify(receipt.url + '/models')}, timeout=1)`,
        ],
        { encoding: 'utf8', timeout: 3000 },
      )
      assert.notEqual(probe.status, 0, 'the owned proxy must be stopped')
    } else {
      assert.equal(existsSync(join(root, 'out/proxy.result.json')), false)
      assert.equal(report.requests, 0)
      assert.match(report.stderr, /unknown ROUTE|proxy did not become ready/)
    }
  })
}

for (const event of ['deadline', 'SIGTERM', 'SIGINT']) {
  test(`task_run.sh stops a hanging verifier and its child on ${event}`, (t) => {
    const { root, parent } = fixture(t)
    const bindir = join(root, 'bin')
    mkdirSync(bindir)
    writeFileSync(
      join(bindir, 'angel'),
      `#!/usr/bin/env python3
import json, os, pathlib, sys
workspace = pathlib.Path(sys.argv[sys.argv.index('--workspace') + 1])
workspace.joinpath('test_calc.py').write_text('''import json, os, pathlib, signal, time
child_ready = pathlib.Path(__file__).with_name('child.ready')
child = os.fork()
if child == 0:
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    child_ready.write_text('ready')
    while True:
        time.sleep(1)
while not child_ready.exists():
    time.sleep(.01)
pathlib.Path(__file__).with_name('verification.ready').write_text(json.dumps({'pid': os.getpid(), 'child': child, 'group': os.getpgrp()}))
while True:
    time.sleep(1)
''')
print(json.dumps({'temporary': os.environ['TMPDIR']}), flush=True)
`,
      { mode: 0o755 },
    )
    const result = spawnSync(
      'python3',
      [
        '-c',
        `import ctypes, json, os, pathlib, signal, subprocess, sys, time
if sys.platform == 'linux':
    assert ctypes.CDLL(None).prctl(36, 1, 0, 0, 0) == 0
root, bindir, parent, event = sys.argv[1:]
env = dict(os.environ, TMPDIR=parent, ROUTE='local', PROXY_UPSTREAM='http://127.0.0.1:9/v1', PROXY_KEY='', PROXY_KEY_CMD='', ANGEL_LOCAL_MODEL='fixture', VERIFY_TIMEOUT_SECS='.4' if event == 'deadline' else '30')
runner = subprocess.Popen(['bash', str(pathlib.Path(root) / 'task_run.sh'), bindir, 'verify', 'onebug'], cwd=root, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
receipt = None
temporary = None
try:
    deadline = time.monotonic() + 4
    while time.monotonic() < deadline:
        try:
            temporary = json.loads((pathlib.Path(root) / 'out/verify.result.json').read_text())['temporary']
            receipt = json.loads((pathlib.Path(temporary) / 'work/verification.ready').read_text())
            break
        except (OSError, ValueError):
            time.sleep(.01)
    assert receipt, 'hanging verifier did not start'
    if event != 'deadline':
        os.kill(runner.pid, getattr(signal, event))
    stdout, stderr = runner.communicate(timeout=4)
    assert runner.returncode == {'deadline': 0, 'SIGTERM': 143, 'SIGINT': 130}[event], (stdout, stderr)
    if event == 'deadline':
        assert 'solved=no' in stdout and 'verification_rc=137' in stdout, stdout
    assert not pathlib.Path(temporary).exists()
    assert sorted(os.listdir(parent)) == ['retained']
    if sys.platform == 'linux':
        for pid in (receipt['pid'], receipt['child']):
            assert os.waitpid(pid, 0)[0] == pid
    for pid in (receipt['pid'], receipt['child']):
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            pass
        else:
            raise AssertionError(f'verifier process {pid} survived')
finally:
    for group in (runner.pid, receipt['group'] if receipt else None):
        if group:
            try:
                os.killpg(group, signal.SIGKILL)
            except ProcessLookupError:
                pass
    runner.wait(timeout=3)
    if sys.platform == 'linux':
        while True:
            try:
                os.waitpid(-1, 0)
            except ChildProcessError:
                break
`,
        root,
        bindir,
        parent,
        event,
      ],
      { encoding: 'utf8', timeout: 12_000 },
    )
    assert.equal(result.status, 0, result.stderr)
  })
}
