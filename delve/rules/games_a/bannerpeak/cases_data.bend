# Cases of the baked data: the whole composed world and the lists layout::info() reads.

def case.bannerpeak.layout.world(+input: J.Val) -> J.Val:
  J.JA{bannerpeak.cases.world.rows(0)}

def bannerpeak.cases.world.rows(+y: U32) -> List<&2, J.Val>:
  bannerpeak.u.ife(List<&2, J.Val>, U32.is_lt(y, 64), _ => Con{J.JA{bannerpeak.cases.world.cols(y, 0)}, bannerpeak.cases.world.rows((y + 1 : U32))}, _ => Nil{})

def bannerpeak.cases.world.cols(+y: U32, +x: U32) -> List<&2, J.Val>:
  bannerpeak.u.ife(List<&2, J.Val>, U32.is_lt(x, 434), _ => Con{J.JU{bannerpeak.world.World.at(x, y)}, bannerpeak.cases.world.cols(y, (x + 1 : U32))}, _ => Nil{})

# Flat (x, y) pairs as an array of [x, y].
def bannerpeak.cases.pairs(+xs: List<&2, U32>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+x, +r}:
      bannerpeak.cases.pairs.y(x, r)

def bannerpeak.cases.pairs.y(+x: U32, +r: List<&2, U32>) -> List<&2, J.Val>:
  match r:
    case Nil{}:
      Nil{}
    case Con{+y, +t}:
      Con{J.JA{[J.JU{x}, J.JU{y}]}, bannerpeak.cases.pairs(t)}

def bannerpeak.cases.triples(+xs: List<&2, U32>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+a, +r}:
      bannerpeak.cases.triples.b(a, r)

def bannerpeak.cases.triples.b(+a: U32, +r: List<&2, U32>) -> List<&2, J.Val>:
  match r:
    case Nil{}:
      Nil{}
    case Con{+b, +s}:
      bannerpeak.cases.triples.c(a, b, s)

def bannerpeak.cases.triples.c(+a: U32, +b: U32, +s: List<&2, U32>) -> List<&2, J.Val>:
  match s:
    case Nil{}:
      Nil{}
    case Con{+c, +t}:
      Con{J.JA{[J.JU{a}, J.JU{b}, J.JU{c}]}, bannerpeak.cases.triples(t)}

# banners(): (x, y, has) triples as null or [x, y].
def bannerpeak.cases.banners(+xs: List<&2, U32>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+a, +r}:
      bannerpeak.cases.banners.b(a, r)

def bannerpeak.cases.banners.b(+a: U32, +r: List<&2, U32>) -> List<&2, J.Val>:
  match r:
    case Nil{}:
      Nil{}
    case Con{+b, +s}:
      bannerpeak.cases.banners.c(a, b, s)

def bannerpeak.cases.banners.c(+a: U32, +b: U32, +s: List<&2, U32>) -> List<&2, J.Val>:
  match s:
    case Nil{}:
      Nil{}
    case Con{+c, +t}:
      Con{bannerpeak.u.ife(J.Val, U32.is_eq(c, 1), _ => J.JA{[J.JU{a}, J.JU{b}]}, _ => J.JNull{}), bannerpeak.cases.banners(t)}

def case.bannerpeak.layout.info(+input: J.Val) -> J.Val:
  +o0 = J.set(J.obj(), "start", J.JA{[J.JU{bannerpeak.layout.info.start_x()}, J.JU{bannerpeak.layout.info.start_y()}]})
  +o1 = J.set(o0, "gates", J.JA{bannerpeak.cases.pairs(bannerpeak.layout.info.gates())})
  +o2 = J.set(o1, "sparks", J.JA{bannerpeak.cases.pairs(bannerpeak.layout.info.sparks())})
  +o3 = J.set(o2, "coins", J.JA{bannerpeak.cases.pairs(bannerpeak.layout.info.coins())})
  +o4 = J.set(o3, "banners", J.JA{bannerpeak.cases.banners(bannerpeak.layout.info.banners())})
  +o5 = J.set(o4, "pole", J.JA{[J.JU{bannerpeak.layout.info.pole_x()}, J.JU{bannerpeak.layout.info.pole_y()}]})
  +o6 = J.set(o5, "great_pole", J.JA{[J.JU{bannerpeak.layout.info.great_pole_x()}, J.JU{bannerpeak.layout.info.great_pole_y()}]})
  +o7 = J.set(o6, "finale", J.JA{[J.JU{bannerpeak.layout.info.finale_x()}, J.JU{bannerpeak.layout.info.finale_y()}]})
  +o8 = J.set(o7, "arch", J.JA{[J.JU{bannerpeak.layout.info.arch_x()}, J.JU{bannerpeak.layout.info.arch_y()}]})
  +o9 = J.set(o8, "board", J.JA{[J.JU{bannerpeak.layout.info.board_x()}, J.JU{bannerpeak.layout.info.board_y()}]})
  +oa = J.set(o9, "cinder", J.JA{[J.JU{bannerpeak.layout.info.cinder_x()}, J.JU{bannerpeak.layout.info.cinder_y()}]})
  +ob = J.set(oa, "foes", J.JA{bannerpeak.cases.triples(bannerpeak.layout.info.foes())})
  J.set(ob, "hints", J.JA{bannerpeak.cases.triples(bannerpeak.layout.info.hints())})
