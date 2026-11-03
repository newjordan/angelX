// Read-only projection of Cut manifest metadata. The worker reads manifest rows
// and normalizes integer fields; authored bodies are never rendered.
const trim = (s) => s.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, '')
const asciiUpper = (s) => s.replace(/[a-z]/g, c => String.fromCharCode(c.charCodeAt(0) - 32))
const asciiLower = (s) => s.replace(/[A-Z]/g, c => String.fromCharCode(c.charCodeAt(0) + 32))
const text = (v) => typeof v === 'string' && trim(v) ? trim(v) : null
const object = (v) => v !== null && typeof v === 'object' && !Array.isArray(v)
const add = (map, key) => { if (key) map.set(key, (map.get(key) || 0) + 1) }
const cmp = (a, b) => {
  if (typeof a !== 'string') return a < b ? -1 : a > b ? 1 : 0
  const x = Array.from(a, c => c.codePointAt(0)), y = Array.from(b, c => c.codePointAt(0))
  for (let i = 0; i < Math.min(x.length, y.length); i++) if (x[i] !== y[i]) return x[i] < y[i] ? -1 : 1
  return cmp(x.length, y.length)
}
const slice = (map) => {
  const values = [...map].sort((a, b) => b[1] - a[1] || cmp(a[0], b[0]))
  return values.length ? values.slice(0, 5).map(([k, n]) => `${k} ${n}`).join(', ') + (values.length > 5 ? ' …' : '') : '(empty)'
}
const pathKind = (path) => {
  if (typeof path !== 'string' || !path) return null
  // Rust Path ignores trailing separators and `.` path components when
  // locating an extension; preserve the original spelling for Cargo.toml.
  const name = path.replace(/(?:\/+|\/\.)+$/, '').split('/').pop()
  const dot = name.lastIndexOf('.')
  if (dot <= 0 || dot === name.length - 1) return null
  const ext = name.slice(dot + 1)
  if (ext === 'rs' || (ext === 'toml' && path.endsWith('Cargo.toml'))) return 'rust'
  if (['ts', 'tsx'].includes(ext)) return 'ts'
  if (['js', 'jsx', 'mjs', 'cjs'].includes(ext)) return 'js'
  return ext
}
const truthy = (v) => typeof v === 'boolean' ? v : typeof v === 'string' ? !!trim(v) && !['false', '0', 'no'].includes(asciiLower(trim(v))) : false
function human(row) {
  const verdict = typeof row.cut?.verdict === 'string' ? row.cut.verdict : row.verdict
  if (typeof verdict === 'string') {
    const v = asciiUpper(trim(verdict))
    if (['KEPT', 'KEEP'].includes(v)) return 'kept'
    if (['EDITED', 'EDIT'].includes(v)) return 'edited'
    if (['DISCARDED', 'DISCARD'].includes(v)) return 'discarded'
    return 'other'
  }
  const keep = Object.hasOwn(row, 'keep') ? row.keep : row.kept
  if (keep === undefined && row.edited === undefined && row.discarded === undefined) return null
  if (truthy(keep)) return 'kept'
  if (truthy(row.edited)) return 'edited'
  if (truthy(row.discarded)) return 'discarded'
  return 'other'
}
export class CutReport {
  constructor(dir, workspace, state) {
    this.dir = dir; this.workspace = workspace; this.state = state
    this.authored = 0; this.labeled = 0; this.passed = 0; this.skipped = 0
    this.timedOut = 0; this.unverified = 0; this.kept = 0; this.edited = 0
    this.discarded = 0; this.humanFields = false; this.unpairable = 0
    this.hits = []; this.slices = Object.fromEntries(['driver', 'model', 'repo', 'path', 'hour'].map(k => [k, new Map()]))
  }
  row(row) {
    if (!object(row)) return
    this.authored++
    add(this.slices.driver, text(row.driver)); add(this.slices.model, text(row.model))
    add(this.slices.repo, text(row.repo?.key)); add(this.slices.path, pathKind(row.path))
    if (typeof row.ts === 'string') add(this.slices.hour, `${String((BigInt(row.ts) / 3600n) % 24n).padStart(2, '0')}h`)
    const m = row.machine
    if (!object(m)) this.unverified++
    else if (Object.hasOwn(m, 'skipped')) this.skipped++
    else if (m.timed_out === true) this.timedOut++
    else if (typeof m.exit !== 'string') this.unverified++
    else {
      const pass = m.exit === '0'; this.labeled++; if (pass) this.passed++
      const path = text(row.path), cmd = text(m.cmd)
      if (typeof row.session !== 'string' || !path || !cmd) this.unpairable++
      else this.hits.push({ session: BigInt(row.session), path, cmd, ts: BigInt(row.ts ?? '0'), seq: BigInt(row.seq ?? '0'), pass })
    }
    const verdict = human(row)
    if (verdict !== null) { this.humanFields = true; if (verdict !== 'other') this[verdict]++ }
  }
  render(hugeShards = 0) {
    let repairs = 0
    this.hits.sort((a,b) => cmp(a.session,b.session) || cmp(a.path,b.path) || cmp(a.cmd,b.cmd) || cmp(a.ts,b.ts) || cmp(a.seq,b.seq))
    for (let i = 1; i < this.hits.length; i++) {
      const a = this.hits[i-1], b = this.hits[i]
      if (a.session === b.session && a.path === b.path && a.cmd === b.cmd && !a.pass && b.pass) repairs++
    }
    let out = `the cut — morning read\nworkspace: ${this.workspace}\nmanifest: ${this.dir}`
    if (this.state !== 'present') out += ` (${this.state} — signal density is zero)\n`
    else {
      out += ` · ${this.authored} row(s)`
      if (hugeShards) out += ` · skipped ${hugeShards} oversized shard(s)`
      if (!this.authored) out += ' — signal density is zero'
      out += '\n'
    }
    out += `  authored: ${this.authored}\n  machine: labeled ${this.labeled} · skipped ${this.skipped} · timed_out ${this.timedOut} · unverified ${this.unverified}\n`
    out += this.labeled ? `  pass rate: ${this.passed}/${this.labeled}\n` : '  pass rate: n/a (no labeled rows)\n'
    const judged = this.kept + this.edited + this.discarded
    out += this.humanFields ? `  human: KEPT ${this.kept} / EDITED ${this.edited} / DISCARDED ${this.discarded} (judged ${judged})\n` : '  judged corpus is empty\n'
    out += this.labeled && this.labeled === this.unpairable ? '  repair trajectories: n/a (manifest has no fail→pass pairing)\n' : `  repair trajectories: ${repairs}\n`
    out += `  signal density: ${this.authored} authored · ${this.labeled} labeled · ${judged} judged\n  slices (counts only; observational — not a ranking)\n`
    out += ['driver','model','repo','path','hour'].map(k => `    ${k}: ${slice(this.slices[k])}`).join('\n')
    return out
  }
}
