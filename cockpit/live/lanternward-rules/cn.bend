# The one context button. E opens, reads, lifts, throws, takes a flame, lights,
# unlocks, relights; its label changes with what the knight faces. With nothing
# to do it is the roll, or, locked on, the hop. crates/delve/src/rules/
# lanternward/ctx.rs. The knight acting is `cur`.
import Base
import ./r.bend as R
import ./c.bend as C
import ./n.bend as N
import ./m.bend as M
import ./l.bend as L
import ./g.bend as G
import ./w.bend as W
import ./pr.bend as Pr
import ./cx.bend as Cx
import ./ct.bend as Ct
import ./bo.bend as Bo
import ./se.bend as Seat
import ./k.bend as K
import ./z.bend as Z
import ./q.bend as Q
import ./ht.bend as H

def S() -> Data: R.W

# ----- what E would do -----------------------------------------------------------------

# Ticks a brazier lit from an ember burns: shorter the more hands.
def burn_ticks(+w: S) -> U32: U32.max(M.sat(C.burn(), U32.mul(C.burn_step(), M.dec(U32.max(L.len64(W.seats(w)), 1)))), C.burn_min())

def beacon_ready(+w: S) -> Bool: U32.is_eq(W.room(w), 4) && U32.is_ge(W.stage(w), 3)

def carrying_boss_key(+w: S) -> Bool: U32.is_eq(W.hold_k(w), C.hold_carried()) && U32.is_eq(W.hold_a(w), K.id(w))

# What E does to this thing for the knight, if anything: its code, or none.
def prop_ctx(+w: S, +p: R.W8) -> U32:
  +k = Pr.kind(p)
  +off = U32.is_eq(Pr.on(p), 0)
  Bool.pick(U32, U32.is_eq(k, C.pk_chest()) && off, C.cx_open(),
  Bool.pick(U32, U32.is_eq(k, C.pk_sign()), C.cx_read(),
  Bool.pick(U32, U32.is_eq(k, C.pk_folk()), C.cx_talk(),
  Bool.pick(U32, U32.is_eq(k, C.pk_lever()) && off, C.cx_pull(),
  Bool.pick(U32, U32.is_eq(k, C.pk_pot()) && off && U32.is_eq(Pr.id(p), 0), C.cx_lift(),
  Bool.pick(U32, U32.is_eq(k, C.pk_ember()), C.cx_flame(),
  Bool.pick(U32, U32.is_eq(k, C.pk_brazier()) && Bool.not(Q.lit(p)) && U32.is_ne(K.taper(w), 0), C.cx_light(),
  Bool.pick(U32, U32.is_eq(k, C.pk_lock()) && off && U32.is_ne(W.p_keys(w), 0), C.cx_unlock(),
  Bool.pick(U32, U32.is_eq(k, C.pk_boss()) && off && carrying_boss_key(w), C.cx_unlock(),
  Bool.pick(U32, U32.is_eq(k, C.pk_beacon()) && off && beacon_ready(w), C.cx_relight(), C.cx_none()))))))))))

# The nearest thing in reach and in front of the knight: the best so far is
# (found, distance, code, index).
def ctx_props(+ps: List<&2, R.W8>, +w: S, +i: U32, +best: R.W8) -> R.W8:
  match ps:
    case Nil{}:
      best
    case Con{p, t}:
      +c = prop_ctx(w, p)
      +dx = U32.sub(Pr.x(p), K.x(w))
      +dy = U32.sub(Pr.y(p), K.y(w))
      +d = G.dist(dx, dy)
      +u = G.unit(dx, dy)
      +ok = U32.is_ne(c, C.cx_none()) && U32.is_le(d, U32.mul(C.ctx_reach(), 256)) && Bool.not(U32.is_gt(d, U32.mul(12, 256)) && N.slt(G.dot(K.fx(w), K.fy(w), G.vx(u), G.vy(u)), C.ctx_front())) && (U32.is_eq(Cx.ok(best), 0) || U32.is_lt(d, Cx.d(best)))
      ctx_props(t, w, U32.add(i, 1), Bool.pick(R.W8, ok, R.W8{1, d, c, i, 0, 0, 0, 0}, best))

# A friend on the ground within reach comes before anything else: the first by
# seat.
def down_near(+ks: List<&2, R.T8>, +w: S, +found: U32, +done: Bool) -> U32:
  match ks:
    case Nil{}:
      found
    case Con{o, t}:
      match done:
        case True{}:
          found
        case False{}:
          +hit = U32.is_eq(Seat.act(o), C.a_down()) && N.slt(G.dist(U32.sub(Seat.x(o), K.x(w)), U32.sub(Seat.y(o), K.y(w))), U32.mul(C.revive_reach(), 256))
          down_near(t, w, Bool.pick(U32, hit, Seat.id(o), found), hit)

