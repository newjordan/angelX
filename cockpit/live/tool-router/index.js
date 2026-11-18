const { rank } = require('./rank.js')

module.exports = {
  apiVersion: 1,
  render({ task = '', candidates = [], max = 3 }) {
    return rank(task, candidates).slice(0, max).map(candidate => candidate.name)
  },
}
