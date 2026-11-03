# The Warden: a great lantern-bearer whose lamp-heart is shuttered. A thrown bolt
# of light, or a flare held on it, opens the shutter for a while (from the second
# third of its health, two knights' light at once); a parried beam knocks it open
# too. Then the blade counts. Three phases, every tell at least 40 ticks, every
# footprint shown for the whole tell. crates/delve/src/rules/lanternward/boss.rs.
import Base
import ./r.bend as R
import ./c.bend as C
import ./n.bend as N
import ./m.bend as M
import ./l.bend as L
import ./g.bend as G
import ./gr.bend as Gr
import ./sh.bend as D
import ./w.bend as W
import ./pr.bend as Pr
import ./nr.bend as Nr
import ./se.bend as Seat
import ./fe.bend as Foe
import ./b.bend as B
import ./z.bend as Z
import ./q.bend as Q
import ./ht.bend as H
import ./fo.bend as Fo

def S() -> Data: R.W

# ----- the flare on the heart ----------------------------------------------------------

def heart_lit_one(+near: Bool, +w: S, +id: U32, +kx: U32, +ky: U32, +hx: U32, +hy: U32) -> S:
  match near:
    case False{}:
      w
    case True{}:
      Bool.pick(S, Q.line_open(w, kx, ky, hx, hy), H.light_heart(w, id), w)

# Flares held on the heart keep it lit.
def heart_lit_go(+ks: List<&2, R.T8>, +w: S, +hx: U32, +hy: U32) -> S:
  match ks:
    case Nil{}:
      w
    case Con{k, t}:
      heart_lit_go(t, heart_lit_one(Q.flaring(k) && Q.up(k) && N.sle(G.dist(U32.sub(Seat.x(k), hx), U32.sub(Seat.y(k), hy)), U32.mul(C.heart_flare_r(), 256)), w, Seat.id(k), Seat.x(k), Seat.y(k), hx, hy), hx, hy)

# ----- its blows ---------------------------------------------------------------------------

# Knight `id` stands in the line and has not been struck: the blow lands. `ret2`
# collects whether any was parried (bit 1); the struck bits are the Warden's.
def hit_blow(+w: S, +id: U32, +dmg: U32, +kind: U32, +bit: U32) -> S:
  +w1 = H.blow(w, id, dmg, B.x(w), B.y(w), kind)
  +res = W.ret(w1)
  +w2 = Bool.pick(S, U32.is_ne(res, C.r_dodged()), B.set_struck(w1, U32.or(B.struck(w1), bit)), w1)
  W.set_ret2(w2, U32.or(W.ret2(w), Bool.to_u32(U32.is_eq(res, C.r_parried()))))

def hit_one(+w: S, +id: U32, +dmg: U32, +kind: U32, +dk: U32, +d0: U32, +d1: U32, +d2: U32, +d3: U32, +d4: U32) -> S:
  +k = H.seat_get(W.seats(w), id)
  +bit = U32.shln(1, U32.to_nat(U32.min(id, 7)))
  +hit = U32.is_eq(U32.and(B.struck(w), bit), 0) && Q.up(k) && D.hits(dk, d0, d1, d2, d3, d4, Seat.x(k), Seat.y(k), 1280)
  Bool.pick(S, hit, hit_blow(w, id, dmg, kind, bit), w)

def hit_ids(+ids: List<&2, U32>, +w: S, +dmg: U32, +kind: U32, +dk: U32, +d0: U32, +d1: U32, +d2: U32, +d3: U32, +d4: U32) -> S:
  match ids:
    case Nil{}:
      w
    case Con{id, t}:
      hit_ids(t, hit_one(w, id, dmg, kind, dk, d0, d1, d2, d3, d4), dmg, kind, dk, d0, d1, d2, d3, d4)

# The Warden's blow on every knight in the footprint it has not yet struck. A
# parry sets bit 1 of `ret2`.
def warden_hits(+w: S, +dmg: U32, +kind: U32, +dk: U32, +d0: U32, +d1: U32, +d2: U32, +d3: U32, +d4: U32) -> S:
  hit_ids(H.seat_ids(W.seats(w)), w, dmg, kind, dk, d0, d1, d2, d3, d4)

