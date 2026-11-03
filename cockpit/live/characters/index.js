const looks = require('./looks')
module.exports = {
  apiVersion: 2,
  initialize: () => ({ legacyChoices: {} }),
  migrate: previous => ({ legacyChoices: previous.state.legacyChoices ?? previous.state.choices ?? {} }),
  reduce: state => ({ state }),
  resources: state => ({ pack: {
    options: { ...Object.fromEntries(Object.keys(looks.options).map(name => [name, looks.resolve(name)])), off: { disabled: true } },
    legacy_choices: state.legacyChoices
  } })
}
