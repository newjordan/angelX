# The knight: a body, a lock-on, a sword, a shield, a lantern and a hop. Every
# seat has all of it. The verbs are the classic lock-on adventure's, changed only
# for the camera: the view is from above, so a hop is a hop along the ground and
# a jump attack lands where the knight comes down. crates/delve/src/rules/
# lanternward/kit.rs. The knight acting is `cur`; the other seats are the table.
#
# Buttons (the Delve's own, nothing new): WASD walk, the arrows face, R holds the
# lock, Space holds the shield, Q is the sword, E is the one context button, F is
# the lantern, X lays a bomb. A seat's input is its Ix: id, move, aim, buttons.
import Base
import ./r.bend as R
import ./c.bend as C
import ./n.bend as N
import ./m.bend as M
import ./l.bend as L
import ./g.bend as G
import ./gd.bend as Gd
import ./w.bend as W
import ./pr.bend as Pr
import ./bo.bend as Bo
import ./fu.bend as Fu
import ./ix.bend as Ix
import ./tg.bend as Tg
import ./nr.bend as Nr
import ./ct.bend as Ct
import ./sh.bend as D
import ./se.bend as Seat
import ./k.bend as K
import ./fe.bend as Foe
import ./b.bend as B
import ./z.bend as Z
import ./q.bend as Q
import ./ht.bend as H
import ./cn.bend as X

def S() -> Data: R.W

# ----- the lantern's flame ---------------------------------------------------------------

# Whether a resting brazier is near, or a friend's flare to kindle by.
def rest_near(+ps: List<&2, R.W8>, +x: U32, +y: U32, +hit: Bool) -> Bool:
  match ps:
    case Nil{}:
      hit
    case Con{p, t}:
      match hit:
        case True{}:
          True{}
        case False{}:
          rest_near(t, x, y, U32.is_eq(Pr.kind(p), C.pk_rest()) && N.slt(G.dist(U32.sub(Pr.x(p), x), U32.sub(Pr.y(p), y)), U32.mul(C.brazier_r(), 256)))

def kindled(+ks: List<&2, R.T8>, +id: U32, +x: U32, +y: U32, +hit: Bool) -> Bool:
  match ks:
    case Nil{}:
      hit
    case Con{o, t}:
      match hit:
        case True{}:
          True{}
        case False{}:
          kindled(t, id, x, y, U32.is_ne(Seat.id(o), id) && Q.flaring(o) && N.slt(G.dist(U32.sub(Seat.x(o), x), U32.sub(Seat.y(o), y)), U32.mul(C.kindle_r(), 256)))

# The lantern's flame comes back: slowly anywhere, quickly at a resting brazier.
def refill_flame(+w: S) -> S:
  +mx = W.r_flame(w)
  +cap = U32.min(K.flame(w), mx)
  +near = rest_near(W.props(w), K.x(w), K.y(w), False{})
  +kind = kindled(W.seats(w), K.id(w), K.x(w), K.y(w), False{})
  +up = Bool.pick(U32, near, 3, Bool.pick(U32, kind, 2, Bool.pick(U32, U32.is_eq(U32.and(W.elapsed(w), 1), 0), 1, 0)))
  +w1 = K.set_flame(w, cap)
  Bool.pick(S, Q.flaring(W.cur(w1)), w1, K.set_flame(w1, U32.min(U32.add(cap, up), mx)))

def refill(+w: S) -> S: Z.lw(U32.is_ne(W.p_lantern(w), 0), u => refill_flame(w), w)

# ----- lock-on ---------------------------------------------------------------------------------

# Whether the knight's lock is of something still there and in reach.
def lock_holds(+w: S, +kind: U32, +i: U32) -> Bool:
  +p = Q.lock_point(w, kind, i)
  U32.is_eq(Nr.ok(p), 1) && N.sle(G.dist(U32.sub(Nr.x(p), K.x(w)), U32.sub(Nr.y(p), K.y(w))), N.sdiv(U32.mul(W.r_lock(w), 3), 2))

# Whether (a, b, d) comes before the best key so far: the Rust tuple order.
def sooner(+a: U32, +b: U32, +d: U32, +best: R.W8) -> Bool:
  U32.is_eq(Tg.ok(best), 0) || U32.is_lt(a, Tg.a(best)) || (U32.is_eq(a, Tg.a(best)) && (U32.is_lt(b, Tg.b(best)) || (U32.is_eq(b, Tg.b(best)) && N.slt(d, Tg.d(best)))))

# One thing a lock may take, in front or not, hostile or not.
def pick_one(+ok: Bool, +w: S, +kind: U32, +id: U32, +x: U32, +y: U32, +hostile: Bool, +best: R.W8) -> R.W8:
  match ok:
    case False{}:
      best
    case True{}:
      +u = G.unit(U32.sub(x, K.x(w)), U32.sub(y, K.y(w)))
      +front = N.sge(G.dot(K.fx(w), K.fy(w), G.vx(u), G.vy(u)), 0)
      +a = Bool.to_u32(Bool.not(hostile))
      +b = Bool.to_u32(Bool.not(front))
      +d = G.dist(U32.sub(x, K.x(w)), U32.sub(y, K.y(w)))
      Bool.pick(R.W8, sooner(a, b, d, best), R.W8{1, a, b, d, kind, id, 0, 0}, best)

# Whether something at (x, y) is within the lock's reach and in the clear. The
# line is only drawn for what is within reach.
def reachable(+near: Bool, +w: S, +x: U32, +y: U32) -> Bool:
  match near:
    case False{}:
      False{}
    case True{}:
      Q.line_open(w, K.x(w), K.y(w), x, y)

def pick_foes(+fs: List<&2, R.T8>, +w: S, +skip_k: U32, +skip_i: U32, +best: R.W8) -> R.W8:
  match fs:
    case Nil{}:
      best
    case Con{f, t}:
      +d = G.dist(U32.sub(Foe.x(f), K.x(w)), U32.sub(Foe.y(f), K.y(w)))
      +skip = U32.is_eq(skip_k, C.lk_foe()) && U32.is_eq(skip_i, Foe.id(f))
      pick_foes(t, w, skip_k, skip_i, pick_one(Q.foe_alive(f) && Bool.not(skip) && reachable(N.sle(d, W.r_lock(w)), w, Foe.x(f), Foe.y(f)), w, C.lk_foe(), Foe.id(f), Foe.x(f), Foe.y(f), True{}, best))

def pick_props(+ps: List<&2, R.W8>, +w: S, +i: U32, +skip_k: U32, +skip_i: U32, +best: R.W8) -> R.W8:
  match ps:
    case Nil{}:
      best
    case Con{p, t}:
      +d = G.dist(U32.sub(Pr.x(p), K.x(w)), U32.sub(Pr.y(p), K.y(w)))
      +skip = U32.is_eq(skip_k, C.lk_prop()) && U32.is_eq(skip_i, i)
      pick_props(t, w, U32.add(i, 1), skip_k, skip_i, pick_one(Q.target(p) && Bool.not(skip) && reachable(N.sle(d, W.r_lock(w)), w, Pr.x(p), Pr.y(p)), w, C.lk_prop(), i, Pr.x(p), Pr.y(p), False{}, best))

# The nearest thing in front of the knight, a foe before a switch; with nothing in
# front, the nearest at all. Found, then its kind and id.
def pick_target(+w: S, +skip_k: U32, +skip_i: U32) -> R.W8:
  +b1 = pick_foes(W.foes(w), w, skip_k, skip_i, R.W8{0, 0, 0, 0, 0, 0, 0, 0})
  +skip_boss = U32.is_eq(skip_k, C.lk_boss())
  +hx = Q.heart_x(w)
  +hy = Q.heart_y(w)
  +d = G.dist(U32.sub(hx, K.x(w)), U32.sub(hy, K.y(w)))
  +b2 = pick_one(Q.boss_up(w) && Bool.not(skip_boss) && reachable(N.sle(d, W.r_lock(w)), w, hx, hy), w, C.lk_boss(), 0, hx, hy, True{}, b1)
  pick_props(W.props(w), w, 0, skip_k, skip_i, b2)

# The next target in the direction the knight pressed, as seen from the current
# one: up and the lock jumps to what is above it on the screen. Each candidate is
# (found, distance, kind, id) and the best is the nearest.
def cycle_one(+ok: Bool, +w: S, +kind: U32, +id: U32, +x: U32, +y: U32, +cx: U32, +cy: U32, +wx: U32, +wy: U32, +best: R.W8) -> R.W8:
  match ok:
    case False{}:
      best
    case True{}:
      +vx = U32.sub(x, cx)
      +vy = U32.sub(y, cy)
      +d = G.dist(vx, vy)
      +u = G.unit(vx, vy)
      Bool.pick(R.W8, N.sgt(d, 0) && N.sgt(G.dot(G.vx(u), G.vy(u), wx, wy), 128) && (U32.is_eq(Tg.ok(best), 0) || N.slt(d, Tg.d(best))), R.W8{1, 0, 0, d, kind, id, 0, 0}, best)

