import Base

type Val is Data:
  JNull{}
  JBool{v: Bool}
  JU{v: U32}
  JI{v: U32}
  JW{hi: U32, lo: U32}
  JF{v: F32}
  JS{v: String}
  JA{v: List<&2, Val>}
  JO{v: Map<&2, Val>}

def get.in(r: Map<&2, Val> & Val) -> Val:
  (m, v) = r
  v

def get(+o: Val, +k: String) -> Val:
  match o:
    case JO{+m}:
      get.in(Map.get(Val, JNull{}, m, k))
    case _:
      JNull{}

def has.in(r: Map<&2, Val> & Bool) -> Bool:
  (m, b) = r
  b

def has(+o: Val, +k: String) -> Bool:
  match o:
    case JO{+m}:
      has.in(Map.has(&2, Val, m, k))
    case _:
      False{}

def at.go(ks: List<&2, String>, +o: Val) -> Val:
  match ks:
    case Nil{}:
      o
    case Con{+k, t}:
      at.go(t, get(o, k))

def at(+o: Val, ks: List<&2, String>) -> Val:
  at.go(ks, o)

def nth.go(+xs: List<&2, Val>, n: Nat) -> Val:
  match xs n:
    case Nil{} _:
      JNull{}
    case Con{+h, +t} 0n:
      h
    case Con{+h, +t} 1n+p:
      nth.go(t, p)

def nth(+a: Val, +i: U32) -> Val:
  match a:
    case JA{+xs}:
      nth.go(xs, U32.to_nat(i))
    case _:
      JNull{}

def items(+a: Val) -> List<&2, Val>:
  match a:
    case JA{+xs}:
      xs
    case _:
      Nil{}

def fields(+o: Val) -> List<&2, Sigma<&2, &2, String, _ => Val>>:
  match o:
    case JO{+m}:
      Map.to_list(&2, Val, m)
    case _:
      Nil{}

def len(+a: Val) -> U32:
  U32.from_nat(List.length(&2, Val, items(a)))

def is_null(+j: Val) -> Bool:
  match j:
    case JNull{}:
      True{}
    case _:
      False{}

def bool(+j: Val) -> Bool:
  match j:
    case JBool{+v}:
      v
    case _:
      False{}

def u32(+j: Val) -> U32:
  match j:
    case JU{+v}:
      v
    case JI{+v}:
      (0 - v : U32)
    case JW{+hi, +lo}:
      lo
    case JF{+v}:
      F32.to_u32(v)
    case _:
      0

def i32(+j: Val) -> U32:
  u32(j)

def w64(+j: Val) -> U32 & U32:
  match j:
    case JU{+v}:
      (0, v)
    case JI{+v}:
      (4294967295, (0 - v : U32))
    case JW{+hi, +lo}:
      (hi, lo)
    case _:
      (0, 0)

def f32(+j: Val) -> F32:
  match j:
    case JF{+v}:
      v
    case JU{+v}:
      U32.to_f32(v)
    case JI{+v}:
      F32.neg(U32.to_f32(v))
    case _:
      0.0

def str(+j: Val) -> String:
  match j:
    case JS{+v}:
      v
    case _:
      ""

def obj() -> Val:
  JO{Map.new(&2, Val)}

def of_u32(+v: U32) -> Val:
  JU{v}

def of_i32.if(neg: Bool, +v: U32) -> Val:
  match neg:
    case True{}:
      JI{(0 - v : U32)}
    case False{}:
      JU{v}

def of_i32(+v: U32) -> Val:
  of_i32.if(U32.is_ge(v, 2147483648), v)

def of_w64.if(small: Bool, +hi: U32, +lo: U32) -> Val:
  match small:
    case True{}:
      JU{lo}
    case False{}:
      JW{hi, lo}

def of_w64(+hi: U32, +lo: U32) -> Val:
  of_w64.if(U32.is_zero(hi), hi, lo)

def of_f32(+v: F32) -> Val:
  JF{v}

def of_bool(+v: Bool) -> Val:
  JBool{v}

def of_str(+v: String) -> Val:
  JS{v}

def set(o: Val, k: String, v: Val) -> Val:
  match o:
    case JO{m}:
      JO{Map.set(&2, Val, m, k, v)}
    case _:
      JO{Map.set(&2, Val, Map.new(&2, Val), k, v)}

def del(o: Val, k: String) -> Val:
  match o:
    case JO{m}:
      JO{Map.del(&2, Val, m, k)}
    case _:
      o

def put.go(ks: List<&2, String>, +o: Val, v: Val) -> Val:
  match ks:
    case Nil{}:
      v
    case Con{+k, t}:
      set(o, k, put.go(t, get(o, k), v))

