import Base
import ./math.bend as M

# The road as curves. Every surface a rider can stand on is a path: a polyline
# with an arc length, and a rider on a path is a distance `s` along it and a
# ground speed. The points of a path live in a balanced tree of `depth` levels
# (2^depth leaves, the unused ones past the end holding 2147483647), each node
# remembering the arc length and the column of the first point on its right, so
# the segment at a distance or a column is found in one walk down.

type Pt is Data:
  Pt{x: U32, y: U32, cum: U32, dx: U32, dy: U32, mid: U32}

type Pts is Data:
  Pl{p: Pt}
  Pn{sc: U32, sx: U32, lo: Pts, hi: Pts}

# Where a path hands a rider on: a portal crossed in `dir` (1 or -1) at speed.
type Portal is Data:
  Portal{s: U32, dir: U32, min_speed: U32, to_path: U32, to_s: U32}

# kind: 0 road, 1 rail, 2 ring. `next` and `prev` carry a flag, a path and a
# distance. `n` is the number of points, `len` the arc length at the last.
type Path is Data:
  Path{kind: U32, n: U32, depth: Nat, len: U32, pts: Pts,
       next_on: U32, next_path: U32, next_s: U32,
       prev_on: U32, prev_path: U32, prev_s: U32,
       portals: List<&2, Portal>}

type Sm is Data:
  Sm{x: U32, y: U32, tx: U32, ty: U32}

def sm_x(+s: Sm) -> U32:
  match s:
    case Sm{x, y, tx, ty}:
      x

def sm_y(+s: Sm) -> U32:
  match s:
    case Sm{x, y, tx, ty}:
      y

def sm_tx(+s: Sm) -> U32:
  match s:
    case Sm{x, y, tx, ty}:
      tx

def sm_ty(+s: Sm) -> U32:
  match s:
    case Sm{x, y, tx, ty}:
      ty

def pt_x(+p: Pt) -> U32:
  match p:
    case Pt{x, y, cum, dx, dy, mid}:
      x

def pt_y(+p: Pt) -> U32:
  match p:
    case Pt{x, y, cum, dx, dy, mid}:
      y

def pt_cum(+p: Pt) -> U32:
  match p:
    case Pt{x, y, cum, dx, dy, mid}:
      cum

def pt_dx(+p: Pt) -> U32:
  match p:
    case Pt{x, y, cum, dx, dy, mid}:
      dx

def pt_dy(+p: Pt) -> U32:
  match p:
    case Pt{x, y, cum, dx, dy, mid}:
      dy

def pt_mid(+p: Pt) -> U32:
  match p:
    case Pt{x, y, cum, dx, dy, mid}:
      mid

def pa_kind(+a: Path) -> U32:
  match a:
    case Path{kind, n, depth, len, pts, no, np, ns, po, pp, ps, portals}:
      kind

def pa_n(+a: Path) -> U32:
  match a:
    case Path{kind, n, depth, len, pts, no, np, ns, po, pp, ps, portals}:
      n

def pa_depth(+a: Path) -> Nat:
  match a:
    case Path{kind, n, depth, len, pts, no, np, ns, po, pp, ps, portals}:
      depth

def pa_len(+a: Path) -> U32:
  match a:
    case Path{kind, n, depth, len, pts, no, np, ns, po, pp, ps, portals}:
      len

def pa_pts(+a: Path) -> Pts:
  match a:
    case Path{kind, n, depth, len, pts, no, np, ns, po, pp, ps, portals}:
      pts

def pa_portals(+a: Path) -> List<&2, Portal>:
  match a:
    case Path{kind, n, depth, len, pts, no, np, ns, po, pp, ps, portals}:
      portals

# ---- the walks down a tree ---------------------------------------------------

def bit(+i: U32, +d: Nat) -> Bool:
  U32.is_eq(U32.and(U32.shrn(i, d), 1), 1)

# The point at index `i`.
def pt_go(+n: Nat, +t: Pts, +i: U32) -> Pt:
  match n t:
    case 0n Pl{p}:
      p
    case 1n+k Pn{sc, sx, lo, hi}:
      pt_go(k, Bool.pick(Pts, bit(i, k), hi, lo), i)
    case _ _:
      Pt{0, 0, 0, 0, 0, 0}

