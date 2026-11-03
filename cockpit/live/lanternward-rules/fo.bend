# The Gloam's small things: the snuffer, a hooded wisp that lunges along a line it
# shows first, and the turret that looses an iron bolt along one. Every attack
# shows its footprint for its whole tell and strikes only there.
# crates/delve/src/rules/lanternward/foes.rs.
import Base
import ./r.bend as R
import ./c.bend as C
import ./n.bend as N
import ./m.bend as M
import ./l.bend as L
import ./g.bend as G
import ./sh.bend as D
import ./w.bend as W
import ./pr.bend as Pr
import ./bo.bend as Bo
import ./st.bend as St
import ./nr.bend as Nr
import ./se.bend as Seat
import ./fe.bend as Foe
import ./b.bend as B
import ./z.bend as Z
import ./q.bend as Q
import ./ht.bend as H

def S() -> Data: R.W

# ----- a foe appears ----------------------------------------------------------------

def foe_new(+kind: U32, +id: U32, +x: U32, +y: U32, +hooded: Bool, +seats: U32) -> R.T8:
  +per = Bool.pick(U32, U32.is_eq(kind, C.fk_snuffer()), C.snuffer_hp(), C.turret_hp())
  +full = U32.mul(per, U32.max(seats, 1))
  +f1 = Foe.set_y(Foe.set_x(Foe.set_kind(Foe.set_id(L.zero64(), id), kind), x), y)
  +f2 = Foe.set_prov(Foe.set_full(Foe.set_hp(f1, full), full), N.sclamp(seats, 1, C.max_players()))
  Foe.set_hooded(Foe.set_fy(f2, 256), Bool.to_u32(hooded))

def spawn_go(+go: Bool, +w: S, +kind: U32, +x: U32, +y: U32, +hooded: Bool) -> S:
  match go:
    case False{}:
      w
    case True{}:
      +id = W.next_foe(w)
      W.set_foes(W.set_next_foe(w, U32.and(U32.add(id, 1), 255)), L.push64(W.foes(w), foe_new(kind, id, x, y, hooded, L.len64(W.seats(w)))))

# A foe of `kind` where a snuffer or turret mark stands.
def spawn_foe(+w: S, +kind: U32, +x: U32, +y: U32, +hooded: Bool) -> S: spawn_go(U32.is_lt(L.len64(W.foes(w)), C.foe_cap()), w, kind, x, y, hooded)

# ----- a table that has grown --------------------------------------------------------

def prov_foes(+fs: List<&2, R.T8>, +now: U32) -> List<&2, R.T8>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{f, t}:
      +had = Foe.prov(f)
      +grow = U32.is_gt(now, had) && U32.is_ne(Foe.hp(f), 0)
      Con{Bool.pick(R.T8, grow, Foe.set_prov(Foe.set_full(Foe.set_hp(f, U32.div(U32.mul(Foe.hp(f), now), had)), U32.div(U32.mul(Foe.full(f), now), had)), now), f), prov_foes(t, now)}

def prov_boss(+w: S, +now: U32) -> S:
  +had = B.prov(w)
  +grow = U32.is_ne(W.boss_on(w), 0) && U32.is_gt(now, had) && U32.is_ne(B.hp(w), 0)
  Bool.pick(S, grow,
    B.set_prov(B.set_start(B.set_full(B.set_hp(w, U32.div(U32.mul(B.hp(w), now), had)), U32.div(U32.mul(B.full(w), now), had)), U32.div(U32.mul(B.start(w), now), had)), now),
    w)

# Scale foes already standing to a table that has grown, keeping the share of
# health they have left; a seat that comes back to a table it had already
# provisioned scales nothing twice.
def provision(+w: S) -> S:
  +now = N.sclamp(L.len64(W.seats(w)), 1, C.max_players())
  prov_boss(W.set_foes(w, prov_foes(W.foes(w), now)), now)

# ----- the snuffer ------------------------------------------------------------------

