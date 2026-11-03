# Lanternward's five rooms: the floor plans the adventure is laid out on, and
# what a body may stand on. Letters: `#` wall, `.` floor, `=` pit, `V` bars
# (bolts pass), `@` a seat's start, `C` a resting brazier, `Q` the way on; the
# rest are things that stand on the floor (`props::Pk::of`).
#
# A plan is a list of rows of text. Asking for a tile past a row's end, or past
# the last row, answers the wall, which is the same answer the Rust ground
# gives, so no bounds check has to branch.
import Base
import ./n.bend as N
import ./g.bend as G

type Plan is Type:
  Plan{name: String, rows: List<String>, dark: Bool}

def stair() -> List<String>:
  [
    "######################",
    "#..............==..\"\"#",
    "#.@.@.@.@.@.@..==..\"\"#",
    "#.C..F.........==....#",
    "#..x....\"\"\"\"...==.o..#",
    "#.......\"\"\"\"...==....#",
    "#..s...........==....#",
    "#.....S........==..c.#",
    "#..\"\"\".........==....#",
    "#..\"\"\".....o...==....#",
    "#..............==....#",
    "#.....o.....S..==..o.#",
    "#..o.........o.==....#",
    "#..............==....#",
    "#..............==....#",
    "#########GG###########",
    "#########QQ###########",
    "######################"
  ]

def lamp() -> List<String>:
  [
    "##############################",
    "#..........#.c.#......########",
    "#.oo.......#111#......#VbV####",
    "#.......S.............##V#####",
    "#....B.....................###",
    "#.......o..................###",
    "#....................B.....###",
    "#..........................###",
    "#.E.....\"\"..............S..GQ#",
    "#.......\"\"........\"\"\"......GQ#",
    "#.................\"\"\"......###",
    "#...s.S....................###",
    "#.............B....S.......###",
    "#.C..............o.......oN2c#",
    "#.@.@.@.@.@.@..............###",
    "##############################"
  ]

def gallery() -> List<String>:
  [
    "######################################",
    "#..o.............#.................###",
    "#......s......o..#.................GQ#",
    "#.@.@.........Y..#...#.............GQ#",
    "#................#...#.............###",
    "#................#................VK##",
    "#.@.@....X....P..1.................###",
    "#................1.................###",
    "#................1...............T.###",
    "#.@.@..\"\"\"\"......#.....#.....#.....###",
    "#......\"\"\"\"......#.....#.....#.....###",
    "#.C....\"\"\"\".o..o.#..o.....#......T.###",
    "#................#.................###",
    "######################~###############",
    "#####################.c.##############",
    "######################################"
  ]

def vault() -> List<String>:
  [
    "################################",
    "#..........##.c..##..........###",
    "#..........##....##..........###",
    "#..........###LL###..........###",
    "#..o.....S..........S........###",
    "#............................###",
    "#............................###",
    "#............................###",
    "#..s.B..................B....MQ#",
    "#.............c..............MQ#",
    "#./..........................###",
    "#......S.................S...###",
    "#............................2c#",
    "#.............S.......S......###",
    "#.C.......................o..###",
    "#.@.@.@.@.@.@....o...........###",
    "#............................###",
    "################################"
  ]

def warden() -> List<String>:
  [
    "##############################",
    "#............................#",
    "#.o............*...........o.#",
    "#............................#",
    "#............................#",
    "#...B..........W.........B...#",
    "#............................#",
    "#............................#",
    "#............................#",
    "#.......f............f.......#",
    "#............................#",
    "#..........#......#..........#",
    "#............................#",
    "#............................#",
    "#.......f............f.......#",
    "#............................#",
    "#...B....................B...#",
    "#..........#......#..........#",
    "#............................#",
    "#............................#",
    "#............................#",
    "#.o........@.@.@.@.@.@.....o.#",
    "#..............C.............#",
    "##############################"
  ]

