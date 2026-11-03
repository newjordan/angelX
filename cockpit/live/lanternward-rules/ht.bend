# What blows do: a foe's on a knight (the shield blocks, the shield in the first
# few ticks parries), a knight's on a foe or on the Warden, and the flight of
# every bolt: iron turned back by a shield, light thrown from a lantern; and the
# bombs. crates/delve/src/rules/lanternward/blow.rs, and the Warden's hurts from
# boss.rs. A knight is hurt by taking it from the table into `cur`, changing it
# there and sitting it down again.
import Base
import ./r.bend as R
import ./c.bend as C
import ./n.bend as N
import ./m.bend as M
import ./l.bend as L
import ./g.bend as G
import ./w.bend as W
import ./pr.bend as Pr
import ./bo.bend as Bo
import ./fu.bend as Fu
import ./nr.bend as Nr
import ./se.bend as Seat
import ./k.bend as K
import ./fe.bend as Foe
import ./b.bend as B
import ./z.bend as Z
import ./q.bend as Q

def S() -> Data: R.W

# ----- the table ----------------------------------------------------------------

# The seat with this id, zeros when it is not at the table.
def seat_get(+ks: List<&2, R.T8>, +id: U32) -> R.T8:
  match ks:
    case Nil{}:
      L.zero64()
    case Con{k, t}:
      Bool.pick(R.T8, U32.is_eq(Seat.id(k), id), k, seat_get(t, id))

def seat_has(+ks: List<&2, R.T8>, +id: U32) -> Bool:
  match ks:
    case Nil{}:
      False{}
    case Con{k, t}:
      U32.is_eq(Seat.id(k), id) || seat_has(t, id)

def seat_drop(+ks: List<&2, R.T8>, +id: U32) -> List<&2, R.T8>:
  match ks:
    case Nil{}:
      Nil{}
    case Con{k, t}:
      Bool.pick(List<&2, R.T8>, U32.is_eq(Seat.id(k), id), t, Con{k, seat_drop(t, id)})

# Seat `k` sits down in its place, by id.
def seat_insert(+ks: List<&2, R.T8>, +k: R.T8) -> List<&2, R.T8>:
  match ks:
    case Nil{}:
      Con{k, Nil{}}
    case Con{h, t}:
      Bool.pick(List<&2, R.T8>, U32.is_lt(Seat.id(k), Seat.id(h)), Con{k, Con{h, t}}, Con{h, seat_insert(t, k)})

# Take seat `id` from the table into `cur`: `ret` says whether it was there.
def take(+w: S, +id: U32) -> S:
  +here = seat_has(W.seats(w), id)
  Bool.pick(S, here, W.set_ret(W.set_cur(W.set_seats(w, seat_drop(W.seats(w), id)), seat_get(W.seats(w), id)), 1), W.set_ret(w, 0))

# `cur` sits back down.
def put(+w: S) -> S: W.set_seats(w, seat_insert(W.seats(w), W.cur(w)))

# ----- a blow on a knight ---------------------------------------------------------

# The last quarter heart: the lock lets go, the revive starts over, a carried pot
# is dropped, and the fall is heard.
def hurt_down(+down: Bool, +w: S) -> S:
  match down:
    case False{}:
      w
    case True{}:
      +w1 = K.set_lock_i(K.set_lock_k(K.set_rev(w, 0), 0), 0)
      Q.sound(K.set_carry(w1, Bool.pick(U32, U32.is_eq(K.carry(w1), 1), 0, K.carry(w1))), 22)

# The knight is hurt: quarter hearts gone, a blink of safety, a shove away, and
# if that was the last, down.
def hurt_knight(+w: S, +dmg: U32, +fromx: U32, +fromy: U32) -> S:
  +kx = K.x(w)
  +ky = K.y(w)
  +hp = M.sat(K.hp(w), dmg)
  +down = U32.is_eq(hp, 0)
  +away = G.unit(U32.sub(kx, fromx), U32.sub(ky, fromy))
  +w1 = K.set_hp(w, hp)
  +w2 = K.set_inv(K.set_ward(w1, C.hurt_inv()), C.hurt_inv())
  +w3 = K.set_act(w2, Bool.pick(U32, down, C.a_down(), C.a_hurt()))
  +w4 = K.set_charge(K.set_step(K.set_chain(K.set_t(w3, 0), 0), 0), 0)
  +w5 = K.set_z(K.set_queued(K.set_lamp(K.set_shield(w4, 0), 0), 0), 0)
  +w6 = K.set_pushy(K.set_pushx(w5, N.sdiv(U32.mul(G.vx(away), C.hurt_push()), 256)), N.sdiv(U32.mul(G.vy(away), C.hurt_push()), 256))
  +w7 = K.set_pusht(w6, 6)
  +w8 = Q.fx(w7, C.fxk_spark(), kx, U32.sub(ky, 1536), 0)
  +w9 = W.set_shake(w8, U32.max(W.shake(w8), C.shake_hit()))
  +w10 = Q.sound(w9, Bool.pick(U32, U32.is_ge(dmg, 4), 24, 23))
  hurt_down(down, w10)