# One tick of a snuffer from the state it is in, toward the nearest knight (its
# place and how far it is). The blow it makes in its lunge is the caller's.
def snuffer_drift(+f: R.T8, +tx: U32, +ty: U32, +d: U32) -> R.T8:
  +u = G.unit(U32.sub(tx, Foe.x(f)), U32.sub(ty, Foe.y(f)))
  +ux = G.vx(u)
  +uy = G.vy(u)
  +want = U32.mul(C.ring(), 256)
  +far = N.sgt(d, U32.add(want, 1024))
  +near = N.slt(d, U32.sub(want, 1024))
  +even = U32.is_eq(U32.and(Foe.id(f), 1), 0)
  +mx = Bool.pick(U32, far, ux, Bool.pick(U32, near, N.sneg(ux), Bool.pick(U32, even, N.sneg(uy), uy)))
  +my = Bool.pick(U32, far, uy, Bool.pick(U32, near, N.sneg(uy), Bool.pick(U32, even, ux, N.sneg(ux))))
  +f1 = Foe.set_y(Foe.set_x(Foe.set_fy(Foe.set_fx(f, ux), uy), U32.add(Foe.x(f), N.sdiv(U32.mul(mx, C.snuffer_drift()), 256))), U32.add(Foe.y(f), N.sdiv(U32.mul(my, C.snuffer_drift()), 256)))
  +t = M.dec(Foe.t(f1))
  +strike = U32.is_eq(t, 0) && N.slt(d, U32.mul(C.strike_from(), 256))
  +l = G.unit(U32.sub(tx, Foe.x(f1)), U32.sub(ty, Foe.y(f1)))
  +f2 = Foe.set_t(f1, t)
  +locked = Foe.set_struck(Foe.set_t(Foe.set_st(Foe.set_d4(Foe.set_d3(Foe.set_d2(Foe.set_d1(Foe.set_d0(Foe.set_dk(Foe.set_fy(Foe.set_fx(f2, G.vx(l)), G.vy(l)), C.dg_capsule()), Foe.x(f2)), Foe.y(f2)), U32.add(Foe.x(f2), U32.mul(G.vx(l), C.lunge_len()))), U32.add(Foe.y(f2), U32.mul(G.vy(l), C.lunge_len()))), 1792), C.fs_tell()), C.snuffer_tell()), 0)
  Bool.pick(R.T8, strike, locked, Foe.set_t(f2, Bool.pick(U32, U32.is_eq(t, 0), 8, t)))

def snuffer_tell(+f: R.T8) -> R.T8:
  +t = M.dec(Foe.t(f))
  Bool.pick(R.T8, U32.is_ne(Foe.flinch(f), 0),
    Foe.set_t(Foe.set_dk(Foe.set_st(f, C.fs_drift()), 0), 30),
    Foe.set_t(Foe.set_st(Foe.set_t(f, t), Bool.pick(U32, U32.is_eq(t, 0), C.fs_strike(), C.fs_tell())), Bool.pick(U32, U32.is_eq(t, 0), C.lunge_ticks(), t)))

def snuffer_strike(+f: R.T8) -> R.T8:
  +t = M.dec(Foe.t(f))
  +f1 = Foe.set_y(Foe.set_x(f, U32.add(Foe.x(f), N.sdiv(U32.mul(Foe.fx(f), C.lunge_speed()), 256))), U32.add(Foe.y(f), N.sdiv(U32.mul(Foe.fy(f), C.lunge_speed()), 256)))
  +done = U32.is_eq(t, 0)
  Foe.set_t(Foe.set_dk(Foe.set_st(f1, Bool.pick(U32, done, C.fs_recover(), C.fs_strike())), Bool.pick(U32, done, 0, Foe.dk(f1))), Bool.pick(U32, done, C.snuffer_recover(), t))

def snuffer_recover(+f: R.T8) -> R.T8:
  +t = M.dec(Foe.t(f))
  +done = U32.is_eq(t, 0)
  +again = U32.add(U32.add(22, U32.mul(U32.mod(Foe.id(f), 4), 6)), U32.mul(U32.mod(Foe.n(f), 3), 4))
  Foe.set_n(Foe.set_t(Foe.set_st(f, Bool.pick(U32, done, C.fs_drift(), C.fs_recover())), Bool.pick(U32, done, again, t)), Bool.pick(U32, done, U32.and(U32.add(Foe.n(f), 1), 255), Foe.n(f)))

def snuffer_stagger(+f: R.T8) -> R.T8:
  +t = M.dec(Foe.t(f))
  +done = U32.is_eq(t, 0)
  Foe.set_t(Foe.set_open(Foe.set_st(f, Bool.pick(U32, done, C.fs_drift(), C.fs_stagger())), Bool.pick(U32, done, 0, Foe.open(f))), Bool.pick(U32, done, 24, t))

