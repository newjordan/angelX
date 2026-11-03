# Griffinwing's rules, in Bend: Moonfall Pass, the on-rails co-op flight shooter of the Delve
# (crates/delve/src/rules/griffinwing keeps the session's types, its saves and its wire).
#
# One call per game tick. The session comes in as a flat frame of 32-bit words (the layout section
# below is the table of where each field lives; crates/delve/src/rules/griffinwing/words.rs is the
# other side of it), `index.js` lays the words into `Mem`, and the next session goes back out the
# same way with the tick's sounds and who left the tally. Every number is a whole number on the
# 30 Hz sim, 256 to the unit, so nothing here is a float: a signed value is its two's-complement
# word (+ - * need no care; the comparisons, the division and the clamps are the signed ones in
# the numbers section), and what is wider than a word (a squared length, a product) goes through
# exact Nat arithmetic. A second op, `bot`, answers what a scripted knight presses.
#
# `Mem` is a persistent binary trie of 2^13 words, so a read costs no threading and a write is a
# new `Mem`. Bend matches only on parameters, so every `if` is a helper that matches a Bool and
# every loop counts a Nat down.
import Base


# ===== numbers =====

def ng(+a: U32) -> U32: U32.sub(0, a)

def neg_p(+a: U32) -> Bool: U32.is_ge(a, 2147483648)
def sx(+a: U32) -> U32: U32.xor(a, 2147483648)
def lt(+a: U32, +b: U32) -> Bool: U32.is_lt(sx(a), sx(b))
def le(+a: U32, +b: U32) -> Bool: U32.is_le(sx(a), sx(b))
def gt(+a: U32, +b: U32) -> Bool: U32.is_gt(sx(a), sx(b))
def ge(+a: U32, +b: U32) -> Bool: U32.is_ge(sx(a), sx(b))
def eq(+a: U32, +b: U32) -> Bool: U32.is_eq(a, b)
def ne(+a: U32, +b: U32) -> Bool: U32.is_ne(a, b)
def z(+a: U32) -> Bool: U32.is_eq(a, 0)
def nz(+a: U32) -> Bool: U32.is_ne(a, 0)
def pk(+c: Bool, +a: U32, +b: U32) -> U32: Bool.pick(U32, c, a, b)
def mn(+a: U32, +b: U32) -> U32: pk(lt(a, b), a, b)
def mx(+a: U32, +b: U32) -> U32: pk(gt(a, b), a, b)
def cl(+x: U32, +lo: U32, +hi: U32) -> U32: mn(mx(x, lo), hi)
def ab(+a: U32) -> U32: pk(neg_p(a), ng(a), a)
def sg(+a: U32) -> U32: pk(neg_p(a), 4294967295, pk(z(a), 0, 1))
def pb(+c: Bool, +a: Bool, +b: Bool) -> Bool: Bool.pick(Bool, c, a, b)
def b2u(+c: Bool) -> U32: pk(c, 1, 0)

# unsigned saturating subtraction, min and max
def ssub(+a: U32, +b: U32) -> U32: pk(U32.is_ge(a, b), U32.sub(a, b), 0)

def umn(+a: U32, +b: U32) -> U32: pk(U32.is_lt(a, b), a, b)
def umx(+a: U32, +b: U32) -> U32: pk(U32.is_gt(a, b), a, b)

# a / b and a % b truncated toward zero, like Rust's on i32
def sdv(+a: U32, +b: U32) -> U32:
  +q = U32.div(ab(a), ab(b))
  pk(neg_p(U32.xor(a, b)), ng(q), q)

# n tenths of a unit as fixed point, and its negative
def dm(+n: U32) -> U32: sdv(U32.mul(n, 256), 10)

def dn(+n: U32) -> U32: ng(dm(n))

# ----- exact wide arithmetic on Nat (a JS integer, exact to 2^48) -----

def nt(+a: U32) -> Nat: U32.to_nat(a)

def un(+n: Nat) -> U32: U32.from_nat(n)

def sq(+a: U32) -> Nat:
  +n = nt(ab(a))
  Nat.mul(n, n)

# floor(sqrt(n)): a float guess, then exact integer steps up and down
def isq_up(+k: Nat, +g: Nat, +n: Nat) -> Nat:
  match k:
    case 0n:
      g
    case 1n+p:
      +h = Nat.add(g, 1n)
      isq_up(p, Bool.pick(Nat, Nat.is_le(Nat.mul(h, h), n), h, g), n)

def isq_down(+k: Nat, +g: Nat, +n: Nat) -> Nat:
  match k:
    case 0n:
      g
    case 1n+p:
      isq_down(p, Bool.pick(Nat, Nat.is_gt(Nat.mul(g, g), n), Nat.sub(g, 1n), g), n)

def isqrt(+n: Nat) -> Nat:
  +w = Nat.mul(65536n, 65536n)
  +hi = un(Nat.div(n, w))
  +lo = un(Nat.mod(n, w))
  +f = (U32.to_f32(hi) * 4294967296.0 + U32.to_f32(lo) : F32)
  +g = F32.to_nat(F32.sqrt(f))
  isq_down(4n, isq_up(4n, g, n), n)

# the length of (a, b, c), rounded down
def norm3(+a: U32, +b: U32, +c: U32) -> U32: un(isqrt(Nat.add(Nat.add(sq(a), sq(b)), sq(c))))

def flat(+ax: U32, +az: U32, +bx: U32, +bz: U32) -> U32: norm3(0, U32.sub(ax, bx), U32.sub(az, bz))

# v * len / m toward zero, for m > 0
def sc1(+v: U32, +len: U32, +m: U32) -> U32:
  +q = un(Nat.div(Nat.mul(nt(ab(v)), nt(ab(len))), nt(m)))
  pk(neg_p(U32.xor(v, len)), ng(q), q)

# a vector scaled to length `len`: its three parts, straight ahead for a zero vector
def scs(+v: U32, +len: U32, +m: U32) -> U32: pk(z(m), len, sc1(v, len, m))

def scx(+v: U32, +len: U32, +m: U32) -> U32: pk(z(m), 0, sc1(v, len, m))

# the signed product a*b, as its positive and its negative magnitude
def ppart(+a: U32, +b: U32) -> Nat: Bool.pick(Nat, neg_p(U32.xor(a, b)), 0n, Nat.mul(nt(ab(a)), nt(ab(b))))

def npart(+a: U32, +b: U32) -> Nat: Bool.pick(Nat, neg_p(U32.xor(a, b)), Nat.mul(nt(ab(a)), nt(ab(b))), 0n)

# the square of how far point part wv lies from the segment part dv * t / l2
def off(+dv: U32, +wv: U32, +t: Nat, +l2: Nat) -> Nat:
  +q = un(Nat.div(Nat.mul(nt(ab(dv)), t), l2))
  sq(U32.sub(wv, pk(neg_p(dv), ng(q), q)))

# ===== memory: the frame as a persistent quad trie of 2^14 words =====

type Mem is Data:
  Z{}
  Leaf{v: U32}
  Node{a: Mem, b: Mem, c: Mem, d: Mem}

# the index is held MSB-aligned: its top two bits choose the branch at this level, and the two flags
# carry them (top bit, second bit) so that a level is one match and one tail call
def rd_go(+d: Nat, +m: Mem, +i: U32, +h: Bool, +l: Bool) -> U32:
  match d m h l:
    case _ Z{} _ _:
      0
    case _ Leaf{v} _ _:
      v
    case 0n Node{_, _, _, _} _ _:
      0
    case 1n+p Node{a, _, _, _} False{} False{}:
      +j = U32.shln(i, 2n)
      rd_go(p, a, j, U32.is_ge(j, 2147483648), U32.is_ge(U32.and(j, 1073741824), 1073741824))
    case 1n+p Node{_, b, _, _} False{} True{}:
      +j = U32.shln(i, 2n)
      rd_go(p, b, j, U32.is_ge(j, 2147483648), U32.is_ge(U32.and(j, 1073741824), 1073741824))
    case 1n+p Node{_, _, c, _} True{} False{}:
      +j = U32.shln(i, 2n)
      rd_go(p, c, j, U32.is_ge(j, 2147483648), U32.is_ge(U32.and(j, 1073741824), 1073741824))
    case 1n+p Node{_, _, _, e} True{} True{}:
      +j = U32.shln(i, 2n)
      rd_go(p, e, j, U32.is_ge(j, 2147483648), U32.is_ge(U32.and(j, 1073741824), 1073741824))

def rd(+m: Mem, +i: U32) -> U32:
  +j = U32.shln(i, 18n)
  rd_go(7n, m, j, U32.is_ge(j, 2147483648), U32.is_ge(U32.and(j, 1073741824), 1073741824))

def g_num_window(+m: Mem) -> U32: rd(m, 41)

# the steer box's half width and half height
def wx(+m: Mem) -> U32: dm(g_num_window(m))

def wz(+m: Mem) -> U32: sdv(U32.mul(dm(g_num_window(m)), 5), 8)

def near_dw(+dx: U32, +dy: U32, +dz: U32, +wx: U32, +wy: U32, +wz: U32, +r: U32) -> Bool:
  +l2 = Nat.add(Nat.add(sq(dx), sq(dy)), sq(dz))
  +pos = Nat.add(Nat.add(ppart(wx, dx), ppart(wy, dy)), ppart(wz, dz))
  +neg = Nat.add(Nat.add(npart(wx, dx), npart(wy, dy)), npart(wz, dz))
  +t = Nat.min(Nat.sub(pos, neg), l2)
  +e = Nat.add(Nat.add(off(dx, wx, t, l2), off(dy, wy, t, l2)), off(dz, wz, t, l2))
  Nat.is_le(e, Nat.mul(nt(r), nt(r)))

# whether the segment a to b passes within r of p
def near(+ax: U32, +ay: U32, +az: U32, +bx: U32, +by: U32, +bz: U32, +px: U32, +py: U32, +pz: U32, +r: U32) -> Bool:
  near_dw(U32.sub(bx, ax), U32.sub(by, ay), U32.sub(bz, az), U32.sub(px, ax), U32.sub(py, ay), U32.sub(pz, az), r)

# a triangle wave through -amp..=amp over `period` ticks
def tri(+t: U32, +period: U32, +amp: U32) -> U32:
  +p = umx(period, 2)
  +k = U32.mod(t, p)
  +half = umx(U32.div(p, 2), 1)
  +up = pk(U32.is_lt(k, half), k, U32.sub(p, k))
  sdv(U32.mul(U32.sub(U32.mul(up, 2), half), amp), half)

def wr_go(+d: Nat, +m: Mem, +i: U32, +v: U32, +h: Bool, +l: Bool) -> Mem:
  match d m h l:
    case 0n _ _ _:
      Leaf{v}
    case 1n+p Node{a, b, c, e} False{} False{}:
      +j = U32.shln(i, 2n)
      Node{wr_go(p, a, j, v, U32.is_ge(j, 2147483648), U32.is_ge(U32.and(j, 1073741824), 1073741824)), b, c, e}
    case 1n+p Node{a, b, c, e} False{} True{}:
      +j = U32.shln(i, 2n)
      Node{a, wr_go(p, b, j, v, U32.is_ge(j, 2147483648), U32.is_ge(U32.and(j, 1073741824), 1073741824)), c, e}
    case 1n+p Node{a, b, c, e} True{} False{}:
      +j = U32.shln(i, 2n)
      Node{a, b, wr_go(p, c, j, v, U32.is_ge(j, 2147483648), U32.is_ge(U32.and(j, 1073741824), 1073741824)), e}
    case 1n+p Node{a, b, c, e} True{} True{}:
      +j = U32.shln(i, 2n)
      Node{a, b, c, wr_go(p, e, j, v, U32.is_ge(j, 2147483648), U32.is_ge(U32.and(j, 1073741824), 1073741824))}
    case 1n+p _ False{} False{}:
      +j = U32.shln(i, 2n)
      Node{wr_go(p, Z{}, j, v, U32.is_ge(j, 2147483648), U32.is_ge(U32.and(j, 1073741824), 1073741824)), Z{}, Z{}, Z{}}
    case 1n+p _ False{} True{}:
      +j = U32.shln(i, 2n)
      Node{Z{}, wr_go(p, Z{}, j, v, U32.is_ge(j, 2147483648), U32.is_ge(U32.and(j, 1073741824), 1073741824)), Z{}, Z{}}
    case 1n+p _ True{} False{}:
      +j = U32.shln(i, 2n)
      Node{Z{}, Z{}, wr_go(p, Z{}, j, v, U32.is_ge(j, 2147483648), U32.is_ge(U32.and(j, 1073741824), 1073741824)), Z{}}
    case 1n+p _ True{} True{}:
      +j = U32.shln(i, 2n)
      Node{Z{}, Z{}, Z{}, wr_go(p, Z{}, j, v, U32.is_ge(j, 2147483648), U32.is_ge(U32.and(j, 1073741824), 1073741824))}

def wr(+m: Mem, +i: U32, +v: U32) -> Mem:
  +j = U32.shln(i, 18n)
  wr_go(7n, m, j, v, U32.is_ge(j, 2147483648), U32.is_ge(U32.and(j, 1073741824), 1073741824))

# copy n words from `from` to `to`, first word first: for a move toward lower addresses
def cpw(+n: Nat, +m: Mem, +from: U32, +to: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      cpw(p, wr(m, to, rd(m, from)), U32.add(from, 1), U32.add(to, 1))

def pkm(+c: Bool, +a: Mem, +b: Mem) -> Mem: Bool.pick(Mem, c, a, b)

# ===== layout: where each field lives in the word frame (`index.js` lays the session out in this order) =====
def SEATS() -> U32: 128

def SEAT_W() -> U32: 51
def WINGS() -> U32: 434
def WING_W() -> U32: 52
def FOES() -> U32: 642
def FOE_W() -> U32: 19
def BOLTS() -> U32: 1402
def BOLT_W() -> U32: 14
def PROPS() -> U32: 3642
def PROP_W() -> U32: 9
def CALLS() -> U32: 4002
def FXS() -> U32: 4014
def FX_W() -> U32: 6
def BOSSB() -> U32: 4302
def SCRATCH() -> U32: 4336
def g_elapsed(+m: Mem) -> U32: rd(m, 0)
def set_g_elapsed(+m: Mem, +v: U32) -> Mem: wr(m, 0, v)
def g_mode(+m: Mem) -> U32: rd(m, 1)
def set_g_mode(+m: Mem, +v: U32) -> Mem: wr(m, 1, v)
def g_mode_t(+m: Mem) -> U32: rd(m, 2)
def set_g_mode_t(+m: Mem, +v: U32) -> Mem: wr(m, 2, v)
def g_stage(+m: Mem) -> U32: rd(m, 3)
def set_g_stage(+m: Mem, +v: U32) -> Mem: wr(m, 3, v)
def g_t(+m: Mem) -> U32: rd(m, 4)
def set_g_t(+m: Mem, +v: U32) -> Mem: wr(m, 4, v)
def g_cp(+m: Mem) -> U32: rd(m, 5)
def set_g_cp(+m: Mem, +v: U32) -> Mem: wr(m, 5, v)
def g_next(+m: Mem) -> U32: rd(m, 6)
def set_g_next(+m: Mem, +v: U32) -> Mem: wr(m, 6, v)
def g_ids(+m: Mem) -> U32: rd(m, 7)
def set_g_ids(+m: Mem, +v: U32) -> Mem: wr(m, 7, v)
def g_retries(+m: Mem) -> U32: rd(m, 8)
def set_g_retries(+m: Mem, +v: U32) -> Mem: wr(m, 8, v)
def set_g_medal(+m: Mem, +v: U32) -> Mem: wr(m, 9, v)
def set_g_done_some(+m: Mem, +v: U32) -> Mem: wr(m, 12, v)
def set_g_done(+m: Mem, +v: U32) -> Mem: wr(m, 13, v)
def set_g_home(+m: Mem, +v: U32) -> Mem: wr(m, 14, v)
def g_bank_pts(+m: Mem) -> U32: rd(m, 16)
def set_g_bank_pts(+m: Mem, +v: U32) -> Mem: wr(m, 16, v)
def g_bank_base(+m: Mem) -> U32: rd(m, 17)
def set_g_bank_base(+m: Mem, +v: U32) -> Mem: wr(m, 17, v)
def g_bank_offered(+m: Mem) -> U32: rd(m, 18)
def set_g_bank_offered(+m: Mem, +v: U32) -> Mem: wr(m, 18, v)
def g_bank_kills(+m: Mem) -> U32: rd(m, 19)
def set_g_bank_kills(+m: Mem, +v: U32) -> Mem: wr(m, 19, v)
def g_bank_saved(+m: Mem) -> U32: rd(m, 20)
def set_g_bank_saved(+m: Mem, +v: U32) -> Mem: wr(m, 20, v)
def g_bank_lost(+m: Mem) -> U32: rd(m, 21)
def set_g_bank_lost(+m: Mem, +v: U32) -> Mem: wr(m, 21, v)
def g_bank_gates(+m: Mem) -> U32: rd(m, 22)
def set_g_bank_gates(+m: Mem, +v: U32) -> Mem: wr(m, 22, v)
def g_beat_pts(+m: Mem) -> U32: rd(m, 23)
def set_g_beat_pts(+m: Mem, +v: U32) -> Mem: wr(m, 23, v)
def g_beat_base(+m: Mem) -> U32: rd(m, 24)
def set_g_beat_base(+m: Mem, +v: U32) -> Mem: wr(m, 24, v)
def g_beat_offered(+m: Mem) -> U32: rd(m, 25)
def set_g_beat_offered(+m: Mem, +v: U32) -> Mem: wr(m, 25, v)
def g_beat_kills(+m: Mem) -> U32: rd(m, 26)
def set_g_beat_kills(+m: Mem, +v: U32) -> Mem: wr(m, 26, v)
def g_beat_saved(+m: Mem) -> U32: rd(m, 27)
def set_g_beat_saved(+m: Mem, +v: U32) -> Mem: wr(m, 27, v)
def g_beat_lost(+m: Mem) -> U32: rd(m, 28)
def set_g_beat_lost(+m: Mem, +v: U32) -> Mem: wr(m, 28, v)
def g_beat_gates(+m: Mem) -> U32: rd(m, 29)
def set_g_beat_gates(+m: Mem, +v: U32) -> Mem: wr(m, 29, v)
def g_n_foes(+m: Mem) -> U32: rd(m, 30)
def set_g_n_foes(+m: Mem, +v: U32) -> Mem: wr(m, 30, v)
def g_n_props(+m: Mem) -> U32: rd(m, 31)
def set_g_n_props(+m: Mem, +v: U32) -> Mem: wr(m, 31, v)
def g_n_bolts(+m: Mem) -> U32: rd(m, 32)
def set_g_n_bolts(+m: Mem, +v: U32) -> Mem: wr(m, 32, v)
def g_n_wing(+m: Mem) -> U32: rd(m, 33)
def set_g_n_wing(+m: Mem, +v: U32) -> Mem: wr(m, 33, v)
def g_n_calls(+m: Mem) -> U32: rd(m, 34)
def set_g_n_calls(+m: Mem, +v: U32) -> Mem: wr(m, 34, v)
def g_n_fx(+m: Mem) -> U32: rd(m, 35)
def set_g_n_fx(+m: Mem, +v: U32) -> Mem: wr(m, 35, v)
def g_boss(+m: Mem) -> U32: rd(m, 36)
def set_g_boss(+m: Mem, +v: U32) -> Mem: wr(m, 36, v)
def g_n_snd(+m: Mem) -> U32: rd(m, 37)
def set_g_n_snd(+m: Mem, +v: U32) -> Mem: wr(m, 37, v)
def g_n_leave(+m: Mem) -> U32: rd(m, 38)
def set_g_n_leave(+m: Mem, +v: U32) -> Mem: wr(m, 38, v)
def g_num_charge(+m: Mem) -> U32: rd(m, 39)
def g_num_roll(+m: Mem) -> U32: rd(m, 40)
def g_num_spur(+m: Mem) -> U32: rd(m, 42)
def g_num_tell(+m: Mem) -> U32: rd(m, 43)
def g_here(+m: Mem) -> U32: rd(m, 44)
def c_x(+m: Mem, +b: U32) -> U32: rd(m, (b + 0 : U32))
def set_c_x(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 0 : U32), v)
def c_z(+m: Mem, +b: U32) -> U32: rd(m, (b + 1 : U32))
def set_c_z(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 1 : U32), v)
def c_vx(+m: Mem, +b: U32) -> U32: rd(m, (b + 2 : U32))
def set_c_vx(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 2 : U32), v)
def c_vz(+m: Mem, +b: U32) -> U32: rd(m, (b + 3 : U32))
def set_c_vz(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 3 : U32), v)
def c_bank(+m: Mem, +b: U32) -> U32: rd(m, (b + 4 : U32))
def set_c_bank(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 4 : U32), v)
def c_lift(+m: Mem, +b: U32) -> U32: rd(m, (b + 5 : U32))
def set_c_lift(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 5 : U32), v)
def c_back(+m: Mem, +b: U32) -> U32: rd(m, (b + 6 : U32))
def set_c_back(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 6 : U32), v)
def c_ds(+m: Mem, +b: U32) -> U32: rd(m, (b + 7 : U32))
def set_c_ds(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 7 : U32), v)
def c_rx(+m: Mem, +b: U32) -> U32: rd(m, (b + 8 : U32))
def set_c_rx(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 8 : U32), v)
def c_rz(+m: Mem, +b: U32) -> U32: rd(m, (b + 9 : U32))
def set_c_rz(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 9 : U32), v)
def c_shield(+m: Mem, +b: U32) -> U32: rd(m, (b + 10 : U32))
def set_c_shield(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 10 : U32), v)
def c_hurt(+m: Mem, +b: U32) -> U32: rd(m, (b + 11 : U32))
def set_c_hurt(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 11 : U32), v)
def c_down(+m: Mem, +b: U32) -> U32: rd(m, (b + 12 : U32))
def set_c_down(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 12 : U32), v)
def c_roll(+m: Mem, +b: U32) -> U32: rd(m, (b + 13 : U32))
def set_c_roll(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 13 : U32), v)
def c_roll_len(+m: Mem, +b: U32) -> U32: rd(m, (b + 14 : U32))
def set_c_roll_len(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 14 : U32), v)
def c_dir(+m: Mem, +b: U32) -> U32: rd(m, (b + 15 : U32))
def set_c_dir(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 15 : U32), v)
def c_rest(+m: Mem, +b: U32) -> U32: rd(m, (b + 16 : U32))
def set_c_rest(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 16 : U32), v)
def c_held(+m: Mem, +b: U32) -> U32: rd(m, (b + 17 : U32))
def set_c_held(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 17 : U32), v)
def c_charge(+m: Mem, +b: U32) -> U32: rd(m, (b + 18 : U32))
def set_c_charge(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 18 : U32), v)
def c_cool(+m: Mem, +b: U32) -> U32: rd(m, (b + 19 : U32))
def set_c_cool(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 19 : U32), v)
def c_nlocks(+m: Mem, +b: U32) -> U32: rd(m, (b + 20 : U32))
def set_c_nlocks(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 20 : U32), v)
def c_dwell(+m: Mem, +b: U32) -> U32: rd(m, (b + 33 : U32))
def set_c_dwell(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 33 : U32), v)
def c_meter(+m: Mem, +b: U32) -> U32: rd(m, (b + 34 : U32))
def set_c_meter(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 34 : U32), v)
def c_spent(+m: Mem, +b: U32) -> U32: rd(m, (b + 35 : U32))
def set_c_spent(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 35 : U32), v)
def c_boost(+m: Mem, +b: U32) -> U32: rd(m, (b + 36 : U32))
def set_c_boost(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 36 : U32), v)
def set_c_brake(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 37 : U32), v)
def c_bombs(+m: Mem, +b: U32) -> U32: rd(m, (b + 38 : U32))
def set_c_bombs(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 38 : U32), v)
def c_keys(+m: Mem, +b: U32) -> U32: rd(m, (b + 39 : U32))
def set_c_keys(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 39 : U32), v)
def c_tap_dir(+m: Mem, +b: U32) -> U32: rd(m, (b + 40 : U32))
def set_c_tap_dir(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 40 : U32), v)
def c_tap_ticks(+m: Mem, +b: U32) -> U32: rd(m, (b + 41 : U32))
def set_c_tap_ticks(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 41 : U32), v)
def c_last_x(+m: Mem, +b: U32) -> U32: rd(m, (b + 42 : U32))
def set_c_last_x(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 42 : U32), v)
def c_chain(+m: Mem, +b: U32) -> U32: rd(m, (b + 43 : U32))
def set_c_chain(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 43 : U32), v)
def c_chain_ticks(+m: Mem, +b: U32) -> U32: rd(m, (b + 44 : U32))
def set_c_chain_ticks(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 44 : U32), v)
def c_pts(+m: Mem, +b: U32) -> U32: rd(m, (b + 45 : U32))
def set_c_pts(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 45 : U32), v)
def c_hits(+m: Mem, +b: U32) -> U32: rd(m, (b + 46 : U32))
def set_c_hits(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 46 : U32), v)
def c_saved(+m: Mem, +b: U32) -> U32: rd(m, (b + 47 : U32))
def set_c_saved(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 47 : U32), v)
def c_gates(+m: Mem, +b: U32) -> U32: rd(m, (b + 48 : U32))
def set_c_gates(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 48 : U32), v)
def st_present(+m: Mem, +b: U32) -> U32: rd(m, (b + 0 : U32))
def set_st_present(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 0 : U32), v)
def set_st_joined(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 1 : U32), v)
def w_id(+m: Mem, +b: U32) -> U32: rd(m, (b + 0 : U32))
def set_w_id(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 0 : U32), v)
def w_lost(+m: Mem, +b: U32) -> U32: rd(m, (b + 1 : U32))
def set_w_lost(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 1 : U32), v)
def w_fire(+m: Mem, +b: U32) -> U32: rd(m, (b + 2 : U32))
def set_w_fire(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 2 : U32), v)
def f_id(+m: Mem, +b: U32) -> U32: rd(m, (b + 0 : U32))
def set_f_id(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 0 : U32), v)
def f_kind(+m: Mem, +b: U32) -> U32: rd(m, (b + 1 : U32))
def set_f_kind(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 1 : U32), v)
def f_s(+m: Mem, +b: U32) -> U32: rd(m, (b + 2 : U32))
def set_f_s(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 2 : U32), v)
def f_x(+m: Mem, +b: U32) -> U32: rd(m, (b + 3 : U32))
def set_f_x(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 3 : U32), v)
def f_z(+m: Mem, +b: U32) -> U32: rd(m, (b + 4 : U32))
def set_f_z(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 4 : U32), v)
def f_vs(+m: Mem, +b: U32) -> U32: rd(m, (b + 5 : U32))
def set_f_vs(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 5 : U32), v)
def f_vx(+m: Mem, +b: U32) -> U32: rd(m, (b + 6 : U32))
def set_f_vx(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 6 : U32), v)
def f_vz(+m: Mem, +b: U32) -> U32: rd(m, (b + 7 : U32))
def set_f_vz(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 7 : U32), v)
def f_hp(+m: Mem, +b: U32) -> U32: rd(m, (b + 8 : U32))
def set_f_hp(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 8 : U32), v)
def f_full(+m: Mem, +b: U32) -> U32: rd(m, (b + 9 : U32))
def set_f_full(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 9 : U32), v)
def f_seats(+m: Mem, +b: U32) -> U32: rd(m, (b + 10 : U32))
def set_f_seats(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 10 : U32), v)
def f_st(+m: Mem, +b: U32) -> U32: rd(m, (b + 11 : U32))
def set_f_st(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 11 : U32), v)
def f_ticks(+m: Mem, +b: U32) -> U32: rd(m, (b + 12 : U32))
def set_f_ticks(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 12 : U32), v)
def f_fx(+m: Mem, +b: U32) -> U32: rd(m, (b + 13 : U32))
def set_f_fx(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 13 : U32), v)
def f_fz(+m: Mem, +b: U32) -> U32: rd(m, (b + 14 : U32))
def set_f_fz(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 14 : U32), v)
def f_on(+m: Mem, +b: U32) -> U32: rd(m, (b + 15 : U32))
def set_f_on(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 15 : U32), v)
def f_flash(+m: Mem, +b: U32) -> U32: rd(m, (b + 16 : U32))
def set_f_flash(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 16 : U32), v)
def f_mode(+m: Mem, +b: U32) -> U32: rd(m, (b + 17 : U32))
def set_f_mode(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 17 : U32), v)
def f_phase(+m: Mem, +b: U32) -> U32: rd(m, (b + 18 : U32))
def set_f_phase(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 18 : U32), v)
def bl_shot(+m: Mem, +b: U32) -> U32: rd(m, (b + 0 : U32))
def set_bl_shot(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 0 : U32), v)
def bl_owner(+m: Mem, +b: U32) -> U32: rd(m, (b + 1 : U32))
def set_bl_owner(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 1 : U32), v)
def bl_s(+m: Mem, +b: U32) -> U32: rd(m, (b + 2 : U32))
def set_bl_s(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 2 : U32), v)
def bl_x(+m: Mem, +b: U32) -> U32: rd(m, (b + 3 : U32))
def set_bl_x(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 3 : U32), v)
def bl_z(+m: Mem, +b: U32) -> U32: rd(m, (b + 4 : U32))
def set_bl_z(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 4 : U32), v)
def bl_vs(+m: Mem, +b: U32) -> U32: rd(m, (b + 5 : U32))
def set_bl_vs(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 5 : U32), v)
def bl_vx(+m: Mem, +b: U32) -> U32: rd(m, (b + 6 : U32))
def set_bl_vx(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 6 : U32), v)
def bl_vz(+m: Mem, +b: U32) -> U32: rd(m, (b + 7 : U32))
def set_bl_vz(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 7 : U32), v)
def bl_dmg(+m: Mem, +b: U32) -> U32: rd(m, (b + 8 : U32))
def set_bl_dmg(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 8 : U32), v)
def bl_life(+m: Mem, +b: U32) -> U32: rd(m, (b + 9 : U32))
def set_bl_life(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 9 : U32), v)
def bl_aim(+m: Mem, +b: U32) -> U32: rd(m, (b + 10 : U32))
def set_bl_aim(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 10 : U32), v)
def bl_src(+m: Mem, +b: U32) -> U32: rd(m, (b + 11 : U32))
def set_bl_src(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 11 : U32), v)
def bl_delay(+m: Mem, +b: U32) -> U32: rd(m, (b + 12 : U32))
def set_bl_delay(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 12 : U32), v)
def bl_full(+m: Mem, +b: U32) -> U32: rd(m, (b + 13 : U32))
def set_bl_full(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 13 : U32), v)
def p_thing(+m: Mem, +b: U32) -> U32: rd(m, (b + 0 : U32))
def set_p_thing(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 0 : U32), v)
def p_s(+m: Mem, +b: U32) -> U32: rd(m, (b + 1 : U32))
def set_p_s(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 1 : U32), v)
def p_x(+m: Mem, +b: U32) -> U32: rd(m, (b + 2 : U32))
def set_p_x(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 2 : U32), v)
def p_z(+m: Mem, +b: U32) -> U32: rd(m, (b + 3 : U32))
def set_p_z(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 3 : U32), v)
def p_w(+m: Mem, +b: U32) -> U32: rd(m, (b + 4 : U32))
def set_p_w(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 4 : U32), v)
def p_h(+m: Mem, +b: U32) -> U32: rd(m, (b + 5 : U32))
def set_p_h(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 5 : U32), v)
def p_len(+m: Mem, +b: U32) -> U32: rd(m, (b + 6 : U32))
def set_p_len(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 6 : U32), v)
def p_taken(+m: Mem, +b: U32) -> U32: rd(m, (b + 7 : U32))
def set_p_taken(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 7 : U32), v)
def p_phase(+m: Mem, +b: U32) -> U32: rd(m, (b + 8 : U32))
def set_p_phase(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 8 : U32), v)
def set_ca_who(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 0 : U32), v)
def ca_line(+m: Mem, +b: U32) -> U32: rd(m, (b + 1 : U32))
def set_ca_line(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 1 : U32), v)
def ca_ticks(+m: Mem, +b: U32) -> U32: rd(m, (b + 2 : U32))
def set_ca_ticks(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 2 : U32), v)
def set_fx_what(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 0 : U32), v)
def set_fx_s(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 1 : U32), v)
def set_fx_x(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 2 : U32), v)
def set_fx_z(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 3 : U32), v)
def fx_ticks(+m: Mem, +b: U32) -> U32: rd(m, (b + 4 : U32))
def set_fx_ticks(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 4 : U32), v)
def set_fx_n(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 5 : U32), v)
def bo_phase(+m: Mem, +b: U32) -> U32: rd(m, (b + 0 : U32))
def set_bo_phase(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 0 : U32), v)
def bo_st(+m: Mem, +b: U32) -> U32: rd(m, (b + 1 : U32))
def set_bo_st(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 1 : U32), v)
def bo_ticks(+m: Mem, +b: U32) -> U32: rd(m, (b + 2 : U32))
def set_bo_ticks(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 2 : U32), v)
def bo_x(+m: Mem, +b: U32) -> U32: rd(m, (b + 3 : U32))
def set_bo_x(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 3 : U32), v)
def bo_z(+m: Mem, +b: U32) -> U32: rd(m, (b + 4 : U32))
def set_bo_z(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 4 : U32), v)
def bo_nmarks(+m: Mem, +b: U32) -> U32: rd(m, (b + 8 : U32))
def set_bo_nmarks(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 8 : U32), v)
def bo_gap(+m: Mem, +b: U32) -> U32: rd(m, (b + 25 : U32))
def set_bo_gap(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 25 : U32), v)
def set_bo_side(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 26 : U32), v)
def bo_col(+m: Mem, +b: U32) -> U32: rd(m, (b + 27 : U32))
def set_bo_col(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 27 : U32), v)
def bo_atk(+m: Mem, +b: U32) -> U32: rd(m, (b + 28 : U32))
def set_bo_atk(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 28 : U32), v)
def bo_seq(+m: Mem, +b: U32) -> U32: rd(m, (b + 29 : U32))
def set_bo_seq(+m: Mem, +b: U32, +v: U32) -> Mem: wr(m, (b + 29 : U32), v)
def c_lock_foe(+m: Mem, +b: U32, +k: U32) -> U32: rd(m, (b + 21 + (k * 2 : U32) : U32))
def c_lock_n(+m: Mem, +b: U32, +k: U32) -> U32: rd(m, (b + 22 + (k * 2 : U32) : U32))
def set_c_lock_foe(+m: Mem, +b: U32, +k: U32, +v: U32) -> Mem: wr(m, (b + 21 + (k * 2 : U32) : U32), v)
def set_c_lock_n(+m: Mem, +b: U32, +k: U32, +v: U32) -> Mem: wr(m, (b + 22 + (k * 2 : U32) : U32), v)
def g_snd(+m: Mem, +k: U32) -> U32: rd(m, (45 + k : U32))
def set_g_snd(+m: Mem, +k: U32, +v: U32) -> Mem: wr(m, (45 + k : U32), v)
def set_g_leave(+m: Mem, +k: U32, +v: U32) -> Mem: wr(m, (57 + k : U32), v)
def g_reg(+m: Mem, +k: U32) -> U32: rd(m, (63 + k : U32))
def set_g_reg(+m: Mem, +k: U32, +v: U32) -> Mem: wr(m, (63 + k : U32), v)
def key_at(+m: Mem, +id: U32, +k: U32) -> U32: rd(m, (71 + (((id - 1 : U32) * 9 : U32) + k : U32) : U32))
def bo_mark_x(+m: Mem, +b: U32, +k: U32) -> U32: rd(m, (b + 9 + (k * 2 : U32) : U32))
def bo_mark_z(+m: Mem, +b: U32, +k: U32) -> U32: rd(m, (b + 10 + (k * 2 : U32) : U32))
def set_bo_mark_x(+m: Mem, +b: U32, +k: U32, +v: U32) -> Mem: wr(m, (b + 9 + (k * 2 : U32) : U32), v)
def set_bo_mark_z(+m: Mem, +b: U32, +k: U32, +v: U32) -> Mem: wr(m, (b + 10 + (k * 2 : U32) : U32), v)
def SEAT_CRAFT() -> U32: 2
def WING_CRAFT() -> U32: 3

