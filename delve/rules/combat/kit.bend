# The combat group's small kit: Data pairs and the state tree's field
# accessors. `cb.*` names are helpers that belong to no Rust file.

# A reusable pair (Bend's `A & B` is affine and cannot be split with `+`).
def cb.DP(-A: Data, -B: Data) -> Data:
  Sigma<&2, &2, A, _ => B>

# Reading a field of a state object (a missing or null field reads as the
# Rust Default).
def cb.gj(+o: J.Val, +k: String) -> J.Val:
  J.get(o, k)

def cb.gf(+o: J.Val, +k: String) -> F32:
  J.f32(J.get(o, k))

def cb.gu(+o: J.Val, +k: String) -> U32:
  J.u32(J.get(o, k))

def cb.gb(+o: J.Val, +k: String) -> Bool:
  J.bool(J.get(o, k))

def cb.gs(+o: J.Val, +k: String) -> String:
  J.str(J.get(o, k))

def cb.gw(+o: J.Val, +k: String) -> W.W:
  W.of_j(J.get(o, k))

# Writing one; the caller owns the skip_serializing_if rule of the field.
def cb.sj(+o: J.Val, +k: String, +v: J.Val) -> J.Val:
  J.set(o, k, v)

def cb.sf(+o: J.Val, +k: String, +v: F32) -> J.Val:
  J.set(o, k, J.of_f32(v))

def cb.su(+o: J.Val, +k: String, +v: U32) -> J.Val:
  J.set(o, k, J.of_u32(v))

def cb.sb(+o: J.Val, +k: String, +v: Bool) -> J.Val:
  J.set(o, k, J.of_bool(v))

def cb.ss(+o: J.Val, +k: String, +v: String) -> J.Val:
  J.set(o, k, J.of_str(v))

def cb.sw(+o: J.Val, +k: String, +v: W.W) -> J.Val:
  J.set(o, k, W.to_j(v))

def cb.del(+o: J.Val, +k: String) -> J.Val:
  J.del(o, k)

# An (f32, f32) tuple in the state is a two-item array.
def cb.pair(+a: J.Val) -> Sigma<&2, &2, F32, _ => F32>:
  (J.f32(J.nth(a, 0)), J.f32(J.nth(a, 1)))

def cb.of_pair(+x: F32, +y: F32) -> J.Val:
  J.JA{[J.of_f32(x), J.of_f32(y)]}

def cb.gp(+o: J.Val, +k: String) -> Sigma<&2, &2, F32, _ => F32>:
  cb.pair(J.get(o, k))

def cb.sp(+o: J.Val, +k: String, +x: F32, +y: F32) -> J.Val:
  J.set(o, k, cb.of_pair(x, y))

# A string equals a literal (string patterns do not match in Bend).
def cb.is(+o: J.Val, +k: String, +want: String) -> Bool:
  String.eq(J.str(J.get(o, k)), want)

# [0, n)
def cb.range.go(more: Bool, +i: U32, +n: U32) -> List<&2, U32>:
  match more:
    case True{}:
      i <> cb.range.go(U32.is_lt((i + 1 : U32), n), (i + 1 : U32), n)
    case False{}:
      Nil{}

def cb.range(+n: U32) -> List<&2, U32>:
  cb.range.go(U32.is_lt(0, n), 0, n)

# Option fields: null (or absent) is None.
def cb.mu(+v: J.Val) -> Maybe<&2, U32>:
  match v:
    case J.JNull{}:
      None{}
    case _:
      Some{J.u32(v)}

def cb.mf(+v: J.Val) -> Maybe<&2, F32>:
  match v:
    case J.JNull{}:
      None{}
    case _:
      Some{J.f32(v)}

def cb.ms(+v: J.Val) -> Maybe<&2, String>:
  match v:
    case J.JNull{}:
      None{}
    case _:
      Some{J.str(v)}

def cb.mp(+v: J.Val) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  match v:
    case J.JNull{}:
      None{}
    case _:
      Some{cb.pair(v)}

def cb.mw(+v: J.Val) -> Maybe<&2, W.W>:
  match v:
    case J.JNull{}:
      None{}
    case _:
      Some{W.of_j(v)}

def cb.gou(+o: J.Val, +k: String) -> Maybe<&2, U32>:
  cb.mu(J.get(o, k))

def cb.gof(+o: J.Val, +k: String) -> Maybe<&2, F32>:
  cb.mf(J.get(o, k))

def cb.gos(+o: J.Val, +k: String) -> Maybe<&2, String>:
  cb.ms(J.get(o, k))

def cb.gop(+o: J.Val, +k: String) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  cb.mp(J.get(o, k))

