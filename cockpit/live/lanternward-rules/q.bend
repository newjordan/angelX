# What stands in a room and what a body can ask of it: the effects every rule
# reaches for (a sound, a puff, a drop, the dice), the ground a body is stopped
# by, the lights, the locks. These are crates/delve/src/rules/lanternward/
# props.rs and the small questions the other files ask of the session. Every
# function takes the world and gives it back; a question gives a word or a
# Bool. Positions are Q pixels, signed words.
import Base
import ./r.bend as R
import ./c.bend as C
import ./g.bend as G
import ./m.bend as M
import ./l.bend as L
import ./n.bend as N
import ./gd.bend as Gd
import ./w.bend as W
import ./pr.bend as Pr
import ./dr.bend as Dr
import ./fx.bend as Fx
import ./bo.bend as Bo
import ./nr.bend as Nr
import ./se.bend as Seat
import ./fe.bend as Foe
import ./b.bend as B

def S() -> Data: R.W

# ----- what a tick tells the rest of the Delve ------------------------------

# A sound, once however many times it is asked for.
def sound(+w: S, +c: U32) -> S:
  Bool.pick(S, L.has(W.snd(w), c), w, W.set_snd(w, L.push(W.snd(w), c)))

# The heaviest impact of the tick.
def stop(+w: S, +s: U32) -> S: W.set_stop(w, U32.max(W.stop(w), s))

# A puff, a spark or a ring for the picture, if there is room for one.
def fx(+w: S, +kind: U32, +x: U32, +y: U32, +a: U32) -> S:
  Bool.pick(S, U32.is_lt(L.len8(W.fx(w)), C.fx_cap()), W.set_fx(w, L.push8(W.fx(w), R.W8{kind, x, y, 0, a, 0, 0, 0})), w)

# Something to pick up lies where it fell.
def drop_at(+w: S, +kind: U32, +x: U32, +y: U32) -> S:
  Bool.pick(S, U32.is_lt(L.len8(W.drops(w)), C.drop_cap()), W.set_drops(w, L.push8(W.drops(w), R.W8{kind, x, y, 0, 0, 0, 0, 0})), w)

# ----- the dice ---------------------------------------------------------------

# The fixed dice: the session's seed steps as a 64-bit linear congruence, and
# the roll is its high bits modulo n. The answer is left in `ret`.
def roll(+w: S, +n: U32) -> S:
  +ah = W.seed_hi(w)
  +al = W.seed_lo(w)
  +p = N.mul32(al, 1284865837)
  +lo0 = N.u64_lo(p)
  +hi0 = U32.add(U32.add(N.u64_hi(p), U32.mul(ah, 1284865837)), U32.mul(al, 1481765933))
  +lo1 = U32.add(lo0, 4150755663)
  +hi1 = U32.add(U32.add(hi0, 335903614), Bool.to_u32(U32.is_lt(lo1, lo0)))
  W.set_ret(W.set_seed_lo(W.set_seed_hi(w, hi1), lo1), U32.mod(U32.shr(hi1), U32.max(n, 1)))

# ----- what a thing is -------------------------------------------------------

# Whether it is lit now: the resting braziers and the ember bowl always, a
# brazier while its flame lasts or once lit for good, the great brazier and the
# beacon when they are on.
def lit(+p: R.W8) -> Bool:
  +k = Pr.kind(p)
  U32.is_eq(k, C.pk_rest()) || U32.is_eq(k, C.pk_ember()) || (U32.is_eq(k, C.pk_brazier()) && (U32.is_ne(Pr.t(p), 0) || U32.is_eq(Pr.on(p), 1))) || ((U32.is_eq(k, C.pk_great()) || U32.is_eq(k, C.pk_beacon())) && U32.is_eq(Pr.on(p), 1))

