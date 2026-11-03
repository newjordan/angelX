// Thin adapter for the `layout` rules' Bend leg. Plain JSON in, the compiled
// leg's rooms out: where each authored room stands, what it is built of and
// which doorways it opens live in layout-rules.bend. Rust owns the records
// (`Room`, `Floor`, `Secret`), the tile enum, the room-kind names, the door
// lanes on a native room, the doors a floor already has, and every rendering
// question. This file only marshals lists and ids.
const bend = require('./layout-rules.compiled.js')

const nil = { $: 'Nil' }
const cons = values => values.reduceRight((tail, head) => ({ $: 'Con', head, tail }), nil)
const arr = list => {
  const out = []
  let at = list
  while (at && at.$ === 'Con') {
    out.push(at.head)
    at = at.tail
  }
  return out
}

const word = value => (Number.isFinite(value) ? Math.max(0, Math.trunc(value)) : 0)
const rect = value => ({
  $: 'Rect',
  col: word(value[0]),
  row: word(value[1]),
  w: word(value[2]),
  h: word(value[3]),
})
const rects = values => cons((values || []).map(rect))

const room = value => ({
  cell_x: word(value.cell_x),
  cell_y: word(value.cell_y),
  kind: word(value.kind),
  doors: arr(value.doors).map(open => open === true),
  cols: word(value.cols),
  rows: word(value.rows),
  tiles: arr(value.tiles).map(word),
})
const rooms = list => arr(list).map(room)

module.exports = {
  apiVersion: 1,
  render(input = {}) {
    switch (input.op) {
      case 'tavern': {
        const dig = bend.layout_tavern(
          word(input.depth),
          cons((input.kinds || []).map(word)),
          cons((input.cells_x || []).map(word)),
          cons((input.cells_y || []).map(word)),
          rect(input.bar),
          rect(input.board),
          rect(input.stage),
          rects(input.tables),
        )
        if (dig.$ !== 'Dug') return { dug: false }
        return { dug: true, hall: word(dig.hall), room: room(dig.room) }
      }
      case 'wing':
        return {
          rooms: rooms(bend.layout_tavern_wing(word(input.cell_x), word(input.cell_y))),
        }
      default:
        return {
          rooms: rooms(
            bend.layout_undercroft(
              rects(input.spots),
              rect(input.bounty_board),
              rect(input.stairwell),
              rect(input.stair_mouth),
              rect(input.wheel_stand),
              rect(input.coffer),
              rects(input.benches),
              rect(input.stall),
              rects(input.plinths),
            ),
          ),
        }
    }
  },
}