# ===== the names of things (enum indices; Rust owns the words) =====

def K_WISP() -> U32: 0

def K_KITE() -> U32: 1
def K_TURRET() -> U32: 2
def K_BOMBER() -> U32: 3
def K_LAMP() -> U32: 4
def S_FLY() -> U32: 0
def S_TELL() -> U32: 1
def S_REST() -> U32: 2
def S_HUNT() -> U32: 3
def S_LATCHED() -> U32: 4
def S_FLEE() -> U32: 5
def S_HIDE() -> U32: 6
def S_DIVE() -> U32: 7
def S_SHUT() -> U32: 8
def S_OPEN() -> U32: 9
def S_BROKEN() -> U32: 10
def A_LANCE() -> U32: 0
def A_STAR() -> U32: 1
def A_ALLY() -> U32: 2
def A_SPARK() -> U32: 3
def A_GLOAM() -> U32: 4
def A_TURNED() -> U32: 5
def T_GATE() -> U32: 0
def T_RING() -> U32: 1
def T_ARCH() -> U32: 2
def T_RIDGE() -> U32: 3
def T_SHUTTER() -> U32: 4
def L_HELLO() -> U32: 0
def L_LATCHED() -> U32: 4
def L_THANKS() -> U32: 5
def L_LOST() -> U32: 6
def L_WYRM() -> U32: 12
def L_OPEN() -> U32: 13
def L_REFLECT() -> U32: 14
def L_COILS() -> U32: 15
def L_ROAR() -> U32: 16
def L_HOME() -> U32: 18
def L_CHAIN() -> U32: 19
def L_JOIN() -> U32: 21
def L_LOW() -> U32: 22
def L_ROOST() -> U32: 23
def L_RUINS() -> U32: 24
def B_POP() -> U32: 0
def B_HIT() -> U32: 1
def B_TURN() -> U32: 2
def B_LOCK() -> U32: 3
def B_GATE() -> U32: 4
def B_SCRAPE() -> U32: 5
def B_FALL() -> U32: 6
def B_HURT() -> U32: 7
def B_LAMP() -> U32: 8
def B_STARFALL() -> U32: 9
def W_RISE() -> U32: 0
def W_IDLE() -> U32: 1
def W_TELL() -> U32: 2
def W_COIL() -> U32: 3
def W_ROAR() -> U32: 4
def W_HURT() -> U32: 5
def W_DYING() -> U32: 6
def M_PLAY() -> U32: 0
def M_RETRY() -> U32: 1
def M_VICTORY() -> U32: 2
def M_TALLY() -> U32: 3
def SND_PLAY_CARD() -> U32: 0
def SND_KNIGHT_FALL() -> U32: 1
def SND_BOSS_DEATH() -> U32: 2
def SND_ROLL() -> U32: 3
def SND_SHOT_BOLT() -> U32: 4
def SND_SPOIL_PICKUP() -> U32: 5
def SND_SHOT_SPARK() -> U32: 6
def SND_VENT_FIRE() -> U32: 7
def SND_SHOT_CROSSBOW() -> U32: 8
def SND_CLANG() -> U32: 9
def SND_HERO_HURT() -> U32: 10
def SND_KNIGHT_HIT_BIG() -> U32: 11
def SND_BOMB_TOSS() -> U32: 12
def SND_HIT_MAGIC() -> U32: 13
def SND_KILL() -> U32: 14
def SND_BOSS_RAGE() -> U32: 15
def SND_BOMB() -> U32: 16
def SND_SHIELD_UP() -> U32: 17
def SND_SHIELD_BLOCK() -> U32: 18
def SND_GOLD() -> U32: 19
def SND_ROCK_LAND() -> U32: 20
def SND_BOSS_RISE() -> U32: 21
def SND_BOSS_ROAR() -> U32: 22

# ===== the numbers of the game =====

def PACE() -> U32: 85

def Q() -> U32: 256
def LEAD() -> U32: dm(640)
def FAR() -> U32: dm(340)
def LOCK_MAX() -> U32: dm(520)
def LOCK_MIN() -> U32: dm(15)
def SHIELD() -> U32: 100
def METER() -> U32: 7200
def HZ() -> U32: 30
def LIMIT() -> U32: 108000
def RETRY() -> U32: 24
def DYING() -> U32: 60
def VICTORY() -> U32: 210
def TALLY_HOLD() -> U32: 45
def RISE() -> U32: 75
def HURT() -> U32: 36

# where the counts of the tables live
def CA_FOES() -> U32: 30

def CA_PROPS() -> U32: 31
def CA_BOLTS() -> U32: 32
def CRAFT_R() -> U32: dm(10)

# what each kind of foe is: its body, its health and what it is worth
def kind5(+k: U32, +a: U32, +b: U32, +c: U32, +d: U32, +e: U32) -> U32:
  pk(eq(k, 0), a, pk(eq(k, 1), b, pk(eq(k, 2), c, pk(eq(k, 3), d, e))))

def k_radius(+k: U32) -> U32: kind5(k, dm(9), dm(8), dm(16), dm(12), dm(16))
def k_health(+k: U32) -> U32: kind5(k, 20, 20, 60, 40, 120)
def k_worth(+k: U32) -> U32: kind5(k, 10, 20, 60, 40, 500)

# the rail's first point of a beat, and where the formation is
def origin(+stage: U32) -> U32: pk(eq(stage, 0), 0, pk(eq(stage, 1), 153000, 357000))

def anchor(+m: Mem) -> U32: U32.add(origin(mn(g_stage(m), 2)), U32.mul(g_t(m), 85))

# ===== places in the frame =====

def seat_b(+id: U32) -> U32: U32.add(SEATS(), U32.mul(U32.sub(id, 1), SEAT_W()))

def seat_c(+id: U32) -> U32: U32.add(seat_b(id), SEAT_CRAFT())
def wing_b(+k: U32) -> U32: U32.add(WINGS(), U32.mul(k, WING_W()))
def wing_c(+k: U32) -> U32: U32.add(wing_b(k), WING_CRAFT())
def foe_b(+i: U32) -> U32: U32.add(FOES(), U32.mul(i, FOE_W()))
def bolt_b(+i: U32) -> U32: U32.add(BOLTS(), U32.mul(i, BOLT_W()))
def prop_b(+i: U32) -> U32: U32.add(PROPS(), U32.mul(i, PROP_W()))
def call_b(+i: U32) -> U32: U32.add(CALLS(), U32.mul(i, 3))
def fx_b(+i: U32) -> U32: U32.add(FXS(), U32.mul(i, FX_W()))

# the knights seated, and who is still at the table
def seated(+m: Mem, +id: U32) -> Bool: nz(st_present(m, seat_b(id)))

def count_seats(+n: Nat, +m: Mem, +id: U32) -> U32:
  match n:
    case 0n:
      0
    case 1n+p:
      U32.add(b2u(seated(m, id)), count_seats(p, m, U32.add(id, 1)))

# how many knights fly: the pressure on the foes (a table with nobody at it is one's worth)
def pressure(+m: Mem) -> U32: cl(count_seats(6n, m, 1), 1, 6)

# ===== the session's little lists =====

# a fresh id for a foe: the one in hand is taken, the next is put by
def bump_ids(+m: Mem) -> Mem:
  +id = g_ids(m)
  set_g_ids(m, pk(ge(id, 60000), 1, U32.add(id, 1)))

def snd_has(+n: Nat, +m: Mem, +k: U32, +name: U32) -> Bool:
  match n:
    case 0n:
      False{}
    case 1n+p:
      Bool.or(eq(g_snd(m, k), name), snd_has(p, m, U32.add(k, 1), name))

def sound_put(+m: Mem, +name: U32, +ok: Bool) -> Mem:
  match ok:
    case True{}:
      set_g_n_snd(set_g_snd(m, g_n_snd(m), name), U32.add(g_n_snd(m), 1))
    case False{}:
      m

# a sound is cued once a tick, and twelve at most
def sound(+m: Mem, +name: U32) -> Mem:
  sound_put(m, name, Bool.and(lt(g_n_snd(m), 12), Bool.not(snd_has(nt(g_n_snd(m)), m, 0, name))))

# shift a table down one row from row `i`: rows i+1.. move to i..
def drop_row(+m: Mem, +base: U32, +w: U32, +n: U32, +i: U32) -> Mem:
  cpw(nt(ssub(U32.mul(U32.sub(n, 1), w), U32.mul(i, w))), m, U32.add(base, U32.mul(U32.add(i, 1), w)), U32.add(base, U32.mul(i, w)))

def call_has(+n: Nat, +m: Mem, +i: U32, +line: U32) -> Bool:
  match n:
    case 0n:
      False{}
    case 1n+p:
      Bool.or(Bool.and(eq(ca_line(m, call_b(i)), line), gt(ca_ticks(m, call_b(i)), 40)), call_has(p, m, U32.add(i, 1), line))

def say_push(+m: Mem, +who: U32, +line: U32) -> Mem:
  +b = call_b(g_n_calls(m))
  set_g_n_calls(set_ca_ticks(set_ca_line(set_ca_who(m, b, who), b, line), b, 100), U32.add(g_n_calls(m), 1))

def say_full(+m: Mem, +who: U32, +line: U32, +full: Bool) -> Mem:
  match full:
    case True{}:
      say_push(set_g_n_calls(drop_row(m, CALLS(), 3, g_n_calls(m), 0), U32.sub(g_n_calls(m), 1)), who, line)
    case False{}:
      say_push(m, who, line)

def say_go(+m: Mem, +who: U32, +line: U32, +dup: Bool) -> Mem:
  match dup:
    case True{}:
      m
    case False{}:
      say_full(m, who, line, ge(g_n_calls(m), 4))

# a rider's short shout, shown as a portrait for a moment: the same line is not said twice in a row
def say(+m: Mem, +who: U32, +line: U32) -> Mem: say_go(m, who, line, call_has(nt(g_n_calls(m)), m, 0, line))

def spark_ticks(+what: U32) -> U32: pk(eq(what, 9), 36, pk(eq(what, 6), 40, pk(eq(what, 3), 8, 14)))

def spark_push(+m: Mem, +what: U32, +s: U32, +x: U32, +z: U32, +n: U32) -> Mem:
  +b = fx_b(g_n_fx(m))
  set_g_n_fx(set_fx_n(set_fx_ticks(set_fx_z(set_fx_x(set_fx_s(set_fx_what(m, b, what), b, s), b, x), b, z), b, spark_ticks(what)), b, n), U32.add(g_n_fx(m), 1))

def spark_full(+m: Mem, +what: U32, +s: U32, +x: U32, +z: U32, +n: U32, +full: Bool) -> Mem:
  match full:
    case True{}:
      spark_push(set_g_n_fx(drop_row(m, FXS(), FX_W(), g_n_fx(m), 0), U32.sub(g_n_fx(m), 1)), what, s, x, z, n)
    case False{}:
      spark_push(m, what, s, x, z, n)

# a flash, a pop or a ring for the stage to draw
def spark(+m: Mem, +what: U32, +s: U32, +x: U32, +z: U32, +n: U32) -> Mem: spark_full(m, what, s, x, z, n, ge(g_n_fx(m), 48))

# ===== the wing, the craft and who is flying =====

# the wing's index with this id, 99 if none
def wing_of(+n: Nat, +m: Mem, +k: U32, +id: U32) -> U32:
  match n:
    case 0n:
      99
    case 1n+p:
      pk(eq(w_id(m, wing_b(k)), id), k, wing_of(p, m, U32.add(k, 1), id))

# the first of the riders still flying, else Aster who leads the wing
def voice_go(+n: Nat, +m: Mem, +k: U32) -> U32:
  match n:
    case 0n:
      7
    case 1n+p:
      pk(z(w_lost(m, wing_b(k))), w_id(m, wing_b(k)), voice_go(p, m, U32.add(k, 1)))

def voice(+m: Mem) -> U32: voice_go(nt(g_n_wing(m)), m, 0)

# a craft by the id a bolt or a foe holds: its record's base (0: none)
def craft_of(+m: Mem, +id: U32) -> U32:
  +k = wing_of(nt(g_n_wing(m)), m, 0, id)
  pk(Bool.and(ge(id, 1), le(id, 6)), pk(seated(m, id), seat_c(id), 0), pk(lt(k, 4), wing_c(k), 0))

def cr_s(+m: Mem, +cb: U32) -> U32: U32.add(anchor(m), c_back(m, cb))

# whether a craft is flying: a rider that is lost is not
def cr_flying(+m: Mem, +id: U32, +cb: U32) -> Bool:
  Bool.and(z(c_down(m, cb)), pb(le(id, 6), True{}, z(w_lost(m, U32.sub(cb, WING_CRAFT())))))

# ===== lists: retaining rows =====

# keep the rows `keep` accepts, in order, moving the rest of the table down over the gaps; the new
# count is written to word `ca`. `keep` reads the row by its index.
def retain_go(~keep: @+m: Mem -> @+i: U32 -> Bool, +n: Nat, +k: Bool, +m: Mem, +base: U32, +w: U32, +i: U32, +j: U32, +ca: U32) -> Mem:
  match n k:
    case 0n _:
      wr(m, ca, j)
    case 1n+p True{}:
      +m2 = cpw(nt(w), m, U32.add(base, U32.mul(i, w)), U32.add(base, U32.mul(j, w)))
      retain_go(~keep, p, keep(m2, U32.add(i, 1)), m2, base, w, U32.add(i, 1), U32.add(j, 1), ca)
    case 1n+p False{}:
      retain_go(~keep, p, keep(m, U32.add(i, 1)), m, base, w, U32.add(i, 1), j, ca)

def retain(~keep: @+m: Mem -> @+i: U32 -> Bool, +m: Mem, +base: U32, +w: U32, +n: U32, +ca: U32) -> Mem:
  retain_go(~keep, nt(n), keep(m, 0), m, base, w, 0, 0, ca)

# ===== foes =====

def f_alive(+m: Mem, +fb: U32) -> Bool: Bool.and(gt(f_hp(m, fb), 0), ne(f_st(m, fb), S_BROKEN()))

# whether a lock may take it: alive, in the open and (a lamp) with its shield open
def f_lockable(+m: Mem, +fb: U32) -> Bool:
  +st = f_st(m, fb)
  Bool.and(f_alive(m, fb), Bool.not(Bool.or(eq(st, S_HIDE()), Bool.or(eq(st, S_SHUT()), eq(st, S_FLEE())))))

def f_radius(+m: Mem, +fb: U32) -> U32: k_radius(f_kind(m, fb))

# the foe with this id: its row's index (99: none)
def foe_idx(+n: Nat, +m: Mem, +i: U32, +id: U32) -> U32:
  match n:
    case 0n:
      99
    case 1n+p:
      pk(eq(f_id(m, foe_b(i)), id), i, foe_idx(p, m, U32.add(i, 1), id))

def foe_by_id(+m: Mem, +id: U32) -> U32: foe_idx(nt(g_n_foes(m)), m, 0, id)

# the same, for a foe that is also alive
def foe_idx_alive(+n: Nat, +m: Mem, +i: U32, +id: U32) -> U32:
  match n:
    case 0n:
      99
    case 1n+p:
      pk(Bool.and(eq(f_id(m, foe_b(i)), id), f_alive(m, foe_b(i))), i, foe_idx_alive(p, m, U32.add(i, 1), id))

def foe_remove(+m: Mem, +i: U32) -> Mem:
  set_g_n_foes(drop_row(m, FOES(), FOE_W(), g_n_foes(m), i), U32.sub(g_n_foes(m), 1))

def beat_offer(+m: Mem, +v: U32) -> Mem: set_g_beat_offered(m, U32.add(g_beat_offered(m), v))

def spawn_put(+m: Mem, +kind: U32, +s: U32, +x: U32, +z: U32, +vs: U32, +vx: U32, +vz: U32, +st: U32, +mode: U32, +phase: U32, +ok: Bool) -> Mem:
  match ok:
    case False{}:
      set_g_reg(m, 0, 0)
    case True{}:
      +b = foe_b(g_n_foes(m))
      +seats = pressure(m)
      +full = U32.mul(k_health(kind), seats)
      +m1 = set_f_id(set_f_kind(set_f_s(set_f_x(set_f_z(m, b, z), b, x), b, s), b, kind), b, g_ids(m))
      +m2 = set_f_vz(set_f_vx(set_f_vs(m1, b, vs), b, vx), b, vz)
      +m3 = set_f_seats(set_f_full(set_f_hp(m2, b, full), b, full), b, seats)
      +m4 = set_f_ticks(set_f_st(m3, b, st), b, 0)
      +m5 = set_f_on(set_f_fz(set_f_fx(m4, b, 0), b, 0), b, 0)
      +m6 = set_f_phase(set_f_mode(set_f_flash(m5, b, 0), b, mode), b, phase)
      set_g_reg(beat_offer(set_g_n_foes(bump_ids(m6), U32.add(g_n_foes(m), 1)), k_worth(kind)), 0, g_ids(m))

# put a foe on the rail, its health provisioned for the knights at the table: its id (0: none) is left in reg 0
def spawn_foe(+m: Mem, +kind: U32, +s: U32, +x: U32, +z: U32, +vs: U32, +vx: U32, +vz: U32, +st: U32, +mode: U32, +phase: U32) -> Mem:
  spawn_put(m, kind, s, x, z, vs, vx, vz, st, mode, phase, lt(g_n_foes(m), 40))

# ===== bolts =====

def bolt_put(+m: Mem, +shot: U32, +owner: U32, +s: U32, +x: U32, +z: U32, +vs: U32, +vx: U32, +vz: U32, +dmg: U32, +life: U32, +aim: U32, +src: U32, +delay: U32, +full: U32, +ok: Bool) -> Mem:
  match ok:
    case False{}:
      m
    case True{}:
      +b = bolt_b(g_n_bolts(m))
      +m1 = set_bl_owner(set_bl_shot(m, b, shot), b, owner)
      +m2 = set_bl_z(set_bl_x(set_bl_s(m1, b, s), b, x), b, z)
      +m3 = set_bl_vz(set_bl_vx(set_bl_vs(m2, b, vs), b, vx), b, vz)
      +m4 = set_bl_life(set_bl_dmg(m3, b, dmg), b, life)
      +m5 = set_bl_src(set_bl_aim(m4, b, aim), b, src)
      set_g_n_bolts(set_bl_full(set_bl_delay(m5, b, delay), b, full), U32.add(g_n_bolts(m), 1))

def push_bolt(+m: Mem, +shot: U32, +owner: U32, +s: U32, +x: U32, +z: U32, +vs: U32, +vx: U32, +vz: U32, +dmg: U32, +life: U32, +aim: U32, +src: U32, +delay: U32, +full: U32) -> Mem:
  bolt_put(m, shot, owner, s, x, z, vs, vx, vz, dmg, life, aim, src, delay, full, lt(g_n_bolts(m), 160))