# A shield up and square to the blow parries it in its first ticks, else blocks.
def guard_parry(+w: S, +kx: U32, +ky: U32) -> S:
  +atx = U32.add(kx, U32.mul(K.fx(w), 9))
  +aty = U32.add(ky, U32.mul(K.fy(w), 9))
  +w1 = K.set_freeze(w, U32.max(K.freeze(w), C.parry_freeze()))
  +w2 = Q.fx(w1, C.fxk_parry(), atx, aty, 0)
  +w3 = W.set_flash(w2, U32.max(W.flash(w2), C.parry_flash()))
  Q.stop(Q.sound(w3, 20), C.stop_phase())

def guard_block(+w: S, +kx: U32, +ky: U32, +tx: U32, +ty: U32) -> S:
  +atx = U32.add(kx, U32.mul(K.fx(w), 9))
  +aty = U32.add(ky, U32.mul(K.fy(w), 9))
  +w1 = K.set_pushy(K.set_pushx(w, N.sdiv(U32.mul(N.sneg(tx), C.block_push()), 256)), N.sdiv(U32.mul(N.sneg(ty), C.block_push()), 256))
  Q.sound(Q.fx(K.set_pusht(w1, 4), C.fxk_ring(), atx, aty, 10), 30)

def blow_guarded(+parried: Bool, +w: S, +kx: U32, +ky: U32, +tx: U32, +ty: U32) -> S:
  match parried:
    case True{}:
      W.set_ret(put(guard_parry(w, kx, ky)), C.r_parried())
    case False{}:
      W.set_ret(put(guard_block(w, kx, ky, tx, ty)), C.r_blocked())

def blow_open(+guarded: Bool, +w: S, +dmg: U32, +fromx: U32, +fromy: U32, +kx: U32, +ky: U32, +tx: U32, +ty: U32) -> S:
  match guarded:
    case True{}:
      blow_guarded(U32.is_le(K.shield(w), W.r_parry(w)), w, kx, ky, tx, ty)
    case False{}:
      W.set_ret(put(hurt_knight(w, dmg, fromx, fromy)), C.r_hit())

def blow_live(+dodged: Bool, +w: S, +dmg: U32, +fromx: U32, +fromy: U32) -> S:
  match dodged:
    case True{}:
      W.set_ret(put(w), C.r_dodged())
    case False{}:
      +kx = K.x(w)
      +ky = K.y(w)
      +t = G.unit(U32.sub(fromx, kx), U32.sub(fromy, ky))
      +guarded = U32.is_ne(K.shield(w), 0) && U32.is_eq(K.act(w), 0) && N.sge(G.dot(K.fx(w), K.fy(w), G.vx(t), G.vy(t)), C.shield_cos())
      blow_open(guarded, w, dmg, fromx, fromy, kx, ky, G.vx(t), G.vy(t))

def blow_there(+found: Bool, +w: S, +dmg: U32, +fromx: U32, +fromy: U32, +kind: U32) -> S:
  match found:
    case False{}:
      W.set_ret(w, C.r_dodged())
    case True{}:
      +k = W.cur(w)
      +dodged = Bool.not(Q.up(k)) || U32.is_ne(Seat.inv(k), 0) || U32.is_eq(Seat.act(k), C.a_item()) || U32.is_eq(Seat.act(k), C.a_fall()) || (U32.is_eq(kind, C.h_low()) && N.sge(Seat.z(k), U32.mul(C.low_clear(), 256)))
      blow_live(dodged, w, dmg, fromx, fromy)

# A blow lands on knight `id`, from a point, for so many quarter hearts. `ret` is
# what came of it for the knight.
def blow(+w: S, +id: U32, +dmg: U32, +fromx: U32, +fromy: U32, +kind: U32) -> S:
  +w1 = take(w, id)
  blow_there(U32.is_ne(W.ret(w1), 0), w1, dmg, fromx, fromy, kind)

# ----- a blow on a foe ------------------------------------------------------------

def foe_hit_end(+dead: Bool, +w: S, +x: U32, +y: U32) -> S:
  match dead:
    case True{}:
      Q.stop(Q.sound(Q.scatter(Q.fx(Q.fx(w, C.fxk_puff(), x, y, 0), C.fxk_light(), x, y, 0), C.pk_fissure(), x, y), 21), C.stop_kill())
    case False{}:
      Q.sound(Q.fx(w, C.fxk_spark(), x, y, 0), 18)

