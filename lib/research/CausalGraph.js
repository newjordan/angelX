export const NODE_TYPE = Object.freeze({
  HYPOTHESIS: 'hypothesis',
  EXPERIMENT: 'experiment',
  GEPA_RUN: 'gepa_run',
  GEPA_CANDIDATE: 'candidate',
  DATASET: 'dataset',
  METRIC: 'metric',
})

export const EDGE_TYPE = Object.freeze({
  TESTS: 'tests',
  INFORMED: 'informed',
  PRODUCED: 'produced',
  COMPARES_TO: 'compares_to',
  USES: 'uses',
  METRIC_OF: 'metric_of',
  CONTRADICTS: 'contradicts',
  SUPPORTS: 'supports',
  DERIVED_FROM: 'derived_from',
})

const edgeIdPart = (value) => String(value).replace(/%|--|^-|-$/g,
  (token) => token === '%' ? '%25' : '%2D'.repeat(token.length))

export const defaultEdgeId = (src, type, dst) =>
  `${edgeIdPart(src)}--${edgeIdPart(type)}-->${edgeIdPart(dst)}`

const sameEdge = (a, b) => a.src === b.src && a.type === b.type && a.dst === b.dst

export default class CausalGraph {
  constructor() {
    this._nodes = new Map()
    this._edges = new Map()
  }

  addNode(node) {
    if (!node.id) throw new Error('CausalGraph: node.id is required')
    this._nodes.set(node.id, { ...node })
    return this
  }

  removeNode(id) {
    this._nodes.delete(id)
    for (const [eid, e] of this._edges) {
      if (e.src === id || e.dst === id) this._edges.delete(eid)
    }
    return this
  }

  updateNode(id, patch) {
    const n = this._nodes.get(id)
    if (!n) return null
    const updated = { ...n, ...patch }
    this._nodes.set(id, updated)
    return updated
  }

  getNode(id) {
    return this._nodes.get(id) ?? null
  }

  hasNode(id) {
    return this._nodes.has(id)
  }

  nodeCount() {
    return this._nodes.size
  }

  nodes() {
    return [...this._nodes.values()]
  }

  nodesOfType(type) {
    return [...this._nodes.values()].filter((n) => n.type === type)
  }

  addEdge(edge) {
    if (!edge.src || !edge.dst || !edge.type) {
      throw new Error('CausalGraph: edge.src, edge.dst, edge.type are required')
    }
    let id = edge.id
    if (id == null) {
      id = defaultEdgeId(edge.src, edge.type, edge.dst)
      const legacyId = `${edge.src}--${edge.type}-->${edge.dst}`
      const legacy = this._edges.get(legacyId)
      if (legacy && sameEdge(legacy, edge)) id = legacyId
      const existing = this._edges.get(id)
      if (existing && !sameEdge(existing, edge)) {
        throw new Error('CausalGraph: generated edge ID collision; provide an explicit edge.id')
      }
    }
    this._edges.set(id, { ...edge, id })
    return this
  }

  removeEdge(id) {
    this._edges.delete(id)
    return this
  }

  getEdge(id) {
    return this._edges.get(id) ?? null
  }

  hasEdge(id) {
    return this._edges.has(id)
  }

  edgeCount() {
    return this._edges.size
  }

  edges() {
    return [...this._edges.values()]
  }

  edgesOfType(type) {
    return [...this._edges.values()].filter((e) => e.type === type)
  }

  adjacent(nodeId) {
    const result = []
    for (const edge of this._edges.values()) {
      if (edge.src === nodeId) {
        const n = this._nodes.get(edge.dst)
        if (n) result.push({ node: n, edge, direction: 'out' })
      } else if (edge.dst === nodeId) {
        const n = this._nodes.get(edge.src)
        if (n) result.push({ node: n, edge, direction: 'in' })
      }
    }
    return result
  }