# ===== who is where =====

# whether some kite hangs on this craft (unless the foes are set aside, as while they take their turn)
def latched_go(+n: Nat, +m: Mem, +i: U32, +id: U32) -> Bool:
  match n:
    case 0n:
      False{}
    case 1n+p:
      Bool.or(Bool.and(eq(f_st(m, foe_b(i)), S_LATCHED()), eq(f_on(m, foe_b(i)), id)), latched_go(p, m, U32.add(i, 1), id))

def latched(+m: Mem, +id: U32) -> Bool: Bool.and(z(g_reg(m, 7)), latched_go(nt(g_n_foes(m)), m, 0, id))

# a craft that is flying and has nothing hanging on it
def free_craft(+m: Mem, +id: U32) -> Bool:
  +cb = craft_of(m, id)
  Bool.and(nz(cb), Bool.and(cr_flying(m, id, cb), Bool.not(latched(m, id))))

# the `want`th rider (in the wing's order) that is free: its id, 0 if none
def nth_rider(+n: Nat, +m: Mem, +k: U32, +want: U32) -> U32:
  match n:
    case 0n:
      0
    case 1n+p:
      +id = w_id(m, wing_b(k))
      +fr = free_craft(m, id)
      pk(Bool.and(fr, z(want)), id, nth_rider(p, m, U32.add(k, 1), pk(fr, U32.sub(want, 1), want)))

def count_riders(+n: Nat, +m: Mem, +k: U32) -> U32:
  match n:
    case 0n:
      0
    case 1n+p:
      U32.add(b2u(free_craft(m, w_id(m, wing_b(k)))), count_riders(p, m, U32.add(k, 1)))

def nth_knight(+n: Nat, +m: Mem, +id: U32, +want: U32) -> U32:
  match n:
    case 0n:
      0
    case 1n+p:
      +fr = Bool.and(seated(m, id), free_craft(m, id))
      pk(Bool.and(fr, z(want)), id, nth_knight(p, m, U32.add(id, 1), pk(fr, U32.sub(want, 1), want)))

def count_knights(+n: Nat, +m: Mem, +id: U32) -> U32:
  match n:
    case 0n:
      0
    case 1n+p:
      U32.add(b2u(Bool.and(seated(m, id), free_craft(m, id))), count_knights(p, m, U32.add(id, 1)))

# the first knight at the table in the air at all
def first_flying(+n: Nat, +m: Mem, +id: U32) -> U32:
  match n:
    case 0n:
      0
    case 1n+p:
      pk(Bool.and(seated(m, id), z(c_down(m, seat_c(id)))), id, first_flying(p, m, U32.add(id, 1)))

# who a hunting kite goes for: a rider not yet latched, else a knight not yet latched, else anyone
# flying. The same hunter always finds the same victim. 0: nobody.
def victim_pick(+m: Mem, +hunter: U32, +nr: U32, +nk: U32) -> U32:
  pk(nz(nr), nth_rider(4n, m, 0, U32.mod(hunter, nr)),
     pk(nz(nk), nth_knight(6n, m, 1, U32.mod(hunter, nk)), first_flying(6n, m, 1)))

def victim(+m: Mem, +hunter: U32) -> U32: victim_pick(m, hunter, count_riders(nt(g_n_wing(m)), m, 0), count_knights(6n, m, 1))

# the knights in the air, in turn: who a turret or the Wyrm fires at (a seat's id; 0: nobody)
def nth_flying(+n: Nat, +m: Mem, +id: U32, +want: U32) -> U32:
  match n:
    case 0n:
      0
    case 1n+p:
      +fl = Bool.and(seated(m, id), z(c_down(m, seat_c(id))))
      pk(Bool.and(fl, z(want)), id, nth_flying(p, m, U32.add(id, 1), pk(fl, U32.sub(want, 1), want)))

def count_flying(+n: Nat, +m: Mem, +id: U32) -> U32:
  match n:
    case 0n:
      0
    case 1n+p:
      U32.add(b2u(Bool.and(seated(m, id), z(c_down(m, seat_c(id))))), count_flying(p, m, U32.add(id, 1)))

def TURNED() -> U32: 60
def V_TURN() -> U32: dm(24)

# ===== a foe falls =====

def seat_ok(+m: Mem, +id: U32) -> Bool: Bool.and(Bool.and(ge(id, 1), le(id, 6)), seated(m, id))

def turn_put(+m: Mem, +bi: U32, +by: U32, +ti: U32) -> Mem:
  +bb = bolt_b(bi)
  +tb = foe_b(ti)
  +found = lt(ti, 99)
  +d0 = U32.sub(f_s(m, tb), bl_s(m, bb))
  +d1 = U32.sub(f_x(m, tb), bl_x(m, bb))
  +d2 = U32.sub(f_z(m, tb), bl_z(m, bb))
  +len = norm3(d0, d1, d2)
  +m1 = set_bl_aim(set_bl_life(set_bl_dmg(set_bl_owner(set_bl_shot(m, bb, A_TURNED()), bb, by), bb, TURNED()), bb, 90), bb, pk(found, f_id(m, tb), 0))
  +m2 = set_bl_vz(set_bl_vx(set_bl_vs(m1, bb, pk(found, scs(d0, V_TURN(), len), V_TURN())), bb, pk(found, scx(d1, V_TURN(), len), 0)), bb, pk(found, scx(d2, V_TURN(), len), 0))
  +m3 = pkm(seat_ok(m2, by), set_c_pts(m2, seat_c(by), U32.add(c_pts(m2, seat_c(by)), 25)), m2)
  sound(spark(set_g_beat_pts(m3, U32.add(g_beat_pts(m3), 25)), B_TURN(), bl_s(m, bb), bl_x(m, bb), bl_z(m, bb), by), SND_SHIELD_BLOCK())

# the lamp that is lit now: its foe's id (0: no Wyrm)
def lit_lamp(+m: Mem) -> U32:
  +b = BOSSB()
  pk(nz(g_boss(m)), rd(m, U32.add(U32.add(b, 5), mn(bo_phase(m, b), 2))), 0)

# a gate or a ring flown through
def take_gate(+m: Mem, +cb: U32, +boosting: Bool, +welcome: Bool) -> Mem:
  match welcome:
    case True{}:
      sound(m, SND_GOLD())
    case False{}:
      +pts = pk(boosting, 200, 100)
      +m1 = set_c_gates(set_c_pts(m, cb, U32.add(c_pts(m, cb), pts)), cb, U32.add(c_gates(m, cb), 1))
      sound(set_g_beat_gates(set_g_beat_base(set_g_beat_pts(m1, U32.add(g_beat_pts(m1), umn(pts, 100))), U32.add(g_beat_base(m1), 100)), U32.add(g_beat_gates(m1), 1)), SND_GOLD())

def MEND() -> U32: 50

def take(+m: Mem, +id: U32, +pb: U32, +boosting: Bool) -> Mem:
  +cb = seat_c(id)
  +m1 = pkm(eq(p_thing(m, pb), T_GATE()), take_gate(m, cb, boosting, nz(p_phase(m, pb))), sound(set_c_shield(m, cb, umn(U32.add(c_shield(m, cb), MEND()), SHIELD())), SND_SHIELD_UP()))
  spark(m1, B_GATE(), p_s(m, pb), p_x(m, pb), p_z(m, pb), id)

# ===== a rolling craft's shield turns a bolt round =====

# the nearest of the foes a bolt may be turned on or homed on, from point (ps, px, pz): row index, 99 for none.
# `min_s`/`max_s` bound how far ahead of the point it may be (max_s < 0: no bound); lamps are left out.
def near_go(+n: Nat, +m: Mem, +i: U32, +ps: U32, +px: U32, +pz: U32, +lo_s: U32, +hi_s: U32, +best: U32, +bd: U32) -> U32:
  match n:
    case 0n:
      best
    case 1n+p:
      +fb = foe_b(i)
      +d = norm3(U32.sub(f_s(m, fb), ps), U32.sub(f_x(m, fb), px), U32.sub(f_z(m, fb), pz))
      +ok = Bool.and(Bool.and(f_lockable(m, fb), ne(f_kind(m, fb), K_LAMP())), Bool.and(ge(U32.sub(f_s(m, fb), ps), lo_s), le(U32.sub(f_s(m, fb), ps), hi_s)))
      +take = Bool.and(ok, Bool.or(eq(best, 99), lt(d, bd)))
      near_go(p, m, U32.add(i, 1), ps, px, pz, lo_s, hi_s, pk(take, i, best), pk(take, d, bd))

def turn_target(+m: Mem, +bb: U32, +ps: U32, +px: U32, +pz: U32) -> U32:
  +by_src = foe_idx_alive(nt(g_n_foes(m)), m, 0, bl_src(m, bb))
  +lamp = pk(nz(g_boss(m)), foe_idx_alive(nt(g_n_foes(m)), m, 0, lit_lamp(m)), 99)
  +near = near_go(nt(g_n_foes(m)), m, 0, ps, px, pz, 1, 2147483647, 99, 0)
  pk(lt(by_src, 99), by_src, pk(lt(lamp, 99), lamp, near))

# a rolling craft's shield turns a bolt round: it is the roller's now, faster, and heads for what
# fired it (or the Wyrm's lit lamp)
def turn(+m: Mem, +bi: U32, +by: U32) -> Mem:
  +cb = seat_c(by)
  turn_put(m, bi, by, turn_target(m, bolt_b(bi), cr_s(m, cb), c_x(m, cb), c_z(m, cb)))

def target_pick(+m: Mem, +turn: U32, +cnt: U32) -> U32: pk(z(cnt), 0, nth_flying(6n, m, 1, U32.mod(turn, cnt)))
def target(+m: Mem, +turn: U32) -> U32: target_pick(m, turn, count_flying(6n, m, 1))

# a bolt from o that meets the formation's plane at `to` as the rail brings the plane to it,
# closing at `closing` a tick: how many ticks it takes
def plane_ticks(+m: Mem, +os: U32, +closing: U32) -> U32: mx(sdv(mx(U32.sub(os, anchor(m)), dm(40)), mx(closing, 1)), 1)

# the marks a turret's shot lands on: a pair side by side, and a pair more to each side for every three
# knights beyond the first. Mark i (across, up).
def mark_x(+fx: U32, +i: U32) -> U32:
  +d = U32.add(dm(8), U32.mul(U32.shr(i), dm(20)))
  pk(z(U32.and(i, 1)), U32.sub(fx, d), U32.add(fx, d))

def mark_count(+seats: U32) -> U32: U32.add(2, U32.mul(2, U32.div(ssub(seats, 1), 3)))

# ===== a craft is struck =====

def drain_if(+m: Mem, +cb: U32, +amount: U32, +ok: Bool) -> Mem:
  match ok:
    case True{}:
      set_c_shield(m, cb, mx(U32.sub(c_shield(m, cb), amount), 0))
    case False{}:
      m

# a drip from a latched kite: no spare moment, no flash
def drain(+m: Mem, +id: U32, +amount: U32) -> Mem:
  +cb = craft_of(m, id)
  drain_if(m, cb, amount, Bool.and(nz(cb), pb(le(id, 6), z(c_down(m, cb)), z(w_lost(m, U32.sub(cb, WING_CRAFT()))))))

def say_if(+m: Mem, +who: U32, +line: U32, +c: Bool) -> Mem:
  match c:
    case True{}:
      say(m, who, line)
    case False{}:
      m

# a craft's shield takes a blow. A knight is spared for a moment after one; a rider is only worn down
# (by what latches on). Reg 0 holds whether it landed.
def hurt_seat(+m: Mem, +id: U32, +cb: U32, +dmg: U32) -> Mem:
  +s = cr_s(m, cb)
  +x = c_x(m, cb)
  +z = c_z(m, cb)
  +m1 = set_c_hurt(set_c_shield(m, cb, mx(U32.sub(c_shield(m, cb), dmg), 0)), cb, HURT())
  +m2 = sound(spark(m1, B_HURT(), s, x, z, id), pk(ge(dmg, 20), SND_KNIGHT_HIT_BIG(), SND_HERO_HURT()))
  set_g_reg(say_if(m2, id, L_LOW(), le(c_shield(m2, cb), 30)), 0, 1)

def hurt_wing(+m: Mem, +cb: U32, +dmg: U32) -> Mem:
  set_g_reg(set_c_shield(m, cb, mx(U32.sub(c_shield(m, cb), dmg), 0)), 0, 1)

def hurt_go(+m: Mem, +id: U32, +cb: U32, +dmg: U32, +seat: Bool, +ok: Bool) -> Mem:
  match seat ok:
    case _ False{}:
      set_g_reg(m, 0, 0)
    case True{} True{}:
      hurt_seat(m, id, cb, dmg)
    case False{} True{}:
      hurt_wing(m, cb, dmg)

def hurt_craft(+m: Mem, +id: U32, +dmg: U32) -> Mem:
  +cb = craft_of(m, id)
  +seat = Bool.and(ge(id, 1), le(id, 6))
  hurt_go(m, id, cb, dmg, seat, Bool.and(nz(cb), pb(seat, Bool.and(z(c_hurt(m, cb)), z(c_down(m, cb))), z(w_lost(m, U32.sub(cb, WING_CRAFT()))))))

def kill_end(+m: Mem, +s: U32, +x: U32, +zz: U32, +by: U32, +gain: U32, +chain: U32) -> Mem:
  say_if(sound(spark(set_g_beat_pts(m, U32.add(g_beat_pts(m), gain)), B_POP(), s, x, zz, mn(chain, 99)), SND_KILL()), by, L_CHAIN(), eq(chain, 5))

def kill_chain(+m: Mem, +s: U32, +x: U32, +zz: U32, +by: U32, +has: Bool, +chain: U32, +gain: U32, +bonus: U32) -> Mem:
  match has:
    case True{}:
      +cb = seat_c(by)
      +m1 = set_c_hits(set_c_chain_ticks(set_c_chain(m, cb, chain), cb, 75), cb, U32.add(c_hits(m, cb), 1))
      kill_end(set_c_pts(m1, cb, U32.add(c_pts(m1, cb), U32.add(gain, bonus))), s, x, zz, by, U32.add(gain, bonus), chain)
    case False{}:
      kill_end(m, s, x, zz, by, gain, 0)

def pk_seat_saved(+m: Mem, +by: U32, +ok: Bool) -> Mem:
  match ok:
    case True{}:
      set_c_saved(m, seat_c(by), U32.add(c_saved(m, seat_c(by)), 1))
    case False{}:
      m

def kill_saved(+m: Mem, +on: U32, +by: U32, +lat: Bool) -> Mem:
  match lat:
    case False{}:
      m
    case True{}:
      +m1 = say(set_g_beat_saved(m, U32.add(g_beat_saved(m), 1)), on, L_THANKS())
      pk_seat_saved(m1, by, seat_ok(m1, by))

# a foe pops into cold smoke and a spark of returned light. A kite that had latched on is a rescue; a
# knight's chain of pops is counted, and pays more the longer it runs.
def kill_a(+m: Mem, +kind: U32, +st: U32, +on: U32, +s: U32, +x: U32, +zz: U32, +by: U32) -> Mem:
  +worth = k_worth(kind)
  +lat = eq(st, S_LATCHED())
  +m1 = set_g_beat_kills(set_g_beat_base(m, U32.add(g_beat_base(m), worth)), U32.add(g_beat_kills(m), 1))
  +m2 = kill_saved(m1, on, by, lat)
  +cb = seat_c(by)
  +chain = U32.add(c_chain(m2, cb), 1)
  kill_chain(m2, s, x, zz, by, seat_ok(m2, by), chain, U32.add(worth, pk(lat, 150, 0)), pk(ge(chain, 3), U32.mul(10, U32.sub(chain, 2)), 0))

def kill(+m: Mem, +fi: U32, +by: U32) -> Mem:
  +fb = foe_b(fi)
  kill_a(foe_remove(m, fi), f_kind(m, fb), f_st(m, fb), f_on(m, fb), f_s(m, fb), f_x(m, fb), f_z(m, fb), by)

# a lamp's flame is out: points, a flash, and the Wyrm reels (the Wyrm's tick sees it)
def break_seat(+m: Mem, +by: U32, +ok: Bool) -> Mem:
  match ok:
    case True{}:
      +cb = seat_c(by)
      set_c_hits(set_c_pts(m, cb, U32.add(c_pts(m, cb), 500)), cb, U32.add(c_hits(m, cb), 1))
    case False{}:
      m

def break_lamp(+m: Mem, +fi: U32, +by: U32) -> Mem:
  +fb = foe_b(fi)
  +m1 = set_f_hp(set_f_st(m, fb, S_BROKEN()), fb, 0)
  +m2 = set_g_beat_kills(set_g_beat_base(set_g_beat_pts(m1, U32.add(g_beat_pts(m1), 500)), U32.add(g_beat_base(m1), 500)), U32.add(g_beat_kills(m1), 1))
  sound(spark(break_seat(m2, by, seat_ok(m2, by)), B_LAMP(), f_s(m, fb), f_x(m, fb), f_z(m, fb), 2), SND_BOSS_RAGE())

def hit_go(+m: Mem, +fi: U32, +by: U32, +alive: Bool, +lamp: Bool) -> Mem:
  match alive lamp:
    case True{} _:
      set_g_reg(sound(m, SND_HIT_MAGIC()), 0, 0)
    case False{} True{}:
      set_g_reg(break_lamp(m, fi, by), 0, 1)
    case False{} False{}:
      set_g_reg(kill(m, fi, by), 0, 1)

# a foe takes `dmg` from `by`; reg 0 holds whether it fell
def hit_foe(+m: Mem, +fi: U32, +dmg: U32, +by: U32) -> Mem:
  +fb = foe_b(fi)
  +hp = ssub(f_hp(m, fb), dmg)
  +m1 = set_f_flash(set_f_hp(m, fb, hp), fb, 5)
  hit_go(m1, fi, by, gt(hp, 0), eq(f_kind(m1, fb), K_LAMP()))

# ===== Starfall =====

def STARFALL() -> U32: 120

def not_hostile(+m: Mem, +i: U32) -> Bool:
  +sh = bl_shot(m, bolt_b(i))
  Bool.and(ne(sh, A_SPARK()), ne(sh, A_GLOAM()))

def sf_hits(+m: Mem, +i: U32) -> Bool:
  +fb = foe_b(i)
  +d = U32.sub(f_s(m, fb), anchor(m))
  Bool.and(Bool.and(ge(d, dn(50)), le(d, dm(600))), Bool.and(f_lockable(m, fb), ne(f_kind(m, fb), K_LAMP())))

def sf_loop(+fuel: Nat, +m: Mem, +i: U32, +by: U32, +live: Bool, +hits: Bool) -> Mem:
  match fuel live hits:
    case 0n _ _:
      m
    case 1n+p False{} _:
      m
    case 1n+p True{} False{}:
      sf_loop(p, m, U32.add(i, 1), by, lt(U32.add(i, 1), g_n_foes(m)), sf_hits(m, U32.add(i, 1)))
    case 1n+p True{} True{}:
      +m2 = hit_foe(m, i, STARFALL(), by)
      +i2 = pk(eq(g_reg(m2, 0), 1), i, U32.add(i, 1))
      sf_loop(p, m2, i2, by, lt(i2, g_n_foes(m2)), sf_hits(m2, i2))

# every hostile bolt is cleared, and everything small on the way is struck for what it can bear
def starfall(+m: Mem, +by: U32, +s_c: U32) -> Mem:
  +m1 = retain(~not_hostile, sound(spark(m, B_STARFALL(), s_c, 0, 0, by), SND_BOMB()), BOLTS(), BOLT_W(), g_n_bolts(m), CA_BOLTS())
  sf_loop(U32.to_nat(U32.add(U32.mul(2, g_n_foes(m1)), 1)), m1, 0, by, lt(0, g_n_foes(m1)), sf_hits(m1, 0))

# ===== the foes' tick =====

def HUNT_V() -> U32: dm(10)

def LATCH_BACK() -> U32: dm(18)
def GLOAM() -> U32: 25
def SPARK() -> U32: 10
def RAM() -> U32: 35

# the foe's move along its way
def f_move(+m: Mem, +fb: U32) -> Mem:
  set_f_z(set_f_x(set_f_s(m, fb, U32.add(f_s(m, fb), f_vs(m, fb))), fb, U32.add(f_x(m, fb), f_vx(m, fb))), fb, U32.add(f_z(m, fb), f_vz(m, fb)))

# a shooting wisp: a short tell, then a slow spark
def wisp_tell(+m: Mem, +fb: U32, +to: U32, +ahead_ok: Bool) -> Mem:
  match ahead_ok:
    case False{}:
      m
    case True{}:
      +cb = seat_c(to)
      set_f_ticks(set_f_st(set_f_fz(set_f_fx(m, fb, c_x(m, cb)), fb, c_z(m, cb)), fb, S_TELL()), fb, g_num_tell(m))

def wisp_aim(+m: Mem, +fb: U32) -> Mem:
  +ticks = ssub(f_ticks(m, fb), 1)
  +m1 = set_f_ticks(m, fb, ticks)
  +ahead = U32.sub(f_s(m1, fb), anchor(m1))
  +to = target(m1, f_id(m1, fb))
  wisp_tell(m1, fb, to, Bool.and(Bool.and(z(ticks), nz(to)), Bool.and(ge(ahead, dm(200)), le(ahead, dm(460)))))

def fly_turn(+m: Mem, +fb: U32, +flip: Bool) -> Mem: pkm(flip, set_f_vx(m, fb, ng(f_vx(m, fb))), m)

def fly_aim(+m: Mem, +fb: U32, +aim: Bool) -> Mem:
  match aim:
    case True{}:
      wisp_aim(m, fb)
    case False{}:
      m

def arm_fly(+m: Mem, +fb: U32) -> Mem:
  +m1 = f_move(m, fb)
  +flip = Bool.and(eq(f_mode(m1, fb), 1), z(U32.mod(U32.add(g_t(m1), f_phase(m1, fb)), 34)))
  +m2 = fly_turn(m1, fb, flip)
  fly_aim(m2, fb, Bool.and(eq(f_mode(m2, fb), 3), eq(f_kind(m2, fb), K_WISP())))

# a plane shot's three parts (across, up) for a bolt from (os, ox, oz) to (tx, tz)
def spit(+m: Mem, +shot: U32, +s: U32, +x: U32, +zz: U32, +tx: U32, +tz: U32, +closing: U32, +dmg: U32, +life: U32, +src: U32) -> Mem:
  +ticks = plane_ticks(m, s, closing)
  push_bolt(m, shot, 0, s, x, zz, U32.sub(PACE(), closing), sdv(U32.sub(tx, x), ticks), sdv(U32.sub(tz, zz), ticks), dmg, life, 0, src, 0, 0)

def wisp_fire(+m: Mem, +fb: U32) -> Mem:
  +m1 = spit(m, A_SPARK(), f_s(m, fb), f_x(m, fb), f_z(m, fb), f_fx(m, fb), f_fz(m, fb), dm(4), SPARK(), 220, f_id(m, fb))
  set_f_ticks(set_f_st(m1, fb, S_FLY()), fb, 100)

def wisp_go(+m: Mem, +fb: U32, +fire: Bool) -> Mem:
  match fire:
    case True{}:
      wisp_fire(m, fb)
    case False{}:
      m

def arm_wisp_tell(+m: Mem, +fb: U32) -> Mem:
  +m1 = f_move(m, fb)
  +ticks = ssub(f_ticks(m1, fb), 1)
  wisp_go(set_f_ticks(m1, fb, ticks), fb, z(ticks))

def arm_flee(+m: Mem, +fb: U32) -> Mem:
  +m1 = f_move(m, fb)
  +ticks = ssub(f_ticks(m1, fb), 1)
  +m2 = set_f_ticks(m1, fb, ticks)
  pkm(z(ticks), set_f_hp(m2, fb, 0), m2)

def turret_rest_go(+m: Mem, +fb: U32, +to: U32, +ok: Bool) -> Mem:
  match ok:
    case False{}:
      m
    case True{}:
      +cb = seat_c(to)
      sound(set_f_ticks(set_f_st(set_f_fz(set_f_fx(m, fb, c_x(m, cb)), fb, c_z(m, cb)), fb, S_TELL()), fb, g_num_tell(m)), SND_VENT_FIRE())

def arm_turret_rest(+m: Mem, +fb: U32) -> Mem:
  +ticks = ssub(f_ticks(m, fb), 1)
  +m1 = set_f_ticks(m, fb, ticks)
  +ahead = U32.sub(f_s(m1, fb), anchor(m1))
  +to = target(m1, U32.add(f_id(m1, fb), f_phase(m1, fb)))
  turret_rest_go(m1, fb, to, Bool.and(Bool.and(z(ticks), nz(to)), Bool.and(ge(ahead, dm(150)), le(ahead, dm(560)))))

