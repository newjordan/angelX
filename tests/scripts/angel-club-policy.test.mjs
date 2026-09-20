import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { chmodSync, copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import test from 'node:test'
import { RELEASE_PATHS, REQUIRED_RELEASE_FILES } from '../../scripts/release/release-evidence.mjs'

const policy = fileURLToPath(new URL('../../scripts/check/angel-club-policy.sh', import.meta.url))

test('the launch-time provider policy is shipped in release archives', () => {
  for (const path of ['scripts/check/angel-club-policy.sh', 'tests/scripts/angel-club-policy.test.mjs']) {
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
  'GPU_COMP_GROK_KEY',
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
  const env = filtered({ META_API_KEY: 'fixture', ANGEL_META_MODEL: 'user-model' })
  assert.equal(env.META_API_KEY, 'fixture')
  assert.equal(env.ANGEL_META_MODEL, 'user-model')
})

test('real launcher loads system.env, preserves API choices, and isolates headless configuration', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'angel-provider-launch-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const home = join(root, 'home')
  const source = join(root, 'source')
  for (const dir of ['bin', 'scripts/check', 'cockpit/target/release']) mkdirSync(join(source, dir), { recursive: true })
  mkdirSync(join(home, '.config/host_env'), { recursive: true })
  copyFileSync(fileURLToPath(new URL('../../bin/angel0', import.meta.url)), join(source, 'bin/angel0'))
  copyFileSync(policy, join(source, 'scripts/check/angel-club-policy.sh'))
  const probe = join(source, 'cockpit/target/release/angel')
  writeFileSync(probe, `#!/usr/bin/env python3
import json, os
print(json.dumps({"grok": bool(os.environ.get("XAI_API_KEY")),
 "openai": bool(os.environ.get("OPENAI_API_KEY")), "marker": os.environ.get("ANGEL_TEST_CONFIG")}))
`)
  chmodSync(probe, 0o755)
  writeFileSync(join(home, '.config/host_env/system.env'),
    'XAI_API_KEY=fixture-xai\nOPENAI_API_KEY=fixture-openai\nANGEL_TEST_CONFIG=system\n')
  const launch = (args = [], extra = {}) => {
    const result = spawnSync('bash', [join(source, 'bin/angel0'), ...args], {
      cwd: root, encoding: 'utf8', timeout: 15000,
      env: { HOME: home, PATH: '/usr/bin:/bin', LANG: 'C.UTF-8', ANGEL_NO_BUILD: '1',
        ANGEL_VIDEO: '0', ANGEL_WEBGPU_PORTAL: '0', ...extra },
    })
    assert.equal(result.status, 0, result.stderr)
    return JSON.parse(result.stdout)
  }
  assert.deepEqual(launch(), { grok: true, openai: true, marker: 'system' })
  assert.deepEqual(launch([], { ANGEL_API_CLUBS: 'none' }), { grok: false, openai: false, marker: 'system' })
  writeFileSync(join(source, '.angel.env'), 'ANGEL_TEST_CONFIG=project\n')
  assert.equal(launch().marker, 'project')
  assert.deepEqual(launch(['--build-info', '--json']), { grok: false, openai: false, marker: null })
})
