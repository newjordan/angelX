# crates/delve/src/rules/griffinwing/types.rs — Griffinwing's bodies and the
# little arithmetic the sim shares. Enums are their serde strings ("Wisp",
# "Fly", ...); structs are J objects in serde shape.

def griffinwing.types.Q() -> U32:
  256

# A length in tenths of a unit, as fixed point: `n * Q / 10`, i32 division.
def griffinwing.types.dm(+n: U32) -> U32:
  RS.i_div((n * 256 : U32), 10)

def griffinwing.types.PACE() -> U32:
  85

def griffinwing.types.LEAD() -> U32:
  griffinwing.types.dm(640)

def griffinwing.types.FAR() -> U32:
  griffinwing.types.dm(340)

def griffinwing.types.NEAR() -> U32:
  griffinwing.types.dm(150)

def griffinwing.types.LOCK_MAX() -> U32:
  griffinwing.types.dm(520)

def griffinwing.types.LOCK_MIN() -> U32:
  griffinwing.types.dm(15)

def griffinwing.types.MAX_LOCKS() -> U32:
  6

def griffinwing.types.MAX_PER_BODY() -> U32:
  3

def griffinwing.types.MAX_FOES() -> U32:
  40

def griffinwing.types.MAX_PROPS() -> U32:
  40

def griffinwing.types.MAX_BOLTS() -> U32:
  160

def griffinwing.types.MAX_CALLS() -> U32:
  4

def griffinwing.types.MAX_FX() -> U32:
  48

def griffinwing.types.SHIELD() -> U32:
  100

def griffinwing.types.WING() -> U32:
  4

# The spur's meter, full.
def griffinwing.types.METER() -> U32:
  7200

# Kind: its body's size, health and worth.
def griffinwing.types.Kind.pick(+k: String, +wisp: U32, +kite: U32, +turret: U32, +bomber: U32, +lamp: U32) -> U32:
  Bool.pick(U32, String.eq(k, "Wisp"), wisp,
    Bool.pick(U32, String.eq(k, "Kite"), kite,
      Bool.pick(U32, String.eq(k, "Turret"), turret,
        Bool.pick(U32, String.eq(k, "Bomber"), bomber, lamp))))

def griffinwing.types.Kind.radius(+k: String) -> U32:
  griffinwing.types.Kind.pick(k, griffinwing.types.dm(9), griffinwing.types.dm(8), griffinwing.types.dm(16), griffinwing.types.dm(12), griffinwing.types.dm(16))

def griffinwing.types.Kind.health(+k: String) -> U32:
  griffinwing.types.Kind.pick(k, 20, 20, 60, 40, 120)

def griffinwing.types.Kind.worth(+k: String) -> U32:
  griffinwing.types.Kind.pick(k, 10, 20, 60, 40, 500)

def griffinwing.types.Foe.radius(+f: J.Val) -> U32:
  griffinwing.types.Kind.radius(ga.s(f, "kind"))

def griffinwing.types.Foe.alive(+f: J.Val) -> Bool:
  Bool.and(U32.is_gt(ga.u(f, "hp"), 0), Bool.not(gw.isn(f, "st", "Broken")))

# Whether a lock may take it: alive, in the open and (a lamp) with its shield
# open.
def griffinwing.types.Foe.lockable(+f: J.Val) -> Bool:
  Bool.and(griffinwing.types.Foe.alive(f),
    Bool.not(Bool.or(gw.isn(f, "st", "Hide"), Bool.or(gw.isn(f, "st", "Shut"), gw.isn(f, "st", "Flee")))))

def gw.alls(+bs: List<&2, Bool>) -> Bool:
  match bs:
    case Nil{}:
      True{}
    case Con{+h, +t}:
      Bool.and(h, gw.alls(t))

