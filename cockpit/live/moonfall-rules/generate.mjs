// Writes the two generated Bend sources of the Moonfall rules module:
//   model.bend  the records and a getter and a setter for every field the rules use
//   town.bend   the fen town's tile rows, stopping places and villager routes
// Run it from the repository root after changing the field lists below, the town's map
// (crates/delve/src/rules/moonfall/town.rs) or the villagers' routine. The rules in
// moonfall-rules.bend are written by hand; these two files are not.
//   node cockpit/live/moonfall-rules/generate.mjs
import fs from 'node:fs'
import path from 'node:path'
import {fileURLToPath} from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.resolve(here, '../../..')
const words = s => s.split(/\s+/).filter(Boolean)
const fl = (s, t = 'U') => words(s).map(f => [f, t])

// ---- the records ------------------------------------------------------------------------
// The two big records, a knight (33 words) and the session's scalars (43 words), are word
// trees: a quad tree three deep (64 slots), read and written along a path of three digits.
// Every other record is a plain record with a getter and a setter per field.
const ty = t => (t === 'U' ? 'U32' : t.startsWith('L:') ? `List<&2, ${t.slice(2) === 'U' ? 'U32' : t.slice(2)}>` : t)
const KN = words(`id joined x y face stride roll roll_rest roll_dx roll_dy keys
  held held_at mask candles hurt down hold push read
  panel parg say_on say_folk say_tk say_ta say_page say_age note_on note_kind note_a note_b note_left`)
const SS = words(`elapsed clock phase p_t p_from p_to p_fell card done_on done_at peak
  flags ink_on ink_given shod_on shod crate_on crate_x crate_y snuffed lit0 lit1 lit2
  facts ticks masks keeps loops
  call_on call_ring call_by call_ayes call_left cry_on cry_folk cry_tk cry_ta cry_age
  n_day n_walk n_wisps n_rewind n_notice`)
// [record, accessor prefix, fields]
const RECORDS = [
  ['Wp', 'w_', fl('x y mode t aim born')],
  ['Nw', 'n_', fl('kind a b left')],
  ['In', 'i_', fl('id valid interact take swing ult play bomb dash mx my ax ay')],
  ['He', 'h_', fl('id awake')],
  ['Pg', 'p_', fl('id pages')],
  ['St', 'st_', fl('at spot inside lamp noted')],
  ['Stop', 'sp_', fl('at spot inside fast routine lamp')],
  ['Leg', 'lg_', fl('idx from depart arrive spot inside routine lamp at pace')],
  ['Fg', 'fg_', fl('x y visible moving face leg arrived routine')],
  ['P', 'pt_', fl('x y')],
  ['Al', 'al_', fl('x y face')],
  ['Bs', 'bs_', fl('d u')],
  ['Lt', 'lt_', fl('id x y d2')],
  ['Wr', 'wr_', [['w', 'Wp'], ['out', 'U']]],
  ['Wl', 'wl_', [['ws', 'L:Wp'], ['outs', 'L:U']]],
  ['Iv', 'iv_', fl('folk a b')],
  ['Tk', 'tk_', fl('light hk hi ak aa ab act opens closes')],
  ['As', 'as_', [...fl('id has'), ['t', 'Tk']]],
  ['Cx', 'cx_', [...fl('l0 l1 l2 l3 l4 l5', 'L:Leg'), ...fl('sn', 'L:Iv'), ...fl('g0 g1 g2 g3 g4 g5', 'Fg')]],
  ['V5', 'v5_', fl('c n s w e')],
  ['Ip', 'ip_', fl('mx my interact ult swing play bomb dash')],
  ['Ar', 'ar_', [...fl('free', 'L:U'), ...fl('given', 'L:As')]],
  ['Pf', 'pf_', fl('ok x y')],
  ['Dg', 'dg_', fl('on dx dy can')],
  ['Ly', 'ly_', [['w', 'U'], ['t', 'Tk']]],
  ['Out', 'o_', [['s', 'Sess'], ['leaving', 'L:U'], ...fl('won best')]],
  ['Ts', 'ts_', [['s', 'Sess'], ...fl('tk ta')]],
  ['Wk', 'wk_', [['k', 'Kn'], ...fl('con cx cy filled moved heavy leaning')]],
  // the session: its lists, and one tree for the scalars
  ['Sess', 's_', [...fl('seats parked', 'L:Kn'), ...fl('wisps', 'L:Wp'), ...fl('news', 'L:Nw'), ...fl('rt cues pending', 'L:U'),
    ...fl('inputs', 'L:In'), ...fl('heroes', 'L:He'), ...fl('pages', 'L:Pg'), ['w', 'V']]],
]

