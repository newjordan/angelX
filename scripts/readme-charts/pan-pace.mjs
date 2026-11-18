export const READ_WPM = 275
export const MARGIN = 1.8
export const MIN_SECONDS = 18

export function readSeconds(words, wpm = READ_WPM) {
  return (Math.max(0, words) / wpm) * 60
}

export function rollSeconds(words, { wpm = READ_WPM, margin = MARGIN, min = MIN_SECONDS } = {}) {
  return Math.max(min, Math.ceil(readSeconds(words, wpm) * margin))
}

export function wordsPerRow(words, textRows) {
  return Math.max(0, words) / Math.max(1, textRows)
}

export function plan({
  words,
  rows,
  windowRows,
  startRow = 0,
  cellPx,
  fps,
  wpm = READ_WPM,
  margin = MARGIN,
  min = MIN_SECONDS,
}) {
  const seconds = rollSeconds(words, { wpm, margin, min })
  const rowsPerSecond = rows / seconds
  const pxPerSecond = rowsPerSecond * cellPx
  return {
    words,
    seconds,
    rows,
    startRow,
    endRow: startRow + rows,
    windowRows,
    rowsPerSecond,
    secondsPerRow: rows ? seconds / rows : Number.POSITIVE_INFINITY,
    cellPx,
    fps,
    pxPerSecond,
    pxPerFrame: pxPerSecond / fps,
    readSeconds: readSeconds(words, wpm),
    wpm,
    margin,
  }
}

export function readable(shot, textRows, wpm = READ_WPM) {
  const needPerRow = readSeconds(wordsPerRow(shot.words, textRows), wpm)
  return {
    needPerRow,
    havePerRow: shot.secondsPerRow,
    ok: shot.secondsPerRow >= needPerRow,
  }
}

export function holdSeconds(shot, blockFrom) {
  if (!shot.rowsPerSecond) return Number.POSITIVE_INFINITY
  return Math.max(0, blockFrom - shot.startRow) / shot.rowsPerSecond
}

export function blockHeld(shot, { words, blockFrom, blockRows, wpm = READ_WPM, beat = 1.5 }) {
  const fits = shot.windowRows >= blockRows
  const need = readSeconds(words, wpm) + beat
  return {
    hold: fits ? holdSeconds(shot, blockFrom) : 0,
    need,
    fits,
    ok: fits && holdSeconds(shot, blockFrom) >= need,
  }
}
