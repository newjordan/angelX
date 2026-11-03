// Pure, metadata-only literature analysis. The host supplies citation lines
// after validating URLs and sanitizing display text; this module never fetches.
const STOPWORDS = new Set('the and for with without via using from into onto between across over under that this these those study studies analysis approach approaches method methods based novel toward towards case review paper results effects role evidence data their have been'.split(' '))
const cmp = (a, b) => {
  const x = Array.from(a, c => c.codePointAt(0)), y = Array.from(b, c => c.codePointAt(0))
  for (let i = 0; i < Math.min(x.length, y.length); i++) if (x[i] !== y[i]) return x[i] - y[i]
  return x.length - y.length
}
const citation = p => BigInt(p.citations)
const keywords = title => title.split(/[^\p{Alphabetic}\p{Number}]+/u).map(w => w.toLowerCase()).filter(w => Buffer.byteLength(w, 'utf8') >= 4 && !STOPWORDS.has(w))
const comparePapers = (a, b) => citation(a) > citation(b) ? -1 : citation(a) < citation(b) ? 1 : cmp(a.title, b.title)
export function themes(papers, top) {
  const counts = new Map()
  for (const p of papers) for (const k of new Set(keywords(p.title))) counts.set(k, (counts.get(k) ?? 0) + 1)
  return [...counts].filter(([, n]) => n >= 2).sort((a,b) => b[1] - a[1] || cmp(a[0], b[0])).slice(0, top)
}
export function clusters(papers, count, anchorCount) {
  const out = []
  for (const [theme] of themes(papers, count * 3)) {
    const matches = papers.filter(p => keywords(p.title).includes(theme)).sort(comparePapers)
    if (matches.length < 2) continue
    const sum = matches.reduce((acc, p) => BigInt.asUintN(64, acc + citation(p)), 0n)
    out.push({ theme, paper_count: matches.length, citation_sum: sum.toString(), anchors: matches.slice(0, Math.max(1, anchorCount)).map(p => p.line) })
  }
  out.sort((a,b) => b.paper_count - a.paper_count || (BigInt(a.citation_sum) > BigInt(b.citation_sum) ? -1 : BigInt(a.citation_sum) < BigInt(b.citation_sum) ? 1 : cmp(a.theme,b.theme)))
  return out.slice(0, count)
}
export function cues(papers, limit) {
  return papers.slice(0, limit).map(p => {
    const t = p.title.replace(/[A-Z]/g, c => c.toLowerCase())
    const kind = ['survey','review','meta-analysis','systematic'].some(x => t.includes(x)) ? 'survey/review' :
      ['benchmark','evaluation','comparison','comparative'].some(x => t.includes(x)) ? 'evaluation' :
      ['improve','improves','improving','increase','increases','reduce','reduces','enhance','enhances'].some(x => t.includes(x)) ? 'effect/intervention' : 'method/finding'
    return `${kind} cue: ${p.title} (${p.year ?? 'n.d.'}, ${p.citations} cites)`
  })
}
export function analyze({ query, papers, source_count, notes, limit, theme_limit = 5, cluster_limit = 4, anchor_limit = 2, cue_limit = 3 }) {
  const ts = themes(papers, theme_limit), cs = clusters(papers, cluster_limit, anchor_limit), qs = cues(papers, cue_limit)
  let out = `◇ synthesis · "${query}"\n  ${papers.length} works across ${source_count} source(s)\n`
  if (ts.length) out += `  themes: ${ts.map(([k,n]) => `${k}×${n}`).join(', ')}\n`
  let seminal, freshest
  for (const p of papers) {
    if (!seminal || citation(p) >= citation(seminal)) seminal = p
    if (p.year != null && (!freshest || p.year >= freshest.year)) freshest = p
  }
  if (seminal) out += `  seminal: ${seminal.line}\n`
  if (freshest) out += `  freshest: ${freshest.line}\n`
  if (notes.length) out += `  (skipped: ${notes.join('; ')})\n`
  if (cs.length) {
    out += '  metadata clusters (shared title keywords only; not citation clusters):\n'
    for (const c of cs) {
      out += `    - ${c.theme}: ${c.paper_count} papers, ${c.citation_sum} citations\n`
      for (const a of c.anchors) out += `      * ${a}\n`
    }
  }
  if (qs.length) {
    out += '  metadata cues (title text only; not evidence—read the paper before relying):\n'
    for (const q of qs) out += `    - ${q}\n`
  }
  out += '  ── reading list ──\n'
  for (const [i,p] of papers.slice(0, limit).entries()) out += `  ${String(i+1).padStart(2)}. ${p.line}\n`
  return { brief: out, themes: ts, clusters: cs, cues: qs }
}
// The host owns canonical identity and URL validation. This module only
// chooses indices; the caller checks them and keeps its original Paper values.
export function fold(papers) {
  const newest = Math.max(0, ...papers.map(p => p.year ?? 0))
  const byKey = new Map()
  for (const p of papers) {
    const old = byKey.get(p.key)
    if (!old || citation(p) > citation(old) || (citation(p) === citation(old) && !old.valid_doi && p.valid_doi)) byKey.set(p.key, p)
  }
  const score = p => Math.log(Number(p.citations) + 1) + (p.year != null && newest >= p.year ? 1 - Math.min((newest - p.year) * .05, 1) : 0) * 1.5
  return [...byKey.values()].sort((a,b) => score(b) - score(a) || cmp(a.title,b.title)).map(p => p.index)
}