def pt_at(+a: Path, +i: U32) -> Pt:
  pt_go(pa_depth(a), pa_pts(a), i)

# How many points have an arc length at or before `s` (non-negative).
def pp_cum(+n: Nat, +t: Pts, +s: U32, +acc: U32) -> U32:
  match n t:
    case 0n Pl{p}:
      (acc + M.b2u((pt_cum(p) <= s : U32)) : U32)
    case 1n+k Pn{sc, sx, lo, hi}:
      +c = (sc <= s : U32)
      pp_cum(k, Bool.pick(Pts, c, hi, lo), s, M.pk((acc + U32.shln(1, k) : U32), acc, c))
    case _ _:
      acc

# How many points lie in a column at or before `xq` (signed).
def pp_x(+n: Nat, +t: Pts, +xq: U32, +acc: U32) -> U32:
  match n t:
    case 0n Pl{p}:
      (acc + M.b2u(M.le(pt_x(p), xq)) : U32)
    case 1n+k Pn{sc, sx, lo, hi}:
      +c = M.le(sx, xq)
      pp_x(k, Bool.pick(Pts, c, hi, lo), xq, M.pk((acc + U32.shln(1, k) : U32), acc, c))
    case _ _:
      acc

# The segment a distance falls in.
def seg_at(+a: Path, +s: U32) -> U32:
  M.min(M.sat1(pp_cum(pa_depth(a), pa_pts(a), s, 0)), (pa_n(a) - 2 : U32))

# ---- sampling ----------------------------------------------------------------

# The tangent between two segment knots, eased by how far along the gap `s` is.
def blend(+a: Path, +s: U32, +lo: U32, +x: U32, +y: U32) -> Sm:
  +pl = pt_at(a, lo)
  +ph = pt_at(a, (lo + 1 : U32))
  +from = pt_mid(pl)
  +span = M.max((pt_mid(ph) - from : U32), 1)
  +w = (s - from : U32)
  Sm{x, y,
     (pt_dx(pl) + M.md((pt_dx(ph) - pt_dx(pl) : U32), w, span) : U32),
     (pt_dy(pl) + M.md((pt_dy(ph) - pt_dy(pl) : U32), w, span) : U32)}

def blend_or_plain(+edge: Bool, +a: Path, +s: U32, +lo: U32, +x: U32, +y: U32, +c: Pt) -> Sm:
  match edge:
    case True{}:
      Sm{x, y, pt_dx(c), pt_dy(c)}
    case False{}:
      blend(a, s, lo, x, y)

# The point at distance `s` (already held to the path) in segment `i`.
def sample_at(+a: Path, +s: U32, +i: U32) -> Sm:
  +c = pt_at(a, i)
  +t = (s - pt_cum(c) : U32)
  +below = M.le(s, pt_mid(c))
  +edge = M.pkb(below, M.eq(i, 0), M.ge((i + 2 : U32), pa_n(a)))
  blend_or_plain(edge, a, s, M.pk((i - 1 : U32), i, below),
                 (pt_x(c) + M.md(pt_dx(c), t, 4096) : U32),
                 (pt_y(c) + M.md(pt_dy(c), t, 4096) : U32), c)

# The point at distance `s` (held to the path) and the blended tangent.
def sample(+a: Path, +s0: U32) -> Sm:
  +s = M.clamp(s0, 0, pa_len(a))
  sample_at(a, s, seg_at(a, s))

# The distance along the path of the point at pixel column `x_px`, for a path
# that runs left to right.
def s_at_x(+a: Path, +x_px: U32) -> U32:
  +xq = (x_px * 256 : U32)
  +i = M.min(M.sat1(pp_x(pa_depth(a), pa_pts(a), xq, 0)), (pa_n(a) - 2 : U32))
  +l = pt_at(a, i)
  +r = pt_at(a, (i + 1 : U32))
  +dx = M.max((pt_x(r) - pt_x(l) : U32), 1)
  (pt_cum(l) + M.md(M.clamp((xq - pt_x(l) : U32), 0, dx), (pt_cum(r) - pt_cum(l) : U32), dx) : U32)

# The height of the surface at pixel column `x_px` (1/256 pixel).
def y_at_x(+a: Path, +x_px: U32) -> U32:
  sm_y(sample(a, s_at_x(a, x_px)))

