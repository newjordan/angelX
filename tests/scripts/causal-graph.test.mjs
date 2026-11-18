import { test } from 'node:test'
import assert from 'node:assert/strict'
import { pathToFileURL } from 'node:url'

const { default: CausalGraph, defaultEdgeId } = await import(
  process.env.ANGEL_GRAPH_TEST_MODULE
    ? pathToFileURL(process.env.ANGEL_GRAPH_TEST_MODULE).href
    : '../../lib/research/CausalGraph.js'
)
const rawId = ({ src, type, dst }) => `${src}--${type}-->${dst}`
const snapshot = (g) => ({ nodes: g.nodes(), edges: g.edges() })
const roundTrip = (g) => CausalGraph.deserialize(JSON.parse(JSON.stringify(g.serialize())))
const triples = (g) => g.edges().map(({ src, type, dst }) => [src, type, dst])

for (const [name, a, b] of [
  ['opposite directions', { src: 'a', type: 'uses', dst: 'a--uses-->a' }, { src: 'a--uses-->a', type: 'uses', dst: 'a' }],
  ['source/type boundary', { src: 'a-', type: 'uses', dst: 'b' }, { src: 'a', type: '-uses', dst: 'b' }],
  ['type/destination boundary', { src: 'a', type: 'uses-->', dst: 'b' }, { src: 'a', type: 'uses', dst: '-->b' }],
]) {
  test(`generated IDs retain distinct edges: ${name}`, () => {
    assert.equal(rawId(a), rawId(b), 'fixture must collide in the old encoding')
    const g = new CausalGraph()
    for (const id of new Set([a.src, a.dst, b.src, b.dst])) g.addNode({ id })
    g.addEdge({ ...a, confidence: 0.2 }).addEdge({ ...b, confidence: 0.8 })
    assert.equal(g.edgeCount(), 2)
    assert.notEqual(g.edges()[0].id, g.edges()[1].id)
    assert.deepEqual(triples(g), [[a.src, a.type, a.dst], [b.src, b.type, b.dst]])
    assert.deepEqual(snapshot(roundTrip(g)), snapshot(g))
    assert.equal(g.outgoing(a.src).some(({ edge }) => edge.dst === a.dst && edge.type === a.type), true)
    assert.equal(g.incoming(b.dst).some(({ edge }) => edge.src === b.src && edge.type === b.type), true)
    g.addEdge({ ...a, confidence: 0.9 })
    assert.equal(g.edgeCount(), 2, 'identical triple is an upsert')
    assert.equal(g.edges()[0].confidence, 0.9)
  })
}

test('escaping is reversible and does not alias literal percent encodings', () => {
  const values = ['a--b', 'a%2D%2Db', '-a', '%2Da', 'a-', 'a%2D', 'a---b', '%', '%25', '雪--😀', 'a\u0000b']
  const g = new CausalGraph()
  for (const src of values) g.addEdge({ src, type: 'uses', dst: 'b' })
  assert.equal(g.edgeCount(), values.length)
  for (const edge of g.edges()) assert.equal(g.getEdge(defaultEdgeId(edge.src, edge.type, edge.dst)), edge)
  assert.deepEqual(snapshot(roundTrip(g)), snapshot(g))
})

test('ordinary IDs and explicitly provided IDs retain their original spelling', () => {
  const g = new CausalGraph()
  const edge = { src: 'exp_auto_2026-07-06', type: 'supports', dst: 'fact_ritual-cargo-test', confidence: 0.5 }
  g.addEdge(edge)
  assert.equal(g.edges()[0].id, rawId(edge))
  for (const id of ['', 0, 'manual--id-->anything', 'custom-id']) {
    g.addEdge({ ...edge, id })
    assert.equal(g.getEdge(id).id, id)
    g.addEdge({ src: 'new-source', type: 'tests', dst: 'new-target', id })
    assert.equal(g.getEdge(id).src, 'new-source', 'explicit ID remains an intentional upsert')
  }
  assert.deepEqual(snapshot(roundTrip(g)), snapshot(g))
})