const TREE = `# A tree of words: four children to a node, three deep, so 64 slots. A slot is named by the
# three base-four digits of its place.
type V is Data:
  Lf{x: U32}
  Qd{a: V, b: V, c: V, d: V}

def v_pick(+n: U32, +a: V, +b: V, +c: V, +d: V) -> V:
  Bool.pick(V, U32.is_lt(n, 2), Bool.pick(V, U32.is_eq(n, 0), a, b), Bool.pick(V, U32.is_eq(n, 2), c, d))

def v_child(+t: V, +n: U32) -> V:
  match t:
    case Lf{_}:
      t
    case Qd{+a, +b, +c, +d}:
      v_pick(n, a, b, c, d)

def v_leaf(+t: V) -> U32:
  match t:
    case Lf{+x}:
      x
    case Qd{_, _, _, _}:
      0

def v_put(+t: V, +n: U32, +child: V) -> V:
  match t:
    case Lf{_}:
      t
    case Qd{+a, +b, +c, +d}:
      Qd{Bool.pick(V, U32.is_eq(n, 0), child, a), Bool.pick(V, U32.is_eq(n, 1), child, b), Bool.pick(V, U32.is_eq(n, 2), child, c), Bool.pick(V, U32.is_eq(n, 3), child, d)}

def vg(+t: V, +a: U32, +b: U32, +c: U32) -> U32:
  v_leaf(v_child(v_child(v_child(t, a), b), c))

def vs(+t: V, +a: U32, +b: U32, +c: U32, +v: U32) -> V:
  +t1 = v_child(t, a)
  +t2 = v_child(t1, b)
  v_put(t, a, v_put(t1, b, v_put(t2, c, Lf{v})))

def v_zero() -> V:
  +l = {Lf{0} : V}
  +r = {Qd{l, l, l, l} : V}
  +q = {Qd{r, r, r, r} : V}
  Qd{q, q, q, q}
`

function model(entry) {
  const head = ['import Base', '', '# The records of the Moonfall rules, and a getter and a setter for every field they use.',
    '# Generated by generate.mjs from its field lists; do not edit by hand.', '', TREE]
  const defs = []
  const add = (name, text) => defs.push([name, text])
  const types = []
  const digits = i => [i >> 4, (i >> 2) & 3, i & 3].join(', ')
  // a record that is one tree of words
  const tree = (name, prefix, slots) => {
    types.push(`type ${name} is Data:\n  ${name}{w: V}\n`)
    slots.forEach((f, i) => {
      add(prefix + f, `def ${prefix}${f}(+r: ${name}) -> U32:\n  match r:\n    case ${name}{+w}:\n      vg(w, ${digits(i)})\n`)
      add(`${prefix}set_${f}`, `def ${prefix}set_${f}(+r: ${name}, +v: U32) -> ${name}:\n  match r:\n    case ${name}{+w}:\n      ${name}{vs(w, ${digits(i)}, v)}\n`)
    })
  }
  const flat = (name, prefix, fields) => {
    const n = fields.length
    types.push(`type ${name} is Data:\n  ${name}{${fields.map(([f, t]) => `${f}: ${ty(t)}`).join(', ')}}\n`)
    fields.forEach(([f, t], i) => {
      const pat = fields.map((_, j) => (j === i ? 'v' : '_')).join(', ')
      add(prefix + f, `def ${prefix}${f}(+r: ${name}) -> ${ty(t)}:\n  match r:\n    case ${name}{${pat}}:\n      v\n`)
    })
    fields.forEach(([f, t], i) => {
      const pat = fields.map((_, j) => (j === i ? '_' : `+f${j}`)).join(', ')
      const build = fields.map((_, j) => (j === i ? 'v' : `f${j}`)).join(', ')
      add(`${prefix}set_${f}`, `def ${prefix}set_${f}(+r: ${name}, +v: ${ty(t)}) -> ${name}:\n  match r:\n    case ${name}{${pat}}:\n      ${name}{${build}}\n`)
    })
  }
  tree('Kn', 'k_', KN)
  for (const [name, prefix, fields] of RECORDS) flat(name, prefix, fields)
  // the session's scalars through its one tree
  SS.forEach((f, i) => {
    add('s_' + f, `def s_${f}(+r: Sess) -> U32:\n  vg(s_w(r), ${digits(i)})\n`)
    add(`s_set_${f}`, `def s_set_${f}(+r: Sess, +v: U32) -> Sess:\n  s_set_w(r, vs(s_w(r), ${digits(i)}, v))\n`)
  })
  // Keep the accessors the rules name, and what those call.
  const texts = new Map(defs)
  const keep = new Set()
  const work = []
  for (const m of fs.readFileSync(entry, 'utf8').matchAll(/\bM\.([A-Za-z_][A-Za-z_0-9]*)/g)) {
    if (texts.has(m[1]) && !keep.has(m[1])) { keep.add(m[1]); work.push(m[1]) }
  }
  keep.add('s_w'); keep.add('s_set_w')
  while (work.length) {
    const n = work.pop()
    for (const m of texts.get(n).matchAll(/[A-Za-z_][A-Za-z_0-9]*/g)) {
      if (texts.has(m[0]) && !keep.has(m[0])) { keep.add(m[0]); work.push(m[0]) }
    }
  }
  return [...head, ...types, ...defs.filter(([n]) => keep.has(n)).map(([, t]) => t)].join('\n')
}