# ---- the paths ---------------------------------------------------------------

def dummy() -> Path:
  Path{2, 2, 0n, 0, Pl{Pt{0, 0, 0, 0, 0, 0}}, 0, 0, 0, 0, 0, 0, Nil{}}

def path_go(+ps: List<&2, Path>, +n: Nat) -> Path:
  match ps n:
    case Nil{} _:
      dummy()
    case Con{h, t} 0n:
      h
    case Con{h, t} 1n+k:
      path_go(t, k)

# Path `i` of the road.
def path_at(+ps: List<&2, Path>, +i: U32) -> Path:
  path_go(ps, U32.to_nat(i))

# ---- moving along the paths ---------------------------------------------------

# Where a move along the road has got to: `done` once it has settled, how it
# ended (0 stay, 1 off the end, 2 against the start of the world), the path and
# distance it stands at, and the distance still to cover.
type Av is Data:
  Av{done: U32, leave: U32, path: U32, s: U32, left: U32}

def av_leave(+v: Av) -> U32:
  match v:
    case Av{done, leave, path, s, left}:
      leave

def av_path(+v: Av) -> U32:
  match v:
    case Av{done, leave, path, s, left}:
      path

def av_s(+v: Av) -> U32:
  match v:
    case Av{done, leave, path, s, left}:
      s

# A portal found: whether, the distance it stands at, and where it leads.
type Fp is Data:
  Fp{found: U32, qs: U32, to_path: U32, to_s: U32}

def fp_found(+f: Fp) -> U32:
  match f:
    case Fp{found, qs, to_path, to_s}:
      found

def fp_qs(+f: Fp) -> U32:
  match f:
    case Fp{found, qs, to_path, to_s}:
      qs

def fp_to_path(+f: Fp) -> U32:
  match f:
    case Fp{found, qs, to_path, to_s}:
      to_path

def fp_to_s(+f: Fp) -> U32:
  match f:
    case Fp{found, qs, to_path, to_s}:
      to_s

# This portal, if it carries a rider moving `fwd` or `back` from `s` toward
# `target` at speed `d`.
def portal_hit(+q: Portal, +fwd: Bool, +back: Bool, +s: U32, +target: U32, +d: U32) -> Fp:
  match q:
    case Portal{qs, dir, min_speed, to_path, to_s}:
      +hit_f = Bool.and(Bool.and(fwd, M.eq(dir, 1)), Bool.and(Bool.and(M.gt(qs, s), M.le(qs, target)), M.ge(d, min_speed)))
      +hit_b = Bool.and(Bool.and(back, M.eq(dir, 4294967295)), Bool.and(Bool.and(M.lt(qs, s), M.ge(qs, target)), M.ge(M.neg(d), min_speed)))
      Bool.pick(Fp, Bool.or(hit_f, hit_b), Fp{1, qs, to_path, to_s}, Fp{0, 0, 0, 0})

# The first portal that carries the rider.
def find_portal(+ps: List<&2, Portal>, +fwd: Bool, +back: Bool, +s: U32, +target: U32, +d: U32) -> Fp:
  match ps:
    case Nil{}:
      Fp{0, 0, 0, 0}
    case Con{q, rest}:
      +here = portal_hit(q, fwd, back, s, target, d)
      Bool.pick(Fp, M.nz(fp_found(here)), here, find_portal(rest, fwd, back, s, target, d))

