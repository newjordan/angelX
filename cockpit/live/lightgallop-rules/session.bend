import Base
import ./math.bend as M
import ./sl.bend as S
import ./hs.bend as H
import ./rs.bend as R
import ./terrain.bend as T
import ./road.bend as O
import ./level.bend as L
import ./rider.bend as D
import ./world.bend as W

# The party's ride: every seated knight on the Moonwell Road at once, one
# Dimming behind them, one set of foes and lanterns ahead.
#
# Each knight has the whole set of moves (rider.bend) and their own camera and
# spark count; the road is shared: a foe popped is popped for everyone (and
# re-forms a while later, so a pack behind has something to chain), a lantern
# post any knight lights is lit for all and holds the Dimming back, and a fallen
# knight is carried to the last lit post. More knights at the table means more
# foes on the road and a quicker Dimming, never weaker foes.

# A run never runs past an hour; a longer one is not a run to play.
def limit() -> U32:
  108000

def none() -> U32:
  4294967295

# ---- the seats -----------------------------------------------------------------

# The base of knight `id`'s seat (ids 1 to 6).
def sb(+id: U32) -> U32:
  (768 + (id - 1) * 192 : U32)

def seated(+st: S.Sl, +id: U32) -> Bool:
  M.nz(R.id(st, sb(id)))

def here(+st: S.Sl, +id: U32) -> Bool:
  M.nz(S.g(st, (111 + id : U32)))

# How many knights are seated.
def count(+st: S.Sl) -> U32:
  (M.b2u(seated(st, 1)) + M.b2u(seated(st, 2)) + M.b2u(seated(st, 3)) + M.b2u(seated(st, 4)) + M.b2u(seated(st, 5)) + M.b2u(seated(st, 6)) : U32)

# What knight `id` pressed: the run, and whether down, jump and spur are held.
def key_x(+st: S.Sl, +id: U32) -> U32:
  S.g(st, (114 + id * 6 : U32))

def key_down(+st: S.Sl, +id: U32) -> U32:
  M.b2u(Bool.or(M.gt(S.g(st, (115 + id * 6 : U32)), 0), M.gt(S.g(st, (116 + id * 6 : U32)), 0)))

def key_jump(+st: S.Sl, +id: U32) -> U32:
  M.b2u(Bool.or(M.nz(S.g(st, (117 + id * 6 : U32))), M.lt(S.g(st, (115 + id * 6 : U32)), 0)))

def key_spur(+st: S.Sl, +id: U32) -> U32:
  M.b2u(Bool.or(M.nz(S.g(st, (118 + id * 6 : U32))), M.nz(S.g(st, (119 + id * 6 : U32)))))

def key_interact(+st: S.Sl, +id: U32) -> U32:
  S.g(st, (117 + id * 6 : U32))

# ---- the posts -----------------------------------------------------------------

def post_lit(+st: S.Sl, +n: U32) -> Bool:
  M.nz(U32.and(U32.shrn(H.posts(st), U32.to_nat(n)), 1))

# The highest post lit, if any (4294967295 if none).
def last_post(+st: S.Sl) -> U32:
  M.pk(5, M.pk(4, M.pk(3, M.pk(2, M.pk(1, M.pk(0, none(), post_lit(st, 0)), post_lit(st, 1)), post_lit(st, 2)), post_lit(st, 3)), post_lit(st, 4)), post_lit(st, 5))

def post_go(+ps: List<&2, L.Post>, +n: Nat) -> L.Post:
  match ps n:
    case Nil{} _:
      L.Post{0, 0, 0, 0, 0}
    case Con{h, t} 0n:
      h
    case Con{h, t} 1n+k:
      post_go(t, k)

def post_nth(+rd: L.Road, +n: U32) -> L.Post:
  post_go(O.ro_posts(rd), U32.to_nat(n))

def post_x(+p: L.Post) -> U32:
  match p:
    case L.Post{x, y, path, s, hold}:
      x

def post_y(+p: L.Post) -> U32:
  match p:
    case L.Post{x, y, path, s, hold}:
      y

def post_path(+p: L.Post) -> U32:
  match p:
    case L.Post{x, y, path, s, hold}:
      path

def post_s(+p: L.Post) -> U32:
  match p:
    case L.Post{x, y, path, s, hold}:
      s

def post_hold(+p: L.Post) -> U32:
  match p:
    case L.Post{x, y, path, s, hold}:
      hold

# How far back post `n` holds the Dimming when it is lit, pixels: less for a
# fuller table. `seats` is how many knights are in `seats` just now.
def hold_of(+rd: L.Road, +n: U32, +seats: U32) -> U32:
  (post_hold(post_nth(rd, n)) - 25 * M.max((seats - 1 : U32), 0) : U32)

# Which act the road is in at column `x` (pixels).
def act_at(+rd: L.Road, +x: U32) -> U32:
  M.pk(3, M.pk(2, M.pk(1, 0, M.ge(x, O.ro_a1(rd))), M.ge(x, O.ro_a2(rd))), M.ge(x, O.ro_a3(rd)))

# ---- the foes ------------------------------------------------------------------

# Whether foe `i` is on the road for this many knights.
def awake(+st: S.Sl, +f: L.Fs) -> Bool:
  (W.fs_min_seats(f) <= H.provisioned(st) : U32)

def warden_go(+fs: List<&2, L.Fs>, +i: U32) -> U32:
  match fs:
    case Nil{}:
      none()
    case Con{h, t}:
      M.pk(i, warden_go(t, (i + 1 : U32)), M.eq(W.fs_kind(h), 3))

# The warden's index among the foes.
def warden_index(+rd: L.Road) -> U32:
  warden_go(O.ro_foes(rd), 0)

def foe_go(+fs: List<&2, L.Fs>, +n: Nat) -> L.Fs:
  match fs n:
    case Nil{} _:
      L.Fs{2, 0, 0, 0, 0, 0, 0}
    case Con{h, t} 0n:
      h
    case Con{h, t} 1n+k:
      foe_go(t, k)

def foe_nth(+rd: L.Road, +i: U32) -> L.Fs:
  foe_go(O.ro_foes(rd), U32.to_nat(i))

# The warden takes one more hit for each knight who sits down; hits already
# taken stay taken.
def grow_go(+fs: List<&2, L.Fs>, +i: U32, +st: S.Sl, +add: U32) -> S.Sl:
  match fs:
    case Nil{}:
      st
    case Con{h, t}:
      +grow = Bool.and(M.eq(W.fs_kind(h), 3), M.gt(W.foe_hp(st, i), 0))
      grow_go(t, (i + 1 : U32), S.pks(grow, W.set_foe(st, i, M.min((W.foe_hp(st, i) + add : U32), 8), W.foe_pop(st, i), W.foe_flinch(st, i)), st), add)

def grow_warden(+rd: L.Road, +st: S.Sl, +add: U32) -> S.Sl:
  grow_go(O.ro_foes(rd), 0, st, add)

def near_seat(+st: S.Sl, +id: U32, +fx: U32) -> Bool:
  Bool.and(seated(st, id), M.lt(M.abs((M.div(R.x(st, sb(id)), 256) - fx : U32)), 160))

def near_knight(+rd: L.Road, +st: S.Sl, +fx: U32) -> Bool:
  Bool.or(Bool.or(Bool.or(near_seat(st, 1, fx), near_seat(st, 2, fx)), Bool.or(near_seat(st, 3, fx), near_seat(st, 4, fx))), Bool.or(near_seat(st, 5, fx), near_seat(st, 6, fx)))