# A craft at its slot, shield full and three bombs.
def griffinwing.types.Craft.new(+x: U32, +z: U32) -> J.Val:
  +c0 = J.obj()
  +c1 = ga.si(c0, "x", x)
  +c2 = ga.si(c1, "z", z)
  +c3 = ga.si(c2, "vx", 0)
  +c4 = ga.si(c3, "vz", 0)
  +c5 = ga.si(c4, "bank", 0)
  +c6 = ga.si(c5, "lift", 0)
  +c7 = ga.si(c6, "back", 0)
  +c8 = ga.si(c7, "ds", griffinwing.types.PACE())
  +c9 = ga.si(c8, "rx", 0)
  +c10 = ga.si(c9, "rz", 0)
  +c11 = ga.si(c10, "shield", griffinwing.types.SHIELD())
  +c12 = ga.su(c11, "hurt", 0)
  +c13 = ga.su(c12, "down", 0)
  +c14 = ga.su(c13, "roll", 0)
  +c15 = ga.su(c14, "roll_len", 0)
  +c16 = ga.si(c15, "dir", 1)
  +c17 = ga.su(c16, "rest", 0)
  +c18 = ga.su(c17, "held", 0)
  +c19 = ga.su(c18, "charge", 0)
  +c20 = ga.su(c19, "cool", 0)
  +c21 = ga.sv(c20, "locks", J.JA{[]})
  +c22 = ga.su(c21, "dwell", 0)
  +c23 = ga.si(c22, "meter", griffinwing.types.METER())
  +c24 = ga.sb(c23, "spent", False{})
  +c25 = ga.sb(c24, "boost", False{})
  +c26 = ga.sb(c25, "brake", False{})
  +c27 = ga.su(c26, "bombs", 3)
  +c28 = ga.su(c27, "keys", 0)
  +c29 = ga.si(c28, "tap_dir", 0)
  +c30 = ga.su(c29, "tap_ticks", 0)
  +c31 = ga.si(c30, "last_x", 0)
  +c32 = ga.su(c31, "chain", 0)
  +c33 = ga.su(c32, "chain_ticks", 0)
  +c34 = ga.su(c33, "pts", 0)
  +c35 = ga.su(c34, "hits", 0)
  +c36 = ga.su(c35, "saved", 0)
  ga.su(c36, "gates", 0)

def griffinwing.types.Craft.flying(+c: J.Val) -> Bool:
  U32.is_zero(ga.u(c, "down"))

# The sum of the locks' counts.
def griffinwing.types.Craft.lock_total(+ls: List<&2, J.Val>) -> U32:
  match ls:
    case Nil{}:
      0
    case Con{+l, +t}:
      (ga.u(l, "n") + griffinwing.types.Craft.lock_total(t) : U32)

def griffinwing.types.Craft.valid(+c: J.Val) -> Bool:
  gw.alls([
    gw.far(ga.u(c, "x"), griffinwing.types.dm(160)),
    gw.far(ga.u(c, "z"), griffinwing.types.dm(120)),
    gw.far(ga.u(c, "vx"), 1024),
    gw.far(ga.u(c, "vz"), 1024),
    gw.far(ga.u(c, "bank"), 512),
    gw.far(ga.u(c, "lift"), 512),
    gw.far(ga.u(c, "back"), griffinwing.types.dm(80)),
    gw.within(ga.u(c, "ds"), 0, 1024),
    gw.far(ga.u(c, "rx"), griffinwing.types.dm(200)),
    gw.far(ga.u(c, "rz"), griffinwing.types.dm(200)),
    gw.within(ga.u(c, "shield"), 0, griffinwing.types.SHIELD()),
    U32.is_le(ga.u(c, "hurt"), 600),
    U32.is_le(ga.u(c, "down"), 600),
    U32.is_le(ga.u(c, "roll"), 120),
    U32.is_le(ga.u(c, "roll_len"), 120),
    Bool.or(U32.is_eq(ga.u(c, "dir"), 1), U32.is_eq(ga.u(c, "dir"), 4294967295)),
    U32.is_le(ga.u(c, "rest"), 240),
    U32.is_le(ga.u(c, "held"), 36000),
    U32.is_le(ga.u(c, "charge"), 240),
    U32.is_le(ga.u(c, "cool"), 240),
    U32.is_le(gw.len(c, "locks"), griffinwing.types.MAX_LOCKS()),
    U32.is_le(griffinwing.types.Craft.lock_total(ga.items(c, "locks")), griffinwing.types.MAX_LOCKS()),
    U32.is_le(ga.u(c, "dwell"), 600),
    gw.within(ga.u(c, "meter"), 0, griffinwing.types.METER()),
    U32.is_le(ga.u(c, "bombs"), 9),
    gw.within(ga.u(c, "tap_dir"), 4294967295, 1),
    gw.within(ga.u(c, "last_x"), 4294967295, 1),
    U32.is_le(ga.u(c, "chain_ticks"), 600)])

