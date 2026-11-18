def gw.ifj(c: Bool, a: Unit -> J.Val, b: Unit -> J.Val) -> J.Val:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

def gw.ifu(c: Bool, a: Unit -> U32, b: Unit -> U32) -> U32:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

def gw.ifb(c: Bool, a: Unit -> Bool, b: Unit -> Bool) -> Bool:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

def gw.lt(+a: U32, +b: U32) -> Bool:
  RS.i_lt(a, b)

def gw.le(+a: U32, +b: U32) -> Bool:
  RS.i_le(a, b)

def gw.gt(+a: U32, +b: U32) -> Bool:
  RS.i_gt(a, b)

def gw.ge(+a: U32, +b: U32) -> Bool:
  RS.i_ge(a, b)

def gw.neg(+a: U32) -> U32:
  (0 - a : U32)

def gw.abs(+a: U32) -> U32:
  RS.i_abs(a)

def gw.min(+a: U32, +b: U32) -> U32:
  RS.i_min(a, b)

def gw.max(+a: U32, +b: U32) -> U32:
  RS.i_max(a, b)

def gw.clamp(+x: U32, +lo: U32, +hi: U32) -> U32:
  RS.i_clamp(x, lo, hi)

def gw.sig(+a: U32) -> U32:
  RS.i_signum(a)

def gw.div(+a: U32, +b: U32) -> U32:
  RS.i_div(a, b)

def gw.rem(+a: U32, +b: U32) -> U32:
  RS.i_rem(a, b)

def gw.within(+v: U32, +lo: U32, +hi: U32) -> Bool:
  Bool.and(RS.i_le(lo, v), RS.i_le(v, hi))

def gw.far(+v: U32, +m: U32) -> Bool:
  gw.within(v, gw.neg(m), m)

def gw.nle(+n: U32, +r: U32) -> Bool:
  Bool.and(Bool.not(RS.i_is_neg(r)), U32.is_le(n, r))

def gw.nlt(+n: U32, +r: U32) -> Bool:
  Bool.and(Bool.not(RS.i_is_neg(r)), U32.is_lt(n, r))

def gw.ngt(+n: U32, +r: U32) -> Bool:
  Bool.or(RS.i_is_neg(r), U32.is_gt(n, r))

def gw.isn(+o: J.Val, +k: String, +name: String) -> Bool:
  String.eq(ga.s(o, k), name)

def gw.sets(+o: J.Val, +k: String, +name: String) -> J.Val:
  J.set(o, k, J.JS{name})

def gw.addi(+o: J.Val, +k: String, +d: U32) -> J.Val:
  ga.si(o, k, (ga.u(o, k) + d : U32))

def gw.addu(+o: J.Val, +k: String, +d: U32) -> J.Val:
  ga.su(o, k, (ga.u(o, k) + d : U32))

def gw.subs(+o: J.Val, +k: String, +d: U32) -> J.Val:
  ga.su(o, k, ga.ssub(ga.u(o, k), d))

def gw.orbits(+o: J.Val, +k: String, +bits: U32) -> J.Val:
  ga.su(o, k, U32.or(ga.u(o, k), bits))

def gw.bitn(+n: U32) -> U32:
  U32.shln(1, U32.to_nat(n))

def gw.len(+o: J.Val, +k: String) -> U32:
  J.len(J.get(o, k))

def gw.item(+o: J.Val, +k: String, +i: U32) -> J.Val:
  J.nth(J.get(o, k), i)

def gw.put_item(+o: J.Val, +k: String, +i: U32, +v: J.Val) -> J.Val:
  J.set(o, k, J.set_nth(J.get(o, k), i, v))

def gw.remove(+o: J.Val, +k: String, +i: U32) -> J.Val:
  J.set(o, k, J.JA{ga.remove_nth(J.items(J.get(o, k)), U32.to_nat(i))})

def gw.trunc(+o: J.Val, +k: String, +n: U32) -> J.Val:
  J.set(o, k, J.truncate(J.get(o, k), n))

def gw.empty() -> J.Val:
  J.JA{[]}

def gw.tri3(+a: U32, +b: U32, +c: U32) -> J.Val:
  J.JA{[J.of_i32(a), J.of_i32(b), J.of_i32(c)]}

def gw.jb(+b: Bool) -> J.Val:
  J.JBool{b}

def gw.opt_u(+some: Bool, +v: U32) -> J.Val:
  Bool.pick(J.Val, some, J.JU{v}, J.JNull{})

def gw.out(+s: J.Val, +r: J.Val) -> J.Val:
  J.set(J.set(J.obj(), "s", s), "r", r)

def gw.out_s(+o: J.Val) -> J.Val:
  J.get(o, "s")

def gw.out_r(+o: J.Val) -> J.Val:
  J.get(o, "r")

def gw.umax(+a: U32, +b: U32) -> U32:
  U32.max(a, b)

def gw.umin(+a: U32, +b: U32) -> U32:
  U32.min(a, b)

def gw.nseats(+s: J.Val) -> U32:
  gw.len_id(ga.ids(J.get(s, "seats")))