# ----- summoning -------------------------------------------------------------------------

# The index of the first prop of a kind standing on a tile, or none.
def prop_find(+ps: List<&2, R.W8>, +kind: U32, +tx: U32, +ty: U32, +j: U32, +found: U32, +done: Bool) -> U32:
  match ps:
    case Nil{}:
      found
    case Con{p, t}:
      match done:
        case True{}:
          found
        case False{}:
          +hit = U32.is_eq(Pr.kind(p), kind) && U32.is_eq(Pr.tx(p), tx) && U32.is_eq(Pr.ty(p), ty)
          prop_find(t, kind, tx, ty, U32.add(j, 1), Bool.pick(U32, hit, j, found), hit)

def fissure_wakes(+w: S, +tx: U32, +ty: U32) -> S:
  +j = prop_find(W.props(w), C.pk_fissure(), tx, ty, 0, C.none(), False{})
  Bool.pick(S, U32.is_ne(j, C.none()), W.set_props(w, L.set8(W.props(w), U32.to_nat(j), Pr.set_t(L.get8(W.props(w), U32.to_nat(j)), 40))), w)

# The foe last in the list, if any, climbs out drifting.
def last_climbs(+w: S) -> S:
  +n = L.len64(W.foes(w))
  +i = U32.to_nat(M.dec(n))
  Bool.pick(S, U32.is_ne(n, 0), W.set_foes(w, L.set64(W.foes(w), i, Foe.set_t(Foe.set_st(L.get64(W.foes(w), i), C.fs_drift()), 30))), w)

def summon_one(+w: S, +mark: U32, +hooded: Bool) -> S:
  +tx = U32.shrn(mark, 16n)
  +ty = U32.and(mark, 65535)
  last_climbs(fissure_wakes(Fo.spawn_foe(w, C.fk_snuffer(), G.middle(tx), G.middle(ty), hooded), tx, ty))

def summon_go(+n: Nat, +k: U32, +w: S, +count: U32, +hooded: Bool) -> S:
  match n:
    case 0n:
      w
    case 1n+p:
      summon_go(p, U32.add(k, 1), Z.lw(U32.is_lt(k, count), u => summon_one(w, Gr.nth_hit(Gr.find(Gr.warden(), 102), k), hooded), w), count, hooded)

# Snuffers climb out of the fissures (hooded, in the last phase).
def summon(+w: S) -> S:
  +seats = U32.max(L.len64(W.seats(w)), 1)
  +n = U32.min(U32.add(2, U32.div(U32.sub(seats, 1), 2)), 4)
  summon_go(U32.to_nat(n), 0, w, Gr.hits(Gr.find(Gr.warden(), 102)), U32.is_ge(B.phase(w), 2))

# ----- choosing a blow ---------------------------------------------------------------------