# Whether its roll turns bolts round just now.
def griffinwing.flight.Craft.reflecting(+c: J.Val, +reflect: U32) -> Bool:
  Bool.and(U32.is_gt(ga.u(c, "roll"), 0), U32.is_lt((ga.u(c, "roll_len") - ga.u(c, "roll") : U32), reflect))

# Beast: the Wyrm's state, valid.
def griffinwing.types.Beast.marks_ok(+ms: List<&2, J.Val>) -> Bool:
  match ms:
    case Nil{}:
      True{}
    case Con{+m, +t}:
      Bool.and(
        Bool.and(gw.far(ga.nu(m, 0), griffinwing.types.dm(300)), gw.far(ga.nu(m, 1), griffinwing.types.dm(300))),
        griffinwing.types.Beast.marks_ok(t))

def griffinwing.types.Beast.valid(+b: J.Val) -> Bool:
  gw.alls([
    U32.is_le(ga.u(b, "phase"), 2),
    U32.is_le(ga.u(b, "atk"), 2),
    U32.is_le(ga.u(b, "ticks"), 3000),
    gw.far(ga.u(b, "x"), griffinwing.types.dm(200)),
    gw.far(ga.u(b, "z"), griffinwing.types.dm(200)),
    U32.is_le(gw.len(b, "marks"), 8),
    griffinwing.types.Beast.marks_ok(ga.items(b, "marks")),
    gw.far(ga.u(b, "gap"), griffinwing.types.dm(200)),
    gw.within(ga.u(b, "side"), 4294967295, 1),
    gw.far(ga.u(b, "col"), griffinwing.types.dm(200))])

# Tally: what the party has banked, and what the beat in hand has made.
def griffinwing.types.Tally.default() -> J.Val:
  +t0 = J.obj()
  +t1 = ga.su(t0, "pts", 0)
  +t2 = ga.su(t1, "base", 0)
  +t3 = ga.su(t2, "offered", 0)
  +t4 = ga.su(t3, "kills", 0)
  +t5 = ga.su(t4, "saved", 0)
  +t6 = ga.su(t5, "lost", 0)
  ga.su(t6, "gates", 0)

def griffinwing.types.Tally.valid(+t: J.Val) -> Bool:
  gw.alls([
    U32.is_le(ga.u(t, "pts"), 1000000),
    U32.is_le(ga.u(t, "base"), 1000000),
    U32.is_le(ga.u(t, "offered"), 1000000),
    U32.is_le(ga.u(t, "kills"), 10000),
    U32.is_le(ga.u(t, "saved"), 1000),
    U32.is_le(ga.u(t, "lost"), 1000),
    U32.is_le(ga.u(t, "gates"), 1000)])