def cycle_foes(+fs: List<&2, R.T8>, +w: S, +cx: U32, +cy: U32, +wx: U32, +wy: U32, +best: R.W8) -> R.W8:
  match fs:
    case Nil{}:
      best
    case Con{f, t}:
      +d = G.dist(U32.sub(Foe.x(f), K.x(w)), U32.sub(Foe.y(f), K.y(w)))
      +cur = U32.is_eq(K.lock_k(w), C.lk_foe()) && U32.is_eq(K.lock_i(w), Foe.id(f))
      cycle_foes(t, w, cx, cy, wx, wy, cycle_one(Q.foe_alive(f) && Bool.not(cur) && reachable(N.sle(d, W.r_lock(w)), w, Foe.x(f), Foe.y(f)), w, C.lk_foe(), Foe.id(f), Foe.x(f), Foe.y(f), cx, cy, wx, wy, best))

def cycle_props(+ps: List<&2, R.W8>, +w: S, +i: U32, +cx: U32, +cy: U32, +wx: U32, +wy: U32, +best: R.W8) -> R.W8:
  match ps:
    case Nil{}:
      best
    case Con{p, t}:
      +d = G.dist(U32.sub(Pr.x(p), K.x(w)), U32.sub(Pr.y(p), K.y(w)))
      +cur = U32.is_eq(K.lock_k(w), C.lk_prop()) && U32.is_eq(K.lock_i(w), i)
      cycle_props(t, w, U32.add(i, 1), cx, cy, wx, wy, cycle_one(Q.target(p) && Bool.not(cur) && reachable(N.sle(d, W.r_lock(w)), w, Pr.x(p), Pr.y(p)), w, C.lk_prop(), i, Pr.x(p), Pr.y(p), cx, cy, wx, wy, best))

def cycle_target(+w: S, +dirx: U32, +diry: U32) -> R.W8:
  +cur = Q.lock_point(w, K.lock_k(w), K.lock_i(w))
  +want = G.unit(dirx, diry)
  +b1 = cycle_foes(W.foes(w), w, Nr.x(cur), Nr.y(cur), G.vx(want), G.vy(want), R.W8{0, 0, 0, 0, 0, 0, 0, 0})
  +hx = Q.heart_x(w)
  +hy = Q.heart_y(w)
  +d = G.dist(U32.sub(hx, K.x(w)), U32.sub(hy, K.y(w)))
  +b2 = cycle_one(Q.boss_up(w) && Bool.not(U32.is_eq(K.lock_k(w), C.lk_boss())) && reachable(N.sle(d, W.r_lock(w)), w, hx, hy), w, C.lk_boss(), 0, hx, hy, Nr.x(cur), Nr.y(cur), G.vx(want), G.vy(want), b1)
  Bool.pick(R.W8, U32.is_eq(Nr.ok(cur), 1), cycle_props(W.props(w), w, 0, Nr.x(cur), Nr.y(cur), G.vx(want), G.vy(want), b2), R.W8{0, 0, 0, 0, 0, 0, 0, 0})

def face_to(+w: S, +x: U32, +y: U32) -> S:
  +f = G.unit(U32.sub(x, K.x(w)), U32.sub(y, K.y(w)))
  Bool.pick(S, U32.is_ne(G.vx(f), 0) || U32.is_ne(G.vy(f), 0), K.set_fy(K.set_fx(w, G.vx(f)), G.vy(f)), w)

def take_lock_at(+ok: Bool, +w: S, +x: U32, +y: U32) -> S:
  match ok:
    case False{}:
      w
    case True{}:
      face_to(Q.fx(w, C.fxk_tick(), x, y, 0), x, y)

# The lock is taken on a thing: the wick-spark settles over it, and the knight
# swings round to face it at once.
def take_lock(+w: S, +kind: U32, +id: U32) -> S:
  +w1 = K.set_lock_t(K.set_lock_i(K.set_lock_k(w, kind), id), 0)
  +p = Q.lock_point(w1, kind, id)
  Q.sound(take_lock_at(U32.is_eq(Nr.ok(p), 1), w1, Nr.x(p), Nr.y(p)), 7)

def take_found(+found: Bool, +w: S, +t: R.W8) -> S:
  match found:
    case True{}:
      take_lock(w, Tg.kind(t), Tg.id(t))
    case False{}:
      w

def lock_pick(+w: S, +pressed: Bool) -> S:
  +t = pick_target(w, 0, 0)
  Bool.pick(S, U32.is_eq(Tg.ok(t), 1), take_lock(w, Tg.kind(t), Tg.id(t)), Bool.pick(S, pressed, K.set_unlock_t(w, C.ease_out()), w))

def lock_free(+look: Bool, +w: S, +pressed: Bool) -> S:
  match look:
    case True{}:
      lock_pick(w, pressed)
    case False{}:
      K.set_relock(w, M.dec(K.relock(w)))

def take_found_cycle(+w: S, +t: R.W8) -> S: take_found(U32.is_eq(Tg.ok(t), 1), w, t)

def lock_hold2(+w: S, +fresh: Bool, +pressed: Bool, +ax: U32, +ay: U32) -> S:
  +has = U32.is_ne(K.lock_k(w), 0)
  Z.lz(has && fresh,
    u => take_found_cycle(w, cycle_target(w, ax, ay)),
    u => Z.lw(Bool.not(has), u => lock_free(pressed || U32.is_eq(K.relock(w), 0), w, pressed), w))

def lock_hold(+w: S, +ax: U32, +ay: U32, +pressed: Bool) -> S:
  +drop = U32.is_ne(K.lock_k(w), 0) && Bool.not(lock_holds(w, K.lock_k(w), K.lock_i(w)))
  +w1 = Bool.pick(S, drop, K.set_relock(K.set_lock_i(K.set_lock_k(w, 0), 0), C.relock()), w)
  +fresh = (U32.is_ne(ax, 0) || U32.is_ne(ay, 0)) && (U32.is_ne(ax, K.aimx(w1)) || U32.is_ne(ay, K.aimy(w1)))
  lock_hold2(K.set_aimy(K.set_aimx(w1, ax), ay), fresh, pressed, ax, ay)

def lock_release(+w: S, +ax: U32, +ay: U32) -> S:
  +w1 = Bool.pick(S, U32.is_ne(K.lock_k(w), 0), K.set_unlock_t(w, C.ease_out()), w)
  K.set_aimy(K.set_aimx(K.set_relock(K.set_lock_i(K.set_lock_k(w1, 0), 0), 0), ax), ay)

# Holding R with the knight up keeps or takes a lock; letting go lets it go.
def lock_on(+w: S, +ix: R.W8, +press: U32) -> S:
  +holding = U32.is_ne(U32.and(Ix.mask(ix), C.b_lock()), 0)
  Z.lz(holding && Q.up(W.cur(w)), u => lock_hold(w, Ix.ax(ix), Ix.ay(ix), U32.is_ne(U32.and(press, C.b_lock()), 0)), u => lock_release(w, Ix.ax(ix), Ix.ay(ix)))

# ----- walking -------------------------------------------------------------------------------------

# The index of the first block, not already sliding, that stands where a body
# leans.
def block_at(+ps: List<&2, R.W8>, +x: U32, +y: U32, +tx: U32, +ty: U32, +j: U32, +found: U32, +done: Bool) -> U32:
  match ps:
    case Nil{}:
      found
    case Con{p, t}:
      match done:
        case True{}:
          found
        case False{}:
          +hit = U32.is_eq(Pr.kind(p), C.pk_block()) && U32.is_eq(Pr.on(p), 0) && Q.blocks(p, x, y, tx, ty)
          block_at(t, x, y, tx, ty, U32.add(j, 1), Bool.pick(U32, hit, j, found), hit)

# Two for each tick leaned, one lost each tick not: a push is a push held for a
# moment, and then the block slides.
def lean_block(+w: S, +b: U32, +sx: U32, +sy: U32) -> S:
  +p = L.get8(W.props(w), U32.to_nat(b))
  +t = U32.min(U32.add(Pr.t(p), 2), 65535)
  +full = U32.is_ge(t, C.push_lean())
  +way = Bool.pick(U32, U32.is_eq(sx, 1), 1, Bool.pick(U32, U32.is_eq(sx, N.sneg(1)), 2, Bool.pick(U32, U32.is_eq(sy, 1), 3, 4)))
  +w1 = W.set_props(w, L.set8(W.props(w), U32.to_nat(b), Bool.pick(R.W8, full, Pr.set_on(Pr.set_t(p, 0), way), Pr.set_t(p, t))))
  Z.lw(full, u => X.start_slide(w1, b), w1)

# A body pressing on a block: after a moment of leaning, it slides.
def lean(+w: S, +sx: U32, +sy: U32) -> S:
  +rx = U32.add(K.x(w), U32.mul(sx, 2048))
  +ry = U32.add(K.y(w), U32.mul(sy, 2048))
  +b = block_at(W.props(w), rx, ry, G.tile_of(rx), G.tile_of(ry), 0, C.none(), False{})
  Bool.pick(S, U32.is_ne(b, C.none()), lean_block(w, b, sx, sy), w)

