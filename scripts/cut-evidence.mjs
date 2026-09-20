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
  return typeof m.exit === 'number' && Number.isFinite(m.exit)
}

export const squash = (s) => s.replace(/\s+/g, ' ').trim()

// Blank lines and lone delimiters carry no authorship — they survive everything.
export const substantive = (text) =>
  text
    .split('\n')
    .map((l) => l.trim())
    .filter((l) => l.length > 3 && !/^[{}()[\];,]+$/.test(l))

/**
 * Did the text angel authored survive into the file as it stands today?
 * KEPT (verbatim, modulo whitespace reflow) / EDITED (≥40% of substantive lines
 * still there — the gradient) / DISCARDED. `current === null` means the file's
 * text is unavailable; callers that cannot see the file must report UNRESOLVED
 * instead of calling this (rule 3: unresolvable is not discarded).
 */
export function classify(authored, current) {
  if (current === null) return 'DISCARDED'
  if (current.includes(authored)) return 'KEPT'
  if (squash(current).includes(squash(authored))) return 'KEPT' // whitespace-only reflow

  const lines = substantive(authored)
  if (lines.length === 0) return 'DISCARDED'
  const haystack = new Set(substantive(current))
  const ratio = lines.filter((l) => haystack.has(l)).length / lines.length

  if (ratio >= 0.95) return 'KEPT'
  if (ratio >= 0.4) return 'EDITED'
  return 'DISCARDED'
}
