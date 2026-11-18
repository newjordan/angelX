import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  blockHeld,
  MIN_SECONDS,
  MARGIN,
  READ_WPM,
  plan,
  readable,
  readSeconds,
  rollSeconds,
  wordsPerRow,
} from '../../scripts/readme-charts/pan-pace.mjs'

test('a moderate reader sets the clock', () => {
  assert.equal(READ_WPM, 275)
  assert.equal(readSeconds(275), 60)
  assert.equal(readSeconds(0), 0)
  assert.equal(readSeconds(-5), 0)
})

test('the take lasts longer than the reading, and never less than the floor', () => {
  assert.equal(readSeconds(46), (46 / READ_WPM) * 60)
  assert.equal(rollSeconds(46), Math.ceil(readSeconds(46) * MARGIN))
  assert.ok(rollSeconds(46) > MIN_SECONDS)
  assert.equal(rollSeconds(2), MIN_SECONDS)
  assert.equal(rollSeconds(550), Math.ceil(120 * MARGIN))
})

test('the answer-pan take is paced by the text it rolls over', () => {
  const shot = plan({ words: 46, rows: 8, windowRows: 24, cellPx: 36, fps: 30 })
  assert.equal(shot.seconds, rollSeconds(46))
  assert.equal(shot.rowsPerSecond, shot.rows / shot.seconds)
  assert.equal(shot.secondsPerRow, shot.seconds / 8)
  assert.equal(shot.pxPerFrame, ((8 / shot.seconds) * 36) / 30)
  assert.equal(shot.endRow, 8)

  const fit = readable(shot, 8)
  assert.equal(wordsPerRow(46, 8), 5.75)
  assert.equal(fit.ok, true)
  assert.ok(fit.havePerRow > fit.needPerRow * 1.5, 'comfortable, not marginal')
})

test('readable() bites when the camera would outrun the reader', () => {
  const rushed = plan({
    words: 400,
    rows: 30,
    windowRows: 24,
    cellPx: 36,
    fps: 30,
    margin: 0.02,
    min: 2,
  })
  const fit = readable(rushed, 8)
  assert.equal(fit.ok, false)
  assert.ok(fit.havePerRow < fit.needPerRow)
})

test('no travel means nothing can outrun anyone', () => {
  const held = plan({ words: 46, rows: 0, windowRows: 24, cellPx: 36, fps: 30 })
  assert.equal(held.secondsPerRow, Number.POSITIVE_INFINITY)
  assert.equal(held.pxPerFrame, 0)
  assert.equal(readable(held, 8).ok, true)
})

test('the whole response stays in frame, not just each row', () => {
  const shot = plan({ words: 45, rows: 6, windowRows: 18, cellPx: 36, fps: 30 })
  const held = blockHeld(shot, { words: 42, blockFrom: 5, blockRows: 6 })
  assert.equal(shot.seconds, 18)
  assert.equal(held.fits, true)
  assert.equal(held.hold, 5 / (6 / 18))
  assert.equal(held.ok, true)
  assert.ok(held.hold / held.need > 1.3)

  const shortWindow = plan({ words: 45, rows: 6, windowRows: 5, cellPx: 36, fps: 30 })
  assert.equal(blockHeld(shortWindow, { words: 42, blockFrom: 5, blockRows: 6 }).fits, false)

  const rushed = plan({
    words: 45,
    rows: 6,
    windowRows: 18,
    cellPx: 36,
    fps: 30,
    min: 1,
    margin: 0.02,
  })
  assert.equal(blockHeld(rushed, { words: 42, blockFrom: 5, blockRows: 6 }).ok, false)
})