def shove_x(+go: Bool, +w: S, +dx: U32) -> S:
  match go:
    case False{}:
      w
    case True{}:
      Z.lz(Q.clear(w, U32.add(K.x(w), dx), K.y(w), 1280), u => K.set_x(w, U32.add(K.x(w), dx)), u => lean(w, M.sign(dx), 0))

def shove_y(+go: Bool, +w: S, +dy: U32) -> S:
  match go:
    case False{}:
      w
    case True{}:
      Z.lz(Q.clear(w, K.x(w), U32.add(K.y(w), dy), 1280), u => K.set_y(w, U32.add(K.y(w), dy)), u => lean(w, 0, M.sign(dy)))

def fall_in(+w: S) -> S: K.set_shield(K.set_lamp(K.set_lock_i(K.set_lock_k(K.set_t(K.set_act(w, C.a_fall()), 0), 0), 0), 0), 0)

# Move a body by (dx, dy), a whole axis at a time so it slides along a wall;
# leans on a block it meets; falls into a pit it walks into.
def shove(+w: S, +dx: U32, +dy: U32) -> S:
  +act = K.act(w)
  +airborne = N.sgt(K.z(w), 0) || U32.is_eq(act, C.a_roll()) || U32.is_eq(act, C.a_hop()) || U32.is_eq(act, C.a_plunge())
  +w2 = shove_y(U32.is_ne(dy, 0), shove_x(U32.is_ne(dx, 0), w, dx), dy)
  Bool.pick(S, Bool.not(airborne) && Q.pit(w2, K.x(w2), K.y(w2)) && U32.is_eq(K.act(w2), C.a_free()), fall_in(w2), w2)

# Walk about a locked target: the sideways part of the step goes round it at the
# same distance, the rest closes or opens it.
def orbit_go(+w: S, +tx: U32, +ty: U32, +vx: U32, +vy: U32, +len: U32) -> S:
  +rx = U32.sub(K.x(w), tx)
  +ry = U32.sub(K.y(w), ty)
  +rh = G.unit(rx, ry)
  +thx = N.sneg(G.vy(rh))
  +thy = G.vx(rh)
  +radial0 = G.dot(vx, vy, G.vx(rh), G.vy(rh))
  +tangent0 = G.dot(vx, vy, thx, thy)
  +speed = G.dist(vx, vy)
  +all_side = N.sge(U32.mul(N.sabs(tangent0), 10), U32.mul(speed, 8))
  +all_along = N.sge(U32.mul(N.sabs(radial0), 10), U32.mul(speed, 8))
  +radial = Bool.pick(U32, all_side, 0, radial0)
  +tangent = Bool.pick(U32, all_side, tangent0, Bool.pick(U32, all_along, 0, tangent0))
  +want = N.smax(U32.add(len, radial), U32.mul(C.orbit_min(), 256))
  +offx = U32.add(N.sdiv(U32.mul(G.vx(rh), len), 256), N.sdiv(U32.mul(tangent, thx), 256))
  +offy = U32.add(N.sdiv(U32.mul(G.vy(rh), len), 256), N.sdiv(U32.mul(tangent, thy), 256))
  +u = G.unit(offx, offy)
  +px = U32.add(tx, N.sdiv(U32.mul(G.vx(u), want), 256))
  +py = U32.add(ty, N.sdiv(U32.mul(G.vy(u), want), 256))
  shove(w, U32.sub(px, K.x(w)), U32.sub(py, K.y(w)))

def orbit(+w: S, +tx: U32, +ty: U32, +vx: U32, +vy: U32) -> S:
  +len = G.dist(U32.sub(K.x(w), tx), U32.sub(K.y(w), ty))
  Z.lz(N.slt(len, 256), u => shove(w, vx, vy), u => orbit_go(w, tx, ty, vx, vy, len))

# The way the knight faces: toward a lock, else the arrows, else the way it walks
# with no shield up and nothing in the way of turning.
def face_toward(+w: S, +ix: R.W8) -> S:
  +lp = Q.lock_point(w, K.lock_k(w), K.lock_i(w))
  +locked = U32.is_ne(K.lock_k(w), 0) && U32.is_eq(Nr.ok(lp), 1)
  +aiming = U32.is_ne(Ix.ax(ix), 0) || U32.is_ne(Ix.ay(ix), 0)
  +walking = U32.is_ne(Ix.mx(ix), 0) || U32.is_ne(Ix.my(ix), 0)
  +u = G.unit(Ix.ax(ix), Ix.ay(ix))
  +v = G.unit(Ix.mx(ix), Ix.my(ix))
  Bool.pick(S, locked, face_to(w, Nr.x(lp), Nr.y(lp)),
    Bool.pick(S, aiming, K.set_fy(K.set_fx(w, G.vx(u)), G.vy(u)),
      Bool.pick(S, walking && U32.is_eq(K.shield(w), 0) && U32.is_eq(K.act(w), C.a_free()), K.set_fy(K.set_fx(w, G.vx(v)), G.vy(v)), w)))

def speed_of(+w: S) -> U32:
  Bool.pick(U32, U32.is_ne(K.shield(w), 0), C.guard_walk(), Bool.pick(U32, U32.is_eq(K.carry(w), 1) || U32.is_gt(K.charge(w), C.charge_walk_after()), C.carry_walk(), Bool.pick(U32, U32.is_ne(K.lock_k(w), 0), C.locked_walk(), C.walk())))

def walk(+w: S, +ix: R.W8) -> S:
  +v = G.unit(Ix.mx(ix), Ix.my(ix))
  +speed = speed_of(w)
  +vx = N.sdiv(U32.mul(G.vx(v), speed), 256)
  +vy = N.sdiv(U32.mul(G.vy(v), speed), 256)
  +lp = Q.lock_point(w, K.lock_k(w), K.lock_i(w))
  Z.lz(U32.is_ne(K.lock_k(w), 0) && U32.is_eq(Nr.ok(lp), 1), u => orbit(w, Nr.x(lp), Nr.y(lp), vx, vy), u => shove(w, vx, vy))

def pushed(+w: S) -> S:
  Z.lw(U32.is_ne(K.pusht(w), 0), u => shove(K.set_pusht(w, M.dec(K.pusht(w))), K.pushx(w), K.pushy(w)), w)

def move_free(+w: S, +ix: R.W8) -> S:
  +w1 = pushed(w)
  Z.lw(U32.is_ne(Ix.mx(ix), 0) || U32.is_ne(Ix.my(ix), 0), u => walk(w1, ix), w1)

def face_and_move(+w: S, +ix: R.W8) -> S:
  +w1 = face_toward(w, ix)
  Z.lw(U32.is_eq(K.act(w1), C.a_free()), u => move_free(w1, ix), w1)

# ----- falling and being hurt ---------------------------------------------------------------------

def falling(+w: S) -> S:
  +t = U32.add(K.t(w), 1)
  +w1 = K.set_t(w, t)
  Bool.pick(S, U32.is_ge(t, C.fall_ticks()), K.set_inv(K.set_t(K.set_act(K.set_y(K.set_x(K.set_hp(w1, U32.max(M.sat(K.hp(w1), C.fall_damage()), 1)), K.safex(w1)), K.safey(w1)), C.a_free()), 0), C.hurt_inv()), w1)

def hurting(+w: S) -> S:
  +w1 = K.set_t(w, U32.add(K.t(w), 1))
  +w2 = pushed(w1)
  Bool.pick(S, U32.is_ge(K.t(w2), C.hurt_ticks()), K.set_t(K.set_act(w2, C.a_free()), 0), w2)

# ----- the sword -------------------------------------------------------------------------------------

# A seat's mark in a foe's or the Warden's record of which swing last landed on
# it: six words, picked by the seat.
def foe_hit_of(+f: R.T8, +s: U32) -> U32:
  Bool.pick(U32, U32.is_eq(s, 0), Foe.h0(f), Bool.pick(U32, U32.is_eq(s, 1), Foe.h1(f), Bool.pick(U32, U32.is_eq(s, 2), Foe.h2(f), Bool.pick(U32, U32.is_eq(s, 3), Foe.h3(f), Bool.pick(U32, U32.is_eq(s, 4), Foe.h4(f), Foe.h5(f))))))

def foe_hit_set(+f: R.T8, +s: U32, +v: U32) -> R.T8:
  Bool.pick(R.T8, U32.is_eq(s, 0), Foe.set_h0(f, v), Bool.pick(R.T8, U32.is_eq(s, 1), Foe.set_h1(f, v), Bool.pick(R.T8, U32.is_eq(s, 2), Foe.set_h2(f, v), Bool.pick(R.T8, U32.is_eq(s, 3), Foe.set_h3(f, v), Bool.pick(R.T8, U32.is_eq(s, 4), Foe.set_h4(f, v), Foe.set_h5(f, v))))))

def boss_hit_of(+w: S, +s: U32) -> U32:
  Bool.pick(U32, U32.is_eq(s, 0), B.h0(w), Bool.pick(U32, U32.is_eq(s, 1), B.h1(w), Bool.pick(U32, U32.is_eq(s, 2), B.h2(w), Bool.pick(U32, U32.is_eq(s, 3), B.h3(w), Bool.pick(U32, U32.is_eq(s, 4), B.h4(w), B.h5(w))))))