# Foes' own clocks: flinches fade, smoke clears, a popped foe comes back if no
# knight is by.
# A popped foe's smoke has cleared: it comes back if no knight is by, and the
# smoke starts again if one is.
def revive(+rd: L.Road, +st: S.Sl, +i: U32, +f: L.Fs, +hp: U32, +flinch: U32) -> S.Sl:
  +stay = near_knight(rd, st, W.foe_x(rd, f, H.elapsed(st)))
  W.set_foe(st, i, M.pk(hp, 1, stay), M.pk(20, 0, stay), flinch)

def back_or_not(+back: Bool, +rd: L.Road, +st: S.Sl, +i: U32, +f: L.Fs, +hp: U32, +pop: U32, +flinch: U32) -> S.Sl:
  match back:
    case True{}:
      revive(rd, st, i, f, hp, flinch)
    case False{}:
      W.set_foe(st, i, hp, pop, flinch)

def foes_tick_live(+rd: L.Road, +st: S.Sl, +i: U32, +f: L.Fs) -> S.Sl:
  +hp = W.foe_hp(st, i)
  +pop = W.foe_pop(st, i)
  +warden = M.eq(W.fs_kind(f), 3)
  +falling = Bool.and(M.eq(hp, 0), Bool.and(warden, Bool.and(M.gt(pop, 0), (pop < 90 : U32))))
  +pop1 = M.pk((pop + 1 : U32), pop, falling)
  +smoke = Bool.and(M.eq(hp, 0), Bool.and(Bool.not(warden), M.gt(pop, 0)))
  +pop2 = M.pk(M.sat1(pop1), pop1, smoke)
  back_or_not(Bool.and(smoke, M.eq(pop2, 0)), rd, st, i, f, hp, pop2, M.sat1(W.foe_flinch(st, i)))

def foes_tick_pick(+rest: Bool, +rd: L.Road, +st: S.Sl, +i: U32, +f: L.Fs) -> S.Sl:
  match rest:
    case True{}:
      st
    case False{}:
      foes_tick_live(rd, st, i, f)

# A foe with no smoke and no flinch has nothing to tick.
def foes_tick_one(+rd: L.Road, +st: S.Sl, +i: U32, +f: L.Fs) -> S.Sl:
  foes_tick_pick(Bool.and(M.eq(W.foe_pop(st, i), 0), M.eq(W.foe_flinch(st, i), 0)), rd, st, i, f)

def foes_tick_go(+rd: L.Road, +fs: List<&2, L.Fs>, +i: U32, +st: S.Sl) -> S.Sl:
  match fs:
    case Nil{}:
      st
    case Con{h, t}:
      foes_tick_go(rd, t, (i + 1 : U32), foes_tick_one(rd, st, i, h))

def tick_foes(+rd: L.Road, +st: S.Sl) -> S.Sl:
  foes_tick_go(rd, O.ro_foes(rd), 0, H.set_warden_inv(st, M.sat1(H.warden_inv(st))))

# ---- the lance-lock ----------------------------------------------------------------

# The lock flies at its foe: point the rider's speed at the foe's middle.
def steer_go(+rd: L.Road, +st: S.Sl, +b: U32, +f: L.Fs) -> S.Sl:
  +fx = W.foe_x(rd, f, H.elapsed(st))
  +fy = W.foe_y(rd, f, H.elapsed(st), fx)
  +cen = D.centre(rd, st, b)
  +dx = (fx * 256 - D.cn_x(cen) : U32)
  +dy = ((fy - U32.div(W.body_h(W.fs_kind(f)), 2) : U32) * 256 - D.cn_y(cen) : U32)
  +s = R.set_vx(st, b, M.div((M.unit_x(dx, dy) * 3072 : U32), 4096))
  +s = R.set_vy(s, b, M.div((M.unit_y(dx, dy) * 3072 : U32), 4096))
  +s = R.set_dir(s, b, M.pk(M.sgn(dx), R.dir(st, b), M.gt(M.abs(dx), 256)))
  R.set_lock_ticks(s, b, M.sat1(R.lock_ticks(st, b)))

def steer_drop(+st: S.Sl, +b: U32) -> S.Sl:
  R.set_lock_cd(R.set_lock_on(st, b, 0), b, 8)

def steer_pick2(+drop: Bool, +rd: L.Road, +st: S.Sl, +b: U32, +f: L.Fs) -> S.Sl:
  match drop:
    case True{}:
      steer_drop(st, b)
    case False{}:
      steer_go(rd, st, b, f)

def steer_pick(+on: Bool, +drop: Bool, +rd: L.Road, +st: S.Sl, +b: U32, +f: L.Fs) -> S.Sl:
  match on:
    case True{}:
      steer_pick2(drop, rd, st, b, f)
    case False{}:
      st

# Point a lance-lock in flight at its foe, or give it up.
def steer_lock(+rd: L.Road, +st: S.Sl, +b: U32) -> S.Sl:
  +i = R.lock_target(st, b)
  +f = foe_nth(rd, i)
  +drop = Bool.or(M.ge(i, S.g(st, 106)), Bool.or(M.eq(W.foe_hp(st, i), 0), Bool.or(Bool.not(awake(st, f)), M.eq(R.lock_ticks(st, b), 0))))
  steer_pick(D.locked_on(st, b), drop, rd, st, b, f)

# The nearest foe so far: a flag, the distance squared and the foe.
type Bl is Data:
  Bl{ok: U32, d2: Nat, i: U32}

def bl_ok(+x: Bl) -> U32:
  match x:
    case Bl{ok, d2, i}:
      ok

def bl_d2(+x: Bl) -> Nat:
  match x:
    case Bl{ok, d2, i}:
      d2

def bl_i(+x: Bl) -> U32:
  match x:
    case Bl{ok, d2, i}:
      i

# One foe a lance might lock: ahead of the horse (a little behind still counts),
# inside a cone about sixty degrees to either side of level, within the reach.
def lock_try(+rd: L.Road, +st: S.Sl, +f: L.Fs, +i: U32, +cen: D.Cn, +dir: U32, +reach: U32, +best: Bl) -> Bl:
  +fx = W.foe_x(rd, f, H.elapsed(st))
  +fy = W.foe_y(rd, f, H.elapsed(st), fx)
  +dx = (fx * 256 - D.cn_x(cen) : U32)
  +dy = ((fy - U32.div(W.body_h(W.fs_kind(f)), 2) : U32) * 256 - D.cn_y(cen) : U32)
  +d2 = M.len2(dx, dy)
  +r = Nat.mul(U32.to_nat(reach), 256n)
  +ok = Bool.and(Bool.not(M.eq(W.fs_kind(f), 2)), Bool.and(awake(st, f), Bool.and(M.gt(W.foe_hp(st, i), 0),
        Bool.and(Bool.not(M.lt((dx * dir : U32), M.neg(1536))),
        Bool.and(Bool.not(M.gt(M.abs(dy), (M.abs(dx) * 2 + 6144 : U32))),
                 Bool.and(Bool.not(Nat.is_gt(d2, Nat.mul(r, r))), Bool.or(Bool.not(M.nz(bl_ok(best))), Nat.is_lt(d2, bl_d2(best)))))))))
  Bool.pick(Bl, ok, Bl{1, d2, i}, best)

def lock_step(+in_window: Bool, +rd: L.Road, +st: S.Sl, +f: L.Fs, +i: U32, +cen: D.Cn, +dir: U32, +reach: U32, +best: Bl) -> Bl:
  match in_window:
    case True{}:
      lock_try(rd, st, f, i, cen, dir, reach, best)
    case False{}:
      best

def lock_go(+rd: L.Road, +st: S.Sl, +fs: List<&2, L.Fs>, +i: U32, +cen: D.Cn, +dir: U32, +reach: U32, +cx: U32, +best: Bl) -> Bl:
  match fs:
    case Nil{}:
      best
    case Con{f, rest}:
      +w = (reach + 60 : U32)
      +near = Bool.and(M.ge(W.fs_x(f), (cx - w : U32)), M.le(W.fs_x(f), (cx + w : U32)))
      lock_go(rd, st, rest, (i + 1 : U32), cen, dir, reach, cx, lock_step(near, rd, st, f, i, cen, dir, reach, best))