# Whether the knight in front of it can lock on to it.
def target(+p: R.W8) -> Bool:
  +k = Pr.kind(p)
  U32.is_eq(k, C.pk_sign()) || U32.is_eq(k, C.pk_folk()) || ((U32.is_eq(k, C.pk_crystal()) || U32.is_eq(k, C.pk_great()) || U32.is_eq(k, C.pk_lever())) && U32.is_eq(Pr.on(p), 0)) || (U32.is_eq(k, C.pk_brazier()) && Bool.not(lit(p)))

# Whether a bolt flies through it: bars, gates, the great brazier's cage and the
# crystals' niches let light and iron pass.
def lets_bolts(+p: R.W8) -> Bool:
  +k = Pr.kind(p)
  U32.is_eq(k, C.pk_gate()) || U32.is_eq(k, C.pk_sigil()) || U32.is_eq(k, C.pk_grass()) || U32.is_eq(k, C.pk_plate()) || U32.is_eq(k, C.pk_fissure()) || U32.is_eq(k, C.pk_pot())

# Whether a body at this point, in tile (tx, ty), is stopped by it.
def blocks(+p: R.W8, +x: U32, +y: U32, +tx: U32, +ty: U32) -> Bool:
  +k = Pr.kind(p)
  +here = U32.is_eq(Pr.tx(p), tx) && U32.is_eq(Pr.ty(p), ty)
  +shut = U32.is_eq(Pr.on(p), 0) && here
  +free = U32.is_eq(k, C.pk_grass()) || U32.is_eq(k, C.pk_plate()) || U32.is_eq(k, C.pk_sigil()) || U32.is_eq(k, C.pk_fissure())
  +door = U32.is_eq(k, C.pk_pot()) || U32.is_eq(k, C.pk_gate()) || U32.is_eq(k, C.pk_lock()) || U32.is_eq(k, C.pk_boss()) || U32.is_eq(k, C.pk_cracked())
  +slab = N.slt(N.sabs(U32.sub(x, Pr.x(p))), 2048) && N.slt(N.sabs(U32.sub(y, Pr.y(p))), 2048)
  Bool.pick(Bool, free, False{}, Bool.pick(Bool, door, shut, Bool.pick(Bool, U32.is_eq(k, C.pk_chest()), U32.is_ne(Pr.on(p), 2) && here, Bool.pick(Bool, U32.is_eq(k, C.pk_block()), slab, here))))

# ----- the ground a body is stopped by ----------------------------------------

# The class of a tile: 0 open, 1 wall, 2 bars, 3 pit, 4 the way on.
def cls(+w: S, +tx: U32, +ty: U32) -> U32: Gd.at(W.grid(w), tx, ty)

def wall_at(+w: S, +tx: U32, +ty: U32) -> Bool:
  +c = cls(w, tx, ty)
  U32.is_eq(c, 1) || U32.is_eq(c, 2)

# Whether any closed thing in the room stops a body at this point.
def any_block(+ps: List<&2, R.W8>, +x: U32, +y: U32, +tx: U32, +ty: U32, +hit: Bool) -> Bool:
  match ps:
    case Nil{}:
      hit
    case Con{p, t}:
      match hit:
        case True{}:
          True{}
        case False{}:
          any_block(t, x, y, tx, ty, blocks(p, x, y, tx, ty))

def blocked(+w: S, +x: U32, +y: U32) -> Bool:
  +tx = G.tile_of(x)
  +ty = G.tile_of(y)
  wall_at(w, tx, ty) || any_block(W.props(w), x, y, tx, ty, False{})

# Whether a body of radius r fits at this point.
def clear(+w: S, +x: U32, +y: U32, +r: U32) -> Bool:
  Bool.not(blocked(w, U32.sub(x, r), U32.sub(y, r)) || blocked(w, U32.add(x, r), U32.sub(y, r)) || blocked(w, U32.sub(x, r), U32.add(y, r)) || blocked(w, U32.add(x, r), U32.add(y, r)))

