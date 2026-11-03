# The adventure's rooms, one after another: what is in each, what each asks of the
# party, the way through, the fallen and the retry. crates/delve/src/rules/
# lanternward/stage.rs, and the Warden's beacon from boss.rs.
import Base
import ./r.bend as R
import ./c.bend as C
import ./n.bend as N
import ./m.bend as M
import ./l.bend as L
import ./g.bend as G
import ./gr.bend as Gr
import ./gd.bend as Gd
import ./w.bend as W
import ./pr.bend as Pr
import ./dr.bend as Dr
import ./pt.bend as Pt
import ./fx.bend as Fx
import ./nr.bend as Nr
import ./se.bend as Seat
import ./k.bend as K
import ./fe.bend as Foe
import ./b.bend as B
import ./z.bend as Z
import ./q.bend as Q
import ./ht.bend as H
import ./fo.bend as Fo
import ./bs.bend as Bs
import ./cn.bend as X

def S() -> Data: R.W

# ----- the room's things ------------------------------------------------------------------

# The kind of thing a letter marks, or none.
def pk_of(+c: U32) -> U32:
  Bool.pick(U32, U32.is_eq(c, 111) || U32.is_eq(c, 120), 0,
  Bool.pick(U32, U32.is_eq(c, 34), 1,
  Bool.pick(U32, U32.is_eq(c, 115), 2,
  Bool.pick(U32, U32.is_eq(c, 66), 3,
  Bool.pick(U32, U32.is_eq(c, 67), 4,
  Bool.pick(U32, U32.is_eq(c, 69), 5,
  Bool.pick(U32, U32.is_eq(c, 98), 6,
  Bool.pick(U32, U32.is_eq(c, 99), 7,
  Bool.pick(U32, U32.is_eq(c, 71) || (U32.is_ge(c, 49) && U32.is_le(c, 57)), 8,
  Bool.pick(U32, U32.is_eq(c, 80) || U32.is_eq(c, 89), 9,
  Bool.pick(U32, U32.is_eq(c, 88), 10,
  Bool.pick(U32, U32.is_eq(c, 75), 11,
  Bool.pick(U32, U32.is_eq(c, 126), 12,
  Bool.pick(U32, U32.is_eq(c, 78), 13,
  Bool.pick(U32, U32.is_eq(c, 76), 14,
  Bool.pick(U32, U32.is_eq(c, 77), 15,
  Bool.pick(U32, U32.is_eq(c, 42), 16,
  Bool.pick(U32, U32.is_eq(c, 102), 17,
  Bool.pick(U32, U32.is_eq(c, 70), 18,
  Bool.pick(U32, U32.is_eq(c, 47), 19, C.none()))))))))))))))))))))

# How many things of a kind are already laid.
def count_kind(+ps: List<&2, R.W8>, +kind: U32) -> U32:
  match ps:
    case Nil{}:
      0
    case Con{p, t}:
      U32.add(Bool.to_u32(U32.is_eq(Pr.kind(p), kind)), count_kind(t, kind))

# A gate's number (the exit is 0), a chest's, sign's or folk's place in its room,
# a bomb pot or the second plate is 1.
def prop_id(+acc: List<&2, R.W8>, +kind: U32, +c: U32) -> U32:
  Bool.pick(U32, U32.is_eq(kind, C.pk_gate()), Bool.pick(U32, U32.is_eq(c, 71), 0, U32.sub(c, 48)),
  Bool.pick(U32, U32.is_eq(kind, C.pk_chest()) || U32.is_eq(kind, C.pk_sign()) || U32.is_eq(kind, C.pk_folk()), count_kind(acc, kind),
  Bool.pick(U32, U32.is_eq(kind, C.pk_pot()) || U32.is_eq(kind, C.pk_plate()), Bool.to_u32(U32.is_eq(c, 120) || U32.is_eq(c, 89)), 0)))

# One tile's thing, if it holds one. The vault's key chest is not there until the
# dark is beaten (on = 2); a resting brazier burns for good.
def lay(+acc: List<&2, R.W8>, +room: U32, +x: U32, +y: U32, +c: U32) -> List<&2, R.W8>:
  +kind = pk_of(c)
  +id = prop_id(acc, kind, c)
  +on = Bool.pick(U32, U32.is_eq(kind, C.pk_chest()) && U32.is_eq(room, 3) && U32.is_eq(id, 1), 2, Bool.pick(U32, U32.is_eq(kind, C.pk_rest()), 1, 0))
  Bool.pick(List<&2, R.W8>, U32.is_eq(kind, C.none()), acc, L.push8(acc, R.W8{kind, x, y, id, on, 0, G.middle(x), G.middle(y)}))

def lay_row(s: String, +x: U32, +y: U32, +room: U32, +acc: List<&2, R.W8>) -> List<&2, R.W8>:
  match s:
    case SNil{}:
      acc
    case SCon{head, tail}:
      lay_row(tail, U32.add(x, 1), y, room, lay(acc, room, x, y, Char.to_u32(head)))

def lay_rows(rows: List<String>, +y: U32, +room: U32, +acc: List<&2, R.W8>) -> List<&2, R.W8>:
  match rows:
    case Nil{}:
      acc
    case Con{h, t}:
      lay_rows(t, U32.add(y, 1), room, lay_row(h, 0, y, room, acc))

# The room's things, in the order the plan's rows list them.
def build(+room: U32) -> List<&2, R.W8>: lay_rows(Gr.rows_of(Gr.plan(room)), 0, room, Nil{})

# ----- where the party stands -------------------------------------------------------------------

