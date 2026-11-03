import Base
import ./answer.bend as Ans
import ./map.bend as Map
import ./nums.bend as N

# King Cinder on the summit. He cannot be hit head on. He stomps, sweeps and
# rains embers by a pattern that tells before it strikes; after a few of them he
# kneels, and the crown that sits on his head comes within reach. A ground pound
# onto the crown (any knight's) knocks a plate off it. Three crowns, each
# heavier than the last, each a harder pattern; a plate for every seat at the
# table, never fewer.
#
# One call a tick: the tick number, how many sit, the feet of the knights
# standing, his state and his hazards in; his state and hazards after the tick
# out. Everything is integer pixels and ticks, so the same feet on the same tick
# replay the same fight.
#
# The feet arrive in six fixed slots and his hazards as a list. A list is a
# `Type` in Bend: a walk consumes it, so the tick filters his hazards once and
# builds the new ones from nothing — the order the session has always used.
#
# A beat and an attack travel as indices, never as text: 0 Dormant, 1 Rest,
# 2 Tell, 3 Strike, 4 Wince, 5 Rage, 6 Down; and 0 Stomp, 1 Sweep, 2 Embers.

def tile() -> U32:
  16

def width() -> U32:
  48

def head_up() -> U32:
  64

def head_down() -> U32:
  44

def phases() -> U32:
  3

def reach() -> U32:
  100

def summit_ox() -> U32:
  392

def summit_oy() -> U32:
  10

# The floor of the arena, pixel row: (SUMMIT.oy + 12) * TILE.
def floor() -> U32:
  352

def mark_home(+p: Map.P) -> U32:
  match p:
    case Map.P{+x, +y}:
      N.add(N.mul(x, tile()), U32.div(tile(), 2))

# Where he stands to begin with, pixels: the map's cinder mark, centre of its
# cell.
def home() -> U32:
  mark_home(Map.cinder())

# The arena a knight has to stand in for him to wake: the summit's own columns.
def arena(+x: U32, +y: U32) -> Bool:
  Bool.and(
    Bool.and(
      N.gt(x, N.mul(N.add(summit_ox(), 3), tile())),
      N.lt(x, N.mul(N.add(summit_ox(), 38), tile()))
    ),
    N.gt(y, N.mul(summit_oy(), tile()))
  )

# ------------------------------------------------------------------ the lists

def rev_hazard(xs: List<Ans.Hazard>, acc: List<Ans.Hazard>) -> List<Ans.Hazard>:
  match xs:
    case Nil{}:
      acc
    case Con{h, t}:
      rev_hazard(t, Con{h, acc})

# The keep-or-drop of a filter, as a match, so the list it keeps is used once.
def kept(+keep: Bool, +h: Ans.Hazard, acc: List<Ans.Hazard>) -> List<Ans.Hazard>:
  match keep:
    case True{}:
      Con{h, acc}
    case False{}:
      acc

# One hazard on the end of a list, keeping the order they were raised in.
def push(+h: Ans.Hazard, xs: List<Ans.Hazard>) -> List<Ans.Hazard>:
  match xs:
    case Nil{}:
      Con{h, Nil{}}
    case Con{hd, tl}:
      Con{hd, push(h, tl)}

def push_all(hs: List<Ans.Hazard>, xs: List<Ans.Hazard>) -> List<Ans.Hazard>:
  match hs:
    case Nil{}:
      xs
    case Con{h, t}:
      push_all(t, push(h, xs))

# ---------------------------------------------------------------- the hazards

# One tick of a hazard: its warning counts down, then it moves and its life does.
def hazard_one(+h: Ans.Hazard) -> Ans.Hazard:
  match h:
    case Ans.Hazard{+x, +y, +w, +ht, +vx, +tell, +live}:
      Bool.pick(
        Ans.Hazard,
        N.gt(tell, 0),
        Ans.Hazard{x, y, w, ht, vx, N.sub(tell, 1), live},
        Ans.Hazard{N.add(x, vx), y, w, ht, vx, tell, N.sat_sub(live, 1)}
      )

