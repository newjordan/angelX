import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'
import { checkSite } from '../../website/build.mjs'

const root = fileURLToPath(new URL('../..', import.meta.url))
const site = join(root, 'website')

const BANNED = [
  /angelx\.dev/i,
  /5\.01/,
  /342 records/,
  /HackerOne/i,
  /Two sites/,
  /noexcuses/i,
  /\$100,000/,
  /pre-Series/i,
  /24-month/i,
  /24 months/i,
]

function walk(dir, acc = []) {
  for (const name of readdirSync(dir)) {
    if (name === 'content') continue
    const abs = join(dir, name)
    if (statSync(abs).isDirectory()) walk(abs, acc)
    else acc.push(abs)
  }
  return acc
}

function pages() {
  return walk(site).filter((file) => file.endsWith('.html'))
}

test('generated pages match website/build.mjs', () => {
  assert.deepEqual(checkSite(), [])
})

test('every page is the omni-space.tech product, with working navigation', () => {
  const files = walk(site).filter((file) => /\.(html|css|js|mjs|md|json|txt|xml)$/.test(file))
  for (const file of files) {
    const text = readFileSync(file, 'utf8')
    for (const pattern of BANNED) {
      assert.equal(pattern.test(text), false, `${file} matched ${pattern}`)
    }
  }
  for (const file of pages()) {
    const text = readFileSync(file, 'utf8')
    assert.match(text, /rel="canonical" href="https:\/\/omni-space\.tech/)
    assert.match(text, /aria-label="Product"/)
    assert.match(text, /href="\/install\.html"/)
    assert.match(text, /href="\/docs\/"/)
    assert.match(text, /href="\/glass\.html"/)
    assert.match(text, /href="\/evidence\.html"/)
    assert.doesNotMatch(text, /http-equiv="refresh"/i)
  }
  const readme = readFileSync(join(root, 'README.md'), 'utf8')
  assert.match(readme, /https:\/\/omni-space\.tech/)
})

test('install and evidence follow the repository, including retained failures', () => {
  const install = readFileSync(join(site, 'install.html'), 'utf8')
  const installer = readFileSync(join(root, 'install.sh'), 'utf8')
  assert.match(
    installer,
    /curl -fsSL https:\/\/raw\.githubusercontent\.com\/newjordan\/angelX\/main\/install\.sh \| sh/,
  )
  assert.match(
    install,
    /curl -fsSL https:\/\/raw\.githubusercontent\.com\/newjordan\/angelX\/main\/install\.sh \| sh/,
  )
  assert.match(install, /git clone https:\/\/github\.com\/newjordan\/angelX\.git/)
  assert.match(install, /\.\/bin\/angelX/)
  assert.match(install, /glibc 2\.31/)
  assert.match(install, /ANGEL_FAST=0/)
  assert.match(install, /ANGEL_NO_BUILD=1/)
  assert.match(install, /ANGEL_BUILD_BACKEND=blaze/)
  assert.match(install, /https:\/\/github\.com\/newjordan\/angelX\/releases/)
  const evidence = readFileSync(join(site, 'evidence.html'), 'utf8')
  for (const cell of [
    '136 / 136',
    '37.2 min',
    '45.6 min',
    '135 / 136',
    '86 / 93',
    '46.1 ms',
    '3.0 ms',
  ]) {
    assert.ok(evidence.includes(cell), cell)
  }
  assert.match(evidence, /100\+ records on Yukon public research leaderboards/)
  assert.match(
    readFileSync(join(root, 'README.md'), 'utf8'),
    /100\+ records on Yukon public research leaderboards/,
  )
  assert.match(readFileSync(join(site, 'docs/models.html'), 'utf8'), /Type <code>\/setup<\/code>/)
  assert.match(readFileSync(join(site, 'docs/commands.html'), 'utf8'), /id="walking-the-map"/)
  assert.match(readFileSync(join(site, 'docs/features.html'), 'utf8'), /benchmark_compare/)
})

test('Glass reels are the approved files and the player is responsive', () => {
  const manifest = JSON.parse(readFileSync(join(site, 'assets/glass-hero.json'), 'utf8'))
  for (const cut of [manifest.wide, manifest.tall]) {
    const bytes = readFileSync(join(site, 'assets', cut.file))
    assert.equal(createHash('sha256').update(bytes).digest('hex'), cut.sha256)
    assert.equal(cut.duration, 12.8)
    assert.ok(statSync(join(site, 'assets', cut.poster)).size > 1000)
  }
  for (const name of ['index.html', 'glass.html']) {
    const html = readFileSync(join(site, name), 'utf8')
    assert.match(html, /controls/)
    assert.match(html, /playsinline/)
    assert.match(html, /preload="none"/)
    assert.match(html, /show\.preload = 'metadata'/)
    assert.match(html, /max-width: 760px/)
    assert.match(html, /\/assets\/glass-hero-16x9\.mp4/)
    assert.match(html, /\/assets\/glass-hero-9x16\.mp4/)
    assert.match(html, /aria-label="Glass cockpit demonstration, 16 by 9, 12\.8 seconds"/)
  }
  const probe = spawnSync(
    'ffprobe',
    [
      '-v',
      'error',
      '-select_streams',
      'v:0',
      '-show_entries',
      'stream=width,height',
      '-show_entries',
      'format=duration',
      '-of',
      'json',
      join(site, 'assets', manifest.wide.file),
    ],
    { encoding: 'utf8' },
  )
  if (probe.status === 0) {
    const parsed = JSON.parse(probe.stdout)
    assert.equal(parsed.streams[0].width, 1280)
    assert.equal(parsed.streams[0].height, 720)
    assert.ok(Math.abs(Number(parsed.format.duration) - 12.8) < 0.05)
  }
})

test('internal links, anchors and doc figures resolve', () => {
  const htmlFiles = pages()
  function target(from, href) {
    const bare = href.split('#')[0]
    const hash = href.includes('#') ? href.slice(href.indexOf('#') + 1) : ''
    if (!bare || bare === '/') return { file: bare === '/' ? join(site, 'index.html') : from, hash }
    const rel = bare.startsWith('/') ? bare.slice(1) : null
    const path = rel === null ? join(from, '..', bare) : join(site, rel)
    const file = path.endsWith('/') || rel?.endsWith('/') ? join(path, 'index.html') : path
    return { file, hash }
  }
  for (const file of htmlFiles) {
    const text = readFileSync(file, 'utf8')
    const hrefs = [...text.matchAll(/href="([^"]+)"/g)].map((match) => match[1])
    for (const href of hrefs) {
      if (/^(https?:|mailto:)/.test(href)) continue
      const { file: dest, hash } = target(file, href)
      assert.equal(
        statSync(dest, { throwIfNoEntry: false })?.isFile(),
        true,
        `${file} -> ${href} (${dest})`,
      )
      if (hash && dest.endsWith('.html')) {
        const page = readFileSync(dest, 'utf8')
        assert.ok(page.includes(`id="${hash}"`), `${href} missing in ${dest}`)
      }
    }
    for (const src of text.matchAll(/(?:src|srcset)="([^"]+)"/g)) {
      const href = src[1]
      if (/^https?:/.test(href)) continue
      const { file: dest } = target(file, href)
      assert.ok(statSync(dest, { throwIfNoEntry: false })?.isFile(), `${file} asset ${href}`)
    }
  }
  const imageRoot = join(site, 'assets/docs-images')
  const copies = walk(imageRoot)
  assert.ok(copies.length >= 12)
  for (const copy of copies) {
    const rel = copy.slice(imageRoot.length + 1)
    const left = createHash('sha256')
      .update(readFileSync(join(root, 'docs/images', rel)))
      .digest('hex')
    const right = createHash('sha256').update(readFileSync(copy)).digest('hex')
    assert.equal(left, right, rel)
  }
})

test('the linked Vercel project still skips, so the older hostname is untouched', () => {
  const config = JSON.parse(readFileSync(join(root, 'vercel.json'), 'utf8'))
  const result = spawnSync('sh', ['-c', config.ignoreCommand], { encoding: 'utf8' })
  assert.equal(result.status, 0)
  const siteConfig = readFileSync(join(site, 'vercel.json'), 'utf8')
  assert.doesNotMatch(siteConfig, /redirects/)
  assert.match(readFileSync(join(root, 'docs/site-rollout.md'), 'utf8'), /vercel deploy --prod/)
  assert.match(readFileSync(join(root, 'docs/site-rollout.md'), 'utf8'), /Do not pass/)
})
