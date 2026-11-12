import { mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { dirname, join, relative } from 'node:path'
import { fileURLToPath } from 'node:url'
import { renderMarkdown } from './markdown.mjs'

const here = dirname(fileURLToPath(import.meta.url))
const repo = dirname(here)
const origin = 'https://omni-space.tech'

export const DOC_SOURCES = [
  ['MODELS.md', 'Connect a model'],
  ['COMMANDS.md', 'Cockpit commands'],
  ['FEATURES.md', 'Implemented behavior, with source paths'],
  ['GLASS_TERM.md', 'The Glass terminal'],
  ['SHOWCASE.md', 'Pictures, video, pages and models over the cockpit'],
  ['HANDS.md', 'Desktop and browser on Omarchy'],
  ['BACKUP.md', 'Backup and peer watch'],
  ['GRAPH_GARDEN.md', 'Charts grown from real graph calls'],
  ['CARTRIDGES.md', 'Plug a tool or command in as a folder'],
  ['MEMORY.md', 'What the cockpit keeps'],
  ['LABYRINTH.md', 'Questions, evidence and refutations on one map'],
  ['RELEASE_0.2.86.md', 'angelX 0.2.86 release notes'],
  ['RELEASE_0.2.85.md', 'angelX 0.2.85 release notes'],
  ['RELEASE_0.2.7.md', 'angelX 0.2.7 release notes'],
]

const FURTHER = [
  ['Repository README', 'https://github.com/newjordan/angelX/blob/main/README.md'],
  ['The Delve', 'https://github.com/newjordan/angelX/blob/main/docs/DELVE.md'],
  ['Contributing', 'https://github.com/newjordan/angelX/blob/main/CONTRIBUTING.md'],
  ['Security', 'https://github.com/newjordan/angelX/blob/main/SECURITY.md'],
  ['Attributions', 'https://github.com/newjordan/angelX/blob/main/THIRD_PARTY_NOTICES.md'],
  ['MIT license', 'https://github.com/newjordan/angelX/blob/main/LICENSE'],
  ['Releases', 'https://github.com/newjordan/angelX/releases'],
]

const NAV = [
  ['/', 'Product', 'home'],
  ['/install.html', 'Install', 'install'],
  ['/docs/', 'Docs', 'docs'],
  ['/glass.html', 'Glass', 'glass'],
  ['/evidence.html', 'Evidence', 'evidence'],
  ['/company.html', 'Company', 'company'],
]

function docHref(file) {
  return `/docs/${file.replace(/\.md$/i, '').toLowerCase().replace(/_/g, '-')}.html`
}

function glassReel() {
  return `<figure class="reel" data-reel>
  <div class="reel-stage">
    <video class="reel-wide" controls playsinline preload="none" poster="/assets/glass-hero-16x9.jpg" width="1280" height="720" aria-label="Glass cockpit demonstration, 16 by 9, 12.8 seconds">
      <source src="/assets/glass-hero-16x9.mp4" type="video/mp4">
    </video>
    <video class="reel-tall" controls playsinline preload="none" poster="/assets/glass-hero-9x16.jpg" width="1080" height="1920" aria-label="Glass cockpit demonstration, 9 by 16, 12.8 seconds">
      <source src="/assets/glass-hero-9x16.mp4" type="video/mp4">
    </video>
  </div>
  <figcaption>Glass, angelX's GPU terminal, 12.8 seconds. A wide window plays the 16:9 cut. A narrow window plays the 9:16 cut of the same film. Use the controls to play, pause and seek. The other cut stays unloaded. Files: <a href="/assets/glass-hero-16x9.mp4">16:9</a> · <a href="/assets/glass-hero-9x16.mp4">9:16</a>.</figcaption>
  <script>
    (function () {
      var root = document.currentScript.parentNode
      var wide = root.querySelector('.reel-wide')
      var tall = root.querySelector('.reel-tall')
      var mq = window.matchMedia('(max-width: 760px)')
      function tune() {
        var show = mq.matches ? tall : wide
        var hide = mq.matches ? wide : tall
        show.preload = 'metadata'
        hide.preload = 'none'
        if (!hide.paused) hide.pause()
      }
      tune()
      if (mq.addEventListener) mq.addEventListener('change', tune)
    })()
  </script>
</figure>`
}

function nav(active) {
  const links = NAV.map(([href, label, key]) => {
    const current = key === active ? ' aria-current="page"' : ''
    return `<a href="${href}"${current}>${label}</a>`
  }).join('')
  return `<a class="skip" href="#content">Skip to content</a>
<header class="nav">
  <div class="wrap nav-in">
    <a class="wordmark" href="/" aria-label="omni-space.tech home"><span class="dot" aria-hidden="true"></span><span>omni-space<span class="tld">.tech</span></span></a>
    <nav class="nav-links" aria-label="Product">
      ${links}
      <a class="btn" href="https://github.com/newjordan/angelX" rel="noopener">GitHub</a>
    </nav>
  </div>
</header>`
}

function footer() {
  return `<footer class="foot">
  <div class="wrap foot-in">
    <span class="wordmark"><span class="dot" aria-hidden="true"></span><span>omni-space<span class="tld">.tech</span></span></span>
    <span>© 2026 OmniSpace Technologies LLC. angelX is a product of OmniSpace Technologies LLC.</span>
    <span><a href="/install.html">Install</a> · <a href="/docs/">Docs</a> · <a href="https://github.com/newjordan/angelX" rel="noopener">GitHub</a></span>
  </div>
</footer>`
}

function attr(text) {
  return String(text).replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;')
}

function page({ title, description, canonicalPath, active, body, jsonLd }) {
  const canonical = `${origin}${canonicalPath}`
  const ld = jsonLd
    ? `\n  <script type="application/ld+json">${JSON.stringify(jsonLd)}</script>`
    : ''
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${attr(title)}</title>
  <meta name="description" content="${attr(description)}">
  <link rel="canonical" href="${canonical}">
  <meta property="og:title" content="${attr(title)}">
  <meta property="og:description" content="${attr(description)}">
  <meta property="og:url" content="${canonical}">
  <meta property="og:type" content="website">
  <meta property="og:image" content="${origin}/assets/glass-hero-16x9.jpg">
  <meta name="theme-color" content="#07090d">
  <link rel="icon" href="/assets/favicon.svg" type="image/svg+xml">
  <link rel="stylesheet" href="/css/site.css">${ld}
</head>
<body>
${nav(active)}
<main id="content">
${body}
</main>
${footer()}
<script src="/js/site.js"></script>
</body>
</html>
`
}

function firstParagraph(markdown) {
  const lines = markdown.replace(/\r\n/g, '\n').split('\n')
  const buf = []
  let started = false
  for (const line of lines) {
    if (!started) {
      if (line.trim() === '' || line.startsWith('#')) continue
      started = true
    } else if (line.trim() === '') break
    if (started) buf.push(line.trim())
  }
  return buf
    .join(' ')
    .replace(/`([^`]*)`/g, '$1')
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    .replace(/[*_]+/g, '')
}

function docsIndex(rendered) {
  const cards = DOC_SOURCES.map(([file, blurb]) => {
    const text = firstParagraph(rendered.get(file).markdown)
    const summary = text.length > 180 ? `${text.slice(0, 177).trim()}…` : text
    return `<a class="card" href="${docHref(file)}"><span class="n">${blurb}</span><h3>${escapeText(rendered.get(file).title)}</h3><p>${escapeText(summary)}</p></a>`
  }).join('\n')
  const more = FURTHER.map(
    ([label, href]) => `<li><a href="${href}" rel="noopener">${label}</a></li>`,
  ).join('\n')
  return `<section class="hero"><div class="wrap">
    <p class="eyebrow">Documentation</p>
    <h1>angelX docs</h1>
    <p class="lede">These pages are the current guides from the angelX repository, so install and everyday use do not depend on another website. Each page links to its source file.</p>
  </div></section>
  <section class="section"><div class="wrap">
    <div class="docs-grid">${cards}</div>
    <h2>Also in the repository</h2>
    <ul>${more}</ul>
  </div></section>`
}

function escapeText(text) {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

function readDocs() {
  const renderedNames = new Set(DOC_SOURCES.map(([file]) => file))
  const out = new Map()
  for (const [file, blurb] of DOC_SOURCES) {
    const markdown = readFileSync(join(repo, 'docs', file), 'utf8')
    const titleLine = markdown.split('\n').find((line) => line.startsWith('# '))
    const title = titleLine ? titleLine.replace(/^#\s+/, '').trim() : file
    const html = renderMarkdown(markdown, { file: `docs/${file}`, rendered: renderedNames })
    out.set(file, { markdown, title, blurb, html })
  }
  return out
}

const HOME_DESCRIPTION =
  'angelX is a terminal coding agent. Install it, read the docs, and watch the Glass reel on omni-space.tech. Source stays on GitHub.'

export function renderSite() {
  const docs = readDocs()
  const files = new Map()
  const fragment = (name) =>
    readFileSync(join(here, 'content', name), 'utf8').replaceAll('{{GLASS_REEL}}', glassReel())

  const homeLd = {
    '@context': 'https://schema.org',
    '@type': 'SoftwareApplication',
    name: 'angelX',
    applicationCategory: 'DeveloperApplication',
    operatingSystem: 'Linux, macOS',
    softwareVersion: '0.2.7',
    url: `${origin}/`,
    codeRepository: 'https://github.com/newjordan/angelX',
    license: 'https://spdx.org/licenses/MIT.html',
    description: HOME_DESCRIPTION,
    publisher: {
      '@type': 'Organization',
      name: 'OmniSpace Technologies LLC',
      url: `${origin}/`,
      email: 'jordan@omnispacetechnologies.com',
    },
  }

  const staticPages = [
    ['index.html', '/', 'home', 'angelX — the terminal coding agent', HOME_DESCRIPTION, 'index.html', homeLd],
    [
      'install.html',
      '/install.html',
      'install',
      'Install angelX',
      'Install angelX with one command, or build it from the GitHub source. Platforms, checksums, glass, and update.',
      'install.html',
    ],
    [
      'glass.html',
      '/glass.html',
      'glass',
      'Glass — angelX terminal reel',
      'The 12.8 second Glass cockpit reel, in 16:9 and 9:16, served from omni-space.tech with controls.',
      'glass.html',
    ],
    [
      'evidence.html',
      '/evidence.html',
      'evidence',
      'angelX evidence',
      'Published angelX measurements from the repository README and the 0.2.7 release notes, with their settings and limits.',
      'evidence.html',
    ],
    [
      'company.html',
      '/company.html',
      'company',
      'OmniSpace Technologies',
      'OmniSpace Technologies LLC builds angelX. The product, its install and its docs are on this website.',
      'company.html',
    ],
  ]

  for (const [out, path, active, title, description, source, jsonLd] of staticPages) {
    files.set(
      out,
      page({
        title,
        description,
        canonicalPath: path,
        active,
        body: fragment(source),
        jsonLd,
      }),
    )
  }

  files.set(
    'docs/index.html',
    page({
      title: 'angelX docs',
      description: 'Install, models, commands, Glass, hands, backup and the 0.2.7 notes, rendered from the angelX repository.',
      canonicalPath: '/docs/',
      active: 'docs',
      body: docsIndex(docs),
    }),
  )

  for (const [file, doc] of docs) {
    const href = docHref(file)
    const plainTitle = doc.title.replace(/[`*]/g, '')
    files.set(
      href.slice(1),
      page({
        title: `${plainTitle} — angelX docs`,
        description: `${doc.blurb}. Rendered from docs/${file} in the angelX repository.`,
        canonicalPath: href,
        active: 'docs',
        body: `<section class="section"><div class="wrap prose">
<p class="crumbs"><a href="/docs/">Docs</a> · <a href="https://github.com/newjordan/angelX/blob/main/docs/${file}" rel="noopener">docs/${file}</a></p>
${doc.html}
</div></section>`,
      }),
    )
  }

  const urls = [...files.keys()].map((file) => {
    if (file === 'index.html') return `${origin}/`
    if (file === 'docs/index.html') return `${origin}/docs/`
    return `${origin}/${file}`
  })
  files.set(
    'sitemap.xml',
    `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls
      .map((url) => `  <url><loc>${url}</loc></url>`)
      .join('\n')}\n</urlset>\n`,
  )
  files.set('robots.txt', `User-agent: *\nAllow: /\nSitemap: ${origin}/sitemap.xml\n`)
  return files
}

function writeAll(files, root) {
  for (const [file, contents] of files) {
    const target = join(root, file)
    mkdirSync(dirname(target), { recursive: true })
    writeFileSync(target, contents.endsWith('\n') ? contents : `${contents}\n`)
  }
}

function listHtml(dir, prefix = '') {
  const found = []
  for (const name of readdirSync(dir)) {
    if (name === 'content') continue
    const abs = join(dir, name)
    const rel = prefix ? `${prefix}/${name}` : name
    if (statSync(abs).isDirectory()) found.push(...listHtml(abs, rel))
    else if (name.endsWith('.html') || name === 'sitemap.xml' || name === 'robots.txt') found.push(rel)
  }
  return found
}

export function checkSite() {
  const expected = renderSite()
  const problems = []
  for (const [file, contents] of expected) {
    const normalized = contents.endsWith('\n') ? contents : `${contents}\n`
    const diskPath = join(here, file)
    let disk = ''
    try {
      disk = readFileSync(diskPath, 'utf8')
    } catch {
      problems.push(`missing ${file}`)
      continue
    }
    if (disk !== normalized) problems.push(`stale ${file}`)
  }
  for (const file of listHtml(here)) {
    if (!expected.has(file)) problems.push(`extra ${file}`)
  }
  return problems
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  if (process.argv.includes('--check')) {
    const problems = checkSite()
    if (problems.length) {
      console.error(problems.join('\n'))
      process.exit(1)
    }
    console.log(`website matches build (${renderSite().size} files)`)
  } else {
    const files = renderSite()
    writeAll(files, here)
    console.log(`wrote ${files.size} website files`)
  }
}