def hazard_tell(+h: Ans.Hazard) -> U32:
  match h:
    case Ans.Hazard{+x, +y, +w, +ht, +vx, +tell, +live}:
      tell

def hazard_live(+h: Ans.Hazard) -> U32:
  match h:
    case Ans.Hazard{+x, +y, +w, +ht, +vx, +tell, +live}:
      live

# Whether a hazard is still doing something: warning, or hurting.
def hazard_keep(+h: Ans.Hazard) -> Bool:
  Bool.or(N.gt(hazard_live(h), 0), N.gt(hazard_tell(h), 0))

def hazard_tick(hs: List<Ans.Hazard>, acc: List<Ans.Hazard>) -> List<Ans.Hazard>:
  match hs:
    case Nil{}:
      rev_hazard(acc, Nil{})
    case Con{h, t}:
      +one = hazard_one(h)
      hazard_tick(t, kept(hazard_keep(one), one, acc))

# ---------------------------------------------------------------- the feet

# How many knights stand in the arena.
def arena_count(+at: Ans.Feet) -> U32:
  match at:
    case Ans.Feet{+n, +ax, +ay, +bx, +by, +cx, +cy, +dx, +dy, +ex, +ey, +fx, +fy}:
      +c0 = Bool.pick(U32, Bool.and(N.gt(n, 0), arena(ax, ay)), 1, 0)
      +c1 = Bool.pick(U32, Bool.and(N.gt(n, 1), arena(bx, by)), N.add(c0, 1), c0)
      +c2 = Bool.pick(U32, Bool.and(N.gt(n, 2), arena(cx, cy)), N.add(c1, 1), c1)
      +c3 = Bool.pick(U32, Bool.and(N.gt(n, 3), arena(dx, dy)), N.add(c2, 1), c2)
      +c4 = Bool.pick(U32, Bool.and(N.gt(n, 4), arena(ex, ey)), N.add(c3, 1), c3)
      Bool.pick(U32, Bool.and(N.gt(n, 5), arena(fx, fy)), N.add(c4, 1), c4)

# The knight nearest `x`, the first one in seat order when two are as near.
def closer(+best: Ans.Pick, +use: Bool, +v: U32, +x: U32) -> Ans.Pick:
  match best:
    case Ans.Pick{+any, +pick}:
      Bool.pick(
        Ans.Pick,
        use,
        Bool.pick(
          Ans.Pick,
          any,
          Bool.pick(
            Ans.Pick,
            N.lt(N.abs(N.sub(v, x)), N.abs(N.sub(pick, x))),
            {Ans.Pick{True{}, v} : Ans.Pick},
            {Ans.Pick{True{}, pick} : Ans.Pick}
          ),
          {Ans.Pick{True{}, v} : Ans.Pick}
        ),
        {Ans.Pick{any, pick} : Ans.Pick}
      )

def nearest(+at: Ans.Feet, +x: U32) -> Ans.Pick:
  match at:
    case Ans.Feet{+n, +ax, +ay, +bx, +by, +cx, +cy, +dx, +dy, +ex, +ey, +fx, +fy}:
      +b0 = {Ans.Pick{False{}, 0} : Ans.Pick}
      +b1 = closer(b0, Bool.and(N.gt(n, 0), arena(ax, ay)), ax, x)
      +b2 = closer(b1, Bool.and(N.gt(n, 1), arena(bx, by)), bx, x)
      +b3 = closer(b2, Bool.and(N.gt(n, 2), arena(cx, cy)), cx, x)
      +b4 = closer(b3, Bool.and(N.gt(n, 3), arena(dx, dy)), dx, x)
      +b5 = closer(b4, Bool.and(N.gt(n, 4), arena(ex, ey)), ex, x)
      closer(b5, Bool.and(N.gt(n, 5), arena(fx, fy)), fx, x)