def adv_on(+done: U32, +leave: U32, +ps: List<&2, Path>, +path: U32, +s: U32, +left: U32, +d: U32, +a: Path) -> Av:
  match a:
    case Path{kind, n, depth, len, pts, no, np, ns, po, pp, ps2, portals}:
      +target = (s + left : U32)
      +fwd = M.gt(left, 0)
      +back = M.lt(left, 0)
      +f = find_portal(portals, fwd, back, s, target, d)
      +hop = Bool.and(M.nz(fp_found(f)), Bool.or(fwd, back))
      +edge = Bool.or(Bool.and(fwd, M.gt(target, len)), Bool.and(back, M.lt(target, 0)))
      +link_on = M.nz(M.pk(no, po, fwd))
      +linked = Bool.and(Bool.not(hop), Bool.and(edge, link_on))
      +stuck = Bool.and(Bool.not(hop), Bool.and(edge, Bool.not(link_on)))
      +moves = Bool.or(hop, linked)
      +to_path = M.pk(fp_to_path(f), M.pk(np, pp, fwd), hop)
      +to_s = M.pk(fp_to_s(f), M.pk(ns, ps2, fwd), hop)
      +to_left = M.pk((target - fp_qs(f) : U32), M.pk((target - len : U32), target, fwd), hop)
      +fresh_done = M.b2u(Bool.not(moves))
      +fresh_leave = M.pk(M.pk(1, 2, fwd), 0, stuck)
      +fresh_s = M.pk(to_s, M.pk(M.pk(len, 0, fwd), target, stuck), moves)
      +old = M.nz(done)
      Av{M.pk(1, fresh_done, old), M.pk(leave, fresh_leave, old), M.pk(path, M.pk(to_path, path, moves), old),
         M.pk(s, fresh_s, old), M.pk(left, M.pk(to_left, left, moves), old)}

# One step of the walk (a hop through a portal or a link, or the settling).
def adv_one(+ps: List<&2, Path>, +v: Av, +d: U32) -> Av:
  match v:
    case Av{done, leave, path, s, left}:
      adv_on(done, leave, ps, path, s, left, d, path_at(ps, path))

# Move along the paths by `d`, through portals and links: at most four hops.
def advance(+ps: List<&2, Path>, +path: U32, +s: U32, +d: U32) -> Av:
  adv_one(ps, adv_one(ps, adv_one(ps, adv_one(ps, Av{0, 0, path, s, d}, d), d), d), d)

# ---- landing -----------------------------------------------------------------

# The first road met moving from `a` to `b` (1/256 pixel) when `a` is on or
# above it and `b` is below it, as a flag, the path, the distance and the point.
type Hit is Data:
  Hit{ok: U32, path: U32, s: U32, x: U32, y: U32}

def hit_ok(+h: Hit) -> U32:
  match h:
    case Hit{ok, path, s, x, y}:
      ok

def hit_path(+h: Hit) -> U32:
  match h:
    case Hit{ok, path, s, x, y}:
      path

def hit_s(+h: Hit) -> U32:
  match h:
    case Hit{ok, path, s, x, y}:
      s

def hit_x(+h: Hit) -> U32:
  match h:
    case Hit{ok, path, s, x, y}:
      x

def hit_y(+h: Hit) -> U32:
  match h:
    case Hit{ok, path, s, x, y}:
      y

# The best crossing so far: how far along the move, the path and the segment.
type Bst is Data:
  Bst{ok: U32, t: U32, pi: U32, si: U32}

# Which side of a segment a point lies on, as the cross product of the segment
# and the vector from its start to the point.
def side(+ex: U32, +ey: U32, +dx: U32, +dy: U32) -> Nat:
  M.zsub(M.zmul(ex, dy), M.zmul(ey, dx))

# Whether (t, pi, si) comes before the best so far.
def earlier(+b: Bst, +t: U32, +pi: U32, +si: U32) -> Bool:
  match b:
    case Bst{ok, bt, bp, bs}:
      Bool.or(Bool.not(M.nz(ok)),
        Bool.or((t < bt : U32),
          Bool.and(U32.is_eq(t, bt), Bool.or((pi < bp : U32), Bool.and(U32.is_eq(pi, bp), (si < bs : U32))))))

# A segment that is crossed from above: where along the move, and whether the
# crossing lies over the segment (a little leeway).
def land_pass(+pi: U32, +si: U32, +ax: U32, +bx: U32, +px0: U32, +px1: U32, +d0: Nat, +d1: Nat, +best: Bst) -> Bst:
  +t = M.zratio(d0, M.zsub(d0, d1))
  +cx = (ax + M.md((bx - ax : U32), t, 65536) : U32)
  +over = Bool.and(M.ge(cx, (px0 - 256 : U32)), M.le(cx, (px1 + 256 : U32)))
  Bool.pick(Bst, Bool.and(over, earlier(best, t, pi, si)), Bst{1, t, pi, si}, best)

def land_try(+skip: Bool, +pi: U32, +si: U32, +ax: U32, +bx: U32, +px0: U32, +px1: U32, +d0: Nat, +d1: Nat, +best: Bst) -> Bst:
  match skip:
    case True{}:
      best
    case False{}:
      land_pass(pi, si, ax, bx, px0, px1, d0, d1, best)

