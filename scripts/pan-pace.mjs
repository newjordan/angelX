// pan-pace.mjs — pure pacing math for pan-to-mp4.mjs.
//
// A pan shot of text is only useful if the camera never outruns the reader, so
// the roll is paced by the words on screen rather than by taste: the duration is
// the time a moderate reader needs for those lines, times a comfort factor, and
// never shorter than MIN_SECONDS. `readable()` is the check that makes that a
// property instead of a claim — no row may leave the frame before a moderate
// reader is done with it.
//
// Kept free of I/O so tests/scripts/pan-pace.test.mjs can hold the pace still.

/** Moderate delivery: 300 wpm is brisk, 200 wpm is careful. */
export const READ_WPM = 275
/** Comfort factor over the fastest moderate reader. */
export const MARGIN = 1.8
/** Floor for a shot that is meant to be watched, not skimmed. */
export const MIN_SECONDS = 18

/** Seconds a reader needs for `words` at `wpm`. */
export function readSeconds(words, wpm = READ_WPM) {
  return (Math.max(0, words) / wpm) * 60
}

/** Seconds the camera should take to carry `words` past the frame. */
export function rollSeconds(words, { wpm = READ_WPM, margin = MARGIN, min = MIN_SECONDS } = {}) {
  return Math.max(min, Math.ceil(readSeconds(words, wpm) * margin))
}

/** Words per row, given the text's own row count. */
export function wordsPerRow(words, textRows) {
  return Math.max(0, words) / Math.max(1, textRows)
}

/**
 * The camera move: `rows` of travel over `seconds`, in cells and pixels.
 * `secondsPerRow` is the number that decides whether a reader keeps up.
 */
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

/** Does the move keep every row on screen long enough to read it? */
export function readable(shot, textRows, wpm = READ_WPM) {
  const needPerRow = readSeconds(wordsPerRow(shot.words, textRows), wpm)
  return {
    needPerRow,
    havePerRow: shot.secondsPerRow,
    /** True when the camera is never faster than the reader. */
    ok: shot.secondsPerRow >= needPerRow,
  }
}

/**
 * Seconds before the camera's top edge passes `blockFrom` — how long a run of
 * rows stays *whole* inside the frame. A per-row check is not enough on its own:
 * a response that has already drifted out of the shot is unreadable however slow
 * the drift is.
 */
export function holdSeconds(shot, blockFrom) {
  if (!shot.rowsPerSecond) return Number.POSITIVE_INFINITY
  return Math.max(0, blockFrom - shot.startRow) / shot.rowsPerSecond
}

/** Is a `blockRows`-tall block of `words` starting at `blockFrom` held whole long enough to read? */
export function blockHeld(shot, { words, blockFrom, blockRows, wpm = READ_WPM, beat = 1.5 }) {
  const fits = shot.windowRows >= blockRows // it can never be whole in a shorter window
  const need = readSeconds(words, wpm) + beat
  return {
    hold: fits ? holdSeconds(shot, blockFrom) : 0,
    need,
    fits,
    ok: fits && holdSeconds(shot, blockFrom) >= need,
  }
}
