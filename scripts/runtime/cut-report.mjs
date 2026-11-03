// Isolated read-only Cut projection. Never writes a manifest or graph.
import { openSync, fstatSync, readSync, writeSync, closeSync, readdirSync, existsSync, constants } from 'node:fs'
import { join } from 'node:path'
import { CutReport } from '../../lib/cut/report.mjs'

const SHARD_LIMIT = 32 * 1024 * 1024
const LINE_LIMIT = 512 * 1024
const TOTAL_LIMIT = 64 * 1024 * 1024
const decoder = new TextDecoder('utf-8', { fatal: true })
const integer = (source, signed) => {
  if (typeof source !== 'string' || source.length > 20 || source === '-0' || !/^-?(0|[1-9][0-9]*)$/.test(source)) return null
  const n = BigInt(source)
  if (signed ? n < -(1n << 63n) || n >= 1n << 63n : n < 0n || n >= 1n << 64n) return null
  return n.toString()
}
class NumericToken {
  constructor(source) { this.source = source }
}
function validUnicode(text) {
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i)
    if (c >= 0xd800 && c <= 0xdbff) {
      const next = text.charCodeAt(++i)
      if (!(next >= 0xdc00 && next <= 0xdfff)) return false
    } else if (c >= 0xdc00 && c <= 0xdfff) return false
  }
  return true
}
// JSON.parse rounds large integers. Preserve each numeric literal before parsing,
// without requiring the newer reviver context.source API. The marker is chosen
// against *decoded* object keys, so user JSON cannot impersonate a token.
function parseRow(text) {
  // serde_json rejects 128 nested containers; reject them before asking V8's
  // recursive reviver to walk the input.
  let depth = 0, quoted = false, escaped = false
  for (const char of text) {
    if (quoted) {
      if (escaped) escaped = false
      else if (char === '\\') escaped = true
      else if (char === '"') quoted = false
    } else if (char === '"') quoted = true
    else if (char === '{' || char === '[') {
      if (++depth >= 128) throw new Error('nesting limit')
    } else if (char === '}' || char === ']') depth--
  }
  const keys = new Set()
  JSON.parse(text, (key, value) => {
    if (!validUnicode(key) || (typeof value === 'string' && !validUnicode(value))) throw new Error('invalid Unicode')
    if (typeof value === 'number' && !Number.isFinite(value)) throw new Error('number out of range')
    if (value && typeof value === 'object') for (const key of Object.keys(value)) keys.add(key)
    return value
  })
  let marker = '__angel_cut_number__'
  while (keys.has(marker)) marker += '_'
  const pieces = []
  const number = /-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/y
  for (let i = 0; i < text.length;) {
    if (text[i] === '"') {
      const start = i++
      while (i < text.length) {
        if (text[i] === '\\') { i += 2; continue }
        if (text[i++] === '"') break
      }
      const literal = text.slice(start, i)
      // JSON.parse's reviver sees only the last duplicate key. Validate each
      // source string before duplicate properties can hide invalid Unicode.
      if (!validUnicode(JSON.parse(literal))) throw new Error('invalid Unicode')
      pieces.push(literal)
    } else if (text[i] === '-' || (text[i] >= '0' && text[i] <= '9')) {
      number.lastIndex = i
      const token = number.exec(text)
      if (!token) throw new Error('invalid number')
      if (!Number.isFinite(Number(token[0]))) throw new Error('number out of range')
      pieces.push(`{"${marker}":${JSON.stringify(token[0])}}`)
      i = number.lastIndex
    } else pieces.push(text[i++])
  }
  return JSON.parse(pieces.join(''), (key, value) => {
    if (value && typeof value === 'object' && Object.hasOwn(value, marker)) value = new NumericToken(value[marker])
    if (['ts', 'session', 'seq'].includes(key)) return value instanceof NumericToken ? integer(value.source, false) : null
    if (key === 'exit') return value instanceof NumericToken ? integer(value.source, true) : null
    if (['keep', 'kept', 'edited', 'discarded'].includes(key) && value instanceof NumericToken) {
      const n = integer(value.source, true)
      return n !== null && n !== '0'
    }
    return value instanceof NumericToken ? Number(value.source) : value
  })
}
function main(dir, workspace) {
  let shards = []
  try { shards = readdirSync(dir).filter(n => n.length === 23 && /^authored-[0-9]{8}\.jsonl$/.test(n)).sort() } catch { /* missing/unreadable is empty */ }
  const report = new CutReport(dir, workspace, !existsSync(dir) ? 'missing' : shards.length ? 'present' : 'empty')
  let huge = 0, total = 0
  for (const shard of shards) {
    let fd
    try {
      fd = openSync(join(dir, shard), constants.O_RDONLY | (constants.O_NONBLOCK ?? 0))
      const stat = fstatSync(fd)
      if (!stat.isFile()) continue
      if (stat.size > SHARD_LIMIT) { huge++; continue }
      let parts = [], length = 0, over = false, read = 0
      const data = Buffer.allocUnsafe(64 * 1024)
      const finish = () => {
        // BufRead::lines removes a final CR as well as the newline.
        if (length && parts.at(-1)?.at(-1) === 13) {
          const last = parts.pop(); parts.push(last.subarray(0, last.length - 1)); length--
        }
        if (!over && length <= LINE_LIMIT && length) {
          try { report.row(parseRow(decoder.decode(Buffer.concat(parts, length)))) } catch { /* torn/invalid row */ }
        }
        parts = []; length = 0; over = false
      }
      for (;;) {
        const count = readSync(fd, data, 0, data.length, null)
        if (!count) break
        read += count; total += count
        if (read > SHARD_LIMIT || total > TOTAL_LIMIT) throw new Error('report input exceeded its limit')
        let start = 0
        for (let i = 0; i < count; i++) {
          if (data[i] !== 10) continue
          const size = i - start
          if (length + size > LINE_LIMIT + 1) over = true
          if (!over && size) { parts.push(Buffer.from(data.subarray(start, i))); length += size }
          finish(); start = i + 1
        }
        const size = count - start
        if (length + size > LINE_LIMIT + 1) over = true
        if (!over && size) { parts.push(Buffer.from(data.subarray(start, count))); length += size }
      }
      finish()
    } catch (error) {
      if (error.message === 'report input exceeded its limit') throw error
      // Unreadable shard: same best-effort treatment as the native reader.
    } finally { if (fd !== undefined) closeSync(fd) }
  }
  const result = Buffer.from(report.render(huge))
  for (let offset = 0; offset < result.length;) offset += writeSync(1, result, offset)
}
if (process.argv.length !== 6 || process.argv[2] !== '--dir' || process.argv[4] !== '--workspace') throw new Error('expected --dir <path> --workspace <path>')
main(process.argv[3], process.argv[5])