def at_start(+ks: List<&2, R.T8>, +room: U32) -> List<&2, R.T8>:
  match ks:
    case Nil{}:
      Nil{}
    case Con{k, t}:
      +x = Gr.start_x(room, Seat.id(k))
      +y = Gr.start_y(room, Seat.id(k))
      +k1 = Seat.set_safey(Seat.set_safex(Seat.set_camy(Seat.set_camx(Seat.set_y(Seat.set_x(k, x), y), x), y), x), y)
      +k2 = Seat.set_t(Seat.set_act(Seat.set_lock_i(Seat.set_lock_k(Seat.set_z(k1, 0), 0), 0), C.a_free()), 0)
      +k3 = Seat.set_charge(Seat.set_step(Seat.set_chain(Seat.set_shield(k2, 0), 0), 0), 0)
      +k4 = Seat.set_taper(Seat.set_pusht(Seat.set_pushy(Seat.set_pushx(k3, 0), 0), 0), 0)
      +k5 = Seat.set_hp(k4, Bool.pick(U32, U32.is_eq(Seat.hp(k4), 0), U32.min(C.revive_hp(), Seat.max_hp(k4)), Seat.hp(k4)))
      Con{Seat.set_inv(k5, C.spawn_inv()), at_start(t, room)}

# The foes (and the Warden) a room opens with, from the marks of the plan: the first
# `n` marks of a letter, each a foe of a kind.
def spawn_marks(+n: Nat, +k: U32, +w: S, +room: U32, +code: U32, +kind: U32, +hooded: Bool, +count: U32) -> S:
  match n:
    case 0n:
      w
    case 1n+p:
      +mark = Gr.nth_hit(Gr.find(Gr.rows_of(Gr.plan(room)), code), k)
      spawn_marks(p, U32.add(k, 1), Z.lw(U32.is_lt(k, count), u => Fo.spawn_foe(w, kind, G.middle(U32.shrn(mark, 16n)), G.middle(U32.and(mark, 65535)), hooded), w), room, code, kind, hooded, count)

# The Lamp Hall's wave: the Gloam answers the Lantern.
def spawn_wave(+w: S) -> S:
  +seats = U32.max(L.len64(W.seats(w)), 1)
  +n = U32.min(U32.add(2, U32.div(U32.sub(seats, 1), 2)), 4)
  spawn_marks(U32.to_nat(n), 0, w, 1, 83, C.fk_snuffer(), False{}, Gr.hits(Gr.find(Gr.rows_of(Gr.plan(1)), 83)))

def lamps_burn(+ps: List<&2, R.W8>) -> List<&2, R.W8>:
  match ps:
    case Nil{}:
      Nil{}
    case Con{q, t}:
      Con{Bool.pick(R.W8, U32.is_eq(Pr.kind(q), C.pk_brazier()), Pr.set_on(q, 1), q), lamps_burn(t)}

def warden_in(+w: S, +mark: U32, +seats: U32) -> S:
  +full = U32.mul(C.warden_hp(), U32.max(seats, 1))
  +b1 = B.set_prov(B.set_full(B.set_hp(B.set_y(B.set_x(W.set_boss(w, L.zero64()), G.middle(U32.shrn(mark, 16n))), G.middle(U32.and(mark, 65535))), full), full), N.sclamp(seats, 1, C.max_players()))
  W.set_boss_on(B.set_aim(B.set_atk(B.set_fy(B.set_start(b1, full), 256), C.atk_slam()), 90), 1)

# The foes (and the Warden) a room opens with.
def provision_room(+w: S, +room: U32) -> S:
  +seats = U32.max(L.len64(W.seats(w)), 1)
  Z.lz(U32.is_eq(room, 0), u => spawn_marks(U32.to_nat(U32.add(1, U32.div(U32.sub(seats, 1), 3))), 0, w, 0, 83, C.fk_snuffer(), False{}, Gr.hits(Gr.find(Gr.rows_of(Gr.plan(0)), 83))),
  u => Z.lz(U32.is_eq(room, 2), u => spawn_marks(U32.to_nat(Bool.pick(U32, U32.is_ge(seats, 4), 2, 1)), 0, w, 2, 84, C.fk_turret(), False{}, Gr.hits(Gr.find(Gr.rows_of(Gr.plan(2)), 84))),
  u => Z.lz(U32.is_eq(room, 3), u => spawn_marks(U32.to_nat(U32.add(2, U32.div(seats, 2))), 0, w, 3, 83, C.fk_snuffer(), True{}, Gr.hits(Gr.find(Gr.rows_of(Gr.plan(3)), 83))),
  u => Z.lw(U32.is_eq(room, 4), u => W.set_props(warden_in(w, Gr.nth_hit(Gr.find(Gr.warden(), 87), 0), seats), lamps_burn(W.props(w))), w))))

# The party goes into room `room`: its things stand, its foes wait, and every
# seat is at its start (the fallen on their feet again, at the next brazier).
def enter_room(+w: S, +room: U32) -> S:
  +w1 = W.set_boss_on(W.set_next_foe(W.set_wipe(W.set_exiting(W.set_stage_t(W.set_stage(W.set_room(w, room), 0), 0), 0), 0), 0), 0)
  +w2 = W.set_grid(W.set_props(w1, build(room)), Gd.of(room))
  +w3 = W.set_fuses(W.set_drops(W.set_bolts(W.set_foes(w2, Nil{}), Nil{}), Nil{}), Nil{})
  provision_room(W.set_seats(w3, at_start(W.seats(w3), room)), room)

# The Warden back at the start of the phase it was in, at the health the table
# gave it (a leaver does not make it weaker).
def reset_boss_phase(+w: S) -> S:
  +mark = Gr.nth_hit(Gr.find(Gr.warden(), 87), 0)
  +hp = U32.max(U32.min(B.start(w), B.full(w)), 1)
  Bool.pick(S, U32.is_ne(W.boss_on(w), 0) && U32.is_ne(B.hp(w), 0),
    B.set_n(B.set_struck(B.set_l5(B.set_l4(B.set_l3(B.set_l2(B.set_l1(B.set_l0(B.set_dk(B.set_y(B.set_x(B.set_t(B.set_st(B.set_hp(w, hp), C.ws_rest()), 60), G.middle(U32.shrn(mark, 16n))), G.middle(U32.and(mark, 65535))), 0), 0), 0), 0), 0), 0), 0), 0), 0),
    w)

# ----- the fallen and the retry --------------------------------------------------------------------------

