// node analyze.mjs <label.jsonl>...  — per-run wire tokens under four tokenizers.
// Input per request = system + messages + tool schemas (JSON). "Fresh" input
// estimates perfect caching of an exact unchanged prefix. It is not measured
// provider cache usage; changed tools invalidate the represented request.
import fs from 'node:fs'
import path from 'node:path'
import { pathToFileURL } from 'node:url'
import { parseRows, prepareRows, profile, SCOPE } from './accounting.mjs'
import { fromPreTrained as ds } from '@lenml/tokenizer-deepseek_v4'
import { fromPreTrained as g4 } from '@lenml/tokenizer-gpt4o'
import { fromPreTrained as q3 } from '@lenml/tokenizer-qwen3'
import { fromPreTrained as l3 } from '@lenml/tokenizer-llama3_1'

export const TOKENIZERS = { deepseek_v4: ds(), gpt4o: g4(), qwen3: q3(), llama3: l3() }
const cache = new Map()
export function count(name, text) {
  if (!text) return 0
  const key = name + '\u0000' + text
  let n = cache.get(key)
  if (n === undefined) {
    n = TOKENIZERS[name].encode(text, { add_special_tokens: false }).length
    cache.set(key, n)
  }
  return n
}

export function analyzeRun(file) {
  const rows = parseRows(fs.readFileSync(file, 'utf8'))
  const prepared = prepareRows(rows)
  const out = {}
  let coverage
  for (const name of Object.keys(TOKENIZERS)) {
    const result = profile(prepared, (text) => count(name, text))
    coverage = result.coverage
    out[name] = {
      requests: result.requests,
      first: result.first,
      total: result.total,
      fresh: result.fresh,
    }
  }
  const reads = rows.filter((r) => JSON.stringify(r.action ?? '').includes('ledger://')).length
  const bytes = rows.map((r) =>
    typeof r.bytes === 'number' && Number.isFinite(r.bytes) && r.bytes >= 0 ? r.bytes : null,
  )
  const bytesReported = bytes.filter((value) => value !== null)
  return {
    label: path.basename(file, '.jsonl'),
    requests: rows.length,
    ledgerReads: reads,
    bytesFirst: bytes[0],
    bytesTotal:
      bytesReported.length === rows.length ? bytesReported.reduce((a, b) => a + b, 0) : null,
    tok: out,
    scope: SCOPE,
    coverage,
    bytesReportedSubtotal: bytesReported.length ? bytesReported.reduce((a, b) => a + b, 0) : null,
    bytesCoverage: { reported: bytesReported.length, requests: rows.length },
  }
}

export function main(args = process.argv.slice(2)) {
  if (!args.length) throw new TypeError('Request log paths are required')
  const results = args.map(analyzeRun)
  console.log(JSON.stringify(results, null, 1))
}

if (
  process.argv[1] &&
  fs.existsSync(process.argv[1]) &&
  import.meta.url === pathToFileURL(fs.realpathSync(process.argv[1])).href
) {
  try {
    main()
  } catch (error) {
    console.error(`Token input rejected (${error.name})`)
    process.exitCode = 1
  }
}