# `self.bank(beat)`: add the beat's tally in (u16 fields wrap).
def griffinwing.types.Tally.bank(+t: J.Val, +beat: J.Val) -> J.Val:
  +u0 = ga.su(t, "pts", (ga.u(t, "pts") + ga.u(beat, "pts") : U32))
  +u1 = ga.su(u0, "base", (ga.u(t, "base") + ga.u(beat, "base") : U32))
  +u2 = ga.su(u1, "offered", (ga.u(t, "offered") + ga.u(beat, "offered") : U32))
  +u3 = ga.su(u2, "kills", (ga.u(t, "kills") + ga.u(beat, "kills") : U32))
  +u4 = ga.su(u3, "saved", RS.as_u16((ga.u(t, "saved") + ga.u(beat, "saved") : U32)))
  +u5 = ga.su(u4, "lost", RS.as_u16((ga.u(t, "lost") + ga.u(beat, "lost") : U32)))
  ga.su(u5, "gates", RS.as_u16((ga.u(t, "gates") + ga.u(beat, "gates") : U32)))

# How many gold pieces the purse adds for the medal.
def griffinwing.types.Medal.rank(+m: String) -> U32:
  Bool.pick(U32, String.eq(m, "Bronze"), 1, Bool.pick(U32, String.eq(m, "Silver"), 2, 3))

def griffinwing.types.Medal.name(+m: String) -> String:
  gw.ifs(String.eq(m, "Bronze"), _ => "BRONZE", _ => gw.ifs(String.eq(m, "Silver"), _ => "SILVER", _ => "GOLD"))

# ─── the little arithmetic the sim shares ───────────────────────────────────

# An i32 sign-extended to an i64.
def gw.i64(+x: U32) -> W.W:
  Bool.pick(W.W, RS.i_is_neg(x), W.of_parts(4294967295, x), W.of_u32(x))

def gw.w_is_neg(+a: W.W) -> Bool:
  U32.is_ge(W.hi32(a), 2147483648)

def gw.w_abs(+a: W.W) -> W.W:
  Bool.pick(W.W, gw.w_is_neg(a), W.neg(a), a)

def gw.w_signed_lt(+a: W.W, +b: W.W) -> Bool:
  W.lt(W.xor(a, W.of_parts(2147483648, 0)), W.xor(b, W.of_parts(2147483648, 0)))

# Truncating i64 division (the quotient's sign is the product of the signs).
def gw.w_sdiv(+a: W.W, +b: W.W) -> W.W:
  Bool.pick(W.W, Bool.xor(gw.w_is_neg(a), gw.w_is_neg(b)),
    W.neg(W.div(gw.w_abs(a), gw.w_abs(b))), W.div(gw.w_abs(a), gw.w_abs(b)))

# floor(sqrt(n)) of a u64 (exact; Rust's f64 sqrt agrees below 2^52).
def gw.w_isqrt.go(+n: Nat, +a: W.W, +res: W.W) -> W.W:
  match n:
    case 0n:
      res
    case 1n+p:
      +cand = W.or(res, W.shl(W.one(), U32.from_nat(p)))
      gw.w_isqrt.go(p, a, Bool.pick(W.W, W.le(W.mul(cand, cand), a), cand, res))

def gw.w_isqrt(+a: W.W) -> U32:
  W.lo32(gw.w_isqrt.go(32n, a, W.zero()))

def gw.sq64(+v: U32) -> W.W:
  W.mul(gw.i64(v), gw.i64(v))

def griffinwing.types.norm.wide(+x: U32, +y: U32, +z: U32) -> U32:
  gw.w_isqrt(W.add(W.add(gw.sq64(x), gw.sq64(y)), gw.sq64(z)))

def griffinwing.types.norm.small(+x: U32, +y: U32, +z: U32) -> U32:
  RS.u_isqrt(((x * x : U32) + (y * y : U32) + (z * z : U32) : U32))

# A whole-number length of the vector, rounded down (an i64 that always fits a
# U32: sqrt of a non-negative i64).
def griffinwing.types.norm(+x: U32, +y: U32, +z: U32) -> U32:
  gw.ifu(Bool.and(U32.is_lt(RS.i_abs(x), 32768), Bool.and(U32.is_lt(RS.i_abs(y), 32768), U32.is_lt(RS.i_abs(z), 32768))),
    _ => griffinwing.types.norm.small(x, y, z),
    _ => griffinwing.types.norm.wide(x, y, z))