# Back to the room's start: hearts full, foes as the encounter opened, every
# solved thing still solved.
def retry_seats(+ks: List<&2, R.T8>, +room: U32, +flame: U32, +lantern: U32) -> List<&2, R.T8>:
  match ks:
    case Nil{}:
      Nil{}
    case Con{k, t}:
      +x = Gr.start_x(room, Seat.id(k))
      +y = Gr.start_y(room, Seat.id(k))
      +k1 = Seat.set_safey(Seat.set_safex(Seat.set_camy(Seat.set_camx(Seat.set_y(Seat.set_x(k, x), y), x), y), x), y)
      +k2 = Seat.set_t(Seat.set_act(Seat.set_hp(k1, Seat.max_hp(k1)), C.a_free()), 0)
      +k3 = Seat.set_lock_i(Seat.set_lock_k(Seat.set_z(k2, 0), 0), 0)
      +k4 = Seat.set_chain(Seat.set_shield(k3, 0), 0)
      +k5 = Seat.set_rev(Seat.set_charge(Seat.set_step(k4, 0), 0), 0)
      +k6 = Seat.set_pusht(Seat.set_pushy(Seat.set_pushx(k5, 0), 0), 0)
      Con{Seat.set_flame(Seat.set_carry(Seat.set_inv(k6, C.spawn_inv()), 0), Bool.pick(U32, U32.is_ne(lantern, 0), flame, 0)), retry_seats(t, room, flame, lantern)}

# A carried key is put down at its carrier's start.
def key_follows(+w: S) -> S:
  +k = H.seat_get(W.seats(w), W.hold_a(w))
  Bool.pick(S, U32.is_eq(W.hold_k(w), C.hold_carried()), W.set_hold_y(W.set_hold_x(w, Seat.x(k)), Seat.y(k)), w)

def retry(+w: S) -> S:
  +room = W.room(w)
  +w1 = W.set_tries(W.set_wipe(w, 0), U32.min(U32.add(W.tries(w), 1), 4294967295))
  +w2 = W.set_fuses(W.set_bolts(W.set_foes(w1, Nil{}), Nil{}), Nil{})
  +w3 = key_follows(W.set_seats(w2, retry_seats(W.seats(w2), room, W.r_flame(w2), W.p_lantern(w2))))
  +stage = W.stage(w3)
  +w4 = Z.lz((U32.is_eq(room, 0) && U32.is_eq(stage, 0)) || (U32.is_eq(room, 3) && U32.is_eq(stage, 0)) || U32.is_eq(room, 2), u => provision_room(w3, room),
    u => Z.lz(U32.is_eq(room, 1) && U32.is_eq(stage, 2), u => spawn_wave(w3),
      u => Z.lw(U32.is_eq(room, 4), u => reset_boss_phase(w3), w3)))
  W.set_flash(Q.sound(w4, 26), U32.max(W.flash(w4), 8))

# ----- the things in the room, a tick ---------------------------------------------------------------------

# Where weights lie (knights on the ground, blocks) and where flares burn, as
# (x, y) pairs gathered before the props move.
def weights_seats(+ks: List<&2, R.T8>, +acc: List<&2, R.W8>) -> List<&2, R.W8>:
  match ks:
    case Nil{}:
      acc
    case Con{k, t}:
      weights_seats(t, Bool.pick(List<&2, R.W8>, Q.up(k) && U32.is_eq(Seat.z(k), 0), L.push8(acc, R.W8{Seat.x(k), Seat.y(k), 0, 0, 0, 0, 0, 0}), acc))

def weights_blocks(+ps: List<&2, R.W8>, +acc: List<&2, R.W8>) -> List<&2, R.W8>:
  match ps:
    case Nil{}:
      acc
    case Con{p, t}:
      weights_blocks(t, Bool.pick(List<&2, R.W8>, U32.is_eq(Pr.kind(p), C.pk_block()), L.push8(acc, R.W8{Pr.x(p), Pr.y(p), 0, 0, 0, 0, 0, 0}), acc))

def flares_of(+ks: List<&2, R.T8>, +acc: List<&2, R.W8>) -> List<&2, R.W8>:
  match ks:
    case Nil{}:
      acc
    case Con{k, t}:
      flares_of(t, Bool.pick(List<&2, R.W8>, Q.flaring(k), L.push8(acc, R.W8{Seat.x(k), Seat.y(k), 0, 0, 0, 0, 0, 0}), acc))

# Whether any pair is within `r` of the point.
def near_any(+ps: List<&2, R.W8>, +x: U32, +y: U32, +r: U32, +hit: Bool) -> Bool:
  match ps:
    case Nil{}:
      hit
    case Con{p, t}:
      match hit:
        case True{}:
          True{}
        case False{}:
          near_any(t, x, y, r, N.slt(G.dist(U32.sub(Pt.x(p), x), U32.sub(Pt.y(p), y)), r))

# A brazier that was alight goes out when its flame runs low.
def brazier_burns(+w: S, +i: U32, +p: R.W8) -> S:
  +t = U32.sub(Pr.t(p), 1)
  +w1 = W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_t(p, t)))
  Bool.pick(S, U32.is_eq(t, 0) && U32.is_eq(Pr.on(p), 0), Q.sound(Q.fx(w1, C.fxk_puff(), Pr.x(p), Pr.y(p), 0), 38), w1)

# A plate is pressed by a knight on the ground or a block on it.
def plate_tick(+w: S, +i: U32, +p: R.W8, +weights: List<&2, R.W8>) -> S:
  +on = Bool.to_u32(near_any(weights, Pr.x(p), Pr.y(p), U32.mul(C.plate_r(), 256), False{}))
  +w1 = W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_on(p, on)))
  Bool.pick(S, U32.is_ne(on, Pr.on(p)), Q.sound(w1, Bool.pick(U32, U32.is_eq(on, 1), 42, 40)), w1)

def block_lands(+w: S, +i: U32, +p: R.W8) -> S:
  +x = G.middle(G.tile_of(Pr.x(p)))
  +y = G.middle(G.tile_of(Pr.y(p)))
  Q.sound(W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_on(Pr.set_ty(Pr.set_tx(Pr.set_y(Pr.set_x(p, x), y), G.tile_of(x)), G.tile_of(y)), 0))), 28)

