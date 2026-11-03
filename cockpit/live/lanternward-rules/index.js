// Thin adapter for Lanternward's Bend leg. The session crosses as serde writes
// it; this packs each record into the words the Bend rules hold (the layout
// block below is generated from the table in scripts/live/lanternward-fields.mjs)
// and unpacks the answer the same way. It holds no rule of the game: names to
// codes, codes to names, nothing else. Rust owns the sounds' words, so an
// answer carries the sounds as codes.
const bend = require('./lanternward-rules.compiled.js')

// layout:begin
const SLOT = {"Seat":{"id":0,"joined":1,"x":2,"y":3,"z":4,"fx":5,"fy":6,"hp":7,"max_hp":8,"gems":9,"bombs":10,"flame":11,"taper":12,"act":13,"t":14,"n":15,"mvx":16,"mvy":17,"pushx":18,"pushy":19,"pusht":20,"step":21,"chain":22,"queued":23,"charge":24,"shield":25,"guard_cd":26,"lock_k":27,"lock_i":28,"lock_t":29,"unlock_t":30,"relock":31,"inv":32,"ward":33,"freeze":34,"swing":35,"lamp":36,"lamp_cd":37,"carry":38,"ctx_k":39,"ctx_i":40,"read_on":41,"read_id":42,"read_t":43,"aimx":44,"aimy":45,"held":46,"safex":47,"safey":48,"camx":49,"camy":50,"rev":51,"beep":52,"item_k":53,"item_n":54},"Foe":{"id":0,"kind":1,"x":2,"y":3,"hp":4,"full":5,"prov":6,"st":7,"t":8,"fx":9,"fy":10,"dk":11,"d0":12,"d1":13,"d2":14,"d3":15,"d4":16,"hooded":17,"lit":18,"flinch":19,"freeze":20,"dead":21,"open":22,"h0":23,"h1":24,"h2":25,"h3":26,"h4":27,"h5":28,"struck":29,"kbx":30,"kby":31,"kbt":32,"n":33},"Warden":{"x":0,"y":1,"hp":2,"full":3,"prov":4,"phase":5,"st":6,"t":7,"atk":8,"n":9,"dk":10,"d0":11,"d1":12,"d2":13,"d3":14,"d4":15,"aim":16,"sweep":17,"l0":18,"l1":19,"l2":20,"l3":21,"l4":22,"l5":23,"h0":24,"h1":25,"h2":26,"h3":27,"h4":28,"h5":29,"struck":30,"dead":31,"start":32,"fx":33,"fy":34,"flinch":35},"Sc":{"elapsed":0,"room":1,"stage":2,"stage_t":3,"shake":4,"flash":5,"exiting":6,"wipe":7,"tries":8,"next_foe":9,"done_on":10,"done_t":11,"paid":12,"seed_hi":13,"seed_lo":14,"cue_on":15,"cue_x":16,"cue_y":17,"cue_t":18,"ban_on":19,"ban_k":20,"ban_n":21,"ban_seat":22,"ban_t":23,"p_map":24,"p_lantern":25,"p_keys":26,"p_cont":27,"hold_k":28,"hold_a":29,"hold_x":30,"hold_y":31,"boss_on":32,"stop":33,"won":34,"ret":35,"ret2":36,"r_lock":37,"r_parry":38,"r_combo":39,"r_flame":40,"r_tell":41,"nseats":42,"ta":43,"tb":44,"tc":45},"Prop":{"kind":0,"tx":1,"ty":2,"id":3,"on":4,"t":5,"x":6,"y":7},"Drop":{"kind":0,"x":1,"y":2,"age":3},"Fx":{"kind":0,"x":1,"y":2,"t":3,"a":4},"Fuse":{"x":0,"y":1,"t":2,"owner":3},"Bolt":{"kind":0,"x":1,"y":2,"vx":3,"vy":4,"ttl":5,"owner":6},"Kept":{"id":0,"gems":1,"bombs":2},"Ix":{"id":0,"mx":1,"my":2,"ax":3,"ay":4,"mask":5},"Strike":{"i":0,"dmg":1,"dk":2,"d0":3,"d1":4,"d2":5,"d3":6,"d4":7},"Nr":{"ok":0,"id":1,"x":2,"y":3,"d":4},"Cx":{"ok":0,"d":1,"c":2,"i":3},"Ct":{"k":0,"i":1},"Pt":{"x":0,"y":1},"Tg":{"ok":0,"a":1,"b":2,"d":3,"kind":4,"id":5}}
// layout:end