# The attack the Warden makes next, by its phase and how many it has made.
def pattern(+phase: U32, +n: U32) -> U32:
  Bool.pick(U32, U32.is_eq(phase, 0), Bool.pick(U32, U32.is_eq(U32.mod(n, 4), 0), C.atk_slam(), Bool.pick(U32, U32.is_eq(U32.mod(n, 4), 3), C.atk_slam(), C.atk_beam())),
         Bool.pick(U32, U32.is_eq(phase, 1), Bool.pick(U32, U32.is_eq(U32.mod(n, 4), 0), C.atk_beam(), Bool.pick(U32, U32.is_eq(U32.mod(n, 4), 1), C.atk_summon(), Bool.pick(U32, U32.is_eq(U32.mod(n, 4), 2), C.atk_slam(), C.atk_beam()))),
           Bool.pick(U32, U32.is_eq(U32.mod(n, 5), 0), C.atk_beam(), Bool.pick(U32, U32.is_eq(U32.mod(n, 5), 1), C.atk_slam(), Bool.pick(U32, U32.is_eq(U32.mod(n, 5), 2), C.atk_summon(), Bool.pick(U32, U32.is_eq(U32.mod(n, 5), 3), C.atk_beam(), C.atk_slam())))))
)
# Choose the Warden's next attack and show where it will land.
def begin_attack(+w: S, +tx: U32, +ty: U32) -> S:
  +atk = pattern(B.phase(w), B.n(w))
  +u = G.unit(U32.sub(tx, B.x(w)), U32.sub(ty, B.y(w)))
  +beam = U32.is_eq(atk, C.atk_beam())
  +slam = U32.is_eq(atk, C.atk_slam())
  +aim = G.angle_of(U32.sub(tx, B.x(w)), U32.sub(ty, B.y(w)))
  +w1 = B.set_struck(B.set_fy(B.set_fx(B.set_t(B.set_st(B.set_atk(w, atk), C.ws_tell()), Fo.tell_for(w, L.len64(W.seats(w)))), G.vx(u)), G.vy(u)), 0)
  +w2 = B.set_d4(B.set_d3(B.set_d2(B.set_d1(B.set_d0(B.set_dk(B.set_aim(w1, Bool.pick(U32, beam, aim, B.aim(w1))), Bool.pick(U32, beam, C.dg_fan(), Bool.pick(U32, slam, C.dg_circle(), C.dg_none()))), B.x(w1)), B.y(w1)), Bool.pick(U32, beam, aim, U32.mul(C.slam_reach(), 256))), Bool.pick(U32, beam, U32.add(C.beam_half(), 6), B.d3(w1))), Bool.pick(U32, beam, U32.mul(C.beam_len(), 256), B.d4(w1)))
  Bool.pick(S, beam || slam, w2, B.set_dk(w2, 0))

# ----- one tick of the Warden -----------------------------------------------------------------

def boss_dies(+w: S) -> S:
  +dead = U32.min(U32.add(B.dead(w), 1), 600)
  +w1 = B.set_dead(w, dead)
  Bool.pick(S, U32.is_eq(dead, 40), W.set_stage(Q.drop_at(w1, C.dk_container(), B.x(w1), B.y(w1)), 2), w1)

def boss_wakes(+w: S) -> S:
  W.set_shake(Q.sound(W.set_stage(B.set_t(B.set_st(w, C.ws_rest()), 70), 1), 6), C.shake_heavy())

def boss_asleep(+w: S, +tgt: R.W8) -> S:
  Bool.pick(S, (U32.is_eq(Nr.ok(tgt), 1) && N.slt(Nr.d(tgt), U32.mul(C.warden_wake(), 256))) || U32.is_gt(W.stage_t(w), C.warden_nap()), boss_wakes(w), w)

def boss_walks(+w: S, +nx: U32, +ny: U32) -> S:
  Bool.pick(S, Q.clear(w, nx, ny, 2304), B.set_y(B.set_x(w, nx), ny), w)

def boss_closes(+w: S, +tgt: R.W8) -> S:
  +w1 = B.set_fy(B.set_fx(w, G.vx(G.unit(U32.sub(Nr.x(tgt), B.x(w)), U32.sub(Nr.y(tgt), B.y(w))))), G.vy(G.unit(U32.sub(Nr.x(tgt), B.x(w)), U32.sub(Nr.y(tgt), B.y(w)))))
  Bool.pick(S, N.sgt(Nr.d(tgt), U32.add(4608, U32.mul(26, 256))), boss_walks(w1, U32.add(B.x(w1), N.sdiv(U32.mul(B.fx(w1), C.warden_walk()), 256)), U32.add(B.y(w1), N.sdiv(U32.mul(B.fy(w1), C.warden_walk()), 256))), w1)

def boss_waits(+w: S, +tgt: R.W8) -> S:
  +w1 = boss_closes(w, tgt)
  +t = M.dec(B.t(w1))
  Z.lz(U32.is_eq(t, 0), u => begin_attack(B.set_t(w1, t), Nr.x(tgt), Nr.y(tgt)), u => B.set_t(w1, t))