# The plan of a room, clamped the way `plan(room.min(ROOMS - 1))` is. The five
# plans are built and one is picked, because Bend only matches parameters.
def plan(+room: U32) -> Plan:
  +i = N.smin(room, 4)
  Bool.pick(
    Plan,
    U32.is_eq(i, 0),
    Plan{"THE STAIR", stair(), False{}},
    Bool.pick(
      Plan,
      U32.is_eq(i, 1),
      Plan{"THE LAMP HALL", lamp(), False{}},
      Bool.pick(
        Plan,
        U32.is_eq(i, 2),
        Plan{"THE ARROW GALLERY", gallery(), False{}},
        Bool.pick(Plan, U32.is_eq(i, 3), Plan{"THE VAULT", vault(), True{}}, Plan{"THE WARDEN", warden(), False{}})
      )
    )
  )

# ----- the marks: where a seat starts -------------------------------------
#
# A list is a `Type`, so a hit cannot be filtered out with a choice: every tile
# is written down, the tiles that do not hold the letter carrying the sentinel,
# and counting and picking skip the sentinel with arithmetic.

def no_tile() -> U32:
  4294967295

def pack(x: U32, y: U32) -> U32:
  U32.or(U32.shln(x, 16n), U32.and(y, 65535))

def rev(xs: List<U32>, acc: List<U32>) -> List<U32>:
  match xs:
    case Nil{}:
      acc
    case Con{h, t}:
      rev(t, Con{h, acc})

# How many tiles in the list actually hold the letter.
def hits(xs: List<U32>) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{h, t}:
      U32.add(N.flag(U32.is_ne(h, no_tile())), hits(t))

# Every tile of a row, numbered left to right; the tiles that do not hold
# `code` carry the sentinel.
def scan(s: String, +x: U32, +y: U32, +code: U32, acc: List<U32>) -> List<U32>:
  match s:
    case SNil{}:
      acc
    case SCon{head, tail}:
      +here = N.cond(N.mask(N.sneg(N.flag(U32.is_eq(Char.to_u32(head), code)))), pack(x, y), no_tile())
      scan(tail, U32.inc(x), y, code, Con{here, acc})

def find_rows(rows: List<String>, +y: U32, +code: U32, acc: List<U32>) -> List<U32>:
  match rows:
    case Nil{}:
      acc
    case Con{h, t}:
      find_rows(t, U32.inc(y), code, scan(h, 0, y, code, acc))

# Every tile with this letter, row by row, as `Plan::find` orders them.
def find(rows: List<String>, +code: U32) -> List<U32>:
  rev(find_rows(rows, 0, code, Nil{}), Nil{})

def rows_of(p: Plan) -> List<String>:
  match p:
    case Plan{_, rows, _}:
      rows

def marks_of(p: Plan) -> List<U32>:
  find(rows_of(p), 64)

def marks(room: U32) -> List<U32>:
  marks_of(plan(room))

# The n-th tile that holds the letter, 0 when there is none that far in.
def nth_hit(xs: List<U32>, +n: U32) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{+h, t}:
      +live = N.flag(U32.is_ne(h, no_tile()))
      +here = N.flag(Bool.and(U32.is_ne(h, no_tile()), U32.is_eq(n, 0)))
      N.cond(N.mask(N.sneg(here)), h, nth_hit(t, U32.sub(n, live)))

# Where seat `id` starts in a room: its own mark, wrapping, the way
# `start_of` reads it.
def start_tile(+room: U32, id: U32) -> U32:
  # The marks are built twice rather than held: a list is a `Type`, and two
  # uses of one list is exactly what affineness forbids.
  nth_hit(marks(room), U32.mod(U32.sub(id, 1), U32.max(hits(marks(room)), 1)))

def start_x(room: U32, id: U32) -> U32:
  G.middle(U32.shrn(start_tile(room, id), 16n))

def start_y(room: U32, id: U32) -> U32:
  G.middle(U32.and(start_tile(room, id), 65535))