def boss_hit_set(+w: S, +s: U32, +v: U32) -> S:
  Bool.pick(S, U32.is_eq(s, 0), B.set_h0(w, v), Bool.pick(S, U32.is_eq(s, 1), B.set_h1(w, v), Bool.pick(S, U32.is_eq(s, 2), B.set_h2(w, v), Bool.pick(S, U32.is_eq(s, 3), B.set_h3(w, v), Bool.pick(S, U32.is_eq(s, 4), B.set_h4(w, v), B.set_h5(w, v))))))

# Whether a point is inside a swing's arc: within reach, and in front of the knight
# or close enough that it does not matter (or an arc all round).
def inside(+w: S, +x: U32, +y: U32, +extra: U32, +reach: U32, +cos: U32) -> Bool:
  +dx = U32.sub(x, K.x(w))
  +dy = U32.sub(y, K.y(w))
  +d = G.dist(dx, dy)
  +u = G.unit(dx, dy)
  N.sle(d, U32.add(U32.mul(reach, 256), extra)) && (N.sle(cos, N.sneg(256)) || N.slt(d, 1536) || N.sge(G.dot(G.vx(u), G.vy(u), K.fx(w), K.fy(w)), cos))

# A swing struck a foe: it is marked so one swing lands once, and hurt. `ret2` is
# whether anything landed.
def sweep_foe_hit(+w: S, +f: R.T8, +i: U32, +seat: U32, +dmg: U32) -> S:
  +w1 = W.set_foes(w, L.set64(W.foes(w), U32.to_nat(i), foe_hit_set(f, seat, K.swing(w))))
  +w2 = H.hurt_foe(w1, i, dmg, K.x(w), K.y(w))
  W.set_ret2(w2, U32.or(W.ret2(w2), W.ret(w2)))

def sweep_foe(+w: S, +i: U32, +seat: U32, +dmg: U32, +reach: U32, +cos: U32) -> S:
  +f = L.get64(W.foes(w), U32.to_nat(i))
  +skip = U32.is_eq(foe_hit_of(f, seat), K.swing(w)) || Bool.not(Q.foe_alive(f)) || Bool.not(inside(w, Foe.x(f), Foe.y(f), Bool.pick(U32, U32.is_eq(Foe.kind(f), C.fk_turret()), 2048, 1536), reach, cos))
  Z.lz(skip, u => w, u => sweep_foe_hit(w, f, i, seat, dmg))

def sweep_foes(+n: Nat, +w: S, +i: U32, +seat: U32, +dmg: U32, +reach: U32, +cos: U32) -> S:
  match n:
    case 0n:
      w
    case 1n+p:
      sweep_foes(p, sweep_foe(w, i, seat, dmg, reach, cos), U32.add(i, 1), seat, dmg, reach, cos)

def sweep_boss_hit(+w: S, +seat: U32, +dmg: U32, +id: U32) -> S:
  +w1 = boss_hit_set(w, seat, K.swing(w))
  +w2 = H.hurt_boss(w1, dmg, K.x(w), K.y(w))
  W.set_ret2(w2, U32.or(W.ret2(w2), W.ret(w2)))

def sweep_boss(+w: S, +seat: U32, +dmg: U32, +id: U32, +reach: U32, +cos: U32) -> S:
  Z.lw(Q.boss_up(w) && inside(w, B.x(w), B.y(w), 4608, reach, cos) && U32.is_ne(boss_hit_of(w, seat), K.swing(w)), u => sweep_boss_hit(w, seat, dmg, id), w)

def sweep_prop(+w: S, +i: U32, +reach: U32, +cos: U32) -> S:
  +p = L.get8(W.props(w), U32.to_nat(i))
  +k = Pr.kind(p)
  +cuttable = (U32.is_eq(k, C.pk_pot()) || U32.is_eq(k, C.pk_grass()) || U32.is_eq(k, C.pk_crystal())) && U32.is_eq(Pr.on(p), 0)
  Z.lw(cuttable && inside(w, Pr.x(p), Pr.y(p), 1536, reach, cos), u => Z.lz(U32.is_eq(k, C.pk_crystal()), u => Q.light_crystal(w, i), u => Q.shatter(w, i)), w)

def sweep_props(+n: Nat, +w: S, +i: U32, +reach: U32, +cos: U32) -> S:
  match n:
    case 0n:
      w
    case 1n+p:
      sweep_props(p, sweep_prop(w, i, reach, cos), U32.add(i, 1), reach, cos)

# Hostile bolts inside the arc are cut down.
def cut_bolts(+bs: List<&2, R.W8>, +w: S, +reach: U32, +cos: U32) -> List<&2, R.W8>:
  match bs:
    case Nil{}:
      Nil{}
    case Con{b, t}:
      Con{Bool.pick(R.W8, U32.is_eq(Bo.kind(b), C.bk_iron()) && inside(w, Bo.x(b), Bo.y(b), 1024, reach, cos), Bo.set_ttl(b, 0), b), cut_bolts(t, w, reach, cos)}

def any_cut(+bs: List<&2, R.W8>, +w: S, +reach: U32, +cos: U32) -> Bool:
  match bs:
    case Nil{}:
      False{}
    case Con{b, t}:
      (U32.is_eq(Bo.kind(b), C.bk_iron()) && inside(w, Bo.x(b), Bo.y(b), 1024, reach, cos)) || any_cut(t, w, reach, cos)

def sweep_cut(+cut: Bool, +w: S) -> S:
  match cut:
    case False{}:
      w
    case True{}:
      Q.fx(Q.sound(w, 10), C.fxk_spark(), U32.add(K.x(w), U32.mul(K.fx(w), 18)), U32.add(K.y(w), U32.mul(K.fy(w), 18)), 0)

# A swing's arc: every foe, pot, blade-cut thing and iron bolt inside it is
# struck once. `ret` is 1 when something was hurt.
def sweep(+w: S, +id: U32, +reach: U32, +cos: U32, +dmg: U32, +heavy: Bool) -> S:
  +seat = U32.min(U32.sub(id, 1), 5)
  +w0 = W.set_ret2(w, 0)
  +w1 = sweep_foes(U32.to_nat(L.len64(W.foes(w0))), w0, 0, seat, dmg, reach, cos)
  +w2 = sweep_boss(w1, seat, dmg, id, reach, cos)
  +w3 = sweep_props(U32.to_nat(L.len8(W.props(w2))), w2, 0, reach, cos)
  +cut = any_cut(W.bolts(w3), w3, reach, cos)
  +w4 = sweep_cut(cut, W.set_bolts(w3, cut_bolts(W.bolts(w3), w3, reach, cos)))
  +landed = U32.is_ne(W.ret2(w4), 0)
  +w5 = Bool.pick(S, landed, Q.stop(w4, C.stop_heavy()), w4)
  +w6 = Bool.pick(S, landed && heavy, Q.sound(W.set_shake(w5, U32.max(W.shake(w5), C.shake_heavy())), 17), w5)
  W.set_ret(w6, Bool.to_u32(landed))

# A lunge's line: from the knight out ahead, narrow and long.
def lunge_foe_hit(+w: S, +f: R.T8, +i: U32, +seat: U32) -> S:
  +w1 = W.set_foes(w, L.set64(W.foes(w), U32.to_nat(i), foe_hit_set(f, seat, K.swing(w))))
  +w2 = H.hurt_foe(w1, i, C.lunge_damage(), K.x(w), K.y(w))
  W.set_ret2(w2, U32.or(W.ret2(w2), W.ret(w2)))

def lunge_foe(+w: S, +i: U32, +seat: U32, +x1: U32, +y1: U32) -> S:
  +f = L.get64(W.foes(w), U32.to_nat(i))
  +skip = U32.is_eq(foe_hit_of(f, seat), K.swing(w)) || Bool.not(Q.foe_alive(f)) || Bool.not(D.hits(C.dg_capsule(), K.x(w), K.y(w), x1, y1, 1280, Foe.x(f), Foe.y(f), Bool.pick(U32, U32.is_eq(Foe.kind(f), C.fk_turret()), 2048, 1536)))
  Z.lz(skip, u => w, u => lunge_foe_hit(w, f, i, seat))

def lunge_foes(+n: Nat, +w: S, +i: U32, +seat: U32, +x1: U32, +y1: U32) -> S:
  match n:
    case 0n:
      w
    case 1n+p:
      lunge_foes(p, lunge_foe(w, i, seat, x1, y1), U32.add(i, 1), seat, x1, y1)

