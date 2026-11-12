// Ordered edges and phase policy belong to this replaceable pack.
const edge = (to, when, phase = 'preserve') => ({ to, when, phase })
const options = { adventure: { initial: 'rest', states: {
  rest: { steps: [{ frame: 0, ticks: 12 }], transitions: [edge('attack', { attacking: true }, 'restart'), edge('guard', { shielding: true }), edge('walk', { moving: true })] },
  walk: { steps: [{ frame: 0, ticks: 4 }, { frame: 1, ticks: 4 }], transitions: [edge('attack', { attacking: true }, 'restart'), edge('guard', { shielding: true }), edge('rest', { moving: false })] },
  attack: { steps: [{ frame: 1, ticks: 3 }], transitions: [edge('rest', { attacking: false })] },
  guard: { steps: [{ frame: 1, ticks: 12 }], transitions: [edge('attack', { attacking: true }, 'restart'), edge('rest', { shielding: false })] }
} } }
module.exports = { apiVersion: 2, initialize: () => ({}), reduce: state => ({ state }), resources: () => ({ pack: { options } }) }
