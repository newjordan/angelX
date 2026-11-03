// A whole-report projection. The host selects the workspace and sends only
// fields used by this report; no ledger files or network are accessed here.
import { readFileSync } from 'node:fs'

// Rust's f64 formatting rounds exact ties to even. Work with the exact IEEE-754
// rational rather than decimal approximations or JavaScript's ties-away toFixed.
function fixed(n, places) {
  if (Number.isNaN(n)) return "NaN"
  if (!Number.isFinite(n)) return n < 0 ? "-inf" : "inf"
  const negative = n < 0 || Object.is(n, -0)
  const bytes = new ArrayBuffer(8), view = new DataView(bytes)
  view.setFloat64(0, Math.abs(n), false)
  const bits = view.getBigUint64(0, false)
  const exponent = Number((bits >> 52n) & 0x7ffn)
  let numerator = exponent ? (1n << 52n) | (bits & ((1n << 52n) - 1n)) : bits & ((1n << 52n) - 1n)
  const shift = (exponent || 1) - 1023 - 52
  numerator *= 10n ** BigInt(places)
  let denominator = 1n
  if (shift >= 0) numerator <<= BigInt(shift)
  else denominator <<= BigInt(-shift)
  let q = numerator / denominator, remainder = numerator % denominator
  if (remainder * 2n > denominator || (remainder * 2n === denominator && (q & 1n))) q++
  let digits = q.toString().padStart(places + 1, '0')
  if (places) digits = `${digits.slice(0, -places)}.${digits.slice(-places)}`
  return (negative ? '-' : '') + digits
}
function scalarCompare(a, b) {
  const x = Array.from(a, c => c.codePointAt(0)), y = Array.from(b, c => c.codePointAt(0))
  for (let i = 0; i < Math.min(x.length, y.length); i++) if (x[i] !== y[i]) return x[i] - y[i]
  return x.length - y.length
}
const fmtCounts = counts => Array.from(counts, ([name, n]) => `${name} ${n}`).join(' · ')
const number = v => typeof v === 'number' && Number.isFinite(v) ? v : null
const integer = v => typeof v === 'string' && /^\d+$/.test(v) ? BigInt(v) : 0n
const mean = values => values.length ? values.reduce((a, b) => a + b, -0) / values.length : 0
function kfmt(n) { return n >= 10000n ? `${fixed(Number(n) / 1000, 1)}k` : n.toString() }
function summarize({ records, lo, hi }) {
  const countBy = key => {
    const out = new Map()
    for (const r of records) if (typeof r[key] === 'string') out.set(r[key], (out.get(r[key]) ?? 0) + 1)
    return out
  }
  const ok = records.filter(r => r.ok === true).length
  const dissents = records.map(r => number(r.dissent)).filter(x => x !== null)
  let out = `moa ledger · ${records.length} turn(s) · ok ${ok}/${records.length}\nroutes: ${fmtCounts(countBy('route'))}`
  const gates = countBy('gate')
  if (gates.size) out += `\ngate: ${fmtCounts(gates)}`
  if (dissents.length) out += `\ndissent: mean ${fixed(mean(dissents), 2)} · range ${fixed(dissents.reduce((a, b) => b < a ? b : a), 2)}–${fixed(dissents.reduce((a, b) => b > a ? b : a), 2)} (gate thresholds: relax ≤ ${typeof lo === 'string' ? lo : fixed(lo, 2)}, escalate ≥ ${typeof hi === 'string' ? hi : fixed(hi, 2)})`
  const waveSets = records.map(r => r.waves).filter(w => Array.isArray(w) && w.length)
  if (waveSets.length) {
    const stops = new Map(), byIdx = []
    for (const waves of waveSets) {
      const decision = waves.at(-1)?.decision
      if (typeof decision === 'string' && decision.startsWith('stop:')) {
        const reason = decision.slice(5); stops.set(reason, (stops.get(reason) ?? 0) + 1)
      }
      waves.forEach((wave, idx) => {
        const slot = byIdx[idx] ??= { novel: 0n, sum: 0, count: 0 }
        slot.novel += integer(wave?.novel)
        const d = number(wave?.dissent)
        if (d !== null) { slot.sum += d; slot.count++ }
      })
    }
    out += `\nwaves: mean ${fixed(mean(waveSets.map(w => w.length)), 2)}`
    if (stops.size) out += ` · stops ${fmtCounts([...stops].sort((a, b) => b[1] - a[1] || scalarCompare(a[0], b[0])))}`
    const stats = byIdx.map((w, idx) => `w${idx + 1} novel ${w.novel} d ${w.count ? fixed(w.sum / w.count, 2) : 'n/a'}`).join(' · ')
    if (stats) out += `\nwave detail: ${stats}`
  }
  const scales = records.map(r => r.scale).filter(s => s !== null && typeof s === 'object' && !Array.isArray(s))
  if (scales.length) {
    const avg = key => mean(scales.map(s => number(s[key])).filter(n => n !== null))
    out += `\nscale: mean clusters ${fixed(avg('clusters'), 2)} · dominant ${fixed(avg('dominant'), 2)} · eclusters ${fixed(avg('eclusters'), 2)}`
  }
  const spend = new Map()
  for (const r of records) for (const [label, value] of r.tokens ?? []) {
    const pair = spend.get(label) ?? [0n, 0n]
    pair[0] += integer(value.in); pair[1] += integer(value.out); spend.set(label, pair)
  }
  if (spend.size) {
    const entries = [...spend].sort((a, b) => {
      const av = a[1][0] + a[1][1], bv = b[1][0] + b[1][1]
      return av === bv ? 0 : av > bv ? -1 : 1
    })
    out += '\ntokens in/out by club:'
    for (const [label, [input, output]] of entries) out += `\n  ${label}${' '.repeat(Math.max(0, 12 - Array.from(label).length))} ${kfmt(input)} / ${kfmt(output)}`
  }
  out += '\nrecent (newest first):\n'
  out += records.slice(-5).reverse().map(r => {
    const d = number(r.dissent), gate = typeof r.gate === 'string' ? ` ${r.gate}` : ''
    return `  ${typeof r.route === 'string' ? r.route : '?'}${gate}${d === null ? '' : ` d=${fixed(d, 2)}`} · ${fixed(Number(integer(r.ms)) / 1000, 1)}s · ${r.ok === true ? 'ok' : 'err'}`
  }).join('\n')
  return out + '\n— /moa clear resets'
}
const request = JSON.parse(readFileSync(0, 'utf8'))
process.stdout.write(summarize(request))