// ---- the town -----------------------------------------------------------------------------
// The spots, in the native order (Spot in town.rs), by tile.
const SPOTS = [[9, 14], [10, 15], [24, 14], [24, 15], [28, 15], [32, 14], [32, 15], [21, 16], [15, 15], [20, 15], [5, 5], [6, 6], [12, 7], [24, 6], [21, 7], [34, 15], [37, 15],
  [9, 17], [13, 17], [16, 17], [20, 17], [24, 17], [28, 17], [18, 5], [17, 24], [17, 15]]
// The legs the villagers walk, as (from spot, to spot): consecutive stops of every program, and
// Aster's two rides once his horse is shod.
const PAIRS = [[0, 0], [0, 1], [1, 3], [1, 17], [2, 2], [2, 3], [2, 4], [3, 1], [3, 2], [3, 4], [3, 10], [4, 3], [4, 4], [5, 5], [5, 6], [6, 5], [6, 7], [7, 6], [8, 0], [9, 16], [10, 10], [10, 12], [11, 3], [12, 11], [13, 13], [13, 14], [14, 13], [15, 9], [15, 16], [16, 15], [17, 18], [18, 19], [19, 20], [20, 21], [21, 22], [22, 8]]
const CLS = {'~': 2, '=': 3, p: 4, r: 5, f: 6, g: 7, d: 8, G: 9, ',': 1, L: 1, c: 1, '.': 0}
const cls = ch => (ch in CLS ? CLS[ch] : 10)
const COLS = 38
const ROWS = 28

