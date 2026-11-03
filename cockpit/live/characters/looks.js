// Authored content belongs here, outside Glass. Add another named look without
// a Rust enum or a host branch. Each frame may instead be {asset: asset('idle.png').id};
// declare those local 32x32 PNG files in module.json's "assets" array.
const options = {
  violet: { armour: [145, 105, 235], trim: [220, 210, 255], frameTicks: 8 },
  gold: { armour: [225, 165, 50], trim: [255, 235, 160], frameTicks: 6 }
}
function frame(look, bob) {
  const pixels = Array.from({ length: 1024 }, () => [0, 0, 0, 0])
  const paint = (x, y, w, h, colour) => {
    for (let py = y; py < y + h; py++) for (let px = x; px < x + w; px++) pixels[py * 32 + px] = [...colour, 255]
  }
  paint(10, 8 + bob, 12, 10, look.armour)
  paint(8, 17 + bob, 16, 9, look.armour)
  paint(10, 12 + bob, 12, 2, [28, 25, 46])
  paint(14, 18 + bob, 4, 7, look.trim)
  paint(9, 26, 5, 4, look.trim)
  paint(18, 26, 5, 4, look.trim)
  return { pixels }
}
exports.options = options
exports.resolve = name => {
  const look = options[name]
  if (!look) throw Error(`selected character option removed without migration: ${name}`)
  return { colour: look.armour, frame_ticks: look.frameTicks, frames: [frame(look, 0), frame(look, 1)] }
}