# The nearest foe in the lance's cone for a rider in the air (4294967295 if there
# is none).
def find_lock(+rd: L.Road, +st: S.Sl, +b: U32) -> U32:
  +cen = D.centre(rd, st, b)
  +best = lock_go(rd, st, O.ro_foes(rd), 0, cen, R.dir(st, b), H.lock_reach_of(st), M.div(D.cn_x(cen), 256), Bl{0, 0n, 0})
  M.pk(bl_i(best), none(), M.nz(bl_ok(best)))

# A press of jump in the air with no leap left: lock the nearest foe in the
# lance's cone, or lunge once.
def lock_begin(+rd: L.Road, +st: S.Sl, +b: U32) -> S.Sl:
  +i = find_lock(rd, st, b)
  +found = Bool.not(M.eq(i, none()))
  +s = H.snd(R.set_lock_ticks(R.set_lock_target(R.set_lock_on(st, b, 1), b, i), b, 14), H.k_swing())
  +lunge = H.snd(R.set_lunged(R.set_lunge(st, b, 8), b, 1), H.k_swing())
  S.pks(found, s, S.pks(Bool.not(M.nz(R.lunged(st, b))), lunge, st))

def lock_begin_pick(+can: Bool, +rd: L.Road, +st: S.Sl, +b: U32) -> S.Sl:
  match can:
    case True{}:
      lock_begin(rd, st, b)
    case False{}:
      st

def begin_lock(+rd: L.Road, +st: S.Sl, +b: U32) -> S.Sl:
  lock_begin_pick(Bool.not(Bool.or(M.gt(R.lock_cd(st, b), 0), Bool.or(D.locked_on(st, b), D.grounded(st, b)))), rd, st, b)

# ---- the warden's wall and the foes' blows ----------------------------------------------

# The gate's warden bars the road: nothing rides past it alive.
def wall_hold(+rd: L.Road, +st: S.Sl, +b: U32, +limit: U32) -> S.Sl:
  +p = D.path_of(rd, st, b)
  +s = M.min(T.s_at_x(p, limit), R.ground_s(st, b))
  +at = T.sample(p, s)
  +on = R.set_gsp(R.set_y(R.set_x(R.set_ground_s(st, b, s), b, T.sm_x(at)), b, T.sm_y(at)), b, M.min(R.gsp(st, b), 0))
  +off = R.set_vx(R.set_x(st, b, (limit * 256 : U32)), b, M.min(R.vx(st, b), 0))
  S.pks(D.grounded(st, b), on, off)

def wall_pick(+bar: Bool, +rd: L.Road, +st: S.Sl, +b: U32, +limit: U32) -> S.Sl:
  match bar:
    case True{}:
      wall_hold(rd, st, b, limit)
    case False{}:
      st

def wall(+rd: L.Road, +st: S.Sl, +b: U32) -> S.Sl:
  +wi = warden_index(rd)
  +w = foe_nth(rd, wi)
  +limit = (W.fs_x(w) + 6 : U32)
  wall_pick(Bool.and(Bool.not(M.eq(wi, none())), Bool.and(M.gt(W.foe_hp(st, wi), 0), M.gt(M.div(R.x(st, b), 256), limit))), rd, st, b, limit)

# A blow lands on the rider from a foe at column `from`: a shield-spark takes it,
# or sparks fly, or he is down.
def hurt(+st: S.Sl, +b: U32, +from: U32) -> S.Sl:
  +kind = D.blow(st, b)
  +s = D.knock(st, b, (from * 256 : U32))
  +s = H.snd_if(s, U32.is_eq(kind, 1), H.k_shield_block())
  +s = H.snd_if(H.snd_if(s, U32.is_eq(kind, 2), H.k_hero_hurt()), U32.is_eq(kind, 2), H.k_spoil_pickup())
  +s = H.snd_if(R.set_shake(s, b, M.pk(12, R.shake(s, b), U32.is_eq(kind, 3))), U32.is_eq(kind, 3), H.k_knight_fall())
  s

# A foe popped into cold smoke and a spark of returned light.
def pop(+st: S.Sl, +b: U32, +i: U32) -> S.Sl:
  +air = D.link(st, b)
  +s = W.set_foe(st, i, 0, 180, 8)
  +s = D.strike(s, b)
  +s = R.set_freeze(s, b, 2)
  +s = R.set_collected(R.set_sparks(s, b, (R.sparks(s, b) + 1 : U32)), b, (R.collected(s, b) + 1 : U32))
  +chain = R.chain(s, b)
  +link = R.set_gauge(s, b, M.min((R.gauge(s, b) + 140 * M.min(chain, 5) : U32), 1000))
  +link = H.snd(link, H.k_chain(M.min(M.sat1(chain), 4)))
  +plain = R.set_gauge(s, b, M.min((R.gauge(s, b) + 60 : U32), 1000))
  +plain = H.snd(plain, H.k_kill())
  S.pks(air, link, plain)

# Every other seated knight is given the light of the warden's fall.
def light_one(+st: S.Sl, +id: U32, +self_id: U32) -> S.Sl:
  +b = sb(id)
  +s = R.set_collected(R.set_sparks(st, b, (R.sparks(st, b) + 10 : U32)), b, (R.collected(st, b) + 10 : U32))
  S.pks(Bool.and(seated(st, id), Bool.not(M.eq(id, self_id))), R.set_gauge(s, b, 1000), st)

def light_all(+st: S.Sl, +self_id: U32) -> S.Sl:
  light_one(light_one(light_one(light_one(light_one(light_one(st, 1, self_id), 2, self_id), 3, self_id), 4, self_id), 5, self_id), 6, self_id)

# The warden takes a blow, if it is not shrugging them off.
def hit_warden_go(+st: S.Sl, +b: U32, +id: U32, +i: U32) -> S.Sl:
  +hp = M.sat1(W.foe_hp(st, i))
  +s = H.set_warden_inv(st, 22)
  +s = W.set_foe(s, i, hp, W.foe_pop(s, i), 12)
  +s = D.strike(s, b)
  +s = R.set_gauge(R.set_shake(R.set_freeze(s, b, 4), b, 10), b, M.min((R.gauge(s, b) + 120 : U32), 1000))
  +fall = W.set_foe(s, i, 0, 1, 12)
  +fall = H.snd(fall, H.k_boss_death())
  +fall = R.set_gauge(R.set_collected(R.set_sparks(fall, b, (R.sparks(fall, b) + 10 : U32)), b, (R.collected(fall, b) + 10 : U32)), b, 1000)
  S.pks(M.eq(hp, 0), light_all(fall, id), H.snd(s, H.k_boss_roar()))

def hit_warden(+st: S.Sl, +b: U32, +id: U32, +i: U32) -> S.Sl:
  S.pks(M.gt(H.warden_inv(st), 0), st, hit_warden_go(st, b, id, i))

# ---- what the rider's swept body touches ------------------------------------------------

# The box a rider swept between two centres, half a body each side.
type Bx is Data:
  Bx{lox: U32, hix: U32, loy: U32, hiy: U32}

def bx_of(+c0: D.Cn, +c1: D.Cn, +h: U32) -> Bx:
  Bx{(M.min(D.cn_x(c0), D.cn_x(c1)) - 1792 : U32), (M.max(D.cn_x(c0), D.cn_x(c1)) + 1792 : U32),
     (M.min(D.cn_y(c0), D.cn_y(c1)) - M.div((h * 256 : U32), 2) : U32), (M.max(D.cn_y(c0), D.cn_y(c1)) + M.div((h * 256 : U32), 2) : U32)}

