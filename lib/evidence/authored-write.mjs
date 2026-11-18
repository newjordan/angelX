export function isMachineLabeled(row) {
  const m = row?.machine
  if (!m || typeof m !== 'object') return false
  if (m.skipped) return false
  if (m.timed_out === true) return false
  return typeof m.exit === 'number' && Number.isFinite(m.exit)
}

export const squash = (s) => s.replace(/\s+/g, ' ').trim()

export const substantive = (text) =>
  text
    .split('\n')
    .map((l) => l.trim())
    .filter((l) => l.length > 3 && !/^[{}()[\];,]+$/.test(l))

export function classify(authored, current) {
  if (current === null) return 'DISCARDED'
  if (current.includes(authored)) return 'KEPT'
  if (squash(current).includes(squash(authored))) return 'KEPT'

  const lines = substantive(authored)
  if (lines.length === 0) return 'DISCARDED'
  const haystack = new Set(substantive(current))
  const ratio = lines.filter((l) => haystack.has(l)).length / lines.length

  if (ratio >= 0.95) return 'KEPT'
  if (ratio >= 0.4) return 'EDITED'
  return 'DISCARDED'
}