test('legacy serialized default IDs are preserved and idless updates do not duplicate them', () => {
  const edge = { src: 'a--uses-->a', type: 'uses', dst: 'a', confidence: 0.2 }
  const g = CausalGraph.deserialize({ version: 1, nodes: [], edges: [{ ...edge, id: rawId(edge) }] })
  assert.equal(g.edges()[0].id, rawId(edge))
  g.addEdge({ ...edge, confidence: 0.9 })
  assert.equal(g.edgeCount(), 1)
  assert.equal(g.getEdge(rawId(edge)).confidence, 0.9)
  assert.deepEqual(snapshot(roundTrip(g)), snapshot(g))
  g.addEdge({ src: 'a', type: 'uses', dst: 'a--uses-->a' })
  assert.equal(g.edgeCount(), 2)
  assert.equal(g.getEdge(rawId(edge)).src, edge.src)
})

for (const src of ['ordinary', 'a--uses-->a']) {
  test(`automatic IDs never overwrite a different explicit edge: ${src}`, () => {
    const edge = { src, type: 'uses', dst: 'b' }
    const g = new CausalGraph()
    const id = defaultEdgeId(edge.src, edge.type, edge.dst)
    g.addEdge({ id, src: 'reserved', type: 'tests', dst: 'elsewhere' })
    const before = snapshot(g)
    assert.throws(() => g.addEdge(edge), /collision/i)
    assert.deepEqual(snapshot(g), before)
  })
}

test('removal and reinsertion use stable generated IDs', () => {
  const edge = { src: '-a--', type: 'uses-', dst: '--b%' }
  const g = new CausalGraph().addEdge(edge)
  const id = g.edges()[0].id
  assert.equal(g.hasEdge(id), true)
  g.removeEdge(id)
  assert.equal(g.edgeCount(), 0)
  g.addEdge(edge)
  assert.equal(g.edges()[0].id, id)
  g.clear().addEdge(edge)
  assert.equal(g.edges()[0].id, id)
})

test('merge and subgraph preserve colliding raw triples after round trip', () => {
  const a = { src: 'a', type: 'uses', dst: 'a--uses-->a' }
  const b = { src: a.dst, type: 'uses', dst: 'a' }
  const graph = () => new CausalGraph().addNode({ id: a.src }).addNode({ id: a.dst })
  const left = graph().addEdge(a), right = graph().addEdge(b)
  left.merge(roundTrip(right))
  assert.equal(left.edgeCount(), 2)
  assert.deepEqual(snapshot(left.subgraph(() => true)), snapshot(left))
  left.removeNode(a.src)
  assert.equal(left.edgeCount(), 0)
})

test('mixed-version merge deduplicates a generated relation and preserves left metadata', () => {
  const edge = { src: 'a--b', type: 'uses', dst: 'c' }
  for (const reverse of [false, true]) {
    const legacy = new CausalGraph().addEdge({ ...edge, id: rawId(edge), label: 'legacy' })
    const fresh = new CausalGraph().addEdge({ ...edge, label: 'fresh' })
    const [left, right] = reverse ? [fresh, legacy] : [legacy, fresh]
    const before = snapshot(left)
    left.merge(right)
    assert.equal(left.edgeCount(), 1)
    assert.deepEqual(snapshot(left), before)
  }
})

test('mixed-version merge collision rejects before modifying any nodes or edges', () => {
  const legacy = new CausalGraph().addEdge({ src: 'a%2D%2Db', type: 'uses', dst: 'c', id: 'a%2D%2Db--uses-->c' })
  const fresh = new CausalGraph().addNode({ id: 'new-node' }).addEdge({ src: 'a--b', type: 'uses', dst: 'c' })
  for (const [left, right] of [[legacy, fresh], [fresh, legacy]]) {
    const beforeLeft = snapshot(left), beforeRight = snapshot(right)
    assert.throws(() => left.merge(right), /collision/i)
    assert.deepEqual(snapshot(left), beforeLeft)
    assert.deepEqual(snapshot(right), beforeRight)
  }
})

test('merge resolves generated aliases within pending additions but keeps explicit parallel edges', () => {
  const edge = { src: '-a--%', type: 'supports', dst: 'b-' }
  const oldId = rawId(edge), newId = defaultEdgeId(edge.src, edge.type, edge.dst)
  for (const ids of [[oldId, newId], [newId, oldId]]) {
    const incoming = new CausalGraph()
    for (const id of [...ids, 'independent-vote-1', 'independent-vote-2']) incoming.addEdge({ ...edge, id })
    const merged = new CausalGraph().merge(incoming)
    assert.deepEqual(merged.edges().map(e => e.id), [ids[0], 'independent-vote-1', 'independent-vote-2'])
    assert.equal(incoming.edgeCount(), 4, 'incoming graph remains untouched')
  }
})
