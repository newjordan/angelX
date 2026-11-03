import Base
import ./math.bend as M
import ./terrain.bend as T
import ./build.bend as B

# The Moonwell Road, laid out once: four acts of one road at dusk.
#
# 1. The Downhill Gate: a run-up, a downhill, a pad, the first snuffer, a
#    trench with a hay bale at its foot, the first lantern post, a rolling
#    meadow and a hay loft.
# 2. The Wheel Arch: the loop, the half-pipe, rolling crests, the corkscrew
#    coil, a rhythm of small valleys, then the fork: riders at a gallop take the
#    high rails, a rider who hops over its mouth the valley road, and the two
#    meet again.
# 3. The Reed Fen: a valley of reeds and brambles with snuffers drifting over it
#    in a line to be chained, a bridge of sparks on the far bank, reed hills and
#    a second, shorter marsh.
# 4. The Bell Gate: a last downhill, a second loop, the warden, the bell.
#
# Heights are written as offsets from `G`; a larger number is lower. A foe's
# kind is 0 snuffer, 1 floater, 2 bramble, 3 warden; a spring's 0 up, 1 diagonal;
# a crate's 0 shield, 1 sparks, 2 spur.

type Fs is Data:
  Fs{kind: U32, x: U32, y: U32, path: U32, range: U32, min_seats: U32, phase: U32}

type Sp is Data:
  Sp{x: U32, y: U32}

type Spr is Data:
  Spr{x: U32, y: U32, kind: U32}

type Pad is Data:
  Pad{x: U32, y: U32}

type Post is Data:
  Post{x: U32, y: U32, path: U32, s: U32, hold: U32}

type Crate is Data:
  Crate{x: U32, y: U32, kind: U32}

type Sign is Data:
  Sign{x: U32, y: U32, words: String}

type Road is Data:
  Road{paths: List<&2, T.Path>, foes: List<&2, Fs>, sparks: List<&2, Sp>,
       springs: List<&2, Spr>, pads: List<&2, Pad>, posts: List<&2, Post>,
       crates: List<&2, Crate>, signs: List<&2, Sign>,
       width: U32, dead_y: U32, a0: U32, a1: U32, a2: U32, a3: U32,
       main: U32, start_path: U32, start_s: U32, bell_x: U32, bell_y: U32, par: U32}

# ---- the numbers -------------------------------------------------------------

# The road's base height, pixels.
def g() -> U32:
  600

# How far behind a lit post the Dimming is held, pixels, for one knight; and for
# the last lantern, the one before the warden's gate.
def hold() -> U32:
  520

def last_hold() -> U32:
  1000

def a1() -> U32:
  0

def a2() -> U32:
  4300

def a3() -> U32:
  11300

def a4() -> U32:
  15700

def end() -> U32:
  20000

# ---- the main road -----------------------------------------------------------

# The road being laid left to right, remembering its height.
type Rd is Data:
  Rd{b: B.B, y: U32}

def rd_b(+r: Rd) -> B.B:
  match r:
    case Rd{b, y}:
      b

def rd_y(+r: Rd) -> U32:
  match r:
    case Rd{b, y}:
      y

# Level ground to column `x`.
def flat(+r: Rd, +x: U32) -> Rd:
  Rd{B.line(rd_b(r), x, (g() + rd_y(r) : U32)), rd_y(r)}

# An S-curve to column `x`, `dy` pixels lower (negative: higher).
def slope(+r: Rd, +x: U32, +dy: U32) -> Rd:
  Rd{B.ease(rd_b(r), x, (g() + rd_y(r) + dy : U32)), (rd_y(r) + dy : U32)}

# A stretch of the road, as the calls that lay it: 0 level ground to `x`, 1 an
# S-curve to `x` that is `dy` lower.
type Op is Data:
  Op{kind: U32, x: U32, dy: U32}

def step(+r: Rd, +o: Op) -> Rd:
  match o:
    case Op{kind, x, dy}:
      Bool.pick(Rd, M.eq(kind, 0), flat(r, x), slope(r, x, dy))

def lay(+ops: List<&2, Op>, +r: Rd) -> Rd:
  match ops:
    case Nil{}:
      r
    case Con{o, rest}:
      lay(rest, step(r, o))

def f(+x: U32) -> Op:
  Op{0, x, 0}

def s(+x: U32, +dy: U32) -> Op:
  Op{1, x, dy}