const nil = { $: 'Nil' }
const cons = values => values.reduceRight((tail, head) => ({ $: 'Con', head, tail }), nil)
const list = node => {
  const out = []
  for (let at = node; at.$ === 'Con'; at = at.tail) out.push(at.head)
  return out
}

// A word: an unsigned 32-bit number; a signed one is the same bits.
const U = value => (Number.isFinite(value) ? Math.trunc(value) >>> 0 : 0)

const ENUM = {
  act: ['Free', 'Slash', 'Lunge', 'Spin', 'Roll', 'Hop', 'Plunge', 'Hurt', 'Item', 'Down', 'Fall'],
  carry: ['Nothing', 'Pot'],
  pk: ['Pot', 'Grass', 'Sign', 'Brazier', 'Rest', 'Ember', 'Great', 'Chest', 'Gate', 'Plate', 'Block', 'Crystal', 'Cracked', 'Sigil', 'Lock', 'Boss', 'Beacon', 'Fissure', 'Folk', 'Lever'],
  dk: ['Heart', 'Gem', 'Bomb', 'Container'],
  fxk: ['Puff', 'Light', 'Spark', 'Ring', 'Parry', 'Tick', 'Dust', 'Solved', 'Blast', 'Wave'],
  bk: ['Iron', 'Light', 'Pot', 'Bright', 'Dim'],
  fk: ['Snuffer', 'Turret'],
  fs: ['Dormant', 'Drift', 'Tell', 'Strike', 'Recover', 'Stagger', 'Dying'],
  ws: ['Asleep', 'Rest', 'Tell', 'Strike', 'Recover', 'Stagger', 'Dying'],
  atk: ['Beam', 'Slam', 'Summon'],
}
const code = (names, name) => {
  const at = names.indexOf(name)
  if (at < 0) throw Error(`unknown name ${JSON.stringify(name)}`)
  return at
}

// Variants that carry a number: the unit ones are listed with it left out.
const CTX = ['None', 'Open', 'Read', 'Lift', 'Throw', 'Flame', 'Light', 'Unlock', 'Revive', 'Relight', 'Talk', 'Pull']
const CTX_UNIT = new Set(['None', 'Throw', 'Flame', 'Relight'])
const ITEM = ['', 'Map', 'Lantern', 'SmallKey', 'BossKey', 'Container', 'Gems', 'Bombs']
const ITEM_UNIT = new Set(['Map', 'Lantern', 'SmallKey', 'BossKey', 'Container'])
const DANGER = [
  ['None'],
  ['Capsule', 'ax', 'ay', 'bx', 'by', 'r'],
  ['Circle', 'x', 'y', 'r'],
  ['Fan', 'x', 'y', 'deg', 'half', 'r'],
  ['Ring', 'x', 'y', 'r', 'w'],
]
const HOLD = ['Unfound', 'Carried', 'Lying', 'Spent']

// What serde writes for a variant, as its name and number (or none).
const variant = value => {
  if (typeof value === 'string') return [value, undefined]
  const [name] = Object.keys(value)
  return [name, value[name]]
}

const CUSTOM = {
  lock: {
    enc(j, w, S) {
      if (j === null) return
      const [name, n] = variant(j)
      w[S.lock_k] = { Foe: 1, Prop: 2, Boss: 3 }[name]
      w[S.lock_i] = U(n)
    },
    dec(w, S) {
      const k = w[S.lock_k]
      return k === 0 ? null : k === 3 ? 'Boss' : { [k === 1 ? 'Foe' : 'Prop']: w[S.lock_i] }
    },
  },
  ctx: {
    enc(j, w, S) {
      const [name, n] = variant(j)
      w[S.ctx_k] = code(CTX, name)
      w[S.ctx_i] = U(n)
    },
    dec(w, S) {
      const name = CTX[w[S.ctx_k]]
      return CTX_UNIT.has(name) ? name : { [name]: w[S.ctx_i] }
    },
  },
  read: {
    enc(j, w, S) {
      if (j === null) return
      w[S.read_on] = 1
      w[S.read_id] = U(j[0])
      w[S.read_t] = U(j[1])
    },
    dec(w, S) {
      return w[S.read_on] === 0 ? null : [w[S.read_id], w[S.read_t]]
    },
  },
  item: {
    enc(j, w, S) {
      if (j === null) return
      const [name, n] = variant(j)
      w[S.item_k] = code(ITEM, name)
      w[S.item_n] = U(n)
    },
    dec(w, S) {
      const name = ITEM[w[S.item_k]]
      return w[S.item_k] === 0 ? null : ITEM_UNIT.has(name) ? name : { [name]: w[S.item_n] }
    },
  },
  danger: {
    enc(j, w, S) {
      const [name, body] = variant(j)
      const at = DANGER.findIndex(d => d[0] === name)
      if (at < 0) throw Error(`unknown danger ${name}`)
      w[S.dk] = at
      DANGER[at].slice(1).forEach((key, n) => { w[S[`d${n}`]] = U(body[key]) })
    },
    dec(w, S) {
      const d = DANGER[w[S.dk]]
      if (d.length === 1) return d[0]
      return { [d[0]]: Object.fromEntries(d.slice(1).map((key, n) => [key, w[S[`d${n}`]] | 0])) }
    },
  },
}

