import assert from 'node:assert/strict'
import { spawn, spawnSync } from 'node:child_process'
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
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import test from 'node:test'

const runner = fileURLToPath(new URL('../../scripts/check/check-cockpit-fast.sh', import.meta.url))
const copier = fileURLToPath(
  new URL('../../scripts/check/copy-cockpit-test-image.py', import.meta.url),
)

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'angel-fast-gate-'))
  const script = join(root, 'scripts/check/check-cockpit-fast.sh')
  const commands = join(root, 'commands')
  const parent = join(root, 'caller temp')
  const artifacts = join(root, 'shared target')
  const ready = join(root, 'ready')
  const release = join(root, 'release')
  let running
  t.after(async () => {
    if (running) {
      writeFileSync(release, 'release')
      await running
    }
    rmSync(root, { recursive: true, force: true })
  })
  for (const directory of [dirname(script), commands, parent, artifacts, join(root, 'cockpit')])
    mkdirSync(directory, { recursive: true })
  copyFileSync(runner, script)
  copyFileSync(copier, join(dirname(script), 'copy-cockpit-test-image.py'))
  writeFileSync(join(dirname(script), 'check-legacy-terminal-boundary.sh'), 'exit 0\n')
  writeFileSync(join(dirname(script), 'check-active-connections.py'), 'pass\n')
  const binary = join(artifacts, 'angel-tests')
  const cli = join(artifacts, 'angel')
  const helper = join(artifacts, 'angel-sandbox')
  writeFileSync(cli, '#!/usr/bin/env bash\nprintf cli\n', { mode: 0o755 })
  writeFileSync(helper, '#!/usr/bin/env bash\nprintf helper\n', { mode: 0o755 })
  writeFileSync(
    binary,
    `#!/usr/bin/env bash
set -eu
[[ "$ANGEL_YOLO" == 0 && "$ANGEL_YOLO_SMART" == 0 ]]
[[ "$PWD" == "$FIXTURE_ROOT/cockpit" ]]
[[ "$*" == '--test-threads=1' ]]
[[ "$CARGO_BIN_EXE_angel" == "$TMPDIR/image/debug/angel" ]]
[[ "$ANGEL_T_SANDBOX_HELPER" == "$TMPDIR/image/debug/angel-sandbox" ]]
printf '%s\n' "$TMPDIR"
mkdir -p "$TMPDIR/unfinished-fixture/nested"
printf fixture > "$TMPDIR/unfinished-fixture/nested/file"
exit "$FIXTURE_GATE_EXIT"
`,
    { mode: 0o755 },
  )
  const artifact = (name, executable, isTest) =>
    JSON.stringify({
      reason: 'compiler-artifact',
      target: { name },
      profile: { test: isTest },
      executable,
    })
  writeFileSync(
    join(commands, 'cargo'),
    `#!/usr/bin/env bash
set -eu
if [[ "$1" == build ]]; then
  [[ " $* " == *' --bins '* && " $* " == *' --message-format=json '* ]]
  if [[ "\${FIXTURE_BUILD_EXIT:-0}" != 0 ]]; then
    printf '%s\n' '${JSON.stringify({ reason: 'compiler-message', message: { rendered: 'error: fixture compiler failure\n' } })}'
    exit "$FIXTURE_BUILD_EXIT"
  fi
  printf '%s\n' '${artifact('angel', cli, false)}' '${artifact('angel-sandbox', helper, false)}'
else
  [[ "$1" == test ]]
  if [[ " $* " == *' --no-run '* ]]; then
    printf '%s\n' '${artifact('angel', binary, true)}'
  else
    exec "$FIXTURE_ARTIFACT" --test-threads=1
  fi
fi
`,
    { mode: 0o755 },
  )
  writeFileSync(
    join(commands, 'rustup'),
    `#!/usr/bin/env bash
case "$*" in
  'which cargo') printf '%s\n' "$FIXTURE_COMMANDS/cargo" ;;
  'show active-toolchain') printf 'fixture-toolchain (override)\n' ;;
  *) exit 2 ;;
esac
`,
    { mode: 0o755 },
  )
  const env = {
    ...process.env,
    PATH: `${commands}:${process.env.PATH}`,
    TMPDIR: parent,
    ANGEL_T_SANDBOX_HELPER: helper,
    ANGEL_YOLO: '1',
    ANGEL_YOLO_SMART: '1',
    FIXTURE_ROOT: root,
    FIXTURE_COMMANDS: commands,
    FIXTURE_ARTIFACT: binary,
    FIXTURE_READY: ready,
    FIXTURE_RELEASE: release,
    FIXTURE_GATE_EXIT: '0',
  }
  return {
    root,
    script,
    parent,
    artifacts,
    binary,
    cli,
    helper,
    ready,
    release,
    env,
    own(child) {
      running = new Promise((resolve) => child.once('close', resolve))
      return running
    },
  }
}

