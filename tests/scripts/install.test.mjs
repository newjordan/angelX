import assert from 'node:assert/strict'
import { spawn, spawnSync } from 'node:child_process'
import {
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  readlinkSync,
  realpathSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import { createServer } from 'node:http'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import test from 'node:test'
import { RELEASE_PATHS, REQUIRED_RELEASE_FILES } from '../../scripts/release/release-evidence.mjs'

const root = fileURLToPath(new URL('../..', import.meta.url))
const installer = join(root, 'install.sh')
const setup = join(root, 'scripts/runtime/angel-setup.sh')
const platform = {
  'linux/x64': 'linux-x86_64',
  'linux/arm64': 'linux-arm64',
  'darwin/arm64': 'macos-arm64',
}[`${process.platform}/${process.arch}`]

function scratch(t) {
  const dir = mkdtempSync(join(tmpdir(), 'angelx-install-'))
  t.after(() => rmSync(dir, { recursive: true, force: true }))
  return dir
}

function write(path, text, mode = 0o644) {
  mkdirSync(join(path, '..'), { recursive: true })
  writeFileSync(path, text, { mode })
}

// A package shaped like package-prebuilt.sh output, with stub programs.
function fakePackage(dir, version, { runs = true, prebuilt = true } = {}) {
  const top = join(dir, 'pkg', `angelX-${version}-${platform}`)
  write(join(top, 'bin/angelX'), '#!/bin/sh\necho launcher\n', 0o755)
  write(
    join(top, 'cockpit/target/release/angel'),
    runs ? `#!/bin/sh\necho "angelX-cockpit ${version}"\n` : '#!/bin/sh\nexit 1\n',
    0o755,
  )
  if (prebuilt) write(join(top, 'PREBUILT.json'), `{\n  "version": "${version}"\n}\n`)
  const archive = join(dir, `angelX-${version}.tar.gz`)
  const tar = spawnSync('tar', ['-czf', archive, '-C', join(dir, 'pkg'), `angelX-${version}-${platform}`])
  assert.equal(tar.status, 0, String(tar.stderr))
  rmSync(join(dir, 'pkg'), { recursive: true })
  return archive
}

function install(home, archive) {
  return spawnSync('sh', [installer], {
    env: { HOME: home, PATH: process.env.PATH, ANGELX_ARCHIVE: archive, SHELL: '/bin/bash' },
    encoding: 'utf8',
  })
}

test('the installer, setup and packaging ship in release archives', () => {
  for (const path of [
    'install.sh',
    'scripts/runtime/angel-setup.sh',
    'scripts/release/build-prebuilt.sh',
    'scripts/release/package-prebuilt.sh',
    'tests/scripts/install.test.mjs',
  ]) {
    assert.ok(RELEASE_PATHS.includes(path), path)
    assert.ok(REQUIRED_RELEASE_FILES.includes(path), path)
  }
})

test('install.sh installs a package and links the angelX command', { skip: !platform }, (t) => {
  const dir = scratch(t)
  const home = join(dir, 'home')
  mkdirSync(home)
  const run = install(home, fakePackage(dir, '9.9.1'))
  assert.equal(run.status, 0, run.stderr)
  assert.match(run.stdout, /angelX 9\.9\.1 is installed/)
  assert.equal(readlinkSync(join(home, '.angelX/current')), `releases/9.9.1-${platform}`)
  assert.equal(
    realpathSync(join(home, '.local/bin/angelX')),
    realpathSync(join(home, `.angelX/releases/9.9.1-${platform}/bin/angelX`)),
  )
  assert.ok(lstatSync(join(home, '.local/bin/angelx')).isSymbolicLink())
})

test('install.sh replaces nothing when the package is wrong or will not run', { skip: !platform }, (t) => {
  const dir = scratch(t)
  const home = join(dir, 'home')
  mkdirSync(home)
  assert.equal(install(home, fakePackage(dir, '9.9.1')).status, 0)
  for (const archive of [
    fakePackage(dir, '9.9.2', { prebuilt: false }),
    fakePackage(dir, '9.9.3', { runs: false }),
  ]) {
    const run = install(home, archive)
    assert.notEqual(run.status, 0)
    assert.equal(readlinkSync(join(home, '.angelX/current')), `releases/9.9.1-${platform}`)
  }
  assert.deepEqual(
    readdirSync(join(home, '.angelX/releases')).filter((name) => !name.startsWith('.')),
    [`9.9.1-${platform}`],
  )
})

test('install.sh upgrades in place and keeps the previous release', { skip: !platform }, (t) => {
  const dir = scratch(t)
  const home = join(dir, 'home')
  mkdirSync(home)
  for (const version of ['9.9.1', '9.9.2', '9.9.3']) {
    assert.equal(install(home, fakePackage(dir, version)).status, 0)
  }
  assert.equal(readlinkSync(join(home, '.angelX/current')), `releases/9.9.3-${platform}`)
  assert.deepEqual(
    readdirSync(join(home, '.angelX/releases'))
      .filter((name) => !name.startsWith('.'))
      .sort(),
    [`9.9.2-${platform}`, `9.9.3-${platform}`],
  )
})

// A stand-in model server: GET /v1/models only.
async function modelServer(t, models) {
  const server = createServer((request, response) => {
    if (request.url === '/v1/models') {
      response.setHeader('Content-Type', 'application/json')
      response.end(JSON.stringify({ object: 'list', data: models.map((id) => ({ id, object: 'model', owned_by: 'vllm' })) }))
    } else {
      response.statusCode = 404
      response.end('{}')
    }
  })
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
  t.after(() => server.close())
  return `127.0.0.1:${server.address().port}`
}

function runSetup(home, answers, env = {}) {
  return new Promise((resolve) => {
    const child = spawn('bash', [setup], {
      // No localhost probing: whatever runs on this machine must not shift the menu.
      env: { HOME: home, PATH: process.env.PATH, ANGEL_SETUP_PORTS: '', ...env },
      stdio: ['pipe', 'pipe', 'pipe'],
    })
    let stdout = ''
    child.stdout.on('data', (chunk) => (stdout += chunk))
    child.stderr.on('data', (chunk) => (stdout += chunk))
    child.on('close', (status) => resolve({ status, stdout }))
    child.stdin.end(answers)
  })
}

test('setup saves a model server typed by address, privately', async (t) => {
  const dir = scratch(t)
  const address = await modelServer(t, ['coder-large', 'coder-small'])
  const run = await runSetup(dir, `a\n${address}\n2\n`)
  assert.equal(run.status, 0, run.stdout)
  const settings = join(dir, '.angelX/angel.env')
  assert.equal(statSync(settings).mode & 0o777, 0o600)
  const text = readFileSync(settings, 'utf8')
  assert.match(text, new RegExp(`^ANGEL_LOCAL_URL='http://${address}/v1'$`, 'm'))
  assert.match(text, /^ANGEL_LOCAL_MODEL='coder-small'$/m)
  assert.match(text, /^ANGEL_DRIVER='local'$/m)
})

test('setup picks a key from the environment and keeps the rest of the file', async (t) => {
  const dir = scratch(t)
  write(join(dir, '.angelX/angel.env'), "FOO='kept'\nANGEL_DRIVER='local'\n", 0o600)
  const run = await runSetup(dir, '1\n', { DEEPSEEK_API_KEY: 'sk-test' })
  assert.equal(run.status, 0, run.stdout)
  assert.match(run.stdout, /DeepSeek, key in \$DEEPSEEK_API_KEY/)
  const text = readFileSync(join(dir, '.angelX/angel.env'), 'utf8')
  assert.match(text, /^FOO='kept'$/m)
  assert.match(text, /^ANGEL_DRIVER='deepseek'$/m)
  assert.doesNotMatch(text, /ANGEL_DRIVER='local'/)
  assert.doesNotMatch(text, /sk-test/)
})

test('setup quotes saved values for the shell that loads them', async (t) => {
  const dir = scratch(t)
  const address = await modelServer(t, ["it's-a-model"])
  const run = await runSetup(dir, `a\n${address}\n`)
  assert.equal(run.status, 0, run.stdout)
  const settings = join(dir, '.angelX/angel.env')
  const loaded = spawnSync('bash', ['-c', `set -a; . "${settings}"; printf %s "$ANGEL_LOCAL_MODEL"`], {
    encoding: 'utf8',
  })
  assert.equal(loaded.stdout, "it's-a-model")
})

test('angelX setup runs the wizard and skipping still marks setup done', (t) => {
  const dir = scratch(t)
  const run = spawnSync('bash', [join(root, 'bin/angelX'), 'setup'], {
    env: { HOME: dir, PATH: process.env.PATH, ANGEL_SETUP_PORTS: '' },
    input: 's\n',
    encoding: 'utf8',
  })
  assert.equal(run.status, 0, run.stderr)
  assert.match(run.stdout, /Skipped/)
  assert.ok(existsSync(join(dir, '.angelX/angel.env')))
})