def snuffer_dormant(+f: R.T8, +d: U32) -> R.T8:
  +wake = N.slt(d, U32.mul(C.aggro(), 256)) || U32.is_lt(Foe.hp(f), Foe.full(f))
  Foe.set_t(Foe.set_st(f, Bool.pick(U32, wake, C.fs_drift(), C.fs_dormant())), Bool.pick(U32, wake, U32.add(18, U32.mul(U32.mod(Foe.id(f), 5), 4)), Foe.t(f)))

def snuffer_next(+ds: Nat, +f: R.T8, +tx: U32, +ty: U32, +d: U32) -> R.T8:
  match ds:
    case 0n:
      snuffer_dormant(f, d)
    case 1n+a1:
      match a1:
        case 0n:
          snuffer_drift(f, tx, ty, d)
        case 1n+a2:
          match a2:
            case 0n:
              snuffer_tell(f)
            case 1n+a3:
              match a3:
                case 0n:
                  snuffer_strike(f)
                case 1n+a4:
                  match a4:
                    case 0n:
                      snuffer_recover(f)
                    case 1n+a5:
                      match a5:
                        case 0n:
                          snuffer_stagger(f)
                        case 1n+a6:
                          f

# ----- the turret -------------------------------------------------------------------

# A big foe's tell for a table of `seats`: tighter the more knights, never under
# the floor.
def tell_for(+w: S, +seats: U32) -> U32: U32.max(M.sat(W.r_tell(w), U32.mul(C.tell_step(), M.dec(seats))), C.tell_floor())

def turret_wait(+f: R.T8, +tx: U32, +ty: U32, +d: U32, +tell: U32) -> R.T8:
  +t = M.dec(Foe.t(f))
  +aim = N.slt(d, U32.mul(C.turret_range(), 256)) && U32.is_eq(t, 0)
  +u = G.unit(U32.sub(tx, Foe.x(f)), U32.sub(ty, Foe.y(f)))
  +ux = G.vx(u)
  +uy = G.vy(u)
  +f1 = Foe.set_t(f, t)
  +f2 = Foe.set_d4(Foe.set_d3(Foe.set_d2(Foe.set_d1(Foe.set_d0(Foe.set_dk(Foe.set_fy(Foe.set_fx(f1, ux), uy), C.dg_capsule()), Foe.x(f1)), Foe.y(f1)), U32.add(Foe.x(f1), U32.mul(ux, C.turret_range()))), U32.add(Foe.y(f1), U32.mul(uy, C.turret_range()))), 1280)
  Bool.pick(R.T8, aim, Foe.set_t(Foe.set_st(f2, C.fs_tell()), tell), f1)

def turret_tell(+f: R.T8) -> R.T8:
  +t = M.dec(Foe.t(f))
  +fire = U32.is_eq(t, 0)
  Foe.set_t(Foe.set_st(f, Bool.pick(U32, fire, C.fs_strike(), C.fs_tell())), Bool.pick(U32, fire, 6, t))

def turret_strike(+f: R.T8) -> R.T8:
  +t = M.dec(Foe.t(f))
  +done = U32.is_eq(t, 0)
  Foe.set_t(Foe.set_dk(Foe.set_st(f, Bool.pick(U32, done, C.fs_recover(), C.fs_strike())), Bool.pick(U32, done, 0, Foe.dk(f))), Bool.pick(U32, done, C.turret_recover(), t))

def turret_recover(+f: R.T8) -> R.T8:
  +t = M.dec(Foe.t(f))
  +done = U32.is_eq(t, 0)
  Foe.set_t(Foe.set_st(f, Bool.pick(U32, done, C.fs_dormant(), C.fs_recover())), Bool.pick(U32, done, 0, t))

def turret_stagger(+f: R.T8) -> R.T8:
  +t = M.dec(Foe.t(f))
  +done = U32.is_eq(t, 0)
  Foe.set_t(Foe.set_open(Foe.set_st(f, Bool.pick(U32, done, C.fs_dormant(), C.fs_stagger())), Bool.pick(U32, done, 0, Foe.open(f))), t)

