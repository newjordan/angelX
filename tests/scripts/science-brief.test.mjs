import { test } from 'node:test'
import assert from 'node:assert/strict'
import { analyze, extract, extractBatch, fold } from '../../lib/science/brief.mjs'

const paper = (title, citations, year, line = title) => ({ title, citations: String(citations), year, line })

test('four providers yield normalized raw fields without discarding integer identities', () => {
  assert.deepEqual(extract('OpenAlex', {results:[{display_name:'Test',publication_year:'2022',cited_by_count:'18446744073709551615',authorships:[{author:{display_name:'A'}}]}]}), [{title:'Test',authors:['A'],year:'2022',doi:null,url:null,citations:'18446744073709551615'}])
  assert.deepEqual(extract('Crossref', {message:{items:[{title:['X'],author:[{given:'Alice',family:'A'},{family:'B'}],issued:{'date-parts':[['2020']]}}]}})[0].authors, ['Alice A','B'])
  assert.equal(extract('SemanticScholar', {data:[{title:'BERT',externalIds:{DOI:'10.1/x'}}]})[0].doi, '10.1/x')
  const europe = extract('EuropePmc', {resultList:{result:[{title:'Hello...',authorString:'A, B.',source:'MED',id:'1'}]}})[0]
  assert.equal(europe.title,'Hello')
  assert.deepEqual(europe.authors,['A','B'])
  assert.equal(europe.record_id,'1')
})

test('batch isolates provider failures and preserves input order', () => {
  const results = extractBatch([
    {source:'OpenAlex', body:{results:[{display_name:'One', cited_by_count:'18446744073709551615'}]}},
    {source:'unknown', body:{}},
    {source:'EuropePmc', body:{resultList:{result:[{title:'Two.', id:'1', source:'MED'}]}}},
  ])
  assert.equal(results[0].papers[0].citations, '18446744073709551615')
  assert.deepEqual(results.map(result => result.source), ['OpenAlex', 'unknown', 'EuropePmc'])
  assert.deepEqual(results[1], {source:'unknown', error:'source extraction failed'})
  assert.equal(results[2].papers[0].title, 'Two')
})

test('fold retains the richest duplicate and orders by citation and recency', () => {
  const p = (index,key,title,citations,year,valid_doi=true) => ({ index,key,title,citations:String(citations),year,valid_doi })
  assert.deepEqual(fold([p(0,'d','Same',1,2020),p(1,'d','Same',30,2020),p(2,'e','Other',5000,2010),p(3,'f','Fresh',40,2024)]), [2,3,1])
  assert.deepEqual(fold([p(0,'a','A','9007199254740992',2020),p(1,'a','A','9007199254740993',2020)]), [1])
})

test('fold excludes discarded publication years from the recency anchor', () => {
  const papers = [
    { ...paper('Established', 300, 2000), index: 0, key: 'a', valid_doi: true },
    { ...paper('Recent', 100, 2020), index: 1, key: 'b', valid_doi: true },
  ]
  const discarded = { ...paper('Recent', 1, 2026), index: 2, key: 'b', valid_doi: true }
  assert.deepEqual(fold(papers), [1, 0])
  assert.deepEqual(fold([...papers, discarded]), [1, 0], 'lower-citation duplicate must be inert')
  assert.deepEqual(fold([discarded, ...papers]), [1, 0], 'input order must not revive discarded metadata')
  assert.deepEqual(fold([]), [])
  assert.deepEqual(fold([{ ...papers[0], year: null }]), [0])
})

test('theme citation totals saturate instead of wrapping and reversing their rank', () => {
  const papers = [
    paper('Graph Mining', '18446744073709551615', 2020),
    paper('Graph Search', 1, 2021),
    paper('Neural Models', 3, 2022),
    paper('Neural Networks', 4, 2023),
  ]
  const result = analyze({ query: 'models', papers, source_count: 1, notes: [], limit: 4 })
  const graph = result.clusters.find(cluster => cluster.theme === 'graph')
  assert.equal(graph.citation_sum, '18446744073709551615', 'stay within the native u64 contract')
  assert.equal(result.clusters[0].theme, 'graph', 'an overflowing high-citation theme must not rank last')
})

test('brief retains native reading-list order and metadata-only caveats', () => {
  const papers = [paper('Graph Neural Networks for Molecules',100,2020,'first citation'),paper('Graph Neural Networks at Scale',10,2021,'second citation'),paper('A Survey of Graph Models',3,2024,'third citation')]
  const result = analyze({query:'graphs',papers,source_count:2,notes:['one unavailable'],limit:2})
  assert.deepEqual(result.themes[0], ['graph',3])
  assert.equal(result.clusters[0].theme,'graph')
  assert.deepEqual(result.clusters[0].anchors,['first citation','second citation'])
  assert.match(result.brief,/seminal: first citation/)
  assert.match(result.brief,/freshest: third citation/)
  assert.match(result.brief,/not citation clusters/)
  assert.match(result.brief,/title text only; not evidence/)
  assert.match(result.brief,/  \(skipped: one unavailable\)/)
  assert.doesNotMatch(result.brief,/  3\. third citation/)
})

test('cluster keyword indexing counts each title once and retains stable equal-key anchors', () => {
  const papers = [
    paper('Graph Graph Graph Δοκιμή', 7, 2020, 'first tie'),
    paper('Graph Graph Graph Δοκιμή', 7, 2020, 'second tie'),
    paper('Other Graph Δοκιμή', 1, 2021, 'third'),
  ]
  const result = analyze({query:'graph',papers,source_count:1,notes:[],limit:3})
  const graph = result.clusters.find(c => c.theme === 'graph')
  assert.equal(graph.paper_count, 3)
  assert.equal(graph.citation_sum, '15')
  assert.deepEqual(graph.anchors, ['first tie','second tie'])
  assert.deepEqual(result.themes.find(([theme]) => theme === 'graph'), ['graph',3])
})
