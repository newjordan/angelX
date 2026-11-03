const book = require('./spells')
module.exports = { apiVersion: 2, initialize: () => ({}), reduce: state => ({ state }), resources: () => ({ pack: { abilities: book.definitions } }), instantiate: () => ({ casts: {} }), reduceInstance: book.tick }