# `(a as i64 * b as i64 / m) as i32` for a, b i32 and a positive m (a U32):
# the quotient truncates toward zero.
def gw.muldiv.sign(+neg: Bool, +q: U32) -> U32:
  Bool.pick(U32, neg, (0 - q : U32), q)

def gw.muldiv.big(+neg: Bool, +a: U32, +b: U32, +m: U32) -> U32:
  gw.muldiv.sign(neg, W.lo32(W.div(W.mul(W.of_u32(a), W.of_u32(b)), W.of_u32(m))))

def gw.muldiv(+a: U32, +b: U32, +m: U32) -> U32:
  gw.ifu(Bool.and(U32.is_lt(RS.i_abs(a), 65536), U32.is_lt(RS.i_abs(b), 65536)),
    _ => gw.muldiv.sign(Bool.xor(RS.i_is_neg(a), RS.i_is_neg(b)), U32.div((RS.i_abs(a) * RS.i_abs(b) : U32), m)),
    _ => gw.muldiv.big(Bool.xor(RS.i_is_neg(a), RS.i_is_neg(b)), RS.i_abs(a), RS.i_abs(b), m))

# The three parts of a vector.
type GwV3 is Data:
  GwV3{a: U32, b: U32, c: U32}

def gw.v3a(+v: GwV3) -> U32:
  match v:
    case GwV3{+a, +b, +c}:
      a

def gw.v3b(+v: GwV3) -> U32:
  match v:
    case GwV3{+a, +b, +c}:
      b

def gw.v3c(+v: GwV3) -> U32:
  match v:
    case GwV3{+a, +b, +c}:
      c

def gw.v3j(+v: GwV3) -> J.Val:
  gw.tri3(gw.v3a(v), gw.v3b(v), gw.v3c(v))

# `d` turned to a length of `len`, along the same way (straight ahead for a
# zero vector).
def griffinwing.types.scale3(+x: U32, +y: U32, +z: U32, +len: U32) -> GwV3:
  griffinwing.types.scale3.m(x, y, z, len, griffinwing.types.norm(x, y, z))

def griffinwing.types.scale3.m(+x: U32, +y: U32, +z: U32, +len: U32, +m: U32) -> GwV3:
  match m:
    case 0:
      GwV3{len, 0, 0}
    case _:
      GwV3{gw.muldiv(x, len, m), gw.muldiv(y, len, m), gw.muldiv(z, len, m)}

# A triangle wave through `-amp..=amp` over `period` ticks (32-bit
# intermediates: the sim's periods are a few hundred and its amps a few
# thousand).
def griffinwing.types.tri(+t: U32, +period: U32, +amp: U32) -> U32:
  griffinwing.types.tri.go(t, U32.max(period, 2), amp)

def griffinwing.types.tri.go(+t: U32, +p: U32, +amp: U32) -> U32:
  griffinwing.types.tri.up(U32.mod(t, p), p, U32.max(U32.div(p, 2), 1), amp)

def griffinwing.types.tri.up(+k: U32, +p: U32, +half: U32, +amp: U32) -> U32:
  gw.muldiv(((Bool.pick(U32, U32.is_lt(k, half), k, (p - k : U32)) * 2 : U32) - half : U32), amp, half)

# Whether the segment from `a` to `b` passes within `r` of `p`: the swept test
# every bolt, dive and latch is judged by. A cheap box test first: the nearest
# point on the segment lies inside its box, so a body farther than `r` from the
# box on any axis is out of reach.
def gw.gap(+p: U32, +a: U32, +b: U32) -> U32:
  gw.gap.go(p, RS.i_min(a, b), RS.i_max(a, b))