def loop1() -> U32:
  (a2() + 400 : U32)

def pipe() -> U32:
  (a2() + 1120 : U32)

def crests() -> U32:
  (pipe() + 680 : U32)

def coil_x() -> U32:
  (crests() + 1020 : U32)

def rhythm() -> U32:
  (coil_x() + 700 : U32)

def fork_x() -> U32:
  (rhythm() + 900 : U32)

def reeds() -> U32:
  (a3() + 2300 : U32)

def marsh() -> U32:
  (reeds() + 1400 : U32)

def loop2() -> U32:
  (a4() + 1060 : U32)

# Act 1 and the long downhill into the Wheel Arch.
def seg1() -> List<&2, Op>:
  [f((a1() + 380 : U32)), s(760, 60), f(1060), s(1260, M.neg(35)), s(1500, 35), f(1700),
   s(1840, 70), f(1870), s(2010, M.neg(70)), f(2560),
   s(2760, M.neg(30)), s(2960, 30), s(3160, M.neg(30)), s(3360, 30), f(3560),
   s(a2(), 110), f((a2() + 380 : U32))]

# The loop's run, the half-pipe and four rolling crests, to the coil.
def seg2() -> List<&2, Op>:
  [f((a2() + 900 : U32)), s((a2() + 1040 : U32), M.neg(30)),
   f(pipe()), s((pipe() + 80 : U32), 66), f((pipe() + 240 : U32)), s((pipe() + 320 : U32), M.neg(66)), f((pipe() + 680 : U32)),
   s((crests() + 220 : U32), M.neg(50)), s((crests() + 440 : U32), 50), s((crests() + 660 : U32), M.neg(50)), s((crests() + 880 : U32), 50), f((crests() + 1020 : U32))]

# The coil, a rhythm of three small valleys, and the fork.
def seg3() -> List<&2, Op>:
  [f((coil_x() + 700 : U32)),
   s((rhythm() + 120 : U32), 40), s((rhythm() + 240 : U32), M.neg(40)), f((rhythm() + 300 : U32)),
   s((rhythm() + 420 : U32), 40), s((rhythm() + 540 : U32), M.neg(40)), f((rhythm() + 600 : U32)),
   s((rhythm() + 720 : U32), 40), s((rhythm() + 840 : U32), M.neg(40)), f((rhythm() + 900 : U32))]

# The valley road under the fork, the Reed Fen, the marsh and the last downhill.
def seg4() -> List<&2, Op>:
  [f(fork_x()), s((fork_x() + 300 : U32), 70), f((fork_x() + 600 : U32)), s((fork_x() + 1040 : U32), M.neg(70)), f((a3() - 700 : U32)),
   f(a3()), s((a3() + 240 : U32), 50), s((a3() + 420 : U32), 90), f((a3() + 1300 : U32)), s((a3() + 1580 : U32), M.neg(90)), s((a3() + 1720 : U32), M.neg(50)), f((a3() + 2300 : U32)),
   s((reeds() + 220 : U32), M.neg(14)), s((reeds() + 440 : U32), 14), s((reeds() + 660 : U32), M.neg(14)), s((reeds() + 880 : U32), 14), f((reeds() + 1400 : U32)),
   s((marsh() + 160 : U32), 70), f((marsh() + 480 : U32)), s((marsh() + 700 : U32), M.neg(70)), f(a4()),
   s((a4() + 600 : U32), 60), f((a4() + 1060 : U32))]

# The bell gate's straight, its last rise, and the world's far edge.
def seg5() -> List<&2, Op>:
  [f((a4() + 1500 : U32)), s((a4() + 1800 : U32), M.neg(40)), f((end() - 40 : U32))]

# ---- things hung on a path ---------------------------------------------------

def with_prev(+a: T.Path, +path: U32, +s: U32) -> T.Path:
  match a:
    case T.Path{kind, n, depth, len, pts, no, np, ns, po, pp, ps, portals}:
      T.Path{kind, n, depth, len, pts, no, np, ns, 1, path, s, portals}

def with_next(+a: T.Path, +path: U32, +s: U32) -> T.Path:
  match a:
    case T.Path{kind, n, depth, len, pts, no, np, ns, po, pp, ps, portals}:
      T.Path{kind, n, depth, len, pts, 1, path, s, po, pp, ps, portals}

