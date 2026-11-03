# A foe's footprint: where its blow lands. A capsule is a line with a width, a
# circle, a fan or a ring is what its name says; each answers whether a body of
# some radius stands in it. crates/delve/src/rules/lanternward/foes.rs `Danger`.
# The capsule's distance is the one place the arithmetic needs more than a
# word: a segment's squared distance is a 64-bit number.
import Base
import ./n.bend as N
import ./g.bend as G
import ./m.bend as M

# Whether a 64-bit two's-complement number is above zero.
def pos(+a: N.U64) -> Bool:
  match a:
    case N.U64{hi, lo}:
      U32.is_eq(U32.shrn(hi, 31n), 0) && (U32.is_ne(hi, 0) || U32.is_ne(lo, 0))

# a * b as a signed 64-bit number, both signed words.
def smul(+a: U32, +b: U32) -> N.U64:
  N.u64_cneg(N.mul32(N.sabs(a), N.sabs(b)), N.mask(U32.xor(a, b)))

def shl10(+a: N.U64) -> N.U64:
  match a:
    case N.U64{hi, lo}:
      N.U64{U32.or(U32.shln(hi, 10n), U32.shrn(lo, 22n)), U32.shln(lo, 10n)}

# floor(a / b) when the answer is under 1024, by ten steps of long division.
def divq(+k: Nat, +a: N.U64, +b: N.U64, +q: U32) -> U32:
  match k:
    case 0n:
      q
    case 1n+p:
      +ge = N.u64_ge(a, b)
      divq(p, Bool.pick(N.U64, ge, N.u64_sub(a, b), a), N.u64_shr1(b), U32.or(U32.shl(q), N.flag(ge)))

# How far along the segment a to b the point p is nearest, in 1024ths: zero
# before a, 1024 past b.
def along(+px: U32, +py: U32, +ax: U32, +ay: U32, +dx: U32, +dy: U32) -> U32:
  +len2 = N.u64_add(N.sq(dx), N.sq(dy))
  +dot = N.u64_add(smul(U32.sub(px, ax), dx), smul(U32.sub(py, ay), dy))
  Bool.pick(U32, Bool.not(pos(len2)) || Bool.not(pos(dot)), 0,
    Bool.pick(U32, N.u64_ge(dot, len2), 1024, divq(10n, shl10(dot), N.u64_shl1(N.u64_shl1(N.u64_shl1(N.u64_shl1(N.u64_shl1(N.u64_shl1(N.u64_shl1(N.u64_shl1(N.u64_shl1(len2))))))))), 0)))

# The squared distance from p to the segment a to b.
def seg_dist2(+px: U32, +py: U32, +ax: U32, +ay: U32, +bx: U32, +by: U32) -> N.U64:
  +dx = U32.sub(bx, ax)
  +dy = U32.sub(by, ay)
  +t = along(px, py, ax, ay, dx, dy)
  +cx = U32.add(ax, N.sdiv(U32.mul(dx, t), 1024))
  +cy = U32.add(ay, N.sdiv(U32.mul(dy, t), 1024))
  N.u64_add(N.sq(U32.sub(px, cx)), N.sq(U32.sub(py, cy)))

def capsule(+ax: U32, +ay: U32, +bx: U32, +by: U32, +r: U32, +x: U32, +y: U32, +rad: U32) -> Bool:
  N.u64_ge(N.sq(U32.add(r, rad)), seg_dist2(x, y, ax, ay, bx, by))

def circle(+cx: U32, +cy: U32, +r: U32, +x: U32, +y: U32, +rad: U32) -> Bool:
  N.sle(G.dist(U32.sub(x, cx), U32.sub(y, cy)), U32.add(r, rad))

def fan(+cx: U32, +cy: U32, +deg: U32, +half: U32, +r: U32, +x: U32, +y: U32, +rad: U32) -> Bool:
  +d = G.dist(U32.sub(x, cx), U32.sub(y, cy))
  +u = G.unit(U32.sub(x, cx), U32.sub(y, cy))
  N.sle(d, U32.add(r, rad)) && (N.slt(d, rad) || N.sge(G.dot(G.vx(u), G.vy(u), G.cos_q(deg), G.sin_q(deg)), G.cos_q(half)))

def ring(+cx: U32, +cy: U32, +r: U32, +w: U32, +x: U32, +y: U32, +rad: U32) -> Bool:
  N.sle(N.sabs(U32.sub(G.dist(U32.sub(x, cx), U32.sub(y, cy)), r)), U32.add(w, rad))

# Whether a body of radius `rad` at (x, y) stands in the footprint: its shape
# (0 none, 1 capsule, 2 circle, 3 fan, 4 ring) and its five words.
def hits(+dk: U32, +d0: U32, +d1: U32, +d2: U32, +d3: U32, +d4: U32, +x: U32, +y: U32, +rad: U32) -> Bool:
  Bool.pick(Bool, U32.is_eq(dk, 1), capsule(d0, d1, d2, d3, d4, x, y, rad),
    Bool.pick(Bool, U32.is_eq(dk, 2), circle(d0, d1, d2, x, y, rad),
      Bool.pick(Bool, U32.is_eq(dk, 3), fan(d0, d1, d2, d3, d4, x, y, rad),
        Bool.pick(Bool, U32.is_eq(dk, 4), ring(d0, d1, d2, d3, x, y, rad), False{}))))
