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
import ./session.bend as N

# What a bot presses: it reads the road ahead the way a player does (a bramble
# to clear, a snuffer to roll through, a loop to take at speed, a line of foes to
# chain, a gate to break) and presses the same keys.
#
# The bot's own settings ride in slots 192 to 197: which way through the fork (0
# the high rails, 1 the valley, 2 by seat: odd seats high, even low), whether the
# bot uses the spur, rolls, how many pixels late it hops, how often it lifts off
# the pace (0: never), and which knight it is.

type Th is Data:
  Th{jump_on: U32, jump_dx: U32, roll_at: U32, chain: U32, warden_on: U32, warden_dx: U32}

def th_jump_on(+t: Th) -> U32:
  match t:
    case Th{jump_on, jump_dx, roll_at, chain, warden_on, warden_dx}:
      jump_on

def th_roll_at(+t: Th) -> U32:
  match t:
    case Th{jump_on, jump_dx, roll_at, chain, warden_on, warden_dx}:
      roll_at

def th_chain(+t: Th) -> U32:
  match t:
    case Th{jump_on, jump_dx, roll_at, chain, warden_on, warden_dx}:
      chain

def th_warden_on(+t: Th) -> U32:
  match t:
    case Th{jump_on, jump_dx, roll_at, chain, warden_on, warden_dx}:
      warden_on

def th_warden_dx(+t: Th) -> U32:
  match t:
    case Th{jump_on, jump_dx, roll_at, chain, warden_on, warden_dx}:
      warden_dx

# One foe ahead, as the bot sees it.
def look_one(+rd: L.Road, +st: S.Sl, +f: L.Fs, +i: U32, +x: U32, +y: U32, +speed: U32, +rspeed: U32, +lag: U32, +skill_roll: U32, +reach: U32, +t: Th) -> Th:
  match t:
    case Th{jump_on, jump_dx, roll_at, chain, warden_on, warden_dx}:
      +fx = W.foe_x(rd, f, H.elapsed(st))
      +fy = W.foe_y(rd, f, H.elapsed(st), fx)
      +dx = (fx - x : U32)
      +kind = W.fs_kind(f)
      +live = Bool.and(Bool.not(M.gt(W.fs_min_seats(f), H.provisioned(st))), M.gt(W.foe_hp(st, i), 0))
      +ok = Bool.and(live, Bool.not(M.lt(dx, M.neg(4))))
      +same = Bool.or(M.lt(M.abs((fy - y : U32)), 70), M.lt(M.abs((fy - W.body_h(kind) - y : U32)), 70))
      +bramble = Bool.and(ok, Bool.and(M.eq(kind, 2), Bool.and(same, Bool.and(M.gt(dx, 0), M.le(dx, (speed * 16 : U32))))))
      +hop = Bool.and(bramble, M.ge(dx, (speed * 10 - 6 - lag : U32)))
      +snuffer = Bool.and(ok, Bool.and(M.eq(kind, 0), Bool.and(same, Bool.and(Bool.and(M.ge(dx, 24), M.le(dx, 110)), M.gt(rspeed, 300)))))
      +floater = Bool.and(ok, Bool.and(M.eq(kind, 1), Bool.and(M.gt(dx, 0), M.lt(dx, reach))))
      +warden = Bool.and(ok, M.eq(kind, 3))
      Th{M.pk(1, jump_on, hop), M.pk(dx, jump_dx, hop), M.pk(skill_roll, roll_at, snuffer), M.pk(1, chain, floater), M.pk(1, warden_on, warden), M.pk(dx, warden_dx, warden)}

def look_step(+near: Bool, +rd: L.Road, +st: S.Sl, +f: L.Fs, +i: U32, +x: U32, +y: U32, +speed: U32, +rspeed: U32, +lag: U32, +skill_roll: U32, +reach: U32, +t: Th) -> Th:
  match near:
    case True{}:
      look_one(rd, st, f, i, x, y, speed, rspeed, lag, skill_roll, reach, t)
    case False{}:
      t