def gw.gap.go(+p: U32, +lo: U32, +hi: U32) -> U32:
  Bool.pick(U32, RS.i_lt(p, lo), (lo - p : U32), Bool.pick(U32, RS.i_gt(p, hi), (p - hi : U32), 0))

def griffinwing.types.near(+ax: U32, +ay: U32, +az: U32, +bx: U32, +by: U32, +bz: U32, +px: U32, +py: U32, +pz: U32, +r: U32) -> Bool:
  gw.ifb(Bool.or(U32.is_gt(gw.gap(px, ax, bx), RS.i_abs(r)),
      Bool.or(U32.is_gt(gw.gap(py, ay, by), RS.i_abs(r)), U32.is_gt(gw.gap(pz, az, bz), RS.i_abs(r)))),
    _ => False{},
    _ => griffinwing.types.near.full(ax, ay, az, bx, by, bz, px, py, pz, r))

def griffinwing.types.near.full(+ax: U32, +ay: U32, +az: U32, +bx: U32, +by: U32, +bz: U32, +px: U32, +py: U32, +pz: U32, +r: U32) -> Bool:
  griffinwing.types.near.d(
    W.sub(gw.i64(bx), gw.i64(ax)), W.sub(gw.i64(by), gw.i64(ay)), W.sub(gw.i64(bz), gw.i64(az)),
    W.sub(gw.i64(px), gw.i64(ax)), W.sub(gw.i64(py), gw.i64(ay)), W.sub(gw.i64(pz), gw.i64(az)), r)

def griffinwing.types.near.d(+dx: W.W, +dy: W.W, +dz: W.W, +wx: W.W, +wy: W.W, +wz: W.W, +r: U32) -> Bool:
  griffinwing.types.near.t(dx, dy, dz, wx, wy, wz, r,
    W.add(W.add(W.mul(dx, dx), W.mul(dy, dy)), W.mul(dz, dz)),
    W.add(W.add(W.mul(wx, dx), W.mul(wy, dy)), W.mul(wz, dz)))

# t = (w . d).clamp(0, len2), or 0 for a point.
def griffinwing.types.near.t(+dx: W.W, +dy: W.W, +dz: W.W, +wx: W.W, +wy: W.W, +wz: W.W, +r: U32, +len2: W.W, +dot: W.W) -> Bool:
  griffinwing.types.near.e(dx, dy, dz, wx, wy, wz, r, len2,
    Bool.pick(W.W, W.is_zero(len2), W.zero(),
      Bool.pick(W.W, gw.w_is_neg(dot), W.zero(), W.min(dot, len2))))

def griffinwing.types.near.e(+dx: W.W, +dy: W.W, +dz: W.W, +wx: W.W, +wy: W.W, +wz: W.W, +r: U32, +len2: W.W, +t: W.W) -> Bool:
  griffinwing.types.near.cmp(
    griffinwing.types.near.c(dx, wx, len2, t),
    griffinwing.types.near.c(dy, wy, len2, t),
    griffinwing.types.near.c(dz, wz, len2, t), r)

# c(dv, wv) = if len2 == 0 { wv } else { wv - dv * t / len2 }
def griffinwing.types.near.c(+dv: W.W, +wv: W.W, +len2: W.W, +t: W.W) -> W.W:
  Bool.pick(W.W, W.is_zero(len2), wv, W.sub(wv, gw.w_sdiv(W.mul(dv, t), len2)))

def griffinwing.types.near.cmp(+ex: W.W, +ey: W.W, +ez: W.W, +r: U32) -> Bool:
  W.le(W.add(W.add(W.mul(ex, ex), W.mul(ey, ey)), W.mul(ez, ez)), W.mul(gw.i64(r), gw.i64(r)))

# How far two points are apart across and up (the plane's own distance).
def griffinwing.types.flat(+ax: U32, +az: U32, +bx: U32, +bz: U32) -> U32:
  griffinwing.types.norm(0, (ax - bx : U32), (az - bz : U32))
