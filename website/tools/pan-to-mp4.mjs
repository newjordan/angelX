#!/usr/bin/env node
// pan-to-mp4.mjs — one slow Y-axis take over a captured frame's response.
//
//   NODE_PATH=/tmp/shot/node_modules node tools/pan-to-mp4.mjs \
//     /tmp/angelx-teaser shotB /tmp/angelx-answer-pan.mp4
//
// Renders the still through the same headless cell renderer teaser-to-mp4.mjs
// uses (see README, "Frame plates"), then rolls a window down it with ffmpeg's
// own per-frame crop expression, so the move is one continuous take with no
// intermediate PNGs. `pan-pace.mjs` sets the pace from the text under the
// window: the film reads at a moderate speed because the camera does.
import { mkdirSync, readFileSync, statSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import { join, resolve } from 'node:path'
import { createRequire } from 'node:module'
import { blockHeld, plan, readable, READ_WPM, wordsPerRow } from './pan-pace.mjs'

const require = createRequire(import.meta.url)
let chromium
try {
  ;({ chromium } = require('playwright-core'))
} catch {
  console.error('playwright-core not found — set NODE_PATH to a tree containing it')
  process.exit(2)
}

function parseArgs(argv) {
  const [srcArg, prefixArg = 'shotB', outArg = '/tmp/angelx-answer-pan.mp4'] = argv
  if (!srcArg) {
    console.error(
      'usage: pan-to-mp4.mjs <frames-dir> [prefix=shotB] [out.mp4]\n' +
        '  env: PAN_CROP_COLS PAN_WINDOW_ROWS PAN_FROM PAN_TO PAN_READ_ROWS PAN_BLOCK_ROWS PAN_TEXT_COLS',
    )
    process.exit(2)
  }
  const int = (name, fallback) => {
    const v = process.env[name]
    return v === undefined || v === '' ? fallback : Number(v)
  }
  const range = (name, fallback) =>
    String(process.env[name] ?? fallback)
      .split(':')
      .map(Number)
  const [readFrom, readTo] = range('PAN_READ_ROWS', '3:10') // what the shot asks the viewer to read
  const [blockFrom, blockTo] = range('PAN_BLOCK_ROWS', '5:10') // the response itself
  return {
    src: resolve(srcArg),
    prefix: prefixArg,
    out: resolve(outArg),
    cropCols: int('PAN_CROP_COLS', 72), // transcript column, incl. its left border
    windowRows: int('PAN_WINDOW_ROWS', 18), // 4:1 window over a 120x40 frame: the response fills half of it
    from: int('PAN_FROM', 0),
    to: int('PAN_TO', 6),
    readFrom,
    readTo,
    blockFrom,
    blockTo,
    textCols: range('PAN_TEXT_COLS', '12:70'), // where the words actually sit
  }
}

const COLS = 120,
  ROWS = 40
const PX = 12 // cell px, logical — the plates' grid
const DPR = 3 // device scale: the crop is a third of the frame, so render it sharp
const FPS = 30

const opt = parseArgs(process.argv.slice(2))

function parseColor(s) {
  // `Rgb(58, 111, 151)` | `Reset` | `Indexed(208)` | `CpuCyan` ...
  const m = /^Rgb\((\d+), (\d+), (\d+)\)$/.exec(s)
  if (m) return `rgb(${m[1]},${m[2]},${m[3]})`
  const i = /^Indexed\((\d+)\)$/.exec(s)
  if (i) return `rgb(${xterm256(+i[1]).join(',')})`
  if (/Cyan/i.test(s)) return 'rgb(88,166,255)'
  if (/Green/i.test(s)) return 'rgb(63,185,80)'
  if (/Magenta|Purple/i.test(s)) return 'rgb(188,140,255)'
  if (/Yellow/i.test(s)) return 'rgb(240,180,41)'
  if (/DarkGray|Gray/i.test(s)) return 'rgb(139,148,158)'
  if (/White|Light/i.test(s)) return 'rgb(232,230,227)'
  return 'rgb(232,230,227)' // Reset → theme fg
}
function xterm256(i) {
  if (i < 16) return [160, 160, 160]
  if (i < 232) {
    const c = i - 16,
      v = [0, 95, 135, 175, 215, 255]
    return [v[(c / 36) | 0], v[((c / 6) | 0) % 6], v[c % 6]]
  }
  const g = 8 + (i - 232) * 10
  return [g, g, g]
}

function frameDom(text, cells) {
  const lines = text.split('\n')
  let html = ''
  for (let y = 0; y < Math.min(lines.length, ROWS); y++) {
    const cps = [...lines[y]]
    let rowHtml = ''
    for (let i = 0, x = 0; i < cps.length && x < COLS; i++, x++) {
      const ch = cps[i]
      const cellStr = cells ? cells[y * COLS + x] : null
      const fg = cellStr ? parseColor(cellStr.split('|')[0]) : 'rgb(232,230,227)'
      if (ch === '\u{10EEEE}') {
        // Dotmax transport: private-use marker + two coordinate diacritics.
        i += 2
        rowHtml += `<span class="cell"><i class="d" style="background:${fg}"></i></span>`
      } else if (ch === ' ') {
        rowHtml += `<span class="cell"> </span>`
      } else
        rowHtml += `<span class="cell"><span class="g" style="color:${fg}">${ch === '<' ? '&lt;' : ch === '&' ? '&amp;' : ch}</span></span>`
    }
    html += `<div>${rowHtml}</div>`
  }
  return html
}

// ---- the still: the shot's frame, rendered once ---------------------------------
const still = join(opt.src, `${opt.prefix}-00.txt`)
const text = readFileSync(still, 'utf8').replace(/\n$/, '')
let cells = null
try {
  cells = readFileSync(still.replace('.txt', '.cells'), 'utf8').split('\n')
} catch {
  /* cells are optional; colors fall back to the theme foreground */
}
const lines = text.split('\n')

// Words the viewer is asked to read: the rows the camera starts on, columns the
// transcript text occupies. This is what sets the pace — see pan-pace.mjs.
const slice = (a, b) =>
  lines
    .slice(a, b + 1)
    .map((l) => l.slice(opt.textCols[0], opt.textCols[1]))
    .join(' ')
    .replace(/[│╭╮╰╯─┴┬┤├◇⠿⣿⡿⠟⠛]/gu, ' ')
    .trim()
const countWords = (s) => s.split(/\s+/).filter((t) => /[\p{L}\p{N}]/u.test(t)).length // an em dash between spaces is not a word
const words = countWords(slice(...[opt.readFrom, opt.readTo]))
const blockWords = countWords(slice(...[opt.blockFrom, opt.blockTo]))
const textRows = opt.readTo - opt.readFrom + 1

const cellPx = PX * DPR
const shot = plan({
  words,
  rows: opt.to - opt.from,
  windowRows: opt.windowRows,
  startRow: opt.from,
  cellPx,
  fps: FPS,
})
const fit = readable(shot, textRows, READ_WPM)
// A per-row check is not enough: the whole response has to stay in the frame
// while a moderate reader works through it, or the shot drifts out from under
// them however gentle the drift is.
const held = blockHeld(shot, {
  words: blockWords,
  blockFrom: opt.blockFrom,
  blockRows: opt.blockTo - opt.blockFrom + 1,
  wpm: READ_WPM,
})
const gateOk = fit.ok && held.ok

const W = COLS * PX,
  H = ROWS * PX
const cropW = opt.cropCols * cellPx
const cropH = opt.windowRows * cellPx
const outW = cropW,
  outH = cropH

console.log(
  `frame ${opt.prefix}-00 · ${COLS}x${ROWS} cells · window ${opt.cropCols}x${opt.windowRows} cells (${(outW / outH).toFixed(2)}:1)`,
)
console.log(
  `text ${words} words over ${textRows} rows → ${shot.readSeconds.toFixed(1)}s at ${READ_WPM} wpm → take ${shot.seconds}s (${shot.margin}x margin)`,
)
console.log(
  `pan rows ${shot.startRow}→${shot.endRow} (${shot.rows} rows = ${shot.rows * cellPx} px at ${cellPx} px/cell) · ` +
    `${shot.rowsPerSecond.toFixed(2)} rows/s · ${shot.secondsPerRow.toFixed(2)}s per row · ${shot.pxPerSecond.toFixed(1)} px/s · ${shot.pxPerFrame.toFixed(2)} px/frame`,
)
console.log(
  `readability: ${fit.havePerRow.toFixed(2)}s per row on screen vs ${fit.needPerRow.toFixed(2)}s to read it (${wordsPerRow(words, textRows).toFixed(1)} words/row) → ${fit.ok ? 'camera never outruns the reader' : 'TOO FAST'}`,
)
console.log(
  `hold: rows ${opt.blockFrom}–${opt.blockTo} (${blockWords} words) whole in frame for ${String(held.hold.toFixed(1))}s vs ${held.need.toFixed(1)}s to read + a beat → ${held.ok ? 'held' : held.fits ? 'DRIFTS OUT TOO SOON' : 'NEVER WHOLE (window shorter than the response)'}`,
)
if (!gateOk)
  console.error(
    'pan-to-mp4: this move is not readable — slow the pan, shorten the travel or lengthen the window',
  )
// PAN_DRY=1 stops here: the pace is the part worth checking in a gate, and it
// needs no browser and no encoder. Exit 3 says a moderate reader would be outrun.
if (process.env.PAN_DRY === '1') process.exit(gateOk ? 0 : 3)

// ---- render + encode ------------------------------------------------------------
const browser = await chromium.launch({
  executablePath: '/usr/bin/chromium',
  args: ['--no-sandbox', `--force-device-scale-factor=${DPR}`],
})
const page = await browser.newPage({ viewport: { width: W, height: H }, deviceScaleFactor: DPR })
await page.setContent(`<!doctype html><meta charset="utf-8"><style>
  html,body{margin:0;background:#0a0c10}
  #t{font:700 ${PX - 2}px/"${PX}px" ui-monospace,SFMono-Regular,Menlo,Monospace,monospace;
     white-space:pre;line-height:${PX}px;letter-spacing:0}
  .cell{display:inline-block;width:${PX}px;height:${PX}px;position:relative}
  span.g{position:absolute;left:0;top:0}
  i.d{position:absolute;left:${PX / 2 - 2}px;top:${PX / 2 - 2}px;width:4px;height:4px;border-radius:50%}
</style><div id="t"></div>`)
await page.evaluate(
  (html) => {
    document.getElementById('t').innerHTML = html
  },
  frameDom(text, cells),
)
mkdirSync(opt.src, { recursive: true })
const stillPng = join(opt.src, `${opt.prefix}-still.png`)
await page.screenshot({ path: stillPng, clip: { x: 0, y: 0, width: W, height: H } })
await browser.close()

// A constant pace is the point: easing would speed the middle of the answer up.
const yExpr = `${(opt.to - opt.from) * cellPx}*t/${shot.seconds}`
const vf = `crop=${outW}:${outH}:0:'${yExpr}',format=yuv420p`

execFileSync(
  'ffmpeg',
  [
    '-y',
    '-loop',
    '1',
    '-framerate',
    String(FPS),
    '-i',
    stillPng,
    '-t',
    String(shot.seconds),
    '-vf',
    vf,
    '-r',
    String(FPS),
    '-c:v',
    'libx264',
    '-preset',
    'medium',
    '-crf',
    '18',
    '-movflags',
    '+faststart',
    opt.out,
  ],
  { stdio: 'inherit' },
)

console.log(
  `OK ${opt.out} (${(statSync(opt.out).size / 1e6).toFixed(2)} MB, ${outW}x${outH} @${FPS}fps, ${shot.seconds}s, still ${stillPng})`,
)