# How many knights stand left of `v`, and left of or on it: the multiset's
# `k`-th value without sorting one.
def count_lt(+at: Ans.Feet, +v: U32) -> U32:
  match at:
    case Ans.Feet{+n, +ax, +ay, +bx, +by, +cx, +cy, +dx, +dy, +ex, +ey, +fx, +fy}:
      +c0 = Bool.pick(U32, Bool.and(Bool.and(N.gt(n, 0), arena(ax, ay)), N.lt(ax, v)), 1, 0)
      +c1 = Bool.pick(U32, Bool.and(Bool.and(N.gt(n, 1), arena(bx, by)), N.lt(bx, v)), N.add(c0, 1), c0)
      +c2 = Bool.pick(U32, Bool.and(Bool.and(N.gt(n, 2), arena(cx, cy)), N.lt(cx, v)), N.add(c1, 1), c1)
      +c3 = Bool.pick(U32, Bool.and(Bool.and(N.gt(n, 3), arena(dx, dy)), N.lt(dx, v)), N.add(c2, 1), c2)
      +c4 = Bool.pick(U32, Bool.and(Bool.and(N.gt(n, 4), arena(ex, ey)), N.lt(ex, v)), N.add(c3, 1), c3)
      Bool.pick(U32, Bool.and(Bool.and(N.gt(n, 5), arena(fx, fy)), N.lt(fx, v)), N.add(c4, 1), c4)

def count_le(+at: Ans.Feet, +v: U32) -> U32:
  match at:
    case Ans.Feet{+n, +ax, +ay, +bx, +by, +cx, +cy, +dx, +dy, +ex, +ey, +fx, +fy}:
      +c0 = Bool.pick(U32, Bool.and(Bool.and(N.gt(n, 0), arena(ax, ay)), N.le(ax, v)), 1, 0)
      +c1 = Bool.pick(U32, Bool.and(Bool.and(N.gt(n, 1), arena(bx, by)), N.le(bx, v)), N.add(c0, 1), c0)
      +c2 = Bool.pick(U32, Bool.and(Bool.and(N.gt(n, 2), arena(cx, cy)), N.le(cx, v)), N.add(c1, 1), c1)
      +c3 = Bool.pick(U32, Bool.and(Bool.and(N.gt(n, 3), arena(dx, dy)), N.le(dx, v)), N.add(c2, 1), c2)
      +c4 = Bool.pick(U32, Bool.and(Bool.and(N.gt(n, 4), arena(ex, ey)), N.le(ex, v)), N.add(c3, 1), c3)
      Bool.pick(U32, Bool.and(Bool.and(N.gt(n, 5), arena(fx, fy)), N.le(fx, v)), N.add(c4, 1), c4)

def holds(+at: Ans.Feet, +k: U32, +v: U32) -> Bool:
  Bool.and(N.le(count_lt(at, v), k), N.lt(k, count_le(at, v)))

# The value `k` places into the sorted arena x's.
def kth(+at: Ans.Feet, +k: U32) -> U32:
  match at:
    case Ans.Feet{+n, +ax, +ay, +bx, +by, +cx, +cy, +dx, +dy, +ex, +ey, +fx, +fy}:
      Bool.pick(
        U32,
        holds(at, k, ax),
        ax,
        Bool.pick(
          U32,
          holds(at, k, bx),
          bx,
          Bool.pick(
            U32,
            holds(at, k, cx),
            cx,
            Bool.pick(
              U32,
              holds(at, k, dx),
              dx,
              Bool.pick(
                U32,
                holds(at, k, ex),
                ex,
                Bool.pick(U32, holds(at, k, fx), fx, home())
              )
            )
          )
        )
      )

# ------------------------------------------------------------------ the pattern

# The attacks of a crown, before he kneels: two, three, then four of them.
def pattern_len(+phase: U32) -> U32:
  Bool.pick(U32, U32.is_zero(phase), 2, Bool.pick(U32, U32.is_eq(phase, 1), 3, 4))