# One tick of a turret: wait for a knight in sight, lock the line, loose an iron
# bolt along it.
def turret_next(+ds: Nat, +f: R.T8, +seen: Bool, +tx: U32, +ty: U32, +d: U32, +tell: U32) -> R.T8:
  match ds:
    case 0n:
      Z.lw64(seen, u => turret_wait(f, tx, ty, d, tell), f)
    case 1n+a1:
      match a1:
        case 0n:
          Z.lw64(seen, u => turret_wait(f, tx, ty, d, tell), f)
        case 1n+a2:
          match a2:
            case 0n:
              turret_tell(f)
            case 1n+a3:
              match a3:
                case 0n:
                  turret_strike(f)
                case 1n+a4:
                  match a4:
                    case 0n:
                      turret_recover(f)
                    case 1n+a5:
                      match a5:
                        case 0n:
                          turret_stagger(f)
                        case 1n+a6:
                          f

# ----- a tick of every foe -------------------------------------------------------------

# A snuffer that lunged this tick lands its blow where its footprint was.
def push_strike(+w: S, +i: U32, +f: R.T8, +dmg: U32) -> S:
  W.set_strikes(w, L.push8(W.strikes(w), R.W8{i, dmg, Foe.dk(f), Foe.d0(f), Foe.d1(f), Foe.d2(f), Foe.d3(f), Foe.d4(f)}))

# A turret that loosed its bolt this tick: the bolt waits to be pushed after the
# loop, with the shot's sound at once.
def push_shot(+w: S, +f: R.T8) -> S:
  Q.sound(W.set_shots(w, L.push8(W.shots(w), R.W8{C.bk_iron(), U32.add(Foe.x(f), U32.mul(Foe.fx(f), 10)), U32.add(Foe.y(f), U32.mul(Foe.fy(f), 10)), N.sdiv(U32.mul(Foe.fx(f), C.bolt_speed()), 256), N.sdiv(U32.mul(Foe.fy(f), C.bolt_speed()), 256), C.bolt_life(), 0, 0})), 33)

def foe_act(+w: S, +i: U32, +f: R.T8, +g: R.T8, +fired: Bool, +struck: Bool) -> S:
  +w1 = W.set_foes(w, L.set64(W.foes(w), U32.to_nat(i), g))
  +w2 = Bool.pick(S, struck, push_strike(w1, i, f, C.snuffer_damage()), w1)
  Bool.pick(S, fired, push_shot(w2, f), w2)

# The foe steps from the state it is in, given the knight it hunts.
def foe_step(+w: S, +i: U32, +f: R.T8, +tgt: R.W8) -> S:
  +seats = L.len64(W.seats(w))
  +seen = U32.is_eq(Nr.ok(tgt), 1)
  +snuffer = U32.is_eq(Foe.kind(f), C.fk_snuffer())
  +g = Z.lz64(snuffer, u => Z.lw64(seen, u => snuffer_next(U32.to_nat(Foe.st(f)), f, Nr.x(tgt), Nr.y(tgt), Nr.d(tgt)), f), u => turret_next(U32.to_nat(Foe.st(f)), f, seen, Nr.x(tgt), Nr.y(tgt), Nr.d(tgt), tell_for(w, seats)))
  foe_act(w, i, f, g, U32.is_eq(Foe.kind(f), C.fk_turret()) && U32.is_eq(Foe.st(f), C.fs_tell()) && U32.is_eq(Foe.st(g), C.fs_strike()), snuffer && seen && U32.is_eq(Foe.st(f), C.fs_strike()))

# A foe before its step: held a tick by a blow, or pushed, and lit or not.
def foe_prepare(+f: R.T8, +burning: Bool) -> R.T8:
  +f1 = Foe.set_flinch(f, M.dec(Foe.flinch(f)))
  +f2 = Foe.set_lit(f1, Bool.pick(U32, burning, C.lit_hold(), M.dec(Foe.lit(f1))))
  +pushed = U32.is_ne(Foe.kbt(f2), 0)
  Bool.pick(R.T8, pushed, Foe.set_y(Foe.set_x(Foe.set_kbt(f2, M.dec(Foe.kbt(f2))), U32.add(Foe.x(f2), Foe.kbx(f2))), U32.add(Foe.y(f2), Foe.kby(f2))), f2)