def snoc(+xs: List<&2, T.Portal>, +q: T.Portal) -> List<&2, T.Portal>:
  match xs:
    case Nil{}:
      Con{q, Nil{}}
    case Con{h, t}:
      Con{h, snoc(t, q)}

# One more portal, after the ones the path has.
def with_portal(+a: T.Path, +q: T.Portal) -> T.Path:
  match a:
    case T.Path{kind, n, depth, len, pts, no, np, ns, po, pp, ps, portals}:
      T.Path{kind, n, depth, len, pts, no, np, ns, po, pp, ps, snoc(portals, q)}

# The path of a loop of radius `r` standing on the road at column `x`, whose
# ground is at height `y`: the path of its curl, entered at the bottom and left
# again `drift` pixels further on.
def ring(+x: U32, +y: U32, +r: U32, +drift: U32) -> T.Path:
  B.build(B.arc(B.new(2, x, y), x, (y - r : U32), r, 64, M.neg(192), drift))

# The ring hung on the road: entered from the road at the column its bottom
# starts on and left back onto the road at the column it ends on. `at` is the
# ring's index.
def hang_ring(+road: T.Path, +at: U32, +r: T.Path) -> T.Path:
  +enter = U32.div(T.pt_x(T.pt_at(r, 0)), 256)
  +leave = U32.div(T.pt_x(T.pt_at(r, (T.pa_n(r) - 1 : U32))), 256)
  with_portal(
    with_portal(road, T.Portal{T.s_at_x(road, enter), 1, 0, at, 0}),
    T.Portal{T.s_at_x(road, leave), M.neg(1), 0, at, T.pa_len(r)})

def link_ring(+road: T.Path, +r: T.Path) -> T.Path:
  +enter = U32.div(T.pt_x(T.pt_at(r, 0)), 256)
  +leave = U32.div(T.pt_x(T.pt_at(r, (T.pa_n(r) - 1 : U32))), 256)
  with_next(with_prev(r, 0, T.s_at_x(road, enter)), 0, T.s_at_x(road, leave))

# The height of the main road at column `x`, in pixels.
def road_y(+m: T.Path, +x: U32) -> U32:
  M.div(T.y_at_x(m, x), 256)

def spot(+m: T.Path, +kind: U32, +x: U32, +range: U32, +min_seats: U32, +phase: U32) -> Fs:
  Fs{kind, x, road_y(m, x), 0, range, min_seats, phase}

# ---- the sparks --------------------------------------------------------------

# Where a line of sparks rides: 0 the road, 1 a fixed height `v`, 2 the arc of
# the lip's bale (about column `v`), 3 the bridge's span (from `v` to `w`).
def spark_y(+m: T.Path, +kind: U32, +v: U32, +w: U32, +x: U32) -> U32:
  +road = road_y(m, x)
  +dx = (x - v : U32)
  +arc = M.clamp(M.div(((78 * dx : U32) * M.max((266 - dx : U32), 0) : U32), 17689), 10, 90)
  +bridge = M.min(M.div(((x - v : U32) * (w - x : U32) : U32), 560), 90)
  M.pk(road, M.pk(v, M.pk((road - arc : U32), (road - bridge : U32), M.eq(kind, 2)), M.eq(kind, 1)), M.eq(kind, 0))

def spark_go(+fuel: Nat, +x: U32, +step: U32, +lift: U32, +m: T.Path, +kind: U32, +v: U32, +w: U32, +acc: List<&2, Sp>) -> List<&2, Sp>:
  match fuel:
    case 0n:
      acc
    case 1n+k:
      spark_go(k, (x + step : U32), step, lift, m, kind, v, w, Con{Sp{x, (spark_y(m, kind, v, w, x) - lift : U32)}, acc})

# A line of sparks from `x0` to `x1` every `step` pixels, `lift` above where it rides.
def line(+acc: List<&2, Sp>, +m: T.Path, +x0: U32, +x1: U32, +step: U32, +lift: U32, +kind: U32, +v: U32, +w: U32) -> List<&2, Sp>:
  spark_go(U32.to_nat(M.pk((U32.div((x1 - x0 : U32), step) + 1 : U32), 0, M.le(x0, x1))), x0, step, lift, m, kind, v, w, acc)

