# core.* — small readers and writers over the J tree that the core group's
# modules share (not Rust functions: invented helpers, named core.<x>).

# Field `k` of object `o` read as a number / bool / text, Rust defaults when
# absent.
def core.u(+o: J.Val, +k: String) -> U32:
  J.u32(J.get(o, k))

def core.f(+o: J.Val, +k: String) -> F32:
  J.f32(J.get(o, k))

def core.b(+o: J.Val, +k: String) -> Bool:
  J.bool(J.get(o, k))

def core.s(+o: J.Val, +k: String) -> String:
  J.str(J.get(o, k))

# Field `k` set to a number / bool / text.
def core.set_u(+o: J.Val, +k: String, +v: U32) -> J.Val:
  J.set(o, k, J.JU{v})

def core.set_f(+o: J.Val, +k: String, +v: F32) -> J.Val:
  J.set(o, k, J.JF{v})

def core.set_b(+o: J.Val, +k: String, +v: Bool) -> J.Val:
  J.set(o, k, J.JBool{v})

def core.set_s(+o: J.Val, +k: String, +v: String) -> J.Val:
  J.set(o, k, J.JS{v})

# `serde(skip_serializing_if = zero / Not::not)`: a zero or false value is
# left out of the object.
def core.set_u_skip.if(zero: Bool, +o: J.Val, +k: String, +v: U32) -> J.Val:
  match zero:
    case True{}:
      J.del(o, k)
    case False{}:
      J.set(o, k, J.JU{v})

def core.set_u_skip(+o: J.Val, +k: String, +v: U32) -> J.Val:
  core.set_u_skip.if(U32.is_zero(v), o, k, v)

def core.set_b_skip.if(off: Bool, +o: J.Val, +k: String) -> J.Val:
  match off:
    case True{}:
      J.del(o, k)
    case False{}:
      J.set(o, k, J.JBool{True{}})

def core.set_b_skip(+o: J.Val, +k: String, +v: Bool) -> J.Val:
  core.set_b_skip.if(Bool.not(v), o, k)

# A value, or the other when the first is JNull (Option::unwrap_or).
def core.or_else.if(null: Bool, +a: J.Val, +b: J.Val) -> J.Val:
  match null:
    case True{}:
      b
    case False{}:
      a

def core.or_else(+a: J.Val, +b: J.Val) -> J.Val:
  core.or_else.if(J.is_null(a), a, b)

# Item `i` of array `a` read as a number / float.
def core.nu(+a: J.Val, +i: U32) -> U32:
  J.u32(J.nth(a, i))

def core.nf(+a: J.Val, +i: U32) -> F32:
  J.f32(J.nth(a, i))

# A two-item array.
def core.pair(+a: J.Val, +b: J.Val) -> J.Val:
  J.JA{[a, b]}

def core.pair_f(+x: F32, +y: F32) -> J.Val:
  J.JA{[J.JF{x}, J.JF{y}]}

# Max / min of two numbers; one number's saturating step.
def core.umax(+a: U32, +b: U32) -> U32:
  U32.max(a, b)

def core.umin(+a: U32, +b: U32) -> U32:
  U32.min(a, b)

def core.usub(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_gt(a, b), (a - b : U32), 0)

def core.uadd(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_lt((a + b : U32), a), 4294967295, (a + b : U32))

# a * b with wrapping, a / b with Rust's zero check left to the caller.
def core.udiv(+a: U32, +b: U32) -> U32:
  U32.div(a, b)

# Rust's `a.div_ceil(b)` for b > 0.
def core.div_ceil(+a: U32, +b: U32) -> U32:
  U32.div((a + b - 1 : U32), b)

# The number of entries in an object (`BTreeMap::len`) or items in an array.
def core.count(+o: J.Val) -> U32:
  U32.from_nat((List.length(&2, J.Val, J.items(o)) + List.length(&2, Sigma<&2, &2, String, _ => J.Val>, J.fields(o)) : Nat))

# `std::f32::consts::TAU` and `PI`.
def core.TAU() -> F32:
  6.2831855

def core.PI() -> F32:
  3.1415927

# Whether `v` is among `xs`.
def core.has_u32(+xs: List<&2, U32>, +v: U32) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(U32.is_eq(h, v), core.has_u32(t, v))

# A hostile shot as the monsters loose it (`Projectile { hostile: true, look: None, last_hit: None,
# empowered: false, traits: Default, .. }`).
def core.hostile_shot(+x: F32, +y: F32, +vx: F32, +vy: F32, +kind: String, +damage: U32, +pierce: U32, +ttl: U32) -> J.Val:
  +p0 = J.set(J.set(J.set(J.set(J.obj(), "x", J.JF{x}), "y", J.JF{y}), "vx", J.JF{vx}), "vy", J.JF{vy})
  +p1 = J.set(J.set(J.set(p0, "hostile", J.JBool{True{}}), "kind", J.JS{kind}), "look", J.JNull{})
  +p2 = J.set(J.set(J.set(p1, "damage", J.JU{damage}), "pierce", J.JU{pierce}), "last_hit", J.JNull{})
  +p3 = J.set(J.set(p2, "empowered", J.JBool{False{}}), "traits", projectile.ShotTraits.default())
  J.set(p3, "ttl", J.JU{ttl})

# `a.saturating_add(b)` on u8 words.
def core.sat_add8(+a: U32, +b: U32) -> U32:
  U32.min((a + b : U32), 255)

# Whether the J array holds the number `v`.
def core.has_ju(+xs: List<&2, J.Val>, +v: U32) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(U32.is_eq(J.u32(h), v), core.has_ju(t, v))
