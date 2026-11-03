import { test } from 'node:test'
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

test('cut rejects malformed values even when duplicate keys overwrite them', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'angel-cut-duplicates-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const rows = [
    String.raw`{"driver":"\ud800","driver":"hidden"}`,
    String.raw`{"repo":{"key":"\udc00","key":"hidden"}}`,
    String.raw`{"\u0064river":"\ud800","driver":"hidden"}`,
    String.raw`{"metadata":{"\ud800":1},"metadata":{}}`,
    String.raw`{"machine":{"exit":1e999},"machine":{"exit":0}}`,
    String.raw`{"driver":"\ud83d\ude00","machine":{"exit":0}}`,
  ]
  writeFileSync(join(root, 'authored-20260101.jsonl'), `${rows.join('\n')}\n`)
  const worker = fileURLToPath(new URL('../../scripts/runtime/cut-report.mjs', import.meta.url))
  const result = spawnSync(process.execPath, [worker, '--dir', root, '--workspace', '/tmp/ws'], {
    encoding: 'utf8', timeout: 10_000,
  })
  assert.equal(result.status, 0, result.stderr)
  assert.match(result.stdout, /authored: 1\n/)
  assert.match(result.stdout, /pass rate: 1\/1/)
  assert.match(result.stdout, /driver: 😀 1/)
  assert.doesNotMatch(result.stdout, /hidden/)
})