# The blow lands: a second blow doubles on a foe that was left open, it flinches
# and is held a moment, is pushed away, wakes if it slept, and falls if that was
# the last of it.
def foe_hit(+w: S, +i: U32, +f: R.T8, +dmg: U32, +fromx: U32, +fromy: U32) -> S:
  +x = Foe.x(f)
  +y = Foe.y(f)
  +dmg2 = Bool.pick(U32, U32.is_ne(Foe.open(f), 0), U32.mul(dmg, 2), dmg)
  +hp = M.sat(Foe.hp(f), dmg2)
  +away = G.unit(U32.sub(x, fromx), U32.sub(y, fromy))
  +dormant = U32.is_eq(Foe.st(f), C.fs_dormant())
  +dead = U32.is_eq(hp, 0)
  +f1 = Foe.set_hp(Foe.set_open(f, 0), hp)
  +f2 = Foe.set_freeze(Foe.set_flinch(f1, C.flinch_ticks()), C.hit_freeze())
  +f3 = Foe.set_kbt(Foe.set_kby(Foe.set_kbx(f2, N.sdiv(U32.mul(G.vx(away), C.foe_push()), 256)), N.sdiv(U32.mul(G.vy(away), C.foe_push()), 256)), 5)
  +f4 = Foe.set_t(Foe.set_st(f3, Bool.pick(U32, dormant, C.fs_drift(), Foe.st(f3))), Bool.pick(U32, dormant, 20, Foe.t(f3)))
  +f5 = Foe.set_kbt(Foe.set_dk(Foe.set_dead(Foe.set_st(f4, Bool.pick(U32, dead, C.fs_dying(), Foe.st(f4))), Bool.pick(U32, dead, 0, Foe.dead(f4))), Bool.pick(U32, dead, 0, Foe.dk(f4))), Bool.pick(U32, dead, 0, Foe.kbt(f4)))
  W.set_ret(foe_hit_end(dead, W.set_foes(w, L.set64(W.foes(w), U32.to_nat(i), f5)), x, y), 1)

def foe_hooded(+w: S, +x: U32, +y: U32) -> S: W.set_ret(Q.fx(Q.sound(w, 38), C.fxk_ring(), x, y, 8), 0)

def foe_iron(+w: S, +x: U32, +y: U32) -> S: W.set_ret(Q.fx(Q.sound(w, 10), C.fxk_spark(), x, y, 0), 0)

def hurt_foe3(+shut: Bool, +w: S, +i: U32, +f: R.T8, +dmg: U32, +fromx: U32, +fromy: U32) -> S:
  match shut:
    case True{}:
      foe_hooded(w, Foe.x(f), Foe.y(f))
    case False{}:
      foe_hit(w, i, f, dmg, fromx, fromy)

def hurt_foe2(+turret: Bool, +w: S, +i: U32, +f: R.T8, +dmg: U32, +fromx: U32, +fromy: U32) -> S:
  match turret:
    case True{}:
      foe_iron(w, Foe.x(f), Foe.y(f))
    case False{}:
      hurt_foe3(U32.is_eq(Foe.kind(f), C.fk_snuffer()) && U32.is_eq(Foe.hooded(f), 1) && U32.is_eq(Foe.lit(f), 0), w, i, f, dmg, fromx, fromy)

def hurt_foe1(+alive: Bool, +w: S, +i: U32, +f: R.T8, +dmg: U32, +fromx: U32, +fromy: U32) -> S:
  match alive:
    case False{}:
      W.set_ret(w, 0)
    case True{}:
      hurt_foe2(U32.is_eq(Foe.kind(f), C.fk_turret()), w, i, f, dmg, fromx, fromy)

# A blade, a bolt or a blast on foe `i`. `ret` is 1 when it hurt (a blade passes
# through a hooded snuffer; nothing hurts the turret).
def hurt_foe(+w: S, +i: U32, +dmg: U32, +fromx: U32, +fromy: U32) -> S:
  +f = L.get64(W.foes(w), U32.to_nat(i))
  hurt_foe1(Q.foe_alive(f), w, i, f, dmg, fromx, fromy)

def light_foe_do(+w: S, +i: U32, +f: R.T8) -> S:
  +dormant = U32.is_eq(Foe.st(f), C.fs_dormant())
  +f1 = Foe.set_flinch(Foe.set_lit(f, U32.mul(C.lit_hold(), 2)), C.flinch_ticks())
  +f2 = Foe.set_t(Foe.set_st(f1, Bool.pick(U32, dormant, C.fs_drift(), Foe.st(f1))), Bool.pick(U32, dormant, 20, Foe.t(f1)))
  Q.sound(Q.fx(W.set_foes(w, L.set64(W.foes(w), U32.to_nat(i), f2)), C.fxk_light(), Foe.x(f), Foe.y(f), 0), 19)

def light_foe_if(+go: Bool, +w: S, +i: U32, +f: R.T8) -> S:
  match go:
    case True{}:
      light_foe_do(w, i, f)
    case False{}:
      w

# Light on foe `i`: a snuffer's hood burns off for a while, and it takes a little.
def light_foe(+w: S, +i: U32) -> S:
  +f = L.get64(W.foes(w), U32.to_nat(i))
  light_foe_if(U32.is_eq(Foe.kind(f), C.fk_snuffer()) && Q.foe_alive(f), w, i, f)

# ----- the Warden's hurts -----------------------------------------------------------

def lit_in(+l: U32, +now: U32) -> U32: Bool.to_u32(U32.is_ne(l, 0) && U32.is_gt(U32.add(l, C.expose_ticks()), now))