# Whether this point is over a pit.
def pit(+w: S, +x: U32, +y: U32) -> Bool: U32.is_eq(cls(w, G.tile_of(x), G.tile_of(y)), 3)

# Whether a thing that is not let through by bolts stands at the point.
def any_stop(+ps: List<&2, R.W8>, +x: U32, +y: U32, +tx: U32, +ty: U32, +hit: Bool) -> Bool:
  match ps:
    case Nil{}:
      hit
    case Con{p, t}:
      match hit:
        case True{}:
          True{}
        case False{}:
          any_stop(t, x, y, tx, ty, Bool.not(lets_bolts(p)) && blocks(p, x, y, tx, ty))

# Whether the way from a to b is open to a bolt: only walls stop it. Each point
# along it, from the first, is tried until one stops it; the two ends' own tiles
# are not in the way.
def line_go(+n: Nat, +w: S, +i: U32, +steps: U32, +ax: U32, +ay: U32, +dx: U32, +dy: U32, +tax: U32, +tay: U32, +tbx: U32, +tby: U32, +ok: Bool) -> Bool:
  match n:
    case 0n:
      ok
    case 1n+p:
      match ok:
        case False{}:
          False{}
        case True{}:
          +x = U32.add(ax, N.sdiv(U32.mul(dx, i), steps))
          +y = U32.add(ay, N.sdiv(U32.mul(dy, i), steps))
          +tx = G.tile_of(x)
          +ty = G.tile_of(y)
          +ends = (U32.is_eq(tx, tax) && U32.is_eq(ty, tay)) || (U32.is_eq(tx, tbx) && U32.is_eq(ty, tby))
          line_go(p, w, U32.add(i, 1), steps, ax, ay, dx, dy, tax, tay, tbx, tby, ends || (U32.is_ne(cls(w, tx, ty), 1) && Bool.not(any_stop(W.props(w), x, y, tx, ty, False{}))))

def line_open(+w: S, +ax: U32, +ay: U32, +bx: U32, +by: U32) -> Bool:
  +dx = U32.sub(bx, ax)
  +dy = U32.sub(by, ay)
  +steps = U32.max(U32.div(G.dist(dx, dy), 1536), 1)
  line_go(U32.to_nat(U32.sub(steps, 1)), w, 1, steps, ax, ay, dx, dy, G.tile_of(ax), G.tile_of(ay), G.tile_of(bx), G.tile_of(by), True{})

# ----- the knights and the lights ---------------------------------------------

# Standing and awake: can be hurt, can hurt, can be a foe's target.
def up(+k: R.T8) -> Bool: U32.is_ne(Seat.hp(k), 0) && U32.is_ne(Seat.act(k), C.a_down())

def flaring(+k: R.T8) -> Bool: U32.is_ge(Seat.lamp(k), C.flare_after()) && U32.is_ne(Seat.flame(k), 0) && U32.is_ne(Seat.act(k), C.a_down())

# Whether a flare or a burning brazier holds the point lit: what strips a
# snuffer's cloak. A knight's flare is judged over the seats at the table.
def burning_seats(+ks: List<&2, R.T8>, +x: U32, +y: U32, +hit: Bool) -> Bool:
  match ks:
    case Nil{}:
      hit
    case Con{k, t}:
      match hit:
        case True{}:
          True{}
        case False{}:
          burning_seats(t, x, y, flaring(k) && N.slt(G.dist(U32.sub(Seat.x(k), x), U32.sub(Seat.y(k), y)), U32.mul(C.flare_r(), 256)))

def burning_props(+ps: List<&2, R.W8>, +x: U32, +y: U32, +hit: Bool) -> Bool:
  match ps:
    case Nil{}:
      hit
    case Con{p, t}:
      match hit:
        case True{}:
          True{}
        case False{}:
          burning_props(t, x, y, lit(p) && N.slt(G.dist(U32.sub(Pr.x(p), x), U32.sub(Pr.y(p), y)), U32.mul(C.brazier_r(), 256)))

