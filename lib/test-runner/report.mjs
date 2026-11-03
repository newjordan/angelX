// Counts displayed by non-Cargo runners. This module does not execute tests or
// decide whether their process succeeded; the host retains those authorities.
const ws = /\p{White_Space}+/u
const trim = value => value.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, '')
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
    for (const line of input) {
      const t = trim(line)
      const stripped = trim(t.replace(/^#+/, '').replace(/^ℹ+/, ''))
      let parts = split(stripped)
      if (parts.length === 2) {
        const n = number(parts[1])
        if (n !== null) {
          if (parts[0] === 'pass') { passed = n; summary = true; continue }
          if (parts[0] === 'fail') { failed = n; summary = true; continue }
          if (parts[0] === 'skipped' || parts[0] === 'todo') { skipped += n; continue }
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
      parts = split(t)
      const n = number(parts[0])
      if (parts.length >= 2 && n !== null) {
        if (parts[1] === 'passing') { passed = n; summary = true }
        else if (parts[1] === 'failing') { failed = n; summary = true }
        else if (parts[1] === 'pending') skipped += n
      }
    }
    if (!summary) for (const line of input) {
      const t = trim(line)
      if (t.startsWith('not ok ')) failed++
      else if (t.startsWith('ok ')) passed++
    }
  } else if (lang === 'python') {
    for (const line of input) {
      const t = trim(trim(line).replace(/^=+|=+$/g, ''))
      if (t.includes(' in ') && (t.includes('passed') || t.includes('failed') || t.includes('error'))) {
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
        if (passed >= bad + omitted) passed -= bad + omitted
        failed += bad; skipped += omitted
      }
    }
  } else if (lang === 'go') {
    for (const line of input) {
      const t = trim(line)
      if (t.startsWith('--- PASS')) passed++
      else if (t.startsWith('--- FAIL')) failed++
      else if (t.startsWith('--- SKIP')) skipped++
    }
    if (passed === 0n && failed === 0n) for (const line of input) {
      const t = trim(line)
      if (t.startsWith('ok ') || t.startsWith('ok\t')) passed++
      else if (t.startsWith('FAIL') && split(t).length >= 2) failed++
    }
  } else if (lang === 'cpp') {
    let registered = null
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
    if (registered) { [passed, failed] = registered }
    else for (const line of input) {
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
      failed = f; passed = total >= f ? total - f : 0n
    }
  } else throw new Error('unknown runner language')
  if ([passed, failed, skipped].some(n => n > maximum) || passed + failed > maximum) throw new Error('runner counts exceed host limit')
  return { version: 1, passed: passed.toString(), failed: failed.toString(), skipped: skipped.toString() }
}