# How many knights' light the shutter wants.
def need(+w: S) -> U32: Bool.pick(U32, U32.is_ge(B.phase(w), 1), U32.min(L.len64(W.seats(w)), 2), 1)

# Whether the shutter is open.
def exposed(+w: S) -> Bool:
  +now = W.elapsed(w)
  +n = U32.add(U32.add(U32.add(lit_in(B.l0(w), now), lit_in(B.l1(w), now)), U32.add(lit_in(B.l2(w), now), lit_in(B.l3(w), now))), U32.add(lit_in(B.l4(w), now), lit_in(B.l5(w), now)))
  U32.is_ne(W.boss_on(w), 0) && (U32.is_eq(B.st(w), C.ws_stagger()) || U32.is_ge(n, need(w)))

# Light fell on the lamp-heart from seat `owner`.
def light_heart(+w: S, +owner: U32) -> S:
  +now = W.elapsed(w)
  Bool.pick(S, U32.is_ne(W.boss_on(w), 0) && U32.is_ge(owner, 1) && U32.is_le(owner, C.max_players()),
    Bool.pick(S, U32.is_eq(owner, 1), B.set_l0(w, now),
    Bool.pick(S, U32.is_eq(owner, 2), B.set_l1(w, now),
    Bool.pick(S, U32.is_eq(owner, 3), B.set_l2(w, now),
    Bool.pick(S, U32.is_eq(owner, 4), B.set_l3(w, now),
    Bool.pick(S, U32.is_eq(owner, 5), B.set_l4(w, now), B.set_l5(w, now)))))),
    w)

# Its summons go out with it.
def foes_out(+fs: List<&2, R.T8>) -> List<&2, R.T8>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{f, t}:
      Con{Bool.pick(R.T8, Q.foe_alive(f), Foe.set_dead(Foe.set_st(Foe.set_hp(f, 0), C.fs_dying()), 0), f), foes_out(t)}

# The Gloam falls: every lamp in the arena is snuffed.
def lamps_out(+ps: List<&2, R.W8>) -> List<&2, R.W8>:
  match ps:
    case Nil{}:
      Nil{}
    case Con{p, t}:
      Con{Bool.pick(R.W8, U32.is_eq(Pr.kind(p), C.pk_brazier()), Pr.set_t(Pr.set_on(p, 0), 0), p), lamps_out(t)}

def boss_falls(+w: S) -> S:
  +x = B.x(w)
  +y = B.y(w)
  +w1 = B.set_dk(B.set_dead(B.set_st(w, C.ws_dying()), 0), 0)
  +w2 = Q.fx(Q.stop(Q.sound(w1, 3), C.stop_boss()), C.fxk_puff(), x, y, 0)
  W.set_ret(W.set_foes(W.set_shake(w2, U32.mul(C.shake_heavy(), 2)), foes_out(W.foes(w2))), 1)

def boss_phase(+w: S, +phase: U32) -> S:
  +w1 = B.set_dk(B.set_t(B.set_st(B.set_start(B.set_phase(w, phase), B.hp(w)), C.ws_recover()), 48), 0)
  +w2 = B.set_l5(B.set_l4(B.set_l3(B.set_l2(B.set_l1(B.set_l0(w1, 0), 0), 0), 0), 0), 0)
  +w3 = Q.sound(w2, 5)
  +w4 = Bool.pick(S, U32.is_eq(phase, 2), Q.sound(W.set_props(w3, lamps_out(W.props(w3))), 38), w3)
  W.set_flash(W.set_shake(Q.stop(w4, C.stop_phase()), C.shake_heavy()), U32.max(W.flash(w4), 6))

def boss_wound(+more: Bool, +w: S, +phase: U32) -> S:
  match more:
    case True{}:
      boss_phase(w, phase)
    case False{}:
      Q.sound(w, 17)

# What a hurt that did not end it leaves: a spark at the heart.
def boss_hurt_end(+w: S) -> S: W.set_ret(Q.fx(w, C.fxk_spark(), Q.heart_x(w), Q.heart_y(w), 0), 1)

def boss_hit(+dead: Bool, +w: S, +phase: U32, +before: U32) -> S:
  match dead:
    case True{}:
      boss_falls(w)
    case False{}:
      boss_hurt_end(boss_wound(U32.is_gt(phase, before), w, phase))

def boss_hurt(+w: S, +dmg: U32) -> S:
  +dmg2 = Bool.pick(U32, U32.is_eq(B.st(w), C.ws_stagger()), U32.mul(dmg, 2), dmg)
  +hp = M.sat(B.hp(w), dmg2)
  +third = U32.div(B.full(w), 3)
  +phase = Bool.pick(U32, U32.is_le(hp, third), 2, Bool.pick(U32, U32.is_le(hp, U32.mul(third, 2)), 1, 0))
  +w1 = B.set_flinch(B.set_hp(w, hp), C.flinch_ticks())
  boss_hit(U32.is_eq(hp, 0), w1, phase, B.phase(w))

def boss_alive(+alive: Bool, +w: S, +dmg: U32) -> S:
  match alive:
    case True{}:
      boss_hurt(w, dmg)
    case False{}:
      W.set_ret(w, 0)

