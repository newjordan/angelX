import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import test from 'node:test'
import { RELEASE_PATHS, REQUIRED_RELEASE_FILES } from '../../scripts/release/release-evidence.mjs'

const policy = fileURLToPath(new URL('../../scripts/check/angel-club-policy.sh', import.meta.url))

test('the launch-time provider policy is shipped in release archives', () => {
  for (const path of [
    'scripts/check/angel-club-policy.sh',
    'tests/scripts/angel-club-policy.test.mjs',
  ]) {
    assert.ok(RELEASE_PATHS.includes(path))
    assert.ok(REQUIRED_RELEASE_FILES.includes(path))
  }
})
const credentials = [
  'ANGEL_OPENAI_KEY',
  'OPENAI_API_KEY',
  'ANGEL_GROK_KEY',
  'ANGEL_XAI_KEY',
  'GROK_API_KEY',
  'XAI_API_KEY',
  'ANGEL_KIMI_KEY',
  'KIMI_API_KEY',
  'MOONSHOT_API_KEY',
  'ANGEL_DEEPSEEK_KEY',
  'DEEPSEEK_API_KEY',
  'ANGEL_GLM_KEY',
  'GLM_API_KEY',
  'ZAI_API_KEY',
  'ZHIPU_API_KEY',
  'BIGMODEL_API_KEY',
  'ANGEL_QWEN_KEY',
  'QWEN_API_KEY',
  'DASHSCOPE_API_KEY',
  'ANGEL_LONGCAT_KEY',
  'LONGCAT_API_KEY',
  'ANGEL_META_KEY',
  'META_API_KEY',
  'ANGEL_CEREBRAS_KEY',
  'CEREBRAS_API_KEY',
  'ANGEL_OPENROUTER_KEY',
  'OPENROUTER_API_KEY',
]

function filtered(extra = {}) {
  const result = spawnSync('bash', ['-c', 'set -eu; source "$1"; env', 'club-policy', policy], {
    encoding: 'utf8',
    env: {
      PATH: process.env.PATH,
      ANGEL_BRAIN_KEY: 'local-test-key',
      ANGEL_OPENAI_MODEL: 'oauth-model',
      ANGEL_GROK_MODEL: 'oauth-grok',
      ...Object.fromEntries(credentials.map((key) => [key, 'test-only-key'])),
      ...extra,
    },
  })
  assert.equal(result.status, 0, result.stderr)
  return Object.fromEntries(
    result.stdout
      .trim()
      .split('\n')
      .map((line) => {
        const separator = line.indexOf('=')
        return [line.slice(0, separator), line.slice(separator + 1)]
      }),
  )
}

test('configured API credentials survive ordinary and no-build launches by default', () => {
  for (const extra of [
    {},
    { ANGEL_NO_BUILD: '1' },
    { ANGEL_API_CLUBS: '*' },
    { ANGEL_API_CLUBS: ' ALL ' },
  ]) {
    const env = filtered(extra)
    for (const key of credentials) assert.equal(env[key], 'test-only-key', key)
    assert.equal(env.ANGEL_BRAIN_KEY, 'local-test-key')
    assert.equal(env.ANGEL_OPENAI_MODEL, 'oauth-model')
    assert.equal(env.ANGEL_GROK_MODEL, 'oauth-grok')
  }
})

test('an operator allowlist enables only its named API providers', () => {
  const env = filtered({ ANGEL_API_CLUBS: ' GLM , kimi ' })
  for (const key of ['ZAI_API_KEY', 'ANGEL_GLM_KEY', 'MOONSHOT_API_KEY']) {
    assert.equal(env[key], 'test-only-key', key)
  }
  for (const key of ['OPENAI_API_KEY', 'XAI_API_KEY', 'OPENROUTER_API_KEY', 'DEEPSEEK_API_KEY']) {
    assert.equal(env[key], undefined, key)
  }
})