# Whether the box meets the rectangle (left, right, top, bottom) in pixels.
def over(+x: Bx, +l: U32, +r: U32, +t: U32, +bt: U32) -> Bool:
  match x:
    case Bx{lox, hix, loy, hiy}:
      Bool.and(Bool.and(M.ge(hix, (l * 256 : U32)), M.le(lox, (r * 256 : U32))), Bool.and(M.ge(hiy, (t * 256 : U32)), M.le(loy, (bt * 256 : U32))))

# The warden throws a rider back: nothing passes.
def warden_back(+st: S.Sl, +b: U32) -> S.Sl:
  +s = R.set_roll(R.set_ground_on(st, b, 0), b, 0)
  +s = R.set_vy(R.set_vx(s, b, (M.neg(R.dir(st, b)) * 700 : U32)), b, M.neg(900))
  R.set_ctrl(s, b, M.max(R.ctrl(st, b), 4))

# The rider's body meets foe `f`, which is awake and standing.
def foe_meets(+st: S.Sl, +b: U32, +id: U32, +f: L.Fs, +i: U32, +fx: U32) -> S.Sl:
  +kind = W.fs_kind(f)
  +hit = D.attacking(st, b)
  +as_bramble = hurt(st, b, fx)
  +as_other = S.pks(hit, pop(st, b, i), hurt(st, b, fx))
  +as_warden = S.pks(hit, hit_warden(st, b, id, i), hurt(st, b, fx))
  +as_warden = S.pks(M.eq(R.down(as_warden, b), 0), warden_back(as_warden, b), as_warden)
  S.pks(M.eq(kind, 2), as_bramble, S.pks(M.eq(kind, 3), as_warden, as_other))

def foe_touch(+rd: L.Road, +st: S.Sl, +b: U32, +id: U32, +bx: Bx, +f: L.Fs, +i: U32) -> S.Sl:
  +fx = W.foe_x(rd, f, H.elapsed(st))
  +fy = W.foe_y(rd, f, H.elapsed(st), fx)
  +hits = over(bx, (fx - W.body_w(W.fs_kind(f)) : U32), (fx + W.body_w(W.fs_kind(f)) : U32), (fy - W.body_h(W.fs_kind(f)) : U32), fy)
  S.pks(hits, foe_meets(st, b, id, f, i, fx), st)

def foe_step(+go: Bool, +rd: L.Road, +st: S.Sl, +b: U32, +id: U32, +bx: Bx, +f: L.Fs, +i: U32) -> S.Sl:
  match go:
    case True{}:
      foe_touch(rd, st, b, id, bx, f, i)
    case False{}:
      st

# A foe in reach of the box, if it is awake and standing and the rider is still up.
def foe_gate(+near: Bool, +rd: L.Road, +st: S.Sl, +b: U32, +id: U32, +bx: Bx, +f: L.Fs, +i: U32) -> S.Sl:
  match near:
    case True{}:
      foe_step(Bool.and(M.eq(R.down(st, b), 0), Bool.and(awake(st, f), M.gt(W.foe_hp(st, i), 0))), rd, st, b, id, bx, f, i)
    case False{}:
      st

def foes_go(+fs: List<&2, L.Fs>, +rd: L.Road, +st: S.Sl, +b: U32, +id: U32, +bx: Bx, +i: U32, +cx: U32) -> S.Sl:
  match fs:
    case Nil{}:
      st
    case Con{f, rest}:
      foes_go(rest, rd, foe_gate(Bool.and(M.ge(W.fs_x(f), (cx - 200 : U32)), M.le(W.fs_x(f), (cx + 200 : U32))), rd, st, b, id, bx, f, i), b, id, bx, (i + 1 : U32), cx)

# What the rider's swept body does to the foes it meets.
def touch_foes(+rd: L.Road, +st: S.Sl, +b: U32, +id: U32, +c0: D.Cn, +c1: D.Cn) -> S.Sl:
  foes_go(O.ro_foes(rd), rd, st, b, id, bx_of(c0, c1, D.body_h(st, b)), 0, M.div(D.cn_x(c1), 256))

# ---- sparks, springs, pads, posts, crates and the bell ----------------------------------------

# The slot and bit of spark `i` among the sparks a rider has taken: the bits are
# 64 to a word, kept as two halves (the high half first).
def taken_slot(+b: U32, +i: U32) -> U32:
  (b + 56 + U32.div(i, 64) * 2 + M.b2u((U32.mod(i, 64) < 32 : U32)) : U32)

def taken_bit(+i: U32) -> Nat:
  U32.to_nat(U32.mod(i, 32))

def taken_has(+st: S.Sl, +b: U32, +i: U32) -> Bool:
  M.nz(U32.and(U32.shrn(S.g(st, taken_slot(b, i)), taken_bit(i)), 1))

def taken_set(+st: S.Sl, +b: U32, +i: U32) -> S.Sl:
  S.p(st, taken_slot(b, i), U32.or(S.g(st, taken_slot(b, i)), U32.shln(1, taken_bit(i))))

def spark_hit(+st: S.Sl, +b: U32, +i: U32) -> S.Sl:
  +s = taken_set(st, b, i)
  +s = R.set_collected(R.set_sparks(s, b, (R.sparks(s, b) + 1 : U32)), b, (R.collected(s, b) + 1 : U32))
  S.p(s, 108, (S.g(s, 108) + 1 : U32))

def spark_step(+go: Bool, +st: S.Sl, +b: U32, +i: U32) -> S.Sl:
  match go:
    case True{}:
      spark_hit(st, b, i)
    case False{}:
      st

# A spark in reach of the swept body and not yet taken is taken.
def spark_gate(+near: Bool, +st: S.Sl, +b: U32, +bx: Bx, +s: L.Sp, +i: U32) -> S.Sl:
  match near:
    case True{}:
      spark_step(Bool.and(Bool.not(taken_has(st, b, i)), over(bx, (L.sp_x(s) - 6 : U32), (L.sp_x(s) + 6 : U32), (L.sp_y(s) - 10 : U32), (L.sp_y(s) + 10 : U32))), st, b, i)
    case False{}:
      st

def sparks_go(+sp: List<&2, L.Sp>, +st: S.Sl, +b: U32, +bx: Bx, +i: U32, +cx: U32) -> S.Sl:
  match sp:
    case Nil{}:
      st
    case Con{s, rest}:
      +x = L.sp_x(s)
      sparks_go(rest, spark_gate(Bool.and(M.ge(x, (cx - 60 : U32)), M.le(x, (cx + 60 : U32))), st, b, bx, s, i), b, bx, (i + 1 : U32), cx)

# Sparks: a bit each, taken once by this knight.
def touch_sparks(+rd: L.Road, +st: S.Sl, +b: U32, +bx: Bx, +cx: U32) -> S.Sl:
  +s = sparks_go(O.ro_sparks(rd), S.p(st, 108, 0), b, bx, 0, cx)
  +got = S.g(s, 108)
  S.pks(M.gt(got, 0), H.snd(R.set_gauge(s, b, M.min((R.gauge(s, b) + 12 * got : U32), 1000)), H.k_gold()), s)

# A hay-bale spring throws the rider up, or up and forward.
def spring_hit(+st: S.Sl, +b: U32, +i: U32, +kind: U32) -> S.Sl:
  +carry = M.pk(M.max(M.abs(R.gsp(st, b)), M.abs(R.vx(st, b))), M.abs(R.vx(st, b)), D.grounded(st, b))
  +dir = M.pk(1, M.neg(1), M.ge(R.dir(st, b), 0))
  +s = R.set_roll(R.set_ground_on(st, b, 0), b, 0)
  +s = R.set_jumping(R.set_ball(s, b, 0), b, 0)
  +s = R.set_spring_cd(R.set_coyote(s, b, 0), b, 8)
  +s = R.set_lunged(R.set_lunge(s, b, 0), b, 0)
  +up = R.set_vy(R.set_vx(s, b, (dir * M.min(carry, 1800) : U32)), b, M.neg(2200))
  +diag = R.set_vy(R.set_vx(s, b, (dir * M.clamp(carry, 1700, 2400) : U32)), b, M.neg(2000))
  +s = S.pks(M.eq(kind, 0), up, diag)
  H.snd(S.p(s, (264 + i : U32), (H.elapsed(st) + 1 : U32)), H.k_wall_up())