def land_seg(+a: Path, +pi: U32, +si: U32, +ax: U32, +ay: U32, +bx: U32, +by: U32, +best: Bst) -> Bst:
  +p0 = pt_at(a, si)
  +p1 = pt_at(a, (si + 1 : U32))
  +ex = (pt_x(p1) - pt_x(p0) : U32)
  +ey = (pt_y(p1) - pt_y(p0) : U32)
  +d0 = side(ex, ey, (ax - pt_x(p0) : U32), (ay - pt_y(p0) : U32))
  +d1 = side(ex, ey, (bx - pt_x(p0) : U32), (by - pt_y(p0) : U32))
  +slack = Nat.div(Nat.mul(Nat.mul(M.isqrt(M.len2(ex, ey)), 256n), 3n), 2n)
  +skip = Bool.or(Nat.is_gt(d0, Nat.add(M.zb(), slack)), Nat.is_le(d1, M.zb()))
  land_try(skip, pi, si, ax, bx, pt_x(p0), pt_x(p1), d0, d1, best)

def land_segs(+fuel: Nat, +a: Path, +pi: U32, +si: U32, +ax: U32, +ay: U32, +bx: U32, +by: U32, +best: Bst) -> Bst:
  match fuel:
    case 0n:
      best
    case 1n+k:
      land_segs(k, a, pi, (si + 1 : U32), ax, ay, bx, by, land_seg(a, pi, si, ax, ay, bx, by, best))

# The segments of one road that a move might cross, by column.
def land_path(+a: Path, +pi: U32, +ax: U32, +ay: U32, +bx: U32, +by: U32, +best: Bst) -> Bst:
  +lo = (M.min(ax, bx) - 256 : U32)
  +hi = (M.max(ax, bx) + 256 : U32)
  +c1 = pp_x(pa_depth(a), pa_pts(a), (lo - 1 : U32), 0)
  +c2 = pp_x(pa_depth(a), pa_pts(a), hi, 0)
  +first = M.sat1(c1)
  +cnt = M.sat(M.min(c2, (pa_n(a) - 1 : U32)), first)
  Bool.pick(Bst, U32.is_eq(pa_kind(a), 2), best, land_segs(U32.to_nat(cnt), a, pi, first, ax, ay, bx, by, best))

def land_paths(+ps: List<&2, Path>, +pi: U32, +ax: U32, +ay: U32, +bx: U32, +by: U32, +best: Bst) -> Bst:
  match ps:
    case Nil{}:
      best
    case Con{h, t}:
      land_paths(t, (pi + 1 : U32), ax, ay, bx, by, land_path(h, pi, ax, ay, bx, by, best))

# The point a landing touches down on: the arrival point's projection on the
# segment, held to it.
def land_at(+a: Path, +pi: U32, +si: U32, +bx: U32, +by: U32) -> Hit:
  +p0 = pt_at(a, si)
  +seg = (pt_cum(pt_at(a, (si + 1 : U32))) - pt_cum(p0) : U32)
  +along = M.zadd(M.zmul((bx - pt_x(p0) : U32), pt_dx(p0)), M.zmul((by - pt_y(p0) : U32), pt_dy(p0)))
  +fwd = Nat.is_gt(along, M.zb())
  +s = (pt_cum(p0) + M.pk(M.min(U32.from_nat(Nat.div(Nat.sub(along, M.zb()), 4096n)), seg), 0, fwd) : U32)
  +at = sample(a, s)
  Hit{1, pi, s, sm_x(at), sm_y(at)}

def land_done(+ps: List<&2, Path>, +bx: U32, +by: U32, +best: Bst) -> Hit:
  match best:
    case Bst{ok, t, pi, si}:
      Bool.pick(Hit, M.nz(ok), land_at(path_at(ps, pi), pi, si, bx, by), Hit{0, 0, 0, 0, 0})

def land(+ps: List<&2, Path>, +ax: U32, +ay: U32, +bx: U32, +by: U32) -> Hit:
  land_done(ps, bx, by, land_paths(ps, 0, ax, ay, bx, by, Bst{0, 0, 0, 0}))
