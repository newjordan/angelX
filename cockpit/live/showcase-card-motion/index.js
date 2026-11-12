// Canonical module-authored card reactions, shared with legacy art packs.
const pack = require('./pack.json')
module.exports = { apiVersion: 2, initialize: () => ({}), reduce: state => ({ state }), resources: () => ({ pack }) }