def pattern_at(+phase: U32, +at: U32) -> U32:
  Bool.pick(
    U32,
    U32.is_zero(phase),
    Bool.pick(U32, U32.is_zero(at), 0, 2),
    Bool.pick(
      U32,
      U32.is_eq(phase, 1),
      Bool.pick(U32, U32.is_zero(at), 1, Bool.pick(U32, U32.is_eq(at, 1), 0, 2)),
      Bool.pick(
        U32,
        U32.is_zero(at),
        2,
        Bool.pick(U32, U32.is_eq(at, 1), 1, Bool.pick(U32, U32.is_eq(at, 2), 0, 2))
      )
    )
  )

def attack_time(+kind: U32) -> U32:
  Bool.pick(U32, U32.is_zero(kind), 40, Bool.pick(U32, U32.is_eq(kind, 1), 32, 30))

def strike_time(+kind: U32) -> U32:
  Bool.pick(U32, U32.is_zero(kind), 22, Bool.pick(U32, U32.is_eq(kind, 1), 14, 12))

# ---------------------------------------------------------------- the warning

# An arm sweeps the floor in front of him, the way the nearest knight stands.
def sweep_hazard(+x: U32, +toward: Ans.Pick, +tell: U32) -> Ans.Hazard:
  match toward:
    case Ans.Pick{+any, +tx}:
      Bool.pick(
        Ans.Hazard,
        Bool.and(any, N.gt(tx, x)),
        Ans.Hazard{N.add(x, 6), N.sub(floor(), 22), reach(), 22, 0, tell, 14},
        Ans.Hazard{N.sub(N.sub(x, 6), reach()), N.sub(floor(), 22), reach(), 22, 0, tell, 14}
      )

def embers_loop(fuel: Nat, +c: U32, +at: Ans.Feet, +elapsed: U32, +tell: U32, acc: List<Ans.Hazard>) -> List<Ans.Hazard>:
  match fuel:
    case 0n:
      rev_hazard(acc, Nil{})
    case 1n+p:
      +count = arena_count(at)
      +pick = Bool.pick(
        U32,
        U32.is_zero(count),
        home(),
        kth(at, U32.mod(N.add(elapsed, N.mul(c, 3)), count))
      )
      +px = N.clamp(
        N.add(pick, N.mul(N.sub(U32.mod(c, 3), 1), 26)),
        N.mul(N.add(summit_ox(), 4), tile()),
        N.mul(N.add(summit_ox(), 36), tile())
      )
      +top = N.mul(N.add(summit_oy(), 1), tile())
      embers_loop(
        p,
        N.add(c, 1),
        at,
        elapsed,
        tell,
        Con{Ans.Hazard{N.sub(px, 10), top, 20, N.sub(floor(), top), 0, tell, 12}, acc}
      )

# Columns of embers fall where the knights stand: more of them for more seats
# and later crowns, each a stride apart.
def embers(+at: Ans.Feet, +seats: U32, +elapsed: U32, +tell: U32, +phase: U32) -> List<Ans.Hazard>:
  +columns = N.add(N.add(1, phase), N.div(N.sat_sub(seats, 1), 2))
  embers_loop(U32.to_nat(columns), 0, at, elapsed, tell, Nil{})

def warn(+st: Ans.State, +at: Ans.Feet, +seats: U32, +elapsed: U32) -> List<Ans.Hazard>:
  match st:
    case Ans.State{+x, +b, +kind, +t, +phase, +plates, +count, +head, +head_before, +x_before, +flinch}:
      Bool.pick(
        List<Ans.Hazard>,
        U32.is_zero(kind),
        Nil{},
        Bool.pick(
          List<Ans.Hazard>,
          U32.is_eq(kind, 1),
          Con{sweep_hazard(x, nearest(at, x), t), Nil{}},
          embers(at, seats, elapsed, t, phase)
        )
      )

# ---------------------------------------------------------------- the blow