// A record's spec: [json key, kind, ...]. `u` (the default) is a word named
// like its key, `i` a signed one, `b` a flag, `e` a unit enum, `p` a pair of
// signed words, `a` an array of unsigned words, anything else a CUSTOM.
const SEAT = [
  ['id'], ['joined'], ['x', 'i'], ['y', 'i'], ['z', 'i'], ['face', 'p', 'fx', 'fy'], ['hp'], ['max_hp'], ['gems'], ['bombs'],
  ['flame'], ['taper'], ['act', 'e', 'act'], ['t'], ['n'], ['mv', 'p', 'mvx', 'mvy'], ['push', 'p', 'pushx', 'pushy'], ['pusht'],
  ['step'], ['chain'], ['queued', 'b'], ['charge'], ['shield'], ['guard_cd'], ['lock', 'lock'], ['lock_t'], ['unlock_t'],
  ['relock'], ['inv'], ['ward'], ['freeze'], ['swing'], ['lamp'], ['lamp_cd'], ['carry', 'e', 'carry'], ['ctx', 'ctx'],
  ['read', 'read'], ['aim', 'p', 'aimx', 'aimy'], ['held'], ['safe', 'p', 'safex', 'safey'], ['cam', 'p', 'camx', 'camy'],
  ['rev'], ['beep'], ['item', 'item'],
]
const FOE = [
  ['id'], ['kind', 'e', 'fk'], ['x', 'i'], ['y', 'i'], ['hp'], ['full'], ['prov'], ['st', 'e', 'fs'], ['t'], ['fx', 'i'], ['fy', 'i'],
  ['danger', 'danger'], ['hooded', 'b'], ['lit'], ['flinch'], ['freeze'], ['dead'], ['open', 'b'], ['hit', 'a', 'h', 6], ['struck'],
  ['kb', 'p', 'kbx', 'kby'], ['kbt'], ['n'],
]
const WARDEN = [
  ['x', 'i'], ['y', 'i'], ['hp'], ['full'], ['prov'], ['phase'], ['st', 'e', 'ws'], ['t'], ['atk', 'e', 'atk'], ['n'],
  ['danger', 'danger'], ['aim', 'i'], ['sweep', 'i'], ['lit', 'a', 'l', 6], ['hit', 'a', 'h', 6], ['struck'], ['dead'], ['start'],
  ['face', 'p', 'fx', 'fy'], ['flinch'],
]
const PROP = [['kind', 'e', 'pk'], ['tx'], ['ty'], ['id'], ['on'], ['t'], ['x', 'i'], ['y', 'i']]
const DROP = [['kind', 'e', 'dk'], ['x', 'i'], ['y', 'i'], ['age']]
const FX = [['kind', 'e', 'fxk'], ['x', 'i'], ['y', 'i'], ['t'], ['a', 'i']]
const FUSE = [['x', 'i'], ['y', 'i'], ['t'], ['owner']]
const BOLT = [['kind', 'e', 'bk'], ['x', 'i'], ['y', 'i'], ['vx', 'i'], ['vy', 'i'], ['ttl'], ['owner']]

const pack = (spec, S, j) => {
  const w = new Array(64).fill(0)
  for (const [key, kind = 'u', a, b] of spec) {
    switch (kind) {
      case 'u': case 'i': w[S[key]] = U(j[key]); break
      case 'b': w[S[key]] = j[key] ? 1 : 0; break
      case 'e': w[S[key]] = code(ENUM[a], j[key]); break
      case 'p': w[S[a]] = U(j[key][0]); w[S[b]] = U(j[key][1]); break
      case 'a': for (let n = 0; n < b; n++) w[S[a + n]] = U(j[key][n]); break
      default: CUSTOM[kind].enc(j[key], w, S)
    }
  }
  return w
}