def put(+o: Val, ks: List<&2, String>, v: Val) -> Val:
  put.go(ks, o, v)

def set_nth.go(xs: List<&2, Val>, n: Nat, v: Val) -> List<&2, Val>:
  match xs n:
    case Nil{} _:
      [v]
    case Con{h, t} 0n:
      v <> t
    case Con{h, t} 1n+p:
      h <> set_nth.go(t, p, v)

def set_nth(+a: Val, +i: U32, v: Val) -> Val:
  JA{set_nth.go(items(a), U32.to_nat(i), v)}

def push(+a: Val, v: Val) -> Val:
  JA{List.append(&2, Val, items(a), [v])}

def truncate(+a: Val, +n: U32) -> Val:
  JA{List.take(&2, Val, items(a), U32.to_nat(n))}

def bool_eq(x: Bool, y: Bool) -> Bool:
  match x y:
    case True{} True{}:
      True{}
    case False{} False{}:
      True{}
    case _ _:
      False{}

def eq?(+a: Val, +b: Val) -> Bool:
  match a b:
    case JNull{} JNull{}:
      True{}
    case JBool{+x} JBool{+y}:
      bool_eq(x, y)
    case JU{+x} JU{+y}:
      U32.is_eq(x, y)
    case JI{+x} JI{+y}:
      U32.is_eq(x, y)
    case JW{+xh, +xl} JW{+yh, +yl}:
      Bool.and(U32.is_eq(xh, yh), U32.is_eq(xl, yl))
    case JF{+x} JF{+y}:
      U32.is_eq(F32.bits(x), F32.bits(y))
    case JS{+x} JS{+y}:
      String.eq(x, y)
    case JA{+xs} JA{+ys}:
      eq.list(xs, ys)
    case JO{+xm} JO{+ym}:
      eq.list.kv(Map.to_list(&2, Val, xm), Map.to_list(&2, Val, ym))
    case _ _:
      False{}

def eq.list?(+xs: List<&2, Val>, +ys: List<&2, Val>) -> Bool:
  match xs ys:
    case Nil{} Nil{}:
      True{}
    case Con{+x, +xt} Con{+y, +yt}:
      Bool.and(eq(x, y), eq.list(xt, yt))
    case _ _:
      False{}

def eq.list.kv?(+xs: List<&2, Sigma<&2, &2, String, _ => Val>>, +ys: List<&2, Sigma<&2, &2, String, _ => Val>>) -> Bool:
  match xs ys:
    case Nil{} Nil{}:
      True{}
    case Con{(+xk, +xv), +xt} Con{(+yk, +yv), +yt}:
      Bool.and(String.eq(xk, yk), Bool.and(eq(xv, yv), eq.list.kv(xt, yt)))
    case _ _:
      False{}

def key.lt.cmp(c: Cmp, +a: String, +b: String) -> Bool:
  match c:
    case LT{}:
      True{}
    case GT{}:
      False{}
    case EQ{}:
      Cmp.is_lt(String.order(a, b))

def key.lt(+a: String, +b: String) -> Bool:
  key.lt.cmp(Nat.cmp(String.length(a), String.length(b)), a, b)

def int_fields.ins?(+k: String, +v: Val, +xs: List<&2, Sigma<&2, &2, String, _ => Val>>) -> List<&2, Sigma<&2, &2, String, _ => Val>>:
  match xs:
    case Nil{}:
      [(k, v)]
    case Con{(+hk, +hv), +t}:
      int_fields.ins.if(key.lt(k, hk), k, v, xs)

def int_fields.ins.if?(before: Bool, +k: String, +v: Val, +xs: List<&2, Sigma<&2, &2, String, _ => Val>>) -> List<&2, Sigma<&2, &2, String, _ => Val>>:
  match before xs:
    case True{} xs:
      (k, v) <> xs
    case False{} Con{(+hk, +hv), +t}:
      (hk, hv) <> int_fields.ins(k, v, t)
    case False{} Nil{}:
      [(k, v)]

def int_fields.go?(+xs: List<&2, Sigma<&2, &2, String, _ => Val>>, +acc: List<&2, Sigma<&2, &2, String, _ => Val>>) -> List<&2, Sigma<&2, &2, String, _ => Val>>:
  match xs:
    case Nil{}:
      acc
    case Con{(+k, +v), +t}:
      int_fields.go(t, int_fields.ins(k, v, acc))

def int_fields(+o: Val) -> List<&2, Sigma<&2, &2, String, _ => Val>>:
  int_fields.go(fields(o), Nil{})