def spring_step(+go: Bool, +st: S.Sl, +b: U32, +i: U32, +kind: U32) -> S.Sl:
  match go:
    case True{}:
      spring_hit(st, b, i, kind)
    case False{}:
      st

def springs_go(+sp: List<&2, L.Spr>, +st: S.Sl, +b: U32, +bx: Bx, +i: U32, +cx: U32) -> S.Sl:
  match sp:
    case Nil{}:
      st
    case Con{s, rest}:
      match s:
        case L.Spr{x, y, kind}:
          +go = Bool.and(M.ge(x, (cx - 40 : U32)), Bool.and(M.le(x, (cx + 40 : U32)),
                Bool.and(M.eq(R.spring_cd(st, b), 0), over(bx, (x - 14 : U32), (x + 14 : U32), (y - 18 : U32), (y + 4 : U32)))))
          springs_go(rest, spring_step(go, st, b, i, kind), b, bx, (i + 1 : U32), cx)

# A boost pad lifts a slow rider to its pace and lets a faster one be.
def pad_hit(+st: S.Sl, +b: U32, +i: U32) -> S.Sl:
  +s = H.snd(S.p(R.set_dir(R.set_gsp(st, b, 1920), b, 1), (272 + i : U32), (H.elapsed(st) + 1 : U32)), H.k_vent_fire())
  S.pks(M.lt(R.gsp(st, b), 1920), s, st)

def pad_step(+go: Bool, +st: S.Sl, +b: U32, +i: U32) -> S.Sl:
  match go:
    case True{}:
      pad_hit(st, b, i)
    case False{}:
      st

def pads_go(+sp: List<&2, L.Pad>, +st: S.Sl, +b: U32, +bx: Bx, +i: U32, +cx: U32) -> S.Sl:
  match sp:
    case Nil{}:
      st
    case Con{s, rest}:
      match s:
        case L.Pad{x, y}:
          +go = Bool.and(M.ge(x, (cx - 40 : U32)), Bool.and(M.le(x, (cx + 40 : U32)),
                Bool.and(D.grounded(st, b), Bool.and(M.ge(R.gsp(st, b), 0), over(bx, (x - 18 : U32), (x + 18 : U32), (y - 12 : U32), (y + 6 : U32))))))
          pads_go(rest, pad_step(go, st, b, i), b, bx, (i + 1 : U32), cx)

# Lantern posts: any knight lights them for everyone.
def post_light(+rd: L.Road, +st: S.Sl, +b: U32, +i: U32, +p: L.Post, +others: U32) -> S.Sl:
  +s = H.set_posts(st, U32.or(H.posts(st), U32.shln(1, U32.to_nat(i))))
  +s = S.p(s, (256 + i : U32), (H.elapsed(st) + 1 : U32))
  +s = H.set_dim(s, M.min(H.dim(st), ((post_x(p) - hold_of(rd, i, others) : U32) * 256 : U32)))
  H.snd(R.set_gauge(s, b, M.min((R.gauge(s, b) + 200 : U32), 1000)), H.k_circle_toll())

def post_step(+go: Bool, +rd: L.Road, +st: S.Sl, +b: U32, +i: U32, +p: L.Post, +others: U32) -> S.Sl:
  match go:
    case True{}:
      post_light(rd, st, b, i, p, others)
    case False{}:
      st

def posts_go(+ps: List<&2, L.Post>, +rd: L.Road, +st: S.Sl, +b: U32, +bx: Bx, +i: U32, +others: U32) -> S.Sl:
  match ps:
    case Nil{}:
      st
    case Con{p, rest}:
      +go = Bool.and(Bool.not(post_lit(st, i)), over(bx, (post_x(p) - 22 : U32), (post_x(p) + 22 : U32), (post_y(p) - 70 : U32), (post_y(p) + 10 : U32)))
      posts_go(rest, rd, post_step(go, rd, st, b, i, p, others), b, bx, (i + 1 : U32), others)

# A lantern crate is broken by anything that attacks, and bounces a falling rider.
def crate_hit(+st: S.Sl, +b: U32, +i: U32, +kind: U32) -> S.Sl:
  +s = R.set_crates(st, b, U32.or(R.crates(st, b), U32.shln(1, U32.to_nat(i))))
  +shield = H.snd(R.set_shield(s, b, 1), H.k_shield_up())
  +sparks = H.snd(R.set_collected(R.set_sparks(s, b, (R.sparks(s, b) + 10 : U32)), b, (R.collected(s, b) + 10 : U32)), H.k_chest_open())
  +spur = H.snd(R.set_gauge(s, b, 1000), H.k_chest_open())
  +s = S.pks(M.eq(kind, 0), shield, S.pks(M.eq(kind, 1), sparks, spur))
  S.pks(Bool.and(Bool.not(D.grounded(s, b)), M.gt(R.vy(s, b), 0)), R.set_vy(s, b, M.neg(1100)), s)

def crate_step(+go: Bool, +st: S.Sl, +b: U32, +i: U32, +kind: U32) -> S.Sl:
  match go:
    case True{}:
      crate_hit(st, b, i, kind)
    case False{}:
      st

def crates_go(+cs: List<&2, L.Crate>, +st: S.Sl, +b: U32, +bx: Bx, +i: U32) -> S.Sl:
  match cs:
    case Nil{}:
      st
    case Con{c, rest}:
      match c:
        case L.Crate{x, y, kind}:
          +go = Bool.and(Bool.not(M.nz(U32.and(U32.shrn(R.crates(st, b), U32.to_nat(i)), 1))),
                Bool.and(D.attacking(st, b), over(bx, (x - 12 : U32), (x + 12 : U32), (y - 24 : U32), y)))
          crates_go(rest, crate_step(go, st, b, i, kind), b, bx, (i + 1 : U32))

# The bell, once the warden has fallen: the first to ring it starts the clock on
# the rest.
def bell_ring(+rd: L.Road, +st: S.Sl, +b: U32, +bx: Bx) -> S.Sl:
  +wi = warden_index(rd)
  +open = Bool.or(M.eq(wi, none()), M.eq(W.foe_hp(st, wi), 0))
  +ring = Bool.and(open, Bool.and(Bool.not(D.finished(st, b)),
          over(bx, (O.ro_bell_x(rd) - 28 : U32), (O.ro_bell_x(rd) + 28 : U32), (O.ro_bell_y(rd) - 120 : U32), (O.ro_bell_y(rd) + 20 : U32))))
  +s = R.set_finish_at(R.set_finish_on(st, b, 1), b, H.elapsed(st))
  +s = H.snd(H.set_bell_t(H.set_bell_on(s, 1), M.pk(H.bell_t(s), H.elapsed(st), M.nz(H.bell_on(s)))), H.k_circle_toll())
  S.pks(ring, s, st)

# Sparks, springs, pads, posts, crates and the bell.
def touch_things(+rd: L.Road, +st: S.Sl, +b: U32, +c0: D.Cn, +c1: D.Cn, +others: U32) -> S.Sl:
  +bx = bx_of(c0, c1, D.body_h(st, b))
  +cx = M.div(D.cn_x(c1), 256)
  +s = touch_sparks(rd, st, b, bx, cx)
  +s = springs_go(O.ro_springs(rd), s, b, bx, 0, cx)
  +s = pads_go(O.ro_pads(rd), s, b, bx, 0, cx)
  +s = posts_go(O.ro_posts(rd), rd, s, b, bx, 0, others)
  +s = crates_go(O.ro_crates(rd), s, b, bx, 0)
  bell_ring(rd, s, b, bx)