def block_slide(+w: S, +i: U32, +p: R.W8) -> S:
  +on = Pr.on(p)
  +dx = Bool.pick(U32, U32.is_eq(on, 1), 1, Bool.pick(U32, U32.is_eq(on, 2), N.sneg(1), 0))
  +dy = Bool.pick(U32, U32.is_eq(on, 3), 1, Bool.pick(U32, U32.is_eq(on, 4), N.sneg(1), 0))
  +p1 = Pr.set_t(Pr.set_y(Pr.set_x(p, U32.add(Pr.x(p), U32.mul(dx, 256))), U32.add(Pr.y(p), U32.mul(dy, 256))), M.dec(Pr.t(p)))
  Bool.pick(S, U32.is_eq(Pr.t(p1), 0), block_lands(w, i, p1), W.set_props(w, L.set8(W.props(w), U32.to_nat(i), p1)))

def block_tick(+w: S, +i: U32, +p: R.W8) -> S:
  Bool.pick(S, U32.is_ne(Pr.on(p), 0), block_slide(w, i, p), W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_t(p, M.dec(Pr.t(p))))))

# A sigil under a flare fills; found, it opens what it hid. `ta` marks it found
# and `tb`, `tc` are where.
def sigil_tick(+w: S, +i: U32, +p: R.W8, +flares: List<&2, R.W8>) -> S:
  +lit = near_any(flares, Pr.x(p), Pr.y(p), U32.mul(C.flare_r(), 256), False{})
  +t = Bool.pick(U32, lit, U32.add(Pr.t(p), 1), M.dec(Pr.t(p)))
  +found = lit && U32.is_ge(t, C.sigil_ticks())
  +p1 = Bool.pick(R.W8, found, Pr.set_on(Pr.set_t(p, t), 1), Pr.set_t(p, t))
  +w1 = W.set_props(w, L.set8(W.props(w), U32.to_nat(i), p1))
  Bool.pick(S, found, W.set_tc(W.set_tb(W.set_ta(w1, 1), Pr.x(p)), Pr.y(p)), w1)

def prop_dispatch(+ds: Nat, +w: S, +i: U32, +p: R.W8, +weights: List<&2, R.W8>, +flares: List<&2, R.W8>) -> S:
  match ds:
    case 0n:
      W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_t(p, M.dec(Pr.t(p)))))
    case 1n+a1:
      match a1:
        case 0n:
          W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_t(p, M.dec(Pr.t(p)))))
        case 1n+a2:
          match a2:
            case 0n:
              W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_t(p, M.dec(Pr.t(p)))))
            case 1n+a3:
              match a3:
                case 0n:
                  brazier_burns(w, i, p)
                case 1n+a4:
                  match a4:
                    case 0n:
                      W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_t(p, M.dec(Pr.t(p)))))
                    case 1n+a5:
                      match a5:
                        case 0n:
                          W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_t(p, M.dec(Pr.t(p)))))
                        case 1n+a6:
                          match a6:
                            case 0n:
                              W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_t(p, M.dec(Pr.t(p)))))
                            case 1n+a7:
                              match a7:
                                case 0n:
                                  W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_t(p, M.dec(Pr.t(p)))))
                                case 1n+a8:
                                  match a8:
                                    case 0n:
                                      W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_t(p, M.dec(Pr.t(p)))))
                                    case 1n+a9:
                                      match a9:
                                        case 0n:
                                          plate_tick(w, i, p, weights)
                                        case 1n+a10:
                                          match a10:
                                            case 0n:
                                              block_tick(w, i, p)
                                            case 1n+a11:
                                              match a11:
                                                case 0n:
                                                  W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_t(p, M.dec(Pr.t(p)))))
                                                case 1n+a12:
                                                  match a12:
                                                    case 0n:
                                                      W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_t(p, M.dec(Pr.t(p)))))
                                                    case 1n+a13:
                                                      match a13:
                                                        case 0n:
                                                          sigil_tick(w, i, p, flares)
                                                        case 1n+a14:
                                                          W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_t(p, M.dec(Pr.t(p)))))

# Most things in a room have nothing to do on a tick.
def prop_tick_if(has_work: Bool, +w: S, +i: U32, +p: R.W8, +weights: List<&2, R.W8>, +flares: List<&2, R.W8>) -> S:
  match has_work:
    case False{}:
      w
    case True{}:
      prop_dispatch(U32.to_nat(Pr.kind(p)), w, i, p, weights, flares)

def prop_tick(+w: S, +i: U32, +weights: List<&2, R.W8>, +flares: List<&2, R.W8>) -> S:
  +p = L.get8(W.props(w), U32.to_nat(i))
  +k = Pr.kind(p)
  prop_tick_if((U32.is_eq(k, C.pk_brazier()) && U32.is_ne(Pr.t(p), 0)) || U32.is_eq(k, C.pk_plate()) || U32.is_eq(k, C.pk_block()) || (U32.is_eq(k, C.pk_sigil()) && U32.is_eq(Pr.on(p), 0)) || U32.is_eq(k, C.pk_fissure()), w, i, p, weights, flares)

def props_go(+n: Nat, +w: S, +i: U32, +weights: List<&2, R.W8>, +flares: List<&2, R.W8>) -> S:
  match n:
    case 0n:
      w
    case 1n+p:
      props_go(p, prop_tick(w, i, weights, flares), U32.add(i, 1), weights, flares)

# A sigil found opens what it was hiding.
def sigil_found(+w: S) -> S:
  Bool.pick(S, U32.is_ne(W.ta(w), 0), Q.sound(Q.solved(Q.set_gate(w, 2, 1), W.tb(w), W.tc(w)), 13), w)

# One tick of the things in the room.
def tick_props(+w: S) -> S:
  +weights = weights_blocks(W.props(w), weights_seats(W.seats(w), Nil{}))
  +flares = flares_of(W.seats(w), Nil{})
  sigil_found(props_go(U32.to_nat(L.len8(W.props(w))), W.set_ta(w, 0), 0, weights, flares))

