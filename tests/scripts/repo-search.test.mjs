import { test } from 'node:test'
import assert from 'node:assert/strict'
import { rank, brief, pushedOrdinal, dispatch } from '../../lib/repo-search/rules.mjs'

const repo = (full_name, stars, pushed_at, archived = false) => ({
  full_name, stars: String(stars), pushed_at, archived,
})

test('repository ranking matches mode, deduplication, archive and exact integer rules', () => {
  const repos = [
    repo('Owner/Repo', 100, '2026-01-01T00:00:00Z'),
    repo('owner/repo', 5000, '2026-01-01T00:00:00Z'),
    repo('b/live', 9000, '2026-07-19T00:00:00Z'),
    repo('a/archived', 10000, '2026-07-19T00:00:00Z', true),
    repo('c/old', 50000, '2020-01-01T00:00:00Z'),
  ]
  assert.deepEqual(rank(repos, 'best'), [4, 2, 1, 3])
  assert.equal(rank(repos, 'latest')[0], 2)
  assert.deepEqual(rank([repo('same/a', '9007199254740992', null), repo('SAME/A', '9007199254740993', null)], 'best'), [1])
  assert.equal(pushedOrdinal('1970-01-01T00:00:00Z'), 0)
  assert.equal(pushedOrdinal('2026-13-01'), null)
  assert.throws(() => dispatch({ version: 2, op: 'rank', repos: [], mode: 'best' }))
})

test('brief preserves shortlist order, anchors, labels and empty fields', () => {
  const input = {
    mode: 'best', query: 'test', limit: 1, notes: ['degraded'],
    repos: [
      {name:'a/one', stars:'10', archived:false, language:'Rust', raw_language:'Rust', pushed:'2026-01-01', pushed_at:'2026-01-01T00:00:00Z', description:'First', url:'https://github.com/a/one'},
      {name:'b/two', stars:'20', archived:true, language:null, raw_language:null, pushed:'2026-02-01', pushed_at:'2026-02-01T00:00:00Z', description:'', url:null},
    ],
  }
  assert.equal(brief(input), '◇ repos · "test" · best mode\n  2 repositories after dedupe\n  languages: Rust×1\n  most-starred: b/two [archived] — ★20 · — · pushed 2026-02-01 · link unavailable · (no description)\n  freshest: b/two [archived] — ★20 · — · pushed 2026-02-01 · link unavailable · (no description)\n  (notes: degraded)\n  ── shortlist ──\n   1. a/one — ★10 · Rust · pushed 2026-01-01 · source: https://github.com/a/one\n      First\n')
})

test('a project request ranks and formats the same selected records', () => {
  const values = [
    { ...repo('a/low', 1, '2026-01-01'), name: 'a/low', language: 'Rust', raw_language: 'Rust', pushed: '2026-01-01', description: '', url: null },
    { ...repo('b/high', 10000, '2026-01-01'), name: 'b/high', language: 'Rust', raw_language: 'Rust', pushed: '2026-01-01', description: '', url: null },
  ]
  const result = dispatch({ version: 1, op: 'project', mode: 'best', query: 'q', repos: values, notes: [], limit: 2 })
  assert.deepEqual(result.indices, [1, 0])
  assert.match(result.brief, /  1\. b\/high/)
  assert.match(result.brief, /  2\. a\/low/)
})