test('OAuth and API providers can be selected independently without removing model settings', () => {
  const env = filtered({ ANGEL_API_CLUBS: 'openai,grok' })
  assert.equal(env.OPENAI_API_KEY, 'test-only-key')
  assert.equal(env.XAI_API_KEY, 'test-only-key')
  assert.equal(env.ANGEL_GROK_MODEL, 'oauth-grok')
  assert.equal(env.ANGEL_OPENAI_MODEL, 'oauth-model')
  assert.equal(env.ZAI_API_KEY, undefined)
})

test('empty and none are explicit operator choices to disable API credentials', () => {
  for (const value of ['', ' none ', 'unknown-provider']) {
    const env = filtered({ ANGEL_API_CLUBS: value })
    for (const key of credentials) assert.equal(env[key], undefined, key)
    assert.equal(env.ANGEL_BRAIN_KEY, 'local-test-key')
    assert.equal(env.ANGEL_OPENAI_MODEL, 'oauth-model')
  }
})

test('unrelated tools keep their configuration instead of inheriting a global provider ban', () => {
  const env = filtered({
    ANGEL_API_CLUBS: 'glm',
    HF_TOKEN: 'fixture',
    ANGEL_META_MODEL: 'user-model',
  })
  assert.equal(env.HF_TOKEN, 'fixture')
  assert.equal(env.ANGEL_META_MODEL, 'user-model')
  assert.equal(env.META_API_KEY, undefined, 'the Muse key belongs to the meta club')
})

test('the meta club keeps its Muse credentials when allowed', () => {
  const env = filtered({ ANGEL_API_CLUBS: 'glm,meta' })
  assert.equal(env.ANGEL_META_KEY, 'test-only-key')
  assert.equal(env.META_API_KEY, 'test-only-key')
  assert.equal(env.DEEPSEEK_API_KEY, undefined)
})

test('real launcher loads system.env only on opt-in, preserves API choices, and isolates headless configuration', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'angel-provider-launch-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const home = join(root, 'home')
  const source = join(root, 'source')
  for (const dir of ['bin', 'scripts/check', 'cockpit/target/release'])
    mkdirSync(join(source, dir), { recursive: true })
  mkdirSync(join(home, '.config/host_env'), { recursive: true })
  copyFileSync(
    fileURLToPath(new URL('../../bin/angelX', import.meta.url)),
    join(source, 'bin/angelX'),
  )
  copyFileSync(policy, join(source, 'scripts/check/angel-club-policy.sh'))
  const probe = join(source, 'cockpit/target/release/angel')
  writeFileSync(
    probe,
    `#!/usr/bin/env python3
import json, os
print(json.dumps({"grok": bool(os.environ.get("XAI_API_KEY")),
 "openai": bool(os.environ.get("OPENAI_API_KEY")), "marker": os.environ.get("ANGEL_TEST_CONFIG")}))
`,
  )
  chmodSync(probe, 0o755)
  writeFileSync(
    join(home, '.config/host_env/system.env'),
    'XAI_API_KEY=fixture-xai\nOPENAI_API_KEY=fixture-openai\nANGEL_TEST_CONFIG=system\n',
  )
  const launch = (args = [], extra = {}) => {
    const result = spawnSync('bash', [join(source, 'bin/angelX'), ...args], {
      cwd: root,
      encoding: 'utf8',
      timeout: 15000,
      env: {
        HOME: home,
        PATH: '/usr/bin:/bin',
        LANG: 'C.UTF-8',
        ANGEL_NO_BUILD: '1',
        ANGEL_VIDEO: '0',
        ANGEL_WEBGPU_PORTAL: '0',
        ...extra,
      },
    })
    assert.equal(result.status, 0, result.stderr)
    return JSON.parse(result.stdout)
  }
  // Generic home env files are other tools' shell code: never read by default.
  assert.deepEqual(launch(), { grok: false, openai: false, marker: null })
  const hostEnv = { ANGEL_HOST_ENV: '1' }
  assert.deepEqual(launch([], hostEnv), { grok: true, openai: true, marker: 'system' })
  assert.deepEqual(launch([], { ...hostEnv, ANGEL_API_CLUBS: 'none' }), {
    grok: false,
    openai: false,
    marker: 'system',
  })
  // The opt-in may live in .angel.env, whose values still override host defaults.
  writeFileSync(join(source, '.angel.env'), 'ANGEL_HOST_ENV=1\nANGEL_TEST_CONFIG=project\n')
  assert.deepEqual(launch(), { grok: true, openai: true, marker: 'project' })
  assert.deepEqual(launch(['--build-info', '--json']), { grok: false, openai: false, marker: null })
})