# Which of the loose sparks stay: those not yet settled enough to be taken back,
# or not near the rider.
def loose_far(+fuel: Nat, +i: U32, +st: S.Sl, +b: U32, +cx: U32, +cy: U32, +acc: U32) -> U32:
  match fuel:
    case 0n:
      acc
    case 1n+k:
      +o = (b + 80 + i * 5 : U32)
      +near = Bool.and(M.ge(S.g(st, (o + 4 : U32)), 24),
              Bool.and(M.lt(M.abs((S.g(st, o) - cx : U32)), 4608), M.lt(M.abs((S.g(st, (o + 1 : U32)) - cy : U32)), 6144)))
      loose_far(k, (i + 1 : U32), st, b, cx, cy, (acc + U32.shln(M.b2u(Bool.not(near)), U32.to_nat(i)) : U32))

# Take back sparks thrown by a hit, once they have settled a moment.
def take_loose(+rd: L.Road, +st: S.Sl, +b: U32) -> S.Sl:
  +c = D.centre(rd, st, b)
  +n = R.loose_n(st, b)
  +keep = loose_far(U32.to_nat(n), 0, st, b, D.cn_x(c), D.cn_y(c), 0)
  +s = D.loose_pack(U32.to_nat(n), 0, 0, st, b, keep)
  +got = (n - R.loose_n(s, b) : U32)
  S.pks(M.gt(got, 0), H.snd(R.set_sparks(s, b, (R.sparks(s, b) + got : U32)), H.k_gold()), s)

# The Dimming has the rider.
def swallow_go(+st: S.Sl, +b: U32) -> S.Sl:
  +s = R.set_falls(R.set_sparks(st, b, 0), b, (R.falls(st, b) + 1 : U32))
  +s = R.set_down(R.set_hits(s, b, (R.hits(st, b) + 1 : U32)), b, 24)
  +s = R.set_vy(R.set_vx(s, b, 0), b, M.neg(1200))
  +s = R.set_ball(R.set_roll(R.set_ground_on(s, b, 0), b, 0), b, 0)
  H.snd(R.set_lock_on(s, b, 0), H.k_boss_roar())

def swallow(+st: S.Sl, +b: U32) -> S.Sl:
  S.pks(M.gt(R.down(st, b), 0), st, swallow_go(st, b))

# The lantern carries a fallen rider to the last post and holds the Dimming back
# from it.
def carry_home(+rd: L.Road, +st: S.Sl, +b: U32, +others: U32) -> S.Sl:
  +n = last_post(st)
  +none_lit = U32.is_eq(n, none())
  +p = post_nth(rd, n)
  +path = M.pk(O.ro_start_path(rd), post_path(p), none_lit)
  +s = M.pk(O.ro_start_s(rd), post_s(p), none_lit)
  +start_x = M.div(T.sm_x(T.sample(T.path_at(O.ro_paths(rd), O.ro_start_path(rd)), O.ro_start_s(rd))), 256)
  +at = M.pk(start_x, post_x(p), none_lit)
  +hold = M.pk(520, hold_of(rd, n, others), none_lit)
  +r = D.place(rd, st, b, path, s)
  H.set_dim(r, M.min(H.dim(r), ((at - M.div((hold * 4 : U32), 5) : U32) * 256 : U32)))

# ---- one knight's tick -------------------------------------------------------------------

# Whether a faster rider just ahead (seat `o`) carries this one in his wake.
def draft_one(+st: S.Sl, +o: U32, +id: U32, +x: U32, +y: U32, +dir: U32) -> Bool:
  +k = (160 + (o - 1) * 5 : U32)
  +dx = ((S.g(st, (k + 1 : U32)) - x : U32) * dir : U32)
  Bool.and(Bool.not(M.eq(o, id)), Bool.and(M.eq(S.g(st, (k + 3 : U32)), dir), Bool.and((S.g(st, (k + 4 : U32)) >= 640 : U32),
    Bool.and(M.le(M.abs((S.g(st, (k + 2 : U32)) - y : U32)), 28), Bool.and(M.ge(dx, 14), M.le(dx, 110))))))

# The pack's slipstream: is a faster rider just ahead?
def drafting(+st: S.Sl, +id: U32, +b: U32) -> Bool:
  +x = M.div(R.x(st, b), 256)
  +y = M.div(R.y(st, b), 256)
  +dir = R.dir(st, b)
  Bool.and(M.eq(R.down(st, b), 0), Bool.and(Bool.not(D.finished(st, b)),
    Bool.or(Bool.or(Bool.or(draft_one(st, 1, id, x, y, dir), draft_one(st, 2, id, x, y, dir)), Bool.or(draft_one(st, 3, id, x, y, dir), draft_one(st, 4, id, x, y, dir))),
            Bool.or(draft_one(st, 5, id, x, y, dir), draft_one(st, 6, id, x, y, dir)))))

# Where everyone is at the start of a tick, for the wake behind a pack.
def snap_one(+st: S.Sl, +id: U32) -> S.Sl:
  +b = sb(id)
  +k = (160 + (id - 1) * 5 : U32)
  +live = Bool.and(M.eq(R.down(st, b), 0), Bool.not(D.finished(st, b)))
  +s = S.p(S.p(S.p(st, k, id), (k + 1 : U32), M.div(R.x(st, b), 256)), (k + 2 : U32), M.div(R.y(st, b), 256))
  S.pks(seated(st, id), S.p(S.p(s, (k + 3 : U32), R.dir(st, b)), (k + 4 : U32), M.pk(D.speed(st, b), 0, live)), S.p(st, (k + 4 : U32), 0))

def snap_all(+st: S.Sl) -> S.Sl:
  snap_one(snap_one(snap_one(snap_one(snap_one(snap_one(st, 1), 2), 3), 4), 5), 6)

def swallow_if(+c: Bool, +st: S.Sl, +b: U32) -> S.Sl:
  S.pks(c, swallow(st, b), st)

# The last of a tick once the rider has been through the foes and is still up:
# the things on the road, the loose sparks, and the Dimming.
def ride_tail(+rd: L.Road, +st: S.Sl, +b: U32, +c0: D.Cn, +c1: D.Cn, +others: U32) -> S.Sl:
  +s = touch_things(rd, st, b, c0, c1, others)
  +s = take_loose(rd, D.tick_loose(rd, s, b), b)
  swallow_if(Bool.and(Bool.not(D.finished(s, b)), Bool.or(M.lt(R.x(s, b), H.dim(s)), M.gt(R.y(s, b), (O.ro_dead_y(rd) * 256 : U32)))), s, b)

def tail_pick(+up: Bool, +rd: L.Road, +st: S.Sl, +b: U32, +c0: D.Cn, +c1: D.Cn, +others: U32) -> S.Sl:
  match up:
    case True{}:
      ride_tail(rd, st, b, c0, c1, others)
    case False{}:
      st

def foes_pick(+go: Bool, +rd: L.Road, +st: S.Sl, +b: U32, +id: U32, +c0: D.Cn, +c1: D.Cn) -> S.Sl:
  match go:
    case True{}:
      touch_foes(rd, st, b, id, c0, c1)
    case False{}:
      st

def lock_pick(+want: Bool, +rd: L.Road, +st: S.Sl, +b: U32) -> S.Sl:
  match want:
    case True{}:
      begin_lock(rd, R.set_want_lock(st, b, 0), b)
    case False{}:
      st