def look_go(+fs: List<&2, L.Fs>, +rd: L.Road, +st: S.Sl, +i: U32, +x: U32, +y: U32, +speed: U32, +rspeed: U32, +lag: U32, +skill_roll: U32, +reach: U32, +lo: U32, +hi: U32, +t: Th) -> Th:
  match fs:
    case Nil{}:
      t
    case Con{f, rest}:
      +near = Bool.and(M.ge(W.fs_x(f), lo), M.le(W.fs_x(f), hi))
      look_go(rest, rd, st, (i + 1 : U32), x, y, speed, rspeed, lag, skill_roll, reach, lo, hi, look_step(near, rd, st, f, i, x, y, speed, rspeed, lag, skill_roll, reach, t))

# Whether a snuffer within a few strides ahead is worth a hop.
def snuffer_go(+fs: List<&2, L.Fs>, +rd: L.Road, +st: S.Sl, +i: U32, +x: U32, +speed: U32, +lo: U32, +hi: U32) -> Bool:
  match fs:
    case Nil{}:
      False{}
    case Con{f, rest}:
      +near = Bool.and(M.ge(W.fs_x(f), lo), M.le(W.fs_x(f), hi))
      +hit = Bool.and(near, Bool.and(M.eq(W.fs_kind(f), 0), Bool.and(Bool.not(M.gt(W.fs_min_seats(f), H.provisioned(st))), Bool.and(M.gt(W.foe_hp(st, i), 0),
             Bool.and(M.ge((W.foe_x(rd, f, H.elapsed(st)) - x : U32), 30), M.le((W.foe_x(rd, f, H.elapsed(st)) - x : U32), (speed * 9 : U32)))))))
      Bool.or(hit, snuffer_go(rest, rd, st, (i + 1 : U32), x, speed, lo, hi))

# Whether a floater (still up) stands within `hi - lo` of the rider.
def floater_go(+fs: List<&2, L.Fs>, +st: S.Sl, +i: U32, +lo: U32, +hi: U32) -> Bool:
  match fs:
    case Nil{}:
      False{}
    case Con{f, rest}:
      Bool.or(Bool.and(Bool.and(M.ge(W.fs_x(f), lo), M.le(W.fs_x(f), hi)), Bool.and(M.eq(W.fs_kind(f), 1), M.gt(W.foe_hp(st, i), 0))), floater_go(rest, st, (i + 1 : U32), lo, hi))

# The column of the fork's mouth: the portal that only a fast rider is carried through.
def fork_go(+ps: List<&2, T.Portal>, +path: T.Path) -> U32:
  match ps:
    case Nil{}:
      N.none()
    case Con{q, rest}:
      match q:
        case T.Portal{s, dir, min_speed, to_path, to_s}:
          Bool.pick(U32, M.gt(min_speed, 0), M.div(T.sm_x(T.sample(path, s)), 256), fork_go(rest, path))

# Whether a loop's mouth (a portal that carries any rider forward) is a stride or
# two ahead and the run is short.
def mouth_go(+ps: List<&2, T.Portal>, +path: T.Path, +x: U32, +short: Bool) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{q, rest}:
      match q:
        case T.Portal{s, dir, min_speed, to_path, to_s}:
          +px = M.div(T.sm_x(T.sample(path, s)), 256)
          +hit = Bool.and(M.eq(min_speed, 0), Bool.and(M.eq(dir, 1), Bool.and(M.lt(x, px), Bool.and(M.lt((px - x : U32), 220), short))))
          Bool.or(hit, mouth_go(rest, path, x, short))

# What a bot presses: the run, and whether down, jump and spur are held.
type Pr is Data:
  Pr{x: U32, down: U32, jump: U32, spur: U32}

def pr_x(+p: Pr) -> U32:
  match p:
    case Pr{x, down, jump, spur}:
      x

def pr_down(+p: Pr) -> U32:
  match p:
    case Pr{x, down, jump, spur}:
      down

def pr_jump(+p: Pr) -> U32:
  match p:
    case Pr{x, down, jump, spur}:
      jump

def pr_spur(+p: Pr) -> U32:
  match p:
    case Pr{x, down, jump, spur}:
      spur