# The spark lines over the chains of floaters: the fen's, and the marsh's.
def fen_chain(+fuel: Nat, +k: U32, +m: T.Path, +acc: List<&2, Sp>) -> List<&2, Sp>:
  match fuel:
    case 0n:
      acc
    case 1n+n:
      fen_chain(n, (k + 1 : U32), m, Con{Sp{(a3() + 340 + k * 100 : U32), (road_y(m, (a3() + 600 : U32)) - 100 - U32.mod(k, 3) * 8 : U32)}, acc})

def marsh_chain(+fuel: Nat, +k: U32, +m: T.Path, +acc: List<&2, Sp>) -> List<&2, Sp>:
  match fuel:
    case 0n:
      acc
    case 1n+n:
      marsh_chain(n, (k + 1 : U32), m, Con{Sp{(marsh() + 90 + k * 110 : U32), (road_y(m, (marsh() + 300 : U32)) - 112 - U32.mod(k, 2) * 30 : U32)}, acc})

def sp_x(+s: Sp) -> U32:
  match s:
    case Sp{x, y}:
      x

def sp_y(+s: Sp) -> U32:
  match s:
    case Sp{x, y}:
      y

# Sorted by column then height; a spark in the same place twice is one.
def sp_before(+a: Sp, +b: Sp) -> Bool:
  Bool.or(M.lt(sp_x(a), sp_x(b)), Bool.and(M.eq(sp_x(a), sp_x(b)), M.le(sp_y(a), sp_y(b))))

def sp_insert(+xs: List<&2, Sp>, +s: Sp) -> List<&2, Sp>:
  match xs:
    case Nil{}:
      Con{s, Nil{}}
    case Con{h, t}:
      Bool.pick(List<&2, Sp>, sp_before(s, h), Con{s, xs}, Con{h, sp_insert(t, s)})

def sp_sort(+xs: List<&2, Sp>, +acc: List<&2, Sp>) -> List<&2, Sp>:
  match xs:
    case Nil{}:
      acc
    case Con{h, t}:
      sp_sort(t, sp_insert(acc, h))

def sp_same(+h: Sp, +t: List<&2, Sp>) -> Bool:
  match t:
    case Nil{}:
      False{}
    case Con{n, more}:
      Bool.and(M.eq(sp_x(h), sp_x(n)), M.eq(sp_y(h), sp_y(n)))

def sp_dedup(+xs: List<&2, Sp>) -> List<&2, Sp>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{h, t}:
      +rest = sp_dedup(t)
      Bool.pick(List<&2, Sp>, sp_same(h, t), rest, Con{h, rest})

# ---- the foes ----------------------------------------------------------------

def fs_x(+f: Fs) -> U32:
  match f:
    case Fs{kind, x, y, path, range, min_seats, phase}:
      x

def fs_min(+f: Fs) -> U32:
  match f:
    case Fs{kind, x, y, path, range, min_seats, phase}:
      min_seats

# Whether `a` stands after `b` by column, then by the seats it takes to wake it.
def fs_after(+a: Fs, +b: Fs) -> Bool:
  Bool.or((fs_x(a) > fs_x(b) : U32), Bool.and(U32.is_eq(fs_x(a), fs_x(b)), (fs_min(a) > fs_min(b) : U32)))

# A stable insertion: `f` goes before the first foe that stands after it.
def fs_insert(+xs: List<&2, Fs>, +f: Fs) -> List<&2, Fs>:
  match xs:
    case Nil{}:
      Con{f, Nil{}}
    case Con{h, t}:
      Bool.pick(List<&2, Fs>, fs_after(h, f), Con{f, xs}, Con{h, fs_insert(t, f)})

def fs_sort(+xs: List<&2, Fs>, +acc: List<&2, Fs>) -> List<&2, Fs>:
  match xs:
    case Nil{}:
      acc
    case Con{h, t}:
      fs_sort(t, fs_insert(acc, h))

def fs_app(+xs: List<&2, Fs>, +ys: List<&2, Fs>) -> List<&2, Fs>:
  match xs:
    case Nil{}:
      ys
    case Con{h, t}:
      Con{h, fs_app(t, ys)}

# Nine floaters in a line over the fen, bobbing at their own paces.
def floaters_a(+fuel: Nat, +k: U32, +m: T.Path, +acc: List<&2, Fs>) -> List<&2, Fs>:
  match fuel:
    case 0n:
      acc
    case 1n+n:
      floaters_a(n, (k + 1 : U32), m,
        Con{Fs{1, (a3() + 340 + k * 100 : U32), (road_y(m, (a3() + 600 : U32)) - 96 - U32.mod(k, 3) * 8 : U32), 0, 6, 0, (k * 13 : U32)}, acc})