def cb.gow(+o: J.Val, +k: String) -> Maybe<&2, W.W>:
  cb.mw(J.get(o, k))

def cb.sp2(+o: J.Val, +k: String, +p: Sigma<&2, &2, F32, _ => F32>) -> J.Val:
  cb.sp2.go(o, k, p)

def cb.sp2.go(+o: J.Val, +k: String, p: Sigma<&2, &2, F32, _ => F32>) -> J.Val:
  (+x, +y) = p
  J.set(o, k, cb.of_pair(x, y))

def cb.sou(+o: J.Val, +k: String, +m: Maybe<&2, U32>) -> J.Val:
  match m:
    case Some{+v}:
      J.set(o, k, J.of_u32(v))
    case None{}:
      J.set(o, k, J.JNull{})

def cb.sou0(+o: J.Val, +k: String, +m: Maybe<&2, U32>) -> J.Val:
  match m:
    case Some{+v}:
      J.set(o, k, J.of_u32(v))
    case None{}:
      J.del(o, k)

def cb.sof(+o: J.Val, +k: String, +m: Maybe<&2, F32>) -> J.Val:
  match m:
    case Some{+v}:
      J.set(o, k, J.of_f32(v))
    case None{}:
      J.set(o, k, J.JNull{})

def cb.sos(+o: J.Val, +k: String, +m: Maybe<&2, String>) -> J.Val:
  match m:
    case Some{+v}:
      J.set(o, k, J.of_str(v))
    case None{}:
      J.set(o, k, J.JNull{})

def cb.sop(+o: J.Val, +k: String, +m: Maybe<&2, Sigma<&2, &2, F32, _ => F32>>) -> J.Val:
  match m:
    case Some{+v}:
      cb.sp2(o, k, v)
    case None{}:
      J.set(o, k, J.JNull{})

def cb.sow(+o: J.Val, +k: String, +m: Maybe<&2, W.W>) -> J.Val:
  match m:
    case Some{+v}:
      J.set(o, k, W.to_j(v))
    case None{}:
      J.set(o, k, J.JNull{})

# skip_serializing_if variants: the key leaves the object at its default.
def cb.su0(+o: J.Val, +k: String, +v: U32) -> J.Val:
  cb.skip.if(U32.is_zero(v), o, k, J.of_u32(v))

def cb.skip.if(drop: Bool, +o: J.Val, +k: String, +v: J.Val) -> J.Val:
  match drop:
    case True{}:
      J.del(o, k)
    case False{}:
      J.set(o, k, v)

def cb.sb0(+o: J.Val, +k: String, +v: Bool) -> J.Val:
  cb.skip.if(Bool.not(v), o, k, J.of_bool(v))

def cb.sf0(+o: J.Val, +k: String, +v: F32) -> J.Val:
  cb.skip.if(F32.is_eq(v, 0.0), o, k, J.of_f32(v))

def cb.sw0(+o: J.Val, +k: String, +v: W.W) -> J.Val:
  cb.skip.if(W.is_zero(v), o, k, W.to_j(v))

def cb.sjn(+o: J.Val, +k: String, +v: J.Val) -> J.Val:
  cb.skip.if(J.is_null(v), o, k, v)

def cb.sje(+o: J.Val, +k: String, +v: J.Val) -> J.Val:
  cb.skip.if(U32.is_zero(J.len(v)), o, k, v)

def cb.sp0.go(p: Sigma<&2, &2, F32, _ => F32>, +o: J.Val, +k: String) -> J.Val:
  (+x, +y) = p
  cb.skip.if(Bool.and(F32.is_eq(x, 0.0), F32.is_eq(y, 0.0)), o, k, cb.of_pair(x, y))

def cb.sp0(+o: J.Val, +k: String, +p: Sigma<&2, &2, F32, _ => F32>) -> J.Val:
  cb.sp0.go(p, o, k)

# Pushing onto an array held in an object (a Rust `Vec::push` on a field).
def cb.push(+o: J.Val, +k: String, +v: J.Val) -> J.Val:
  J.set(o, k, J.push(J.get(o, k), v))

def cb.len(+o: J.Val, +k: String) -> U32:
  J.len(J.get(o, k))

def cb.sskip(+o: J.Val, +k: String, +v: String, +omit: String) -> J.Val:
  cb.skip.if(String.eq(v, omit), o, k, J.of_str(v))

# A state value written as BJ1 text (floats as f + 8 hex digits of the f32).
def cb.lit(+text: String) -> J.Val:
  cb.lit.go(B.read(text))

def cb.lit.go(r: B.Read) -> J.Val:
  match r:
    case B.Got{+c, rest}:
      c
    case B.Bad{at}:
      J.JNull{}