def burning(+w: S, +x: U32, +y: U32) -> Bool: burning_seats(W.seats(w), x, y, False{}) || burning_props(W.props(w), x, y, False{})

# The living knight nearest a point, by distance then seat: ok, id, x, y, d.
def near_go(+ks: List<&2, R.T8>, +x: U32, +y: U32, +best: R.W8) -> R.W8:
  match ks:
    case Nil{}:
      best
    case Con{k, t}:
      +d = G.dist(U32.sub(Seat.x(k), x), U32.sub(Seat.y(k), y))
      +better = up(k) && (U32.is_eq(Nr.ok(best), 0) || U32.is_lt(d, Nr.d(best)) || (U32.is_eq(d, Nr.d(best)) && U32.is_lt(Seat.id(k), Nr.id(best))))
      near_go(t, x, y, Bool.pick(R.W8, better, R.W8{1, Seat.id(k), Seat.x(k), Seat.y(k), d, 0, 0, 0}, best))

def nearest_knight(+w: S, +x: U32, +y: U32) -> R.W8: near_go(W.seats(w), x, y, R.W8{0, 0, 0, 0, 0, 0, 0, 0})

# ----- what a lock holds --------------------------------------------------------

def foe_alive(+f: R.T8) -> Bool: U32.is_ne(Foe.hp(f), 0) && U32.is_ne(Foe.st(f), C.fs_dying())

# The Warden's lamp-heart: in its chest, toward where it faces.
def heart_x(+w: S) -> U32: U32.add(B.x(w), U32.mul(B.fx(w), 9))

def heart_y(+w: S) -> U32: U32.add(B.y(w), U32.mul(B.fy(w), 9))

def boss_up(+w: S) -> Bool: U32.is_ne(W.boss_on(w), 0) && U32.is_ne(B.hp(w), 0)

def foe_point(+fs: List<&2, R.T8>, +id: U32) -> R.W8:
  match fs:
    case Nil{}:
      R.W8{0, 0, 0, 0, 0, 0, 0, 0}
    case Con{f, t}:
      Bool.pick(R.W8, U32.is_eq(Foe.id(f), id) && foe_alive(f), R.W8{1, 0, Foe.x(f), Foe.y(f), 0, 0, 0, 0}, foe_point(t, id))

# Where a lock holds, if the thing is still there: ok, x, y.
def lock_point(+w: S, +kind: U32, +i: U32) -> R.W8:
  +p = L.get8(W.props(w), U32.to_nat(i))
  Bool.pick(R.W8,
    U32.is_eq(kind, C.lk_foe()), foe_point(W.foes(w), i),
    Bool.pick(R.W8, U32.is_eq(kind, C.lk_boss()),
      Bool.pick(R.W8, boss_up(w), R.W8{1, 0, heart_x(w), heart_y(w), 0, 0, 0, 0}, R.W8{0, 0, 0, 0, 0, 0, 0, 0}),
      Bool.pick(R.W8, U32.is_eq(kind, C.lk_prop()) && U32.is_lt(i, L.len8(W.props(w))) && target(p), R.W8{1, 0, Pr.x(p), Pr.y(p), 0, 0, 0, 0}, R.W8{0, 0, 0, 0, 0, 0, 0, 0})))

# ----- puzzles --------------------------------------------------------------------

# The gate with this number, opened or shut: whether any changed.
def gate_go(+ps: List<&2, R.W8>, +id: U32, +want: U32) -> List<&2, R.W8>:
  match ps:
    case Nil{}:
      Nil{}
    case Con{p, t}:
      Con{Bool.pick(R.W8, U32.is_eq(Pr.kind(p), C.pk_gate()) && U32.is_eq(Pr.id(p), id), Pr.set_on(p, want), p), gate_go(t, id, want)}