# Stuck at the foot of a wall: crouch and rev a charge, then go.
def revving(+st: S.Sl, +b: U32, +x: U32) -> Pr:
  +low = M.lt(R.charge(st, b), 1100)
  Pr{R.dir(st, b), M.b2u(low), M.b2u(Bool.and(low, Bool.and(Bool.not(M.nz(R.jump_key(st, b))), U32.is_eq(U32.mod(H.elapsed(st), 2), 0)))), 0}

def rail_links(+p: T.Path) -> Bool:
  match p:
    case T.Path{kind, n, depth, len, pts, no, np, ns, po, pp, ps, portals}:
      M.nz(no)

# The keys for the road ahead of knight `b`, riding as the bot's settings say.
def think_go(+rd: L.Road, +st: S.Sl, +b: U32, +id: U32, +x: U32, +y: U32, +rspeed: U32, +speed: U32, +reach: U32, +th: Th) -> Pr:
  +grounded = D.grounded(st, b)
  +air = Bool.not(grounded)
  +skill_roll = M.nz(S.g(st, 194))
  +skill_spur = M.nz(S.g(st, 193))
  +path = D.path_of(rd, st, b)
  +main = T.path_at(O.ro_paths(rd), O.ro_main(rd))
  +on_rail = Bool.and(grounded, U32.is_eq(T.pa_kind(path), 1))
  +fork = fork_go(T.pa_portals(main), main)
  +jump_on = M.nz(th_jump_on(th))
  +warden_on = M.nz(th_warden_on(th))
  +roll_at = M.nz(th_roll_at(th))
  +gauge = R.gauge(st, b)
  +jump0 = Bool.and(M.nz(R.jumping(st, b)), M.lt(R.vy(st, b), 0))
  +hop = Bool.and(jump_on, grounded)
  +tuck = Bool.and(Bool.not(hop), Bool.and(roll_at, Bool.and(grounded, Bool.not(M.nz(R.roll(st, b))))))
  +hop_snuffer = Bool.and(Bool.and(Bool.not(hop), Bool.not(tuck)), Bool.and(Bool.not(skill_roll), Bool.and(grounded,
      snuffer_go(O.ro_foes(rd), rd, st, 0, x, speed, (x - 0 : U32), (x + 120 : U32)))))
  +jump1 = Bool.or(jump0, Bool.or(hop, hop_snuffer))
  +down1 = Bool.and(Bool.not(hop), tuck)
  +gate = Bool.and(warden_on, Bool.and(M.lt(th_warden_dx(th), 190), grounded))
  +down2 = M.b2u(Bool.or(Bool.and(gate, M.gt(rspeed, 500)), Bool.and(Bool.not(gate), down1)))
  +spur1 = Bool.and(gate, Bool.and(M.gt(gauge, 80), M.gt(th_warden_dx(th), 60)))
  +lockable = Bool.and(air, Bool.and(Bool.not(D.locked_on(st, b)), M.eq(R.lock_cd(st, b), 0)))
  +jump2 = M.pk(M.b2u(Bool.not(M.nz(R.jump_key(st, b)))), M.b2u(jump1), Bool.and(lockable, Bool.not(M.eq(N.find_lock(rd, st, b), N.none()))))
  +jump3 = Bool.or(M.nz(jump2), Bool.and(M.nz(th_chain(th)), Bool.and(grounded, Bool.and(Bool.not(warden_on), Bool.and(M.gt(rspeed, 600),
      floater_go(O.ro_foes(rd), st, 0, (x : U32), (x + 140 : U32)))))))
  +mouth = mouth_go(T.pa_portals(main), main, x, Bool.and(M.lt(rspeed, 1500), M.gt(gauge, 60)))
  +found = Bool.not(U32.is_eq(fork, N.none()))
  +high = Bool.and(found, Bool.and(M.lt(x, fork), Bool.and(M.lt((fork - x : U32), 260), Bool.and(M.lt(rspeed, 1500), M.gt(gauge, 40)))))
  +upper = Bool.or(M.eq(S.g(st, 192), 0), Bool.and(M.eq(S.g(st, 192), 2), U32.is_eq(U32.mod(id, 2), 1)))
  +low_ease = Bool.and(Bool.not(upper), Bool.and(found, Bool.and(grounded, Bool.and(M.lt(x, fork), Bool.and(M.lt((fork - x : U32), 70), Bool.not(on_rail))))))
  +kx = M.pk(M.pk(M.neg(1), 0, M.ge(rspeed, 1180)), 1, low_ease)
  +rail_end = Bool.and(grounded, Bool.and(U32.is_eq(T.pa_kind(path), 1), Bool.and(Bool.not(rail_links(path)), (U32.div((T.pa_len(path) - R.ground_s(st, b) : U32), 256) < 22 : U32))))
  +jump4 = Bool.or(jump3, rail_end)
  +cruise = Bool.and(skill_spur, Bool.and(Bool.not(air), Bool.and(Bool.not(warden_on), Bool.not(jump_on))))
  +spur2 = Bool.or(Bool.or(spur1, mouth), Bool.or(Bool.and(upper, high), Bool.or(Bool.and(Bool.and(cruise, Bool.not(roll_at)), M.gt(gauge, 900)),
      Bool.and(cruise, Bool.and(M.nz(R.spur(st, b)), M.gt(gauge, 120))))))
  Pr{kx, down2, M.b2u(jump4), M.b2u(spur2)}


