// Read the numeric store limits from the same checked-in manifest embedded by
// the cockpit. This deliberately supports only the manifest's simple integer
// assignments, not arbitrary TOML values or configuration overrides.
import { readFileSync } from 'node:fs'
const manifest = readFileSync(new URL('../../docs/telemetry/store-caps.toml', import.meta.url), 'utf8')
export function storeCap(store, key) {
  let section
  for (const line of manifest.split('\n')) {
    const header = line.match(/^\[([a-z_]+)\]\s*$/)
    if (header) section = header[1]
    const assignment = line.match(/^([a-z_]+)\s*=\s*(\d+)\s*$/)
    if (section === store && assignment?.[1] === key) {
      const value = Number(assignment[2])
      if (Number.isSafeInteger(value) && value > 0) return value
    }
  }
  throw new Error(`Missing positive store cap: ${store}.${key}`)
}