# The shutter is shut: the blow rings off.
def boss_shut(+w: S, +fromx: U32, +fromy: U32) -> S:
  +bx = B.x(w)
  +by = B.y(w)
  W.set_ret(Q.sound(Q.fx(w, C.fxk_ring(), U32.add(bx, N.sdiv(U32.sub(fromx, bx), 6)), U32.add(by, N.sdiv(U32.sub(fromy, by), 6)), 12), 10), 0)

def boss_open_to(+open: Bool, +w: S, +dmg: U32, +fromx: U32, +fromy: U32) -> S:
  match open:
    case True{}:
      boss_alive(U32.is_ne(B.hp(w), 0) && U32.is_ne(B.st(w), C.ws_dying()), w, dmg)
    case False{}:
      boss_shut(w, fromx, fromy)

# A blade or a pot on the Warden. Only the open heart can be hurt. `ret` is 1
# when it hurt.
def hurt_boss(+w: S, +dmg: U32, +fromx: U32, +fromy: U32) -> S: boss_open_to(exposed(w), w, dmg, fromx, fromy)

def blast_if(+near: Bool, +w: S, +fromx: U32, +fromy: U32) -> S:
  match near:
    case True{}:
      hurt_boss(w, C.blast_damage(), fromx, fromy)
    case False{}:
      w

# A bomb's blast near the Warden: it counts only on an open heart.
def blast_boss(+w: S, +fromx: U32, +fromy: U32) -> S:
  blast_if(U32.is_ne(W.boss_on(w), 0) && U32.is_le(G.dist(U32.sub(B.x(w), fromx), U32.sub(B.y(w), fromy)), U32.add(U32.mul(C.blast_r(), 256), U32.mul(C.warden_r(), 256))), w, fromx, fromy)

# ----- bolts ------------------------------------------------------------------------

# A bolt is done: a pot breaks, light flickers, iron goes out.
def bolt_end_with(+w: S, +kind: U32, +x: U32, +y: U32) -> S:
  Z.lz(U32.is_eq(kind, C.bk_pot()),
    u => Q.sound(Q.scatter(Q.fx(w, C.fxk_dust(), x, y, 0), C.pk_pot(), x, y), 28),
    u => Q.fx(w, Bool.pick(U32, U32.is_eq(kind, C.bk_light()) || U32.is_eq(kind, C.bk_bright()), C.fxk_spark(), C.fxk_dust()), x, y, 0))

def bolt_ends(+w: S, +i: U32) -> S:
  +b = L.get8(W.bolts(w), U32.to_nat(i))
  bolt_end_with(W.set_bolts(w, L.set8(W.bolts(w), U32.to_nat(i), Bo.set_ttl(b, 0))), Bo.kind(b), Bo.x(b), Bo.y(b))

# A shield turns an iron bolt: back where the knight's lock holds, else where the
# shield faces. A parry sends it bright and quick; a late block sends it back dim
# and short.
def turn_bolt(+w: S, +i: U32, +id: U32, +parried: Bool) -> S:
  +k = seat_get(W.seats(w), id)
  +kx = Seat.x(k)
  +ky = Seat.y(k)
  +lp = Q.lock_point(w, Seat.lock_k(k), Seat.lock_i(k))
  +u = G.unit(U32.sub(Nr.x(lp), kx), U32.sub(Nr.y(lp), ky))
  +use = U32.is_eq(Nr.ok(lp), 1) && N.sge(G.dot(G.vx(u), G.vy(u), Seat.fx(k), Seat.fy(k)), C.reflect_cos())
  +ax = Bool.pick(U32, use, G.vx(u), Seat.fx(k))
  +ay = Bool.pick(U32, use, G.vy(u), Seat.fy(k))
  +speed = Bool.pick(U32, parried, 930, C.bolt_speed())
  +b = L.get8(W.bolts(w), U32.to_nat(i))
  +b1 = Bo.set_y(Bo.set_x(Bo.set_owner(Bo.set_kind(b, Bool.pick(U32, parried, C.bk_bright(), C.bk_dim())), id), U32.add(kx, U32.mul(Seat.fx(k), 9))), U32.add(ky, U32.mul(Seat.fy(k), 9)))
  +b2 = Bo.set_ttl(Bo.set_vy(Bo.set_vx(b1, N.sdiv(U32.mul(ax, speed), 256)), N.sdiv(U32.mul(ay, speed), 256)), Bool.pick(U32, parried, C.bolt_life(), C.dim_life()))
  W.set_bolts(w, L.set8(W.bolts(w), U32.to_nat(i), b2))

# What knight `id` makes of an iron bolt at (px, py): `ret` is the blow's result,
# or dodged when the knight is not there for it.
def iron_blow(+near: Bool, +w: S, +id: U32, +px: U32, +py: U32, +vx: U32, +vy: U32) -> S:
  match near:
    case True{}:
      blow(w, id, C.bolt_damage(), U32.sub(px, vx), U32.sub(py, vy), C.h_bolt())
    case False{}:
      W.set_ret(w, C.r_dodged())