def think_pick(+rev: Bool, +rd: L.Road, +st: S.Sl, +b: U32, +id: U32, +x: U32, +y: U32, +rspeed: U32, +speed: U32, +reach: U32) -> Pr:
  match rev:
    case True{}:
      revving(st, b, x)
    case False{}:
      think_go(rd, st, b, id, x, y, rspeed, speed, reach,
        look_go(O.ro_foes(rd), rd, st, 0, x, y, speed, rspeed, S.g(st, 195), S.g(st, 194), reach,
                (x + U32.div(reach, 2) - (U32.div(reach, 2) + 40) : U32), (x + U32.div(reach, 2) + U32.div(reach, 2) + 40 : U32), Th{0, 0, 0, 0, 0, 0}))

def think(+rd: L.Road, +st: S.Sl, +b: U32, +id: U32) -> Pr:
  +x = M.div(R.x(st, b), 256)
  +y = M.div(R.y(st, b), 256)
  +rspeed = D.speed(st, b)
  +speed = M.max(M.div(rspeed, 256), 2)
  +stuck = Bool.and(D.grounded(st, b), Bool.and(M.lt(M.abs(R.gsp(st, b)), 160), Bool.not(M.nz(R.roll(st, b)))))
  +wedged = Bool.and(stuck, Bool.and((H.elapsed(st) > 90 : U32), Bool.and(U32.is_eq(U32.mod(H.elapsed(st), 3), 0),
      Bool.and(M.eq(R.charge(st, b), 0), Bool.and(M.gt(R.coyote(st, b), 0), M.gt(x, 40))))))
  think_pick(Bool.or(M.nz(R.crouch(st, b)), wedged), rd, st, b, id, x, y, rspeed, speed, M.min((speed * 22 + 60 : U32), 420))

# The keys for knight `id`, riding as the settings say (route 2: by seat).
def drive(+rd: L.Road, +st: S.Sl, +id: U32) -> Pr:
  +b = N.sb(id)
  +k = think(rd, st, b, id)
  +hesitate = S.g(st, 196)
  +lift = Bool.and(M.gt(hesitate, 0), Bool.and(D.grounded(st, b), (U32.mod((H.elapsed(st) + id * 37 : U32), M.max(hesitate, 1)) < 14 : U32)))
  +idle = Bool.or(Bool.not(N.seated(st, id)), Bool.or(M.nz(H.tally_on(st)), Bool.or(M.gt(R.down(st, b), 0), D.finished(st, b))))
  Pr{M.pk(0, M.pk(0, pr_x(k), lift), idle), M.pk(0, pr_down(k), idle), M.pk(0, pr_jump(k), idle), M.pk(0, M.pk(0, pr_spur(k), lift), idle)}
