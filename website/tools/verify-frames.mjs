#!/usr/bin/env node
// Headless gate: every <img> on the page must load a real bitmap.
//   node tools/verify-frames.mjs http://127.0.0.1:PORT/index.html
import { createRequire } from 'node:module'
const require = createRequire(import.meta.url)
const { chromium } = require('playwright-core')
const url = process.argv[2]
if (!url) {
  console.error('usage: verify-frames.mjs <page-url>')
  process.exit(2)
}
// The plates index.html shows today: four frame plates plus the section backdrop.
// The constant is what stops a plate from disappearing quietly, so update it in
// the same change that adds or removes one.
const EXPECTED_PLATES = Number(process.env.PLATES ?? 5)
const browser = await chromium.launch({
  executablePath: '/usr/bin/chromium',
  args: ['--no-sandbox', '--disable-gpu'],
})
const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, deviceScaleFactor: 1 })
const failed = []
page.on('requestfailed', (r) => failed.push(r.url()))
await page.goto(url, { waitUntil: 'load' })
await page.evaluate(() =>
  Promise.all(
    [...document.images].map((i) =>
      i.complete
        ? null
        : new Promise((res) => {
            i.onload = res
            i.onerror = res
          }),
    ),
  ),
)
const imgs = await page.$$eval('img', (nodes) =>
  nodes.map((n) => ({ src: n.getAttribute('src'), w: n.naturalWidth, h: n.naturalHeight })),
)
const plates = imgs.filter(
  (i) => i.src && (i.src.includes('assets/frames/') || i.src.includes('background_lion')),
)
console.log(
  `images on page: ${imgs.length} · plates: ${plates.length} · failed requests: ${failed.length}`,
)
for (const i of plates) console.log(`  ${i.w > 0 ? 'OK ' : 'BROKEN'} ${i.src} ${i.w}x${i.h}`)
if (failed.length) console.log('failed:', failed.join(', '))
await page.close()
await browser.close()
const ok =
  plates.length === EXPECTED_PLATES &&
  plates.every((i) => i.w > 0 && i.h > 0) &&
  failed.length === 0
console.log(ok ? `GATE: PASS — ${EXPECTED_PLATES} plates loaded as real pixels` : 'GATE: FAIL')
process.exit(ok ? 0 : 1)