# The nearest thing in reach and in front of the knight that E would act on:
# its code, then the index (or a knight's id) it acts on.
def context(+w: S) -> R.W8:
  +friend = down_near(W.seats(w), w, C.none(), False{})
  +best = ctx_props(W.props(w), w, 0, R.W8{0, 0, 0, 0, 0, 0, 0, 0})
  Bool.pick(R.W8, U32.is_eq(K.carry(w), 1), R.W8{C.cx_throw(), 0, 0, 0, 0, 0, 0, 0},
    Bool.pick(R.W8, U32.is_ne(friend, C.none()), R.W8{C.cx_revive(), friend, 0, 0, 0, 0, 0, 0},
      Bool.pick(R.W8, U32.is_eq(Cx.ok(best), 1), R.W8{Cx.c(best), Cx.i(best), 0, 0, 0, 0, 0, 0}, R.W8{C.cx_none(), 0, 0, 0, 0, 0, 0, 0})))

# ----- what a chest holds and what a knight is given ------------------------------------------

# What a chest holds, by its room and its place in it: the item, then its number.
def chest_item(+room: U32, +id: U32) -> R.W8:
  Bool.pick(R.W8, U32.is_eq(room, 0), R.W8{C.it_map(), 0, 0, 0, 0, 0, 0, 0},
  Bool.pick(R.W8, U32.is_eq(room, 1), Bool.pick(R.W8, U32.is_eq(id, 0), R.W8{C.it_lantern(), 0, 0, 0, 0, 0, 0, 0}, R.W8{C.it_gems(), 25, 0, 0, 0, 0, 0, 0}),
  Bool.pick(R.W8, U32.is_eq(room, 2), R.W8{C.it_gems(), 20, 0, 0, 0, 0, 0, 0},
  Bool.pick(R.W8, U32.is_eq(room, 3), Bool.pick(R.W8, U32.is_eq(id, 0), R.W8{C.it_bosskey(), 0, 0, 0, 0, 0, 0, 0}, Bool.pick(R.W8, U32.is_eq(id, 1), R.W8{C.it_smallkey(), 0, 0, 0, 0, 0, 0, 0}, R.W8{C.it_gems(), 25, 0, 0, 0, 0, 0, 0})),
    R.W8{C.it_gems(), 10, 0, 0, 0, 0, 0, 0}))))

def flame_all(+ks: List<&2, R.T8>, +flame: U32) -> List<&2, R.T8>:
  match ks:
    case Nil{}:
      Nil{}
    case Con{o, t}:
      Con{Seat.set_flame(o, flame), flame_all(t, flame)}

def heart_all(+ks: List<&2, R.T8>) -> List<&2, R.T8>:
  match ks:
    case Nil{}:
      Nil{}
    case Con{o, t}:
      +mx = U32.add(Seat.max_hp(o), C.heart())
      Con{Seat.set_hp(Seat.set_max_hp(o, mx), mx), heart_all(t)}

# The item's gift to the party: a map, the lantern with a full tank for all, a
# key, a heart container (every knight's hearts full), gems, bombs.
def gift(+w: S, +id: U32, +item: U32, +n: U32) -> S:
  +flame = W.r_flame(w)
  +mx = U32.add(K.max_hp(w), C.heart())
  Bool.pick(S, U32.is_eq(item, C.it_map()), W.set_p_map(w, 1),
  Bool.pick(S, U32.is_eq(item, C.it_lantern()), K.set_flame(W.set_seats(W.set_p_lantern(w, 1), flame_all(W.seats(w), flame)), flame),
  Bool.pick(S, U32.is_eq(item, C.it_smallkey()), W.set_p_keys(w, U32.min(U32.add(W.p_keys(w), 1), 255)),
  Bool.pick(S, U32.is_eq(item, C.it_bosskey()), W.set_hold_y(W.set_hold_x(W.set_hold_a(W.set_hold_k(w, C.hold_carried()), id), K.x(w)), K.y(w)),
  Bool.pick(S, U32.is_eq(item, C.it_container()), K.set_hp(K.set_max_hp(W.set_seats(W.set_p_cont(w, U32.min(U32.add(W.p_cont(w), 1), 255)), heart_all(W.seats(w))), mx), mx),
  Bool.pick(S, U32.is_eq(item, C.it_gems()), K.set_gems(w, U32.add(K.gems(w), n)), K.set_bombs(w, U32.add(K.bombs(w), n))))))))