test('launcher preflight preserves opaque values and fails before env/log effects', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'angel-entry-launch-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const home = join(root, 'home')
  const source = join(root, 'source')
  const workspace = join(root, 'workspace')
  for (const dir of ['bin', 'scripts/check', 'cockpit/target/release'])
    mkdirSync(join(source, dir), { recursive: true })
  mkdirSync(home)
  mkdirSync(workspace)
  copyFileSync(fileURLToPath(new URL('../../bin/angelX', import.meta.url)), join(source, 'bin/angelX'))
  copyFileSync(policy, join(source, 'scripts/check/angel-club-policy.sh'))
  const probe = join(source, 'cockpit/target/release/angel')
  writeFileSync(probe, `#!/usr/bin/env python3
import json, os, sys
print(json.dumps({'argv': sys.argv[1:], 'cwd': os.getcwd(), 'marker': os.environ.get('ENTRY_MARKER')}))
`)
  chmodSync(probe, 0o755)
  writeFileSync(join(source, '.angel.env'), 'ENTRY_MARKER=interactive\ntouch "$HOME/env-effect"\n')
  const launch = (args) => spawnSync('bash', [join(source, 'bin/angelX'), ...args], {
    cwd: root, encoding: 'utf8', timeout: 15000,
    env: { HOME: home, PATH: '/usr/bin:/bin', LANG: 'C', ANGEL_NO_BUILD: '1',
      ANGEL_VIDEO: '0', ANGEL_WEBGPU_PORTAL: '0', ANGEL_WORKSPACE: root },
  })
  for (const args of [
    ['--unknown', 'private-fixture'], ['--draft'], ['--draft', '   '],
    ['--draft', 'a', '--draft', 'b'], ['--draft', 'a', '--resume'],
    ['--resume', 'id', 'private-fixture'], ['--doctor', 'private-fixture'],
    ['--workspace', workspace, '--workspace', workspace],
    ['--model', 'private-fixture'], ['--driver'], ['--effort'],
    ['--prompt', 'a', '--draft', 'b'], ['--prompt', 'a', '--resume'],
    ['--draft', '\u00a0\u2003\u3000'], ['--resume', '\u00a0\u2003\u3000'],
    ['--driver', ' x '], ['--effort', '\u2003'],
  ]) {
    const result = launch(args)
    assert.equal(result.status, 2, result.stderr)
    assert.ok(!result.stderr.includes('private-fixture'))
    assert.ok(!existsSync(join(home, 'env-effect')))
    assert.ok(!existsSync(join(home, '.angelX')), 'preflight must precede launch logging')
  }
  // JS cannot represent invalid argv bytes; Python supplies them directly.
  for (const flag of ['--draft', '--prompt', '--resume']) {
    const result = spawnSync('python3', ['-c', `import subprocess,sys
r=subprocess.run([b'bash',sys.argv[1].encode(),sys.argv[2].encode(),b'\\xff'],capture_output=True)
sys.stdout.buffer.write(r.stdout);sys.stderr.buffer.write(r.stderr);sys.exit(r.returncode)`, join(source, 'bin/angelX'), flag], {
      cwd: root, encoding: 'utf8', env: { HOME: home, PATH: '/usr/bin:/bin', LANG: 'C', ANGEL_NO_BUILD: '0' },
    })
    assert.equal(result.status, 2, result.stderr)
    assert.ok(!existsSync(join(home, 'env-effect')))
    assert.ok(!existsSync(join(home, '.angelX')))
  }
  for (const value of ['--doctor', '--task', '--help', '--workspace', '/quit', '  α\nβ  ']) {
    const draft = launch(['--draft', value, '--workspace', workspace])
    assert.equal(draft.status, 0, draft.stderr)
    assert.deepEqual(JSON.parse(draft.stdout), { argv: ['--draft', value], cwd: workspace, marker: 'interactive' })
    for (const mode of ['--ask', '--task', '--task-json']) {
      const machine = launch([mode, value])
      assert.equal(machine.status, 0, machine.stderr)
      assert.deepEqual(JSON.parse(machine.stdout), { argv: [mode, value], cwd: root, marker: null })
    }
  }
  const positional = launch([workspace, '--draft', '--resume'])
  assert.equal(positional.status, 0, positional.stderr)
  assert.deepEqual(JSON.parse(positional.stdout), { argv: ['--draft', '--resume'], cwd: workspace, marker: 'interactive' })
})

