import { readFileSync, readdirSync, existsSync } from 'node:fs'
import { join } from 'node:path'
import { homedir } from 'node:os'

export function collectManifest(
  dir = process.env.ANGEL_CUT_DIR || join(homedir(), '.angelX', 'cut'),
) {
  if (!existsSync(dir)) return []
  const rows = []
  for (const name of readdirSync(dir)
    .filter((n) => /^authored-\d{8}\.jsonl$/.test(n))
    .sort()) {
    let text
    try {
      text = readFileSync(join(dir, name), 'utf8')
    } catch {
      continue
    }
    for (const line of text.split('\n')) {
      if (!line.trim()) continue
      try {
        rows.push(JSON.parse(line))
      } catch {
      }
    }
  }
  return rows
}

export * from '../../lib/evidence/authored-write.mjs'
