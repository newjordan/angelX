import Base
import ./map.bend as Map
import ./nums.bend as N

# The world's moving platforms: two logs on the marsh's wide gap, one in the
# mill-race, and the waterwheel's six buckets. Pure in the tick, so a body can
# be carried by the difference between two ticks — which is why this leg is
# exact integer arithmetic and no state at all.
#
# Ids are layout::LOG_ID (100, 101, 102) and layout::WHEEL_ID (200 to 205), and
# they travel as numbers: a body records which mover it stands on by id, never
# by text.

type Mv is Data:
  Mv{id: U32, x: U32, y: U32, w: U32}

def tile() -> U32:
  16

# The waterwheel in the mill: centre in pixels, radius, buckets, ticks a turn.
def wheel_x() -> U32:
  3496

def wheel_y() -> U32:
  760

def wheel_radius() -> U32:
  40

def wheel_turn() -> U32:
  270

def wheel_id() -> U32:
  200

def log_id() -> U32:
  100

# A sine in 1/1024 for whole degrees: layout::sine's table and its four
# quarters, which is the rounding the buckets have always climbed by.
def table() -> List<U32>:
  [0, 18, 36, 54, 71, 89, 107, 125, 143, 160, 178, 195, 213, 230, 248, 265, 282, 299, 316,
   333, 350, 367, 384, 400, 416, 433, 449, 465, 481, 496, 512, 527, 543, 558, 573, 587, 602,
   616, 630, 644, 658, 672, 685, 698, 711, 724, 737, 749, 761, 773, 784, 796, 807, 818, 828,
   839, 849, 859, 868, 878, 887, 896, 904, 912, 920, 928, 935, 943, 949, 956, 962, 968, 974,
   979, 984, 989, 994, 998, 1002, 1005, 1008, 1011, 1014, 1016, 1018, 1020, 1022, 1023, 1023,
   1024, 1024]

def nth(xs: List<U32>, n: Nat) -> U32:
  match xs n:
    case Nil{} _:
      0
    case Con{h, t} 0n:
      h
    case Con{h, t} 1n+p:
      nth(t, p)

def sine(+deg: U32) -> U32:
  +d = N.rem_euclid(deg, 360)
  Bool.pick(
    U32,
    U32.is_le(d, 90),
    nth(table(), U32.to_nat(d)),
    Bool.pick(
      U32,
      U32.is_le(d, 180),
      nth(table(), U32.to_nat((180 - d : U32))),
      Bool.pick(
        U32,
        U32.is_le(d, 270),
        N.neg(nth(table(), U32.to_nat((d - 180 : U32)))),
        N.neg(nth(table(), U32.to_nat((360 - d : U32))))
      )
    )
  )

def cosine(+deg: U32) -> U32:
  sine(N.add(deg, 90))

# A value going back and forth between `lo` and `hi`, `period` ticks a round
# trip, `phase` ticks along: layout::ping_pong, in the game's fixed units.
def ping_pong_at(+k: U32, +period: U32, +half: U32, +lo: U32, +hi: U32) -> U32:
  Bool.pick(
    U32,
    U32.is_lt(k, half),
    N.add(lo, N.div(N.mul(N.sub(hi, lo), k), half)),
    N.add(lo, N.div(N.mul(N.sub(hi, lo), (period - k : U32)), half))
  )

def ping_pong(+t: U32, +period: U32, +phase: U32, +lo: U32, +hi: U32) -> U32:
  +half = U32.div(period, 2)
  ping_pong_at(U32.mod(N.add(t, phase), period), period, half, lo, hi)

def to_u32(n: Nat) -> U32:
  match n:
    case 0n:
      0
    case 1n+p:
      (1 + to_u32(p) : U32)

def rim(n: Nat, +deg: U32, +phi: U32) -> Mv:
  +turn = N.add(deg, phi)
  Mv{
    N.add(wheel_id(), to_u32(n)),
    N.sub(N.add(wheel_x(), N.div(N.mul(wheel_radius(), cosine(turn)), 1024)), 14),
    N.add(wheel_y(), N.div(N.mul(wheel_radius(), sine(turn)), 1024)),
    28
  }

# The wheel turns clockwise, so the near side climbs: bucket `k` of `n`, each a
# platform 28 wide on the rim, sixty degrees apart.
def bucket(n: Nat, k: Nat, +deg: U32) -> Mv:
  rim(n, deg, U32.mul(to_u32(k), 60))

def buckets(n: Nat, +k: Nat, +deg: U32) -> List<Mv>:
  match n:
    case 0n:
      Nil{}
    case 1n+p:
      Con{bucket(k, k, deg), buckets(p, Nat.add(k, 1n), deg)}

# Every moving platform at tick `t`, in layout::movers' order: the three logs,
# then the six buckets.
def movers(+t: U32) -> List<Mv>:
  +bank = ((36 + 12) * 16 : U32)
  +gap = ((80 + 62) * 16 : U32)
  +water = ((36 + 14) * 16 : U32)
  +race = ((185 + 19) * 16 : U32)
  +deg = U32.div(U32.mul(U32.mod(t, wheel_turn()), 360), wheel_turn())
  Con{
    Mv{log_id(), ping_pong(t, 150, 0, (gap + 4 : U32), (gap + 52 : U32)), bank, 48},
    Con{
      Mv{N.add(log_id(), 1), ping_pong(t, 150, 75, (gap + 68 : U32), (gap + 116 : U32)), bank, 48},
      Con{
        Mv{N.add(log_id(), 2), ping_pong(t, 120, 0, race, (race + 32 : U32)), water, 48},
        buckets(6n, 0n, deg)
      }
    }
  }

# How many platforms a tick carries, and where the first one is: the two shapes
# a law can hold a whole list to without comparing lists.
def count(ms: List<Mv>, +acc: U32) -> U32:
  match ms:
    case Nil{}:
      acc
    case Con{_, t}:
      count(t, (acc + 1 : U32))

def first_x(ms: List<Mv>) -> U32:
  match ms:
    case Nil{}:
      0
    case Con{h, _}:
      match h:
        case Mv{_, +x, _, _}:
          x