def iron_act(+w: S, +i: U32, +id: U32) -> S:
  +res = W.ret(w)
  Z.lz(U32.is_eq(res, C.r_dodged()), u => w,
    u => Z.lz(U32.is_eq(res, C.r_hit()),
      u => W.set_bolts(w, L.set8(W.bolts(w), U32.to_nat(i), Bo.set_ttl(L.get8(W.bolts(w), U32.to_nat(i)), 0))),
      u => turn_bolt(w, i, id, U32.is_eq(res, C.r_parried()))))

# Each knight at the table in turn, until one is struck or turns the bolt.
def iron_go(+ids: List<&2, U32>, +w: S, +i: U32, +px: U32, +py: U32, +vx: U32, +vy: U32, +done: Bool) -> S:
  match ids:
    case Nil{}:
      w
    case Con{id, t}:
      match done:
        case True{}:
          w
        case False{}:
          +k = seat_get(W.seats(w), id)
          +w1 = iron_blow(Q.up(k) && U32.is_le(G.dist(U32.sub(Seat.x(k), px), U32.sub(Seat.y(k), py)), 2048), w, id, px, py, vx, vy)
          +hit = U32.is_ne(W.ret(w1), C.r_dodged())
          iron_go(t, Z.lw(hit, u => iron_act(w1, i, id), w1), i, px, py, vx, vy, hit)

def seat_ids(+ks: List<&2, R.T8>) -> List<&2, U32>:
  match ks:
    case Nil{}:
      Nil{}
    case Con{k, t}:
      Con{Seat.id(k), seat_ids(t)}

# The first foe a bolt of light or a pot has met: its index.
def foe_near(+fs: List<&2, R.T8>, +px: U32, +py: U32, +j: U32, +found: U32, +done: Bool) -> U32:
  match fs:
    case Nil{}:
      found
    case Con{f, t}:
      match done:
        case True{}:
          found
        case False{}:
          +hit = Q.foe_alive(f) && U32.is_le(G.dist(U32.sub(Foe.x(f), px), U32.sub(Foe.y(f), py)), U32.add(Bool.pick(U32, U32.is_eq(Foe.kind(f), C.fk_turret()), 2048, 1536), 512))
          foe_near(t, px, py, U32.add(j, 1), Bool.pick(U32, hit, j, found), hit)

# A bright bolt stuns a turret open; on anything else it burns.
def bright_foe(+w: S, +j: U32, +fromx: U32, +fromy: U32) -> S:
  +f = L.get64(W.foes(w), U32.to_nat(j))
  Z.lz(U32.is_eq(Foe.kind(f), C.fk_turret()),
    u => Q.sound(Q.fx(W.set_foes(w, L.set64(W.foes(w), U32.to_nat(j), Foe.set_open(Foe.set_dk(Foe.set_t(Foe.set_st(f, C.fs_stagger()), C.turret_stun()), 0), 1))), C.fxk_parry(), Foe.x(f), Foe.y(f), 0), 10),
    u => hurt_foe(w, j, C.bright_damage(), fromx, fromy))

def bolt_foe(+w: S, +i: U32, +j: U32, +kind: U32, +fromx: U32, +fromy: U32) -> S:
  bolt_ends(Z.lz(U32.is_eq(kind, C.bk_light()), u => light_foe(w, j),
    u => Z.lz(U32.is_eq(kind, C.bk_bright()), u => bright_foe(w, j, fromx, fromy),
    u => Z.lz(U32.is_eq(kind, C.bk_dim()), u => hurt_foe(w, j, C.dim_damage(), fromx, fromy), u => hurt_foe(w, j, C.pot_damage(), fromx, fromy)))), i)

# A bolt that found the Warden: light on the heart lights it, a pot hurts it.
def bolt_boss(+near_heart: Bool, +w: S, +i: U32, +kind: U32, +owner: U32, +fromx: U32, +fromy: U32) -> S:
  bolt_ends(Z.lz((U32.is_eq(kind, C.bk_light()) || U32.is_eq(kind, C.bk_bright())) && near_heart, u => light_heart(w, owner),
    u => Z.lw(U32.is_eq(kind, C.bk_pot()), u => hurt_boss(w, C.pot_damage(), fromx, fromy), w)), i)

def bolt_lights2(+hit: Bool, +w: S, +i: U32, +kind: U32, +owner: U32, +fromx: U32, +fromy: U32, +near_heart: Bool) -> S:
  match hit:
    case True{}:
      bolt_boss(near_heart, w, i, kind, owner, fromx, fromy)
    case False{}:
      w

def bolt_lights1(+up: Bool, +w: S, +i: U32, +px: U32, +py: U32, +kind: U32, +owner: U32, +fromx: U32, +fromy: U32) -> S:
  match up:
    case False{}:
      w
    case True{}:
      +near_heart = U32.is_le(G.dist(U32.sub(Q.heart_x(w), px), U32.sub(Q.heart_y(w), py)), U32.mul(C.heart_r(), 256))
      +near_body = U32.is_le(G.dist(U32.sub(B.x(w), px), U32.sub(B.y(w), py)), U32.mul(C.warden_r(), 256))
      bolt_lights2(near_heart || near_body, w, i, kind, owner, fromx, fromy, near_heart)