const unpack = (spec, S, w) => {
  const j = {}
  for (const [key, kind = 'u', a, b] of spec) {
    switch (kind) {
      case 'u': j[key] = w[S[key]]; break
      case 'i': j[key] = w[S[key]] | 0; break
      case 'b': j[key] = w[S[key]] !== 0; break
      case 'e': j[key] = ENUM[a][w[S[key]]]; break
      case 'p': j[key] = [w[S[a]] | 0, w[S[b]] | 0]; break
      case 'a': j[key] = Array.from({ length: b }, (_, n) => w[S[a + n]]); break
      default: j[key] = CUSTOM[kind].dec(w, S)
    }
  }
  return j
}

const KEYS = 'abcdefgh'
const w8 = w => ({ $: 'W8', a: w[0] || 0, b: w[1] || 0, c: w[2] || 0, d: w[3] || 0, e: w[4] || 0, f: w[5] || 0, g: w[6] || 0, h: w[7] || 0 })
const t8 = w => ({ $: 'T8', a: w8(w.slice(0, 8)), b: w8(w.slice(8, 16)), c: w8(w.slice(16, 24)), d: w8(w.slice(24, 32)), e: w8(w.slice(32, 40)), f: w8(w.slice(40, 48)), g: w8(w.slice(48, 56)), h: w8(w.slice(56, 64)) })
const words8 = leaf => KEYS.split('').map(k => leaf[k])
const words64 = tree => KEYS.split('').flatMap(k => words8(tree[k]))

const record8 = (spec, S) => ({ enc: j => w8(pack(spec, S, j)), dec: node => unpack(spec, S, words8(node)) })
const record64 = (spec, S) => ({ enc: j => t8(pack(spec, S, j)), dec: node => unpack(spec, S, words64(node)) })

const seat = record64(SEAT, SLOT.Seat)
const foe = record64(FOE, SLOT.Foe)
const warden = record64(WARDEN, SLOT.Warden)
const prop = record8(PROP, SLOT.Prop)
const drop = record8(DROP, SLOT.Drop)
const fx = record8(FX, SLOT.Fx)
const fuse = record8(FUSE, SLOT.Fuse)
const bolt = record8(BOLT, SLOT.Bolt)

// The session's own numbers, which are not a list of records.
const scalars = (s, rules) => {
  const S = SLOT.Sc
  const w = new Array(64).fill(0)
  for (const key of ['elapsed', 'room', 'stage', 'stage_t', 'shake', 'flash', 'exiting', 'wipe', 'tries', 'next_foe', 'paid']) w[S[key]] = U(s[key])
  w[S.seed_hi] = U(s.seed[0])
  w[S.seed_lo] = U(s.seed[1])
  if (s.done !== null) { w[S.done_on] = 1; w[S.done_t] = U(s.done) }
  if (s.cue !== null) { w[S.cue_on] = 1; w[S.cue_x] = U(s.cue[0]); w[S.cue_y] = U(s.cue[1]); w[S.cue_t] = U(s.cue[2]) }
  if (s.banner !== null) {
    const [name, n] = variant(s.banner.item)
    w[S.ban_on] = 1
    w[S.ban_k] = code(ITEM, name)
    w[S.ban_n] = U(n)
    w[S.ban_seat] = U(s.banner.seat)
    w[S.ban_t] = U(s.banner.t)
  }
  w[S.p_map] = s.inv.map ? 1 : 0
  w[S.p_lantern] = s.inv.lantern ? 1 : 0
  w[S.p_keys] = U(s.inv.small_keys)
  w[S.p_cont] = U(s.inv.containers)
  const [hold, body] = variant(s.inv.boss)
  w[S.hold_k] = code(HOLD, hold)
  if (hold === 'Carried') { w[S.hold_a] = U(body[0]); w[S.hold_x] = U(body[1]); w[S.hold_y] = U(body[2]) }
  if (hold === 'Lying') { w[S.hold_x] = U(body[0]); w[S.hold_y] = U(body[1]) }
  w[S.boss_on] = s.boss === null ? 0 : 1
  w[S.r_lock] = U(rules.lock)
  w[S.r_parry] = U(rules.parry)
  w[S.r_combo] = U(rules.combo)
  w[S.r_flame] = U(rules.flame)
  w[S.r_tell] = U(rules.tell)
  return w
}

