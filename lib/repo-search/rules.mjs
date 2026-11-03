// Pure GitHub shortlist policy. The native host owns network, credentials,
// citation validation, cache and all input/output bounds.
const asciiLower = s => s.replace(/[A-Z]/g, c => c.toLowerCase())
const trim = s => s.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, '')
const cmp = (a, b) => {
  const x = Array.from(a, c => c.codePointAt(0)), y = Array.from(b, c => c.codePointAt(0))
  for (let i = 0; i < Math.min(x.length, y.length); i++) if (x[i] !== y[i]) return x[i] < y[i] ? -1 : 1
  return Math.sign(x.length - y.length)
}

// Match the host's day-ordinal contract, including its permissive day 1..31.
export function pushedOrdinal(raw) {
  if (typeof raw !== 'string') return null
  const s = trim(raw)
  if (s.length < 10 || s[4] !== '-' || s[7] !== '-') return null
  const numeric = v => /^[+-]?[0-9]+$/.test(v) ? Number(v) : NaN
  const year = numeric(s.slice(0, 4)), month = numeric(s.slice(5, 7)), day = numeric(s.slice(8, 10))
  if (!Number.isInteger(year) || !(month >= 1 && month <= 12) || !(day >= 1 && day <= 31)) return null
  const y = month <= 2 ? year - 1 : year
  const era = Math.trunc((y >= 0 ? y : y - 399) / 400)
  const yoe = y - era * 400
  const doy = Math.trunc((153 * (month > 2 ? month - 3 : month + 9) + 2) / 5) + day - 1
  return era * 146097 + yoe * 365 + Math.trunc(yoe / 4) - Math.trunc(yoe / 100) + doy - 719468
}

export function rank(repos, mode) {
  if (!Array.isArray(repos) || !['best', 'latest'].includes(mode)) throw new Error('invalid rank request')
  const entries = repos.map((repo, index) => {
    if (!repo || typeof repo.full_name !== 'string' || typeof repo.stars !== 'string' || !/^(0|[1-9][0-9]*)$/.test(repo.stars) || BigInt(repo.stars) > 18446744073709551615n || typeof repo.archived !== 'boolean') throw new Error('invalid repo')
    return { repo, index, stars: BigInt(repo.stars), ordinal: pushedOrdinal(repo.pushed_at) }
  })
  const newest = entries.reduce((n, item) => item.ordinal === null ? n : Math.max(n, item.ordinal), entries.some(item => item.ordinal !== null) ? -Infinity : 0)
  const deduped = new Map()
  for (const entry of entries) {
    const key = asciiLower(entry.repo.full_name), previous = deduped.get(key)
    if (!previous || entry.stars > previous.stars) deduped.set(key, entry)
  }
  const score = entry => {
    const stars = Math.log(Number(entry.stars) + 1)
    const recency = entry.ordinal !== null && newest >= entry.ordinal ? Math.max(0, 1 - (newest - entry.ordinal) / 730) : 0
    return (mode === 'best' ? stars + recency * 1.5 : recency * 6 + stars * 0.5) * (entry.repo.archived ? 0.4 : 1)
  }
  return [...deduped.values()].sort((a, b) => score(b) - score(a) || (a.stars === b.stars ? 0 : a.stars > b.stars ? -1 : 1) || cmp(a.repo.full_name, b.repo.full_name)).map(entry => entry.index)
}

export function brief({ query, repos, notes, mode, limit }) {
  if (typeof query !== 'string' || !Array.isArray(repos) || !Array.isArray(notes) || !['best', 'latest'].includes(mode) || !Number.isSafeInteger(limit) || limit < 0) throw new Error('invalid brief request')
  let out = `◇ repos · "${query}" · ${mode} mode\n  ${repos.length} repositor${repos.length === 1 ? 'y' : 'ies'} after dedupe\n`
  const langs = new Map()
  for (const r of repos) if (r.raw_language) langs.set(r.raw_language, (langs.get(r.raw_language) || 0) + 1)
  const top = [...langs].sort((a, b) => b[1] - a[1] || cmp(a[0], b[0])).slice(0, 4)
  if (top.length) out += `  languages: ${top.map(([k, n]) => `${k}×${n}`).join(', ')}\n`
  let most, freshest
  for (const r of repos) {
    if (!most || BigInt(r.stars) >= BigInt(most.stars)) most = r
    const ordinal = pushedOrdinal(r.pushed_at)
    if (ordinal !== null && (!freshest || ordinal >= freshest.ordinal)) freshest = { ordinal, repo: r }
  }
  const oneLine = r => `${r.name}${r.archived ? ' [archived]' : ''} — ★${r.stars} · ${r.language || '—'} · pushed ${r.pushed ?? 'n.d.'} · ${r.url ? `source: ${r.url}` : 'link unavailable'}\n      ${r.description || '(no description)'}`
  if (most) out += `  most-starred: ${oneLine(most).replace('\n      ', ' · ')}\n`
  if (freshest) out += `  freshest: ${oneLine(freshest.repo).replace('\n      ', ' · ')}\n`
  if (notes.length) out += `  (notes: ${notes.join('; ')})\n`
  out += '  ── shortlist ──\n'
  repos.slice(0, limit).forEach((r, i) => { out += `  ${String(i + 1).padStart(2)}. ${oneLine(r)}\n` })
  return out
}

export function dispatch(request) {
  if (!request || request.version !== 1) throw new Error('unsupported repo policy version')
  if (request.op === 'rank') return { version: 1, indices: rank(request.repos, request.mode) }
  if (request.op === 'project') {
    const indices = rank(request.repos, request.mode)
    return { version: 1, indices, brief: brief({ ...request, repos: indices.map(i => request.repos[i]) }) }
  }
  throw new Error('unknown repo policy operation')
}