# More floaters for a fuller table: a second, higher line over the fen.
def floaters_b(+fuel: Nat, +k: U32, +m: T.Path, +acc: List<&2, Fs>) -> List<&2, Fs>:
  match fuel:
    case 0n:
      acc
    case 1n+n:
      floaters_b(n, (k + 1 : U32), m,
        Con{Fs{1, (a3() + 490 + k * 220 : U32), (road_y(m, (a3() + 600 : U32)) - 160 : U32), 0, 10, (2 + k : U32), (k * 29 : U32)}, acc})

# The marsh's chain, staggered over the short water.
def floaters_c(+fuel: Nat, +k: U32, +m: T.Path, +acc: List<&2, Fs>) -> List<&2, Fs>:
  match fuel:
    case 0n:
      acc
    case 1n+n:
      floaters_c(n, (k + 1 : U32), m,
        Con{Fs{1, (marsh() + 90 + k * 110 : U32), (road_y(m, (marsh() + 300 : U32)) - 112 - U32.mod(k, 2) * 30 : U32), 0, 8,
               M.pk(0, (k - 2 : U32), (k < 4 : U32)), (k * 17 : U32)}, acc})

# ---- the whole road ----------------------------------------------------------

def post_at(+m: T.Path, +x: U32, +hold: U32) -> Post:
  Post{x, road_y(m, x), 0, T.s_at_x(m, x), hold}

def explicit_foes(+m: T.Path, +top: U32) -> List<&2, Fs>:
  [spot(m, 2, 640, 0, 0, 0),
   spot(m, 0, 1000, 60, 0, 0),
   spot(m, 2, 1420, 0, 0, 0),
   spot(m, 0, 2120, 80, 0, 40),
   spot(m, 0, 2660, 70, 0, 10),
   spot(m, 0, 3000, 70, 2, 30),
   spot(m, 2, 3100, 0, 3, 0),
   spot(m, 0, 3920, 60, 4, 0),
   spot(m, 0, (a2() + 700 : U32), 90, 0, 20),
   spot(m, 0, (a2() + 800 : U32), 60, 3, 30),
   spot(m, 2, (crests() - 100 : U32), 0, 0, 0),
   spot(m, 0, (crests() + 300 : U32), 70, 0, 0),
   spot(m, 0, (crests() + 760 : U32), 60, 4, 50),
   spot(m, 2, (crests() + 900 : U32), 0, 0, 0),
   spot(m, 0, (coil_x() + 330 : U32), 90, 0, 0),
   spot(m, 2, (coil_x() + 430 : U32), 0, 5, 0),
   spot(m, 0, (rhythm() + 60 : U32), 60, 0, 0),
   spot(m, 0, (rhythm() + 480 : U32), 60, 2, 25),
   spot(m, 2, (rhythm() + 760 : U32), 0, 0, 0),
   spot(m, 0, (fork_x() + 450 : U32), 50, 0, 0),
   spot(m, 2, (fork_x() + 560 : U32), 0, 0, 0),
   spot(m, 0, (fork_x() + 720 : U32), 60, 2, 20),
   spot(m, 0, (fork_x() + 1240 : U32), 80, 0, 0),
   spot(m, 0, (a3() - 1000 : U32), 90, 2, 0),
   spot(m, 0, (a3() - 500 : U32), 80, 0, 0),
   Fs{2, (fork_x() + 530 : U32), top, 4, 0, 0, 0},
   Fs{2, (fork_x() + 780 : U32), top, 5, 0, 3, 0},
   Fs{0, (fork_x() + 1020 : U32), top, 6, 40, 0, 0},
   spot(m, 2, (a3() + 600 : U32), 0, 0, 0),
   spot(m, 0, (a3() + 740 : U32), 90, 0, 0),
   spot(m, 2, (a3() + 1000 : U32), 0, 0, 0),
   spot(m, 0, (a3() + 1140 : U32), 70, 2, 35),
   spot(m, 2, (a3() + 1280 : U32), 0, 4, 0),
   spot(m, 0, (reeds() - 200 : U32), 80, 0, 0),
   spot(m, 0, (reeds() + 300 : U32), 80, 0, 0),
   spot(m, 2, (reeds() + 500 : U32), 0, 0, 0),
   spot(m, 0, (reeds() + 700 : U32), 80, 3, 0),
   spot(m, 0, (reeds() + 1000 : U32), 90, 0, 0),
   spot(m, 2, (reeds() + 1160 : U32), 0, 5, 0),
   spot(m, 0, (marsh() + 250 : U32), 60, 0, 0),
   spot(m, 2, (marsh() + 150 : U32), 0, 0, 0),
   spot(m, 2, (marsh() + 450 : U32), 0, 3, 0),
   spot(m, 0, (a4() + 100 : U32), 80, 0, 0),
   spot(m, 0, (a4() + 800 : U32), 90, 0, 0),
   spot(m, 0, (loop2() + 420 : U32), 80, 0, 0),
   spot(m, 0, (loop2() + 480 : U32), 40, 4, 20),
   spot(m, 2, (a4() + 1760 : U32), 0, 0, 0),
   spot(m, 0, (a4() + 2100 : U32), 70, 0, 0),
   spot(m, 0, (a4() + 2400 : U32), 70, 3, 0),
   spot(m, 0, (a4() + 2900 : U32), 60, 2, 0),
   spot(m, 3, (a4() + 3350 : U32), 0, 0, 0)]