def wave(+x: U32, +dir: U32, +k: U32) -> Ans.Hazard:
  Ans.Hazard{
    N.add(N.sub(x, 8), N.mul(N.mul(dir, k), 40)),
    N.sub(floor(), 14),
    16,
    14,
    N.mul(dir, 3),
    N.mul(k, 14),
    70
  }

# The blow lands: a stomp sends waves along the floor, each following the last.
def strike_hazards(+st: Ans.State) -> List<Ans.Hazard>:
  match st:
    case Ans.State{+x, +b, +kind, +t, +phase, +plates, +count, +head, +head_before, +x_before, +flinch}:
      Bool.pick(
        List<Ans.Hazard>,
        Bool.not(U32.is_zero(kind)),
        Nil{},
        Bool.pick(
          List<Ans.Hazard>,
          U32.is_zero(phase),
          Con{wave(x, N.neg(1), 0), Con{wave(x, 1, 0), Nil{}}},
          Con{
            wave(x, N.neg(1), 0),
            Con{wave(x, 1, 0), Con{wave(x, N.neg(1), 1), Con{wave(x, 1, 1), Nil{}}}}
          }
        )
      )

# ---------------------------------------------------------------- the beats

# Asleep until a knight walks in.
def dormant(+st: Ans.State, +at: Ans.Feet) -> Ans.Branch:
  match st:
    case Ans.State{+x, +b, +kind, +t, +phase, +plates, +count, +head, +head_before, +x_before, +flinch}:
      Bool.pick(
        Ans.Branch,
        N.gt(arena_count(at), 0),
        Ans.Branch{Ans.State{x, 1, kind, 60, phase, plates, count, head, head, x, flinch}, Nil{}},
        Ans.Branch{Ans.State{x, b, kind, t, phase, plates, count, head, head, x, flinch}, Nil{}}
      )

# Between crowns: he sinks a little.
def down(+st: Ans.State) -> Ans.Branch:
  match st:
    case Ans.State{+x, +b, +kind, +t, +phase, +plates, +count, +head, +head_before, +x_before, +flinch}:
      Ans.Branch{
        Ans.State{
          x,
          b,
          kind,
          t,
          phase,
          plates,
          count,
          N.max(N.sub(head, 1), N.sub(head_down(), 1)),
          head,
          x,
          flinch
        },
        Nil{}
      }

# The wait after a crown falls: the crown is mended for the seats at the table.
def raging(+st: Ans.State, +seats: U32) -> Ans.Branch:
  match st:
    case Ans.State{+x, +b, +kind, +t, +phase, +plates, +count, +head, +head_before, +x_before, +flinch}:
      +left = N.sat_sub(t, 1)
      +up = N.min(N.add(head, 1), head_up())
      Bool.pick(
        Ans.Branch,
        U32.is_zero(left),
        Ans.Branch{
          Ans.State{x, 1, kind, 40, phase, N.max(seats, 1), 0, up, head, x, flinch},
          Nil{}
        },
        Ans.Branch{Ans.State{x, b, kind, left, phase, plates, count, up, head, x, flinch}, Nil{}}
      )

def step_to(+near: Ans.Pick, +x: U32, +elapsed: U32) -> U32:
  match near:
    case Ans.Pick{+any, +tx}:
      Bool.pick(
        U32,
        any,
        Bool.pick(
          U32,
          U32.is_zero(U32.mod(elapsed, 2)),
          N.clamp(
            N.add(x, N.clamp(N.sub(tx, x), N.neg(1), 1)),
            N.sub(home(), 110),
            N.add(home(), 110)
          ),
          N.clamp(x, N.sub(home(), 110), N.add(home(), 110))
        ),
        x
      )

def rest_tell(+moved: U32, +x0: U32, +up: U32, +phase: U32, +count: U32, +plates: U32, +head: U32, +flinch: U32, +at: Ans.Feet, +elapsed: U32, +seats: U32, +kind: U32) -> Ans.Branch:
  +st = {
    Ans.State{
      moved,
      2,
      kind,
      N.sub(attack_time(kind), N.mul(3, phase)),
      phase,
      plates,
      count,
      up,
      head,
      x0,
      flinch
    } : Ans.State
  }
  Ans.Branch{st, warn(st, at, seats, elapsed)}

