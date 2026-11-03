const { rank } = require('./rank.js')

// The actual per-turn tool-selection policy. A turn pins one generation, so
// editing routing while a model streams never changes that turn's tools.
module.exports = {
  apiVersion: 1,
  render({ task = '', candidates = [], max = 3 }) {
    return rank(task, candidates).slice(0, max).map(candidate => candidate.name)
  },
}
