// Authored graphs, shared by native and browser Glass. Add cues, primitives
// and curves here; painting never evaluates this source or reads an asset.
const fairies = require('./fairies')
const field = resource('ability-code', 'pack').value.abilities.fairies.parameters
const key = (tick, scale, colour, opacity = 255, offset = [0, 0]) => ({ tick, scale, colour, opacity, offset })
const spark = colour => ({ ticks: 90, nodes: [
  { layer: 'ground', shape: { type: 'ring', radius: 1.2, width: 1 }, keys: [key(0, 0.3, colour), key(20, 2.5, colour, 180), key(90, 3.2, colour, 0)] },
  { shape: { type: 'particles', count: 20, spread: 5, gravity: 1 }, keys: [key(0, 1, colour), key(90, 1, colour, 0)] },
  { shape: { type: 'sprite', frames: [{ asset: asset('sigil.png').id }], frame_ticks: 6 }, keys: [key(0, 0.5, [255, 255, 255]), key(16, 1.8, colour, 220), key(90, 2.5, colour, 0)] }
] })
const bloom = colour => ({ ticks: 120, nodes: [
  { layer: 'ground', shape: { type: 'disc', radius: 1.2 }, keys: [key(0, 0.1, colour, 40), key(30, 2, colour, 80), key(120, 3, colour, 0)] },
  { shape: { type: 'particles', count: 24, spread: 3, gravity: -3 }, keys: [key(0, 1, colour), key(120, 1, colour, 0)] }
] })
const hidden = { ticks: 1, nodes: [{ shape: { type: 'disc', radius: 0.05 }, keys: [key(0, 1, [0, 0, 0], 0)] }] }
exports.description = 'Crystal sigils'
exports.banks = {
  crystal: { fairies: fairies.field([150, 240, 200], field.radius, field.duration), spark: spark([80, 220, 255]), bloom: bloom([100, 245, 150]) },
  ember: { fairies: fairies.field([255, 200, 120], field.radius, field.duration), spark: spark([255, 130, 60]), bloom: bloom([255, 200, 90]) },
  quiet: { fairies: hidden, spark: hidden, bloom: hidden }
}