for (const replaced of [false, true]) {
  test(`selected launcher image ${replaced ? 'refuses replacement after source verification' : 'runs with its expected digest'}`, (t) => {
    const root = mkdtempSync(join(tmpdir(), 'angel-selected-launch-'))
    t.after(() => rmSync(root, { recursive: true, force: true }))
    const source = join(root, 'source')
    const home = join(root, 'home')
    const tools = join(root, 'tools')
    for (const path of [
      source,
      home,
      tools,
      join(source, 'bin'),
      join(source, 'scripts/check'),
      join(source, 'cockpit/target/release'),
    ])
      mkdirSync(path, { recursive: true })
    copyFileSync(
      fileURLToPath(new URL('../../bin/angelX', import.meta.url)),
      join(source, 'bin/angelX'),
    )
    copyFileSync(policy, join(source, 'scripts/check/angel-club-policy.sh'))
    const selected = join(root, 'selected')
    const replacement = join(root, 'replacement')
    const marker = join(root, 'executed')
    const image =
      '#!/usr/bin/env python3\nimport os, pathlib\npathlib.Path(os.environ["FIXTURE_EXECUTED"]).write_text("selected")\nprint("selected image")\n'
    writeFileSync(selected, image, { mode: 0o755 })
    writeFileSync(replacement, image.replaceAll('selected', 'replacement'), { mode: 0o755 })
    // Replace a regular source at the actual link boundary, after the
    // launcher's first SHA check. Only private fixture files are involved.
    if (replaced) {
      writeFileSync(
        join(tools, 'ln'),
        `#!/usr/bin/env python3
import os, sys
if sys.argv[-2] == os.environ['FIXTURE_SOURCE']:
    os.replace(os.environ['FIXTURE_REPLACEMENT'], sys.argv[-2])
os.execv('/usr/bin/ln', ['ln', *sys.argv[1:]])
`,
        { mode: 0o755 },
      )
    }
    const result = spawnSync('bash', [join(source, 'bin/angelX'), '--build-info', '--json'], {
      cwd: root,
      encoding: 'utf8',
      timeout: 10_000,
      env: {
        HOME: home,
        PATH: `${tools}:/usr/bin:/bin`,
        LANG: 'C.UTF-8',
        ANGEL_NO_BUILD: '1',
        ANGEL_LAUNCH_BINARY: selected,
        ANGEL_LAUNCH_BINARY_SHA256: createHash('sha256').update(image).digest('hex'),
        ANGEL_WEBGPU_PORTAL: '0',
        FIXTURE_SOURCE: selected,
        FIXTURE_REPLACEMENT: replacement,
        FIXTURE_EXECUTED: marker,
      },
    })
    if (replaced) {
      assert.notEqual(result.status, 0, 'a replacement with another digest must not execute')
      assert.equal(existsSync(marker), false)
      assert.match(result.stderr, /launch image SHA-256 differs/u)
    } else {
      assert.equal(result.status, 0, result.stderr)
      assert.equal(result.stdout, 'selected image\n')
      assert.equal(existsSync(marker), true)
    }
    assert.deepEqual(readdirSync(join(source, 'cockpit/target/release/run')), [])
  })
}