# An item comes to the party; its finder holds it up.
def grant(+w: S, +id: U32, +item: U32, +n: U32) -> S:
  +w1 = gift(w, id, item, n)
  +w2 = K.set_charge(K.set_step(K.set_chain(K.set_shield(K.set_lock_i(K.set_lock_k(K.set_item_n(K.set_item_k(K.set_lamp(K.set_t(K.set_act(w1, C.a_item()), 0), 0), item), n), 0), 0), 0), 0), 0), 0)
  +w3 = W.set_ban_t(W.set_ban_seat(W.set_ban_n(W.set_ban_k(W.set_ban_on(w2, 1), item), n), id), C.item_ticks())
  Q.sound(w3, Bool.pick(U32, U32.is_eq(item, C.it_gems()) || U32.is_eq(item, C.it_bombs()), 37, 44))

def open_chest(+w: S, +id: U32, +i: U32) -> S:
  +p = L.get8(W.props(w), U32.to_nat(i))
  +it = chest_item(W.room(w), Pr.id(p))
  +w1 = W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_on(p, 1)))
  +w2 = Q.fx(Q.sound(w1, 8), C.fxk_light(), Pr.x(p), Pr.y(p), 0)
  grant(w2, id, Cx.ok(it), Cx.d(it))

# ----- the context button pressed ----------------------------------------------------------------

def read_sign(+w: S, +i: U32, +base: U32) -> S:
  +p = L.get8(W.props(w), U32.to_nat(i))
  Q.sound(K.set_read_t(K.set_read_id(K.set_read_on(w, 1), U32.add(base, Pr.id(p))), C.read_ticks()), 7)

# The index of the first gate with this number, or none.
def gate_find(+ps: List<&2, R.W8>, +id: U32, +j: U32, +found: U32, +done: Bool) -> U32:
  match ps:
    case Nil{}:
      found
    case Con{p, t}:
      match done:
        case True{}:
          found
        case False{}:
          +hit = U32.is_eq(Pr.kind(p), C.pk_gate()) && U32.is_eq(Pr.id(p), id)
          gate_find(t, id, U32.add(j, 1), Bool.pick(U32, hit, j, found), hit)

def pull_lever(+w: S, +i: U32) -> S:
  +p = L.get8(W.props(w), U32.to_nat(i))
  +w1 = Q.sound(W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_on(p, 1))), 42)
  +g = gate_find(W.props(w1), 2, 0, C.none(), False{})
  +gp = L.get8(W.props(w1), U32.to_nat(g))
  Bool.pick(S, U32.is_ne(g, C.none()), Q.sound(Q.solved(Q.set_gate(w1, 2, 1), Pr.x(gp), Pr.y(gp)), 13), w1)

def lift_pot(+w: S, +i: U32) -> S:
  +p = L.get8(W.props(w), U32.to_nat(i))
  Q.sound(K.set_carry(W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_on(p, 1))), 1), 2)

def pot_bolt(+x: U32, +y: U32, +fx: U32, +fy: U32, +id: U32) -> R.W8:
  R.W8{C.bk_pot(), U32.add(x, U32.mul(fx, 8)), U32.add(y, U32.mul(fy, 8)), N.sdiv(U32.mul(fx, C.pot_speed()), 256), N.sdiv(U32.mul(fy, C.pot_speed()), 256), C.pot_life(), id, 0}

def throw_pot(+w: S, +id: U32) -> S:
  +w1 = K.set_carry(w, 0)
  Q.sound(Bool.pick(S, U32.is_lt(L.len8(W.bolts(w1)), C.bolt_cap()), W.set_bolts(w1, L.push8(W.bolts(w1), pot_bolt(K.x(w), K.y(w), K.fx(w), K.fy(w), id))), w1), 2)

def take_flame(+w: S) -> S: Q.sound(Q.fx(K.set_taper(w, C.taper()), C.fxk_light(), K.x(w), U32.sub(K.y(w), 2048), 0), 36)

def light_brazier(+w: S, +i: U32) -> S:
  +p = L.get8(W.props(w), U32.to_nat(i))
  Q.sound(Q.fx(W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_t(p, burn_ticks(w)))), C.fxk_light(), Pr.x(p), Pr.y(p), 0), 36)

