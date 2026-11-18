def ga.v(+o: J.Val, +k: String) -> J.Val:
  J.get(o, k)

def ga.u(+o: J.Val, +k: String) -> U32:
  J.u32(J.get(o, k))

def ga.f(+o: J.Val, +k: String) -> F32:
  J.f32(J.get(o, k))

def ga.b(+o: J.Val, +k: String) -> Bool:
  J.bool(J.get(o, k))

def ga.s(+o: J.Val, +k: String) -> String:
  J.str(J.get(o, k))

def ga.nu(+a: J.Val, +i: U32) -> U32:
  J.u32(J.nth(a, i))

def ga.nf(+a: J.Val, +i: U32) -> F32:
  J.f32(J.nth(a, i))

def ga.su(o: J.Val, +k: String, +v: U32) -> J.Val:
  J.set(o, k, J.JU{v})

def ga.si(o: J.Val, +k: String, +v: U32) -> J.Val:
  J.set(o, k, J.of_i32(v))

def ga.sf(o: J.Val, +k: String, +v: F32) -> J.Val:
  J.set(o, k, J.JF{v})

def ga.sb(o: J.Val, +k: String, +v: Bool) -> J.Val:
  J.set(o, k, J.JBool{v})

def ga.ss(o: J.Val, +k: String, +v: String) -> J.Val:
  J.set(o, k, J.JS{v})

def ga.sv(o: J.Val, +k: String, v: J.Val) -> J.Val:
  J.set(o, k, v)

def ga.su_skip.if(zero: Bool, o: J.Val, +k: String, +v: U32) -> J.Val:
  match zero:
    case True{}:
      J.del(o, k)
    case False{}:
      J.set(o, k, J.JU{v})

def ga.su_skip(o: J.Val, +k: String, +v: U32) -> J.Val:
  ga.su_skip.if(U32.is_zero(v), o, k, v)

def ga.sb_skip.if(off: Bool, o: J.Val, +k: String) -> J.Val:
  match off:
    case True{}:
      J.del(o, k)
    case False{}:
      J.set(o, k, J.JBool{True{}})

def ga.sb_skip(o: J.Val, +k: String, +v: Bool) -> J.Val:
  ga.sb_skip.if(Bool.not(v), o, k)

def ga.sv_skip.if(none: Bool, o: J.Val, +k: String, v: J.Val) -> J.Val:
  match none:
    case True{}:
      J.del(o, k)
    case False{}:
      J.set(o, k, v)

def ga.sv_skip(o: J.Val, +k: String, +v: J.Val) -> J.Val:
  ga.sv_skip.if(J.is_null(v), o, k, v)

def ga.pu(+c: Bool, +a: U32, +b: U32) -> U32:
  Bool.pick(U32, c, a, b)

def ga.pf(+c: Bool, +a: F32, +b: F32) -> F32:
  Bool.pick(F32, c, a, b)

def ga.pb(+c: Bool, +a: Bool, +b: Bool) -> Bool:
  Bool.pick(Bool, c, a, b)

def ga.pv(+c: Bool, a: J.Val, b: J.Val) -> J.Val:
  Bool.pick(J.Val, c, a, b)

def ga.key(+n: U32) -> String:
  U32.show(n)

def ga.at(+m: J.Val, +id: U32) -> J.Val:
  J.get(m, U32.show(id))

def ga.put_at(m: J.Val, +id: U32, v: J.Val) -> J.Val:
  J.set(m, U32.show(id), v)

def ga.has_key(+m: J.Val, +id: U32) -> Bool:
  J.has(m, U32.show(id))

def ga.ids.go(xs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{(+k, +v), t}:
      Con{Maybe.default(&2, U32, U32.read(k), 0), ga.ids.go(t)}

def ga.ids(+m: J.Val) -> List<&2, U32>:
  ga.ids.go(J.int_fields(m))

def ga.pair(a: J.Val, b: J.Val) -> J.Val:
  J.JA{[a, b]}

def ga.pair_f(+x: F32, +y: F32) -> J.Val:
  J.JA{[J.JF{x}, J.JF{y}]}

def ga.pair_i(+x: U32, +y: U32) -> J.Val:
  J.JA{[J.of_i32(x), J.of_i32(y)]}

def ga.trio(a: J.Val, b: J.Val, c: J.Val) -> J.Val:
  J.JA{[a, b, c]}

def ga.ssub(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_ge(a, b), (a - b : U32), 0)

def ga.sadd8(+a: U32, +b: U32) -> U32:
  U32.min((a + b : U32), 255)

def ga.sadd16(+a: U32, +b: U32) -> U32:
  U32.min((a + b : U32), 65535)

def ga.sadd32(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_lt((a + b : U32), a), 4294967295, (a + b : U32))

def ga.items(+o: J.Val, +k: String) -> List<&2, J.Val>:
  J.items(J.get(o, k))

def ga.push(+o: J.Val, +k: String, v: J.Val) -> J.Val:
  J.set(o, k, J.push(J.get(o, k), v))

def ga.remove_nth(+xs: List<&2, J.Val>, +n: Nat) -> List<&2, J.Val>:
  match xs n:
    case Nil{} _:
      Nil{}
    case Con{h, t} 0n:
      t
    case Con{h, t} 1n+p:
      Con{h, ga.remove_nth(t, p)}

def ga.count(+xs: List<&2, J.Val>) -> U32:
  U32.from_nat(List.length(&2, J.Val, xs))

def ga.is(+s: J.Val, +name: String) -> Bool:
  String.eq(J.str(s), name)

def ga.stone(+t: J.Val) -> Bool:
  Bool.or(ga.is(t, "wall"), ga.is(t, "block"))

def ga.umax(+a: U32, +b: U32) -> U32:
  U32.max(a, b)

def ga.umin(+a: U32, +b: U32) -> U32:
  U32.min(a, b)

def ga.pick(+o: J.Val, +ks: List<&2, String>) -> J.Val:
  ga.pick.go(o, ks, J.obj())

def ga.pick.go(+o: J.Val, +ks: List<&2, String>, +acc: J.Val) -> J.Val:
  match ks:
    case Nil{}:
      acc
    case Con{+k, +t}:
      ga.pick.go(o, t, ga.pick.put(J.has(o, k), o, k, acc))

def ga.pick.put(has: Bool, +o: J.Val, +k: String, +acc: J.Val) -> J.Val:
  match has:
    case True{}:
      J.set(acc, k, J.get(o, k))
    case False{}:
      acc

def ga.grid(+room: J.Val, +barred: Bool) -> J.Val:
  ga.sv(ga.sb(J.obj(), "barred", barred), "room", room)

def ga.xys(+a: J.Val) -> List<&2, Sigma<&2, &2, F32, _ => F32>>:
  ga.xys.go(J.items(a))

def ga.xys.go(+xs: List<&2, J.Val>) -> List<&2, Sigma<&2, &2, F32, _ => F32>>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Con{(J.f32(J.nth(h, 0)), J.f32(J.nth(h, 1))), ga.xys.go(t)}

def ga.ps(+c: Bool, +a: String, +b: String) -> String:
  Bool.pick(String, c, a, b)

def ga.nfields(+o: J.Val) -> U32:
  U32.from_nat(List.length(&2, Sigma<&2, &2, String, _ => J.Val>, J.fields(o)))
