import test from 'node:test'
import assert from 'node:assert/strict'
import { copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'
import { project as call } from '../../scripts/runtime/self-map.mjs'
import { RELEASE_PATHS } from '../../scripts/release/release-evidence.mjs'

test('batch projection retains paths, grouping and public symbol summaries', () => {
  const result = call({ op: 'render', top: [
    { stem: 'harness', rel: 'src/agent/harness/mod.rs', src: '//! Harness spans\n//! two lines. Further detail.\n\npub struct A;\nfn f() {}\n' },
    { stem: 'new_module', rel: 'src/new_module.rs', src: 'pub trait Example {}\n' },
  ], tools: [{ stem: 'shell', rel: 'src/agent/tools/shell.rs', src: '//! Shell.\npub enum Tool {}\n' }] })
  assert.equal(result.version, 1)
  assert.match(result.full, /### Agent harness & tooling\n- `src\/agent\/harness\/mod.rs` \(2 symbols\) — Harness spans two lines\.  \[A\]/)
  assert.match(result.full, /### Other\n- `src\/new_module.rs` \(1 symbols\) — \(no module doc — see outline\)  \[Example\]/)
  assert.match(result.compact, /\*\*Agent harness & tooling\*\*: harness \(Harness spans two lines\)/)
  assert.match(result.tools, /`src\/agent\/tools\/shell.rs` \(1 symbols\) — Shell\.  \[Tool\]/)
  assert.equal(result.toolNames, 'shell')
})

test('outline preserves line numbers and caps the returned declarations', () => {
  const src = '//! Purpose. More.\r\n\r\n' + Array.from({length: 401}, (_, i) => `pub fn f${i}() {}`).join('\n')
  const { outline } = call({ op: 'outline', rel: 'src/test.rs', src })
  assert.match(outline, /^# src\/test\.rs\nPurpose\.\n\nSymbols:\n3: pub fn f0\(\) \{\}/)
  assert.match(outline, /402: pub fn f399\(\) \{\}\n…\[truncated at 400 symbols\]\n$/)
  assert.doesNotMatch(outline, /f400/)
})

test('source analysis handles Unicode whitespace, CRLF and scalar truncation', () => {
  const src = `\u0085//! ${'🦀'.repeat(201)}\r\npub(crate) struct Δοκιμή;\r\n`
  const result = call({ op: 'analyze', src })
  assert.equal(result.purpose, '🦀'.repeat(200) + '…')
  assert.equal(result.count, 1)
  assert.deepEqual(result.keyTypes, ['Δοκιμή'])
})

test('the release contains an independently loadable self-map module', async t => {
  assert.ok(RELEASE_PATHS.includes('scripts/runtime/self-map.mjs'))
  assert.ok(RELEASE_PATHS.includes('lib/self-map/report.mjs'))
  const root = mkdtempSync(join(tmpdir(), 'angel-self-map-bundle-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const module = join(root, 'scripts/runtime/self-map.mjs')
  const library = join(root, 'lib/self-map/report.mjs')
  mkdirSync(join(root, 'scripts/runtime'), { recursive: true })
  mkdirSync(join(root, 'lib/self-map'), { recursive: true })
  copyFileSync(new URL('../../scripts/runtime/self-map.mjs', import.meta.url), module)
  copyFileSync(new URL('../../lib/self-map/report.mjs', import.meta.url), library)
  const { project } = await import(pathToFileURL(module).href)
  assert.equal(project({ op: 'analyze', src: '//! Bundled.\npub struct Bundle;\n' }).purpose, 'Bundled.')
})