def bolt_lights(+found: U32, +w: S, +i: U32, +px: U32, +py: U32, +kind: U32, +owner: U32, +fromx: U32, +fromy: U32) -> S:
  Z.lz(U32.is_ne(found, C.none()), u => bolt_foe(w, i, found, kind, fromx, fromy), u => bolt_lights1(Q.boss_up(w), w, i, px, py, kind, owner, fromx, fromy))

# What a thing in the bolt's way does to it.
def bolt_prop(+w: S, +i: U32, +j: U32, +kind: U32) -> S:
  +q = L.get8(W.props(w), U32.to_nat(j))
  +kq = Pr.kind(q)
  +lightish = U32.is_eq(kind, C.bk_light()) || U32.is_eq(kind, C.bk_bright())
  +woken = W.set_props(w, L.set8(W.props(w), U32.to_nat(j), Pr.set_on(q, 1)))
  bolt_ends(Z.lz(lightish && U32.is_eq(kq, C.pk_crystal()) && U32.is_eq(Pr.on(q), 0), u => Q.light_crystal(w, j),
    u => Z.lz(lightish && U32.is_eq(kq, C.pk_brazier()) && Bool.not(Q.lit(q)), u => Q.sound(Q.fx(woken, C.fxk_light(), Pr.x(q), Pr.y(q), 0), 36),
    u => Z.lw(lightish && U32.is_eq(kq, C.pk_great()) && U32.is_eq(Pr.on(q), 0), u => Q.solved(woken, Pr.x(q), Pr.y(q)), w))), i)

# The index of the first thing that stops a bolt at the point.
def first_stop(+ps: List<&2, R.W8>, +x: U32, +y: U32, +tx: U32, +ty: U32, +j: U32, +found: U32, +done: Bool) -> U32:
  match ps:
    case Nil{}:
      found
    case Con{p, t}:
      match done:
        case True{}:
          found
        case False{}:
          +hit = Q.blocks(p, x, y, tx, ty) && Bool.not(Q.lets_bolts(p))
          first_stop(t, x, y, tx, ty, U32.add(j, 1), Bool.pick(U32, hit, j, found), hit)

def bolt_open(+w: S, +i: U32, +px: U32, +py: U32, +kind: U32, +owner: U32, +vx: U32, +vy: U32) -> S:
  Z.lz(U32.is_eq(kind, C.bk_iron()),
    u => iron_go(seat_ids(W.seats(w)), w, i, px, py, vx, vy, False{}),
    u => bolt_lights(foe_near(W.foes(w), px, py, 0, C.none(), False{}), w, i, px, py, kind, owner, U32.sub(px, vx), U32.sub(py, vy)))

def bolt_clear(+stopped: Bool, +j: U32, +w: S, +i: U32, +px: U32, +py: U32, +kind: U32, +owner: U32, +vx: U32, +vy: U32) -> S:
  match stopped:
    case True{}:
      bolt_prop(w, i, j, kind)
    case False{}:
      bolt_open(w, i, px, py, kind, owner, vx, vy)

def bolt_free(+wall: Bool, +w: S, +i: U32, +px: U32, +py: U32, +kind: U32, +owner: U32, +vx: U32, +vy: U32) -> S:
  match wall:
    case True{}:
      bolt_ends(w, i)
    case False{}:
      +j = first_stop(W.props(w), px, py, G.tile_of(px), G.tile_of(py), 0, C.none(), False{})
      bolt_clear(U32.is_ne(j, C.none()), j, w, i, px, py, kind, owner, vx, vy)

# One half step's meeting of bolt `i` with whatever is where it is now.
def bolt_meets(+w: S, +i: U32) -> S:
  +b = L.get8(W.bolts(w), U32.to_nat(i))
  bolt_free(U32.is_eq(Q.cls(w, G.tile_of(Bo.x(b)), G.tile_of(Bo.y(b))), 1), w, i, Bo.x(b), Bo.y(b), Bo.kind(b), Bo.owner(b), Bo.vx(b), Bo.vy(b))

# Fast bolts go in two half steps so none skips a body.
def bolt_half(+w: S, +i: U32) -> S:
  +b = L.get8(W.bolts(w), U32.to_nat(i))
  Z.lz(U32.is_eq(Bo.ttl(b), 0), u => w,
    u => bolt_meets(W.set_bolts(w, L.set8(W.bolts(w), U32.to_nat(i), Bo.set_y(Bo.set_x(b, U32.add(Bo.x(b), N.sdiv(Bo.vx(b), 2))), U32.add(Bo.y(b), N.sdiv(Bo.vy(b), 2))))), i))

def bolt_one(+live: Bool, +w: S, +i: U32) -> S:
  match live:
    case False{}:
      w
    case True{}:
      +w2 = bolt_half(bolt_half(w, i), i)
      +b = L.get8(W.bolts(w2), U32.to_nat(i))
      W.set_bolts(w2, L.set8(W.bolts(w2), U32.to_nat(i), Bo.set_ttl(b, M.dec(Bo.ttl(b)))))

