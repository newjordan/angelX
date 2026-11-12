// Authored playback policy over the independent character art bank.
// Change ordering and dwell times without replacing art or executable code.
const options = {
  breathe: [{ frame: 0, ticks: 8 }, { frame: 1, ticks: 8 }],
  staccato: [{ frame: 0, ticks: 3 }, { frame: 1, ticks: 12 }]
}
module.exports = { apiVersion: 2, initialize: () => ({}), reduce: state => ({ state }), resources: () => ({ pack: { options } }) }