for (const exitCode of [0, 17]) {
  test(`gate removes only its own temporary fixtures after exit ${exitCode}`, (t) => {
    const f = fixture(t)
    const callerFile = join(f.parent, 'caller-owned')
    writeFileSync(callerFile, 'retain')
    const result = spawnSync('bash', [f.script, '--test-threads=1'], {
      cwd: f.root,
      encoding: 'utf8',
      timeout: 10_000,
      env: { ...f.env, FIXTURE_GATE_EXIT: String(exitCode) },
    })
    assert.equal(result.status, exitCode, result.stderr)
    const ownTemp = result.stdout.trim().split('\n').at(-1)
    assert.ok(ownTemp.startsWith(join(f.parent, 'angelx-tests.')), result.stdout)
    assert.equal(existsSync(ownTemp), false)
    assert.equal(existsSync(callerFile), true)
  })
}

test('compiler failures retain diagnostics and status while removing the owned image directory', (t) => {
  const f = fixture(t)
  const result = spawnSync('bash', [f.script], {
    cwd: f.root,
    encoding: 'utf8',
    timeout: 10_000,
    env: { ...f.env, FIXTURE_BUILD_EXIT: '23' },
  })
  assert.equal(result.status, 23, result.stderr)
  assert.match(result.stderr, /error: fixture compiler failure/)
  assert.deepEqual(readdirSync(f.parent), [])
})

test('a nonregular helper refuses setup without waiting for a pipe writer', (t) => {
  if (process.platform !== 'linux') return t.skip('Linux FIFO fixture')
  const f = fixture(t)
  const fifo = join(f.artifacts, 'helper-fifo')
  const created = spawnSync('mkfifo', [fifo], { encoding: 'utf8' })
  assert.equal(created.status, 0, created.stderr)
  const result = spawnSync('bash', [f.script], {
    cwd: f.root,
    encoding: 'utf8',
    timeout: 5000,
    env: { ...f.env, ANGEL_T_SANDBOX_HELPER: fifo },
  })
  assert.equal(result.status, 2, result.stderr)
  assert.match(result.stderr, /regular executable files/)
  assert.deepEqual(readdirSync(f.parent), [])
})

test('gate test image and CLI/helper survive unlinking the shared build artifacts', async (t) => {
  if (process.platform !== 'linux') return t.skip('Linux executable unlink semantics')
  const f = fixture(t)
  const compiled = spawnSync('cc', ['-x', 'c', '-o', f.binary, '-'], {
    encoding: 'utf8',
    input: `
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
int main(int argc, char **argv) {
  FILE *ready = fopen(getenv("FIXTURE_READY"), "w");
  if (!ready) return 30;
  fprintf(ready, "%s", getenv("TMPDIR")); fclose(ready);
  for (int i = 0; access(getenv("FIXTURE_RELEASE"), F_OK) && i < 4000; i++) usleep(1000);
  char path[4096]; ssize_t n = readlink("/proc/self/exe", path, sizeof(path)-1);
  if (n < 0) return 31; path[n] = 0;
  if (strstr(path, " (deleted)") || access(path, X_OK)) return 19;
  if (access(getenv("CARGO_BIN_EXE_angel"), X_OK) || access(getenv("ANGEL_T_SANDBOX_HELPER"), X_OK)) return 20;
  if (argc != 2 || strcmp(argv[1], "--test-threads=1")) return 21;
  return 0;
}`,
  })
  if (compiled.error?.code === 'ENOENT')
    return t.skip('C compiler is required for an actual executable unlink fixture')
  assert.equal(compiled.status, 0, compiled.stderr)
  const child = spawn('bash', [f.script, '--test-threads=1'], {
    cwd: f.root,
    env: f.env,
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  let stderr = ''
  child.stderr.on('data', (chunk) => {
    stderr += chunk
  })
  child.stdout.resume()
  const closed = f.own(child)
  const deadline = Date.now() + 5000
  while (!existsSync(f.ready) && child.exitCode === null && Date.now() < deadline)
    await new Promise((resolve) => setTimeout(resolve, 10))
  assert.equal(existsSync(f.ready), true, stderr)
  const owned = readFileSync(f.ready, 'utf8')
  for (const artifact of [f.binary, f.cli, f.helper]) rmSync(artifact)
  writeFileSync(f.release, 'release')
  const status = await closed
  assert.equal(status, 0, stderr)
  assert.equal(existsSync(owned), false, 'private image is removed after the process settles')
  assert.deepEqual(readdirSync(f.parent), [])
})
