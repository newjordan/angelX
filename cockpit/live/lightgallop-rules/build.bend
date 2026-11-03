import Base
import ./math.bend as M
import ./terrain.bend as T

# A path being laid down: pixel coordinates in, 1/256 pixel kept. The points are
# kept newest first; `build` turns them into a path with its arc lengths, its
# segment directions and the tree the rules walk.

type V2 is Data:
  V2{x: U32, y: U32}

type B is Data:
  B{kind: U32, pts: List<&2, V2>}

def b_kind(+b: B) -> U32:
  match b:
    case B{kind, pts}:
      kind

def b_pts(+b: B) -> List<&2, V2>:
  match b:
    case B{kind, pts}:
      pts

def v_x(+v: V2) -> U32:
  match v:
    case V2{x, y}:
      x

def v_y(+v: V2) -> U32:
  match v:
    case V2{x, y}:
      y

def last(+b: B) -> V2:
  match b:
    case B{kind, pts}:
      match pts:
        case Nil{}:
          V2{0, 0}
        case Con{h, t}:
          h

def new(+kind: U32, +x: U32, +y: U32) -> B:
  B{kind, [V2{(x * 256 : U32), (y * 256 : U32)}]}

# One point on the end, unless the path is already standing on it.
def push(+b: B, +x: U32, +y: U32) -> B:
  match b:
    case B{kind, pts}:
      Bool.pick(B, Bool.and(M.eq(v_x(last(b)), x), M.eq(v_y(last(b)), y)), b, B{kind, Con{V2{x, y}, pts}})

# The longest a straight run is left before it is cut, in pixels: short pieces
# keep the blended tangent honest.
def cut() -> U32:
  24

def line_go(+fuel: Nat, +i: U32, +n: U32, +b: B, +x0: U32, +y0: U32, +dx: U32, +dy: U32) -> B:
  match fuel:
    case 0n:
      b
    case 1n+k:
      line_go(k, (i + 1 : U32), n, push(b, (x0 + M.md(dx, i, n) : U32), (y0 + M.md(dy, i, n) : U32)), x0, y0, dx, dy)

# A straight run to (x, y), in pixels.
def line(+b: B, +x: U32, +y: U32) -> B:
  +x0 = v_x(last(b))
  +y0 = v_y(last(b))
  +dx = (x * 256 - x0 : U32)
  +dy = (y * 256 - y0 : U32)
  +n = M.max((U32.div(M.hyp(dx, dy), (cut() * 256 : U32)) + 1 : U32), 1)
  line_go(U32.to_nat(n), 1, n, b, x0, y0, dx, dy)

def ease_go(+fuel: Nat, +i: U32, +n: U32, +b: B, +x0: U32, +y0: U32, +dx: U32, +dy: U32) -> B:
  match fuel:
    case 0n:
      b
    case 1n+k:
      +s = M.smooth(M.div((i * 4096 : U32), n))
      ease_go(k, (i + 1 : U32), n, push(b, (x0 + M.md(dx, i, n) : U32), (y0 + M.md(dy, s, 4096) : U32)), x0, y0, dx, dy)

# An S-curve to (x, y), flat at both ends: a hill's flank.
def ease(+b: B, +x: U32, +y: U32) -> B:
  +x0 = v_x(last(b))
  +y0 = v_y(last(b))
  +dx = (x * 256 - x0 : U32)
  +dy = (y * 256 - y0 : U32)
  +n = M.max(M.div(M.abs(dx), 2048), 2)
  ease_go(U32.to_nat(n), 1, n, b, x0, y0, dx, dy)

def arc_go(+fuel: Nat, +i: U32, +n: U32, +b: B, +cx: U32, +cy: U32, +r: U32, +a0: U32, +a1: U32, +drift: U32) -> B:
  match fuel:
    case 0n:
      b
    case 1n+k:
      +a = (a0 + M.div(((a1 - a0 : U32) * i : U32), n) : U32)
      +x = (cx * 256 + M.md((r * 256 : U32), M.cos(a), 4096) + M.div(((drift * 256 : U32) * i : U32), n) : U32)
      +y = (cy * 256 + M.md((r * 256 : U32), M.sin(a), 4096) : U32)
      arc_go(k, (i + 1 : U32), n, push(b, x, y), cx, cy, r, a0, a1, drift)

# An arc of the circle of radius `r` about (cx, cy), from turn `a0` to turn
# `a1`, drifting `drift` pixels to the right over its length (a loop's lean).
def arc(+b: B, +cx: U32, +cy: U32, +r: U32, +a0: U32, +a1: U32, +drift: U32) -> B:
  +n = M.max(M.div(M.abs((a1 - a0 : U32)), 2), 1)
  arc_go(U32.to_nat(n), 1, n, b, cx, cy, r, a0, a1, drift)