# A gate that closed or a block that slid over a body puts it out on the nearest
# open ground: nobody is walled in.
def unstick_to(+stuck: Bool, +w: S, +k: R.T8) -> R.T8:
  match stuck:
    case False{}:
      k
    case True{}:
      +n = Fo.nudge(w, Seat.x(k), Seat.y(k), 1280)
      Seat.set_y(Seat.set_x(k, Nr.x(n)), Nr.y(n))

def unstick_list(+ks: List<&2, R.T8>, +w: S) -> List<&2, R.T8>:
  match ks:
    case Nil{}:
      Nil{}
    case Con{k, t}:
      Con{unstick_to(Bool.not(Q.clear(w, Seat.x(k), Seat.y(k), 640)), w, k), unstick_list(t, w)}

def unstick(+w: S) -> S: W.set_seats(w, unstick_list(W.seats(w), w))

# ----- what each room asks -----------------------------------------------------------------------------------

def prop_of(+ps: List<&2, R.W8>, +kind: U32, +id: U32, +j: U32, +found: U32, +done: Bool) -> U32:
  match ps:
    case Nil{}:
      found
    case Con{p, t}:
      match done:
        case True{}:
          found
        case False{}:
          +hit = U32.is_eq(Pr.kind(p), kind) && U32.is_eq(Pr.id(p), id)
          prop_of(t, kind, id, U32.add(j, 1), Bool.pick(U32, hit, j, found), hit)

# The thing's index, or none.
def find_prop(+w: S, +kind: U32, +id: U32) -> U32: prop_of(W.props(w), kind, id, 0, C.none(), False{})

# Whether that thing is there and has its `on` at 1.
def prop_on(+w: S, +kind: U32, +id: U32) -> Bool:
  +j = find_prop(w, kind, id)
  U32.is_ne(j, C.none()) && U32.is_eq(Pr.on(L.get8(W.props(w), U32.to_nat(j))), 1)

# Every thing of a kind is lit (and there is one).
def all_lit_go(+ps: List<&2, R.W8>, +kind: U32, +any: Bool, +all: Bool) -> Bool:
  match ps:
    case Nil{}:
      any && all
    case Con{p, t}:
      +mine = U32.is_eq(Pr.kind(p), kind)
      all_lit_go(t, kind, any || mine, all && (Bool.not(mine) || Q.lit(p)))

def foes_down(+fs: List<&2, R.T8>) -> Bool:
  match fs:
    case Nil{}:
      True{}
    case Con{f, t}:
      Bool.not(Q.foe_alive(f)) && foes_down(t)

# Whether the way on is open.
def exit_open(+w: S) -> Bool:
  +room = W.room(w)
  +stage = W.stage(w)
  (U32.is_eq(room, 0) && U32.is_ge(stage, 1)) || (U32.is_eq(room, 1) && U32.is_ge(stage, 4)) || (U32.is_eq(room, 2) && U32.is_ge(stage, 1)) || (U32.is_eq(room, 3) && U32.is_ge(stage, 4))

# The exit gate swings open.
def open_exit(+w: S) -> S:
  +w1 = Q.set_gate(w, 0, 1)
  +g = L.get8(W.props(w1), U32.to_nat(find_prop(w1, C.pk_gate(), 0)))
  Bool.pick(S, U32.is_ne(W.ret(w1), 0), Q.sound(Q.solved(w1, Pr.x(g), Pr.y(g)), 13), w1)

def exit_if(+go: Bool, +w: S) -> S:
  match go:
    case True{}:
      open_exit(W.set_stage(w, 1))
    case False{}:
      w

# The lamp hall: three braziers lit at once from one ember, the Lantern in the
# chest, the Gloam's wave, one light left to throw at the great brazier.
def chest_opens(+w: S) -> S:
  +g = Q.set_gate(W.set_stage(w, 1), 1, 1)
  +c = L.get8(W.props(w), U32.to_nat(find_prop(w, C.pk_chest(), 0)))
  Q.sound(Q.solved(g, Pr.x(c), Pr.y(c)), 13)

def lamp_hall(+ds: Nat, +w: S) -> S:
  match ds:
    case 0n:
      Z.lw(all_lit_go(W.props(w), C.pk_brazier(), False{}, True{}), u => chest_opens(w), w)
    case 1n+a1:
      match a1:
        case 0n:
          Z.lw(prop_on(w, C.pk_chest(), 0), u => Q.sound(spawn_wave(W.set_stage_t(W.set_stage(w, 2), 0)), 43), w)
        case 1n+a2:
          match a2:
            case 0n:
              Z.lw(U32.is_gt(W.stage_t(w), 20) && foes_down(W.foes(w)), u => Q.sound(W.set_stage(w, 3), 26), w)
            case 1n+a3:
              match a3:
                case 0n:
                  Z.lw(prop_on(w, C.pk_great(), 0), u => open_exit(W.set_stage(w, 4)), w)
                case 1n+a4:
                  w

# Plates and a crystal hold the gallery's gate: two plates for two or more
# knights (the block or a knight on the first, another on the second), one plate
# alone for one; the crystal once woken holds it for good.
def plate_held(+ps: List<&2, R.W8>, +id: U32) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{p, t}:
      (U32.is_eq(Pr.kind(p), C.pk_plate()) && U32.is_eq(Pr.id(p), id) && U32.is_eq(Pr.on(p), 1)) || plate_held(t, id)

def any_on(+ps: List<&2, R.W8>, +kind: U32, +id: U32, +use_id: Bool) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{p, t}:
      (U32.is_eq(Pr.kind(p), kind) && (Bool.not(use_id) || U32.is_eq(Pr.id(p), id)) && U32.is_eq(Pr.on(p), 1)) || any_on(t, kind, id, use_id)

def gallery_gate(+w: S, +want: Bool, +was: Bool) -> S:
  +g = L.get8(W.props(w), U32.to_nat(find_prop(w, C.pk_gate(), 1)))
  +w1 = Q.sound(w, Bool.pick(U32, want, 13, 15))
  Bool.pick(S, want, Q.fx(w1, C.fxk_dust(), Pr.x(g), Pr.y(g), 0), w1)