def lunge_hit(+w: S, +id: U32) -> S:
  +seat = U32.min(U32.sub(id, 1), 5)
  +x1 = U32.add(K.x(w), U32.mul(K.fx(w), C.lunge_reach()))
  +y1 = U32.add(K.y(w), U32.mul(K.fy(w), C.lunge_reach()))
  +w0 = W.set_ret2(w, 0)
  +w1 = lunge_foes(U32.to_nat(L.len64(W.foes(w0))), w0, 0, seat, x1, y1)
  +strikes = Q.boss_up(w1) && U32.is_ne(boss_hit_of(w1, seat), K.swing(w1)) && D.hits(C.dg_capsule(), K.x(w1), K.y(w1), x1, y1, 1280, B.x(w1), B.y(w1), 4608)
  +w2 = Z.lw(strikes, u => sweep_boss_hit(w1, seat, C.lunge_damage(), id), w1)
  +landed = U32.is_ne(W.ret2(w2), 0)
  W.set_ret(Bool.pick(S, landed, Q.stop(w2, C.stop_heavy()), w2), Bool.to_u32(landed))

def begin_slash(+w: S, +step: U32) -> S:
  Q.sound(K.set_mvy(K.set_mvx(K.set_queued(K.set_swing(K.set_n(K.set_t(K.set_act(w, C.a_slash()), 0), U32.min(step, 2)), U32.add(K.swing(w), 1)), 0), 0), 0), 41)

def begin_lunge(+w: S) -> S:
  Q.sound(K.set_mvy(K.set_mvx(K.set_step(K.set_chain(K.set_swing(K.set_t(K.set_act(w, C.a_lunge()), 0), U32.add(K.swing(w), 1)), 0), 0), N.sdiv(U32.mul(K.fx(w), C.lunge_step()), 256)), N.sdiv(U32.mul(K.fy(w), C.lunge_step()), 256)), 41)

def begin_spin(+w: S) -> S:
  Q.sound(K.set_step(K.set_chain(K.set_charge(K.set_swing(K.set_t(K.set_act(w, C.a_spin()), 0), U32.add(K.swing(w), 1)), 0), 0), 0), 0)

# The slash tables: wind-up, active ticks, rest, reach, the arc's cosine, damage.
def slash_wind(+n: U32) -> U32: Bool.pick(U32, U32.is_eq(n, 2), 4, 2)
def slash_active(+n: U32) -> U32: Bool.pick(U32, U32.is_eq(n, 2), 4, 3)
def slash_rest(+n: U32) -> U32: Bool.pick(U32, U32.is_eq(n, 2), 12, 6)
def slash_reach(+n: U32) -> U32: Bool.pick(U32, U32.is_eq(n, 2), 22, 18)
def slash_cos(+n: U32) -> U32: Bool.pick(U32, U32.is_eq(n, 2), N.sneg(44), 66)
def slash_damage(+n: U32) -> U32: Bool.pick(U32, U32.is_eq(n, 2), 4, 2)

# Whether a slash or a lunge has struck and is only coming back.
def recovering(+w: S) -> Bool:
  +n = U32.min(K.n(w), 2)
  (U32.is_eq(K.act(w), C.a_slash()) && U32.is_gt(K.t(w), U32.add(slash_wind(n), slash_active(n)))) || (U32.is_eq(K.act(w), C.a_lunge()) && U32.is_ge(K.t(w), U32.add(C.lunge_wind(), C.lunge_active())))

# A blade that landed holds its owner a moment: the knight's own hit-stop.
def freeze_if(+hit: Bool, +w: S, +ticks: U32) -> S: Bool.pick(S, hit, K.set_freeze(w, U32.max(K.freeze(w), ticks)), w)

def slash_strike(+live: Bool, +w: S, +id: U32, +n: U32) -> S:
  match live:
    case False{}:
      w
    case True{}:
      +s = sweep(w, id, slash_reach(n), slash_cos(n), slash_damage(n), U32.is_eq(n, 2))
      freeze_if(U32.is_ne(W.ret(s), 0), s, Bool.pick(U32, U32.is_eq(n, 2), 4, 2))

# When the rest is over the knight is free, and a slash queued in time begins the
# next of the chain.
def slash_end(+over: Bool, +w: S) -> S:
  match over:
    case False{}:
      w
    case True{}:
      +w1 = K.set_t(K.set_act(w, C.a_free()), 0)
      Bool.pick(S, U32.is_ne(K.queued(w1), 0) && U32.is_ne(K.step(w1), 0) && U32.is_ne(K.chain(w1), 0), begin_slash(w1, K.step(w1)), w1)

# Slash `n` of the chain: queued if the sword is pressed again, struck in its
# active ticks, the next slash opened for a window when the blow lands.
def slash_tick(+w: S, +id: U32, +press: U32) -> S:
  +n = U32.min(K.n(w), 2)
  +wind = slash_wind(n)
  +active = slash_active(n)
  +w1 = Bool.pick(S, U32.is_ne(U32.and(press, C.b_sword()), 0), K.set_queued(w, 1), w)
  +t = K.t(w1)
  +w2 = slash_strike(U32.is_gt(t, wind) && U32.is_le(t, U32.add(wind, active)), w1, id, n)
  +w3 = Bool.pick(S, U32.is_eq(t, U32.add(wind, active)), K.set_step(K.set_chain(w2, U32.add(slash_rest(n), W.r_combo(w2))), Bool.pick(U32, U32.is_eq(n, 2), 0, U32.add(n, 1))), w2)
  slash_end(U32.is_ge(t, U32.add(U32.add(wind, active), slash_rest(n))), w3)

def lunge_strike(+live: Bool, +w: S, +id: U32) -> S:
  match live:
    case False{}:
      w
    case True{}:
      +h = lunge_hit(shove(w, K.mvx(w), K.mvy(w)), id)
      freeze_if(U32.is_ne(W.ret(h), 0), h, 3)

def lunge_tick(+w: S, +id: U32) -> S:
  +t = K.t(w)
  +w1 = lunge_strike(U32.is_ge(t, C.lunge_wind()) && U32.is_lt(t, U32.add(C.lunge_wind(), C.lunge_active())), w, id)
  Bool.pick(S, U32.is_ge(t, U32.add(U32.add(C.lunge_wind(), C.lunge_active()), C.lunge_rest())), K.set_t(K.set_act(w1, C.a_free()), 0), w1)

def spin_strike(+live: Bool, +w: S, +id: U32) -> S:
  match live:
    case False{}:
      w
    case True{}:
      +s = sweep(w, id, C.spin_reach(), N.sneg(256), C.spin_damage(), True{})
      freeze_if(U32.is_ne(W.ret(s), 0), s, 3)

def spin_tick(+w: S, +id: U32) -> S:
  +t = K.t(w)
  +w1 = spin_strike(U32.is_ge(t, 2) && U32.is_lt(t, U32.sub(C.spin_ticks(), 2)), w, id)
  Bool.pick(S, U32.is_ge(t, C.spin_ticks()), K.set_t(K.set_act(w1, C.a_free()), 0), w1)

def swinging(+w: S, +id: U32, +press: U32) -> S:
  +w1 = K.set_t(w, U32.add(K.t(w), 1))
  +act = K.act(w1)
  Z.lz(U32.is_eq(act, C.a_slash()), u => slash_tick(w1, id, press), u => Z.lz(U32.is_eq(act, C.a_lunge()), u => lunge_tick(w1, id), u => Z.lw(U32.is_eq(act, C.a_spin()), u => spin_tick(w1, id), w1)))

# ----- the dodges --------------------------------------------------------------------------------------

# E with nothing to do, locked on: the hop the stick asks for, forward, back or to
# a side.
def hop_dir(+w: S, +ix: R.W8, +lx: U32, +ly: U32) -> S:
  +fwd = G.unit(U32.sub(lx, K.x(w)), U32.sub(ly, K.y(w)))
  +fx = G.vx(fwd)
  +fy = G.vy(fwd)
  +sx = N.sneg(fy)
  +sy = fx
  +walk = G.unit(Ix.mx(ix), Ix.my(ix))
  +still = U32.is_eq(Ix.mx(ix), 0) && U32.is_eq(Ix.my(ix), 0)
  +f = Bool.pick(U32, still, N.sneg(256), G.dot(G.vx(walk), G.vy(walk), fx, fy))
  +s = Bool.pick(U32, still, 0, G.dot(G.vx(walk), G.vy(walk), sx, sy))
  +side = N.sgt(N.sabs(s), N.sabs(f)) && N.sgt(N.sabs(s), 64)
  +sign = M.sign(s)
  +forward = Bool.not(side) && N.sgt(f, 64)
  +kind = Bool.pick(U32, side, 0, Bool.pick(U32, forward, 2, 1))
  +vx = Bool.pick(U32, side, U32.mul(sx, sign), Bool.pick(U32, forward, fx, N.sneg(fx)))
  +vy = Bool.pick(U32, side, U32.mul(sy, sign), Bool.pick(U32, forward, fy, N.sneg(fy)))
  +speed = Bool.pick(U32, side, C.hop_side(), Bool.pick(U32, forward, C.hop_forward(), C.hop_back()))
  Q.sound(K.set_lock_t(K.set_mvy(K.set_mvx(K.set_t(K.set_n(K.set_act(w, C.a_hop()), kind), 0), N.sdiv(U32.mul(vx, speed), 256)), N.sdiv(U32.mul(vy, speed), 256)), 0), 29)