# ---- the points become a path ------------------------------------------------

def rev(+xs: List<&2, V2>, +acc: List<&2, V2>) -> List<&2, V2>:
  match xs:
    case Nil{}:
      acc
    case Con{h, t}:
      rev(t, Con{h, acc})

def seg_len(+h: V2, +rest: List<&2, V2>) -> U32:
  match rest:
    case Nil{}:
      0
    case Con{n, more}:
      M.hyp((v_x(n) - v_x(h) : U32), (v_y(n) - v_y(h) : U32))

# The point `h`, with its arc length and the direction and midpoint of the
# segment it starts (none at the last point).
def pt_of(+h: V2, +rest: List<&2, V2>, +cum: U32) -> T.Pt:
  match rest:
    case Nil{}:
      T.Pt{v_x(h), v_y(h), cum, 0, 0, 0}
    case Con{n, more}:
      +dx = (v_x(n) - v_x(h) : U32)
      +dy = (v_y(n) - v_y(h) : U32)
      T.Pt{v_x(h), v_y(h), cum, M.unit_x(dx, dy), M.unit_y(dx, dy), (cum + U32.div(M.hyp(dx, dy), 2) : U32)}

# The points with their arc lengths: `cum` is the arc length at the first.
def pts_go(+xs: List<&2, V2>, +cum: U32) -> List<&2, T.Pt>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{h, rest}:
      Con{pt_of(h, rest, cum), pts_go(rest, (cum + seg_len(h, rest) : U32))}

def count(+xs: List<&2, T.Pt>) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{h, t}:
      (1 + count(t) : U32)

def last_cum(+xs: List<&2, T.Pt>) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{h, t}:
      Bool.pick(U32, M.eq(count(t), 0), T.pt_cum(h), last_cum(t))

# The depth of the smallest balanced tree with room for `n` points.
def dgo(+fuel: Nat, +d: Nat, +cap: U32, +n: U32) -> Nat:
  match fuel:
    case 0n:
      d
    case 1n+k:
      +done = M.ge(cap, n)
      dgo(k, Bool.pick(Nat, done, d, Nat.add(d, 1n)), M.pk(cap, (cap * 2 : U32), done), n)

def sentinel() -> T.Pt:
  T.Pt{2147483647, 2147483647, 2147483647, 0, 0, 0}

def pad(+m: Nat) -> List<&2, T.Pt>:
  match m:
    case 0n:
      Nil{}
    case 1n+k:
      Con{sentinel(), pad(k)}

def app(+xs: List<&2, T.Pt>, +ys: List<&2, T.Pt>) -> List<&2, T.Pt>:
  match xs:
    case Nil{}:
      ys
    case Con{h, t}:
      Con{h, app(t, ys)}

def leaves(+xs: List<&2, T.Pt>) -> List<&2, T.Pts>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{h, t}:
      Con{T.Pl{h}, leaves(t)}

def first_cum(+t: T.Pts) -> U32:
  match t:
    case T.Pl{p}:
      T.pt_cum(p)
    case T.Pn{sc, sx, lo, hi}:
      first_cum(lo)

def first_x(+t: T.Pts) -> U32:
  match t:
    case T.Pl{p}:
      T.pt_x(p)
    case T.Pn{sc, sx, lo, hi}:
      first_x(lo)

# Neighbours joined in pairs, each pair remembering where its right half starts.
def pair_up(+xs: List<&2, T.Pts>) -> List<&2, T.Pts>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{a, more}:
      match more:
        case Nil{}:
          Con{a, Nil{}}
        case Con{b, rest}:
          Con{T.Pn{first_cum(b), first_x(b), a, b}, pair_up(rest)}

def climb(+d: Nat, +xs: List<&2, T.Pts>) -> T.Pts:
  match d:
    case 0n:
      match xs:
        case Nil{}:
          T.Pl{sentinel()}
        case Con{h, t}:
          h
    case 1n+k:
      climb(k, pair_up(xs))

def tree(+xs: List<&2, T.Pt>, +n: U32, +d: Nat) -> T.Pts:
  climb(d, leaves(app(xs, pad(U32.to_nat((U32.shln(1, d) - n : U32))))))

# The path the builder has laid down, with nothing yet linked to it.
def build(+b: B) -> T.Path:
  +ps = pts_go(rev(b_pts(b), Nil{}), 0)
  +n = count(ps)
  +d = dgo(24n, 0n, 1, n)
  T.Path{b_kind(b), n, d, last_cum(ps), tree(ps, n, d), 0, 0, 0, 0, 0, 0, Nil{}}
