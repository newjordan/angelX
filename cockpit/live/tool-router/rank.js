// BM25, k1=1.5 and b=0.75: the original native shortlist policy, now editable
// together with index.js. Candidates contain only tools already admitted by
// the native registry; this module ranks them and cannot grant new tools.
const tokens = value => value.toLowerCase().match(/[\p{L}\p{N}]+/gu) || []

exports.rank = (task, candidates) => {
  const query = tokens(task)
  if (!query.length || !candidates.length) return []
  const terms = new Set(query)
  const documents = candidates.map(candidate => {
    const words = tokens(candidate.text)
    const frequencies = new Map()
    for (const word of words) {
      if (terms.has(word)) frequencies.set(word, (frequencies.get(word) || 0) + 1)
    }
    return { candidate, length: words.length, frequencies }
  })
  const count = documents.length
  const average = Math.max(1, documents.reduce((sum, doc) => sum + doc.length, 0) / count)
  const frequencies = new Map()
  for (const doc of documents) {
    for (const term of doc.frequencies.keys()) frequencies.set(term, (frequencies.get(term) || 0) + 1)
  }
  return documents.map((doc, order) => {
    let score = 0
    for (const term of query) {
      const tf = doc.frequencies.get(term) || 0
      if (!tf) continue
      const df = frequencies.get(term)
      const idf = Math.log(1 + (count - df + 0.5) / (df + 0.5))
      score += idf * tf * 2.5 / (tf + 1.5 * (0.25 + 0.75 * doc.length / average))
    }
    return { candidate: doc.candidate, score, order }
  }).filter(doc => doc.score > 0)
    .sort((left, right) => right.score - left.score || left.order - right.order)
    .map(doc => doc.candidate)
}