# E with nothing to do, free: a roll along the walk, or the way it faces.
def roll_dir(+w: S, +ix: R.W8) -> S:
  +still = U32.is_eq(Ix.mx(ix), 0) && U32.is_eq(Ix.my(ix), 0)
  +walk = G.unit(Ix.mx(ix), Ix.my(ix))
  +vx = Bool.pick(U32, still, K.fx(w), G.vx(walk))
  +vy = Bool.pick(U32, still, K.fy(w), G.vy(walk))
  Q.sound(K.set_fy(K.set_fx(K.set_mvy(K.set_mvx(K.set_n(K.set_t(K.set_act(w, C.a_roll()), 0), 0), N.sdiv(U32.mul(vx, C.roll_speed()), 256)), N.sdiv(U32.mul(vy, C.roll_speed()), 256)), vx), vy), 29)

def dodge_with(+locked: Bool, +w: S, +ix: R.W8, +lx: U32, +ly: U32) -> S:
  match locked:
    case True{}:
      hop_dir(w, ix, lx, ly)
    case False{}:
      roll_dir(w, ix)

def begin_dodge(+w: S, +ix: R.W8) -> S:
  +lp = Q.lock_point(w, K.lock_k(w), K.lock_i(w))
  +w1 = dodge_with(U32.is_ne(K.lock_k(w), 0) && U32.is_eq(Nr.ok(lp), 1), w, ix, Nr.x(lp), Nr.y(lp))
  K.set_charge(K.set_step(K.set_chain(w1, 0), 0), 0)

def rolling(+w: S) -> S:
  +w1 = K.set_t(w, U32.add(K.t(w), 1))
  +w2 = Z.lw(U32.is_lt(K.t(w1), C.roll_move()), u => shove(w1, K.mvx(w1), K.mvy(w1)), w1)
  +done = U32.is_ge(K.t(w2), C.roll_ticks())
  +w3 = Bool.pick(S, done, K.set_act(K.set_t(K.set_act(w2, C.a_free()), 0), Bool.pick(U32, Q.pit(w2, K.x(w2), K.y(w2)), C.a_fall(), C.a_free())), w2)
  K.set_inv(w3, U32.max(K.inv(w3), Bool.to_u32(U32.is_ge(K.t(w3), C.roll_iframes_lo()) && U32.is_le(K.t(w3), C.roll_iframes_hi()))))

# The hop's last tick: down on the ground in a puff of dust, and into a pit if
# that is where it came down.
def hop_down(+w: S) -> S:
  +w1 = Q.fx(K.set_t(K.set_act(K.set_z(w, 0), C.a_free()), 0), C.fxk_dust(), K.x(w), K.y(w), 0)
  Bool.pick(S, Q.pit(w1, K.x(w1), K.y(w1)), K.set_act(w1, C.a_fall()), w1)

def hop_plunge(+w: S) -> S: Q.sound(K.set_swing(K.set_mvy(K.set_mvx(K.set_t(K.set_act(w, C.a_plunge()), 0), 0), 0), U32.add(K.swing(w), 1)), 41)

def hop_air(+w: S, +press: U32) -> S:
  +t = K.t(w)
  +z = N.smax(N.sdiv(U32.mul(U32.mul(U32.mul(C.hop_height(), 256), 4), U32.mul(t, U32.sub(C.hop_ticks(), t))), U32.mul(C.hop_ticks(), C.hop_ticks())), 0)
  +w1 = K.set_z(w, z)
  +w2 = Bool.pick(S, U32.is_ge(t, C.hop_iframes_lo()) && U32.is_le(t, C.hop_iframes_hi()), K.set_inv(w1, U32.max(K.inv(w1), 1)), w1)
  Bool.pick(S, U32.is_ne(U32.and(press, C.b_sword()), 0) && N.sgt(z, 512), hop_plunge(w2), Bool.pick(S, U32.is_ge(t, C.hop_ticks()), hop_down(w2), w2))

def hopping(+w: S, +press: U32) -> S:
  +w1 = K.set_t(w, U32.add(K.t(w), 1))
  +w2 = Z.lw(U32.is_lt(K.t(w1), U32.sub(C.hop_ticks(), 2)), u => shove(w1, K.mvx(w1), K.mvy(w1)), w1)
  hop_air(w2, press)

# Down: the blow lands in a ring about the knight.
def plunge_land(+w: S, +id: U32) -> S:
  +s = sweep(w, id, C.plunge_reach(), N.sneg(256), C.plunge_damage(), True{})
  +w1 = Q.fx(s, C.fxk_ring(), K.x(s), K.y(s), 24)
  +w2 = W.set_shake(Q.sound(w1, 28), U32.max(W.shake(w1), C.shake_heavy()))
  +w3 = Bool.pick(S, U32.is_ne(W.ret(s), 0), K.set_freeze(w2, 3), w2)
  +w4 = K.set_guard_cd(K.set_t(K.set_act(w3, C.a_free()), 0), C.plunge_rest())
  Bool.pick(S, Q.pit(w4, K.x(w4), K.y(w4)), K.set_act(w4, C.a_fall()), w4)

def plunging(+w: S, +id: U32) -> S:
  +w1 = K.set_z(K.set_inv(K.set_t(w, U32.add(K.t(w), 1)), U32.max(K.inv(w), 1)), N.smax(U32.sub(K.z(w), U32.mul(C.plunge_fall(), 256)), 0))
  Z.lw(U32.is_eq(K.z(w1), 0), u => plunge_land(w1, id), w1)

# ----- the lantern ---------------------------------------------------------------------------------------

def light_bolt(+x: U32, +y: U32, +ax: U32, +ay: U32, +id: U32) -> R.W8:
  R.W8{C.bk_light(), U32.add(x, U32.mul(ax, 8)), U32.add(y, U32.mul(ay, 8)), N.sdiv(U32.mul(ax, C.light_speed()), 256), N.sdiv(U32.mul(ay, C.light_speed()), 256), C.light_life(), id, 0}

# A tap of the lantern throws a bolt of light on the release, if the flame has the
# cost of one.
def lamp_throw(+w: S, +id: U32) -> S:
  +cost = U32.div(W.r_flame(w), C.bolt_cost_parts())
  +lp = Q.lock_point(w, K.lock_k(w), K.lock_i(w))
  +u = G.unit(U32.sub(Nr.x(lp), K.x(w)), U32.sub(Nr.y(lp), K.y(w)))
  +ax = Bool.pick(U32, U32.is_eq(Nr.ok(lp), 1), G.vx(u), K.fx(w))
  +ay = Bool.pick(U32, U32.is_eq(Nr.ok(lp), 1), G.vy(u), K.fy(w))
  +w1 = K.set_lamp_cd(K.set_flame(w, U32.sub(K.flame(w), cost)), C.lamp_rest())
  +w2 = Bool.pick(S, U32.is_lt(L.len8(W.bolts(w1)), C.bolt_cap()), W.set_bolts(w1, L.push8(W.bolts(w1), light_bolt(K.x(w), K.y(w), ax, ay, id))), w1)
  Bool.pick(S, U32.is_ge(K.flame(w), cost), Q.sound(w2, 34), Q.sound(w, 25))

def lamp_release(+w: S, +id: U32) -> S:
  +w1 = Z.lw(U32.is_ne(K.lamp(w), 0) && U32.is_lt(K.lamp(w), C.flare_after()) && U32.is_eq(K.lamp_cd(w), 0), u => lamp_throw(w, id), w)
  K.set_lamp(w1, 0)

# Held, the lantern builds to a flare and burns the tank; when the tank runs dry
# it sputters out and cannot flare for a bit.
def lamp_hold(+w: S) -> S:
  +lamp = U32.min(U32.add(K.lamp(w), 1), 255)
  +w1 = K.set_lamp(w, lamp)
  +w2 = Bool.pick(S, U32.is_eq(lamp, C.flare_after()) && U32.is_ne(K.flame(w1), 0), Q.sound(w1, 35), w1)
  +w3 = Bool.pick(S, Q.flaring(W.cur(w2)), K.set_flame(w2, M.dec(K.flame(w2))), w2)
  Bool.pick(S, Q.flaring(W.cur(w2)) && U32.is_eq(K.flame(w3), 0), Q.sound(K.set_lamp_cd(K.set_lamp(w3, 0), C.sputter()), 25), w3)

def lantern_on(+w: S, +id: U32, +mask: U32) -> S:
  Z.lz(U32.is_ne(U32.and(mask, C.b_lamp()), 0), u => lamp_hold(w), u => lamp_release(w, id))

def lantern(+w: S, +id: U32, +mask: U32) -> S:
  +act = K.act(w)
  Bool.pick(S, U32.is_ne(W.p_lantern(w), 0) && (U32.is_eq(act, C.a_free()) || U32.is_eq(act, C.a_slash()) || U32.is_eq(act, C.a_hop())), lantern_on(w, id, mask), K.set_lamp(w, 0))

# ----- the view ------------------------------------------------------------------------------------------

