// node curve.mjs <tokenizer> <label.jsonl>... — cumulative total and fresh input at checkpoints.
import fs from 'node:fs'
import path from 'node:path'
import { pathToFileURL } from 'node:url'
import { count, TOKENIZERS } from './analyze.mjs'
import { parseRows, prepareRows, profile, SCOPE } from './accounting.mjs'
export const CHECKPOINTS = [7, 25, 50, 100, 150, 213]

export function curveRun(name, file) {
  if (!Object.hasOwn(TOKENIZERS, name)) throw new TypeError('Unknown tokenizer')
  const prepared = prepareRows(parseRows(fs.readFileSync(file, 'utf8')))
  const { points, coverage } = profile(prepared, (text) => count(name, text))
  return {
    label: path.basename(file, '.jsonl'),
    scope: SCOPE,
    coverage,
    checkpoints: points.filter((point) => CHECKPOINTS.includes(point.request)),
  }
}

export function main(args = process.argv.slice(2)) {
  const [name, ...files] = args
  if (!files.length || !Object.hasOwn(TOKENIZERS, name))
    throw new TypeError('Tokenizer and request log paths are required')
  // Validate all inputs before emitting any result.
  const results = files.map((file) => curveRun(name, file))
  for (const row of results) {
    const out = row.checkpoints.map(
      (p) => `${p.request}: total ${p.total} fresh ${p.fresh} ctx ${p.ctx}`,
    )
    console.log(
      row.label,
      '|',
      out.join(' | '),
      '| fresh=perfect-prefix-estimate',
      `prefix_rewrites=${row.coverage.prefixRewrites} tool_changes=${row.coverage.toolSchemaChanges}`,
    )
  }
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
