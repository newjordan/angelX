// Read authored-write verdicts recorded by the native cockpit.
import { readFileSync, readdirSync, existsSync } from 'node:fs'
import { join } from 'node:path'
import { homedir } from 'node:os'

export function collectManifest(
  dir = process.env.ANGEL_CUT_DIR || join(homedir(), '.angel0', 'cut'),
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
        /* a torn row is not a verdict */
      }
    }
  }
  return rows
}

/**
 * Did the machine actually adjudicate this write?
 *
 * A label is an exit code from a command that ran against the code. The two
 * non-labels matter as much as the label:
 *   • `machine:{skipped:"not-source"}` — a docs edit does not earn a compile. It
 *     is a row, not a verdict, and counting it would inflate the corpus with
 *     writes nothing ever checked.
 *   • `timed_out` — the verify blew its deadline. That says something about the
 *     BUILD, not about angel's code, and cut.rs deliberately records it without
 *     an exit code. "We ran out of time" is not "you broke it."
 */
export function isMachineLabeled(row) {
  const m = row?.machine
  if (!m || typeof m !== 'object') return false
  if (m.skipped) return false
  if (m.timed_out === true) return false
  return Number.isFinite(Number(m.exit))
}