# The moment of choice: kneel, or tell the next attack.
def rest_act(+moved: U32, +x0: U32, +up: U32, +kind: U32, +phase: U32, +count: U32, +plates: U32, +head: U32, +flinch: U32, +at: Ans.Feet, +elapsed: U32, +seats: U32) -> Ans.Branch:
  Bool.pick(
    Ans.Branch,
    U32.is_ge(count, pattern_len(phase)),
    Ans.Branch{
      Ans.State{
        moved,
        4,
        kind,
        N.max(N.sub(N.sub(150, N.mul(20, phase)), N.mul(3, N.sat_sub(seats, 1))), 70),
        phase,
        plates,
        count,
        up,
        head,
        x0,
        flinch
      },
      Nil{}
    },
    rest_tell(moved, x0, up, phase, count, plates, head, flinch, at, elapsed, seats, pattern_at(phase, count))
  )

# Standing: he steps toward the nearest knight, slowly, and then attacks.
def rest(+st: Ans.State, +at: Ans.Feet, +elapsed: U32, +seats: U32) -> Ans.Branch:
  match st:
    case Ans.State{+x, +b, +kind, +t, +phase, +plates, +count, +head, +head_before, +x_before, +flinch}:
      +up = N.min(N.add(head, 1), head_up())
      +moved = step_to(nearest(at, x), x, elapsed)
      +left = N.sat_sub(t, 1)
      Bool.pick(
        Ans.Branch,
        U32.is_zero(left),
        rest_act(moved, x, up, kind, phase, count, plates, head, flinch, at, elapsed, seats),
        Ans.Branch{Ans.State{moved, b, kind, left, phase, plates, count, up, head, x, flinch}, Nil{}}
      )

# The warning is out; when it runs out the blow lands.
def tell(+st: Ans.State) -> Ans.Branch:
  match st:
    case Ans.State{+x, +b, +kind, +t, +phase, +plates, +count, +head, +head_before, +x_before, +flinch}:
      +left = N.sat_sub(t, 1)
      Bool.pick(
        Ans.Branch,
        U32.is_zero(left),
        Ans.Branch{
          Ans.State{x, 3, kind, strike_time(kind), phase, plates, count, head, head, x, flinch},
          strike_hazards(st)
        },
        Ans.Branch{Ans.State{x, b, kind, left, phase, plates, count, head, head, x, flinch}, Nil{}}
      )

def striking(+st: Ans.State, +seats: U32) -> Ans.Branch:
  match st:
    case Ans.State{+x, +b, +kind, +t, +phase, +plates, +count, +head, +head_before, +x_before, +flinch}:
      +left = N.sat_sub(t, 1)
      +crowd = N.sub(N.max(seats, 1), 1)
      Bool.pick(
        Ans.Branch,
        U32.is_zero(left),
        Ans.Branch{
          Ans.State{
            x,
            1,
            kind,
            N.max(N.sub(N.sub(44, N.mul(6, phase)), N.mul(3, crowd)), 16),
            phase,
            plates,
            N.add(count, 1),
            head,
            head,
            x,
            flinch
          },
          Nil{}
        },
        Ans.Branch{Ans.State{x, b, kind, left, phase, plates, count, head, head, x, flinch}, Nil{}}
      )

# Kneeling: the crown is low, and a slam can take a plate off it.
def kneeling(+st: Ans.State) -> Ans.Branch:
  match st:
    case Ans.State{+x, +b, +kind, +t, +phase, +plates, +count, +head, +head_before, +x_before, +flinch}:
      +left = N.sat_sub(t, 1)
      +low = N.max(N.sub(head, 2), head_down())
      Bool.pick(
        Ans.Branch,
        U32.is_zero(left),
        Ans.Branch{Ans.State{x, 1, kind, 30, phase, plates, 0, low, head, x, flinch}, Nil{}},
        Ans.Branch{Ans.State{x, b, kind, left, phase, plates, count, low, head, x, flinch}, Nil{}}
      )

