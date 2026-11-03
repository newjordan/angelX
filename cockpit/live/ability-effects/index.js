const effects = require('./effects')
module.exports = { apiVersion: 2, initialize: () => ({}), reduce: state => ({ state }), resources: () => ({ pack: { banks: effects.banks, description: effects.description } }) }
