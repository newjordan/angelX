def bannerpeak.u.ife(-A: Type, c: Bool, a: Unit -> A, b: Unit -> A) -> A:
  match c:
    case True{}:
      a(Unit{})
    case False{}:
      b(Unit{})

def bannerpeak.u.fst(-A: Data, -B: Data, +p: Sigma<&2, &2, A, _ => B>) -> A:
  match p:
    case (+a, +b):
      a

def bannerpeak.u.snd(-A: Data, -B: Data, +p: Sigma<&2, &2, A, _ => B>) -> B:
  match p:
    case (+a, +b):
      b

def bannerpeak.u.b2u(+b: Bool) -> U32:
  Bool.pick(U32, b, 1, 0)

def bannerpeak.u.sat_sub(+a: U32, +b: U32) -> U32:
  RS.u_sat_sub(a, b)

def bannerpeak.u.head(+xs: List<&2, U32>) -> U32:
  match xs:
    case Nil{}:
      0
    case Con{+h, +t}:
      h

def bannerpeak.u.tail(+xs: List<&2, U32>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      t

def bannerpeak.u.is_empty(+xs: List<&2, U32>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Con{+h, +t}:
      False{}

def bannerpeak.u.nth(+xs: List<&2, U32>, +n: U32) -> U32:
  bannerpeak.u.nth.go(xs, U32.to_nat(n))

def bannerpeak.u.nth.go(+xs: List<&2, U32>, +n: Nat) -> U32:
  match xs n:
    case Nil{} _:
      0
    case Con{+h, +t} 0n:
      h
    case Con{+h, +t} 1n+p:
      bannerpeak.u.nth.go(t, p)

def bannerpeak.u.ri2(+j: J.Val) -> Sigma<&2, &2, U32, _ => U32>:
  (J.u32(J.nth(j, 0)), J.u32(J.nth(j, 1)))

def bannerpeak.u.ji2(+p: Sigma<&2, &2, U32, _ => U32>) -> J.Val:
  J.JA{[J.of_i32(bannerpeak.u.fst(U32, U32, p)), J.of_i32(bannerpeak.u.snd(U32, U32, p))]}

def bannerpeak.u.ids(+m: J.Val) -> List<&2, U32>:
  ga.ids(m)

def bannerpeak.u.at(+m: J.Val, +id: U32) -> J.Val:
  J.get(m, U32.show(id))

def bannerpeak.u.at_u32(+m: J.Val, +id: U32) -> U32:
  J.u32(J.get(m, U32.show(id)))

def bannerpeak.u.put(+m: J.Val, +id: U32, +v: J.Val) -> J.Val:
  J.set(m, U32.show(id), v)

def bannerpeak.u.has(+m: J.Val, +id: U32) -> Bool:
  J.has(m, U32.show(id))

def bannerpeak.u.del(+m: J.Val, +id: U32) -> J.Val:
  J.del(m, U32.show(id))

def bannerpeak.u.vec_remove(+a: J.Val, +i: U32) -> J.Val:
  J.JA{ga.remove_nth(J.items(a), U32.to_nat(i))}

def bannerpeak.u.vec_len(+a: J.Val) -> U32:
  J.len(a)

def bannerpeak.u.vec_new() -> J.Val:
  J.JA{Nil{}}

def bannerpeak.u.vec_clear(+a: J.Val) -> J.Val:
  J.JA{Nil{}}

def bannerpeak.u.set_has(+a: J.Val, +x: U32) -> Bool:
  bannerpeak.u.set_has.go(J.items(a), x)

def bannerpeak.u.set_has.go(+xs: List<&2, J.Val>, +x: U32) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(U32.is_eq(J.u32(h), x), bannerpeak.u.set_has.go(t, x))

def bannerpeak.u.set_insert(+a: J.Val, +x: U32) -> J.Val:
  J.JA{bannerpeak.u.set_insert.go(J.items(a), x)}

def bannerpeak.u.set_insert.go(+xs: List<&2, J.Val>, +x: U32) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      [J.JU{x}]
    case Con{+h, +t}:
      bannerpeak.u.set_insert.at(U32.is_lt(x, J.u32(h)), U32.is_eq(x, J.u32(h)), h, t, x)

def bannerpeak.u.set_insert.at(+before: Bool, +same: Bool, +h: J.Val, +t: List<&2, J.Val>, +x: U32) -> List<&2, J.Val>:
  match before:
    case True{}:
      Con{J.JU{x}, Con{h, t}}
    case False{}:
      match same:
        case True{}:
          Con{h, t}
        case False{}:
          Con{h, bannerpeak.u.set_insert.go(t, x)}

def bannerpeak.u.vec_or(+j: J.Val) -> J.Val:
  match j:
    case J.JNull{}:
      J.JA{Nil{}}
    case _:
      j

def bannerpeak.u.input_default() -> J.Val:
  J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.set(J.obj(), "move_x", J.JU{0}), "move_y", J.JU{0}), "aim_x", J.JU{0}), "aim_y", J.JU{0}), "fire", J.JBool{False{}}), "dash", J.JBool{False{}}), "bomb", J.JBool{False{}}), "swing", J.JBool{False{}}), "play", J.JU{0}), "cast", J.JU{0})

def bannerpeak.u.map_len(+m: J.Val) -> U32:
  U32.from_nat(List.length(&2, Sigma<&2, &2, String, _ => J.Val>, J.fields(m)))

def bannerpeak.u.view_insert(+m: J.Val, +key: U32, +code: U32) -> J.Val:
  J.set(m, U32.show(key), J.JU{code})

def bannerpeak.u.board_banner(+i: U32) -> U32:
  bannerpeak.u.nth([2, 3, 4, 5, 6, 7], i)

def bannerpeak.u.board_gate(+i: U32) -> U32:
  bannerpeak.u.nth([0, 2, 3, 5, 0, 7], i)

def bannerpeak.u.pole_score(+i: U32) -> U32:
  bannerpeak.u.nth([100, 200, 400, 800, 1000, 2000, 4000, 5000, 8000, 10000], i)