def boss_rest(+w: S, +tgt: R.W8) -> S: Z.lw(U32.is_eq(Nr.ok(tgt), 1), u => boss_waits(w, tgt), w)

# The tell is over: the blow begins, and a summoning calls up its snuffers.
def boss_strikes(+w: S) -> S:
  +atk = B.atk(w)
  +w1 = B.set_t(B.set_sweep(B.set_struck(B.set_st(w, C.ws_strike()), 0), 0), Bool.pick(U32, U32.is_eq(atk, C.atk_beam()), C.beam_ticks(), Bool.pick(U32, U32.is_eq(atk, C.atk_slam()), C.slam_ticks(), 12)))
  +w2 = Z.lw(U32.is_eq(atk, C.atk_summon()), u => summon(w1), w1)
  Q.sound(w2, Bool.pick(U32, U32.is_eq(atk, C.atk_beam()), 32, Bool.pick(U32, U32.is_eq(atk, C.atk_slam()), 28, 11)))

def boss_tell(+w: S) -> S:
  +t = M.dec(B.t(w))
  Z.lz(U32.is_eq(t, 0), u => boss_strikes(B.set_t(w, t)), u => B.set_t(w, t))

# The beam: each of one or two lines, swept across the fan as the blow runs.
def beam_line(+w: S, +b: U32, +elapsed: U32) -> S:
  +angle = U32.add(U32.sub(B.aim(w), C.beam_half()), N.sdiv(U32.mul(U32.mul(C.beam_half(), 2), elapsed), C.beam_ticks()))
  +a = U32.add(angle, U32.mul(180, b))
  +tipx = U32.add(B.x(w), U32.mul(G.cos_q(a), C.beam_len()))
  +tipy = U32.add(B.y(w), U32.mul(G.sin_q(a), C.beam_len()))
  +w1 = Bool.pick(S, U32.is_eq(b, 0), B.set_d4(B.set_d3(B.set_d2(B.set_d1(B.set_d0(B.set_dk(w, C.dg_capsule()), B.x(w)), B.y(w)), tipx), tipy), 1536), w)
  warden_hits(w1, C.beam_damage(), C.h_beam(), C.dg_capsule(), B.x(w), B.y(w), tipx, tipy, 1536)

def boss_beam(+w: S, +elapsed: U32) -> S:
  +w1 = beam_line(w, 0, elapsed)
  Z.lw(U32.is_ge(B.phase(w), 2), u => beam_line(w1, 1, elapsed), w1)

# The slam: a ring running out along the ground that a hop or a roll gets through.
def boss_slam(+w: S, +elapsed: U32) -> S:
  +radius = U32.mul(U32.add(12, U32.mul(C.slam_speed(), elapsed)), 256)
  +w1 = B.set_d2(B.set_d1(B.set_d0(B.set_dk(w, C.dg_circle()), B.x(w)), B.y(w)), U32.mul(C.slam_reach(), 256))
  +w2 = warden_hits(w1, C.slam_damage(), C.h_low(), C.dg_ring(), B.x(w), B.y(w), radius, 1536, 0)
  Bool.pick(S, U32.is_eq(elapsed, 0), Q.fx(w2, C.fxk_wave(), B.x(w2), B.y(w2), 0), w2)

def strike_end(+parried: Bool, +w: S) -> S:
  match parried:
    case True{}:
      W.set_shake(Q.stop(Q.sound(B.set_l5(B.set_l4(B.set_l3(B.set_l2(B.set_l1(B.set_l0(B.set_dk(B.set_t(B.set_st(w, C.ws_stagger()), C.stagger_boss()), 0), 0), 0), 0), 0), 0), 0), 5), C.stop_phase()), C.shake_heavy())
    case False{}:
      +t = M.dec(B.t(w))
      +done = U32.is_eq(t, 0)
      B.set_dk(B.set_t(B.set_st(B.set_t(w, t), Bool.pick(U32, done, C.ws_recover(), B.st(w))), Bool.pick(U32, done, Bool.pick(U32, U32.is_eq(B.atk(w), C.atk_summon()), 30, C.recover_ticks()), t)), Bool.pick(U32, done, 0, B.dk(w)))