def unlock(+w: S, +i: U32) -> S:
  +p = L.get8(W.props(w), U32.to_nat(i))
  +door = U32.is_eq(Pr.kind(p), C.pk_lock())
  +w1 = Bool.pick(S, door, W.set_p_keys(w, M.dec(W.p_keys(w))), W.set_hold_k(w, C.hold_spent()))
  Q.sound(Q.solved(W.set_props(w1, L.set8(W.props(w1), U32.to_nat(i), Pr.set_on(p, 1))), Pr.x(p), Pr.y(p)), 13)

# E pressed with a thing in reach. `ret` is 0 when there is nothing to do (the
# caller rolls or hops); a revive or a relight is held, not pressed, so it
# counts as done and does nothing here.
# What E does, by what it meets: a revive or a relight is held, not pressed, so
# it does nothing here.
def use_dispatch(+ds: Nat, +w: S, +id: U32, +i: U32) -> S:
  match ds:
    case 0n:
      w
    case 1n+a1:
      match a1:
        case 0n:
          open_chest(w, id, i)
        case 1n+a2:
          match a2:
            case 0n:
              read_sign(w, i, 0)
            case 1n+a3:
              match a3:
                case 0n:
                  lift_pot(w, i)
                case 1n+a4:
                  match a4:
                    case 0n:
                      throw_pot(w, id)
                    case 1n+a5:
                      match a5:
                        case 0n:
                          take_flame(w)
                        case 1n+a6:
                          match a6:
                            case 0n:
                              light_brazier(w, i)
                            case 1n+a7:
                              match a7:
                                case 0n:
                                  unlock(w, i)
                                case 1n+a8:
                                  match a8:
                                    case 0n:
                                      w
                                    case 1n+a9:
                                      match a9:
                                        case 0n:
                                          w
                                        case 1n+a10:
                                          match a10:
                                            case 0n:
                                              read_sign(w, i, 100)
                                            case 1n+a11:
                                              match a11:
                                                case 0n:
                                                  pull_lever(w, i)
                                                case 1n+a12:
                                                  w

def use_context(+w: S, +id: U32) -> S:
  +c = context(w)
  +i = Ct.i(c)
  W.set_ret(use_dispatch(U32.to_nat(Ct.k(c)), w, id, i), Bool.to_u32(Bool.not(U32.is_eq(Ct.k(c), C.cx_none()))))

# ----- a block pushed -------------------------------------------------------------------------------

# Whether any thing but plates and the block itself stands where the block
# would go.
def slide_stops(+ps: List<&2, R.W8>, +b: U32, +j: U32, +x: U32, +y: U32, +tx: U32, +ty: U32, +hit: Bool) -> Bool:
  match ps:
    case Nil{}:
      hit
    case Con{q, t}:
      match hit:
        case True{}:
          True{}
        case False{}:
          slide_stops(t, b, U32.add(j, 1), x, y, tx, ty, U32.is_ne(j, b) && U32.is_ne(Pr.kind(q), C.pk_plate()) && Q.blocks(q, x, y, tx, ty))

# Start a block sliding one tile the way it was pushed, if the place is open: not
# onto a tile against a wall, a pit or the way on, so none is ever stuck for good.
def start_slide(+w: S, +b: U32) -> S:
  +p = L.get8(W.props(w), U32.to_nat(b))
  +on = Pr.on(p)
  +dx = Bool.pick(U32, U32.is_eq(on, 1), 1, Bool.pick(U32, U32.is_eq(on, 2), N.sneg(1), 0))
  +dy = Bool.pick(U32, U32.is_eq(on, 3), 1, Bool.pick(U32, U32.is_eq(on, 4), N.sneg(1), 0))
  +nx = U32.add(Pr.x(p), U32.mul(dx, 4096))
  +ny = U32.add(Pr.y(p), U32.mul(dy, 4096))
  +dtx = G.tile_of(nx)
  +dty = G.tile_of(ny)
  +against = U32.is_ne(Q.cls(w, U32.add(dtx, 1), dty), 0) || U32.is_ne(Q.cls(w, U32.sub(dtx, 1), dty), 0) || U32.is_ne(Q.cls(w, dtx, U32.add(dty, 1)), 0) || U32.is_ne(Q.cls(w, dtx, U32.sub(dty, 1)), 0)
  +open = Bool.not(against) && U32.is_eq(Q.cls(w, dtx, dty), 0) && Bool.not(slide_stops(W.props(w), b, 0, nx, ny, dtx, dty, False{}))
  W.set_props(w, L.set8(W.props(w), U32.to_nat(b), Bool.pick(R.W8, open, Pr.set_t(p, 16), Pr.set_t(Pr.set_on(p, 0), 0))))