# The rider moves, locks a foe if the press asked for one, and meets what is on
# the road.
def ride_moved(+rd: L.Road, +st: S.Sl, +b: U32, +id: U32, +c0: D.Cn, +others: U32) -> S.Sl:
  +s = lock_pick(M.nz(R.want_lock(st, b)), rd, st, b)
  +c1 = D.centre(rd, s, b)
  +s = wall(rd, s, b)
  +s = foes_pick(Bool.not(D.finished(s, b)), rd, s, b, id, c0, c1)
  tail_pick(M.eq(R.down(s, b), 0), rd, s, b, c0, c1, others)

def ride_alive(+rd: L.Road, +st: S.Sl, +id: U32, +b: U32, +others: U32) -> S.Sl:
  +s = R.set_draft(st, b, M.b2u(drafting(st, id, b)))
  +s = steer_lock(rd, s, b)
  +c0 = D.centre(rd, s, b)
  ride_moved(rd, D.step(rd, s, b, key_x(st, id), key_down(st, id), key_jump(st, id), key_spur(st, id)), b, id, c0, others)

# A fallen rider only falls: no keys, no road, until the lantern comes.
def ride_down(+rd: L.Road, +st: S.Sl, +id: U32, +b: U32, +others: U32) -> S.Sl:
  +s = R.set_spur_key(R.set_jump_key(st, b, key_jump(st, id)), b, key_spur(st, id))
  +s = D.tick_loose(rd, D.tumble(s, b), b)
  S.pks(M.eq(R.down(s, b), 0), H.snd(carry_home(rd, s, b, others), H.k_potion()), s)

def ride_pick(+fallen: Bool, +rd: L.Road, +st: S.Sl, +id: U32, +b: U32, +others: U32) -> S.Sl:
  match fallen:
    case True{}:
      ride_down(rd, st, id, b, others)
    case False{}:
      ride_alive(rd, st, id, b, others)

# One knight's tick. A press of E at the tally, once it has ticked up, asks to
# ride again.
def ride(+rd: L.Road, +st: S.Sl, +id: U32) -> S.Sl:
  +b = sb(id)
  +again = Bool.and(M.nz(H.tally_on(st)), Bool.and(M.nz(key_interact(st, id)), Bool.and(Bool.not(M.nz(R.jump_key(st, b))), (H.elapsed(st) >= H.t_at(st) + 45 : U32))))
  ride_pick(M.gt(R.down(st, b), 0), rd, H.set_again(st, M.pk(1, H.again(st), again)), id, b, M.sat1(count(st)))

# ---- the Dimming, the bell and the tally ---------------------------------------------------

# The Dimming's pace at each act, percent of the number.
def act_pace(+act: U32) -> U32:
  M.pk(90, M.pk(110, M.pk(130, 150, M.eq(act, 2)), M.eq(act, 1)), M.eq(act, 0))

# The column of the rearmost rider still riding (2147483647 if none).
def rear_one(+st: S.Sl, +id: U32, +best: U32) -> U32:
  +b = sb(id)
  M.pk(M.min(best, R.x(st, b)), best, Bool.and(seated(st, id), Bool.and(Bool.not(D.finished(st, b)), M.eq(R.down(st, b), 0))))

def rear(+st: S.Sl) -> U32:
  rear_one(st, 6, rear_one(st, 5, rear_one(st, 4, rear_one(st, 3, rear_one(st, 2, rear_one(st, 1, 2147483647))))))

def dimming_go(+rd: L.Road, +st: S.Sl) -> S.Sl:
  +pace = M.div((M.div((M.div((H.dimming_of(st) * (100 + 8 * M.max((count(st) - 1 : U32), 0) : U32) : U32), 100) * act_pace(act_at(rd, M.div(H.dim(st), 256))) : U32), 100)), 1)
  +dim = (H.dim(st) + pace : U32)
  +r = rear(st)
  H.set_dim(st, M.pk(dim, M.max(dim, (r - 307200 : U32)), U32.is_eq(r, 2147483647)))

# The Dimming gains on the party: never so far behind the rearmost knight that
# the chase is lost.
def tick_dimming(+rd: L.Road, +st: S.Sl) -> S.Sl:
  S.pks(M.nz(H.bell_on(st)), st, dimming_go(rd, st))

def sp_len(+xs: List<&2, L.Sp>) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{h, t}:
      (1 + sp_len(t) : U32)

# A knight's own score for the ride, from his sparks, chain, falls and finish.
def personal(+collected: U32, +best_chain: U32, +falls: U32, +finish: U32, +total: U32) -> U32:
  +t = U32.div((400 * M.min(M.sat(300, U32.div(finish, 30)), 200) : U32), 200)
  M.sat((t + M.min(U32.div((300 * collected : U32), total), 300) + 25 * M.min(best_chain, 8) : U32), (falls * 15 : U32))

def finish_one(+st: S.Sl, +id: U32) -> S.Sl:
  +b = sb(id)
  S.pks(Bool.and(seated(st, id), Bool.not(D.finished(st, b))), R.set_finish_at(R.set_finish_on(st, b, 1), b, H.elapsed(st)), st)

# Each seated knight's finish, sparks, chain, falls, in a sum or a maximum.
def sum_one(+st: S.Sl, +id: U32, +slot: U32) -> U32:
  M.pk(S.g(st, (sb(id) + slot : U32)), 0, seated(st, id))

def max_one(+st: S.Sl, +id: U32, +slot: U32) -> U32:
  M.pk(S.g(st, (sb(id) + slot : U32)), 0, seated(st, id))

def sum_all(+st: S.Sl, +slot: U32) -> U32:
  (sum_one(st, 1, slot) + sum_one(st, 2, slot) + sum_one(st, 3, slot) + sum_one(st, 4, slot) + sum_one(st, 5, slot) + sum_one(st, 6, slot) : U32)

def max_all(+st: S.Sl, +slot: U32) -> U32:
  M.max(M.max(M.max(max_one(st, 1, slot), max_one(st, 2, slot)), M.max(max_one(st, 3, slot), max_one(st, 4, slot))), M.max(max_one(st, 5, slot), max_one(st, 6, slot)))

def score_one(+st: S.Sl, +id: U32, +total: U32) -> S.Sl:
  +b = sb(id)
  +score = M.min(personal(R.collected(st, b), R.best_chain(st, b), R.falls(st, b), R.finish_at(st, b), total), 1000)
  S.pks(seated(st, id), S.p(S.p(st, (18 + id : U32), score), (24 + id : U32), 1), st)

# The best score: the highest, the lowest seat winning a tie.
def best_one(+st: S.Sl, +id: U32, +best: U32) -> U32:
  M.pk(id, best, Bool.and(U32.is_eq(S.g(st, (24 + id : U32)), 1),
    Bool.or(U32.is_eq(S.g(st, (24 + best : U32)), 0), (S.g(st, (18 + id : U32)) > S.g(st, (18 + best : U32)) : U32))))

def bits(+n: U32) -> U32:
  (U32.and(n, 1) + U32.and(U32.shrn(n, 1n), 1) + U32.and(U32.shrn(n, 2n), 1) + U32.and(U32.shrn(n, 3n), 1) + U32.and(U32.shrn(n, 4n), 1) + U32.and(U32.shrn(n, 5n), 1) : U32)