def gallery(+w: S) -> S:
  +plate = plate_held(W.props(w), 0) && (plate_held(W.props(w), 1) || U32.is_lt(L.len64(W.seats(w)), 2))
  +crystal = any_on(W.props(w), C.pk_crystal(), 0, False{})
  +was = any_on(W.props(w), C.pk_gate(), 1, True{})
  +want = plate || crystal
  +w1 = Q.set_gate(w, 1, Bool.to_u32(want))
  +w2 = Bool.pick(S, U32.is_ne(W.ret(w1), 0) && Bool.not(U32.is_eq(Bool.to_u32(want), Bool.to_u32(was))), gallery_gate(w1, want, was), w1)
  exit_if(U32.is_eq(W.stage(w2), 0) && crystal, w2)

# The index of the first prop of a kind, or none.
def find_kind(+ps: List<&2, R.W8>, +kind: U32, +j: U32, +found: U32, +done: Bool) -> U32:
  match ps:
    case Nil{}:
      found
    case Con{p, t}:
      match done:
        case True{}:
          found
        case False{}:
          +hit = U32.is_eq(Pr.kind(p), kind)
          find_kind(t, kind, U32.add(j, 1), Bool.pick(U32, hit, j, found), hit)

# The vault: the dark beaten lights the braziers and wakes the key chest, the
# Moon Key opens the locked door, the Boss Key chest and then its door.
def braziers_lit(+ps: List<&2, R.W8>) -> List<&2, R.W8>:
  match ps:
    case Nil{}:
      Nil{}
    case Con{p, t}:
      Con{Bool.pick(R.W8, U32.is_eq(Pr.kind(p), C.pk_brazier()), Pr.set_on(p, 1), p), braziers_lit(t)}

def key_chest_wakes(+w: S) -> S:
  +j = find_prop(w, C.pk_chest(), 1)
  +c = L.get8(W.props(w), U32.to_nat(j))
  Bool.pick(S, U32.is_ne(j, C.none()), Q.solved(Q.fx(W.set_props(w, L.set8(W.props(w), U32.to_nat(j), Pr.set_on(c, 0))), C.fxk_light(), Pr.x(c), Pr.y(c), 0), Pr.x(c), Pr.y(c)), w)

def vault_clear(+w: S) -> S:
  +w1 = key_chest_wakes(W.set_stage(w, 1))
  W.set_props(w1, braziers_lit(W.props(w1)))

def vault_rows(+ds: Nat, +w: S) -> S:
  match ds:
    case 0n:
      Z.lw(foes_down(W.foes(w)), u => vault_clear(w), w)
    case 1n+a1:
      match a1:
        case 0n:
          Z.lw(prop_on(w, C.pk_chest(), 1), u => W.set_stage(w, 2), w)
        case 1n+a2:
          match a2:
            case 0n:
              Z.lw(any_on(W.props(w), C.pk_lock(), 0, False{}), u => W.set_stage(w, 3), w)
            case 1n+a3:
              match a3:
                case 0n:
                  Z.lw(prop_on(w, C.pk_chest(), 0), u => W.set_stage(w, 4), w)
                case 1n+a4:
                  w

def vault(+w: S) -> S:
  +w1 = vault_rows(U32.to_nat(W.stage(w)), w)
  Bool.pick(S, any_on(W.props(w1), C.pk_boss(), 0, False{}) && U32.is_ge(W.stage(w1), 4), W.set_stage(w1, 5), w1)

# The last room: the heart container, then the Beacon, relit by knights holding E
# before it.
def holders(+ks: List<&2, R.T8>) -> U32:
  match ks:
    case Nil{}:
      0
    case Con{k, t}:
      U32.add(Bool.to_u32(Q.up(k) && U32.is_eq(Seat.act(k), C.a_free()) && U32.is_ne(U32.and(Seat.held(k), C.b_ctx()), 0) && U32.is_eq(Seat.ctx_k(k), C.cx_relight())), holders(t))

def beacon_lights(+w: S, +i: U32, +p: R.W8) -> S:
  +w1 = W.set_props(w, L.set8(W.props(w), U32.to_nat(i), Pr.set_on(p, 1)))
  +w2 = W.set_won(W.set_shake(W.set_flash(W.set_done_t(W.set_done_on(W.set_stage(w1, 4), 1), W.elapsed(w1)), 12), C.shake_heavy()), 1)
  Q.sound(Q.sound(Q.fx(w2, C.fxk_solved(), Pr.x(p), Pr.y(p), 1), 9), 45)

def beacon_tick(+w: S, +i: U32, +p: R.W8) -> S:
  +n = holders(W.seats(w))
  +t = Bool.pick(U32, U32.is_ne(n, 0), U32.min(U32.add(Pr.t(p), n), 65535), M.dec(Pr.t(p)))
  +p1 = Pr.set_t(p, t)
  Bool.pick(S, U32.is_ge(t, C.beacon_ticks()) && U32.is_eq(Pr.on(p), 0), beacon_lights(w, i, p1), W.set_props(w, L.set8(W.props(w), U32.to_nat(i), p1)))

def warden_room(+w: S) -> S:
  +i = find_kind(W.props(w), C.pk_beacon(), 0, C.none(), False{})
  Z.lw(U32.is_eq(W.stage(w), 3) && U32.is_ne(i, C.none()), u => beacon_tick(w, i, L.get8(W.props(w), U32.to_nat(i))), w)

# ----- the way on ---------------------------------------------------------------------------------------------------

def on_exit(+ks: List<&2, R.T8>, +w: S) -> Bool:
  match ks:
    case Nil{}:
      False{}
    case Con{k, t}:
      (Q.up(k) && U32.is_eq(Seat.z(k), 0) && U32.is_eq(Q.cls(w, G.tile_of(Seat.x(k)), G.tile_of(Seat.y(k))), 4)) || on_exit(t, w)