# ---------------------------------------------------------------- the tick

# The beat he is on, one tick on, and any hazards that tick raised.
def beat(+st: Ans.State, +elapsed: U32, +seats: U32, +at: Ans.Feet) -> Ans.Branch:
  match st:
    case Ans.State{+x, +b, +kind, +t, +phase, +plates, +count, +head, +head_before, +x_before, +flinch}:
      # The flinch counts down before the beat is read, as it always has.
      +calm = {
        Ans.State{x, b, kind, t, phase, plates, count, head, head_before, x_before, N.sat_sub(flinch, 1)} : Ans.State
      }
      Bool.pick(
        Ans.Branch,
        U32.is_zero(b),
        dormant(calm, at),
        Bool.pick(
          Ans.Branch,
          U32.is_eq(b, 1),
          rest(calm, at, elapsed, seats),
          Bool.pick(
            Ans.Branch,
            U32.is_eq(b, 2),
            tell(calm),
            Bool.pick(
              Ans.Branch,
              U32.is_eq(b, 3),
              striking(calm, seats),
              Bool.pick(
                Ans.Branch,
                U32.is_eq(b, 4),
                kneeling(calm),
                Bool.pick(Ans.Branch, U32.is_eq(b, 5), raging(calm, seats), down(calm))
              )
            )
          )
        )
      )

def assemble(b: Ans.Branch, hazards: List<Ans.Hazard>) -> Ans.BossOut:
  match b:
    case Ans.Branch{+next, extra}:
      Ans.BossOut{next, push_all(extra, hazard_tick(hazards, Nil{}))}

def step(+elapsed: U32, +seats: U32, +at: Ans.Feet, +st: Ans.State, hazards: List<Ans.Hazard>) -> Ans.BossOut:
  assemble(beat(st, elapsed, seats, at), hazards)

# ---------------------------------------------------------------- the slam

def slam_hit(+go: Bool, +st: Ans.State, hazards: List<Ans.Hazard>) -> Ans.SlamOut:
  match go:
    case False{}:
      Ans.SlamOut{st, hazards, False{}}
    case True{}:
      match st:
        case Ans.State{+sx, +b, +kind, +t, +phase, +plates, +count, +head, +head_before, +x_before, +flinch}:
          +plates2 = N.sub(plates, 1)
          Bool.pick(
            Ans.SlamOut,
            U32.is_zero(plates2),
            Bool.pick(
              Ans.SlamOut,
              U32.is_ge(N.add(phase, 1), phases()),
              Ans.SlamOut{
                Ans.State{sx, 6, kind, 0, N.add(phase, 1), 0, count, head, head_before, x_before, 14},
                Nil{},
                True{}
              },
              Ans.SlamOut{
                Ans.State{sx, 5, kind, 54, N.add(phase, 1), 0, count, head, head_before, x_before, 14},
                Nil{},
                True{}
              }
            ),
            Ans.SlamOut{
              Ans.State{sx, b, kind, t, phase, plates2, count, head, head_before, x_before, 14},
              hazards,
              True{}
            }
          )

# A slam landed with its feet at `(x, y)` pixels: whether a plate fell, and the
# state after it. A crown that loses its last plate falls: the next crown, or the
# last of them, and its hazards go with it.
def slam(+st: Ans.State, +x: U32, +y: U32, hazards: List<Ans.Hazard>) -> Ans.SlamOut:
  match st:
    case Ans.State{+sx, +b, +kind, +t, +phase, +plates, +count, +head, +head_before, +x_before, +flinch}:
      slam_hit(
        Bool.and(
          Bool.and(U32.is_eq(b, 4), Bool.and(N.gt(plates, 0), N.le(N.abs(N.sub(x, sx)), 24))),
          N.le(N.abs(N.sub(y, N.sub(floor(), head))), 3)
        ),
        st,
        hazards
      )
