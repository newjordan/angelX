import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import test from 'node:test'
import { RELEASE_PATHS, REQUIRED_RELEASE_FILES } from './release-evidence.mjs'

const policy = fileURLToPath(new URL('./angel-club-policy.sh', import.meta.url))

test('the launch-time provider policy is shipped in release archives', () => {
  for (const path of ['scripts/angel-club-policy.sh', 'scripts/angel-club-policy.test.mjs']) {
    assert.ok(RELEASE_PATHS.includes(path))
    assert.ok(REQUIRED_RELEASE_FILES.includes(path))
  }
})
const credentials = [
  'ANGEL_META_KEY',
  'META_API_KEY',
  'ANGEL_LUNA_KEY',
  'LUNA_API_KEY',
  'CHATGPT_LUNA_KEY',
  'OPENAI_API_KEY',
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

test('ordinary and no-build launches retain OAuth/local settings, not API keys', () => {
  for (const extra of [
    {},
    { ANGEL_NO_BUILD: '1' },
    { ANGEL_API_CLUBS: '' },
    { ANGEL_API_CLUBS: '*' },
  ]) {
    const env = filtered(extra)
    for (const key of credentials) assert.equal(env[key], undefined, key)
    assert.equal(env.ANGEL_BRAIN_KEY, 'local-test-key')
    assert.equal(env.ANGEL_OPENAI_MODEL, 'oauth-model')
    assert.equal(env.ANGEL_GROK_MODEL, 'oauth-grok')
  }
})

test('an explicit subscription worker enables only its named provider', () => {
  const env = filtered({ ANGEL_API_CLUBS: ' GLM , kimi ' })
  for (const key of ['ZAI_API_KEY', 'ANGEL_GLM_KEY', 'MOONSHOT_API_KEY']) {
    assert.equal(env[key], 'test-only-key', key)
  }
  for (const key of ['OPENAI_API_KEY', 'META_API_KEY', 'OPENROUTER_API_KEY', 'DEEPSEEK_API_KEY']) {
    assert.equal(env[key], undefined, key)
  }
})

test('stale provider pins cannot re-enable removed Muse or OpenAI API seats', () => {
  const env = filtered({ ANGEL_API_CLUBS: 'meta,luna,openai', ANGEL_META_MODEL: 'muse-spark-1.3' })
  for (const key of credentials) assert.equal(env[key], undefined, key)
  assert.equal(env.ANGEL_META_MODEL, undefined)
})