def room_rows(+ds: Nat, +w: S) -> S:
  match ds:
    case 0n:
      exit_if(U32.is_eq(W.stage(w), 0) && foes_down(W.foes(w)), w)
    case 1n+a1:
      match a1:
        case 0n:
          lamp_hall(U32.to_nat(W.stage(w)), w)
        case 1n+a2:
          match a2:
            case 0n:
              gallery(w)
            case 1n+a3:
              match a3:
                case 0n:
                  vault(w)
                case 1n+a4:
                  warden_room(w)

def walk_on(+w: S) -> S:
  +e = U32.sub(W.exiting(w), 1)
  +w1 = W.set_exiting(w, e)
  Z.lw(U32.is_eq(e, 0), u => Q.sound(enter_room(w1, U32.add(W.room(w1), 1)), 12), w1)

def leave_on(+w: S) -> S:
  Bool.pick(S, on_exit(W.seats(w), w), Q.sound(W.set_exiting(w, C.exit_ticks()), 14), w)

# What each room asks, and what it gives; then the way on, stood in, a moment, and
# the next room.
def tick_stage(+w: S) -> S:
  +w1 = room_rows(U32.to_nat(W.room(w)), W.set_stage_t(w, U32.min(U32.add(W.stage_t(w), 1), C.limit())))
  Z.lz(U32.is_eq(W.exiting(w1), 0) && exit_open(w1) && U32.is_lt(W.room(w1), 4), u => leave_on(w1), u => Z.lw(U32.is_ne(W.exiting(w1), 0), u => walk_on(w1), w1))

# ----- what lies on the floor ------------------------------------------------------------------------------------------

# The first knight (by seat) up and within reach of a point: their id, or none.
def taker_go(+ks: List<&2, R.T8>, +x: U32, +y: U32, +found: U32, +done: Bool) -> U32:
  match ks:
    case Nil{}:
      found
    case Con{k, t}:
      match done:
        case True{}:
          found
        case False{}:
          +hit = Q.up(k) && N.slt(G.dist(U32.sub(Seat.x(k), x), U32.sub(Seat.y(k), y)), U32.mul(C.pick_r(), 256))
          taker_go(t, x, y, Bool.pick(U32, hit, Seat.id(k), found), hit)

# The Boss Key follows its carrier; a carrier who falls or leaves puts it down
# where they stood, and a knight who comes by takes it up.
def key_carry(+w: S) -> S:
  +k = H.seat_get(W.seats(w), W.hold_a(w))
  Bool.pick(S, U32.is_eq(W.hold_k(w), C.hold_carried()),
    Bool.pick(S, Q.up(k), W.set_hold_y(W.set_hold_x(w, Seat.x(k)), Seat.y(k)), W.set_hold_k(w, C.hold_lying())),
    w)

def key_pick(+w: S) -> S:
  +id = taker_go(W.seats(w), W.hold_x(w), W.hold_y(w), C.none(), False{})
  Bool.pick(S, U32.is_eq(W.hold_k(w), C.hold_lying()) && U32.is_ne(id, C.none()), Q.sound(W.set_hold_a(W.set_hold_k(w, C.hold_carried()), id), 7), w)

def drop_heart(+w: S, +id: U32) -> S:
  +w1 = H.take(w, id)
  +hurt = U32.is_lt(K.hp(w1), K.max_hp(w1))
  W.set_ret(H.put(Bool.pick(S, hurt, Q.sound(K.set_hp(w1, U32.min(U32.add(K.hp(w1), C.heart()), K.max_hp(w1))), 27), w1)), Bool.to_u32(hurt))

def drop_gem(+w: S, +id: U32) -> S:
  +w1 = H.take(w, id)
  W.set_ret(H.put(Q.sound(K.set_gems(w1, U32.add(K.gems(w1), 1)), 16)), 1)

def drop_bomb(+w: S, +id: U32) -> S:
  +w1 = H.take(w, id)
  W.set_ret(H.put(Q.sound(K.set_bombs(w1, U32.min(U32.add(K.bombs(w1), 1), C.bomb_cap())), 37)), 1)

def drop_container(+w: S, +id: U32) -> S:
  +w1 = H.take(w, id)
  +w2 = H.put(X.grant(w1, id, C.it_container(), 0))
  W.set_ret(W.set_stage(w2, U32.max(W.stage(w2), 3)), 1)

def drop_taken(+w: S, +id: U32, +kind: U32) -> S:
  Z.lz(U32.is_eq(kind, C.dk_heart()), u => drop_heart(w, id),
  u => Z.lz(U32.is_eq(kind, C.dk_gem()), u => drop_gem(w, id),
  u => Z.lz(U32.is_eq(kind, C.dk_bomb()), u => drop_bomb(w, id), u => drop_container(w, id))))

# One drop: it ages, a knight in reach takes it (a heart only if they need it), and
# it fades after a while unless it is the container. `ret` is 1 when it is gone.
def drop_one(+w: S, +i: U32) -> S:
  +d = L.get8(W.drops(w), U32.to_nat(i))
  +age = U32.min(U32.add(Dr.age(d), 1), 65535)
  +w1 = W.set_drops(w, L.set8(W.drops(w), U32.to_nat(i), Dr.set_age(d, age)))
  +id = taker_go(W.seats(w1), Dr.x(d), Dr.y(d), C.none(), False{})
  +w2 = Z.lz(U32.is_ne(id, C.none()), u => drop_taken(w1, id, Dr.kind(d)), u => W.set_ret(w1, 0))
  W.set_ret(w2, Bool.to_u32(U32.is_ne(W.ret(w2), 0) || (U32.is_gt(age, C.drop_life()) && U32.is_ne(Dr.kind(d), C.dk_container()))))

def drops_go(+n: Nat, +w: S, +i: U32) -> S:
  match n:
    case 0n:
      w
    case 1n+p:
      +live = U32.is_lt(i, L.len8(W.drops(w)))
      +w1 = Z.lw(live, u => drop_one(w, i), w)
      +gone = live && U32.is_ne(W.ret(w1), 0)
      drops_go(p, Bool.pick(S, gone, W.set_drops(w1, L.swap_remove8(W.drops(w1), U32.to_nat(i))), w1), Bool.pick(U32, gone, i, U32.add(i, 1)))

