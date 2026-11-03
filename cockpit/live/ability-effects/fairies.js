// Indexed artwork stays compact in module metadata; the worker prepares its
// RGBA frames once. Glass only samples the captured animation and keyframes.
const wings = [
  ['........', '..W..W..', '.WWWWH..', '..WBB...', '...BB...', '..G..G..', '........', '........'],
  ['........', '.....H..', '..WWWH..', '.WWBBWW.', '..WBBW..', '..G..G..', '........', '........']
]
const key = (tick, offset, scale, colour, opacity = 255) => ({ tick, offset, scale, colour, opacity })
exports.field = (colour, radius, duration) => {
  const steps = Math.min(10, duration)
  const ringScale = radius / Math.min(radius, 8)
  const ringKeys = [key(0, [0, 0], ringScale, colour, 150)]
  if (duration > 1) ringKeys.push(key(Math.max(1, Math.min(duration - 1, Math.floor(duration * 0.9))), [0, 0], ringScale, colour, 130))
  ringKeys.push(key(duration, [0, 0], ringScale, colour, 0))
  return { ticks: duration, nodes: [
  { layer: 'ground', shape: { type: 'ring', radius: Math.min(radius, 8), width: 1 }, keys: ringKeys
  },
  ...Array.from({ length: 6 }, (_, fairy) => ({
    shape: { type: 'sprite', frame_ticks: 3, frames: wings.map(rows => ({ rows, palette: {
      W: [...colour, 170], H: [255, 240, 220, 255], B: [245, 255, 240, 255], G: [...colour, 255]
    } })) },
    keys: Array.from({ length: steps + 1 }, (_, index) => {
      const angle = fairy * Math.PI / 3 + index * Math.PI * 2 / steps
      const bob = Math.sin(index * Math.PI / 2 + fairy) * 0.3
      return key(Math.floor(index * duration / steps), [Math.cos(angle) * radius * 0.8, Math.sin(angle) * radius * 0.8 - 0.5 + bob], 0.6, [255, 255, 255], index === steps ? 0 : 255)
    })
  }))
] }
}