def explicit_sparks(+m: T.Path, +top: U32, +loft_top: U32) -> List<&2, Sp>:
  +pipe_c = (pipe() + 420 : U32)
  +l0 = line(Nil{}, m, 220, 340, 20, 14, 0, 0, 0)
  +l1 = line(l0, m, 820, 960, 20, 14, 0, 0, 0)
  +l2 = line(l1, m, 1160, 1420, 26, 16, 0, 0, 0)
  +l3 = line(l2, m, 1720, 2000, 28, 40, 1, (g() + 20 : U32), 0)
  +l4 = line(l3, m, 2060, 2200, 24, 14, 0, 0, 0)
  +l5 = line(l4, m, 2600, 3480, 44, 14, 0, 0, 0)
  +l6 = line(l5, m, 3560, 3840, 28, 14, 1, loft_top, 0)
  +l7 = line(l6, m, 3600, 4180, 48, 14, 0, 0, 0)
  +l8 = line(l7, m, (a2() + 40 : U32), (a2() + 340 : U32), 26, 14, 0, 0, 0)
  +l9 = line(l8, m, (loop1() + 90 : U32), (loop1() + 520 : U32), 34, 14, 0, 0, 0)
  +l10 = line(l9, m, (pipe() + 10 : U32), (pipe() + 330 : U32), 24, 20, 0, 0, 0)
  +l11 = line(l10, m, (pipe() + 400 : U32), (pipe() + 660 : U32), 40, 14, 0, 0, 0)
  +l12 = line(l11, m, (pipe() + 700 : U32), (pipe() + 1000 : U32), 44, 14, 0, 0, 0)
  +l13 = line(l12, m, (pipe() + 470 : U32), (pipe() + 680 : U32), 20, 0, 2, pipe_c, 0)
  +l14 = line(l13, m, (crests() + 1040 : U32), (coil_x() - 20 : U32), 40, 14, 0, 0, 0)
  +l15 = line(l14, m, (coil_x() + 150 : U32), (rhythm() - 20 : U32), 44, 14, 0, 0, 0)
  +l16 = line(l15, m, rhythm(), (fork_x() - 60 : U32), 38, 14, 0, 0, 0)
  +l17 = line(l16, m, (fork_x() + 40 : U32), (fork_x() + 1040 : U32), 44, 14, 0, 0, 0)
  +l18 = line(l17, m, (fork_x() + 400 : U32), (fork_x() + 540 : U32), 28, 14, 1, top, 0)
  +l19 = line(l18, m, (fork_x() + 620 : U32), (fork_x() + 860 : U32), 30, 14, 1, top, 0)
  +l20 = line(l19, m, (fork_x() + 940 : U32), (fork_x() + 1100 : U32), 30, 14, 1, top, 0)
  +l21 = line(l20, m, (fork_x() + 1200 : U32), (a3() - 40 : U32), 48, 14, 0, 0, 0)
  +l22 = line(l21, m, (a3() + 60 : U32), (a3() + 230 : U32), 30, 14, 0, 0, 0)
  +l23 = line(l22, m, (a3() + 440 : U32), (a3() + 1280 : U32), 56, 14, 0, 0, 0)
  +l24 = line(l23, m, (a3() + 1340 : U32), (a3() + 1700 : U32), 28, 40, 3, (a3() + 1340 : U32), (a3() + 1700 : U32))
  +l25 = line(l24, m, (a3() + 1760 : U32), (a3() + 2260 : U32), 46, 14, 0, 0, 0)
  +l26 = line(l25, m, (reeds() + 20 : U32), (reeds() + 1380 : U32), 48, 14, 0, 0, 0)
  +l27 = line(l26, m, (marsh() + 20 : U32), (marsh() + 140 : U32), 30, 14, 0, 0, 0)
  +l28 = line(l27, m, (marsh() + 180 : U32), (marsh() + 460 : U32), 56, 14, 0, 0, 0)
  +l29 = line(l28, m, (marsh() + 720 : U32), (a4() - 20 : U32), 44, 14, 0, 0, 0)
  +l30 = line(l29, m, (a4() + 40 : U32), (a4() + 560 : U32), 40, 14, 0, 0, 0)
  +l31 = line(l30, m, (a4() + 640 : U32), (a4() + 1030 : U32), 36, 14, 0, 0, 0)
  +l32 = line(l31, m, (loop2() + 90 : U32), (loop2() + 400 : U32), 34, 14, 0, 0, 0)
  +l33 = line(l32, m, (a4() + 1560 : U32), (a4() + 2000 : U32), 44, 14, 0, 0, 0)
  +l34 = line(l33, m, (a4() + 2100 : U32), (a4() + 2700 : U32), 46, 14, 0, 0, 0)
  +l35 = line(l34, m, (a4() + 3000 : U32), (a4() + 3300 : U32), 28, 14, 0, 0, 0)
  marsh_chain(6n, 0, m, fen_chain(9n, 0, m, l35))

