// A small CommonMark subset for the public docs rendered onto omni-space.tech.
// It covers the constructs those guides actually use: headings, lists, tables,
// fenced code, blockquotes, links, images and emphasis.

const DOCS_IMAGE_PREFIX = '/assets/docs-images/'
const GITHUB_BLOB = 'https://github.com/newjordan/angelX/blob/main/'
const GITHUB_RAW = 'https://github.com/newjordan/angelX/raw/main/'

export function slugify(raw) {
  const plain = String(raw)
    .replace(/`([^`]*)`/g, '$1')
    .replace(/!\[[^\]]*\]\([^)]*\)/g, '')
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    .replace(/[*_]+/g, '')
    .toLowerCase()
    .normalize('NFKD')
    .replace(/[\u0300-\u036f]/g, '')
  return plain
    .replace(/[^\p{L}\p{N}\s-]/gu, '')
    .trim()
    .replace(/\s+/g, '-')
    .replace(/-+/g, '-')
}

function escapeHtml(text) {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

function pageSlug(fileName) {
  return fileName.replace(/\.md$/i, '').toLowerCase().replace(/_/g, '-')
}

/**
 * @param {string} url
 * @param {{ file: string, rendered: Set<string> }} ctx
 */
export function rewriteUrl(url, ctx) {
  if (!url || /^(https?:|mailto:|data:)/i.test(url)) return url
  const hashIndex = url.indexOf('#')
  const hash = hashIndex === -1 ? '' : url.slice(hashIndex)
  const path = hashIndex === -1 ? url : url.slice(0, hashIndex)
  if (!path) return hash || url
  const fromDir = ctx.file.includes('/') ? ctx.file.slice(0, ctx.file.lastIndexOf('/') + 1) : ''
  const parts = (fromDir + path).split('/')
  const stack = []
  for (const part of parts) {
    if (part === '' || part === '.') continue
    if (part === '..') stack.pop()
    else stack.push(part)
  }
  const resolved = stack.join('/')
  if (resolved.startsWith('docs/images/')) {
    return `${DOCS_IMAGE_PREFIX}${resolved.slice('docs/images/'.length)}${hash}`
  }
  if (resolved.endsWith('.md')) {
    const base = resolved.slice(resolved.lastIndexOf('/') + 1)
    if (ctx.rendered.has(base) && resolved.startsWith('docs/')) {
      return `/docs/${pageSlug(base)}.html${hash}`
    }
  }
  const image = /\.(png|jpg|jpeg|gif|svg|webp)$/i.test(resolved)
  return `${image ? GITHUB_RAW : GITHUB_BLOB}${resolved}${hash}`
}

function inline(text, ctx) {
  let out = ''
  let i = 0
  while (i < text.length) {
    if (text.startsWith('<http://', i) || text.startsWith('<https://', i)) {
      const end = text.indexOf('>', i)
      if (end !== -1) {
        const href = text.slice(i + 1, end)
        out += `<a href="${escapeHtml(href)}">${escapeHtml(href)}</a>`
        i = end + 1
        continue
      }
    }
    if (text[i] === '`') {
      const end = text.indexOf('`', i + 1)
      if (end !== -1) {
        out += `<code>${escapeHtml(text.slice(i + 1, end))}</code>`
        i = end + 1
        continue
      }
    }
    if (text.startsWith('**', i) || text.startsWith('__', i)) {
      const marker = text.slice(i, i + 2)
      const end = text.indexOf(marker, i + 2)
      if (end !== -1) {
        out += `<strong>${inline(text.slice(i + 2, end), ctx)}</strong>`
        i = end + 2
        continue
      }
    }
    if (text[i] === '!' && text[i + 1] === '[') {
      const close = text.indexOf(']', i + 2)
      if (close !== -1 && text[close + 1] === '(') {
        const urlEnd = text.indexOf(')', close + 2)
        if (urlEnd !== -1) {
          const alt = text.slice(i + 2, close)
          const url = text.slice(close + 2, urlEnd).trim()
          out += `<img src="${escapeHtml(rewriteUrl(url, ctx))}" alt="${escapeHtml(alt)}">`
          i = urlEnd + 1
          continue
        }
      }
    }
    if (text[i] === '[') {
      const close = text.indexOf(']', i + 1)
      if (close !== -1 && text[close + 1] === '(') {
        const urlEnd = text.indexOf(')', close + 2)
        if (urlEnd !== -1) {
          const label = text.slice(i + 1, close)
          const url = text.slice(close + 2, urlEnd).trim()
          const href = rewriteUrl(url, ctx)
          const external = /^(https?:|mailto:)/i.test(href)
          const rel = external ? ' rel="noopener"' : ''
          out += `<a href="${escapeHtml(href)}"${rel}>${inline(label, ctx)}</a>`
          i = urlEnd + 1
          continue
        }
      }
    }
    if ((text[i] === '*' || text[i] === '_') && text[i + 1] !== text[i]) {
      const marker = text[i]
      const wordBefore = i > 0 && /[\p{L}\p{N}]/u.test(text[i - 1])
      if (!(marker === '_' && wordBefore)) {
        const end = text.indexOf(marker, i + 1)
        const wordAfter = end !== -1 && end + 1 < text.length && /[\p{L}\p{N}]/u.test(text[end + 1])
        if (end > i + 1 && !(marker === '_' && wordAfter)) {
          out += `<em>${inline(text.slice(i + 1, end), ctx)}</em>`
          i = end + 1
          continue
        }
      }
    }
    if (text.startsWith('https://', i) || text.startsWith('http://', i)) {
      const match = /^(https?:\/\/[^\s<]+)/.exec(text.slice(i))
      if (match) {
        let href = match[1].replace(/[),.;:]+$/, '')
        const trailing = match[1].slice(href.length)
        out += `<a href="${escapeHtml(href)}" rel="noopener">${escapeHtml(href)}</a>${escapeHtml(trailing)}`
        i += match[1].length
        continue
      }
    }
    out += escapeHtml(text[i])
    i += 1
  }
  return out
}