# Reckon the ride: the pack's time, sparks, chain and posts.
def make_tally(+rd: L.Road, +st: S.Sl) -> S.Sl:
  +s = finish_one(finish_one(finish_one(finish_one(finish_one(finish_one(st, 1), 2), 3), 4), 5), 6)
  +total = M.max(sp_len(O.ro_sparks(rd)), 1)
  +n = M.max(count(s), 1)
  +time = max_all(s, 51)
  +sparks = U32.div(sum_all(s, 46), n)
  +chain = max_all(s, 26)
  +posts = bits(H.posts(s))
  +falls = sum_all(s, 47)
  +secs = U32.div(time, 30)
  +time_pts = U32.div((400 * M.min(M.sat(300, secs), 200) : U32), 200)
  +points = M.min(M.sat((time_pts + M.min(U32.div((300 * sparks : U32), total), 300) + 25 * M.min(chain, 8) + U32.div((100 * posts : U32), 6) : U32), M.min((falls * 15 : U32), 100)), 1000)
  +rank = M.pk(3, M.pk(2, M.pk(1, 0, (points >= 420 : U32)), (points >= 650 : U32)), (points >= 850 : U32))
  +s = score_one(score_one(score_one(score_one(score_one(score_one(s, 1, total), 2, total), 3, total), 4, total), 5, total), 6, total)
  +best = best_one(s, 6, best_one(s, 5, best_one(s, 4, best_one(s, 3, best_one(s, 2, best_one(s, 1, 1))))))
  +s = H.set_t_best(H.set_t_rank(H.set_t_points(H.set_t_falls(H.set_t_posts(H.set_t_chain(H.set_t_sparks(H.set_t_time(H.set_t_at(H.set_tally_on(s, 1), H.elapsed(s)), time), sparks), chain), posts), falls), points), rank), best)
  s

# The bell has rung: once every knight has, or the rest have had their grace,
# the ride is tallied.
def all_finished(+st: S.Sl, +id: U32) -> Bool:
  Bool.or(Bool.not(seated(st, id)), D.finished(st, sb(id)))

def tick_bell(+rd: L.Road, +st: S.Sl) -> S.Sl:
  +all = Bool.and(Bool.and(Bool.and(all_finished(st, 1), all_finished(st, 2)), Bool.and(all_finished(st, 3), all_finished(st, 4))), Bool.and(all_finished(st, 5), all_finished(st, 6)))
  +due = Bool.or(all, (H.elapsed(st) >= H.bell_t(st) + 300 : U32))
  S.pks(Bool.and(M.nz(H.bell_on(st)), Bool.and(Bool.not(M.nz(H.tally_on(st))), due)), make_tally(rd, st), st)

# ---- sounds, seats and the tick ----------------------------------------------------------------

# Whether `code` is among the first `m` sounds of `st`.
def seen_go(+fuel: Nat, +i: U32, +st: S.Sl, +code: U32) -> Bool:
  match fuel:
    case 0n:
      False{}
    case 1n+k:
      Bool.or(U32.is_eq(S.g(st, (37 + i : U32)), code), seen_go(k, (i + 1 : U32), st, code))

def seen_before(+m: U32, +st: S.Sl, +code: U32) -> Bool:
  seen_go(U32.to_nat(m), 0, st, code)

# Each sound once, in the order it was first made.
def dedup_go(+fuel: Nat, +i: U32, +m: U32, +st: S.Sl, +out: S.Sl) -> S.Sl:
  match fuel:
    case 0n:
      S.p(out, 36, m)
    case 1n+k:
      +code = S.g(st, (37 + i : U32))
      +seen = seen_before(m, out, code)
      dedup_go(k, (i + 1 : U32), (m + M.b2u(Bool.not(seen)) : U32), st, S.pks(seen, out, S.p(out, (37 + m : U32), code)))

def dedup(+st: S.Sl) -> S.Sl:
  dedup_go(U32.to_nat(S.g(st, 36)), 0, 0, st, st)

# A knight sits down, at the gate side by side a few pixels apart or at the last
# lit post (4294967295: none).
def seat_up(+rd: L.Road, +st: S.Sl, +id: U32, +joined: U32, +post: U32) -> S.Sl:
  +none_lit = U32.is_eq(post, none())
  +p = post_nth(rd, post)
  +path = M.pk(O.ro_start_path(rd), post_path(p), none_lit)
  +s = M.pk((O.ro_start_s(rd) - (id - 1) * 12 * 256 : U32), post_s(p), none_lit)
  D.new_on(rd, st, sb(id), id, joined, path, s)

# Whoever is here and awake and not yet seated sits down; the one who has left
# is gone.
def retain_one(+st: S.Sl, +id: U32) -> S.Sl:
  S.pks(Bool.and(seated(st, id), Bool.not(here(st, id))), R.set_id(st, sb(id), 0), st)

def join_one(+rd: L.Road, +st: S.Sl, +id: U32, +post: U32) -> S.Sl:
  S.pks(Bool.and(here(st, id), Bool.and(Bool.not(seated(st, id)), (count(st) < 6 : U32))), seat_up(rd, st, id, H.elapsed(st), post), st)

def seat_all(+rd: L.Road, +st: S.Sl) -> S.Sl:
  +s = retain_one(retain_one(retain_one(retain_one(retain_one(retain_one(st, 1), 2), 3), 4), 5), 6)
  +post = last_post(s)
  join_one(rd, join_one(rd, join_one(rd, join_one(rd, join_one(rd, join_one(rd, s, 1, post), 2, post), 3, post), 4, post), 5, post), 6, post)

def ride_one(+rd: L.Road, +st: S.Sl, +id: U32) -> S.Sl:
  S.pks(seated(st, id), ride(rd, st, id), st)

# The tick once the knights are seated: the foes' clocks, every knight, the
# Dimming and the bell.
def tick_seated(+rd: L.Road, +st: S.Sl) -> S.Sl:
  +s = snap_all(tick_foes(rd, st))
  +s = ride_one(rd, ride_one(rd, ride_one(rd, ride_one(rd, ride_one(rd, ride_one(rd, s, 1), 2), 3), 4), 5), 6)
  dedup(tick_bell(rd, tick_dimming(rd, s)))

def tick_pick(+any: Bool, +rd: L.Road, +st: S.Sl) -> S.Sl:
  match any:
    case True{}:
      tick_seated(rd, st)
    case False{}:
      st

def tick_grown(+rd: L.Road, +st: S.Sl) -> S.Sl:
  +n = count(st)
  +s = S.pks((n > H.provisioned(st) : U32), grow_warden(rd, st, (n - H.provisioned(st) : U32)), st)
  +s = H.set_provisioned(s, M.pk(n, H.provisioned(s), (n > H.provisioned(s) : U32)))
  tick_pick(M.gt(n, 0), rd, s)

# One tick: seat whoever is here and awake, run every knight, the foes and the
# Dimming. The sounds the tick made are in the slots, once each.
def tick(+rd: L.Road, +st: S.Sl) -> S.Sl:
  tick_grown(rd, seat_all(rd, S.p(H.set_elapsed(st, M.min((H.elapsed(st) + 1 : U32), limit())), 36, 0)))

# ---- a fresh ride ------------------------------------------------------------------------------

# Hits each foe starts with: the warden takes several, the rest one.
def foes_init(+fs: List<&2, L.Fs>, +i: U32, +st: S.Sl) -> S.Sl:
  match fs:
    case Nil{}:
      S.p(st, 106, i)
    case Con{f, rest}:
      foes_init(rest, (i + 1 : U32), W.set_foe(st, i, M.pk(3, 1, M.eq(W.fs_kind(f), 3)), 0, 0))

def sit_one(+rd: L.Road, +st: S.Sl, +id: U32) -> S.Sl:
  S.pks(here(st, id), seat_up(rd, st, id, 0, none()), st)

# A new session: the knights who are here sit down at the gate, the foes are
# woken for as many as sit.
def start(+rd: L.Road, +st: S.Sl) -> S.Sl:
  +s = foes_init(O.ro_foes(rd), 0, H.set_provisioned(H.set_dim(H.set_elapsed(st, 0), M.neg(179200)), 1))
  +s = sit_one(rd, sit_one(rd, sit_one(rd, sit_one(rd, sit_one(rd, sit_one(rd, s, 1), 2), 3), 4), 5), 6)
  +n = M.max(count(s), 1)
  grow_warden(rd, H.set_provisioned(s, n), (n - 1 : U32))
