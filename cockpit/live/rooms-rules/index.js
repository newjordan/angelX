// Thin adapter for the rooms module. Plain JSON in, the compiled art out: the
// quadrants, their sizes, the authored share and the least floor all live in
// rooms-rules.bend and the per-pack files beside it. Rust owns the pack
// indices, the room types, the mirroring and every error string; here the only
// work is putting the id on the module's numbered arms and reading the tagged
// terms back as rows of text.
const rooms = require('./rooms-rules.compiled.js')

const word = value => (Number.isFinite(value) ? Math.trunc(value) >>> 0 : 0)

const list = values => values.reduceRight((tail, head) => ({$: 'Con', head, tail}), {$: 'Nil'})

const rows = value => {
  const out = []
  for (let node = value; node && node.$ === 'Con'; node = node.tail) out.push(node.head)
  return out
}

const art = value => {
  const quads = []
  for (let node = value.quads; node && node.$ === 'Con'; node = node.tail) {
    const quad = node.head
    quads.push({w: word(quad.w), h: word(quad.h), rows: rows(quad.rows)})
  }
  return {quads}
}

module.exports = {
  apiVersion: 1,
  render(input = {}) {
    switch (input.op) {
      case 'consts': {
        const consts = rooms.consts()
        const sizes = []
        for (let node = consts.sizes; node && node.$ === 'Con'; node = node.tail) {
          sizes.push([word(node.head.w), word(node.head.h)])
        }
        return {sizes, share: word(consts.share), least_floor: word(consts.least_floor)}
      }
      case 'small':
        return art(rooms.small(word(input.pack)))
      case 'great':
        return art(rooms.great(word(input.pack)))
      case 'sized':
        return art(rooms.sized(word(input.pack), word(input.w), word(input.h)))
      default:
        throw Error(`rooms-rules: unknown op ${String(input.op)}`)
    }
  },
}