// A session before anything has happened: the rules fill it in.
const BLANK = {
  seats: {}, elapsed: 0, room: 0, stage: 0, stage_t: 0, props: [], foes: [], bolts: [], drops: [], fx: [], fuses: [], boss: null,
  inv: { map: false, lantern: false, small_keys: 0, boss: 'Unfound', containers: 0 }, banner: null, seed: [0, 0], shake: 0,
  flash: 0, exiting: 0, wipe: 0, tries: 0, cue: null, next_foe: 0, done: null, paid: 0, kept: {},
}

const world = (s, here, rules) => ({
  $: 'W',
  s: t8(scalars(s, rules)),
  cur: t8([]),
  cf: t8([]),
  boss: s.boss === null ? t8([]) : warden.enc(s.boss),
  seats: cons(Object.keys(s.seats).map(Number).sort((a, b) => a - b).map(id => seat.enc(s.seats[id]))),
  foes: cons(s.foes.map(foe.enc)),
  props: cons(s.props.map(prop.enc)),
  bolts: cons(s.bolts.map(bolt.enc)),
  drops: cons(s.drops.map(drop.enc)),
  fx: cons(s.fx.map(fx.enc)),
  fuses: cons(s.fuses.map(fuse.enc)),
  kept: cons(Object.keys(s.kept).map(Number).sort((a, b) => a - b).map(id => w8([id, s.kept[id][0], s.kept[id][1]]))),
  ins: cons(here.map(row => w8(row.map(U)))),
  strikes: nil,
  shots: nil,
  grid: nil,
  snd: nil,
  here: cons(here.map(row => U(row[0]))),
})

const session = w => {
  const S = SLOT.Sc
  const x = words64(w.s)
  const seats = {}
  for (const k of list(w.seats)) {
    const j = seat.dec(k)
    seats[j.id] = j
  }
  const kept = {}
  for (const k of list(w.kept)) {
    const [id, gems, bombs] = words8(k)
    kept[id] = [gems, bombs]
  }
  const hold = HOLD[x[S.hold_k]]
  return {
    seats,
    elapsed: x[S.elapsed],
    room: x[S.room],
    stage: x[S.stage],
    stage_t: x[S.stage_t],
    props: list(w.props).map(prop.dec),
    foes: list(w.foes).map(foe.dec),
    bolts: list(w.bolts).map(bolt.dec),
    drops: list(w.drops).map(drop.dec),
    fx: list(w.fx).map(fx.dec),
    fuses: list(w.fuses).map(fuse.dec),
    boss: x[S.boss_on] === 0 ? null : warden.dec(w.boss),
    inv: {
      map: x[S.p_map] !== 0,
      lantern: x[S.p_lantern] !== 0,
      small_keys: x[S.p_keys],
      boss: hold === 'Carried' ? { Carried: [x[S.hold_a], x[S.hold_x] | 0, x[S.hold_y] | 0] } : hold === 'Lying' ? { Lying: [x[S.hold_x] | 0, x[S.hold_y] | 0] } : hold,
      containers: x[S.p_cont],
    },
    banner: x[S.ban_on] === 0 ? null : {
      item: ITEM_UNIT.has(ITEM[x[S.ban_k]]) ? ITEM[x[S.ban_k]] : { [ITEM[x[S.ban_k]]]: x[S.ban_n] },
      seat: x[S.ban_seat],
      t: x[S.ban_t],
    },
    seed: [x[S.seed_hi], x[S.seed_lo]],
    shake: x[S.shake],
    flash: x[S.flash],
    exiting: x[S.exiting],
    wipe: x[S.wipe],
    tries: x[S.tries],
    cue: x[S.cue_on] === 0 ? null : [x[S.cue_x] | 0, x[S.cue_y] | 0, x[S.cue_t]],
    next_foe: x[S.next_foe],
    done: x[S.done_on] === 0 ? null : x[S.done_t],
    paid: x[S.paid],
    kept,
  }
}

const answer = w => {
  const x = words64(w.s)
  return {
    session: session(w),
    sounds: list(w.snd),
    stop: x[SLOT.Sc.stop],
    won: x[SLOT.Sc.won] !== 0,
  }
}

const word = U

module.exports = {
  apiVersion: 1,
  // Exposed for the harnesses that replay a corpus; not part of the leg.
  codec: { world, session, answer },
  render(input = {}) {
    switch (input.op) {
      case 'tick':
        return answer(bend.tick(world(input.session, input.here, input.rules)))
      case 'upto':
        return answer(bend.upto(word(input.phases), world(input.session, input.here, input.rules)))
      case 'new':
        return answer(bend.new_session(world(BLANK, input.here, input.rules)))
      default:
        throw Error(`unknown op ${JSON.stringify(input.op)}`)
    }
  },
}