  outgoing(nodeId) {
    return [...this._edges.values()]
      .filter((e) => e.src === nodeId)
      .map((e) => ({ edge: e, node: this._nodes.get(e.dst) }))
      .filter((r) => r.node !== undefined)
  }

  incoming(nodeId) {
    return [...this._edges.values()]
      .filter((e) => e.dst === nodeId)
      .map((e) => ({ edge: e, node: this._nodes.get(e.src) }))
      .filter((r) => r.node !== undefined)
  }

  path(srcId, dstId) {
    if (!this._nodes.has(srcId) || !this._nodes.has(dstId)) return null
    if (srcId === dstId) return [{ node: this._nodes.get(srcId) }]

    const visited = new Set([srcId])
    const queue = [{ nodeId: srcId, trail: [{ node: this._nodes.get(srcId) }] }]

    while (queue.length > 0) {
      const { nodeId, trail } = queue.shift()
      for (const { edge, node, direction } of this.adjacent(nodeId)) {
        if (visited.has(node.id)) continue
        visited.add(node.id)
        const step = { node, edge, direction }
        const newTrail = [...trail, step]
        if (node.id === dstId) return newTrail
        queue.push({ nodeId: node.id, trail: newTrail })
      }
    }
    return null
  }

  ancestors(nodeId) {
    const result = new Set()
    const queue = [nodeId]
    while (queue.length > 0) {
      const current = queue.shift()
      for (const { node } of this.incoming(current)) {
        if (!result.has(node.id)) {
          result.add(node.id)
          queue.push(node.id)
        }
      }
    }
    return [...result].map((id) => this._nodes.get(id)).filter(Boolean)
  }

  descendants(nodeId) {
    const result = new Set()
    const queue = [nodeId]
    while (queue.length > 0) {
      const current = queue.shift()
      for (const { node } of this.outgoing(current)) {
        if (!result.has(node.id)) {
          result.add(node.id)
          queue.push(node.id)
        }
      }
    }
    return [...result].map((id) => this._nodes.get(id)).filter(Boolean)
  }

  subgraph(filter) {
    const matching = new Set(
      this._nodes
        .values()
        .filter(filter)
        .map((n) => n.id),
    )
    const nodes = [...matching].map((id) => this._nodes.get(id))
    const edges = [...this._edges.values()].filter(
      (e) => matching.has(e.src) && matching.has(e.dst),
    )
    const g = new CausalGraph()
    nodes.forEach((n) => g.addNode(n))
    edges.forEach((e) => g.addEdge(e))
    return g
  }

  queryEdges(predicate) {
    return [...this._edges.values()].filter(predicate)
  }

  serialize() {
    return {
      version: 1,
      lastIngest: new Date().toISOString(),
      nodes: [...this._nodes.values()],
      edges: [...this._edges.values()],
    }
  }

  static deserialize(data) {
    const g = new CausalGraph()
    if (!data) return g
    for (const n of data.nodes ?? []) g.addNode(n)
    for (const e of data.edges ?? []) g.addEdge(e)
    return g
  }

  clear() {
    this._nodes.clear()
    this._edges.clear()
    return this
  }

  merge(other) {
    const additions = new Map()
    const findEdge = id => this._edges.get(id) ?? additions.get(id)
    for (const e of other._edges.values()) {
      const existing = findEdge(e.id)
      if (existing) {
        if (!sameEdge(existing, e)) throw new Error('CausalGraph: merge edge ID collision')
        continue
      }
      const generatedId = defaultEdgeId(e.src, e.type, e.dst)
      const legacyId = `${e.src}--${e.type}-->${e.dst}`
      if ((e.id === generatedId || e.id === legacyId) &&
          [generatedId, legacyId].some(id => {
            const edge = findEdge(id)
            return edge && sameEdge(edge, e)
          })) continue
      additions.set(e.id, e)
    }
    for (const n of other._nodes.values()) {
      if (!this._nodes.has(n.id)) this.addNode(n)
    }
    for (const e of additions.values()) this.addEdge(e)
    return this
  }
}
