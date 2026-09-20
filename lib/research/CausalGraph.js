// CausalGraph — pure node/edge store. No Three.js, no network, no side effects.
// All state lives in this._nodes / this._edges Maps. Serialize to JSON for
// persistence; deserialize to restore.

export const NODE_TYPE = Object.freeze({
  HYPOTHESIS: 'hypothesis',
  EXPERIMENT: 'experiment',
  GEPA_RUN: 'gepa_run',
  GEPA_CANDIDATE: 'candidate',
  DATASET: 'dataset',
  METRIC: 'metric',
})

export const EDGE_TYPE = Object.freeze({
  TESTS: 'tests', // experiment → hypothesis
  INFORMED: 'informed', // experiment A informed experiment B
  PRODUCED: 'produced', // GEPA run → candidate
  COMPARES_TO: 'compares_to', // candidate A vs candidate B
  USES: 'uses', // experiment → dataset
  METRIC_OF: 'metric_of', // metric observation → experiment
  CONTRADICTS: 'contradicts', // experiment → hypothesis (negative result)
  SUPPORTS: 'supports', // experiment → hypothesis (positive result)
  DERIVED_FROM: 'derived_from', // candidate → parent candidate
})

// ─── construction ────────────────────────────────────────────────────────────

export default class CausalGraph {
  constructor() {
    this._nodes = new Map() // id → node
    this._edges = new Map() // id → edge
  }

  // ─── node ops ─────────────────────────────────────────────────────────────

  addNode(node) {
    if (!node.id) throw new Error('CausalGraph: node.id is required')
    this._nodes.set(node.id, { ...node })
    return this
  }

  removeNode(id) {
    this._nodes.delete(id)
    // drop orphaned edges
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

  // ─── edge ops ─────────────────────────────────────────────────────────────

  /**
   * Add an edge. id is auto-generated from src + type + dst if omitted.
   * @param {{id?, src, dst, type, confidence?, label?}} edge
   */
  addEdge(edge) {
    if (!edge.src || !edge.dst || !edge.type) {
      throw new Error('CausalGraph: edge.src, edge.dst, edge.type are required')
    }
    const id = edge.id ?? `${edge.src}--${edge.type}-->${edge.dst}`
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

  // ─── graph queries ─────────────────────────────────────────────────────────

  /**
   * All nodes adjacent to nodeId (connected by any edge, either direction).
   * @returns {{node, edges, direction}[]}
   */
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

  /**
   * Outgoing edges from nodeId.
   */
  outgoing(nodeId) {
    return [...this._edges.values()]
      .filter((e) => e.src === nodeId)
      .map((e) => ({ edge: e, node: this._nodes.get(e.dst) }))
      .filter((r) => r.node !== undefined)
  }

  /**
   * Incoming edges to nodeId.
   */
  incoming(nodeId) {
    return [...this._edges.values()]
      .filter((e) => e.dst === nodeId)
      .map((e) => ({ edge: e, node: this._nodes.get(e.src) }))
      .filter((r) => r.node !== undefined)
  }

  /**
   * BFS shortest path from srcId to dstId. Returns node+edge list or null.
   * @returns Array<{node, edge?, direction?}> | null
   */
  path(srcId, dstId) {
    if (!this._nodes.has(srcId) || !this._nodes.has(dstId)) return null
    if (srcId === dstId) return [{ node: this._nodes.get(srcId) }]

    const visited = new Set([srcId])
    // queue items: { nodeId, trail }
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

  /**
   * Ancestors of nodeId (all nodes that can reach it, via directed edges).
   * @returns Set<node>
   */
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

  /**
   * Descendants of nodeId (all nodes reachable from it, via directed edges).
   * @returns Array<node>
   */
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

  /**
   * Induced subgraph containing only nodes matching the given filter.
   * @param {(node) => boolean} filter
   */
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

  /**
   * Filter edges by predicate.
   */
  queryEdges(predicate) {
    return [...this._edges.values()].filter(predicate)
  }

  // ─── persistence ────────────────────────────────────────────────────────────

  serialize() {
    return {
      version: 1,
      lastIngest: new Date().toISOString(),
      nodes: [...this._nodes.values()],
      edges: [...this._edges.values()],
    }
  }

  /** @returns {CausalGraph} a new instance loaded from data */
  static deserialize(data) {
    const g = new CausalGraph()
    if (!data) return g
    for (const n of data.nodes ?? []) g.addNode(n)
    for (const e of data.edges ?? []) g.addEdge(e)
    return g
  }

  // ─── mutations ─────────────────────────────────────────────────────────────

  clear() {
    this._nodes.clear()
    this._edges.clear()
    return this
  }

  /** Merge another graph into this one (union of nodes + edges). */
  merge(other) {
    for (const n of other._nodes.values()) {
      if (!this._nodes.has(n.id)) this.addNode(n)
    }
    for (const e of other._edges.values()) {
      if (!this._edges.has(e.id)) this.addEdge(e)
    }
    return this
  }
}