# a turret fires on its locked marks
def fire_marks(+n: Nat, +m: Mem, +fb: U32, +i: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      fire_marks(p, spit(m, A_GLOAM(), f_s(m, fb), f_x(m, fb), U32.add(f_z(m, fb), dm(12)), mark_x(f_fx(m, fb), i), f_fz(m, fb), dm(5), GLOAM(), 260, f_id(m, fb)), fb, U32.add(i, 1))

def turret_fire(+m: Mem, +fb: U32, +go: Bool) -> Mem:
  match go:
    case False{}:
      m
    case True{}:
      +m1 = sound(fire_marks(nt(mark_count(pressure(m))), m, fb, 0), SND_SHOT_CROSSBOW())
      set_f_ticks(set_f_st(set_f_phase(m1, fb, U32.and(U32.add(f_phase(m1, fb), 1), 255)), fb, S_REST()), fb, 26)

def arm_turret_tell(+m: Mem, +fb: U32) -> Mem:
  +ticks = ssub(f_ticks(m, fb), 1)
  +m1 = set_f_ticks(m, fb, ticks)
  turret_fire(m1, fb, z(ticks))

def bomber_hide_go(+m: Mem, +fb: U32, +to: U32, +ok: Bool) -> Mem:
  match ok:
    case False{}:
      m
    case True{}:
      +cb = seat_c(to)
      set_f_ticks(set_f_st(set_f_fz(set_f_fx(m, fb, c_x(m, cb)), fb, c_z(m, cb)), fb, S_TELL()), fb, g_num_tell(m))

def arm_bomber_hide(+m: Mem, +fb: U32) -> Mem:
  +to = target(m, f_id(m, fb))
  bomber_hide_go(m, fb, to, Bool.and(le(U32.sub(f_s(m, fb), anchor(m)), dm(330)), nz(to)))

def bomber_dive(+m: Mem, +fb: U32, +go: Bool) -> Mem:
  match go:
    case False{}:
      m
    case True{}:
      +closing = sdv(mx(U32.sub(f_s(m, fb), anchor(m)), dm(60)), 32)
      +m1 = set_f_vs(m, fb, U32.sub(PACE(), closing))
      set_f_st(set_f_vz(set_f_vx(m1, fb, sdv(U32.sub(f_fx(m, fb), f_x(m, fb)), 32)), fb, sdv(U32.sub(f_fz(m, fb), f_z(m, fb)), 32)), fb, S_DIVE())

def BOSS_AT() -> U32: 480
def RISE_T() -> U32: 90

def rise_again(+m: Mem) -> Mem:
  +b = BOSSB()
  set_bo_nmarks(set_bo_ticks(set_bo_st(m, b, W_RISE()), b, RISE_T()), b, 0)

# ===== the Gloam Wyrm: three lamps in cold-light shields, three phases =====

def HEAD() -> U32: dm(340)

def FRONT() -> U32: dm(40)

# the lamps' places about the head
def lamp_x(+k: U32) -> U32: kind5(k, dn(45), dm(45), 0, 0, 0)

def HIGH() -> U32: dm(12)
def lamp_z(+k: U32) -> U32: kind5(k, dm(10), dm(10), dm(42), dm(42), dm(42))
def set_lamp_id(+m: Mem, +k: U32, +v: U32) -> Mem: wr(m, U32.add(U32.add(BOSSB(), 5), k), v)

def rise_lamp(+n: Nat, +m: Mem, +k: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      +m1 = spawn_foe(m, K_LAMP(), U32.sub(U32.add(anchor(m), HEAD()), FRONT()), lamp_x(k), U32.add(HIGH(), lamp_z(k)), 0, 0, 0, S_SHUT(), k, 0)
      rise_lamp(p, set_lamp_id(m1, k, g_reg(m1, 0)), U32.add(k, 1))

def rise_new(+m: Mem) -> Mem:
  +b = BOSSB()
  +m1 = rise_lamp(3n, set_g_boss(m, 1), 0)
  +m2 = set_bo_z(set_bo_x(set_bo_ticks(set_bo_st(set_bo_phase(m1, b, 0), b, W_RISE()), b, RISE_T()), b, 0), b, HIGH())
  +m3 = set_bo_seq(set_bo_atk(set_bo_col(set_bo_side(set_bo_gap(set_bo_nmarks(m2, b, 0), b, 0), b, 1), b, 0), b, 0), b, 0)
  sound(say(m3, voice(m3), L_WYRM()), SND_BOSS_RISE())

# the Wyrm rises out of the roost (or, after a fall, is back in its place, its lamps as they were)
def rise(+m: Mem) -> Mem:
  +m1 = set_g_cp(m, BOSS_AT())
  pkm(nz(g_boss(m1)), rise_again(m1), rise_new(m1))

# it rises over the ridge, then dives down the locked lane
def arm_bomber_tell(+m: Mem, +fb: U32) -> Mem:
  +rise = lt(f_z(m, fb), U32.add(mx(f_fz(m, fb), dm(10)), dm(24)))
  +m1 = set_f_z(m, fb, pk(rise, U32.add(f_z(m, fb), dm(1)), f_z(m, fb)))
  +ticks = ssub(f_ticks(m1, fb), 1)
  bomber_dive(set_f_ticks(m1, fb, ticks), fb, z(ticks))

def hunt_end(+m: Mem, +fb: U32, +tx: U32, +ty: U32, +tz: U32, +d0: U32, +d1: U32, +d2: U32, +len: U32, +near_: Bool) -> Mem:
  match near_:
    case True{}:
      +m1 = set_f_z(set_f_x(set_f_s(set_f_ticks(set_f_st(m, fb, S_LATCHED()), fb, 0), fb, tx), fb, ty), fb, tz)
      sound(say(m1, f_on(m, fb), L_LATCHED()), SND_CLANG())
    case False{}:
      +v0 = scs(d0, HUNT_V(), len)
      +v1 = scx(d1, HUNT_V(), len)
      +v2 = scx(d2, HUNT_V(), len)
      +m1 = set_f_vz(set_f_vx(set_f_vs(m, fb, v0), fb, v1), fb, v2)
      set_f_z(set_f_x(set_f_s(m1, fb, U32.add(f_s(m, fb), v0)), fb, U32.add(f_x(m, fb), v1)), fb, U32.add(f_z(m, fb), v2))

# a kite going for its victim: it picks one, closes, and latches onto the tail
def hunt_close(+m: Mem, +fb: U32, +vs: U32, +vx: U32, +vz: U32) -> Mem:
  +tx = U32.sub(vs, LATCH_BACK())
  +ty = vx
  +tz = U32.add(vz, dm(4))
  +d0 = U32.sub(tx, f_s(m, fb))
  +d1 = U32.sub(ty, f_x(m, fb))
  +d2 = U32.sub(tz, f_z(m, fb))
  +len = norm3(d0, d1, d2)
  hunt_end(m, fb, tx, ty, tz, d0, d1, d2, len, le(len, dm(9)))

def hunt_go(+m: Mem, +fb: U32) -> Mem:
  +cb = craft_of(m, f_on(m, fb))
  pkm(nz(cb), hunt_close(m, fb, cr_s(m, cb), c_x(m, cb), c_z(m, cb)), m)

def hunt_pick(+m: Mem, +fb: U32, +v: U32) -> Mem:
  pkm(z(v), set_f_vz(set_f_ticks(set_f_st(m, fb, S_FLEE()), fb, 40), fb, 60), hunt_go(set_f_on(m, fb, v), fb))

def hunt_held(+m: Mem, +fb: U32, +held: Bool) -> Mem:
  match held:
    case True{}:
      hunt_go(m, fb)
    case False{}:
      hunt_pick(m, fb, victim(m, f_id(m, fb)))

def arm_hunt(+m: Mem, +fb: U32) -> Mem:
  +on = f_on(m, fb)
  +cb = craft_of(m, on)
  hunt_held(m, fb, Bool.and(nz(on), Bool.and(nz(cb), cr_flying(m, on, cb))))

# a latched kite hangs on its victim's tail, draining its shield, until it is shot, or shaken off by a roll
def hang_go(+m: Mem, +fb: U32, +vs: U32, +vx: U32, +vz: U32, +drip: Bool) -> Mem:
  +m1 = set_f_z(set_f_x(set_f_s(m, fb, U32.sub(vs, LATCH_BACK())), fb, vx), fb, U32.add(vz, dm(4)))
  +ticks = U32.mod(U32.add(f_ticks(m1, fb), 1), 9996)
  +m2 = set_f_ticks(m1, fb, ticks)
  pkm(z(U32.mod(ticks, 12)), drain(m2, f_on(m2, fb), pk(le(f_on(m2, fb), 6), 2, 3)), m2)

def hang_flee(+m: Mem, +fb: U32, +vx: U32) -> Mem:
  set_f_vz(set_f_vx(set_f_vs(set_f_ticks(set_f_st(m, fb, S_FLEE()), fb, 40), fb, 0), fb, pk(ge(vx, 0), 90, ng(90))), fb, 70)

def hang_pick(+m: Mem, +fb: U32, +cb: U32, +gone: Bool, +off: Bool) -> Mem:
  match gone off:
    case True{} _:
      set_f_ticks(set_f_st(m, fb, S_FLEE()), fb, 40)
    case False{} True{}:
      hang_flee(m, fb, c_x(m, cb))
    case False{} False{}:
      hang_go(m, fb, cr_s(m, cb), c_x(m, cb), c_z(m, cb), False{})

def arm_hang(+m: Mem, +fb: U32) -> Mem:
  +on = f_on(m, fb)
  +cb = craft_of(m, on)
  +rolling = Bool.and(seat_ok(m, on), nz(c_roll(m, seat_c(on))))
  hang_pick(m, fb, cb, z(cb), Bool.or(Bool.not(cr_flying(m, on, cb)), rolling))

# a hit stops the foe a moment where it is, each foe by itself: the hit lands
def step_foe_go(+m: Mem, +fb: U32, +skip: Bool, +code: Nat) -> Mem:
  match skip code:
    case True{} _:
      m
    case False{} 0n:
      arm_fly(m, fb)
    case False{} 16n:
      arm_fly(m, fb)
    case False{} 1n:
      arm_wisp_tell(m, fb)
    case False{} 19n:
      arm_hunt(m, fb)
    case False{} 20n:
      arm_hang(m, fb)
    case False{} 5n:
      arm_flee(m, fb)
    case False{} 21n:
      arm_flee(m, fb)
    case False{} 37n:
      arm_flee(m, fb)
    case False{} 53n:
      arm_flee(m, fb)
    case False{} 69n:
      arm_flee(m, fb)
    case False{} 34n:
      arm_turret_rest(m, fb)
    case False{} 33n:
      arm_turret_tell(m, fb)
    case False{} 54n:
      arm_bomber_hide(m, fb)
    case False{} 49n:
      arm_bomber_tell(m, fb)
    case False{} 55n:
      f_move(m, fb)
    case False{} _:
      m

def step_foe(+m: Mem, +fb: U32) -> Mem:
  +st = f_st(m, fb)
  step_foe_go(m, fb, Bool.and(ge(f_flash(m, fb), 4), Bool.or(eq(st, S_FLY()), eq(st, S_HUNT()))), nt(U32.add(U32.mul(f_kind(m, fb), 16), st)))

def step_foes_loop(+n: Nat, +m: Mem, +i: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      step_foes_loop(p, step_foe(m, foe_b(i)), U32.add(i, 1))

# those that have gone by, or fled out of sight, are gone
def foe_keep(+m: Mem, +i: U32) -> Bool:
  +fb = foe_b(i)
  +a = anchor(m)
  Bool.or(eq(f_kind(m, fb), K_LAMP()), Bool.and(gt(f_hp(m, fb), 0), Bool.and(gt(f_s(m, fb), U32.sub(a, dm(300))), lt(f_s(m, fb), U32.add(a, dm(1500))))))

# ===== dive-bombers meet craft =====

# scratch past the session's words: whether a bolt died, the segment it flew, the bombers that rammed
def DEAD() -> U32: SCRATCH()

def SEG() -> U32: U32.add(SCRATCH(), 160)
def SPENT_N() -> U32: U32.add(SCRATCH(), 1280)
def SPENT() -> U32: U32.add(SCRATCH(), 1281)
def seg_b(+i: U32) -> U32: U32.add(SEG(), U32.mul(i, 7))
def sg_ok(+m: Mem, +i: U32) -> U32: rd(m, seg_b(i))
def sg_fs(+m: Mem, +i: U32) -> U32: rd(m, U32.add(seg_b(i), 1))
def sg_fx(+m: Mem, +i: U32) -> U32: rd(m, U32.add(seg_b(i), 2))
def sg_fz(+m: Mem, +i: U32) -> U32: rd(m, U32.add(seg_b(i), 3))
def sg_ts(+m: Mem, +i: U32) -> U32: rd(m, U32.add(seg_b(i), 4))
def sg_tx(+m: Mem, +i: U32) -> U32: rd(m, U32.add(seg_b(i), 5))
def sg_tz(+m: Mem, +i: U32) -> U32: rd(m, U32.add(seg_b(i), 6))
def dead_at(+m: Mem, +i: U32) -> Bool: nz(rd(m, U32.add(DEAD(), i)))
def set_dead(+m: Mem, +i: U32, +v: U32) -> Mem: wr(m, U32.add(DEAD(), i), v)

def set_seg(+m: Mem, +i: U32, +ok: U32, +fs: U32, +fx: U32, +fz: U32, +ts: U32, +tx: U32, +tz: U32) -> Mem:
  +b = seg_b(i)
  wr(wr(wr(wr(wr(wr(wr(m, b, ok), U32.add(b, 1), fs), U32.add(b, 2), fx), U32.add(b, 3), fz), U32.add(b, 4), ts), U32.add(b, 5), tx), U32.add(b, 6), tz)

# the first knight (by seat) whose craft the diving bomber at row fb meets this tick: 0 for none
def ram_hit(+m: Mem, +id: U32, +fb: U32) -> Bool:
  +cb = seat_c(id)
  Bool.and(Bool.and(seated(m, id), z(c_down(m, cb))),
    near(U32.sub(f_s(m, fb), f_vs(m, fb)), U32.sub(f_x(m, fb), f_vx(m, fb)), U32.sub(f_z(m, fb), f_vz(m, fb)), f_s(m, fb), f_x(m, fb), f_z(m, fb), cr_s(m, cb), c_x(m, cb), c_z(m, cb), U32.add(f_radius(m, fb), CRAFT_R())))

def first_ram(+n: Nat, +m: Mem, +id: U32, +fb: U32) -> U32:
  match n:
    case 0n:
      0
    case 1n+p:
      pk(ram_hit(m, id, fb), id, first_ram(p, m, U32.add(id, 1), fb))

def ram_note(+m: Mem, +fid: U32, +id: U32) -> Mem:
  +k = rd(m, SPENT_N())
  wr(wr(wr(m, SPENT_N(), U32.add(k, 1)), U32.add(SPENT(), U32.mul(k, 2)), fid), U32.add(SPENT(), U32.add(U32.mul(k, 2), 1)), id)

def ram_scan(+n: Nat, +m: Mem, +i: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      +fb = foe_b(i)
      +id = first_ram(6n, m, 1, fb)
      ram_scan(p, pkm(Bool.and(Bool.and(eq(f_kind(m, fb), K_BOMBER()), eq(f_st(m, fb), S_DIVE())), nz(id)), ram_note(m, f_id(m, fb), id), m), U32.add(i, 1))

def ram_pop(+m: Mem, +fid: U32) -> Mem:
  +i = foe_by_id(m, fid)
  +fb = foe_b(i)
  pkm(lt(i, 99), sound(spark(foe_remove(m, i), B_POP(), f_s(m, fb), f_x(m, fb), f_z(m, fb), 0), SND_BOMB_TOSS()), m)

def ram_apply(+n: Nat, +m: Mem, +k: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      +fid = rd(m, U32.add(SPENT(), U32.mul(k, 2)))
      +m1 = hurt_craft(m, rd(m, U32.add(SPENT(), U32.add(U32.mul(k, 2), 1))), RAM())
      ram_apply(p, pkm(eq(g_reg(m1, 0), 1), ram_pop(m1, fid), m1), U32.add(k, 1))

# a bomber that was thrown at a craft: a blow for the knight, and the bomber is spent
def bombers_ram(+m: Mem) -> Mem:
  +m1 = ram_scan(nt(g_n_foes(m)), wr(m, SPENT_N(), 0), 0)
  ram_apply(nt(rd(m1, SPENT_N())), m1, 0)

# the foes' tick: flights, tells, latching, dives
def step_foes(+m: Mem) -> Mem:
  +m1 = set_g_reg(step_foes_loop(nt(g_n_foes(m)), set_g_reg(m, 7, 1), 0), 7, 0)
  bombers_ram(retain(~foe_keep, m1, FOES(), FOE_W(), g_n_foes(m1), CA_FOES()))

# ===== a friendly bolt lands =====

def OPEN() -> U32: 150

def V_STAR() -> U32: dm(26)
def GLOAM_R() -> U32: dm(6)
def SPARK_R() -> U32: dm(5)
def BOLT_R() -> U32: dm(5)
def REFLECT_R() -> U32: dm(26)

# only a charged starburst breaks an open lamp
def land_star(+m: Mem, +fi: U32, +bi: U32, +at_s: U32, +at_x: U32, +at_z: U32, +full: Bool) -> Mem:
  match full:
    case True{}:
      spark(hit_foe(m, fi, bl_dmg(m, bolt_b(bi)), bl_owner(m, bolt_b(bi))), B_HIT(), at_s, at_x, at_z, 1)
    case False{}:
      sound(spark(set_f_flash(m, foe_b(fi), 4), B_HIT(), at_s, at_x, at_z, 0), SND_CLANG())

def land_lamp_go(+m: Mem, +fi: U32, +bi: U32, +code: Nat, +active: Bool, +at_s: U32, +at_x: U32, +at_z: U32) -> Mem:
  match code active:
    case 88n True{}:
      say(sound(spark(set_f_ticks(set_f_st(m, foe_b(fi), S_OPEN()), foe_b(fi), OPEN()), B_LAMP(), at_s, at_x, at_z, 1), SND_SHIELD_UP()), bl_owner(m, bolt_b(bi)), L_OPEN())
    case 89n _:
      set_f_ticks(m, foe_b(fi), OPEN())
    case 25n True{}:
      land_star(m, fi, bi, at_s, at_x, at_z, nz(bl_full(m, bolt_b(bi))))
    case _ _:
      sound(spark(set_f_flash(m, foe_b(fi), 4), B_HIT(), at_s, at_x, at_z, 0), SND_CLANG())

def land_lamp(+m: Mem, +fi: U32, +bi: U32, +active: Bool) -> Mem:
  +fb = foe_b(fi)
  +bb = bolt_b(bi)
  +shot = bl_shot(m, bb)
  +st = f_st(m, fb)
  +at_s = f_s(m, fb)
  +at_x = f_x(m, fb)
  +at_z = f_z(m, fb)
  land_lamp_go(m, fi, bi, nt(U32.add(U32.mul(shot, 16), st)), active, at_s, at_x, at_z)

def land_pick(+m: Mem, +fi: U32, +bi: U32, +lamp: Bool, +active: Bool) -> Mem:
  match lamp:
    case True{}:
      land_lamp(m, fi, bi, active)
    case False{}:
      +fb = foe_b(fi)
      +bb = bolt_b(bi)
      hit_foe(spark(m, B_HIT(), f_s(m, fb), f_x(m, fb), f_z(m, fb), 0), fi, pk(eq(bl_shot(m, bb), A_TURNED()), TURNED(), bl_dmg(m, bb)), bl_owner(m, bb))

# a friendly bolt lands on foe fi
def land(+m: Mem, +fi: U32, +bi: U32) -> Mem:
  +fb = foe_b(fi)
  +bb = bolt_b(bi)
  +lamp = eq(f_kind(m, fb), K_LAMP())
  +active = Bool.and(nz(g_boss(m)), eq(lit_lamp(m), f_id(m, fb)))
  land_pick(m, fi, bi, lamp, active)

# ===== a hostile bolt against the knights =====

def reflecting(+m: Mem, +cb: U32) -> Bool:
  Bool.and(nz(c_roll(m, cb)), U32.is_lt(U32.sub(c_roll_len(m, cb), c_roll(m, cb)), umx(ssub(g_num_roll(m), 10), 8)))

def hostile_a(+m: Mem, +bi: U32, +id: U32, +r: U32) -> Bool:
  +cb = seat_c(id)
  Bool.and(Bool.and(seated(m, id), z(c_down(m, cb))), Bool.and(reflecting(m, cb),
    near(sg_fs(m, bi), sg_fx(m, bi), sg_fz(m, bi), sg_ts(m, bi), sg_tx(m, bi), sg_tz(m, bi), cr_s(m, cb), c_x(m, cb), c_z(m, cb), U32.add(REFLECT_R(), r))))

def hostile_h(+m: Mem, +bi: U32, +id: U32, +r: U32) -> Bool:
  +cb = seat_c(id)
  Bool.and(Bool.and(seated(m, id), z(c_down(m, cb))), Bool.and(z(c_hurt(m, cb)),
    near(sg_fs(m, bi), sg_fx(m, bi), sg_fz(m, bi), sg_ts(m, bi), sg_tx(m, bi), sg_tz(m, bi), cr_s(m, cb), c_x(m, cb), c_z(m, cb), U32.add(CRAFT_R(), r))))

def hostile_loop(+n: Nat, +m: Mem, +bi: U32, +id: U32, +r: U32, +a: Bool, +h: Bool) -> Mem:
  match n a h:
    case 0n _ _:
      set_g_reg(m, 0, 1)
    case 1n+p True{} _:
      set_g_reg(turn(m, bi, id), 0, 1)
    case 1n+p False{} True{}:
      +bb = bolt_b(bi)
      set_g_reg(spark(hurt_craft(m, id, bl_dmg(m, bb)), B_HIT(), bl_s(m, bb), bl_x(m, bb), bl_z(m, bb), 0), 0, 0)
    case 1n+p False{} False{}:
      hostile_loop(p, m, bi, U32.add(id, 1), r, hostile_a(m, bi, U32.add(id, 1), r), hostile_h(m, bi, U32.add(id, 1), r))

# whether the bolt flies on (reg 0)
def hostile_bolt(+m: Mem, +bi: U32) -> Mem:
  +r = pk(eq(bl_shot(m, bolt_b(bi)), A_GLOAM()), GLOAM_R(), SPARK_R())
  hostile_loop(6n, m, bi, 1, r, hostile_a(m, bi, 1, r), hostile_h(m, bi, 1, r))

# ===== a friendly bolt against the foes and the hostile sparks =====

def fr_hit(+m: Mem, +bi: U32, +i: U32) -> Bool:
  +fb = foe_b(i)
  +bb = bolt_b(bi)
  Bool.and(Bool.and(f_alive(m, fb), ne(f_st(m, fb), S_HIDE())),
    Bool.and(Bool.or(Bool.not(Bool.and(eq(bl_shot(m, bb), A_STAR()), nz(bl_aim(m, bb)))), eq(f_id(m, fb), bl_aim(m, bb))),
      near(sg_fs(m, bi), sg_fx(m, bi), sg_fz(m, bi), sg_ts(m, bi), sg_tx(m, bi), sg_tz(m, bi), f_s(m, fb), f_x(m, fb), f_z(m, fb), U32.add(f_radius(m, fb), BOLT_R()))))

# the foe a friendly bolt hits first: the one it passes nearest to its start
def best_foe(+n: Nat, +m: Mem, +bi: U32, +i: U32, +best: U32, +bd: U32) -> U32:
  match n:
    case 0n:
      best
    case 1n+p:
      +fb = foe_b(i)
      +d = norm3(U32.sub(f_s(m, fb), sg_fs(m, bi)), U32.sub(f_x(m, fb), sg_fx(m, bi)), U32.sub(f_z(m, fb), sg_fz(m, bi)))
      +take = Bool.and(fr_hit(m, bi, i), Bool.or(eq(best, 99), lt(d, bd)))
      best_foe(p, m, bi, U32.add(i, 1), pk(take, i, best), pk(take, d, bd))

# the first hostile spark the bolt passes through
def spark_hit(+n: Nat, +m: Mem, +bi: U32, +j: U32) -> U32:
  match n:
    case 0n:
      99
    case 1n+p:
      +hb = bolt_b(j)
      pk(Bool.and(Bool.and(Bool.not(dead_at(m, j)), eq(bl_shot(m, hb), A_SPARK())),
        near(sg_fs(m, bi), sg_fx(m, bi), sg_fz(m, bi), sg_ts(m, bi), sg_tx(m, bi), sg_tz(m, bi), bl_s(m, hb), bl_x(m, hb), bl_z(m, hb), U32.add(SPARK_R(), BOLT_R()))), j, spark_hit(p, m, bi, U32.add(j, 1)))

def friendly_spark(+m: Mem, +j: U32) -> Mem:
  +hb = bolt_b(j)
  spark(set_dead(m, j, 1), B_HIT(), bl_s(m, hb), bl_x(m, hb), bl_z(m, hb), 0)

def friendly_spark_pick(+m: Mem, +j: U32, +ok: Bool) -> Mem:
  match ok:
    case True{}:
      set_g_reg(friendly_spark(m, j), 0, 0)
    case False{}:
      set_g_reg(m, 0, 1)

def friendly_pick(+m: Mem, +bi: U32, +i: U32, +hit: Bool) -> Mem:
  match hit:
    case True{}:
      set_g_reg(land(m, i, bi), 0, 0)
    case False{}:
      +j = spark_hit(nt(g_n_bolts(m)), m, bi, 0)
      friendly_spark_pick(m, j, lt(j, 99))

# whether the bolt flies on (reg 0)
def friendly_bolt(+m: Mem, +bi: U32) -> Mem:
  +i = best_foe(nt(g_n_foes(m)), m, bi, 0, 99, 0)
  friendly_pick(m, bi, i, lt(i, 99))

# ===== the bolts' tick: they fly and home, then land =====

# a starburst and a turned bolt close on their body
def home_to(+m: Mem, +bi: U32, +fi: U32) -> Mem:
  +bb = bolt_b(bi)
  +fb = foe_b(fi)
  +speed = pk(eq(bl_shot(m, bb), A_TURNED()), V_TURN(), V_STAR())
  +d0 = U32.sub(f_s(m, fb), bl_s(m, bb))
  +d1 = U32.sub(f_x(m, fb), bl_x(m, bb))
  +d2 = U32.sub(f_z(m, fb), bl_z(m, bb))
  +len = norm3(d0, d1, d2)
  set_bl_vz(set_bl_vx(set_bl_vs(m, bb, U32.add(bl_vs(m, bb), sdv(U32.sub(scs(d0, speed, len), bl_vs(m, bb)), 3))), bb, U32.add(bl_vx(m, bb), sdv(U32.sub(scx(d1, speed, len), bl_vx(m, bb)), 3))), bb, U32.add(bl_vz(m, bb), sdv(U32.sub(scx(d2, speed, len), bl_vz(m, bb)), 3)))

# its body is gone: on to the nearest ahead
def home_seek(+m: Mem, +bi: U32) -> Mem:
  +bb = bolt_b(bi)
  +ti = near_go(nt(g_n_foes(m)), m, 0, bl_s(m, bb), bl_x(m, bb), bl_z(m, bb), dm(20), dm(500), 99, 0)
  set_bl_aim(m, bb, pk(lt(ti, 99), f_id(m, foe_b(ti)), 0))

def home_pick(+m: Mem, +bi: U32, +fi: U32, +found: Bool) -> Mem:
  match found:
    case True{}:
      home_to(m, bi, fi)
    case False{}:
      home_seek(m, bi)

def home(+m: Mem, +bi: U32, +go: Bool) -> Mem:
  match go:
    case False{}:
      m
    case True{}:
      +fi = foe_idx_alive(nt(g_n_foes(m)), m, 0, bl_aim(m, bolt_b(bi)))
      home_pick(m, bi, fi, lt(fi, 99))

def fly_bolt(+m: Mem, +bi: U32) -> Mem:
  +bb = bolt_b(bi)
  +s = bl_s(m, bb)
  +x = bl_x(m, bb)
  +zz = bl_z(m, bb)
  +ns = U32.add(s, bl_vs(m, bb))
  +nx = U32.add(x, bl_vx(m, bb))
  +nz_ = U32.add(zz, bl_vz(m, bb))
  set_seg(set_bl_life(set_bl_z(set_bl_x(set_bl_s(m, bb, ns), bb, nx), bb, nz_), bb, ssub(bl_life(m, bb), 1)), bi, 1, s, x, zz, ns, nx, nz_)

# a released starburst waits at its craft's muzzle for its turn
def wait_bolt(+m: Mem, +bi: U32, +ok: Bool) -> Mem:
  match ok:
    case False{}:
      set_seg(set_dead(m, bi, 1), bi, 0, 0, 0, 0, 0, 0, 0)
    case True{}:
      +bb = bolt_b(bi)
      +cb = craft_of(m, bl_owner(m, bb))
      set_seg(set_dead(set_bl_delay(set_bl_z(set_bl_x(set_bl_s(m, bb, U32.add(cr_s(m, cb), dm(12))), bb, c_x(m, cb)), bb, c_z(m, cb)), bb, U32.sub(bl_delay(m, bb), 1)), bi, 0), bi, 0, 0, 0, 0, 0, 0, 0)

def bolt_move_go(+m: Mem, +bi: U32, +waiting: Bool) -> Mem:
  match waiting:
    case True{}:
      +bb = bolt_b(bi)
      +owner = bl_owner(m, bb)
      +cb = craft_of(m, owner)
      wait_bolt(m, bi, Bool.and(nz(cb), cr_flying(m, owner, cb)))
    case False{}:
      +bb = bolt_b(bi)
      +shot = bl_shot(m, bb)
      fly_bolt(home(set_dead(m, bi, 0), bi, Bool.and(nz(bl_aim(m, bb)), Bool.or(eq(shot, A_STAR()), eq(shot, A_TURNED())))), bi)

def bolt_moves(+n: Nat, +m: Mem, +i: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      bolt_moves(p, bolt_move_go(m, i, nz(bl_delay(m, bolt_b(i)))), U32.add(i, 1))

def bolt_land_end(+m: Mem, +i: U32) -> Mem: pkm(eq(g_reg(m, 0), 1), m, set_dead(m, i, 1))

def bolt_land_kind(+m: Mem, +i: U32, +hostile: Bool) -> Mem:
  match hostile:
    case True{}:
      bolt_land_end(hostile_bolt(m, i), i)
    case False{}:
      bolt_land_end(friendly_bolt(m, i), i)

def bolt_land_go(+m: Mem, +i: U32, +live: Bool) -> Mem:
  match live:
    case False{}:
      m
    case True{}:
      +shot = bl_shot(m, bolt_b(i))
      bolt_land_kind(m, i, Bool.or(eq(shot, A_SPARK()), eq(shot, A_GLOAM())))

def bolt_lands(+n: Nat, +m: Mem, +i: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      bolt_lands(p, bolt_land_go(m, i, Bool.and(nz(sg_ok(m, i)), Bool.not(dead_at(m, i)))), U32.add(i, 1))

def bolt_keep(+m: Mem, +i: U32) -> Bool:
  +bb = bolt_b(i)
  +a = anchor(m)
  Bool.and(Bool.and(Bool.not(dead_at(m, i)), nz(bl_life(m, bb))), Bool.and(ge(bl_s(m, bb), U32.sub(a, dm(80))), le(bl_s(m, bb), U32.add(a, dm(1200)))))

def step_bolts(+m: Mem) -> Mem:
  +m1 = bolt_lands(nt(g_n_bolts(m)), bolt_moves(nt(g_n_bolts(m)), m, 0), 0)
  retain(~bolt_keep, m1, BOLTS(), BOLT_W(), g_n_bolts(m1), CA_BOLTS())

# ===== the craft in the air =====

def K_FIRE() -> U32: 1

def K_DASH() -> U32: 2
def K_BOMB() -> U32: 4
def K_SPUR() -> U32: 8
def V_X() -> U32: 62
def V_Z() -> U32: 50
def ACC() -> U32: 14
def RET_V() -> U32: 100
def RX_MAX() -> U32: dm(120)
def RZ_MAX() -> U32: dm(80)
def TAP_REST() -> U32: 6
def HOLD_AT() -> U32: 7
def REL_REST() -> U32: 12
def LOCK_R() -> U32: dm(16)
def V_BOLT() -> U32: dm(22)
def BOLT_LIFE() -> U32: 26
def STAR_LIFE() -> U32: 46
def LANCE() -> U32: 10
def BOOST_BACK() -> U32: dm(55)
def BRAKE_BACK() -> U32: dm(45)
def BACK_V() -> U32: 28
def ROLL_REST() -> U32: 12
def ROLL_V() -> U32: 84
def TAP_WINDOW() -> U32: 9

# a seat's place in the formation: across and up
def slot_x(+id: U32) -> U32: pk(ge(id, 6), 0, kind5(U32.sub(id, 1), 0, dn(22), dm(22), dn(40), dm(40)))

def slot_z(+id: U32) -> U32: pk(ge(id, 6), dm(12), kind5(U32.sub(id, 1), 0, dn(4), dn(4), dn(8), dn(8)))
def k_move_x(+m: Mem, +id: U32) -> U32: key_at(m, id, 0)
def k_move_y(+m: Mem, +id: U32) -> U32: key_at(m, id, 1)
def k_aim_x(+m: Mem, +id: U32) -> U32: key_at(m, id, 2)
def k_aim_y(+m: Mem, +id: U32) -> U32: key_at(m, id, 3)
def k_fire(+m: Mem, +id: U32) -> Bool: nz(key_at(m, id, 4))
def k_dash(+m: Mem, +id: U32) -> Bool: nz(key_at(m, id, 5))
def k_bomb(+m: Mem, +id: U32) -> Bool: nz(key_at(m, id, 6))
def k_interact(+m: Mem, +id: U32) -> Bool: nz(key_at(m, id, 7))
def k_swing(+m: Mem, +id: U32) -> Bool: nz(key_at(m, id, 8))

# the keys as bits, for the next tick's presses
def key_bits(+m: Mem, +id: U32) -> U32:
  U32.or(U32.or(U32.or(b2u(k_fire(m, id)), U32.shln(b2u(k_dash(m, id)), 1n)), U32.or(U32.shln(b2u(k_bomb(m, id)), 2n), U32.shln(b2u(k_interact(m, id)), 3n))), U32.shln(b2u(k_swing(m, id)), 4n))

# a seat's bit
def bit(+id: U32) -> U32: U32.shln(1, U32.to_nat(U32.sub(umn(umx(id, 1), 8), 1)))

# whether a key is down now and was not held last tick
def edge(+m: Mem, +id: U32, +bit: U32, +on: Bool) -> Bool: Bool.and(on, z(U32.and(c_keys(m, seat_c(id)), bit)))

# the timers wind down
def fly_timers(+m: Mem, +cb: U32) -> Mem:
  +m1 = set_c_rest(set_c_cool(set_c_hurt(m, cb, ssub(c_hurt(m, cb), 1)), cb, ssub(c_cool(m, cb), 1)), cb, ssub(c_rest(m, cb), 1))
  +ct = ssub(c_chain_ticks(m1, cb), 1)
  set_c_tap_ticks(set_c_chain(set_c_chain_ticks(m1, cb, ct), cb, pk(z(ct), 0, c_chain(m1, cb))), cb, ssub(c_tap_ticks(m1, cb), 1))

# a knight who is down falls and, when the time comes, rises again at the formation
def fly_up(+m: Mem, +id: U32, +cb: U32) -> Mem:
  set_c_roll(set_c_hurt(set_c_shield(set_c_vz(set_c_vx(set_c_z(set_c_x(m, cb, slot_x(id)), cb, slot_z(id)), cb, 0), cb, 0), cb, 60), cb, 60), cb, 0)

def fly_down(+m: Mem, +id: U32) -> Mem:
  +cb = seat_c(id)
  +d = U32.sub(c_down(m, cb), 1)
  +m1 = set_c_nlocks(set_c_charge(set_c_held(set_c_ds(set_c_down(m, cb, d), cb, PACE()), cb, 0), cb, 0), cb, 0)
  pkm(z(d), fly_up(m1, id, cb), m1)

# a second tap of a direction is a roll that way: reg 1 holds whether this tick is one
def tap_go(+m: Mem, +cb: U32, +mxv: U32, +hit: Bool) -> Mem:
  match hit:
    case True{}:
      set_c_tap_ticks(set_g_reg(m, 1, 1), cb, 0)
    case False{}:
      set_c_tap_ticks(set_c_tap_dir(m, cb, mxv), cb, TAP_WINDOW())

def fly_double(+m: Mem, +id: U32) -> Mem:
  +cb = seat_c(id)
  +mxv = k_move_x(m, id)
  +m1 = set_g_reg(m, 1, 0)
  +fresh = Bool.and(nz(mxv), ne(c_last_x(m, cb), mxv))
  +hit = Bool.and(eq(c_tap_dir(m, cb), mxv), nz(c_tap_ticks(m, cb)))
  set_c_last_x(pkm(fresh, tap_go(m1, cb, mxv, hit), m1), cb, mxv)

def roll_start(+m: Mem, +id: U32, +cb: U32) -> Mem:
  +mxv = k_move_x(m, id)
  +vx = c_vx(m, cb)
  +dir = pk(nz(mxv), sg(mxv), pk(gt(ab(vx), 8), sg(vx), c_dir(m, cb)))
  +nr = g_num_roll(m)
  +m1 = set_c_vx(set_c_rest(set_c_roll_len(set_c_roll(set_c_dir(m, cb, dir), cb, nr), cb, nr), cb, U32.add(nr, ROLL_REST())), cb, U32.mul(dir, ROLL_V()))
  sound(set_c_nlocks(set_c_charge(set_c_held(m1, cb, 0), cb, 0), cb, 0), SND_ROLL())

def fly_roll(+m: Mem, +id: U32, +double: Bool) -> Mem:
  +cb = seat_c(id)
  +go = Bool.and(Bool.or(edge(m, id, K_DASH(), k_dash(m, id)), double), Bool.and(z(c_rest(m, cb)), z(c_roll(m, cb))))
  +m1 = pkm(go, roll_start(m, id, cb), m)
  set_c_roll(m1, cb, ssub(c_roll(m1, cb), 1))

# the reins: the craft's speed toward where the stick points, then its place held inside the window
def fly_reins(+m: Mem, +id: U32) -> Mem:
  +cb = seat_c(id)
  +tx = U32.mul(k_move_x(m, id), V_X())
  +tz = ng(U32.mul(k_move_y(m, id), V_Z()))
  +vx = U32.add(c_vx(m, cb), cl(U32.sub(tx, c_vx(m, cb)), ng(ACC()), ACC()))
  +vz = U32.add(c_vz(m, cb), cl(U32.sub(tz, c_vz(m, cb)), ng(ACC()), ACC()))
  +x = U32.add(c_x(m, cb), vx)
  +zz = U32.add(c_z(m, cb), vz)
  +xo = gt(ab(x), wx(m))
  +zo = gt(ab(zz), wz(m))
  +x2 = pk(xo, U32.mul(wx(m), sg(x)), x)
  +z2 = pk(zo, U32.mul(wz(m), sg(zz)), zz)
  +vx2 = pk(xo, 0, vx)
  +vz2 = pk(zo, 0, vz)
  +bank = U32.add(c_bank(m, cb), sdv(U32.sub(sdv(U32.mul(vx2, Q()), V_X()), c_bank(m, cb)), 4))
  +lift = U32.add(c_lift(m, cb), sdv(U32.sub(sdv(U32.mul(vz2, Q()), V_Z()), c_lift(m, cb)), 4))
  set_c_lift(set_c_bank(set_c_vz(set_c_vx(set_c_z(set_c_x(m, cb, x2), cb, z2), cb, vx2), cb, vz2), cb, bank), cb, lift)

# the spur on a short meter, and the brake
def fly_spur(+m: Mem, +id: U32) -> Mem:
  +cb = seat_c(id)
  +sp = umx(g_num_spur(m), 1)
  +drain = U32.div(METER(), sp)
  +refill = U32.div(METER(), umx(U32.div(U32.mul(sp, 5), 2), 1))
  +boost = Bool.and(Bool.and(k_interact(m, id), z(c_spent(m, cb))), Bool.and(gt(c_meter(m, cb), 0), z(c_roll(m, cb))))
  +brake = Bool.and(k_swing(m, id), Bool.not(boost))
  +meter0 = c_meter(m, cb)
  +meter_b = mx(U32.sub(meter0, drain), 0)
  +meter_r = mn(U32.add(meter0, refill), METER())
  +meter = pk(boost, meter_b, meter_r)
  +spent = pk(boost, pk(z(meter_b), 1, c_spent(m, cb)), pk(Bool.and(nz(c_spent(m, cb)), ge(meter_r, U32.div(METER(), 4))), 0, c_spent(m, cb)))
  +want = pk(boost, BOOST_BACK(), pk(brake, ng(BRAKE_BACK()), 0))
  +back0 = c_back(m, cb)
  +back = U32.add(back0, cl(U32.sub(want, back0), ng(BACK_V()), BACK_V()))
  set_c_back(set_c_ds(set_c_brake(set_c_boost(set_c_spent(set_c_meter(m, cb, meter), cb, spent), cb, b2u(boost)), cb, b2u(brake)), cb, U32.add(PACE(), U32.sub(back, back0))), cb, back)

def fly_reticle(+m: Mem, +id: U32) -> Mem:
  +cb = seat_c(id)
  set_c_rz(set_c_rx(m, cb, cl(U32.add(c_rx(m, cb), U32.mul(k_aim_x(m, id), RET_V())), ng(RX_MAX()), RX_MAX())), cb, cl(U32.sub(c_rz(m, cb), U32.mul(k_aim_y(m, id), RET_V())), ng(RZ_MAX()), RZ_MAX()))

# a tap: two bolts, side by side, both to where the reticle points
def tap_bolt(+m: Mem, +id: U32, +cb: U32, +s_c: U32, +side: U32) -> Mem:
  +fs = U32.add(s_c, dm(12))
  +fx = U32.add(c_x(m, cb), U32.mul(side, dm(4)))
  +fz = c_z(m, cb)
  +d0 = U32.sub(U32.add(s_c, FAR()), fs)
  +d1 = U32.sub(U32.add(c_x(m, cb), c_rx(m, cb)), fx)
  +d2 = U32.sub(U32.add(c_z(m, cb), c_rz(m, cb)), fz)
  +len = norm3(d0, d1, d2)
  push_bolt(m, A_LANCE(), id, fs, fx, fz, U32.add(PACE(), scs(d0, V_BOLT(), len)), scx(d1, V_BOLT(), len), scx(d2, V_BOLT(), len), LANCE(), BOLT_LIFE(), 0, 0, 0, 0)

def tap(+m: Mem, +id: U32, +cb: U32, +s_c: U32) -> Mem:
  sound(tap_bolt(tap_bolt(m, id, cb, s_c, ng(1)), id, cb, s_c, 1), SND_SHOT_BOLT())

# ----- the lock: everything the reticle sweeps over while F is held -----

# the lock this foe holds on the craft: its index in the craft's list, 99 for none
def lock_idx(+n: Nat, +m: Mem, +cb: U32, +k: U32, +id: U32) -> U32:
  match n:
    case 0n:
      99
    case 1n+p:
      pk(eq(c_lock_foe(m, cb, k), id), k, lock_idx(p, m, cb, U32.add(k, 1), id))

def lock_total(+n: Nat, +m: Mem, +cb: U32, +k: U32) -> U32:
  match n:
    case 0n:
      0
    case 1n+p:
      U32.add(c_lock_n(m, cb, k), lock_total(p, m, cb, U32.add(k, 1)))

# a lock is kept while its foe is still lockable and in reach: reg 1 holds the craft, reg 2 its place on the rail
def lock_keep(+m: Mem, +k: U32) -> Bool:
  +cb = g_reg(m, 1)
  +fi = foe_by_id(m, c_lock_foe(m, cb, k))
  +fb = foe_b(fi)
  +ds = U32.sub(f_s(m, fb), g_reg(m, 2))
  Bool.and(lt(fi, 99), Bool.and(f_lockable(m, fb), Bool.and(ge(ds, LOCK_MIN()), le(ds, U32.add(LOCK_MAX(), dm(40))))))

# a foe the reticle sweeps over takes a bracket (or a lamp, held on, a second and a third)
def lock_new(+m: Mem, +cb: U32, +fb: U32, +s: U32, +x: U32, +zz: U32) -> Mem:
  +k = c_nlocks(m, cb)
  +total = U32.add(lock_total(nt(k), m, cb, 0), 1)
  spark(sound(set_c_dwell(set_c_nlocks(set_c_lock_n(set_c_lock_foe(m, cb, k, f_id(m, fb)), cb, k, 1), cb, U32.add(k, 1)), cb, 0), SND_SPOIL_PICKUP()), B_LOCK(), s, x, zz, total)

def lock_more(+m: Mem, +cb: U32, +li: U32, +ok: Bool) -> Mem:
  match ok:
    case False{}:
      m
    case True{}:
      +d = U32.add(c_dwell(m, cb), 1)
      +m1 = set_g_reg(set_c_dwell(m, cb, d), 4, 1)
      pkm(ge(d, 8), sound(set_c_dwell(set_c_lock_n(m1, cb, li, U32.add(c_lock_n(m1, cb, li), 1)), cb, 0), SND_SPOIL_PICKUP()), m1)

def lock_foe_go(+m: Mem, +cb: U32, +fb: U32, +li: U32, +hit: Bool) -> Mem:
  match hit:
    case False{}:
      m
    case True{}:
      +total = lock_total(nt(c_nlocks(m, cb)), m, cb, 0)
      pkm(lt(li, 99),
        lock_more(m, cb, li, Bool.and(eq(f_kind(m, fb), K_LAMP()), Bool.and(lt(c_lock_n(m, cb, li), 3), lt(total, 6)))),
        pkm(lt(total, 6), lock_new(m, cb, fb, f_s(m, fb), f_x(m, fb), f_z(m, fb)), m))

def lock_foes(+n: Nat, +m: Mem, +cb: U32, +i: U32, +s_c: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      +fb = foe_b(i)
      +ds = U32.sub(f_s(m, fb), s_c)
      +ax = sdv(U32.mul(c_rx(m, cb), ds), FAR())
      +az = sdv(U32.mul(c_rz(m, cb), ds), FAR())
      +hit = Bool.and(Bool.and(f_lockable(m, fb), Bool.and(ge(ds, LOCK_MIN()), le(ds, LOCK_MAX()))),
        le(flat(U32.sub(f_x(m, fb), c_x(m, cb)), U32.sub(f_z(m, fb), c_z(m, cb)), ax, az), U32.add(f_radius(m, fb), LOCK_R())))
      lock_foes(p, lock_foe_go(m, cb, fb, lock_idx(nt(c_nlocks(m, cb)), m, cb, 0, f_id(m, fb)), hit), cb, U32.add(i, 1), s_c)

# everything the reticle sweeps over while F is held is locked: a bracket on each, snapping on
def acquire(+m: Mem, +cb: U32, +s_c: U32) -> Mem:
  +m1 = retain(~lock_keep, set_g_reg(set_g_reg(m, 1, cb), 2, s_c), U32.add(cb, 21), 2, c_nlocks(m, cb), U32.add(cb, 20))
  +m2 = lock_foes(nt(g_n_foes(m1)), set_g_reg(m1, 4, 0), cb, 0, s_c)
  pkm(z(g_reg(m2, 4)), set_c_dwell(m2, cb, 0), m2)

# ----- letting go: a starburst for every lock, a chain of pops two ticks apart, homing on their bodies -----

def spread_vx(+k: U32) -> U32: kind5(U32.mod(k, 6), ng(70), 70, 0, ng(40), pk(eq(U32.mod(k, 6), 4), 40, 0))

def spread_vz(+k: U32) -> U32: kind5(U32.mod(k, 6), 40, 40, 90, ng(40), pk(eq(U32.mod(k, 6), 4), ng(40), 0))

def burst_go(+n: Nat, +m: Mem, +id: U32, +cb: U32, +s_c: U32, +foe: U32, +k: U32, +dmg: U32, +full: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      burst_go(p, push_bolt(m, A_STAR(), id, U32.add(s_c, dm(12)), c_x(m, cb), c_z(m, cb), U32.add(PACE(), 120), spread_vx(k), spread_vz(k), dmg, STAR_LIFE(), foe, 0, U32.mul(k, 2), full), id, cb, s_c, foe, U32.add(k, 1), dmg, full)

def release_locks(+n: Nat, +m: Mem, +id: U32, +cb: U32, +s_c: U32, +li: U32, +k: U32, +dmg: U32, +full: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      +cnt = c_lock_n(m, cb, li)
      release_locks(p, burst_go(nt(cnt), m, id, cb, s_c, c_lock_foe(m, cb, li), k, dmg, full), id, cb, s_c, U32.add(li, 1), U32.add(k, cnt), dmg, full)

def release_end(+m: Mem, +cb: U32, +k: U32) -> Mem:
  pkm(gt(k, 0), sound(set_c_cool(m, cb, REL_REST()), SND_SHOT_SPARK()), m)

def release(+m: Mem, +id: U32, +cb: U32, +s_c: U32) -> Mem:
  +ch = g_num_charge(m)
  +full = ge(c_charge(m, cb), ch)
  +dmg = U32.add(16, U32.div(U32.mul(20, umn(c_charge(m, cb), ch)), umx(ch, 1)))
  release_end(release_locks(nt(c_nlocks(m, cb)), m, id, cb, s_c, 0, 0, dmg, b2u(full)), cb, lock_total(nt(c_nlocks(m, cb)), m, cb, 0))

# ----- the fire key -----

def fire_held(+m: Mem, +id: U32, +cb: U32, +s_c: U32, +over: Bool) -> Mem:
  match over:
    case False{}:
      m
    case True{}:
      acquire(set_c_charge(m, cb, umn(U32.add(c_charge(m, cb), 1), g_num_charge(m))), cb, s_c)

def fire_tap(+m: Mem, +id: U32, +cb: U32, +s_c: U32, +ready: Bool) -> Mem:
  match ready:
    case False{}:
      m
    case True{}:
      set_c_cool(tap(m, id, cb, s_c), cb, TAP_REST())

def fire_down(+m: Mem, +id: U32, +cb: U32, +s_c: U32, +press: Bool) -> Mem:
  match press:
    case True{}:
      fire_tap(set_c_held(m, cb, 1), id, cb, s_c, z(c_cool(m, cb)))
    case False{}:
      +h = c_held(m, cb)
      pkm(gt(h, 0), fire_held(set_c_held(m, cb, umn(U32.add(h, 1), 30000)), id, cb, s_c, U32.is_gt(umn(U32.add(h, 1), 30000), HOLD_AT())), m)

def fire_up(+m: Mem, +id: U32, +cb: U32, +s_c: U32, +letgo: Bool) -> Mem:
  match letgo:
    case False{}:
      m
    case True{}:
      +m1 = pkm(Bool.and(nz(c_charge(m, cb)), z(c_roll(m, cb))), release(m, id, cb, s_c), m)
      set_c_nlocks(set_c_dwell(set_c_charge(set_c_held(m1, cb, 0), cb, 0), cb, 0), cb, 0)

def fly_fire_go(+m: Mem, +id: U32, +cb: U32, +s_c: U32, +hold: Bool, +press: Bool, +letgo: Bool) -> Mem:
  match hold:
    case True{}:
      fire_down(m, id, cb, s_c, press)
    case False{}:
      fire_up(m, id, cb, s_c, letgo)

def fly_fire(+m: Mem, +id: U32) -> Mem:
  +cb = seat_c(id)
  +s_c = cr_s(m, cb)
  +hold = Bool.and(z(c_roll(m, cb)), k_fire(m, id))
  fly_fire_go(m, id, cb, s_c, hold, edge(m, id, K_FIRE(), True{}), Bool.and(Bool.not(hold), nz(c_held(m, cb))))

# Starfall: the kills it makes are scored on the craft as it stood before this tick (reg 5, 6: its chain and
# the chain's clock then), and the knight's own tally, flown on a copy, is put back over them
def bomb_go(+m: Mem, +id: U32, +cb: U32, +s_c: U32) -> Mem:
  +chain = c_chain(m, cb)
  +ct = c_chain_ticks(m, cb)
  +hits = c_hits(m, cb)
  +pts = c_pts(m, cb)
  +saved = c_saved(m, cb)
  +m0 = set_c_chain(set_c_chain_ticks(set_c_bombs(m, cb, U32.sub(c_bombs(m, cb), 1)), cb, g_reg(m, 6)), cb, g_reg(m, 5))
  +m1 = starfall(m0, id, s_c)
  set_c_saved(set_c_pts(set_c_hits(set_c_chain_ticks(set_c_chain(m1, cb, chain), cb, ct), cb, hits), cb, pts), cb, saved)

def fly_bomb(+m: Mem, +id: U32) -> Mem:
  +cb = seat_c(id)
  pkm(Bool.and(Bool.and(edge(m, id, K_BOMB(), k_bomb(m, id)), gt(c_bombs(m, cb), 0)), z(c_roll(m, cb))), bomb_go(m, id, cb, cr_s(m, cb)), m)

# one knight's tick in the air
def fly_live(+m: Mem, +id: U32) -> Mem:
  +m1 = fly_double(m, id)
  +m2 = fly_roll(m1, id, eq(g_reg(m1, 1), 1))
  fly_bomb(fly_fire(fly_reticle(fly_spur(fly_reins(m2, id), id), id), id), id)

def fly_go(+m: Mem, +id: U32, +down: Bool) -> Mem:
  match down:
    case True{}:
      fly_down(m, id)
    case False{}:
      fly_live(m, id)

def fly(+m: Mem, +id: U32) -> Mem:
  +m1 = fly_timers(set_g_reg(set_g_reg(m, 5, c_chain(m, seat_c(id))), 6, c_chain_ticks(m, seat_c(id))), seat_c(id))
  fly_go(m1, id, nz(c_down(m1, seat_c(id))))

# ===== the wing: formation flying with a little drift, a bolt now and then at what is nearest ahead =====

def rider_x(+k: U32) -> U32: kind5(k, dn(30), dm(30), 0, dn(62), dn(62))

def rider_z(+k: U32) -> U32: kind5(k, dm(8), dm(8), dm(18), dm(2), dm(2))
def rider_back(+k: U32) -> U32: kind5(k, dm(60), dm(60), dm(78), dm(50), dm(50))

def wing_shoot(+m: Mem, +k: U32, +wb: U32, +cb: U32, +me_s: U32, +ti: U32) -> Mem:
  +tb = foe_b(ti)
  +d0 = U32.sub(f_s(m, tb), me_s)
  +d1 = U32.sub(f_x(m, tb), c_x(m, cb))
  +d2 = U32.sub(f_z(m, tb), c_z(m, cb))
  +len = norm3(d0, d1, d2)
  set_w_fire(push_bolt(m, A_ALLY(), w_id(m, wb), U32.add(me_s, dm(12)), c_x(m, cb), c_z(m, cb), U32.add(PACE(), scs(d0, V_BOLT(), len)), scx(d1, V_BOLT(), len), scx(d2, V_BOLT(), len), 6, BOLT_LIFE(), 0, 0, 0, 0), wb, U32.add(14, U32.mul(3, k)))

def wing_fire_go(+m: Mem, +k: U32, +wb: U32, +cb: U32, +me_s: U32, +ti: U32) -> Mem:
  pkm(lt(ti, 99), wing_shoot(m, k, wb, cb, me_s, ti), set_w_fire(m, wb, 6))

def wing_fire(+m: Mem, +k: U32, +wb: U32, +cb: U32, +go: Bool) -> Mem:
  match go:
    case False{}:
      m
    case True{}:
      +me_s = U32.add(anchor(m), rider_back(k))
      wing_fire_go(m, k, wb, cb, me_s, near_go(nt(g_n_foes(m)), m, 0, me_s, c_x(m, cb), c_z(m, cb), dm(60), dm(400), 99, 0))

def wing_one(+m: Mem, +k: U32) -> Mem:
  +wb = wing_b(k)
  +cb = wing_c(k)
  +t = g_t(m)
  +tx = U32.add(rider_x(k), tri(U32.add(t, U32.mul(41, k)), U32.add(130, U32.mul(16, k)), dm(16)))
  +tz = U32.add(rider_z(k), tri(U32.add(t, U32.mul(29, k)), U32.add(96, U32.mul(12, k)), dm(6)))
  +ox = c_x(m, cb)
  +oz = c_z(m, cb)
  +x = U32.add(ox, sdv(U32.sub(tx, ox), 8))
  +zz = U32.add(oz, sdv(U32.sub(tz, oz), 8))
  +vx = U32.sub(x, ox)
  +vz = U32.sub(zz, oz)
  +bank = cl(U32.add(c_bank(m, cb), sdv(U32.sub(sdv(U32.mul(vx, Q()), V_X()), c_bank(m, cb)), 4)), ng(Q()), Q())
  +lift = cl(U32.add(c_lift(m, cb), sdv(U32.sub(sdv(U32.mul(vz, Q()), V_Z()), c_lift(m, cb)), 4)), ng(Q()), Q())
  +m1 = set_c_hurt(set_c_back(set_c_lift(set_c_bank(set_c_vz(set_c_vx(set_c_z(set_c_x(m, cb, x), cb, zz), cb, vx), cb, vz), cb, bank), cb, lift), cb, rider_back(k)), cb, ssub(c_hurt(m, cb), 1))
  +fire = ssub(w_fire(m1, wb), 1)
  wing_fire(set_w_fire(m1, wb, fire), k, wb, cb, z(fire))

# falling away, out of the picture
def wing_lost(+m: Mem, +k: U32) -> Mem:
  +cb = wing_c(k)
  set_c_z(m, cb, mx(U32.sub(c_z(m, cb), 20), dn(110)))

def wing_go(+m: Mem, +k: U32, +lost: Bool) -> Mem:
  match lost:
    case True{}:
      wing_lost(m, k)
    case False{}:
      wing_one(m, k)

def wing_loop(+n: Nat, +m: Mem, +k: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      wing_loop(p, wing_go(m, k, nz(w_lost(m, wing_b(k)))), U32.add(k, 1))

def wing_fly(+m: Mem) -> Mem: wing_loop(nt(g_n_wing(m)), m, 0)

# ===== after the beat's blows: a craft with no shield falls and rises again a moment later =====

def let_go(+n: Nat, +m: Mem, +i: U32, +id: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      +fb = foe_b(i)
      let_go(p, pkm(Bool.and(eq(f_st(m, fb), S_LATCHED()), eq(f_on(m, fb), id)), set_f_ticks(set_f_st(m, fb, S_FLEE()), fb, 40), m), U32.add(i, 1), id)

def seat_falls(+m: Mem, +id: U32) -> Mem:
  +cb = seat_c(id)
  +at_s = cr_s(m, cb)
  +at_x = c_x(m, cb)
  +at_z = c_z(m, cb)
  +m1 = set_c_nlocks(set_c_charge(set_c_held(set_c_roll(set_c_shield(set_c_down(m, cb, RISE()), cb, 0), cb, 0), cb, 0), cb, 0), cb, 0)
  let_go(nt(g_n_foes(m1)), sound(spark(m1, B_FALL(), at_s, at_x, at_z, id), SND_KNIGHT_FALL()), 0, id)

def craft_seat(+n: Nat, +m: Mem, +id: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      +cb = seat_c(id)
      craft_seat(p, pkm(Bool.and(seated(m, id), Bool.and(le(c_shield(m, cb), 0), z(c_down(m, cb)))), seat_falls(m, id), m), U32.add(id, 1))

def rider_falls(+m: Mem, +k: U32) -> Mem:
  +wb = wing_b(k)
  +cb = wing_c(k)
  +id = w_id(m, wb)
  +at_s = U32.add(anchor(m), c_back(m, cb))
  +m1 = set_g_beat_lost(set_w_lost(m, wb, 1), U32.add(g_beat_lost(m), 1))
  let_go(nt(g_n_foes(m1)), sound(spark(say(m1, id, L_LOST()), B_FALL(), at_s, c_x(m, cb), c_z(m, cb), id), SND_KNIGHT_FALL()), 0, id)

def craft_wing(+n: Nat, +m: Mem, +k: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      craft_wing(p, pkm(Bool.and(z(w_lost(m, wing_b(k))), le(c_shield(m, wing_c(k)), 0)), rider_falls(m, k), m), U32.add(k, 1))

def step_craft(+m: Mem) -> Mem: craft_wing(nt(g_n_wing(m)), craft_seat(6n, m, 1), 0)
def GAP_HALF() -> U32: dm(30)
def COL_HALF() -> U32: dm(24)
def HURT_T() -> U32: 70
def COIL_T() -> U32: 22
def ROAR_T() -> U32: 26
def BLOW() -> U32: 35
def lamp_id(+m: Mem, +k: U32) -> U32: rd(m, U32.add(U32.add(BOSSB(), 5), k))

# a kite that hunts a wingman and latches onto its tail
def spawn_hunter(+m: Mem, +side: U32) -> Mem:
  spawn_foe(m, K_KITE(), U32.sub(U32.add(anchor(m), LEAD()), dm(40)), U32.mul(side, dm(90)), dm(40), 0, 0, 0, S_HUNT(), 1, 0)

# how long the Wyrm rests between attacks: shorter the more knights fly and the later the phase
def rest(+m: Mem, +phase: U32) -> U32: umx(U32.sub(U32.sub(64, U32.mul(4, U32.sub(pressure(m), 1))), U32.mul(8, phase)), 34)

# the head holds its place ahead and sways; each lamp follows it
def lamp_follow(+m: Mem, +k: U32) -> Mem:
  +b = BOSSB()
  +fi = foe_by_id(m, lamp_id(m, k))
  +fb = foe_b(fi)
  +m1 = set_f_z(set_f_x(set_f_s(m, fb, U32.sub(U32.add(anchor(m), HEAD()), FRONT())), fb, U32.add(bo_x(m, b), lamp_x(k))), fb, U32.add(bo_z(m, b), lamp_z(k)))
  +open_ = eq(f_st(m1, fb), S_OPEN())
  +t = ssub(f_ticks(m1, fb), 1)
  pkm(lt(fi, 99), pkm(open_, set_f_st(set_f_ticks(m1, fb, t), fb, pk(z(t), S_SHUT(), S_OPEN())), m1), m)

def head_holds(+m: Mem) -> Mem:
  +b = BOSSB()
  +m1 = set_bo_z(set_bo_x(m, b, tri(g_t(m), 260, dm(24))), b, U32.sub(HIGH(), pk(eq(bo_st(m, b), W_RISE()), sdv(U32.mul(bo_ticks(m, b), dm(1)), 2), 0)))
  lamp_follow(lamp_follow(lamp_follow(m1, 0), 1), 2)

def owned(+m: Mem, +i: U32) -> Bool: nz(bl_owner(m, bolt_b(i)))

# the lit lamp is out: the Wyrm reels (or, the last lamp, dies); whatever it spat is gone
def reel(+m: Mem) -> Mem:
  +b = BOSSB()
  +m1 = sound(set_bo_nmarks(retain(~owned, m, BOLTS(), BOLT_W(), g_n_bolts(m), CA_BOLTS()), b, 0), SND_BOSS_ROAR())
  pkm(ge(bo_phase(m1, b), 2), set_bo_ticks(set_bo_st(m1, b, W_DYING()), b, DYING()), set_bo_ticks(set_bo_st(m1, b, W_HURT()), b, HURT_T()))

def attack_pat(+phase: U32, +seq: U32) -> U32:
  pk(eq(phase, 0), 0, pk(eq(phase, 1), pk(z(U32.mod(seq, 2)), 1, 0), kind5(U32.mod(seq, 4), 2, 0, 1, 0, 0)))

# an attack's footprints are locked and shown for the tell's ticks
def mark_push(+m: Mem, +x: U32, +zz: U32) -> Mem:
  +b = BOSSB()
  +k = bo_nmarks(m, b)
  set_bo_nmarks(set_bo_mark_z(set_bo_mark_x(m, b, k, x), b, k, zz), b, U32.add(k, 1))

def tell_spit(+n: Nat, +m: Mem, +k: U32, +turn: U32, +cnt: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      +id = target(m, U32.add(turn, k))
      +cb = seat_c(id)
      +jitter = U32.mul(U32.div(k, cnt), dm(16))
      tell_spit(p, pkm(nz(id), mark_push(m, U32.add(c_x(m, cb), pk(z(U32.mod(k, 2)), jitter, ng(jitter))), c_z(m, cb)), m), U32.add(k, 1), turn, cnt)

def tell_end(+m: Mem) -> Mem:
  +b = BOSSB()
  sound(set_bo_ticks(set_bo_st(m, b, W_TELL()), b, umx(g_num_tell(m), 40)), SND_VENT_FIRE())

def match_atk(+m: Mem, +atk: Nat, +turn: U32, +seats: U32, +b: U32) -> Mem:
  match atk:
    case 0n:
      +count = umn(U32.add(U32.add(2, U32.div(U32.sub(seats, 1), 2)), bo_phase(m, b)), 7)
      +m1 = tell_spit(nt(count), m, 0, turn, umx(count_seats(6n, m, 1), 1))
      tell_end(say_if(m1, voice(m1), L_REFLECT(), Bool.and(z(bo_phase(m1, b)), eq(bo_seq(m1, b), 1))))
    case 1n:
      +tg = target(m, turn)
      +gap = cl(pk(nz(tg), c_x(m, seat_c(tg)), 0), U32.add(ng(wx(m)), U32.div(GAP_HALF(), 2)), U32.sub(wx(m), U32.div(GAP_HALF(), 2)))
      tell_end(mark_push(set_bo_gap(set_bo_side(m, b, pk(z(U32.mod(turn, 2)), ng(1), 1)), b, gap), gap, 0))
    case _:
      +col = pk(nz(target(m, U32.add(turn, 1))), c_x(m, seat_c(target(m, U32.add(turn, 1)))), 0)
      tell_end(mark_push(set_bo_col(m, b, col), col, 0))

def begin_atk(+m: Mem, +atk: U32) -> Mem:
  +b = BOSSB()
  +seats = pressure(m)
  +turn = bo_seq(m, b)
  +m0 = set_bo_nmarks(m, b, 0)
  match_atk(m0, nt(atk), turn, seats, b)

def begin_tell(+m: Mem) -> Mem:
  +b = BOSSB()
  pkm(z(target(m, 0)), set_bo_ticks(set_bo_st(m, b, W_IDLE()), b, 30), begin_atk(m, bo_atk(m, b)))

# the Wyrm's spit: a Gloam bolt for each locked mark, from its mouth, each owing its return to the lit lamp
def spit_marks(+n: Nat, +m: Mem, +k: U32, +lit: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      +b = BOSSB()
      spit_marks(p, spit(m, A_GLOAM(), U32.add(anchor(m), HEAD()), bo_x(m, b), bo_z(m, b), bo_mark_x(m, b, k), bo_mark_z(m, b, k), dm(5), 25, 300, lit), U32.add(k, 1), lit)

def spit_all(+m: Mem) -> Mem:
  +b = BOSSB()
  sound(spit_marks(nt(bo_nmarks(m, b)), m, 0, lamp_id(m, mn(bo_phase(m, b), 2))), SND_SHOT_CROSSBOW())

def to_idle(+m: Mem) -> Mem:
  +b = BOSSB()
  set_bo_nmarks(set_bo_ticks(set_bo_st(m, b, W_IDLE()), b, rest(m, bo_phase(m, b))), b, 0)

# the coils sweep the window, the roar is a column of cold light: a craft they catch is struck
def sweep_seat(+m: Mem, +id: U32, +coil: Bool) -> Mem:
  +b = BOSSB()
  +cb = seat_c(id)
  +x = c_x(m, cb)
  +caught = pb(coil, gt(ab(U32.sub(x, bo_gap(m, b))), U32.sub(GAP_HALF(), U32.div(CRAFT_R(), 2))), le(ab(U32.sub(x, bo_col(m, b))), U32.add(COL_HALF(), U32.div(CRAFT_R(), 2))))
  pkm(Bool.and(Bool.and(seated(m, id), z(c_down(m, cb))), caught), hurt_craft(m, id, BLOW()), m)

def sweep_loop(+n: Nat, +m: Mem, +id: U32, +coil: Bool) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      sweep_loop(p, sweep_seat(m, id, coil), U32.add(id, 1), coil)

def hunters_loop(+n: Nat, +m: Mem, +k: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      hunters_loop(p, spawn_hunter(m, pk(z(U32.mod(k, 2)), ng(1), 1)), U32.add(k, 1))

# one tick of each state of the Wyrm
def boss_rise_st(+m: Mem) -> Mem:
  +b = BOSSB()
  +t = ssub(bo_ticks(m, b), 1)
  +m1 = set_bo_ticks(m, b, t)
  pkm(z(t), say(set_bo_ticks(set_bo_st(m1, b, W_IDLE()), b, 50), voice(m1), L_REFLECT()), m1)

def boss_idle_st(+m: Mem) -> Mem:
  +b = BOSSB()
  +t = ssub(bo_ticks(m, b), 1)
  +m1 = set_bo_ticks(m, b, t)
  +atk = attack_pat(bo_phase(m1, b), bo_seq(m1, b))
  pkm(z(t), begin_tell(set_bo_seq(set_bo_atk(m1, b, atk), b, U32.and(U32.add(bo_seq(m1, b), 1), 255))), m1)

def tell_over(+m: Mem, +atk: Nat, +b: U32) -> Mem:
  match atk:
    case 0n:
      to_idle(spit_all(m))
    case 1n:
      sound(set_bo_ticks(set_bo_st(m, b, W_COIL()), b, COIL_T()), SND_BOSS_RAGE())
    case _:
      sound(set_bo_ticks(set_bo_st(m, b, W_ROAR()), b, ROAR_T()), SND_BOSS_ROAR())

def boss_tell_st(+m: Mem) -> Mem:
  +b = BOSSB()
  +t = ssub(bo_ticks(m, b), 1)
  +m1 = set_bo_ticks(m, b, t)
  pkm(z(t), tell_over(m1, nt(bo_atk(m1, b)), b), m1)

def boss_sweep_st(+m: Mem) -> Mem:
  +b = BOSSB()
  +m1 = sweep_loop(6n, m, 1, eq(bo_st(m, b), W_COIL()))
  +t = ssub(bo_ticks(m1, b), 1)
  +m2 = set_bo_ticks(m1, b, t)
  pkm(z(t), to_idle(m2), m2)

def boss_hurt_over(+m: Mem) -> Mem:
  +b = BOSSB()
  +phase = U32.add(bo_phase(m, b), 1)
  +m1 = set_bo_seq(set_bo_ticks(set_bo_st(set_bo_phase(m, b, phase), b, W_IDLE()), b, U32.add(rest(m, phase), 20)), b, 0)
  hunters_loop(nt(U32.add(1, U32.div(pressure(m1), 3))), say(m1, voice(m1), pk(eq(phase, 1), L_COILS(), L_ROAR())), 0)

def boss_hurt_st(+m: Mem) -> Mem:
  +b = BOSSB()
  +t = ssub(bo_ticks(m, b), 1)
  +m1 = set_bo_ticks(m, b, t)
  pkm(z(t), boss_hurt_over(m1), m1)

def boss_dying_st(+m: Mem) -> Mem:
  +b = BOSSB()
  +t = ssub(bo_ticks(m, b), 1)
  +m1 = set_bo_ticks(m, b, t)
  pkm(z(U32.mod(t, 12)), spark(m1, B_POP(), U32.add(anchor(m1), HEAD()), bo_x(m1, b), bo_z(m1, b), 0), m1)

def boss_state(+m: Mem, +st: Nat) -> Mem:
  match st:
    case 0n:
      boss_rise_st(m)
    case 1n:
      boss_idle_st(m)
    case 2n:
      boss_tell_st(m)
    case 3n:
      boss_sweep_st(m)
    case 4n:
      boss_sweep_st(m)
    case 5n:
      boss_hurt_st(m)
    case _:
      boss_dying_st(m)

def boss_go(+m: Mem, +out: Bool) -> Mem:
  match out:
    case True{}:
      reel(m)
    case False{}:
      boss_state(m, nt(bo_st(m, BOSSB())))

# the Wyrm's tick
def step_boss_live(+m: Mem) -> Mem:
  +b = BOSSB()
  +m1 = head_holds(m)
  +fi = foe_by_id(m1, lamp_id(m1, mn(bo_phase(m1, b), 2)))
  +st = bo_st(m1, b)
  boss_go(m1, Bool.and(Bool.and(lt(fi, 99), eq(f_st(m1, foe_b(fi)), S_BROKEN())), Bool.and(ne(st, W_HURT()), ne(st, W_DYING()))))

def boss_present(+m: Mem, +on: Bool) -> Mem:
  match on:
    case True{}:
      step_boss_live(m)
    case False{}:
      m

def step_boss(+m: Mem) -> Mem: boss_present(m, nz(g_boss(m)))

# ===== the script's acts =====

def prop_room(+m: Mem) -> Bool: lt(g_n_props(m), 40)

def prop_put(+m: Mem, +thing: U32, +s: U32, +x: U32, +zz: U32, +w: U32, +h: U32, +len: U32, +phase: U32, +ok: Bool) -> Mem:
  match ok:
    case False{}:
      set_g_reg(m, 0, 0)
    case True{}:
      +b = prop_b(g_n_props(m))
      +m1 = set_p_z(set_p_x(set_p_s(set_p_thing(m, b, thing), b, s), b, x), b, zz)
      +m2 = set_p_phase(set_p_taken(set_p_len(set_p_h(set_p_w(m1, b, w), b, h), b, len), b, 0), b, phase)
      set_g_reg(set_g_n_props(m2, U32.add(g_n_props(m), 1)), 0, 1)

# a thing that stays where it is on the rail: reg 0 holds whether there was room for it
def push_prop(+m: Mem, +thing: U32, +s: U32, +x: U32, +zz: U32, +w: U32, +h: U32, +len: U32, +phase: U32) -> Mem:
  prop_put(m, thing, s, x, zz, w, h, len, phase, prop_room(m))

# a centred spread: the kth of count, in half steps
def spreadk(+k: U32, +count: U32) -> U32: U32.sub(U32.mul(2, k), U32.sub(count, 1))

# how many of a wave there are: more knights at the table means more of each wave, never less
def wave(+m: Mem, +n: U32, +plus: U32) -> U32: U32.add(n, sdv(U32.mul(U32.sub(pressure(m), 1), plus), 2))

def wisp_dx(+shape: Nat, +k: U32, +sp: U32) -> U32:
  match shape:
    case 0n: U32.mul(sp, dm(16))
    case 1n: U32.mul(sp, dm(13))
    case 2n: 0
    case 3n: U32.mul(pk(z(U32.mod(k, 2)), 1, ng(1)), dm(95))
    case _: U32.mul(sp, dm(15))

def wisp_dz(+shape: Nat, +k: U32, +sp: U32) -> U32:
  match shape:
    case 0n: 0
    case 1n: ng(U32.mul(ab(sp), dm(3)))
    case 2n: 0
    case 3n: U32.mul(U32.mod(k, 3), dm(10))
    case _: U32.mul(U32.mod(k, 2), dm(12))

def wisp_ds(+shape: Nat, +k: U32, +sp: U32) -> U32:
  match shape:
    case 0n: 0
    case 1n: U32.mul(ab(sp), dm(10))
    case 2n: U32.mul(k, dm(34))
    case 3n: U32.mul(k, dm(22))
    case _: U32.mul(k, dm(16))

def wisp_vx(+shape: Nat, +k: U32) -> U32:
  match shape:
    case 3n: ng(U32.mul(pk(z(U32.mod(k, 2)), 1, ng(1)), 44))
    case _: 0

def wisps_loop(+n: Nat, +m: Mem, +k: U32, +count: U32, +shape: U32, +x: U32, +zz: U32, +mode: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      +sp = spreadk(k, count)
      +far = U32.add(anchor(m), LEAD())
      wisps_loop(p, spawn_foe(m, K_WISP(), U32.add(far, wisp_ds(nt(shape), k, sp)), U32.add(dm(x), wisp_dx(nt(shape), k, sp)), U32.add(dm(zz), wisp_dz(nt(shape), k, sp)), ng(15), wisp_vx(nt(shape), k), 0, S_FLY(), mode, U32.and(U32.mul(k, 7), 255)), U32.add(k, 1), count, shape, x, zz, mode)

# every flier already shooting, and not yet ticking, is given its first count
def shooters(+n: Nat, +m: Mem, +i: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      +fb = foe_b(i)
      shooters(p, pkm(Bool.and(eq(f_mode(m, fb), 3), z(f_ticks(m, fb))), set_f_ticks(m, fb, 60), m), U32.add(i, 1))

def act_wisps(+m: Mem, +n: U32, +plus: U32, +shape: U32, +x: U32, +zz: U32, +mode: U32) -> Mem:
  +m1 = wisps_loop(nt(wave(m, n, plus)), m, 0, wave(m, n, plus), shape, x, zz, mode)
  pkm(eq(mode, 3), shooters(nt(g_n_foes(m1)), m1, 0), m1)

def convoy_loop(+n: Nat, +m: Mem, +k: U32, +side: U32, +zz: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      convoy_loop(p, spawn_foe(m, K_KITE(), U32.add(U32.add(anchor(m), LEAD()), U32.mul(k, dm(26))), U32.mul(side, dm(95)), dm(zz), ng(15), ng(U32.mul(side, 46)), 0, S_FLY(), 0, U32.and(U32.mul(k, 5), 255)), U32.add(k, 1), side, zz)

def act_convoy(+m: Mem, +n: U32, +plus: U32, +from: U32, +zz: U32) -> Mem: convoy_loop(nt(wave(m, n, plus)), m, 0, from, zz)

def act_turret(+m: Mem, +x: U32, +zz: U32) -> Mem:
  +m1 = spawn_foe(m, K_TURRET(), U32.add(anchor(m), LEAD()), dm(x), dm(zz), 0, 0, 0, S_REST(), 0, U32.mod(g_next(m), 5))
  +fi = foe_by_id(m1, g_reg(m1, 0))
  pkm(Bool.and(nz(g_reg(m1, 0)), lt(fi, 99)), set_f_ticks(m1, foe_b(fi), 30), m1)

def bombers_loop(+n: Nat, +m: Mem, +k: U32, +x: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      bombers_loop(p, spawn_foe(m, K_BOMBER(), U32.add(U32.add(anchor(m), LEAD()), U32.add(dm(10), U32.mul(k, dm(8)))), U32.add(dm(x), U32.mul(U32.sub(U32.mod(k, 3), 1), dm(14))), dn(6), 0, 0, 0, S_HIDE(), 0, U32.and(U32.mul(k, 3), 255)), U32.add(k, 1), x)

def act_ridge(+m: Mem, +x: U32, +bombers: U32, +plus: U32) -> Mem:
  +m1 = push_prop(m, T_RIDGE(), U32.add(anchor(m), LEAD()), dm(x), dn(30), dm(30), dm(55), dm(60), 0)
  bombers_loop(nt(wave(m1, bombers, plus)), m1, 0, x)

def act_gate(+m: Mem, +rel: U32, +x: U32, +zz: U32) -> Mem:
  +m1 = push_prop(m, T_GATE(), U32.add(origin(mn(g_stage(m), 2)), rel), dm(x), dm(zz), dm(20), 0, 0, 0)
  pkm(eq(g_reg(m1, 0), 1), beat_offer(m1, U32.mul(100, pressure(m1))), m1)

def act_ring(+m: Mem, +rel: U32, +x: U32, +zz: U32) -> Mem:
  push_prop(m, T_RING(), U32.add(origin(mn(g_stage(m), 2)), rel), dm(x), dm(zz), dm(22), 0, 0, 0)

def act_arch(+m: Mem, +rel: U32, +x: U32, +zz: U32, +w: U32, +h: U32) -> Mem:
  push_prop(m, T_ARCH(), U32.add(origin(mn(g_stage(m), 2)), rel), dm(x), dm(zz), dm(w), dm(h), dm(30), 0)

def act_shutter(+m: Mem, +rel: U32, +phase: U32) -> Mem:
  push_prop(m, T_SHUTTER(), U32.add(origin(mn(g_stage(m), 2)), rel), 0, 0, 0, 0, dm(30), phase)

# (a row is its word plus `o & 0`, so that the compiler lays the rows out as a chain of tests and not as a constant table: the build adapter
# reads a module that ends in its exports, which a table in front of them is not; `o` is always 0)
def word(+v: U32, +o: U32) -> U32: U32.add(v, U32.and(o, 0))

def ev_lo(+c: Nat, +o: U32) -> U32:
  match c:
    case 1n: word(259, o)
    case 2n: word(7910, o)
    case 3n: word(9728, o)
    case 4n: word(7706, o)
    case 5n: word(66053, o)
    case 6n: word(1, o)
    case 7n: word(10466, o)
    case 8n: word(10270, o)
    case 9n: word(2, o)
    case 10n: word(131588, o)
    case 11n: word(253236224, o)
    case 12n: word(3, o)
    case 13n: word(197124, o)
    case 14n: word(197124, o)
    case 15n: word(255, o)
    case 16n: word(516, o)
    case 17n: word(262662, o)
    case 18n: word(262662, o)
    case 19n: word(9216, o)
    case 20n: word(3870, o)
    case 21n: word(62976, o)
    case 22n: word(4066, o)
    case 23n: word(9216, o)
    case 24n: word(1, o)
    case 25n: word(515, o)
    case 26n: word(255, o)
    case 27n: word(10240, o)
    case 28n: word(259, o)
    case 64n: word(24, o)
    case 65n: word(7, o)
    case 66n: word(60577, o)
    case 67n: word(8, o)
    case 68n: word(63071, o)
    case 69n: word(156, o)
    case 70n: word(9, o)
    case 71n: word(419496454, o)
    case 72n: word(6630, o)
    case 73n: word(516, o)
    case 74n: word(9216, o)
    case 75n: word(6426, o)
    case 76n: word(236323062, o)
    case 77n: word(10, o)
    case 78n: word(131770, o)
    case 79n: word(11, o)
    case 81n: word(268370437, o)
    case 82n: word(60511, o)
    case 83n: word(75, o)
    case 84n: word(66053, o)
    case 85n: word(131654, o)
    case 86n: word(131514, o)
    case 87n: word(7906, o)
    case 88n: word(7710, o)
    case 89n: word(63132, o)
    case 90n: word(61796, o)
    case 91n: word(1, o)
    case 92n: word(255, o)
    case 93n: word(516, o)
    case 94n: word(1, o)
    case 95n: word(255, o)
    case 96n: word(9216, o)
    case 97n: word(4834, o)
    case 99n: word(4638, o)
    case 128n: word(23, o)
    case 129n: word(516, o)
    case 130n: word(255, o)
    case 131n: word(1, o)
    case 132n: word(7680, o)
    case 133n: word(6636, o)
    case _: word(0, o)

def ev_hi(+c: Nat, +o: U32) -> U32:
  match c:
    case 0n: word(589824, o)
    case 1n: word(20, o)
    case 2n: word(327680, o)
    case 3n: word(327680, o)
    case 4n: word(327680, o)
    case 5n: word(25, o)
    case 6n: word(589824, o)
    case 7n: word(393216, o)
    case 8n: word(393216, o)
    case 9n: word(589824, o)
    case 10n: word(20, o)
    case 11n: word(458752, o)
    case 12n: word(589824, o)
    case 13n: word(15, o)
    case 14n: word(30, o)
    case 15n: word(131072, o)
    case 16n: word(793, o)
    case 17n: word(276, o)
    case 18n: word(288, o)
    case 19n: word(327680, o)
    case 20n: word(327680, o)
    case 21n: word(327680, o)
    case 22n: word(327680, o)
    case 23n: word(327680, o)
    case 24n: word(131072, o)
    case 25n: word(25, o)
    case 26n: word(131072, o)
    case 27n: word(393216, o)
    case 28n: word(793, o)
    case 64n: word(589824, o)
    case 65n: word(589824, o)
    case 66n: word(196608, o)
    case 67n: word(589824, o)
    case 68n: word(196608, o)
    case 69n: word(196608, o)
    case 70n: word(589824, o)
    case 71n: word(65536, o)
    case 72n: word(327680, o)
    case 73n: word(793, o)
    case 74n: word(327680, o)
    case 75n: word(327680, o)
    case 76n: word(458752, o)
    case 77n: word(589824, o)
    case 78n: word(262144, o)
    case 79n: word(589824, o)
    case 80n: word(524288, o)
    case 81n: word(65536, o)
    case 82n: word(196608, o)
    case 83n: word(524288, o)
    case 84n: word(793, o)
    case 85n: word(262144, o)
    case 86n: word(262144, o)
    case 87n: word(393216, o)
    case 88n: word(393216, o)
    case 89n: word(196608, o)
    case 90n: word(196608, o)
    case 91n: word(131072, o)
    case 92n: word(131072, o)
    case 93n: word(796, o)
    case 94n: word(131072, o)
    case 95n: word(131072, o)
    case 96n: word(327680, o)
    case 97n: word(327680, o)
    case 98n: word(327680, o)
    case 99n: word(327680, o)
    case 128n: word(589824, o)
    case 129n: word(793, o)
    case 130n: word(131072, o)
    case 131n: word(131072, o)
    case 132n: word(393216, o)
    case 133n: word(327680, o)
    case 134n: word(655360, o)
    case _: word(0, o)

# the kth signed byte of event c (a first)
def evb(+c: Nat, +k: U32) -> U32:
  +v = pk(U32.is_lt(k, 4), ev_lo(c, 0), ev_hi(c, 0))
  +x = U32.and(U32.shrn(v, U32.to_nat(U32.mul(8, U32.mod(k, 4)))), 255)
  pk(U32.is_ge(x, 128), U32.sub(x, 256), x)

def ev_kind(+c: Nat) -> U32: U32.and(U32.shrn(ev_hi(c, 0), 16n), 255)

def ev_at(+c: Nat, +o: U32) -> U32:
  match c:
    case 0n: word(30, o)
    case 1n: word(110, o)
    case 2n: word(138, o)
    case 3n: word(153, o)
    case 4n: word(168, o)
    case 5n: word(330, o)
    case 6n: word(350, o)
    case 7n: word(508, o)
    case 8n: word(508, o)
    case 9n: word(560, o)
    case 10n: word(560, o)
    case 11n: word(668, o)
    case 12n: word(700, o)
    case 13n: word(800, o)
    case 14n: word(850, o)
    case 15n: word(1010, o)
    case 16n: word(1100, o)
    case 17n: word(1180, o)
    case 18n: word(1240, o)
    case 19n: word(1308, o)
    case 20n: word(1323, o)
    case 21n: word(1338, o)
    case 22n: word(1353, o)
    case 23n: word(1368, o)
    case 24n: word(1380, o)
    case 25n: word(1400, o)
    case 26n: word(1500, o)
    case 27n: word(1508, o)
    case 28n: word(1560, o)
    case 64n: word(30, o)
    case 65n: word(80, o)
    case 66n: word(90, o)
    case 67n: word(150, o)
    case 68n: word(330, o)
    case 69n: word(345, o)
    case 70n: word(500, o)
    case 71n: word(520, o)
    case 72n: word(608, o)
    case 73n: word(620, o)
    case 74n: word(623, o)
    case 75n: word(638, o)
    case 76n: word(708, o)
    case 77n: word(750, o)
    case 78n: word(760, o)
    case 79n: word(980, o)
    case 80n: word(998, o)
    case 81n: word(1150, o)
    case 82n: word(1170, o)
    case 83n: word(1198, o)
    case 84n: word(1300, o)
    case 85n: word(1400, o)
    case 86n: word(1450, o)
    case 87n: word(1558, o)
    case 88n: word(1558, o)
    case 89n: word(1650, o)
    case 90n: word(1670, o)
    case 91n: word(1780, o)
    case 92n: word(1850, o)
    case 93n: word(1900, o)
    case 94n: word(1990, o)
    case 95n: word(2050, o)
    case 96n: word(2058, o)
    case 97n: word(2073, o)
    case 98n: word(2088, o)
    case 99n: word(2103, o)
    case 128n: word(30, o)
    case 129n: word(110, o)
    case 130n: word(250, o)
    case 131n: word(280, o)
    case 132n: word(308, o)
    case 133n: word(328, o)
    case 134n: word(480, o)
    case _: word(0, o)

# ===== the three beats' scripts: what appears, and when, along the rail =====
# An event of beat `stage` (0 Thunderhead Run, 1 the Ruined Causeway, 2 the Wyrm's Roost) is row stage*64 + i, in the order the beat plays
# (sorted by its tick). `ev_at` is the tick; `ev_lo` and `ev_hi` hold six signed bytes a..f (a first) and the kind above them:
# kind 0 wisps (n, plus, shape, x, z, mode), 1 convoy (n, plus, from, z), 2 hunter (side), 3 turret (x, z), 4 ridge (x, bombers, plus),
# 5 gate (x, z), 6 ring (x, z), 7 arch (x, z, w, h), 8 shutter (phase), 9 say (line), 10 the Wyrm. A prop stands where the rail brings it
# to the formation at its own arrival tick, which is `LEAD_TICKS` after the event.
def LEAD_TICKS() -> U32: 192

# a prop's place on the rail: where the rail brings it at its arrival tick
def ev_rel(+c: Nat) -> U32: U32.mul(U32.add(ev_at(c, 0), LEAD_TICKS()), PACE())

def act_go(+m: Mem, +kind: Nat, +c: Nat) -> Mem:
  match kind:
    case 0n:
      act_wisps(m, evb(c, 0), evb(c, 1), evb(c, 2), evb(c, 3), evb(c, 4), evb(c, 5))
    case 1n:
      act_convoy(m, evb(c, 0), evb(c, 1), evb(c, 2), evb(c, 3))
    case 2n:
      spawn_hunter(m, evb(c, 0))
    case 3n:
      act_turret(m, evb(c, 0), evb(c, 1))
    case 4n:
      act_ridge(m, evb(c, 0), evb(c, 1), evb(c, 2))
    case 5n:
      act_gate(m, ev_rel(c), evb(c, 0), evb(c, 1))
    case 6n:
      act_ring(m, ev_rel(c), evb(c, 0), evb(c, 1))
    case 7n:
      act_arch(m, ev_rel(c), evb(c, 0), evb(c, 1), evb(c, 2), evb(c, 3))
    case 8n:
      act_shutter(m, ev_rel(c), evb(c, 0))
    case 9n:
      say(m, voice(m), evb(c, 0))
    case _:
      rise(m)

def ev_count(+stage: U32) -> U32: kind5(stage, 29, 36, 7, 0, 0)

def script_due(+m: Mem, +st: U32) -> Bool:
  +i = g_next(m)
  Bool.and(U32.is_lt(i, ev_count(st)), U32.is_le(ev_at(nt(U32.add(U32.mul(st, 64), i)), 0), g_t(m)))

# run the events that are due
def script_go(+n: Nat, +m: Mem, +st: U32, +due: Bool) -> Mem:
  match n due:
    case 0n _:
      m
    case 1n+p False{}:
      m
    case 1n+p True{}:
      +i = g_next(m)
      +c = nt(U32.add(U32.mul(st, 64), i))
      +m2 = act_go(set_g_next(m, U32.add(i, 1)), nt(ev_kind(c)), c)
      script_go(p, m2, st, script_due(m2, st))

def run_script(+m: Mem) -> Mem: script_go(40n, m, mn(g_stage(m), 2), script_due(m, mn(g_stage(m), 2)))

# the number of events that fall before the tick `cp`: where a beat's script stands when it comes back to it
def resume_go(+n: Nat, +st: U32, +i: U32, +cp: U32) -> U32:
  match n:
    case 0n:
      0
    case 1n+p:
      pk(U32.is_lt(ev_at(nt(U32.add(U32.mul(st, 64), i)), 0), cp), U32.add(1, resume_go(p, st, U32.add(i, 1), cp)), 0)

def resume(+st: U32, +cp: U32) -> U32: resume_go(nt(ev_count(st)), st, 0, cp)

# ===== the warm fly-through after the Wyrm: a procession of gold gates and hearth rings, lit for the way home =====

def victory_loop(+n: Nat, +m: Mem, +k: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      +zz = U32.add(dm(30), tri(U32.mul(k, 25), 90, dm(18)))
      victory_loop(p, push_prop(m, pk(eq(U32.mod(k, 3), 2), T_RING(), T_GATE()), U32.add(U32.add(anchor(m), dm(150)), U32.mul(k, dm(75))), tri(U32.mul(k, 40), 120, dm(30)), zz, dm(24), 0, 0, 1), U32.add(k, 1))

def victory_gates(+m: Mem) -> Mem: victory_loop(9n, m, 0)

# ===== the things on the rail =====

def SHUTTER_PERIOD() -> U32: 150

def SHUTTER_MIN() -> U32: dm(5)
def SCRAPE() -> U32: 15

# a shutter's half gap at tick t: open and closing and opening again over its cycle
def shutter_gap(+m: Mem, +pb: U32, +t: U32) -> U32:
  +k = U32.add(tri(U32.add(t, p_phase(m, pb)), SHUTTER_PERIOD(), 1000), 1000)
  U32.add(SHUTTER_MIN(), sdv(U32.mul(U32.sub(U32.add(wx(m), dm(10)), SHUTTER_MIN()), k), 2000))

# rock met: a blow, and the craft is spared a moment
def scrape(+m: Mem, +id: U32, +s: U32, +x: U32, +zz: U32) -> Mem:
  +m1 = hurt_craft(m, id, SCRAPE())
  pkm(eq(g_reg(m1, 0), 1), sound(spark(m1, B_SCRAPE(), s, x, zz, id), SND_ROCK_LAND()), m1)

def prop_gate_hit(+m: Mem, +id: U32, +pb: U32, +cb: U32, +s0: U32, +s1: U32) -> Bool:
  Bool.and(Bool.and(lt(s0, p_s(m, pb)), le(p_s(m, pb), s1)), Bool.and(z(U32.and(p_taken(m, pb), bit(id))),
    le(flat(c_x(m, cb), c_z(m, cb), p_x(m, pb), p_z(m, pb)), U32.add(p_w(m, pb), CRAFT_R()))))

def prop_hits(+m: Mem, +pb: U32, +cb: U32, +code: Nat, +over: Bool, +s0: U32, +s1: U32, +id: U32) -> Bool:
  match code:
    case 0n:
      prop_gate_hit(m, id, pb, cb, s0, s1)
    case 1n:
      prop_gate_hit(m, id, pb, cb, s0, s1)
    case 2n:
      Bool.and(over, Bool.not(Bool.and(le(U32.add(ab(U32.sub(c_x(m, cb), p_x(m, pb))), CRAFT_R()), p_w(m, pb)), le(U32.add(ab(U32.sub(c_z(m, cb), p_z(m, pb))), CRAFT_R()), p_h(m, pb)))))
    case 3n:
      Bool.and(over, Bool.and(lt(ab(U32.sub(c_x(m, cb), p_x(m, pb))), U32.add(p_w(m, pb), CRAFT_R())), lt(ab(U32.sub(c_z(m, cb), p_z(m, pb))), U32.add(p_h(m, pb), CRAFT_R()))))
    case _:
      Bool.and(over, gt(ab(U32.sub(c_x(m, cb), p_x(m, pb))), U32.sub(shutter_gap(m, pb, g_t(m)), CRAFT_R())))

def prop_apply(+m: Mem, +id: U32, +pb: U32, +cb: U32, +gate: Bool, +s1: U32) -> Mem:
  match gate:
    case True{}:
      take(set_p_taken(m, pb, U32.or(p_taken(m, pb), bit(id))), id, pb, nz(c_boost(m, cb)))
    case False{}:
      scrape(m, id, s1, c_x(m, cb), c_z(m, cb))

def prop_one(+m: Mem, +id: U32, +pi: U32) -> Mem:
  +pb = prop_b(pi)
  +cb = seat_c(id)
  +s1 = cr_s(m, cb)
  +s0 = U32.sub(s1, c_ds(m, cb))
  +over = Bool.and(lt(s0, U32.add(p_s(m, pb), p_len(m, pb))), gt(s1, p_s(m, pb)))
  +th = p_thing(m, pb)
  +hit = prop_hits(m, pb, cb, nt(th), over, s0, s1, id)
  pkm(hit, prop_apply(m, id, pb, cb, le(th, T_RING()), s1), m)

def prop_one_if(+m: Mem, +id: U32, +pi: U32) -> Mem: prop_one(m, id, pi)

def props_loop(+n: Nat, +m: Mem, +id: U32, +pi: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      props_loop(p, prop_one_if(m, id, pi), id, U32.add(pi, 1))

def props_seat(+m: Mem, +id: U32, +fly: Bool) -> Mem:
  match fly:
    case False{}:
      m
    case True{}:
      props_loop(nt(g_n_props(m)), m, id, 0)

def props_seats(+n: Nat, +m: Mem, +id: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      props_seats(p, props_seat(m, id, Bool.and(seated(m, id), z(c_down(m, seat_c(id))))), U32.add(id, 1))

def prop_keep(+m: Mem, +i: U32) -> Bool:
  +pb = prop_b(i)
  gt(U32.add(p_s(m, pb), p_len(m, pb)), U32.sub(anchor(m), dm(300)))

# props and the knights who cross them
def step_props(+m: Mem) -> Mem:
  +m1 = props_seats(6n, m, 1)
  retain(~prop_keep, m1, PROPS(), PROP_W(), g_n_props(m1), CA_PROPS())

# ===== the calls, flashes and counters wind down =====

def calls_dec(+n: Nat, +m: Mem, +i: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      calls_dec(p, set_ca_ticks(m, call_b(i), ssub(ca_ticks(m, call_b(i)), 1)), U32.add(i, 1))

def fx_dec(+n: Nat, +m: Mem, +i: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      fx_dec(p, set_fx_ticks(m, fx_b(i), ssub(fx_ticks(m, fx_b(i)), 1)), U32.add(i, 1))

def flash_dec(+n: Nat, +m: Mem, +i: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      flash_dec(p, set_f_flash(m, foe_b(i), ssub(f_flash(m, foe_b(i)), 1)), U32.add(i, 1))

def call_keep(+m: Mem, +i: U32) -> Bool: nz(ca_ticks(m, call_b(i)))
def fx_keep(+m: Mem, +i: U32) -> Bool: nz(fx_ticks(m, fx_b(i)))

def tick_lists(+m: Mem) -> Mem:
  +m1 = retain(~call_keep, calls_dec(nt(g_n_calls(m)), m, 0), CALLS(), 3, g_n_calls(m), 34)
  +m2 = retain(~fx_keep, fx_dec(nt(g_n_fx(m1)), m1, 0), FXS(), FX_W(), g_n_fx(m1), 35)
  flash_dec(nt(g_n_foes(m2)), m2, 0)

# ===== seating and the wing =====

# a fresh craft at its slot, shield full and three bombs
def craft_new(+m: Mem, +cb: U32, +x: U32, +zz: U32) -> Mem:
  +m1 = set_c_rz(set_c_rx(set_c_ds(set_c_back(set_c_lift(set_c_bank(set_c_vz(set_c_vx(set_c_z(set_c_x(m, cb, x), cb, zz), cb, 0), cb, 0), cb, 0), cb, 0), cb, 0), cb, PACE()), cb, 0), cb, 0)
  +m2 = set_c_rest(set_c_dir(set_c_roll_len(set_c_roll(set_c_down(set_c_hurt(set_c_shield(m1, cb, SHIELD()), cb, 0), cb, 0), cb, 0), cb, 0), cb, 1), cb, 0)
  +m3 = set_c_nlocks(set_c_cool(set_c_charge(set_c_held(m2, cb, 0), cb, 0), cb, 0), cb, 0)
  +m4 = set_c_bombs(set_c_brake(set_c_boost(set_c_spent(set_c_meter(set_c_dwell(m3, cb, 0), cb, METER()), cb, 0), cb, 0), cb, 0), cb, 3)
  +m5 = set_c_chain_ticks(set_c_chain(set_c_last_x(set_c_tap_ticks(set_c_tap_dir(set_c_keys(m4, cb, 0), cb, 0), cb, 0), cb, 0), cb, 0), cb, 0)
  set_c_gates(set_c_saved(set_c_hits(set_c_pts(m5, cb, 0), cb, 0), cb, 0), cb, 0)

# seat a knight at the formation, unhurt for a moment
def sit(+m: Mem, +id: U32, +joined: U32) -> Mem:
  +cb = seat_c(id)
  +sb = seat_b(id)
  set_c_hurt(craft_new(set_st_joined(set_st_present(m, sb, 1), sb, joined), cb, slot_x(id), slot_z(id)), cb, 60)

def here_p(+m: Mem, +id: U32) -> Bool: nz(U32.and(g_here(m), U32.shln(1, nt(U32.sub(id, 1)))))

def muster_seats(+n: Nat, +m: Mem, +id: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      +sb = seat_b(id)
      +here = here_p(m, id)
      +seat = seated(m, id)
      +m1 = pkm(Bool.and(seat, Bool.not(here)), set_st_present(m, sb, 0), m)
      +m2 = pkm(Bool.and(here, Bool.not(seat)), sound(say(sit(m1, id, g_elapsed(m1)), id, L_JOIN()), SND_PLAY_CARD()), m1)
      muster_seats(p, m2, U32.add(id, 1))

# foes provisioned for the seats now flying: a seat that sits down scales the beat up by its share
def provision_one(+m: Mem, +fb: U32, +now: U32) -> Mem:
  +was = umx(f_seats(m, fb), 1)
  +hp = un(Nat.div(Nat.mul(nt(f_hp(m, fb)), nt(now)), nt(was)))
  set_f_seats(set_f_full(set_f_hp(m, fb, hp), fb, U32.div(U32.mul(f_full(m, fb), now), was)), fb, now)

def provision(+n: Nat, +m: Mem, +i: U32, +now: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      +fb = foe_b(i)
      provision(p, pkm(U32.is_lt(f_seats(m, fb), now), provision_one(m, fb, now), m), U32.add(i, 1), now)

# the wing: riders fill the seats nobody sits in, up to four
def wing_add(+n: Nat, +m: Mem) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      +k = g_n_wing(m)
      +wb = wing_b(k)
      +cb = wing_c(k)
      +m1 = craft_new(m, cb, rider_x(k), rider_z(k))
      wing_add(p, set_g_n_wing(set_w_fire(set_w_lost(set_w_id(set_c_back(m1, cb, rider_back(k)), wb, U32.add(7, k)), wb, 0), wb, U32.mul(12, k)), U32.add(k, 1)))

# the number of seats (not clamped)
def pressure_seats(+m: Mem) -> U32: count_seats(6n, m, 1)

def fill_wing(+m: Mem) -> Mem:
  +want = ssub(4, pressure_seats(m))
  wing_add(nt(ssub(want, g_n_wing(m))), set_g_n_wing(m, umn(g_n_wing(m), want)))

# who is at the table now, seated and stood up, and the foes' health provisioned for the seats flying
def muster(+m: Mem) -> Mem:
  +m1 = muster_seats(6n, m, 1)
  fill_wing(provision(nt(g_n_foes(m1)), m1, 0, pressure(m1)))

def pre(+m: Mem) -> Mem: muster(set_g_elapsed(m, umn(U32.add(g_elapsed(m), 1), LIMIT())))

# ===== the beat's end, a wipe, or the Wyrm's fall =====

def LEN0() -> U32: 1800

def LEN1() -> U32: 2400

def all_down(+n: Nat, +m: Mem, +id: U32) -> Bool:
  match n:
    case 0n:
      True{}
    case 1n+p:
      Bool.and(Bool.or(Bool.not(seated(m, id)), nz(c_down(m, seat_c(id)))), all_down(p, m, U32.add(id, 1)))

def bank_add(+m: Mem) -> Mem:
  +s = set_g_bank_pts(set_g_bank_base(set_g_bank_offered(set_g_bank_kills(m, U32.add(g_bank_kills(m), g_beat_kills(m))), U32.add(g_bank_offered(m), g_beat_offered(m))), U32.add(g_bank_base(m), g_beat_base(m))), U32.add(g_bank_pts(m), g_beat_pts(m)))
  set_g_bank_gates(set_g_bank_lost(set_g_bank_saved(s, U32.add(g_bank_saved(s), g_beat_saved(m))), U32.add(g_bank_lost(s), g_beat_lost(m))), U32.add(g_bank_gates(s), g_beat_gates(m)))

def beat_clear(+m: Mem) -> Mem:
  set_g_beat_gates(set_g_beat_lost(set_g_beat_saved(set_g_beat_kills(set_g_beat_offered(set_g_beat_base(set_g_beat_pts(m, 0), 0), 0), 0), 0), 0), 0)

def seat_mend(+m: Mem, +id: U32) -> Mem:
  +cb = seat_c(id)
  set_c_spent(set_c_meter(set_c_bombs(set_c_shield(m, cb, umn(U32.add(c_shield(m, cb), 40), SHIELD())), cb, umn(U32.add(c_bombs(m, cb), 1), 3)), cb, METER()), cb, 0)

def mend_seats(+n: Nat, +m: Mem, +id: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      mend_seats(p, pkm(seated(m, id), seat_mend(m, id), m), U32.add(id, 1))

def mend_wing(+n: Nat, +m: Mem, +k: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      mend_wing(p, set_c_shield(set_w_lost(m, wing_b(k), 0), wing_c(k), umn(U32.add(c_shield(m, wing_c(k)), 40), SHIELD())), U32.add(k, 1))

# the next beat flies on from where this one ends, the party mended a little and a bomb richer
def next_beat(+m: Mem) -> Mem:
  +m1 = set_g_next(set_g_cp(set_g_t(set_g_stage(beat_clear(bank_add(m)), U32.add(g_stage(m), 1)), 0), 0), 0)
  +m2 = sound(say(m1, voice(m1), pk(eq(g_stage(m1), 1), L_RUINS(), L_ROOST())), SND_PLAY_CARD())
  mend_wing(nt(g_n_wing(m2)), mend_seats(6n, m2, 1), 0)

def retry_set(+m: Mem) -> Mem: sound(set_g_mode_t(set_g_mode(m, M_RETRY()), RETRY()), SND_KNIGHT_FALL())

def victory_set(+m: Mem) -> Mem:
  +m1 = set_g_beat_offered(set_g_beat_base(set_g_beat_pts(set_g_home(set_g_done(set_g_done_some(set_g_mode_t(set_g_mode(m, M_VICTORY()), 0), 1), g_elapsed(m)), anchor(m)), U32.add(g_beat_pts(m), 1000)), U32.add(g_beat_base(m), 1000)), U32.add(g_beat_offered(m), 1000))
  +m2 = set_g_n_bolts(set_g_n_foes(m1, 0), 0)
  victory_gates(sound(say(m2, voice(m2), L_HOME()), SND_BOSS_DEATH()))

def dying_done(+m: Mem) -> Bool: Bool.and(nz(g_boss(m)), Bool.and(eq(bo_st(m, BOSSB()), W_DYING()), z(bo_ticks(m, BOSSB()))))

def end_go(+m: Mem, +wipe: Bool, +over: Bool, +dying: Bool) -> Mem:
  match wipe over dying:
    case True{} _ _:
      retry_set(m)
    case False{} True{} _:
      next_beat(m)
    case False{} False{} True{}:
      victory_set(m)
    case False{} False{} False{}:
      m

# the beat's end, a wipe, or the Wyrm's fall
def end_beat(+m: Mem) -> Mem:
  +st = g_stage(m)
  end_go(m, Bool.and(nz(count_seats(6n, m, 1)), all_down(6n, m, 1)), Bool.and(lt(st, 2), U32.is_ge(g_t(m), pk(z(st), LEN0(), LEN1()))), Bool.and(eq(st, 2), dying_done(m)))

# ===== a beat starts over from its checkpoint =====

def keep_lamp(+m: Mem, +i: U32) -> Bool: eq(f_kind(m, foe_b(i)), K_LAMP())

def restart_seat(+m: Mem, +id: U32) -> Mem:
  +cb = seat_c(id)
  +bombs = umx(c_bombs(m, cb), 2)
  +pts = c_pts(m, cb)
  +hits = c_hits(m, cb)
  +saved = c_saved(m, cb)
  +gates = c_gates(m, cb)
  +m1 = set_c_hurt(craft_new(m, cb, slot_x(id), slot_z(id)), cb, 60)
  set_c_gates(set_c_saved(set_c_hits(set_c_pts(set_c_bombs(m1, cb, bombs), cb, pts), cb, hits), cb, saved), cb, gates)

def restart_seats(+n: Nat, +m: Mem, +id: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      restart_seats(p, pkm(seated(m, id), restart_seat(m, id), m), U32.add(id, 1))

def restart_wing(+n: Nat, +m: Mem, +k: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      restart_wing(p, set_c_down(set_c_shield(set_w_lost(m, wing_b(k), 0), wing_c(k), SHIELD()), wing_c(k), 0), U32.add(k, 1))

def restart_boss(+m: Mem) -> Mem:
  +b = BOSSB()
  pkm(nz(g_boss(m)), set_bo_nmarks(set_bo_ticks(set_bo_st(m, b, W_IDLE()), b, 60), b, 0), m)

# the beat's foes are gone and its script runs again from there; what was banked stays, as do the Wyrm's lamps
def restart(+m: Mem) -> Mem:
  +m1 = set_g_n_fx(set_g_n_calls(set_g_n_bolts(set_g_n_props(set_g_next(set_g_t(beat_clear(set_g_retries(m, umn(U32.add(g_retries(m), 1), 255))), g_cp(m)), resume(g_stage(m), g_cp(m))), 0), 0), 0), 0)
  +m2 = restart_boss(retain(~keep_lamp, m1, FOES(), FOE_W(), g_n_foes(m1), CA_FOES()))
  +m3 = set_g_mode_t(set_g_mode(restart_wing(nt(g_n_wing(m2)), restart_seats(6n, m2, 1), 0), M_PLAY()), 0)
  say(m3, voice(m3), L_HELLO())

# the Wyrm is down and the fly-through is over: the medal, and the tally
def medal_of(+m: Mem) -> U32:
  +all = umx(g_bank_offered(m), 1)
  +pct = U32.sub(U32.sub(un(Nat.div(Nat.mul(nt(g_bank_base(m)), 100n), nt(all))), U32.mul(5, g_retries(m))), U32.mul(4, g_bank_lost(m)))
  pk(ge(pct, 85), 3, pk(ge(pct, 60), 2, 1))

def finish(+m: Mem) -> Mem:
  +m1 = bank_add(m)
  sound(set_g_mode_t(set_g_mode(set_g_medal(beat_clear(m1), medal_of(m1)), M_TALLY()), 0), SND_PLAY_CARD())

# ===== a bot's hands: what a scripted knight presses to fly Griffinwing =====
# It reads only what a player can see and presses only the buttons a player has. It works in a few
# words of scratch past the session (the answer, nine words: move x, move y, aim x, aim y, fire, dash,
# bomb, interact, swing) and leaves the session alone.

def BOT() -> U32: U32.add(SCRATCH(), 1400)

def bv(+m: Mem, +k: U32) -> U32: rd(m, U32.add(BOT(), k))
def set_bv(+m: Mem, +k: U32, +v: U32) -> Mem: wr(m, U32.add(BOT(), k), v)
def V_WANT() -> U32: 16
def V_WX() -> U32: 17
def V_WZ() -> U32: 18
def V_ROLLS() -> U32: 19
def V_BRAKE() -> U32: 20
def V_ONME() -> U32: 21
def V_TOK() -> U32: 22
def V_TT() -> U32: 23
def V_TPX() -> U32: 24
def V_TPZ() -> U32: 25
def V_TGL() -> U32: 26
def V_SOK() -> U32: 27
def V_ST() -> U32: 28
def V_SPX() -> U32: 29
def V_SPZ() -> U32: 30
def V_LAMP() -> U32: 31
def V_LAT() -> U32: 32
def V_FLN() -> U32: 33
def V_CROWD() -> U32: 34
def V_TARGET() -> U32: 35
def V_K1() -> U32: 36
def V_K2() -> U32: 37
def HOLD_AT_BOT() -> U32: 7
def bv_b(+m: Mem, +k: U32) -> Bool: nz(bv(m, k))

# a steady scatter in -span..=span from a knight, a tick and a salt
def noise(+id: U32, +t: U32, +salt: U32, +span: U32) -> U32:
  +h0 = U32.xor(U32.xor(U32.mul(id, 2654435761), U32.mul(t, 2246822507)), U32.mul(salt, 3266489909))
  +h1 = U32.mul(U32.xor(h0, U32.shrn(h0, 15n)), 739982445)
  +h2 = U32.xor(h1, U32.shrn(h1, 13n))
  U32.sub(U32.mod(h2, U32.add(U32.mul(2, span), 1)), span)

# whether this knight is one of the rollers: the even seats, and any knight at all when no even seat flies
def even_flying(+n: Nat, +m: Mem, +id: U32) -> Bool:
  match n:
    case 0n:
      False{}
    case 1n+p:
      Bool.or(Bool.and(Bool.and(seated(m, id), z(U32.mod(id, 2))), z(c_down(m, seat_c(id)))), even_flying(p, m, U32.add(id, 1)))

def reflector(+m: Mem, +id: U32) -> Bool: Bool.or(z(U32.mod(id, 2)), Bool.not(even_flying(6n, m, 1)))

# the soonest hostile bolt that will pass near this craft's plane: ticks to it, where it will cross, whether it is a Gloam
def thr_one(+m: Mem, +i: U32, +cb: U32, +s_c: U32) -> Mem:
  +bb = bolt_b(i)
  +shot = bl_shot(m, bb)
  +gap = U32.sub(bl_s(m, bb), s_c)
  +closing = mx(U32.sub(PACE(), bl_vs(m, bb)), 1)
  +t = mx(sdv(gap, closing), 0)
  +px = U32.add(bl_x(m, bb), U32.mul(bl_vx(m, bb), t))
  +pz = U32.add(bl_z(m, bb), U32.mul(bl_vz(m, bb), t))
  +ok = Bool.and(Bool.or(eq(shot, A_SPARK()), eq(shot, A_GLOAM())), Bool.and(gt(gap, 0), le(gap, dm(220))))
  +cand = Bool.and(Bool.and(ok, lt(flat(px, pz, c_x(m, cb), c_z(m, cb)), dm(26))), Bool.or(Bool.not(bv_b(m, V_TOK())), U32.is_lt(t, bv(m, V_TT()))))
  pkm(cand, set_bv(set_bv(set_bv(set_bv(set_bv(m, V_TOK(), 1), V_TT(), t), V_TPX(), px), V_TPZ(), pz), V_TGL(), b2u(eq(shot, A_GLOAM()))), m)

def thr_loop(+n: Nat, +m: Mem, +i: U32, +cb: U32, +s_c: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      thr_loop(p, thr_one(m, i, cb, s_c), U32.add(i, 1), cb, s_c)

# while the lit lamp is shut, the soonest of the Wyrm's spits that is owed to it
def icp_one(+m: Mem, +i: U32, +lit: U32, +s_c: U32) -> Mem:
  +bb = bolt_b(i)
  +t = mx(sdv(U32.sub(bl_s(m, bb), s_c), mx(U32.sub(PACE(), bl_vs(m, bb)), 1)), 0)
  +cand = Bool.and(Bool.and(eq(bl_shot(m, bb), A_GLOAM()), eq(bl_src(m, bb), lit)), Bool.and(gt(bl_s(m, bb), s_c), Bool.or(Bool.not(bv_b(m, V_SOK())), U32.is_lt(t, bv(m, V_ST())))))
  pkm(cand, set_bv(set_bv(set_bv(set_bv(m, V_SOK(), 1), V_ST(), t), V_SPX(), U32.add(bl_x(m, bb), U32.mul(bl_vx(m, bb), t))), V_SPZ(), U32.add(bl_z(m, bb), U32.mul(bl_vz(m, bb), t))), m)

def icp_loop(+n: Nat, +m: Mem, +i: U32, +lit: U32, +s_c: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      icp_loop(p, icp_one(m, i, lit, s_c), U32.add(i, 1), lit, s_c)

def shut_lit(+m: Mem, +lit: U32) -> Bool:
  +fi = foe_by_id(m, lit)
  Bool.and(lt(fi, 99), eq(f_st(m, foe_b(fi)), S_SHUT()))

def intercept(+m: Mem, +s_c: U32) -> Mem:
  +lit = lit_lamp(m)
  pkm(Bool.and(nz(g_boss(m)), shut_lit(m, lit)), icp_loop(nt(g_n_bolts(m)), m, 0, lit, s_c), m)

# four ways out of the point (ox, oz): those the steer box allows, the one that ends farthest from it
def room_pick(+m: Mem, +cb: U32, +ox: U32, +oz: U32, +k: U32) -> U32:
  +dx = kind5(k, dm(40), dn(40), 0, 0, 0)
  +dz = kind5(k, 0, 0, dm(30), dn(30), 0)
  flat(cl(U32.add(c_x(m, cb), dx), ng(wx(m)), wx(m)), cl(U32.add(c_z(m, cb), dz), ng(wz(m)), wz(m)), ox, oz)

def want_set(+m: Mem, +x: U32, +zz: U32) -> Mem: set_bv(set_bv(set_bv(m, V_WANT(), 1), V_WX(), x), V_WZ(), zz)

# the last of the best (max_by_key keeps the last of equals): writes the chosen way to the want
def room_from(+m: Mem, +cb: U32, +ox: U32, +oz: U32) -> Mem:
  +d0 = room_pick(m, cb, ox, oz, 0)
  +d1 = room_pick(m, cb, ox, oz, 1)
  +d2 = room_pick(m, cb, ox, oz, 2)
  +d3 = room_pick(m, cb, ox, oz, 3)
  +k1 = pk(U32.is_ge(d1, d0), 1, 0)
  +b1 = umx(d0, d1)
  +k2 = pk(U32.is_ge(d2, b1), 2, k1)
  +b2 = umx(b1, d2)
  +k3 = pk(U32.is_ge(d3, b2), 3, k2)
  +dx = kind5(k3, dm(40), dn(40), 0, 0, 0)
  +dz = kind5(k3, 0, 0, dm(30), dn(30), 0)
  want_set(m, cl(U32.add(c_x(m, cb), dx), ng(wx(m)), wx(m)), cl(U32.add(c_z(m, cb), dz), ng(wz(m)), wz(m)))

# a footprint locked on this craft (a turret's, a bomber's lane): the place to be instead
def foot_marks(+n: Nat, +m: Mem, +fb: U32, +i: U32, +cb: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      +mxx = mark_x(f_fx(m, fb), i)
      +hit = Bool.and(Bool.not(bv_b(m, V_WANT())), lt(flat(c_x(m, cb), c_z(m, cb), mxx, f_fz(m, fb)), dm(24)))
      foot_marks(p, pkm(hit, room_from(m, cb, mxx, f_fz(m, fb)), m), fb, U32.add(i, 1), cb)

def foot_pick(+m: Mem, +fb: U32, +cb: U32, +live: Bool) -> Mem:
  match live:
    case False{}:
      m
    case True{}:
      foot_marks(nt(mark_count(pressure(m))), m, fb, 0, cb)

def foot_foes(+n: Nat, +m: Mem, +i: U32, +cb: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      +fb = foe_b(i)
      +k = f_kind(m, fb)
      +live = Bool.and(eq(f_st(m, fb), S_TELL()), Bool.or(Bool.or(eq(k, K_TURRET()), eq(k, K_BOMBER())), eq(k, K_WISP())))
      foot_foes(p, foot_pick(m, fb, cb, live), U32.add(i, 1), cb)

def footprint_away(+m: Mem, +cb: U32) -> Mem: foot_foes(nt(g_n_foes(m)), m, 0, cb)

# the Wyrm's coil wants the gap, its roar wants the column left
def corridor_go(+m: Mem, +cb: U32, +atk: Nat) -> Mem:
  match atk:
    case 1n:
      want_set(m, bo_gap(m, BOSSB()), c_z(m, cb))
    case 2n:
      +col = bo_col(m, BOSSB())
      +away = pk(ge(c_x(m, cb), col), 1, ng(1))
      pkm(lt(ab(U32.sub(c_x(m, cb), col)), U32.add(COL_HALF(), dm(16))), want_set(m, cl(U32.add(col, U32.mul(away, U32.add(COL_HALF(), dm(30)))), ng(wx(m)), wx(m)), c_z(m, cb)), m)
    case _:
      m

def boss_corridor(+m: Mem, +cb: U32) -> Mem:
  +st = bo_st(m, BOSSB())
  pkm(Bool.and(nz(g_boss(m)), Bool.or(eq(st, W_TELL()), Bool.or(eq(st, W_COIL()), eq(st, W_ROAR())))), corridor_go(m, cb, nt(bo_atk(m, BOSSB()))), m)

# the next thing on the rail to thread or take
def prop_want(+m: Mem, +pb: U32, +id: U32, +cb: U32, +s_c: U32) -> Mem:
  +th = p_thing(m, pb)
  +d = U32.sub(p_s(m, pb), s_c)
  +arch = Bool.and(eq(th, T_ARCH()), lt(d, dm(260)))
  +gate = Bool.and(Bool.and(le(th, T_RING()), z(U32.and(p_taken(m, pb), bit(id)))), Bool.or(eq(th, T_GATE()), lt(c_shield(m, cb), 80)))
  +shut = Bool.and(eq(th, T_SHUTTER()), lt(d, dm(220)))
  +ridge = Bool.and(eq(th, T_RIDGE()), Bool.and(lt(ab(U32.sub(p_x(m, pb), c_x(m, cb))), U32.add(p_w(m, pb), dm(16))), lt(d, dm(200))))
  +ticks = mx(sdv(d, PACE()), 0)
  +m1 = pkm(Bool.or(arch, gate), want_set(m, p_x(m, pb), p_z(m, pb)),
        pkm(shut, want_set(m, p_x(m, pb), c_z(m, cb)),
          pkm(ridge, want_set(m, U32.mul(ng(sg(p_x(m, pb))), dm(20)), c_z(m, cb)), m)))
  +m2 = pkm(Bool.and(shut, lt(shutter_gap(m, pb, U32.add(g_t(m), ticks)), dm(16))), set_bv(m1, V_BRAKE(), 1), m1)
  m2

def props_want(+n: Nat, +m: Mem, +i: U32, +id: U32, +cb: U32, +s_c: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      +pb = prop_b(i)
      +d = U32.sub(p_s(m, pb), s_c)
      props_want(p, pkm(Bool.and(Bool.not(bv_b(m, V_WANT())), Bool.and(ge(d, 0), le(d, dm(300)))), prop_want(m, pb, id, cb, s_c), m), U32.add(i, 1), id, cb, s_c)

# a friend with a kite on their tail, level with this knight: drop back until the chaser is in front
def chaser_one(+m: Mem, +i: U32, +id: U32, +s_c: U32, +found: Bool) -> Bool:
  +fb = foe_b(i)
  Bool.and(Bool.and(eq(f_st(m, fb), S_LATCHED()), Bool.and(le(f_on(m, fb), 6), Bool.and(ne(f_on(m, fb), id), nz(f_on(m, fb))))), Bool.not(found))

def chaser_loop(+n: Nat, +m: Mem, +i: U32, +id: U32, +s_c: U32, +found: Bool, +brake: Bool) -> Bool:
  match n:
    case 0n:
      brake
    case 1n+p:
      +hit = chaser_one(m, i, id, s_c, found)
      chaser_loop(p, m, U32.add(i, 1), id, s_c, Bool.or(found, hit), pb(hit, lt(U32.sub(f_s(m, foe_b(i)), s_c), dm(30)), brake))

# what a bot presses to move: where it wants to be, the buttons of the roll
def steer(+from: U32, +to: U32, +dead: U32) -> U32: pk(le(ab(U32.sub(to, from)), dead), 0, sg(U32.sub(to, from)))

def bot_move(+m: Mem, +id: U32, +cb: U32) -> Mem:
  +wnt = bv_b(m, V_WANT())
  +tx = pk(wnt, bv(m, V_WX()), 0)
  +tz = pk(wnt, bv(m, V_WZ()), 0)
  wr(wr(wr(m, U32.add(BOT(), 0), steer(c_x(m, cb), tx, dm(8))), U32.add(BOT(), 1), ng(steer(c_z(m, cb), tz, dm(8)))), U32.add(BOT(), 8), bv(m, V_BRAKE()))

def not_dash(+m: Mem, +cb: U32) -> U32: b2u(z(U32.and(c_keys(m, cb), K_DASH())))

# the lockable foes (what a player can see and a lock would take), and what the reticle seeks
def lk_ok(+m: Mem, +fb: U32, +s_c: U32) -> Bool:
  +near_ = pk(eq(f_st(m, fb), S_LATCHED()), dm(15), dm(40))
  +ds = U32.sub(f_s(m, fb), s_c)
  Bool.and(f_lockable(m, fb), Bool.and(ge(ds, near_), le(ds, dm(480))))

def in_locks(+n: Nat, +m: Mem, +cb: U32, +k: U32, +id: U32) -> Bool:
  match n:
    case 0n:
      False{}
    case 1n+p:
      Bool.or(eq(c_lock_foe(m, cb, k), id), in_locks(p, m, cb, U32.add(k, 1), id))

def scan_one(+m: Mem, +i: U32, +cb: U32, +s_c: U32, +held: Bool) -> Mem:
  +fb = foe_b(i)
  +ok = lk_ok(m, fb, s_c)
  +lamp = eq(f_kind(m, fb), K_LAMP())
  +fl = Bool.and(ok, Bool.not(lamp))
  +ds = U32.sub(f_s(m, fb), s_c)
  +m1 = pkm(Bool.and(Bool.and(ok, lamp), eq(bv(m, V_LAMP()), 99)), set_bv(m, V_LAMP(), i), m)
  +m2 = pkm(Bool.and(Bool.and(ok, eq(f_st(m, fb), S_LATCHED())), Bool.or(eq(bv(m1, V_LAT()), 99), lt(f_s(m1, fb), f_s(m1, foe_b(bv(m1, V_LAT())))))), set_bv(m1, V_LAT(), i), m1)
  +m3 = pkm(fl, set_bv(set_bv(m2, V_FLN(), U32.add(bv(m2, V_FLN()), 1)), V_CROWD(), U32.add(bv(m2, V_CROWD()), b2u(lt(ds, dm(260))))), m2)
  +key1 = pk(held, U32.add(ab(U32.sub(f_x(m, fb), c_x(m, cb))), ab(U32.sub(f_z(m, fb), c_z(m, cb)))), ds)
  +cand = Bool.and(fl, Bool.and(pb(held, Bool.not(in_locks(nt(c_nlocks(m, cb)), m, cb, 0, f_id(m, fb))), True{}),
    Bool.or(eq(bv(m3, V_TARGET()), 99), Bool.or(lt(key1, bv(m3, V_K1())), Bool.and(Bool.and(Bool.not(held), eq(key1, bv(m3, V_K1()))), lt(f_id(m, fb), bv(m3, V_K2())))))))
  pkm(cand, set_bv(set_bv(set_bv(m3, V_TARGET(), i), V_K1(), key1), V_K2(), f_id(m, fb)), m3)

def scan_loop(+n: Nat, +m: Mem, +i: U32, +cb: U32, +s_c: U32, +held: Bool) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      scan_loop(p, scan_one(m, i, cb, s_c, held), U32.add(i, 1), cb, s_c, held)

def noise_t(+m: Mem, +id: U32, +div: U32, +salt: U32, +span: U32) -> U32: noise(id, U32.div(g_t(m), div), salt, span)

# the reticle seeks its target, or comes home
def bot_aim(+m: Mem, +id: U32, +cb: U32, +s_c: U32, +ti: U32, +novice: Bool) -> Mem:
  +fb = foe_b(ti)
  +ds = mx(U32.sub(f_s(m, fb), s_c), dm(30))
  +jx = pk(novice, noise_t(m, id, 14, 1, dm(15)), 0)
  +jz = pk(novice, noise_t(m, id, 14, 2, dm(10)), 0)
  +rx = cl(sdv(U32.mul(U32.add(U32.sub(f_x(m, fb), c_x(m, cb)), jx), FAR()), ds), ng(RX_MAX()), RX_MAX())
  +rz = cl(sdv(U32.mul(U32.add(U32.sub(f_z(m, fb), c_z(m, cb)), jz), FAR()), ds), ng(RZ_MAX()), RZ_MAX())
  +dead = U32.div(U32.mul(RET_V(), 3), 4)
  wr(wr(m, U32.add(BOT(), 2), steer(c_rx(m, cb), rx, dead)), U32.add(BOT(), 3), ng(steer(c_rz(m, cb), rz, dead)))

def bot_home(+m: Mem, +cb: U32) -> Mem:
  wr(wr(m, U32.add(BOT(), 2), steer(c_rx(m, cb), 0, RET_V())), U32.add(BOT(), 3), ng(steer(c_rz(m, cb), 0, RET_V())))

def bot_aim_pick(+m: Mem, +id: U32, +cb: U32, +s_c: U32, +novice: Bool) -> Mem:
  +ti = bv(m, V_TARGET())
  pkm(lt(ti, 99), bot_aim(m, id, cb, s_c, ti, novice), bot_home(m, cb))

# the hands on the fire key
def bot_fire(+m: Mem, +id: U32, +cb: U32, +novice: Bool) -> Mem:
  +total = lock_total(nt(c_nlocks(m, cb)), m, cb, 0)
  +lamp_o = lt(bv(m, V_LAMP()), 99)
  +lat_n = eq(bv(m, V_LAT()), 99)
  +on_lamp = Bool.and(lamp_o, lat_n)
  +want_locks = pk(on_lamp, 3, umn(bv(m, V_FLN()), 6))
  +held = U32.is_gt(c_held(m, cb), HOLD_AT_BOT())
  +ch = g_num_charge(m)
  +early = Bool.and(novice, eq(noise_t(m, id, 90, 3, 2), 0))
  +done = pb(on_lamp, Bool.or(Bool.and(U32.is_ge(c_charge(m, cb), ch), U32.is_ge(total, 2)), Bool.and(early, U32.is_ge(c_charge(m, cb), U32.div(ch, 2)))),
    Bool.or(Bool.and(U32.is_ge(total, want_locks), U32.is_ge(c_charge(m, cb), U32.div(ch, 2))), U32.is_ge(c_charge(m, cb), ch)))
  +cluster = U32.is_ge(bv(m, V_FLN()), 3)
  +fresh = z(U32.and(c_keys(m, cb), K_FIRE()))
  +fire = pb(held, Bool.not(done), pb(nz(c_held(m, cb)), True{}, pb(Bool.or(lamp_o, cluster), fresh, pb(lt(bv(m, V_TARGET()), 99), fresh, False{}))))
  +bomb = Bool.and(Bool.and(le(c_shield(m, cb), 35), U32.is_ge(bv(m, V_CROWD()), 3)), Bool.and(gt(c_bombs(m, cb), 0), z(U32.and(c_keys(m, cb), K_BOMB()))))
  wr(wr(m, U32.add(BOT(), 4), b2u(fire)), U32.add(BOT(), 6), b2u(bomb))

def bot_shoot(+m: Mem, +id: U32, +cb: U32, +s_c: U32, +novice: Bool) -> Mem:
  +held = U32.is_gt(c_held(m, cb), HOLD_AT_BOT())
  +m1 = scan_loop(nt(g_n_foes(m)), set_bv(set_bv(set_bv(set_bv(set_bv(m, V_LAMP(), 99), V_LAT(), 99), V_FLN(), 0), V_CROWD(), 0), V_TARGET(), 99), 0, cb, s_c, held)
  +ti = pk(lt(bv(m1, V_LAT()), 99), bv(m1, V_LAT()), pk(lt(bv(m1, V_LAMP()), 99), bv(m1, V_LAMP()), bv(m1, V_TARGET())))
  bot_fire(bot_aim_pick(set_bv(m1, V_TARGET(), ti), id, cb, s_c, novice), id, cb, novice)

# the roll: down on the key when the last tick did not hold it, and sideways away from the threat
def bot_roll(+m: Mem, +cb: U32) -> Mem:
  +side = pk(Bool.and(bv_b(m, V_TOK()), ge(bv(m, V_TPX()), c_x(m, cb))), ng(1), 1)
  +m1 = wr(m, U32.add(BOT(), 5), not_dash(m, cb))
  pkm(Bool.and(bv_b(m, V_ONME()), Bool.not(bv_b(m, V_TOK()))), m1, wr(m1, BOT(), side))

# where to be: round what is coming, through a gate, away from a footprint, to the middle of an opening
def bot_want(+m: Mem, +id: U32, +cb: U32, +s_c: U32, +novice: Bool) -> Mem:
  +m1 = pkm(Bool.not(bv_b(m, V_WANT())), footprint_away(m, cb), m)
  +m2 = pkm(Bool.not(bv_b(m1, V_WANT())), boss_corridor(m1, cb), m1)
  pkm(Bool.not(bv_b(m2, V_WANT())), props_want(nt(g_n_props(m2)), m2, 0, id, cb, s_c), m2)

# the threat or the Wyrm's spit decides: roll into it, step out of its way, or neither
def bot_react(+m: Mem, +id: U32, +cb: U32, +novice: Bool) -> Mem:
  +idle = Bool.and(z(c_rest(m, cb)), z(c_roll(m, cb)))
  +t = bv(m, V_TT())
  +seek_t = bv(m, V_ST())
  +willing = Bool.or(Bool.not(novice), ge(noise_t(m, id, 60, 5, 1), 0))
  +m_seek = set_bv(want_set(m, bv(m, V_SPX()), bv(m, V_SPZ())), V_ROLLS(), b2u(Bool.and(Bool.and(willing, idle), U32.is_le(seek_t, pk(novice, 6, 8)))))
  +eager = Bool.or(reflector(m, id), U32.is_le(t, 6))
  +slip = Bool.and(novice, eq(noise_t(m, id, 45, 9, 2), 0))
  +roll_now = Bool.and(Bool.and(Bool.and(idle, bv_b(m, V_TGL())), Bool.and(eager, U32.is_le(t, pk(novice, 6, 9)))), Bool.not(slip))
  +step_out = Bool.and(Bool.not(bv_b(m, V_ROLLS())), U32.is_le(t, pk(novice, 22, 40)))
  +m_thr = pkm(roll_now, set_bv(m, V_ROLLS(), 1), pkm(Bool.and(Bool.not(roll_now), step_out), room_from(m, cb, bv(m, V_TPX()), bv(m, V_TPZ())), m))
  pkm(bv_b(m, V_SOK()), m_seek, pkm(bv_b(m, V_TOK()), m_thr, m))

def bot_play(+m: Mem, +id: U32, +cb: U32, +novice: Bool) -> Mem:
  +s_c = cr_s(m, cb)
  +on_me = latched(m, id)
  +m0 = set_bv(set_bv(set_bv(set_bv(set_bv(set_bv(m, V_WANT(), 0), V_SOK(), 0), V_TOK(), 0), V_ONME(), b2u(on_me)), V_ROLLS(), b2u(Bool.and(on_me, Bool.and(z(c_rest(m, cb)), z(c_roll(m, cb)))))),
    V_BRAKE(), b2u(chaser_loop(nt(g_n_foes(m)), m, 0, id, s_c, False{}, False{})))
  +m1b = pkm(reflector(m0, id), intercept(m0, s_c), m0)
  +m2 = thr_loop(nt(g_n_bolts(m1b)), m1b, 0, cb, s_c)
  +m3 = bot_react(m2, id, cb, novice)
  +m4 = bot_move(bot_want(m3, id, cb, s_c, novice), id, cb)
  pkm(bv_b(m4, V_ROLLS()), bot_roll(m4, cb), bot_shoot(m4, id, cb, s_c, novice))

def bot_zero(+n: Nat, +m: Mem, +k: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      bot_zero(p, set_bv(m, k, 0), U32.add(k, 1))

def bot_mode(+m: Mem, +id: U32, +novice: Bool, +tally: Bool, +off: Bool) -> Mem:
  match tally off:
    case True{} _:
      set_bv(m, 7, b2u(z(U32.mod(g_mode_t(m), 2))))
    case False{} True{}:
      m
    case False{} False{}:
      bot_play(m, id, seat_c(id), novice)

def bot_go(+m: Mem, +id: U32, +novice: Bool, +alive: Bool) -> Mem:
  match alive:
    case False{}:
      m
    case True{}:
      bot_mode(m, id, novice, eq(g_mode(m), M_TALLY()), Bool.or(eq(g_mode(m), M_RETRY()), nz(c_down(m, seat_c(id)))))

# what knight `id` presses this tick: nine words at the start of the bot's scratch
def bot(+m: Mem, +id: U32, +novice: Bool) -> Mem: bot_go(bot_zero(48n, m, 0), id, novice, seated(m, id))

# ===== one tick =====

def fly_seats(+n: Nat, +m: Mem, +id: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      fly_seats(p, pkm(seated(m, id), fly(m, id), m), U32.add(id, 1))

# the keys of this tick stand for the next one's presses
def keys_seats(+n: Nat, +m: Mem, +id: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      keys_seats(p, pkm(seated(m, id), set_c_keys(m, seat_c(id), key_bits(m, id)), m), U32.add(id, 1))

def leave_push(+m: Mem, +id: U32) -> Mem: set_g_n_leave(set_g_leave(m, g_n_leave(m), id), U32.add(g_n_leave(m), 1))

# the tally holds, then a fresh press of E leaves
def leave_seats(+n: Nat, +m: Mem, +id: U32) -> Mem:
  match n:
    case 0n:
      m
    case 1n+p:
      leave_seats(p, pkm(Bool.and(Bool.and(seated(m, id), z(U32.and(c_keys(m, seat_c(id)), K_SPUR()))), k_interact(m, id)), leave_push(m, id), m), U32.add(id, 1))

def play_tick(+m: Mem) -> Mem:
  +m1 = fly_seats(6n, set_g_t(m, umn(U32.add(g_t(m), 1), LIMIT())), 1)
  end_beat(tick_lists(step_craft(step_props(step_bolts(step_boss(step_foes(run_script(wing_fly(m1)))))))))

def retry_tick(+m: Mem) -> Mem:
  +m1 = set_g_mode_t(tick_lists(m), ssub(g_mode_t(m), 1))
  pkm(z(g_mode_t(m1)), restart(m1), m1)

def after_tally(+m: Mem, +mode: U32) -> Mem:
  pkm(Bool.and(eq(mode, M_VICTORY()), U32.is_ge(g_mode_t(m), VICTORY())), finish(m),
    pkm(Bool.and(eq(mode, M_TALLY()), U32.is_gt(g_mode_t(m), TALLY_HOLD())), leave_seats(6n, m, 1), m))

def home_tick(+m: Mem) -> Mem:
  +mode = g_mode(m)
  +m0 = pkm(eq(mode, M_VICTORY()), set_g_t(m, umn(U32.add(g_t(m), 1), LIMIT())), m)
  +m1 = tick_lists(step_craft(step_props(step_bolts(wing_fly(fly_seats(6n, m0, 1))))))
  after_tally(set_g_mode_t(m1, umn(U32.add(g_mode_t(m1), 1), LIMIT())), mode)

def tick_mode(+m: Mem, +mode: Nat) -> Mem:
  match mode:
    case 0n:
      play_tick(m)
    case 1n:
      retry_tick(m)
    case _:
      home_tick(m)

# one tick: the seats are whoever is here and awake; then the beat
def tick_after(+m: Mem) -> Mem: keys_seats(6n, tick_mode(m, nt(g_mode(m))), 1)

def tick(+m: Mem) -> Mem: tick_after(pre(m))
