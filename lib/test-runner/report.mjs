// Counts displayed by non-Cargo runners. This module does not execute tests or
// decide whether their process succeeded; the host retains those authorities.
const ws = /\p{White_Space}+/u
const space = /\p{White_Space}/u
const trim = value => {
  let start = 0, end = value.length
  while (start < end && space.test(value[start])) start++
  while (end > start && space.test(value[end - 1])) end--
  return start === 0 && end === value.length ? value : value.slice(start, end)
}
const split = value => trim(value).split(ws).filter(Boolean)
const lines = value => value.split('\n').map(line => line.endsWith('\r') ? line.slice(0, -1) : line)

export function project({ version, lang, stdout, stderr, max }) {
  if (version !== 1 || typeof stdout !== 'string' || typeof stderr !== 'string' || !/^[0-9]+$/.test(max)) {
    throw new Error('invalid runner report request')
  }
  const maximum = BigInt(max)
  const number = (value, strict = false) => {
    if (typeof value !== 'string' || !(strict ? /^[0-9]+$/ : /^\+?[0-9]+$/).test(value)) return null
    // Project output can contain a huge digit run. Avoid constructing an
    // enormous BigInt while preserving valid leading-zero inputs.
    const digits = value.replace(/^\+?0*/, '')
    if (digits.length > maximum.toString().length) return null
    const n = BigInt(digits || '0')
    return n <= maximum ? n : null
  }
  const before = (parts, i) => number(parts[i - 1]) ?? 0n
  let passed = 0n, failed = 0n, skipped = 0n
  const input = lines(`${stdout}\n${stderr}`)
  if (lang === 'js') {
    let summary = false
    let cancelled = 0n
    for (const line of input) {
      const t = trim(line)
      const stripped = t.startsWith('#') || t.startsWith('ℹ')
        ? trim(t.replace(/^#+/, '').replace(/^ℹ+/, '')) : t
      let parts
      // Diagnostic/stack lines cannot be summaries. Avoid allocating token
      // arrays for every word of potentially megabytes of test output.
      if (/^(?:pass|fail|cancelled|skipped|todo)\p{White_Space}/u.test(stripped)) {
        parts = split(stripped)
        if (parts.length === 2) {
          const n = number(parts[1])
          if (n !== null) {
            if (parts[0] === 'pass') { passed = n; summary = true; continue }
            if (parts[0] === 'fail') { failed = n; summary = true; continue }
            if (parts[0] === 'cancelled') { cancelled = n; summary = true; continue }
            if (parts[0] === 'skipped' || parts[0] === 'todo') { skipped += n; continue }
          }
        }
      }
      if (t.startsWith('Tests:')) {
        parts = t.split(/[,\p{White_Space}]+/u).filter(Boolean)
        for (const [i, w] of parts.entries()) {
          if (w === 'passed') passed = before(parts, i)
          else if (w === 'failed') failed = before(parts, i)
          else if (w === 'skipped' || w === 'todo') skipped += before(parts, i)
        }
        summary = true
        continue
      }
      if (t.startsWith('Tests ') && (t.includes(' passed') || t.includes(' failed'))) {
        parts = t.split(/[|()\p{White_Space}]+/u).filter(Boolean)
        for (const [i, w] of parts.entries()) {
          if (w === 'passed') passed = before(parts, i)
          else if (w === 'failed') failed = before(parts, i)
          else if (w === 'skipped') skipped += before(parts, i)
        }
        summary = true
        continue
      }
      if (!/^\+?[0-9]/.test(t)) continue
      parts = split(t)
      const n = number(parts[0])
      if (parts.length >= 2 && n !== null) {
        if (parts[1] === 'passing') { passed = n; summary = true }
        else if (parts[1] === 'failing') { failed = n; summary = true }
        else if (parts[1] === 'pending') skipped += n
      }
    }
    // Node excludes timed-out/cancelled tests from its `fail` count even
    // though the process fails. They are not successful or intentionally
    // omitted tests, so retain them in the displayed failure count.
    failed += cancelled
    if (!summary) for (const line of input) {
      const t = trim(line)
      if (/^(?:not ok|ok) /.test(t)) {
        if (/#\s*(?:SKIP|TODO)\b/i.test(t)) skipped++
        else if (t.startsWith('not ok ')) failed++
        else passed++
      }
    }
  } else if (lang === 'python') {
    for (const line of input) {
      const t = trim(trim(line).replace(/^=+|=+$/g, ''))
      if (t.includes(' in ') && /\b(?:passed|failed|errors?|skipped|xfailed|deselected)\b/.test(t)) {
        const parts = t.split(/[,\p{White_Space}]+/u).filter(Boolean)
        let hit = false
        for (const [i, w] of parts.entries()) {
          if (w === 'passed') { passed = before(parts, i); hit = true }
          else if (w === 'failed') { failed = before(parts, i); hit = true }
          else if (w === 'error' || w === 'errors') { failed += before(parts, i); hit = true }
          else if (w === 'skipped' || w === 'xfailed' || w === 'deselected') skipped += before(parts, i)
        }
        if (hit) continue
      }
      if (t.startsWith('Ran ')) {
        const n = number(split(t)[1]); if (n !== null) passed = n
        continue
      }
      if (t.startsWith('FAILED') || t.startsWith('OK')) {
        let bad = 0n, omitted = 0n
        const body = trim(t.replace(/^(?:FAILED|OK)/, '')).replace(/^[()]+|[()]+$/g, '')
        for (const part of body.split(',')) {
          const at = part.indexOf('=')
          const k = trim(at < 0 ? part : part.slice(0, at))
          const n = number(at < 0 ? '' : trim(part.slice(at + 1))) ?? 0n
          if (k === 'failures' || k === 'errors' || k === 'unexpected successes') bad += n
          else if (k === 'skipped' || k === 'expected failures') omitted += n
        }
        // Subtests can produce more failures than the parent test count.
        passed = passed >= bad + omitted ? passed - bad - omitted : 0n
        failed += bad; skipped += omitted
      }
    }
  } else if (lang === 'go') {
    // `go test -v ./...` can mix verbose test results with packages that
    // never reached a test (build/setup failures, or an early panic). A
    // verbose row in one package must not hide another package's failure.
    let verbose = false, verboseFailure = false
    for (const line of input) {
      const t = trim(line)
      if (t.startsWith('--- PASS')) { passed++; verbose = true }
      else if (t.startsWith('--- FAIL')) { failed++; verbose = true; verboseFailure = true }
      else if (t.startsWith('--- SKIP')) { skipped++; verbose = true }
      else if (/^(?:ok|FAIL|\?)\s+\S+/.test(t)) {
        if (t.startsWith('FAIL') && !verboseFailure) failed++
        else if (t.startsWith('ok') && !verbose && !t.includes('[no tests to run]')) passed++
        verbose = false; verboseFailure = false
      }
    }
  } else if (lang === 'cpp') {
    let registered = null
    // CTest includes runtime skips in its nominal passing count, while
    // excluding disabled cases from the summary total. Only registered result
    // rows count here, not the repeated footer or nested Catch2/GTest output.
    const omitted = new Map()
    for (const line of input) {
      const match = /^[0-9]+\/[0-9]+\s+Test\s+#([0-9]+): .*\*{3}(Skipped|Not Run \(Disabled\))\s+[0-9]+(?:\.[0-9]+)?\s+sec$/.exec(trim(line))
      if (!match) continue
      const id = number(match[1], true), disabled = match[2] !== 'Skipped'
      if (id === null || (omitted.has(id) && omitted.get(id) !== disabled)) return { version: 1, passed: '0', failed: '0', skipped: '0' }
      omitted.set(id, disabled)
    }
    for (const line of input) {
      const parts = split(line)
      const recognizable = parts.some((w, i) => w.endsWith('%') && parts[i + 1] === 'tests' && ['passed', 'passed,'].includes(parts[i + 2]))
      if (!recognizable) continue
      let f, total
      if (parts.length === 6 && parts[1] === 'tests' && parts[2] === 'passed' && parts[3] === 'out' && parts[4] === 'of') {
        f = 0n; total = number(parts[5], true)
      } else if (parts.length === 9 && parts[1] === 'tests' && parts[2] === 'passed,' && parts[4] === 'tests' && parts[5] === 'failed' && parts[6] === 'out' && parts[7] === 'of') {
        f = number(parts[3], true); total = number(parts[8], true)
      } else return { version: 1, passed: '0', failed: '0', skipped: '0' }
      const percent = parts[0].endsWith('%') ? number(parts[0].slice(0, -1), true) : null
      if (f === null || total === null || percent === null || percent > 100n || f > total ||
          (total === 0n && percent !== 0n && percent !== 100n) ||
          (total > 0n && percent !== ((total - f) * 100n + total / 2n) / total)) {
        return { version: 1, passed: '0', failed: '0', skipped: '0' }
      }
      const counts = [total - f, f]
      if (registered && (registered[0] !== counts[0] || registered[1] !== counts[1])) return { version: 1, passed: '0', failed: '0', skipped: '0' }
      registered = counts
    }
    const disabled = BigInt([...omitted.values()].filter(Boolean).length)
    const runtimeSkipped = BigInt(omitted.size) - disabled
    if (registered) {
      if (runtimeSkipped > registered[0]) return { version: 1, passed: '0', failed: '0', skipped: '0' }
      passed = registered[0] - runtimeSkipped; failed = registered[1]
      skipped = runtimeSkipped + disabled
    } else if (omitted.size && input.some(line => trim(line) === 'No tests were found!!!')) {
      // CTest emits no numeric summary for an entirely disabled suite.
      if (runtimeSkipped > 0n) return { version: 1, passed: '0', failed: '0', skipped: '0' }
      skipped = disabled
    } else for (const line of input) {
      const t = trim(line)
      if (t.startsWith('All tests passed (')) {
        const parts = split(t.slice('All tests passed ('.length))
        const i = parts.indexOf('test'); if (i >= 0) passed += before(parts, i)
      } else if (t.startsWith('test cases:')) {
        for (const field of t.slice('test cases:'.length).split('|').slice(1)) {
          const parts = split(field)
          if (parts[1] === 'passed') passed += before(parts, 1)
          else if (parts[1] === 'failed') failed += before(parts, 1)
        }
      } else if (t.startsWith('[  PASSED  ]')) passed += number(split(t.slice(12))[0]) ?? 0n
      else if (t.startsWith('[  FAILED  ]') && t.endsWith('listed below:')) failed += number(split(t.slice(12))[0]) ?? 0n
    }
  } else if (lang === 'rust' || lang === 'swift') {
    for (const line of input) {
      const t = trim(line)
      if (!t.startsWith('Executed ')) continue
      const parts = t.split(/[,\p{White_Space}]+/u).filter(Boolean)
      const total = number(parts[1]) ?? 0n
      const i = parts.findIndex(w => w.startsWith('failure'))
      const f = i < 0 ? 0n : before(parts, i)
      // XCTest includes skipped cases in executionCount. The optional
      // "N test(s) skipped and" clause must not turn omissions into passes.
      const j = parts.indexOf('skipped')
      const s = j >= 2 && /^(?:test|tests)$/.test(parts[j - 1]) ? number(parts[j - 2]) ?? 0n : 0n
      failed = f; skipped = s; passed = total >= f + s ? total - f - s : 0n
    }
  } else throw new Error('unknown runner language')
  if ([passed, failed, skipped].some(n => n > maximum) || passed + failed > maximum) throw new Error('runner counts exceed host limit')
  return { version: 1, passed: passed.toString(), failed: failed.toString(), skipped: skipped.toString() }
}