def signs(+m: T.Path, +top: U32) -> List<&2, Sign>:
  [Sign{150, road_y(m, 150), "A D RUN"},
   Sign{560, road_y(m, 560), "E JUMP"},
   Sign{760, road_y(m, 760), "S ROLL"},
   Sign{1700, road_y(m, 1700), "SPACE SPUR"},
   Sign{3400, road_y(m, 3400), "HAY LOFT"},
   Sign{(a2() + 200 : U32), road_y(m, (a2() + 200 : U32)), "SPEED HOLDS"},
   Sign{(pipe() - 60 : U32), road_y(m, (pipe() - 60 : U32)), "S E CHARGE"},
   Sign{(fork_x() - 230 : U32), road_y(m, (fork_x() - 230 : U32)), "FAST UP"},
   Sign{(fork_x() - 60 : U32), road_y(m, (fork_x() - 60 : U32)), "SLOW DOWN"},
   Sign{(a3() + 160 : U32), road_y(m, (a3() + 160 : U32)), "E E E CHAIN"},
   Sign{(fork_x() + 440 : U32), top, "E HOP"},
   Sign{(a4() + 3100 : U32), road_y(m, (a4() + 3100 : U32)), "ROLL OR LEAP"}]

def paths_of(+m0: T.Path, +ring1: T.Path, +ring2: T.Path, +ring3: T.Path, +y_fork: U32, +top: U32, +loft_top: U32) -> List<&2, T.Path>:
  [with_portal(
     hang_ring(hang_ring(hang_ring(m0, 1, ring1), 2, ring2), 3, ring3),
     T.Portal{T.s_at_x(m0, fork_x()), 1, 1200, 4, 0}),
   link_ring(m0, ring1), link_ring(m0, ring2), link_ring(m0, ring3),
   # The high rails over the valley: level with the road at the fork, then
   # lifting away from it, planks with gaps between them, and the hay loft.
   B.build(B.line(B.ease(B.line(B.new(1, fork_x(), (g() + y_fork : U32)), (fork_x() + 100 : U32), (g() + y_fork : U32)), (fork_x() + 380 : U32), top), (fork_x() + 560 : U32), top)),
   B.build(B.line(B.new(1, (fork_x() + 620 : U32), top), (fork_x() + 860 : U32), top)),
   with_next(B.build(B.ease(B.line(B.new(1, (fork_x() + 940 : U32), top), (fork_x() + 1000 : U32), top), (fork_x() + 1180 : U32), (g() + y_fork : U32))), 0, T.s_at_x(m0, (fork_x() + 1180 : U32))),
   B.build(B.line(B.new(1, 3540, loft_top), 3860, loft_top))]