# How far the camera looks at a door or switch that came right: eased in and out.
def cue_on(+w: S) -> U32:
  +t = W.cue_t(w)
  Bool.pick(U32, U32.is_eq(W.cue_on(w), 0), 0, Bool.pick(U32, U32.is_gt(t, U32.sub(C.cue_ticks(), 8)), N.sdiv(U32.mul(U32.sub(C.cue_ticks(), t), 100), 8), Bool.pick(U32, U32.is_gt(t, 8), 100, N.sdiv(U32.mul(t, 100), 8))))

# Where the view wants to look: toward a lock, leaning toward the awake Warden
# unless it is the lock, then toward a cue.
def view_x(+w: S) -> U32:
  +lp = Q.lock_point(w, K.lock_k(w), K.lock_i(w))
  +kx = K.x(w)
  +t0 = Bool.pick(U32, U32.is_eq(Nr.ok(lp), 1), U32.add(kx, N.sdiv(U32.mul(U32.sub(Nr.x(lp), kx), C.lock_lean()), 100)), kx)
  +ty0 = Bool.pick(U32, U32.is_eq(Nr.ok(lp), 1), U32.add(K.y(w), N.sdiv(U32.mul(U32.sub(Nr.y(lp), K.y(w)), C.lock_lean()), 100)), K.y(w))
  +dx = U32.sub(B.x(w), t0)
  +dy = U32.sub(B.y(w), ty0)
  +d = N.smax(G.dist(dx, dy), 1)
  +pull = N.smin(N.sdiv(U32.mul(d, 70), 100), U32.mul(C.boss_pull(), 256))
  +lean = Bool.not(U32.is_eq(K.lock_k(w), C.lk_boss())) && Q.boss_up(w) && U32.is_ne(B.st(w), C.ws_asleep())
  +t1 = Bool.pick(U32, lean, U32.add(t0, N.sdiv(U32.mul(dx, pull), d)), t0)
  U32.add(t1, N.sdiv(U32.mul(U32.sub(W.cue_x(w), t1), cue_on(w)), 100))

def view_y(+w: S) -> U32:
  +lp = Q.lock_point(w, K.lock_k(w), K.lock_i(w))
  +kx = K.x(w)
  +t0 = Bool.pick(U32, U32.is_eq(Nr.ok(lp), 1), U32.add(kx, N.sdiv(U32.mul(U32.sub(Nr.x(lp), kx), C.lock_lean()), 100)), kx)
  +ty0 = Bool.pick(U32, U32.is_eq(Nr.ok(lp), 1), U32.add(K.y(w), N.sdiv(U32.mul(U32.sub(Nr.y(lp), K.y(w)), C.lock_lean()), 100)), K.y(w))
  +dx = U32.sub(B.x(w), t0)
  +dy = U32.sub(B.y(w), ty0)
  +d = N.smax(G.dist(dx, dy), 1)
  +pull = N.smin(N.sdiv(U32.mul(d, 70), 100), U32.mul(C.boss_pull(), 256))
  +lean = Bool.not(U32.is_eq(K.lock_k(w), C.lk_boss())) && Q.boss_up(w) && U32.is_ne(B.st(w), C.ws_asleep())
  +t1 = Bool.pick(U32, lean, U32.add(ty0, N.sdiv(U32.mul(dy, pull), d)), ty0)
  U32.add(t1, N.sdiv(U32.mul(U32.sub(W.cue_y(w), t1), cue_on(w)), 100))

# The view eases to the knight, leaning toward a lock; a fresh lock swings it in
# fast, a let-go eases it back slowly. It stays inside the room, or centred on one
# smaller than the picture.
def camera(+w: S) -> S:
  +cued = U32.is_ne(cue_on(w), 0)
  +locked = U32.is_ne(K.lock_k(w), 0)
  +rate = Bool.pick(U32, cued, 3, Bool.pick(U32, locked && U32.is_lt(K.lock_t(w), C.snap_ticks()), 6, Bool.pick(U32, locked, 3, Bool.pick(U32, U32.is_ne(K.unlock_t(w), 0), 2, 4))))
  +cx = U32.add(K.camx(w), N.sdiv(U32.mul(U32.sub(view_x(w), K.camx(w)), rate), 8))
  +cy = U32.add(K.camy(w), N.sdiv(U32.mul(U32.sub(view_y(w), K.camy(w)), rate), 8))
  +rw = U32.mul(Gd.width(W.grid(w)), 4096)
  +rh = U32.mul(Gd.height(W.grid(w)), 4096)
  K.set_camy(K.set_camx(w, Bool.pick(U32, N.sle(rw, U32.mul(2, 49152)), N.sdiv(rw, 2), N.sclamp(cx, 49152, U32.sub(rw, 49152)))), Bool.pick(U32, N.sle(rh, U32.mul(2, 28672)), N.sdiv(rh, 2), N.sclamp(cy, 28672, U32.sub(rh, 28672))))

# ----- one tick of one knight --------------------------------------------------------------------------------

# Things that go on whatever the knight is doing: the last safe footing, the low-
# heart beep, and what E would do.
def ledge_near(+w: S, +tx: U32, +ty: U32) -> Bool:
  U32.is_eq(Q.cls(w, U32.sub(tx, 1), U32.sub(ty, 1)), 3) || U32.is_eq(Q.cls(w, tx, U32.sub(ty, 1)), 3) || U32.is_eq(Q.cls(w, U32.add(tx, 1), U32.sub(ty, 1)), 3) || U32.is_eq(Q.cls(w, U32.sub(tx, 1), ty), 3) || U32.is_eq(Q.cls(w, tx, ty), 3) || U32.is_eq(Q.cls(w, U32.add(tx, 1), ty), 3) || U32.is_eq(Q.cls(w, U32.sub(tx, 1), U32.add(ty, 1)), 3) || U32.is_eq(Q.cls(w, tx, U32.add(ty, 1)), 3) || U32.is_eq(Q.cls(w, U32.add(tx, 1), U32.add(ty, 1)), 3)

def safe_check(+w: S) -> S:
  +x = K.x(w)
  +y = K.y(w)
  Z.lw(Bool.not(Q.pit(w, x, y)) && Bool.not(ledge_near(w, G.tile_of(x), G.tile_of(y))) && Q.clear(w, x, y, 1280), u => K.set_safey(K.set_safex(w, x), y), w)

def safe_ground(+w: S) -> S: Z.lw(U32.is_eq(K.act(w), C.a_free()) && U32.is_eq(K.z(w), 0), u => safe_check(w), w)

def beep(+w: S) -> S:
  +b = U32.mod(U32.add(K.beep(w), 1), U32.mul(C.beep_every(), 4))
  Bool.pick(S, Q.up(W.cur(w)) && U32.is_le(K.hp(w), C.low_hp()),
    Bool.pick(S, U32.is_eq(U32.mod(b, C.beep_every()), 1), Q.sound(K.set_beep(w, b), 25), K.set_beep(w, b)),
    K.set_beep(w, 0))

def set_ctx(+w: S, +c: R.W8) -> S: K.set_ctx_i(K.set_ctx_k(w, Ct.k(c)), Ct.i(c))

def upkeep(+w: S) -> S:
  +w1 = beep(safe_ground(w))
  Z.lz(U32.is_eq(K.act(w1), C.a_free()), u => set_ctx(w1, X.context(w1)), u => K.set_ctx_i(K.set_ctx_k(w1, 0), 0))

# ----- the buttons ----------------------------------------------------------------------------------------------

def shield_tick(+w: S, +mask: U32) -> S:
  +can_guard = U32.is_eq(K.carry(w), 0) && U32.is_eq(K.z(w), 0)
  +raise = U32.is_ne(U32.and(mask, C.b_guard()), 0) && U32.is_eq(K.act(w), C.a_free()) && can_guard && U32.is_eq(K.guard_cd(w), 0)
  Bool.pick(S, raise,
    K.set_shield(Bool.pick(S, U32.is_eq(K.shield(w), 0), Q.sound(w, 31), w), U32.min(U32.add(K.shield(w), 1), 255)),
    K.set_shield(Bool.pick(S, U32.is_ne(K.shield(w), 0) && U32.is_eq(K.act(w), C.a_free()), K.set_guard_cd(w, C.guard_rest()), w), 0))

# The sword button held builds the spin.
def charge_tick(+w: S, +mask: U32) -> S:
  +c = U32.min(U32.add(K.charge(w), 1), 255)
  +w1 = K.set_charge(w, c)
  Bool.pick(S, U32.is_ne(U32.and(mask, C.b_sword()), 0) && U32.is_eq(K.shield(w), 0), Bool.pick(S, U32.is_eq(c, C.charge_ticks()), Q.fx(Q.sound(w1, 35), C.fxk_spark(), K.x(w1), U32.sub(K.y(w1), 2048), 1), w1), w)

# A swing's recovery gives way to a dodge or the shield: the knight is never locked
# into a blow while something is coming.
def break_swing(+w: S, +press: U32, +mask: U32) -> S:
  Bool.pick(S, recovering(w) && (U32.is_ne(U32.and(press, C.b_ctx()), 0) || U32.is_ne(U32.and(mask, C.b_guard()), 0)), K.set_queued(K.set_step(K.set_chain(K.set_t(K.set_act(w, C.a_free()), 0), 0), 0), 0), w)