def foe_tick_live(+frozen: Bool, +w: S, +i: U32, +f: R.T8, +tgt: R.W8, +burning: Bool) -> S:
  match frozen:
    case True{}:
      W.set_foes(w, L.set64(W.foes(w), U32.to_nat(i), Foe.set_freeze(f, M.dec(Foe.freeze(f)))))
    case False{}:
      foe_step(w, i, foe_prepare(f, burning), tgt)

def foe_tick_dying(+dying: Bool, +w: S, +i: U32, +f: R.T8, +tgt: R.W8, +burning: Bool) -> S:
  match dying:
    case True{}:
      W.set_foes(w, L.set64(W.foes(w), U32.to_nat(i), Foe.set_dead(f, U32.min(U32.add(Foe.dead(f), 1), 255))))
    case False{}:
      foe_tick_live(U32.is_ne(Foe.freeze(f), 0), w, i, f, tgt, burning)

def foe_tick(+w: S, +i: U32) -> S:
  +f = L.get64(W.foes(w), U32.to_nat(i))
  foe_tick_dying(U32.is_eq(Foe.st(f), C.fs_dying()), w, i, f, Q.nearest_knight(w, Foe.x(f), Foe.y(f)), Q.burning(w, Foe.x(f), Foe.y(f)))

def foes_go(+n: Nat, +w: S, +i: U32) -> S:
  match n:
    case 0n:
      w
    case 1n+p:
      foes_go(p, foe_tick(w, i), U32.add(i, 1))

# The nearest open point to one that is in a wall: the rings out from it, each of
# eight ways, the first that fits. ok, then the point (x, y) of a Nr.
def nudge_go(+n: Nat, +k: U32, +w: S, +x: U32, +y: U32, +r: U32, +found: Bool, +ox: U32, +oy: U32) -> R.W8:
  match n:
    case 0n:
      R.W8{Bool.to_u32(found), 0, Bool.pick(U32, found, ox, x), Bool.pick(U32, found, oy, y), 0, 0, 0, 0}
    case 1n+p:
      match found:
        case True{}:
          R.W8{1, 0, ox, oy, 0, 0, 0, 0}
        case False{}:
          +ring = U32.add(U32.div(k, 8), 1)
          +dir = U32.mod(k, 8)
          +dx = Bool.pick(U32, U32.is_eq(dir, 0) || U32.is_eq(dir, 4) || U32.is_eq(dir, 6), 1, Bool.pick(U32, U32.is_eq(dir, 1) || U32.is_eq(dir, 5) || U32.is_eq(dir, 7), N.sneg(1), 0))
          +dy = Bool.pick(U32, U32.is_eq(dir, 2) || U32.is_eq(dir, 4) || U32.is_eq(dir, 7), 1, Bool.pick(U32, U32.is_eq(dir, 3) || U32.is_eq(dir, 5) || U32.is_eq(dir, 6), N.sneg(1), 0))
          +px = U32.add(x, U32.mul(U32.mul(dx, ring), 1024))
          +py = U32.add(y, U32.mul(U32.mul(dy, ring), 1024))
          nudge_go(p, U32.add(k, 1), w, x, y, r, Q.clear(w, px, py, r), px, py)

def nudge(+w: S, +x: U32, +y: U32, +r: U32) -> R.W8: nudge_go(64n, 0, w, x, y, r, False{}, x, y)

# Keep a snuffer out of the walls it drifts into.
def wall_foe(+stuck: Bool, +w: S, +i: U32, +f: R.T8) -> S:
  match stuck:
    case False{}:
      w
    case True{}:
      +n = nudge(w, Foe.x(f), Foe.y(f), 1024)
      W.set_foes(w, L.set64(W.foes(w), U32.to_nat(i), Foe.set_y(Foe.set_x(f, Nr.x(n)), Nr.y(n))))

def walls_go(+n: Nat, +w: S, +i: U32) -> S:
  match n:
    case 0n:
      w
    case 1n+p:
      +f = L.get64(W.foes(w), U32.to_nat(i))
      walls_go(p, wall_foe(U32.is_eq(Foe.kind(f), C.fk_snuffer()) && Bool.not(Q.clear(w, Foe.x(f), Foe.y(f), 1024)), w, i, f), U32.add(i, 1))