def bolts_go(+n: Nat, +w: S, +i: U32) -> S:
  match n:
    case 0n:
      w
    case 1n+p:
      bolts_go(p, bolt_one(U32.is_ne(Bo.ttl(L.get8(W.bolts(w), U32.to_nat(i))), 0), w, i), U32.add(i, 1))

# Fly every bolt one tick.
def step_bolts(+w: S) -> S:
  +w1 = bolts_go(U32.to_nat(L.len8(W.bolts(w))), w, 0)
  W.set_bolts(w1, L.keep8(~(b => U32.is_ne(Bo.ttl(b), 0)), W.bolts(w1)))

# ----- bombs ------------------------------------------------------------------------

# A blast on foe `j`: a turret is stunned, anything else is lit and hurt.
def blast_foe(+w: S, +j: U32, +bx: U32, +by: U32) -> S:
  +f = L.get64(W.foes(w), U32.to_nat(j))
  +near = Q.foe_alive(f) && U32.is_le(G.dist(U32.sub(Foe.x(f), bx), U32.sub(Foe.y(f), by)), U32.mul(C.blast_r(), 256))
  Z.lw(near, u => Z.lz(U32.is_eq(Foe.kind(f), C.fk_turret()),
      u => W.set_foes(w, L.set64(W.foes(w), U32.to_nat(j), Foe.set_dk(Foe.set_t(Foe.set_st(f, C.fs_stagger()), C.turret_stun()), 0))),
      u => hurt_foe(W.set_foes(w, L.set64(W.foes(w), U32.to_nat(j), Foe.set_lit(f, C.lit_hold()))), j, C.blast_damage(), bx, by)), w)

def blast_foes(+n: Nat, +w: S, +j: U32, +bx: U32, +by: U32) -> S:
  match n:
    case 0n:
      w
    case 1n+p:
      blast_foes(p, blast_foe(w, j, bx, by), U32.add(j, 1), bx, by)

def blast_prop(+w: S, +j: U32, +bx: U32, +by: U32) -> S:
  +p = L.get8(W.props(w), U32.to_nat(j))
  +x = Pr.x(p)
  +y = Pr.y(p)
  +kind = Pr.kind(p)
  +near = U32.is_le(G.dist(U32.sub(x, bx), U32.sub(y, by)), U32.mul(C.blast_r(), 256))
  +cracked = near && U32.is_eq(kind, C.pk_cracked()) && U32.is_eq(Pr.on(p), 0)
  +breaks = near && (U32.is_eq(kind, C.pk_pot()) || U32.is_eq(kind, C.pk_grass())) && U32.is_eq(Pr.on(p), 0)
  Z.lz(cracked, u => Q.sound(Q.solved(Q.fx(W.set_props(w, L.set8(W.props(w), U32.to_nat(j), Pr.set_on(p, 1))), C.fxk_dust(), x, y, 0), x, y), 11),
    u => Z.lw(breaks, u => Q.shatter(w, j), w))

def blast_props(+n: Nat, +w: S, +j: U32, +bx: U32, +by: U32) -> S:
  match n:
    case 0n:
      w
    case 1n+p:
      blast_props(p, blast_prop(w, j, bx, by), U32.add(j, 1), bx, by)

# One bomb goes off: the blast, the shake, and what it opens, breaks and hurts.
def burst(+w: S, +bx: U32, +by: U32) -> S:
  +w1 = Q.stop(Q.sound(W.set_shake(Q.fx(w, C.fxk_blast(), bx, by, 0), U32.max(W.shake(w), C.shake_heavy())), 1), C.stop_heavy())
  +w2 = blast_foes(U32.to_nat(L.len64(W.foes(w1))), w1, 0, bx, by)
  +w3 = blast_props(U32.to_nat(L.len8(W.props(w2))), w2, 0, bx, by)
  Z.lw(U32.is_ne(W.boss_on(w3), 0), u => blast_boss(w3, bx, by), w3)

def bursts(+bs: List<&2, R.W8>, +w: S) -> S:
  match bs:
    case Nil{}:
      w
    case Con{f, t}:
      bursts(t, burst(w, Fu.x(f), Fu.y(f)))

def fuses_tick(+fs: List<&2, R.W8>) -> List<&2, R.W8>:
  match fs:
    case Nil{}:
      Nil{}
    case Con{f, t}:
      Con{Fu.set_t(f, M.dec(Fu.t(f))), fuses_tick(t)}

# Bombs: tick the fuses; a burst opens cracked walls, breaks pots and grass and
# hurts the Gloam, never the knights.
def step_fuses(+w: S) -> S:
  +ticked = fuses_tick(W.fuses(w))
  +w1 = W.set_fuses(w, L.keep8(~(f => U32.is_ne(Fu.t(f), 0)), ticked))
  bursts(L.keep8(~(f => U32.is_eq(Fu.t(f), 0)), ticked), w1)
