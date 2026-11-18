const art = require('./pack.json')
const card = resource('showcase-card-motion', 'pack').value
module.exports = { apiVersion: 2, initialize: () => ({}), reduce: state => ({ state }), resources: () => ({ pack: { ...art, card } }) }