# Rust's `as` casts and the like, short.
def cb.f(+n: U32) -> F32:
  RS.u_as_f32(n)

def cb.imin(+a: U32, +b: U32) -> U32:
  RS.u_min(a, b)

def cb.fmin(+a: F32, +b: F32) -> F32:
  RS.f_min(a, b)

def cb.fmax(+a: F32, +b: F32) -> F32:
  RS.f_max(a, b)

def cb.hyp(+a: F32, +b: F32) -> F32:
  RS.f_hypot(a, b)

def cb.sub0(+a: U32, +b: U32) -> U32:
  RS.u_sat_sub(a, b)

def cb.add0(+a: U32, +b: U32) -> U32:
  RS.u_sat_add(a, b)

# A Vec<Projectile>-shaped list, as the state holds it.
def cb.nth_pair(+a: J.Val, +i: U32) -> Sigma<&2, &2, F32, _ => F32>:
  cb.pair(J.nth(a, i))


def cb.pair_zero(+p: Sigma<&2, &2, F32, _ => F32>) -> Bool:
  cb.pair_zero.go(p)

def cb.pair_zero.go(+p: Sigma<&2, &2, F32, _ => F32>) -> Bool:
  (+x, +y) = p
  Bool.and(F32.is_eq(x, 0.0), F32.is_eq(y, 0.0))

# A reusable pair as the affine pair the other groups' functions take.
def cb.aff(+p: Sigma<&2, &2, F32, _ => F32>) -> F32 & F32:
  cb.aff.go(p)

def cb.aff.go(p: Sigma<&2, &2, F32, _ => F32>) -> F32 & F32:
  (+x, +y) = p
  (x, y)

def cb.dat(p: F32 & F32) -> Sigma<&2, &2, F32, _ => F32>:
  (+x, +y) = p
  (x, y)

def cb.of_f32s(+xs: List<&2, F32>) -> J.Val:
  J.JA{cb.of_f32s.go(xs)}

def cb.of_f32s.go(+xs: List<&2, F32>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      J.of_f32(h) <> cb.of_f32s.go(t)

def cb.of_pairs(+xs: List<&2, Sigma<&2, &2, F32, _ => F32>>) -> J.Val:
  J.JA{cb.of_pairs.go(xs)}

def cb.of_pairs.go(+xs: List<&2, Sigma<&2, &2, F32, _ => F32>>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      cb.of_pair2(h) <> cb.of_pairs.go(t)

def cb.of_pair2(+p: Sigma<&2, &2, F32, _ => F32>) -> J.Val:
  cb.of_pair2.go(p)

def cb.of_pair2.go(p: Sigma<&2, &2, F32, _ => F32>) -> J.Val:
  (+x, +y) = p
  cb.of_pair(x, y)

def cb.of_opt_f32(+m: Maybe<&2, F32>) -> J.Val:
  match m:
    case Some{+v}:
      J.of_f32(v)
    case None{}:
      J.JNull{}

def cb.of_opt_pair(+m: Maybe<&2, Sigma<&2, &2, F32, _ => F32>>) -> J.Val:
  match m:
    case Some{+v}:
      cb.of_pair2(v)
    case None{}:
      J.JNull{}

# Rust's `Deeds::default()`.
def foes.Deeds.default() -> J.Val:
  cb.lit("{\"clouds\":[],\"pulls\":[],\"sweeps\":[],\"sounds\":[],\"shake\":0,\"spawns\":[],\"kegs\":[],\"lobs\":[],\"flings\":[],\"wells\":[],\"rams\":[],\"escaped\":[],\"cues\":[],\"hooks\":[],\"webs\":[],\"rot\":[],\"slams\":[],\"ravages\":[],\"bursts\":[],\"holes\":[],\"crit\":{\"erupts\":[],\"bashes\":[],\"globs\":[],\"rings\":[],\"rams\":[]},\"swamp\":{\"grabs\":[],\"lures\":[]}}")

# An (x, y) pair held as a Data pair, as the state's [x, y].
def cb.pair_j(+p: Sigma<&2, &2, F32, _ => F32>) -> J.Val:
  cb.of_pair2(p)

# A BTreeMap<u32, _> key back to its number.
def cb.key_u32(+k: String) -> U32:
  cb.key_u32.go(U32.read(k))

def cb.key_u32.go(m: Maybe<&2, U32>) -> U32:
  match m:
    case Some{+v}:
      v
    case None{}:
      0

def cb.sos0(+o: J.Val, +k: String, +m: Maybe<&2, String>) -> J.Val:
  match m:
    case Some{+v}:
      J.set(o, k, J.of_str(v))
    case None{}:
      J.del(o, k)
