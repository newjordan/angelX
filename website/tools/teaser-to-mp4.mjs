#!/usr/bin/env node
// teaser-to-mp4.mjs — assemble the teaser shot plates into a slow, looping MP4.
//   node tools/teaser-to-mp4.mjs /tmp/angelx-teaser assets/teaser.mp4
// Renders each .txt/.cells pair through a headless browser terminal renderer,
// holds and crossfades the shots, encodes a ~24s seamless loop with ffmpeg.
import { readdirSync, readFileSync, mkdirSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import { join, resolve } from 'node:path'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
let chromium
try {
  ;({ chromium } = require('playwright-core'))
} catch {
  console.error('playwright-core not found — set NODE_PATH to a tree containing it')
  process.exit(2)
}

const [srcArg, outArg = 'assets/teaser.mp4'] = process.argv.slice(2)
if (!srcArg) {
  console.error('usage: teaser-to-mp4.mjs <frames-dir> [out.mp4]')
  process.exit(2)
}
const src = resolve(srcArg)
const out = resolve(outArg)

const COLS = 120,
  ROWS = 40
const PX = 12 // cell px — 1440x960 canvas
const W = COLS * PX,
  H = ROWS * PX
const FPS = 30
const XFADE = 0.8 // crossfade seconds between shots

function load(prefix) {
  const names = readdirSync(src)
    .filter((f) => f.startsWith(prefix) && f.endsWith('.txt'))
    .sort((a, b) => +a.match(/-(\d+)/)[1] - +b.match(/-(\d+)/)[1])
  if (!names.length) return []
  return names.map((n) => ({
    text: readFileSync(join(src, n), 'utf8').replace(/\n$/, ''),
    cells: (() => {
      try {
        return readFileSync(join(src, n.replace('.txt', '.cells')), 'utf8').split('\n')
      } catch {
        return null
      }
    })(),
  }))
}

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

// Build the browser renderer: draw every frame of a shot as one wide strip,
// then screenshot each frame region at high fidelity (real font, real colors).
const browser = await chromium.launch({
  executablePath: '/usr/bin/chromium',
  args: ['--no-sandbox', '--force-device-scale-factor=2'],
})
const page = await browser.newPage({ viewport: { width: W, height: H }, deviceScaleFactor: 2 })
await page.setContent(`<!doctype html><meta charset="utf-8"><style>
  html,body{margin:0;background:#0a0c10}
  #t{font:700 ${PX - 2}px/"${PX}px" ui-monospace,SFMono-Regular,Menlo,Monospace,monospace;
     white-space:pre;line-height:${PX}px;letter-spacing:0}
  .cell{display:inline-block;width:${PX}px;height:${PX}px;position:relative}
  span.g{position:absolute;left:0;top:0}
  i.d{position:absolute;left:${PX / 2 - 2}px;top:${PX / 2 - 2}px;width:4px;height:4px;border-radius:50%}
</style><div id="t"></div>`)

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
        // The live terminal hook rasterizes these to 1px dots — do the same.
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

async function shotPngs(prefix) {
  const frames = load(prefix)
  const pngs = []
  for (const f of frames) {
    await page.evaluate(
      (html) => {
        document.getElementById('t').innerHTML = html
      },
      frameDom(f.text, f.cells),
    )
    // dedupe: skip if text identical to previous (already-rendered png reused)
    const buf = await page.screenshot({ type: 'png', clip: { x: 0, y: 0, width: W, height: H } })
    pngs.push(buf)
    if (f.text === frames[frames.indexOf(f) - 1]?.text)
      pngs[pngs.length - 1] = pngs[pngs.length - 2]
  }
  return pngs
}

console.log('rendering shots in chromium…')
const shots = []
for (const p of ['shotA-', 'shotB-', 'shotC-', 'shotD-']) {
  const pngs = await shotPngs(p)
  if (pngs.length) shots.push({ prefix: p, pngs })
  console.log(`  ${p} ${pngs.length} frames`)
}
await browser.close()

// ---- sequence plan: one PNG dir per shot; crossfades happen at encode ----
// Target ~22s: slow typing, then long deliberate holds, 0.8s xfades between.
const tmp = '/tmp/teaser-png'
rmSync(tmp, { recursive: true, force: true })
const TYPE = 15 // frames per typing state (~2 chars/sec)
const TAIL = 45 // hold after the typing settles
const HOLD = 150 // frames held on each showcase shot (~5s)
const XFD = 0.8 // crossfade seconds

const plan = []
for (const s of shots) {
  const pngs = s.pngs
  let frames
  if (s.prefix === 'shotA-') {
    const uniq = []
    for (const p of pngs) if (!uniq.includes(p)) uniq.push(p)
    frames = []
    for (const u of uniq) for (let k = 0; k < TYPE; k++) frames.push(u)
    for (let k = 0; k < TAIL; k++) frames.push(uniq[uniq.length - 1])
  } else {
    frames = Array.from({ length: HOLD }, () => pngs[pngs.length - 1])
  }
  const dir = join(tmp, s.prefix.replace(/-$/, ''))
  mkdirSync(dir, { recursive: true })
  frames.forEach((buf, i) => writeFileSync(join(dir, `f${String(i).padStart(5, '0')}.png`), buf))
  plan.push({ dir, dur: frames.length / FPS })
  console.log(`  ${s.prefix} ${frames.length} frames (${(frames.length / FPS).toFixed(1)}s)`)
}

// ---- encode: chained xfades between shots, gentle fade in/out for the loop ----
const args = ['-y']
for (const p of plan) args.push('-framerate', String(FPS), '-i', join(p.dir, 'f%05d.png'))
const fades = plan.length - 1
const total = plan.reduce((a, p) => a + p.dur, 0) - fades * XFD

let chain = ''
let prev = '[0:v]setsar=1[s0]'
{
  let cur = '[s0]'
  let acc = 0
  for (let i = 0; i < fades; i++) {
    acc += plan[i].dur
    const off = Math.max(0, acc - (i + 1) * XFD)
    const next = `[s${i + 1}]`
    chain += `${cur}[${i + 1}:v]xfade=transition=fade:duration=${XFD}:offset=${off.toFixed(3)}${next};`
    cur = next
  }
  chain += `${cur}fade=t=in:st=0:d=0.6,fade=t=out:st=${(total - 0.6).toFixed(2)}:d=0.6,format=yuv420p[v]`
  void prev
}

args.push(
  '-filter_complex',
  chain,
  '-map',
  '[v]',
  '-c:v',
  'libx264',
  '-preset',
  'medium',
  '-crf',
  '18',
  '-movflags',
  '+faststart',
  out,
)
execFileSync('ffmpeg', args, { stdio: 'inherit' })
console.log(
  `OK ${out} (${(statSync(out).size / 1e6).toFixed(1)} MB, ${total.toFixed(1)}s @${FPS}fps)`,
)