# The turrets' bolts fly, as many as there is room for.
def shots_go(+ss: List<&2, R.W8>, +w: S) -> S:
  match ss:
    case Nil{}:
      w
    case Con{b, t}:
      shots_go(t, Bool.pick(S, U32.is_lt(L.len8(W.bolts(w)), C.bolt_cap()), W.set_bolts(w, L.push8(W.bolts(w), b)), w))

# A parried blow staggers its foe open; a blocked one pushes it back.
def strike_blocked(+w: S, +i: U32) -> S:
  +f = L.get64(W.foes(w), U32.to_nat(i))
  W.set_foes(w, L.set64(W.foes(w), U32.to_nat(i), Foe.set_kbt(Foe.set_kby(Foe.set_kbx(f, N.sdiv(N.sneg(Foe.fx(f)), 40)), N.sdiv(N.sneg(Foe.fy(f)), 40)), 6)))

def strike_parried(+w: S, +i: U32) -> S:
  +f = L.get64(W.foes(w), U32.to_nat(i))
  Q.stop(W.set_foes(w, L.set64(W.foes(w), U32.to_nat(i), Foe.set_freeze(Foe.set_dk(Foe.set_open(Foe.set_t(Foe.set_st(f, C.fs_stagger()), C.stagger_ticks()), 1), 0), 3))), C.stop_phase())

def strike_after(+res: U32, +w: S, +i: U32) -> S:
  Bool.pick(S, U32.is_eq(res, C.r_parried()), strike_parried(w, i), Bool.pick(S, U32.is_eq(res, C.r_blocked()), strike_blocked(w, i), w))

def strike_blow(+w: S, +i: U32, +id: U32, +dmg: U32, +fromx: U32, +fromy: U32) -> S:
  +w1 = H.blow(w, id, dmg, fromx, fromy, C.h_melee())
  strike_after(W.ret(w1), w1, i)

def strike_hit(+hit: Bool, +w: S, +i: U32, +id: U32, +dmg: U32) -> S:
  match hit:
    case False{}:
      w
    case True{}:
      +f = L.get64(W.foes(w), U32.to_nat(i))
      strike_blow(w, i, id, dmg, Foe.x(f), Foe.y(f))

def strike_mark(+w: S, +i: U32, +bit: U32) -> S:
  +f = L.get64(W.foes(w), U32.to_nat(i))
  W.set_foes(w, L.set64(W.foes(w), U32.to_nat(i), Foe.set_struck(f, U32.or(Foe.struck(f), bit))))

# Foe `i`'s blow on knight `id`, if the knight stands in the footprint and has not
# been struck by it.
def strike_one(+w: S, +i: U32, +id: U32, +s: R.W8) -> S:
  +k = H.seat_get(W.seats(w), id)
  +f = L.get64(W.foes(w), U32.to_nat(i))
  +bit = U32.shln(1, U32.to_nat(U32.min(id, 7)))
  +hit = U32.is_eq(U32.and(Foe.struck(f), bit), 0) && H.seat_has(W.seats(w), id) && Q.up(k) && D.hits(St.dk(s), St.d0(s), St.d1(s), St.d2(s), St.d3(s), St.d4(s), Seat.x(k), Seat.y(k), 1280)
  strike_hit(hit, Bool.pick(S, hit, strike_mark(w, i, bit), w), i, id, St.dmg(s))

def strike_ids(+ids: List<&2, U32>, +w: S, +s: R.W8) -> S:
  match ids:
    case Nil{}:
      w
    case Con{id, t}:
      strike_ids(t, strike_one(w, St.i(s), id, s), s)

def strikes_go(+ss: List<&2, R.W8>, +w: S) -> S:
  match ss:
    case Nil{}:
      w
    case Con{s, t}:
      strikes_go(t, strike_ids(H.seat_ids(W.seats(w)), w, s))

# One tick of every foe.
def step_foes(+w: S) -> S:
  +w1 = foes_go(U32.to_nat(L.len64(W.foes(w))), W.set_shots(W.set_strikes(w, Nil{}), Nil{}), 0)
  +w2 = walls_go(U32.to_nat(L.len64(W.foes(w1))), w1, 0)
  +w3 = strikes_go(W.strikes(w2), shots_go(W.shots(w2), w2))
  W.set_foes(w3, L.keep64(~(f => Bool.not(U32.is_eq(Foe.st(f), C.fs_dying()) && U32.is_ge(Foe.dead(f), C.death_ticks()))), W.foes(w3)))