function isFence(line) {
  return /^```/.test(line)
}

function isHeading(line) {
  return /^#{1,6}\s+/.test(line)
}

function isHr(line) {
  return /^(\*\s*\*\s*\*|-{3,}|_{3,})\s*$/.test(line)
}

function isQuote(line) {
  return /^>\s?/.test(line)
}

function listMatch(line) {
  return /^(\s*)([-*+]|\d+\.)\s+(.*)$/.exec(line)
}

function isSeparator(line) {
  return /^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?\s*$/.test(line)
}

function isPipeRow(line) {
  const trimmed = line.trim()
  return trimmed.startsWith('|') && trimmed.endsWith('|') && trimmed.split('|').length >= 4
}

function isTableStart(lines, index) {
  if (!isPipeRow(lines[index]) || index + 1 >= lines.length) return false
  return isSeparator(lines[index + 1]) || isPipeRow(lines[index + 1])
}

function isHtmlBlock(line) {
  return /^<\/?[A-Za-z][\w:-]*(\s[^<>]*)?>?\s*$/.test(line.trim())
}

function isBlockStart(lines, index) {
  const line = lines[index]
  if (line === undefined) return false
  if (line.trim() === '') return false
  return (
    isFence(line) ||
    isHeading(line) ||
    isHr(line) ||
    isQuote(line) ||
    listMatch(line) ||
    isTableStart(lines, index) ||
    isHtmlBlock(line)
  )
}

function splitRow(line) {
  let raw = line.trim()
  if (raw.startsWith('|')) raw = raw.slice(1)
  if (raw.endsWith('|')) raw = raw.slice(0, -1)
  const cells = []
  let current = ''
  for (let i = 0; i < raw.length; i += 1) {
    if (raw[i] === '\\' && raw[i + 1] === '|') {
      current += '|'
      i += 1
      continue
    }
    if (raw[i] === '|') {
      cells.push(current.trim())
      current = ''
      continue
    }
    current += raw[i]
  }
  cells.push(current.trim())
  return cells
}

function parseTable(lines, index, ctx) {
  const separated = isSeparator(lines[index + 1] || '')
  const header = separated ? splitRow(lines[index]) : null
  const align = separated
    ? splitRow(lines[index + 1]).map((cell) => {
        const left = cell.startsWith(':')
        const right = cell.endsWith(':')
        if (left && right) return 'center'
        if (right) return 'right'
        return 'left'
      })
    : []
  const body = []
  let cursor = separated ? index + 2 : index
  while (cursor < lines.length && isPipeRow(lines[cursor])) {
    body.push(splitRow(lines[cursor]))
    cursor += 1
  }
  const cell = (text, tag, column) => {
    const style = align[column] && align[column] !== 'left' ? ` style="text-align:${align[column]}"` : ''
    return `<${tag}${style}>${inline(text, ctx)}</${tag}>`
  }
  const head = header
    ? `<thead><tr>${header.map((text, column) => cell(text, 'th', column)).join('')}</tr></thead>`
    : ''
  const rows = body
    .map((row) => `<tr>${row.map((text, column) => cell(text, 'td', column)).join('')}</tr>`)
    .join('')
  return { html: `<div class="table-wrap"><table>${head}<tbody>${rows}</tbody></table></div>`, index: cursor }
}

function parseList(lines, index, minIndent, ctx) {
  const first = listMatch(lines[index])
  const ordered = /^\d+\.$/.test(first[2])
  const items = []
  while (index < lines.length) {
    if (lines[index].trim() === '') {
      let look = index + 1
      while (look < lines.length && lines[look].trim() === '') look += 1
      const next = listMatch(lines[look] || '')
      if (next && next[1].length === minIndent && /^\d+\.$/.test(next[2]) === ordered) {
        index = look
      } else break
    }
    const match = listMatch(lines[index])
    if (!match || match[1].length !== minIndent || /^\d+\.$/.test(match[2]) !== ordered) break
    const indent = match[1].length
    const chunks = [match[3]]
    index += 1
    const nested = []
    while (index < lines.length) {
      if (lines[index].trim() === '') break
      const nestedMatch = listMatch(lines[index])
      if (nestedMatch && nestedMatch[1].length > indent) {
        const child = parseList(lines, index, nestedMatch[1].length, ctx)
        nested.push(child.html)
        index = child.index
        continue
      }
      if (nestedMatch && nestedMatch[1].length <= indent) break
      if (isFence(lines[index]) || isHeading(lines[index]) || isTableStart(lines, index)) break
      if (/^\s+/.test(lines[index]) && lines[index].search(/\S/) > indent) {
        chunks.push(lines[index].trim())
        index += 1
        continue
      }
      break
    }
    items.push(`<li><p>${inline(chunks.join(' '), ctx)}</p>${nested.join('')}</li>`)
  }
  const tag = ordered ? 'ol' : 'ul'
  return { html: `<${tag}>${items.join('')}</${tag}>`, index }
}

function parseBlocks(lines, ctx) {
  const blocks = []
  const usedIds = new Map()
  let index = 0
  while (index < lines.length) {
    const line = lines[index]
    if (line.trim() === '') {
      index += 1
      continue
    }
    if (isFence(line)) {
      const lang = line.slice(3).trim()
      const body = []
      index += 1
      while (index < lines.length && !isFence(lines[index])) {
        body.push(lines[index])
        index += 1
      }
      if (index < lines.length) index += 1
      const className = lang ? ` class="language-${escapeHtml(lang.split(/\s+/)[0])}"` : ''
      blocks.push(`<pre><code${className}>${escapeHtml(body.join('\n'))}</code></pre>`)
      continue
    }
    if (isHeading(line)) {
      const level = line.match(/^#+/)[0].length
      const text = line.replace(/^#{1,6}\s+/, '').trim()
      let id = slugify(text) || `section-${blocks.length}`
      const seen = usedIds.get(id) || 0
      usedIds.set(id, seen + 1)
      if (seen > 0) id = `${id}-${seen}`
      blocks.push(`<h${level} id="${id}">${inline(text, ctx)}</h${level}>`)
      index += 1
      continue
    }
    if (isHr(line)) {
      blocks.push('<hr>')
      index += 1
      continue
    }
    if (isQuote(line)) {
      const quote = []
      while (index < lines.length && isQuote(lines[index])) {
        quote.push(lines[index].replace(/^>\s?/, ''))
        index += 1
      }
      blocks.push(`<blockquote><p>${inline(quote.join(' '), ctx)}</p></blockquote>`)
      continue
    }
    if (listMatch(line)) {
      const parsed = parseList(lines, index, listMatch(line)[1].length, ctx)
      blocks.push(parsed.html)
      index = parsed.index
      continue
    }
    if (isTableStart(lines, index)) {
      const parsed = parseTable(lines, index, ctx)
      blocks.push(parsed.html)
      index = parsed.index
      continue
    }
    if (isHtmlBlock(line)) {
      const html = []
      while (index < lines.length && lines[index].trim() !== '' && /^\s*</.test(lines[index])) {
        html.push(lines[index].trim())
        index += 1
      }
      const rendered = html
        .join('\n')
        .replace(/\b(src|srcset|href)="([^"]*)"/g, (_match, attr, url) => {
          return `${attr}="${escapeHtml(rewriteUrl(url, ctx))}"`
        })
      blocks.push(rendered)
      continue
    }
    const paragraph = [line]
    index += 1
    while (index < lines.length && lines[index].trim() !== '' && !isBlockStart(lines, index)) {
      paragraph.push(lines[index])
      index += 1
    }
    blocks.push(`<p>${inline(paragraph.join(' '), ctx)}</p>`)
  }
  return blocks.join('\n')
}

/**
 * @param {string} markdown
 * @param {{ file: string, rendered: Set<string> }} ctx file is repo-relative, e.g. docs/MODELS.md
 */
export function renderMarkdown(markdown, ctx) {
  const lines = markdown.replace(/\r\n/g, '\n').split('\n')
  return parseBlocks(lines, ctx)
}