def boss_strike(+w: S) -> S:
  +atk = B.atk(w)
  +total = Bool.pick(U32, U32.is_eq(atk, C.atk_beam()), C.beam_ticks(), Bool.pick(U32, U32.is_eq(atk, C.atk_slam()), C.slam_ticks(), 12))
  +elapsed = U32.sub(total, B.t(w))
  +w0 = W.set_ret2(w, 0)
  +w1 = Z.lz(U32.is_eq(atk, C.atk_beam()), u => boss_beam(w0, elapsed), u => Z.lw(U32.is_eq(atk, C.atk_slam()), u => boss_slam(w0, elapsed), w0))
  strike_end(U32.is_ne(W.ret2(w1), 0), w1)

def boss_recover(+w: S) -> S:
  +t = M.dec(B.t(w))
  +done = U32.is_eq(t, 0)
  B.set_n(B.set_t(B.set_st(w, Bool.pick(U32, done, C.ws_rest(), C.ws_recover())), Bool.pick(U32, done, U32.sub(C.rest_ticks(), U32.mul(12, B.phase(w))), t)), Bool.pick(U32, done, U32.and(U32.add(B.n(w), 1), 255), B.n(w)))

def boss_stagger(+w: S) -> S:
  +t = M.dec(B.t(w))
  +done = U32.is_eq(t, 0)
  B.set_t(B.set_st(w, Bool.pick(U32, done, C.ws_rest(), C.ws_stagger())), Bool.pick(U32, done, 30, t))

def boss_step(+ds: Nat, +w: S, +tgt: R.W8) -> S:
  match ds:
    case 0n:
      boss_asleep(w, tgt)
    case 1n+a1:
      match a1:
        case 0n:
          boss_rest(w, tgt)
        case 1n+a2:
          match a2:
            case 0n:
              boss_tell(w)
            case 1n+a3:
              match a3:
                case 0n:
                  boss_strike(w)
                case 1n+a4:
                  match a4:
                    case 0n:
                      boss_recover(w)
                    case 1n+a5:
                      match a5:
                        case 0n:
                          boss_stagger(w)
                        case 1n+a6:
                          match a6:
                            case 0n:
                              boss_dies(w)
                            case 1n+a7:
                              w

# A knight cannot stand in the Warden: a body in it is put out at its rim.
def push_out(+k: R.T8, +bx: U32, +by: U32) -> R.T8:
  +dx = U32.sub(Seat.x(k), bx)
  +dy = U32.sub(Seat.y(k), by)
  +d = G.dist(dx, dy)
  +min = U32.add(4608, 1280)
  +u = G.unit(dx, dy)
  Bool.pick(R.T8, Q.up(k) && N.slt(d, min) && N.sgt(d, 0), Seat.set_y(Seat.set_x(k, U32.add(bx, N.sdiv(U32.mul(G.vx(u), min), 256))), U32.add(by, N.sdiv(U32.mul(G.vy(u), min), 256))), k)

def push_all(+ks: List<&2, R.T8>, +bx: U32, +by: U32) -> List<&2, R.T8>:
  match ks:
    case Nil{}:
      Nil{}
    case Con{k, t}:
      Con{push_out(k, bx, by), push_all(t, bx, by)}

def boss_body(+w: S) -> S:
  Bool.pick(S, U32.is_ne(B.st(w), C.ws_dying()), W.set_seats(w, push_all(W.seats(w), B.x(w), B.y(w))), w)

def boss_live(+w: S) -> S:
  +w1 = B.set_flinch(w, M.dec(B.flinch(w)))
  boss_body(boss_step(U32.to_nat(B.st(w1)), w1, Q.nearest_knight(w1, B.x(w1), B.y(w1))))

# One tick of the Warden.
def step_boss(+w: S) -> S:
  +w1 = heart_lit_go(W.seats(w), w, Q.heart_x(w), Q.heart_y(w))
  Z.lw(U32.is_ne(W.boss_on(w), 0), u => boss_live(w1), w)