# Free, with the shield down or up: E meets the world, the sword swings or
# lunges, E with nothing to meet dodges, X lays a bomb.
def lay_bomb(+w: S, +id: U32) -> S:
  +x = U32.add(K.x(w), U32.mul(K.fx(w), 12))
  +y = U32.add(K.y(w), U32.mul(K.fy(w), 12))
  +w1 = K.set_bombs(w, U32.sub(K.bombs(w), 1))
  Q.sound(Bool.pick(S, U32.is_lt(L.len8(W.fuses(w1)), C.fuse_cap()), W.set_fuses(w1, L.push8(W.fuses(w1), R.W8{x, y, C.fuse_ticks(), id, 0, 0, 0, 0})), w1), 2)

def dodge_or(+used: Bool, +w: S, +ix: R.W8) -> S:
  match used:
    case True{}:
      w
    case False{}:
      begin_dodge(w, ix)

def free_ctx(+w: S, +id: U32, +ix: R.W8) -> S:
  +w1 = X.use_context(w, id)
  dodge_or(U32.is_ne(W.ret(w1), 0), w1, ix)

def free_bomb(+go: Bool, +w: S, +id: U32) -> S:
  match go:
    case True{}:
      lay_bomb(w, id)
    case False{}:
      w

def swing_sword(+w: S, +ix: R.W8) -> S:
  +lp = Q.lock_point(w, K.lock_k(w), K.lock_i(w))
  +v = G.unit(Ix.mx(ix), Ix.my(ix))
  +u = G.unit(U32.sub(Nr.x(lp), K.x(w)), U32.sub(Nr.y(lp), K.y(w)))
  +forward = U32.is_ne(K.lock_k(w), 0) && U32.is_eq(Nr.ok(lp), 1) && (U32.is_ne(G.vx(v), 0) || U32.is_ne(G.vy(v), 0)) && N.sge(G.dot(G.vx(v), G.vy(v), G.vx(u), G.vy(u)), C.lunge_forward())
  Bool.pick(S, forward && U32.is_eq(K.chain(w), 0), begin_lunge(w), begin_slash(w, Bool.pick(U32, U32.is_ne(K.chain(w), 0), K.step(w), 0)))

def free_press(+w: S, +id: U32, +press: U32, +ix: R.W8) -> S:
  Z.lz(U32.is_ne(U32.and(press, C.b_sword()), 0) && U32.is_eq(K.carry(w), 0), u => swing_sword(w, ix),
    u => Z.lz(U32.is_ne(U32.and(press, C.b_ctx()), 0), u => free_ctx(w, id, ix),
      u => free_bomb(U32.is_ne(U32.and(press, C.b_bomb()), 0) && U32.is_ne(K.bombs(w), 0) && U32.is_eq(K.carry(w), 0), w, id)))

def free(+w: S, +id: U32, +press: U32, +ix: R.W8) -> S:
  Z.lz(U32.is_ne(K.shield(w), 0), u => Z.lw(U32.is_ne(U32.and(press, C.b_ctx()), 0), u => X.use_context(w, id), w), u => free_press(w, id, press, ix))

def act_verb(+ds: Nat, +w: S, +id: U32, +press: U32, +ix: R.W8) -> S:
  match ds:
    case 0n:
      free(w, id, press, ix)
    case 1n+a1:
      match a1:
        case 0n:
          swinging(w, id, press)
        case 1n+a2:
          match a2:
            case 0n:
              swinging(w, id, press)
            case 1n+a3:
              match a3:
                case 0n:
                  swinging(w, id, press)
                case 1n+a4:
                  match a4:
                    case 0n:
                      rolling(w)
                    case 1n+a5:
                      match a5:
                        case 0n:
                          hopping(w, press)
                        case 1n+a6:
                          match a6:
                            case 0n:
                              plunging(w, id)
                            case 1n+a7:
                              w

# Letting the sword go ends the winding; a wound spin goes off.
def release_sword(+w: S, +mask: U32) -> S:
  Bool.pick(S, U32.is_eq(U32.and(mask, C.b_sword()), 0),
    K.set_charge(Bool.pick(S, U32.is_ge(K.charge(w), C.charge_ticks()) && U32.is_eq(K.act(w), C.a_free()) && U32.is_eq(K.shield(w), 0), begin_spin(w), w), 0),
    w)

def verbs(+w: S, +id: U32, +mask: U32, +press: U32, +ix: R.W8) -> S:
  +w1 = break_swing(charge_tick(shield_tick(w, mask), mask), press, mask)
  lantern(release_sword(act_verb(U32.to_nat(K.act(w1)), w1, id, press, ix), mask), id, mask)

# ----- a knight's tick --------------------------------------------------------------------------------------------

# The counters that run down every tick whatever the knight does.
def run_down(+w: S, +mask: U32) -> S:
  +w1 = K.set_held(w, mask)
  +w2 = K.set_chain(K.set_guard_cd(K.set_taper(K.set_ward(K.set_inv(w1, M.dec(K.inv(w1))), M.dec(K.ward(w1))), M.dec(K.taper(w1))), M.dec(K.guard_cd(w1))), M.dec(K.chain(w1)))
  +w3 = K.set_lamp_cd(w2, M.dec(K.lamp_cd(w1)))
  +w4 = Bool.pick(S, U32.is_eq(K.chain(w3), 0) && U32.is_eq(K.queued(w3), 0), K.set_step(w3, 0), w3)
  +w5 = K.set_lock_t(K.set_unlock_t(w4, M.dec(K.unlock_t(w4))), U32.min(U32.add(K.lock_t(w4), 1), 255))
  +w6 = Bool.pick(S, U32.is_ne(K.read_on(w5), 0), K.set_read_t(w5, M.dec(K.read_t(w5))), w5)
  Bool.pick(S, U32.is_ne(K.read_on(w6), 0) && U32.is_eq(K.read_t(w6), 0), K.set_read_on(w6, 0), w6)

def acting(+w: S, +id: U32, +mask: U32, +press: U32, +ix: R.W8) -> S:
  verbs(face_and_move(lock_on(w, ix, press), ix), id, mask, press, ix)

def down_tick(+w: S) -> S: K.set_shield(K.set_lock_i(K.set_lock_k(K.set_t(w, U32.min(U32.add(K.t(w), 1), 4000)), 0), 0), 0)

def item_tick(+w: S) -> S:
  +w1 = K.set_t(w, U32.add(K.t(w), 1))
  Bool.pick(S, U32.is_ge(K.t(w1), C.item_ticks()), K.set_t(K.set_item_k(K.set_act(w1, C.a_free()), 0), 0), w1)

def seat_acts(+ds: Nat, +w: S, +id: U32, +mask: U32, +press: U32, +ix: R.W8) -> S:
  match ds:
    case 0n:
      acting(w, id, mask, press, ix)
    case 1n+a1:
      match a1:
        case 0n:
          acting(w, id, mask, press, ix)
        case 1n+a2:
          match a2:
            case 0n:
              acting(w, id, mask, press, ix)
            case 1n+a3:
              match a3:
                case 0n:
                  acting(w, id, mask, press, ix)
                case 1n+a4:
                  match a4:
                    case 0n:
                      acting(w, id, mask, press, ix)
                    case 1n+a5:
                      match a5:
                        case 0n:
                          acting(w, id, mask, press, ix)
                        case 1n+a6:
                          match a6:
                            case 0n:
                              acting(w, id, mask, press, ix)
                            case 1n+a7:
                              match a7:
                                case 0n:
                                  hurting(w)
                                case 1n+a8:
                                  match a8:
                                    case 0n:
                                      item_tick(w)
                                    case 1n+a9:
                                      match a9:
                                        case 0n:
                                          down_tick(w)
                                        case 1n+a10:
                                          match a10:
                                            case 0n:
                                              falling(w)
                                            case 1n+a11:
                                              acting(w, id, mask, press, ix)

def seat_live(+w: S, +id: U32, +mask: U32, +press: U32, +ix: R.W8) -> S:
  camera(upkeep(seat_acts(U32.to_nat(K.act(w)), w, id, mask, press, ix)))

# A blade that landed holds its owner a moment: the knight's own hit-stop, so six
# knights never freeze each other.
def seat_frozen(+frozen: Bool, +w: S, +id: U32, +mask: U32, +press: U32, +ix: R.W8) -> S:
  match frozen:
    case True{}:
      camera(K.set_freeze(w, M.dec(K.freeze(w))))
    case False{}:
      seat_live(w, id, mask, press, ix)

def seat_here(+found: Bool, +w: S, +id: U32, +ix: R.W8) -> S:
  match found:
    case False{}:
      w
    case True{}:
      +mask = Ix.mask(ix)
      +press = U32.and(mask, U32.not(K.held(w)))
      +w1 = refill(run_down(w, mask))
      H.put(seat_frozen(U32.is_ne(K.freeze(w1), 0), w1, id, mask, press, ix))

# One tick of one knight.
def step_seat(+w: S, +id: U32, +ix: R.W8) -> S:
  +w1 = H.take(w, id)
  seat_here(U32.is_ne(W.ret(w1), 0), w1, id, ix)