def gate_changes(+ps: List<&2, R.W8>, +id: U32, +want: U32) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{p, t}:
      (U32.is_eq(Pr.kind(p), C.pk_gate()) && U32.is_eq(Pr.id(p), id) && U32.is_ne(Pr.on(p), want)) || gate_changes(t, id, want)

# Open (1) or shut (0) every gate with this number; `ret` says whether any
# gate moved.
def set_gate(+w: S, +id: U32, +want: U32) -> S:
  W.set_ret(W.set_props(w, gate_go(W.props(w), id, want)), Bool.to_u32(gate_changes(W.props(w), id, want)))

# A puzzle came right: the chime, the star burst, a cue on the screen.
def solved(+w: S, +x: U32, +y: U32) -> S:
  +w1 = fx(w, C.fxk_solved(), x, y, 0)
  +w2 = Bool.pick(S, U32.is_eq(W.cue_on(w1), 0), W.set_cue_t(W.set_cue_y(W.set_cue_x(W.set_cue_on(w1, 1), x), y), C.cue_ticks()), w1)
  sound(w2, 45)

# ----- loot ----------------------------------------------------------------------

# What a cut or broken thing lets fall: a roll of a hundred picks a heart, a
# gem, a bomb or nothing by what broke.
def scatter_pick(+kind: U32, +r: U32) -> U32:
  Bool.pick(U32, U32.is_eq(kind, C.pk_grass()),
    Bool.pick(U32, U32.is_lt(r, 12), 1, Bool.pick(U32, U32.is_lt(r, 22), 0, 9)),
    Bool.pick(U32, U32.is_eq(kind, C.pk_pot()),
      Bool.pick(U32, U32.is_lt(r, 18), 0, Bool.pick(U32, U32.is_lt(r, 45), 1, Bool.pick(U32, U32.is_lt(r, 55), 2, 9))),
      Bool.pick(U32, U32.is_lt(r, 30), 0, Bool.pick(U32, U32.is_lt(r, 65), 1, 9))))

def scatter(+w: S, +kind: U32, +x: U32, +y: U32) -> S:
  +w1 = roll(w, 100)
  +d = scatter_pick(kind, W.ret(w1))
  Bool.pick(S, U32.is_lt(d, 9), drop_at(w1, d, x, y), w1)

# A pot or a tuft of grass breaks: what it held lies on the floor. A bomb pot
# always holds three bombs.
def shatter_loot(+bombs: Bool, +w: S, +kind: U32, +x: U32, +y: U32) -> S:
  match bombs:
    case True{}:
      drop_at(drop_at(drop_at(w, C.dk_bomb(), U32.sub(x, 1280), U32.add(y, 768)), C.dk_bomb(), x, U32.add(y, 768)), C.dk_bomb(), U32.add(x, 1280), U32.add(y, 768))
    case False{}:
      scatter(w, kind, x, y)

def shatter(+w: S, +i: U32) -> S:
  +p = L.get8(W.props(w), U32.to_nat(i))
  +kind = Pr.kind(p)
  +w1 = W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_on(p, 1)))
  +w2 = fx(w1, C.fxk_dust(), Pr.x(p), Pr.y(p), 0)
  +w3 = shatter_loot(U32.is_eq(kind, C.pk_pot()) && U32.is_eq(Pr.id(p), 1), w2, kind, Pr.x(p), Pr.y(p))
  sound(w3, Bool.pick(U32, U32.is_eq(kind, C.pk_pot()), 28, 39))

# A crystal wakes: the chime, and whatever it is wired to (the stage rows read
# the prop's `on`).
def light_crystal(+w: S, +i: U32) -> S:
  +p = L.get8(W.props(w), U32.to_nat(i))
  Bool.pick(S, U32.is_eq(Pr.on(p), 1), w, solved(W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_on(p, 1))), Pr.x(p), Pr.y(p)))
