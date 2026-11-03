const sounds = require('./sounds')
module.exports = { apiVersion: 2, initialize: () => ({}), reduce: state => ({ state }), resources: () => ({ pack: { banks: sounds.banks } }) }