// The host converts unsigned JSON numbers to decimal strings before this
// boundary. It sanitizes all text and validates all identifiers after return.
const str = x => typeof x === 'string' ? x : null
const arr = x => Array.isArray(x) ? x : []
export function extract(source, body) {
  let records
  if (source === 'OpenAlex') records = arr(body?.results)
  else if (source === 'Crossref') records = arr(body?.message?.items)
  else if (source === 'SemanticScholar') records = arr(body?.data)
  else if (source === 'EuropePmc') records = arr(body?.resultList?.result)
  else throw new Error('unknown science source')
  return records.map(w => {
    if (source === 'OpenAlex') return {
      title: str(w?.display_name), authors: arr(w?.authorships).map(a => str(a?.author?.display_name)).filter(a => a !== null),
      year: str(w?.publication_year), doi: str(w?.doi), url: str(w?.id), citations: str(w?.cited_by_count),
    }
    if (source === 'Crossref') return {
      title: str(arr(w?.title)[0]), authors: arr(w?.author).flatMap(a => {
        const f = str(a?.family), g = str(a?.given)
        return f === null ? [] : [g === null ? f : `${g} ${f}`]
      }), year: str(arr(arr(w?.issued?.['date-parts'])[0])[0]), doi: str(w?.DOI), url: str(w?.URL), citations: str(w?.['is-referenced-by-count']),
    }
    if (source === 'SemanticScholar') return {
      title: str(w?.title), authors: arr(w?.authors).map(a => str(a?.name)).filter(a => a !== null),
      year: str(w?.year), doi: str(w?.externalIds?.DOI), url: str(w?.url), citations: str(w?.citationCount),
    }
    const author = str(w?.authorString)
    return {
      title: str(w?.title)?.replace(/\.+$/, '') ?? null,
      authors: author === null ? [] : author.replace(/\.+$/, '').split(', ').filter(a => a.length).slice(0, 4),
      year: str(w?.pubYear), doi: str(w?.doi), url: null, citations: str(w?.citedByCount),
      record_source: str(w?.source), record_id: str(w?.id),
    }
  })
}

// A single malformed provider must not discard the other sources. The host
// validates each returned record and attributes failures to its own source.
export function extractBatch(items) {
  return items.map(({ source, body }) => {
    try { return { source, papers: extract(source, body) } }
    catch { return { source, error: 'source extraction failed' } }
  })
}