def springs_of(+m0: T.Path) -> List<&2, Spr>:
  [Spr{1855, road_y(m0, 1855), 0}, Spr{3480, road_y(m0, 3480), 0}, Spr{(pipe() + 420 : U32), road_y(m0, (pipe() + 420 : U32)), 1},
   Spr{(a3() + 700 : U32), road_y(m0, (a3() + 700 : U32)), 0}, Spr{(marsh() + 330 : U32), road_y(m0, (marsh() + 330 : U32)), 0}]

def pads_of(+m0: T.Path, +top: U32) -> List<&2, Pad>:
  [Pad{880, road_y(m0, 880)}, Pad{(fork_x() + 450 : U32), top}, Pad{(fork_x() + 700 : U32), top}, Pad{(a4() + 460 : U32), road_y(m0, (a4() + 460 : U32))}]

# The last lantern, before the warden, holds the Gloam furthest back.
def posts_of(+m0: T.Path) -> List<&2, Post>:
  [post_at(m0, 2300, hold()), post_at(m0, (a2() + 960 : U32), hold()), post_at(m0, (a3() - 900 : U32), hold()),
   post_at(m0, (a3() + 2340 : U32), hold()), post_at(m0, (a4() + 1560 : U32), hold()), post_at(m0, (a4() + 2800 : U32), last_hold())]

def crates_of(+m0: T.Path, +top: U32, +loft_top: U32) -> List<&2, Crate>:
  [Crate{1560, road_y(m0, 1560), 0}, Crate{3700, loft_top, 1}, Crate{(crests() + 140 : U32), road_y(m0, (crests() + 140 : U32)), 0},
   Crate{(fork_x() + 500 : U32), top, 2}, Crate{(a3() + 2200 : U32), road_y(m0, (a3() + 2200 : U32)), 0},
   Crate{(a4() + 300 : U32), road_y(m0, (a4() + 300 : U32)), 2}, Crate{(a4() + 2400 : U32), road_y(m0, (a4() + 2400 : U32)), 0}]

# The pieces of the road around the finished main road `m0` and the heights
# its four landmarks stand at.
def assemble(+y_loop1: U32, +y_coil: U32, +y_fork: U32, +y_loop2: U32, +m0: T.Path) -> Road:
  +top = (g() + y_fork - 78 : U32)
  +loft_top = (g() + 60 - 92 : U32)
  +ring1 = ring(loop1(), (g() + y_loop1 : U32), 40, 16)
  +ring2 = ring(coil_x(), (g() + y_coil : U32), 26, 130)
  +ring3 = ring(loop2(), (g() + y_loop2 : U32), 40, 16)
  Road{paths_of(m0, ring1, ring2, ring3, y_fork, top, loft_top),
       fs_sort(fs_app(explicit_foes(m0, top), floaters_c(6n, 0, m0, floaters_b(4n, 0, m0, floaters_a(9n, 0, m0, Nil{})))), Nil{}),
       sp_dedup(sp_sort(explicit_sparks(m0, top, loft_top), Nil{})),
       springs_of(m0), pads_of(m0, top), posts_of(m0), crates_of(m0, top, loft_top), signs(m0, top),
       end(), (g() + 420 : U32), a1(), a2(), a3(), a4(), 0, 0, T.s_at_x(m0, 96),
       (a4() + 3900 : U32), road_y(m0, (a4() + 3900 : U32)), 5100}

def road4(+y_loop1: U32, +y_coil: U32, +y_fork: U32, +r4: Rd) -> Road:
  assemble(y_loop1, y_coil, y_fork, rd_y(r4), B.build(rd_b(lay(seg5(), r4))))

def road3(+y_loop1: U32, +y_coil: U32, +r3: Rd) -> Road:
  road4(y_loop1, y_coil, rd_y(r3), lay(seg4(), r3))

def road2(+y_loop1: U32, +r2: Rd) -> Road:
  road3(y_loop1, rd_y(r2), lay(seg3(), r2))

def road1(+r1: Rd) -> Road:
  road2(rd_y(r1), lay(seg2(), r1))

# The Moonwell Road.
def road() -> Road:
  road1(lay(seg1(), Rd{B.new(0, 0, g()), 0}))
