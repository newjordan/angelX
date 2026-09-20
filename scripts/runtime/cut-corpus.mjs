import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { collectManifest, isMachineLabeled } from './cut-evidence.mjs'
import { scoreManifest, summarize, DEFAULT_SETTLE_HOURS } from './cut-tick.mjs'
import { homedir } from 'node:os'

export function corpusSummary(rows, options = {}) {
  const { scored } = scoreManifest(rows, {
    nowSec: Math.floor(Date.now() / 1000),
    settleHours: DEFAULT_SETTLE_HOURS,
    home: homedir(),
    readFile: (path) => {
      try {
        return readFileSync(path, 'utf8')
      } catch {
        return null
      }
    },
    ...options,
  })
  return { writes: summarize(scored), machine: { labeled: rows.filter(isMachineLabeled).length } }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const index = process.argv.indexOf('--cut')
  console.log(
    JSON.stringify(corpusSummary(collectManifest(index < 0 ? undefined : process.argv[index + 1]))),
  )
}
