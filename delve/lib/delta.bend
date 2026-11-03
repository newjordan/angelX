import Base
import ./j.bend as J

# Deltas between two states (docs/architecture/delve-in-bend.md): an op is
# ["s", path, value], ["d", path] or ["t", path, n]; a path is object keys and
# array indices, root first. `patch` applies ops in order; `diff` answers the
# ops that turn one state into another.

# Patching
# --------

def set.at?(+v: J.Val, path: List<&2, J.Val>, x: J.Val) -> J.Val:
  match path:
    case Nil{}:
      x
    case Con{J.JS{+k}, t}:
      J.set(v, k, set.at(J.get(v, k), t, x))
    case Con{J.JU{+i}, t}:
      J.set_nth(v, i, set.at(J.nth(v, i), t, x))
    case Con{_, t}:
      v

def del.at?(+v: J.Val, path: List<&2, J.Val>) -> J.Val:
  match path:
    case Con{J.JS{+k}, Nil{}}:
      J.del(v, k)
    case Con{J.JS{+k}, t}:
      J.set(v, k, del.at(J.get(v, k), t))
    case Con{J.JU{+i}, t}:
      J.set_nth(v, i, del.at(J.nth(v, i), t))
    case _:
      v

def trunc.at?(+v: J.Val, path: List<&2, J.Val>, +n: U32) -> J.Val:
  match path:
    case Nil{}:
      J.truncate(v, n)
    case Con{J.JS{+k}, t}:
      J.set(v, k, trunc.at(J.get(v, k), t, n))
    case Con{J.JU{+i}, t}:
      J.set_nth(v, i, trunc.at(J.nth(v, i), t, n))
    case Con{_, t}:
      v

def op.pick?(tag: String, +rest: List<&2, J.Val>, +v: J.Val) -> J.Val:
  match tag rest:
    case SCon{'s', SNil{}} Con{+p, Con{+x, Nil{}}}:
      set.at(v, J.items(p), x)
    case SCon{'d', SNil{}} Con{+p, Nil{}}:
      del.at(v, J.items(p))
    case SCon{'t', SNil{}} Con{+p, Con{+n, Nil{}}}:
      trunc.at(v, J.items(p), J.u32(n))
    case _ _:
      v

def op.go?(+xs: List<&2, J.Val>, +v: J.Val) -> J.Val:
  match xs:
    case Con{+tag, +rest}:
      op.pick(J.str(tag), rest, v)
    case Nil{}:
      v

def op?(+v: J.Val, +o: J.Val) -> J.Val:
  op.go(J.items(o), v)

def patch.go?(ops: List<&2, J.Val>, +v: J.Val) -> J.Val:
  match ops:
    case Nil{}:
      v
    case Con{+o, t}:
      patch.go(t, op(v, o))

def patch(+v: J.Val, +ops: J.Val) -> J.Val:
  patch.go(J.items(ops), v)

# Diffing
# -------

def path(+rp: List<&2, J.Val>) -> J.Val:
  J.JA{List.reverse(&2, J.Val, rp)}

def op.set(+rp: List<&2, J.Val>, x: J.Val) -> J.Val:
  J.JA{[J.JS{"s"}, path(rp), x]}

def op.del(+rp: List<&2, J.Val>, k: String) -> J.Val:
  J.JA{[J.JS{"d"}, path(J.JS{k} <> rp)]}

def op.trunc(+rp: List<&2, J.Val>, +n: U32) -> J.Val:
  J.JA{[J.JS{"t"}, path(rp), J.JU{n}]}

def leaf(same: Bool, +rp: List<&2, J.Val>, +x: J.Val, acc: List<&2, J.Val>) -> List<&2, J.Val>:
  match same:
    case True{}:
      acc
    case False{}:
      op.set(rp, x) <> acc

# Ops that turn `a` into `b` at the reversed path `rp`, prepended onto `acc`
# newest first.
def go?(+a: J.Val, +b: J.Val, +rp: List<&2, J.Val>, acc: List<&2, J.Val>) -> List<&2, J.Val>:
  match a b:
    case J.JO{+am} J.JO{+bm}:
      fields(Map.to_list(&2, J.Val, am), Map.to_list(&2, J.Val, bm), rp, acc)
    case J.JA{+xs} J.JA{+ys}:
      items(xs, ys, 0, rp, acc)
    case _ _:
      leaf(J.eq(a, b), rp, b, acc)

def fields?(
  +xs: List<&2, Sigma<&2, &2, String, _ => J.Val>>,
  +ys: List<&2, Sigma<&2, &2, String, _ => J.Val>>,
  +rp: List<&2, J.Val>, acc: List<&2, J.Val>
) -> List<&2, J.Val>:
  match xs ys:
    case Nil{} Nil{}:
      acc
    case Con{(+k, +v), +t} Nil{}:
      fields(t, Nil{}, rp, op.del(rp, k) <> acc)
    case Nil{} Con{(+k, +v), +t}:
      fields(Nil{}, t, rp, op.set(J.JS{k} <> rp, v) <> acc)
    case Con{(+ak, +av), +at} Con{(+bk, +bv), +bt}:
      fields.cmp(String.order(ak, bk), xs, ys, rp, acc)

def fields.cmp?(
  c: Cmp,
  +xs: List<&2, Sigma<&2, &2, String, _ => J.Val>>,
  +ys: List<&2, Sigma<&2, &2, String, _ => J.Val>>,
  +rp: List<&2, J.Val>, acc: List<&2, J.Val>
) -> List<&2, J.Val>:
  match c xs ys:
    case EQ{} Con{(+k, +av), +at} Con{(+bk, +bv), +bt}:
      fields(at, bt, rp, go(av, bv, J.JS{k} <> rp, acc))
    case LT{} Con{(+k, +av), +at} ys:
      fields(at, ys, rp, op.del(rp, k) <> acc)
    case GT{} xs Con{(+k, +bv), +bt}:
      fields(xs, bt, rp, op.set(J.JS{k} <> rp, bv) <> acc)
    case _ _ _:
      acc

def items?(+xs: List<&2, J.Val>, +ys: List<&2, J.Val>, +i: U32, +rp: List<&2, J.Val>, acc: List<&2, J.Val>) -> List<&2, J.Val>:
  match xs ys:
    case Nil{} Nil{}:
      acc
    case Con{+x, +xt} Con{+y, +yt}:
      items(xt, yt, (i + 1 : U32), rp, go(x, y, J.JU{i} <> rp, acc))
    case Nil{} Con{+y, +yt}:
      items(Nil{}, yt, (i + 1 : U32), rp, op.set(J.JU{i} <> rp, y) <> acc)
    case Con{+x, +xt} Nil{}:
      op.trunc(rp, i) <> acc

def diff(+a: J.Val, +b: J.Val) -> J.Val:
  J.JA{List.reverse(&2, J.Val, go(a, b, Nil{}, Nil{}))}