function town(source) {
  const map = source.match(/pub const MAP: \[&str; ROWS as usize\] = \[(.*?)\];/s)[1]
  const rows = [...map.matchAll(/"([^"]*)"/g)].map(m => m[1])
  if (rows.length !== ROWS || rows.some(r => r.length !== COLS)) throw Error('the map is not 38 by 28')
  const out = ['import Base', '', "# The fen town as data: tile rows, the villagers' stopping places and the routes", '# between them. Generated by generate.mjs from the native map (town.rs) and its walking routes;', '# the parity tests compare the module to the native town on every tick.', '']
  // A decision tree over sorted keys: one def per inner node, the root named `name`.
  const tree = (name, ret, keys, leaf) => {
    let n = 0
    const sub = (lo, hi) => {
      if (hi - lo === 1) return ['leaf', leaf(keys[lo])]
      const mid = (lo + hi) >> 1
      n += 1
      const id = `${name}_${n}`
      const left = sub(lo, mid)
      const right = sub(mid, hi)
      const call = (child, a, b) => (child[0] === 'leaf' ? child[1] : `${child[1]}(U32.is_lt(i, ${keys[(a + b) >> 1]}), i)`)
      out.push(`def ${id}(+c: Bool, +i: U32) -> ${ret}:\n  match c:\n    case True{}:\n      ${call(left, lo, mid)}\n    case False{}:\n      ${call(right, mid, hi)}\n`)
      return ['node', id, mid]
    }
    const root = sub(0, keys.length)
    out.push(`def ${name}(+i: U32) -> ${ret}:\n  ${root[0] === 'leaf' ? root[1] : `${root[1]}(U32.is_lt(i, ${keys[root[2]]}), i)`}\n`)
  }
  // Rows: 8 cells of 4 bits to a word, 5 words to a row.
  out.push('type Row is Data:\n  Row{w0: U32, w1: U32, w2: U32, w3: U32, w4: U32}\n')
  const rowWords = r => [0, 1, 2, 3, 4].map(w => { let v = 0; for (let c = 0; c < 8; c++) { const x = w * 8 + c; v += (x < COLS ? cls(r[x]) : 2) * 2 ** (4 * c) } return v })
  tree('row', 'Row', [...Array(ROWS).keys()], y => `Row{${rowWords(rows[y]).join(', ')}}`)
  out.push('def row_word(+r: Row, +w: U32) -> U32:\n  match r:\n    case Row{+w0, +w1, +w2, +w3, +w4}:\n      Bool.pick(U32, U32.is_lt(w, 2), Bool.pick(U32, U32.is_lt(w, 1), w0, w1), Bool.pick(U32, U32.is_lt(w, 4), Bool.pick(U32, U32.is_lt(w, 3), w2, w3), w4))\n')
  out.push(`# The tile class at column tx, row ty: 0 grass, 1 cobble, 2 water, 3 bridge, 4 plank,
# 5 reeds, 6 hedge, 7 ditch, 8 door, 9 gate, 10 a wall. Beyond the map is water.
def cls_in(+tx: U32, +r: Row) -> U32:
  U32.and(U32.shrn(row_word(r, (tx / 8 : U32)), Nat.mul(U32.to_nat((tx % 8 : U32)), 4n)), 15)

def cls_at(+tx: U32, +ty: U32) -> U32:
  cls_in(tx, row(ty))

def cls_pick(+ok: Bool, +tx: U32, +ty: U32) -> U32:
  match ok:
    case True{}:
      cls_at(tx, ty)
    case False{}:
      2

def cls(+tx: U32, +ty: U32) -> U32:
  cls_pick(Bool.and(U32.is_lt(tx, ${COLS}), U32.is_lt(ty, ${ROWS})), tx, ty)
`)
  // The spots' tiles: a short chain of picks, read once or twice a tick.
  for (const [name, at] of [['spot_tx', 0], ['spot_ty', 1]]) {
    const chain = SPOTS.reduceRight((rest, spot, i) => (rest === null ? String(spot[at]) : `Bool.pick(U32, U32.is_eq(i, ${i}), ${spot[at]}, ${rest})`), null)
    out.push(`def ${name}(+i: U32) -> U32:\n  ${chain}\n`)
  }
  // Routes: a flood from the goal over the ground villagers use by day (the east gate theirs), then
  // from the start the first neighbour one step nearer, north, south, west, east.
  const open = ch => { const c = cls(ch); return c === 0 || c === 1 || c === 5 || c === 8 || c === 3 || c === 9 }
  const flood = goal => {
    const field = new Array(COLS * ROWS).fill(65535)
    if (!open(rows[goal[1]][goal[0]])) return field
    field[goal[1] * COLS + goal[0]] = 0
    const queue = [goal]
    while (queue.length) {
      const [x, y] = queue.shift()
      const here = field[y * COLS + x]
      for (const [dx, dy] of [[0, -1], [0, 1], [-1, 0], [1, 0]]) {
        const nx = x + dx, ny = y + dy
        if (nx >= 0 && nx < COLS && ny >= 0 && ny < ROWS && open(rows[ny][nx]) && field[ny * COLS + nx] === 65535) {
          field[ny * COLS + nx] = here + 1
          queue.push([nx, ny])
        }
      }
    }
    return field
  }
  const moves = (from, to) => {
    const field = flood(SPOTS[to])
    let [x, y] = SPOTS[from]
    const out = []
    while ((x !== SPOTS[to][0] || y !== SPOTS[to][1]) && field[y * COLS + x] !== 65535) {
      const here = field[y * COLS + x]
      const step = [[0, -1, 0], [0, 1, 1], [-1, 0, 2], [1, 0, 3]].find(([dx, dy]) => {
        const nx = x + dx, ny = y + dy
        return nx >= 0 && nx < COLS && ny >= 0 && ny < ROWS && Math.min(field[ny * COLS + nx] + 1, 65535) === here
      })
      if (!step) break
      x += step[0]; y += step[1]
      out.push(step[2])
    }
    return out
  }
  out.push('type Rt is Data:\n  Rt{n: U32, w0: U32, w1: U32}\n')
  const routes = new Map(PAIRS.map(([a, b]) => [a * 32 + b, moves(a, b)]))
  const keys = [...routes.keys()].sort((a, b) => a - b)
  tree('route', 'Rt', keys, k => {
    const m = routes.get(k)
    if (m.length > 32) throw Error('a route is longer than two words')
    const ws = [0, 0]
    m.forEach((mv, i) => { ws[i >> 4] += mv * 2 ** (2 * (i % 16)) })
    return `Rt{${m.length}, ${ws.join(', ')}}`
  })
  return out.join('\n')
}

fs.writeFileSync(path.join(here, 'model.bend'), model(path.join(here, 'moonfall-rules.bend')))
fs.writeFileSync(path.join(here, 'town.bend'), town(fs.readFileSync(path.join(root, 'crates/delve/src/rules/moonfall/town.rs'), 'utf8')))