# Drops: hearts mend, gems and bombs are kept, the Boss Key is carried.
def pickups(+w: S) -> S:
  +w1 = key_pick(key_carry(w))
  drops_go(U32.to_nat(U32.add(U32.mul(L.len8(W.drops(w1)), 2), 1)), w1, 0)

# ----- the fallen ----------------------------------------------------------------------------------------------------------

# How many friends stand up beside a fallen knight holding E.
def helpers_of(+ks: List<&2, R.T8>, +d: U32, +x: U32, +y: U32) -> U32:
  match ks:
    case Nil{}:
      0
    case Con{k, t}:
      U32.add(Bool.to_u32(U32.is_ne(Seat.id(k), d) && Q.up(k) && U32.is_ne(U32.and(Seat.held(k), C.b_ctx()), 0) && U32.is_eq(Seat.act(k), C.a_free()) && N.slt(G.dist(U32.sub(Seat.x(k), x), U32.sub(Seat.y(k), y)), U32.mul(C.revive_reach(), 256))), helpers_of(t, d, x, y))

# A friend holding E gets them up where they lie; a long wait brings them to the
# next brazier on their own.
def stand_up(+w: S, +d: U32, +auto: Bool) -> S:
  +hp = U32.max(U32.min(Bool.pick(U32, auto, U32.div(C.revive_hp(), 2), C.revive_hp()), K.max_hp(w)), 1)
  +w1 = K.set_inv(K.set_rev(K.set_t(K.set_act(K.set_hp(w, hp), C.a_free()), 0), 0), C.spawn_inv())
  +sx = Gr.start_x(W.room(w1), d)
  +sy = Gr.start_y(W.room(w1), d)
  +w2 = Bool.pick(S, auto, K.set_safey(K.set_safex(K.set_y(K.set_x(w1, sx), sy), sx), sy), w1)
  Q.sound(Q.fx(w2, C.fxk_light(), K.x(w2), K.y(w2), 0), 27)

def revive_one(+w: S, +d: U32) -> S:
  +k = H.seat_get(W.seats(w), d)
  +helpers = helpers_of(W.seats(w), d, Seat.x(k), Seat.y(k))
  +w1 = H.take(w, d)
  +rev = Bool.pick(U32, U32.is_ne(helpers, 0), U32.add(K.rev(w1), U32.min(helpers, 2)), M.sat(K.rev(w1), 2))
  +auto = U32.is_ge(K.t(w1), C.down_auto())
  +w2 = K.set_rev(w1, rev)
  H.put(Z.lw(U32.is_ge(rev, C.revive_hold()) || auto, u => stand_up(w2, d, auto), w2))

def downed_ids(+ks: List<&2, R.T8>) -> List<&2, U32>:
  match ks:
    case Nil{}:
      Nil{}
    case Con{k, t}:
      Bool.pick(List<&2, U32>, U32.is_eq(Seat.act(k), C.a_down()), Con{Seat.id(k), downed_ids(t)}, downed_ids(t))

def revive_go(+ids: List<&2, U32>, +w: S) -> S:
  match ids:
    case Nil{}:
      w
    case Con{d, t}:
      revive_go(t, revive_one(w, d))

def revive_pass(+w: S) -> S: revive_go(downed_ids(W.seats(w)), w)

def all_down(+ks: List<&2, R.T8>) -> Bool:
  match ks:
    case Nil{}:
      True{}
    case Con{k, t}:
      Bool.not(Q.up(k)) && all_down(t)

# Every knight down: a breath, then the party is back at the room's start with the
# world as it was.
def wipe_tick(+w: S) -> S:
  +n = U32.add(W.wipe(w), 1)
  +w1 = W.set_wipe(w, n)
  +w2 = Bool.pick(S, U32.is_eq(n, 1), Q.sound(w1, 4), w1)
  Z.lw(U32.is_ge(n, C.wipe_ticks()), u => retry(w2), w2)

def tick_wipe(+w: S) -> S:
  Z.lz(U32.is_ne(L.len64(W.seats(w)), 0) && all_down(W.seats(w)), u => wipe_tick(w), u => W.set_wipe(w, 0))

# ----- the tick's leftovers ------------------------------------------------------------------------------------------------

def fx_life(+kind: U32) -> U32:
  Bool.pick(U32, U32.is_eq(kind, C.fxk_puff()) || U32.is_eq(kind, C.fxk_light()), 16, Bool.pick(U32, U32.is_eq(kind, C.fxk_spark()) || U32.is_eq(kind, C.fxk_tick()), 8, Bool.pick(U32, U32.is_eq(kind, C.fxk_ring()) || U32.is_eq(kind, C.fxk_dust()), 10, Bool.pick(U32, U32.is_eq(kind, C.fxk_parry()), 12, Bool.pick(U32, U32.is_eq(kind, C.fxk_solved()), 24, Bool.pick(U32, U32.is_eq(kind, C.fxk_blast()), 14, 20))))))

def fx_age(+fs: List<&2, R.W8>) -> List<&2, R.W8>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{f, t}:
      Con{Fx.set_t(f, U32.min(U32.add(Fx.t(f), 1), 255)), fx_age(t)}

# Puffs and sparks age out; the shake and the flash die down; the cue and the
# banner run out.
def age(+w: S) -> S:
  +w1 = W.set_fx(w, L.keep8(~(f => U32.is_lt(Fx.t(f), fx_life(Fx.kind(f)))), fx_age(W.fx(w))))
  +w2 = W.set_flash(W.set_shake(w1, M.dec(W.shake(w1))), M.dec(W.flash(w1)))
  +w3 = W.set_cue_t(w2, M.dec(W.cue_t(w2)))
  +w4 = Bool.pick(S, U32.is_gt(W.cue_t(w2), 1), w3, W.set_cue_on(w3, 0))
  +w5 = Bool.pick(S, U32.is_ne(W.ban_on(w4), 0), W.set_ban_t(w4, M.dec(W.ban_t(w4))), w4)
  Bool.pick(S, U32.is_ne(W.ban_on(w5), 0) && U32.is_eq(W.ban_t(w5), 0), W.set_ban_on(w5, 0), w5)
