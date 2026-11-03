import Base

# The 3D races' rules, moved out of Rust. Rust keeps the course geometry it
# draws, the public state types, the saves' wire shape and the rewards' text;
# this leg owns the simulation step: gravity, throttle, hops, pickups, rotten
# planks, crossing carts, the rope, the title clock and the finish order.
#
# Status: the Shaft Run and the race-level rules (grid, title clock, finish
# order, pot and laurels) live here. The Ore Cart, the Griffin Run and the
# Torchlight Touge are still native; see .seat/REPORT.md.
#
# Every `(.. : F32)` rounds once, exactly as Rust's f32 did, so both legs agree
# bit for bit. `hyp` (sqrt of squares) and `deg` are spelled out so both legs
# compute the same bits; trigonometry is Bend's F32.sin/F32.cos.
#
# Wreck codes: 0 none, 1 fall, 2 bonk, 3 buffer, 4 gap, 5 timber, 6 tumble,
# 7 spin. Ground codes: 0 floor, 1 bridge, 2 crossing, 3 lodestone, 4 sluice,
# 5 collapse, 6 sorter, 7 rails, 8 trestle, 9 gorge. Item kinds: 0 gem,
# 1 ring, 2 gargoyle, 3 timber. Sound codes are the adapter's table.

type Drift is Data:
  Drift{
    heading: F32, travel: F32, sliding: Bool, charge: U32, boost: U32,
    stage: U32, corner: U32, last_corner: U32, score: U32, corners: U32
  }

type Rd is Data:
  Rd{
    id: U32, drift: Drift, s: F32, x: F32, z: F32, vs: F32, vx: F32, vz: F32,
    lane: U32, spin: F32, crash: U32, wreck: U32, crashes: U32, stun: U32,
    burst: U32, rolling: U32, rest: U32, reload: U32, haul: U32,
    rings_hi: U32, rings_lo: U32, fin: U32, keys: U32
  }

type Bolt is Data:
  Bolt{owner: U32, s: F32, x: F32, z: F32, life: U32}

type Smp is Data:
  Smp{slope: F32, bend: F32, width: F32, walls: Bool, ground: U32, lanes: U32, funnel: Bool}

type It is Data:
  It{s: F32, x: F32, z: F32, kind: U32}

type Bl is Data:
  Bl{s: F32, len: F32, x0: F32, x1: F32, z0: F32, z1: F32}

type Gp is Data:
  Gp{lane: U32, s0: F32, s1: F32}

type Trk is Data:
  Trk{
    n: U32, looped: Bool, finish: F32,
    samples: List<&2, Smp>, posts: List<&2, F32>, items: List<&2, It>,
    blocks: List<&2, Bl>, gaps: List<&2, Gp>
  }

type Cart is Data:
  Cart{s: F32, x: F32}

type Seat is Data:
  Seat{id: U32, mx: F32, my: F32, fire: Bool, interact: Bool, dash: Bool, standing: Bool}

type Rs is Data:
  Rs{t: U32, taken: List<&2, U32>, planks: List<&2, U32>, bolts: List<&2, Bolt>}

type Ra is Data:
  Ra{
    t: U32, done: Bool, ended: U32, practice: Bool, crossroads: Bool,
    won: List<&2, U32>, ante: List<&2, U32>
  }

type In is Data:
  In{
    ra: Ra, rs: Rs, seats: List<&2, Seat>, riders: List<&2, Rd>,
    carts: List<&2, Cart>, track: Trk, course: U32
  }

type Step is Data:
  Step{r: Rd, rs: Rs, snd: List<&2, U32>}

type Pl is Data:
  Pl{planks: List<&2, U32>, began: U32}

type Pick is Data:
  Pick{taken: List<&2, U32>, haul: U32, snd: List<&2, U32>}

type Vz is Data:
  Vz{z: F32, vz: F32, snd: List<&2, U32>}

type Pr is Data:
  Pr{stop: Bool, r: Rd, rs: Rs, snd: List<&2, U32>}

type Loop is Data:
  Loop{riders: List<&2, Rd>, rs: Rs, snd: List<&2, U32>}

type Gr is Data:
  Gr{s: F32, x: F32, lane: U32}

type Out is Data:
  Out{
    ra: Ra, rs: Rs, riders: List<&2, Rd>, snd: List<&2, U32>,
    best: U32, best_home: Bool, practice: Bool, cleared: Bool,
    rivals: Bool, mark: Bool, won: List<&2, U32>
  }

def rd_id(+x: Rd) -> U32:
  match x:
    case Rd{+id, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      id

def rd_drift(+x: Rd) -> Drift:
  match x:
    case Rd{_, +drift, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      drift

def rd_s(+x: Rd) -> F32:
  match x:
    case Rd{_, _, +s, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      s

def rd_x(+x: Rd) -> F32:
  match x:
    case Rd{_, _, _, +x, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      x

def rd_z(+x: Rd) -> F32:
  match x:
    case Rd{_, _, _, _, +z, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      z

def rd_vs(+x: Rd) -> F32:
  match x:
    case Rd{_, _, _, _, _, +vs, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      vs

def rd_vx(+x: Rd) -> F32:
  match x:
    case Rd{_, _, _, _, _, _, +vx, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      vx

def rd_vz(+x: Rd) -> F32:
  match x:
    case Rd{_, _, _, _, _, _, _, +vz, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      vz

def rd_lane(+x: Rd) -> U32:
  match x:
    case Rd{_, _, _, _, _, _, _, _, +lane, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      lane

def rd_spin(+x: Rd) -> F32:
  match x:
    case Rd{_, _, _, _, _, _, _, _, _, +spin, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      spin

def rd_crash(+x: Rd) -> U32:
  match x:
    case Rd{_, _, _, _, _, _, _, _, _, _, +crash, _, _, _, _, _, _, _, _, _, _, _, _}:
      crash

def rd_wreck(+x: Rd) -> U32:
  match x:
    case Rd{_, _, _, _, _, _, _, _, _, _, _, +wreck, _, _, _, _, _, _, _, _, _, _, _}:
      wreck

def rd_crashes(+x: Rd) -> U32:
  match x:
    case Rd{_, _, _, _, _, _, _, _, _, _, _, _, +crashes, _, _, _, _, _, _, _, _, _, _}:
      crashes

def rd_stun(+x: Rd) -> U32:
  match x:
    case Rd{_, _, _, _, _, _, _, _, _, _, _, _, _, +stun, _, _, _, _, _, _, _, _, _}:
      stun

def rd_burst(+x: Rd) -> U32:
  match x:
    case Rd{_, _, _, _, _, _, _, _, _, _, _, _, _, _, +burst, _, _, _, _, _, _, _, _}:
      burst

def rd_rolling(+x: Rd) -> U32:
  match x:
    case Rd{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, +rolling, _, _, _, _, _, _, _}:
      rolling

def rd_rest(+x: Rd) -> U32:
  match x:
    case Rd{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, +rest, _, _, _, _, _, _}:
      rest

def rd_reload(+x: Rd) -> U32:
  match x:
    case Rd{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, +reload, _, _, _, _, _}:
      reload

def rd_haul(+x: Rd) -> U32:
  match x:
    case Rd{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, +haul, _, _, _, _}:
      haul

def rd_rings_hi(+x: Rd) -> U32:
  match x:
    case Rd{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, +rings_hi, _, _, _}:
      rings_hi

def rd_rings_lo(+x: Rd) -> U32:
  match x:
    case Rd{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, +rings_lo, _, _}:
      rings_lo

def rd_fin(+x: Rd) -> U32:
  match x:
    case Rd{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, +fin, _}:
      fin

def rd_keys(+x: Rd) -> U32:
  match x:
    case Rd{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, +keys}:
      keys

def seat_id(+x: Seat) -> U32:
  match x:
    case Seat{+id, _, _, _, _, _, _}:
      id

def seat_mx(+x: Seat) -> F32:
  match x:
    case Seat{_, +mx, _, _, _, _, _}:
      mx

def seat_my(+x: Seat) -> F32:
  match x:
    case Seat{_, _, +my, _, _, _, _}:
      my

def seat_fire(+x: Seat) -> Bool:
  match x:
    case Seat{_, _, _, +fire, _, _, _}:
      fire

def seat_interact(+x: Seat) -> Bool:
  match x:
    case Seat{_, _, _, _, +interact, _, _}:
      interact

def seat_dash(+x: Seat) -> Bool:
  match x:
    case Seat{_, _, _, _, _, +dash, _}:
      dash

def seat_standing(+x: Seat) -> Bool:
  match x:
    case Seat{_, _, _, _, _, _, +standing}:
      standing

def it_s(+x: It) -> F32:
  match x:
    case It{+s, _, _, _}:
      s

def it_x(+x: It) -> F32:
  match x:
    case It{_, +x, _, _}:
      x

def it_z(+x: It) -> F32:
  match x:
    case It{_, _, +z, _}:
      z

def it_kind(+x: It) -> U32:
  match x:
    case It{_, _, _, +kind}:
      kind

def cart_s(+x: Cart) -> F32:
  match x:
    case Cart{+s, _}:
      s

def cart_x(+x: Cart) -> F32:
  match x:
    case Cart{_, +x}:
      x

def ra_t(+x: Ra) -> U32:
  match x:
    case Ra{+t, _, _, _, _, _, _}:
      t

def ra_done(+x: Ra) -> Bool:
  match x:
    case Ra{_, +done, _, _, _, _, _}:
      done

def ra_ended(+x: Ra) -> U32:
  match x:
    case Ra{_, _, +ended, _, _, _, _}:
      ended

def ra_practice(+x: Ra) -> Bool:
  match x:
    case Ra{_, _, _, +practice, _, _, _}:
      practice

def ra_crossroads(+x: Ra) -> Bool:
  match x:
    case Ra{_, _, _, _, +crossroads, _, _}:
      crossroads

def ra_won(+x: Ra) -> List<&2, U32>:
  match x:
    case Ra{_, _, _, _, _, +won, _}:
      won

def ra_ante(+x: Ra) -> List<&2, U32>:
  match x:
    case Ra{_, _, _, _, _, _, +ante}:
      ante

def st_r(+x: Step) -> Rd:
  match x:
    case Step{+r, _, _}:
      r

def st_rs(+x: Step) -> Rs:
  match x:
    case Step{_, +rs, _}:
      rs

def st_snd(+x: Step) -> List<&2, U32>:
  match x:
    case Step{_, _, +snd}:
      snd

def pick_taken(+x: Pick) -> List<&2, U32>:
  match x:
    case Pick{+taken, _, _}:
      taken

def pick_haul(+x: Pick) -> U32:
  match x:
    case Pick{_, +haul, _}:
      haul

def pick_snd(+x: Pick) -> List<&2, U32>:
  match x:
    case Pick{_, _, +snd}:
      snd

def pr_stop(+x: Pr) -> Bool:
  match x:
    case Pr{+stop, _, _, _}:
      stop

def pr_r(+x: Pr) -> Rd:
  match x:
    case Pr{_, +r, _, _}:
      r

def pr_rs(+x: Pr) -> Rs:
  match x:
    case Pr{_, _, +rs, _}:
      rs

def pr_snd(+x: Pr) -> List<&2, U32>:
  match x:
    case Pr{_, _, _, +snd}:
      snd

def loop_riders(+x: Loop) -> List<&2, Rd>:
  match x:
    case Loop{+riders, _, _}:
      riders

def loop_rs(+x: Loop) -> Rs:
  match x:
    case Loop{_, +rs, _}:
      rs

def loop_snd(+x: Loop) -> List<&2, U32>:
  match x:
    case Loop{_, _, +snd}:
      snd

def gr_s(+x: Gr) -> F32:
  match x:
    case Gr{+s, _, _}:
      s

def gr_x(+x: Gr) -> F32:
  match x:
    case Gr{_, +x, _}:
      x

def gr_lane(+x: Gr) -> U32:
  match x:
    case Gr{_, _, +lane}:
      lane

def s_slope(+x: Smp) -> F32:
  match x:
    case Smp{+slope, _, _, _, _, _, _}:
      slope

def s_bend(+x: Smp) -> F32:
  match x:
    case Smp{_, +bend, _, _, _, _, _}:
      bend

def s_width(+x: Smp) -> F32:
  match x:
    case Smp{_, _, +width, _, _, _, _}:
      width

def s_walls(+x: Smp) -> Bool:
  match x:
    case Smp{_, _, _, +walls, _, _, _}:
      walls

def s_ground(+x: Smp) -> U32:
  match x:
    case Smp{_, _, _, _, +ground, _, _}:
      ground

def s_lanes(+x: Smp) -> U32:
  match x:
    case Smp{_, _, _, _, _, +lanes, _}:
      lanes

def s_funnel(+x: Smp) -> Bool:
  match x:
    case Smp{_, _, _, _, _, _, +funnel}:
      funnel

def gp_lane(+x: Gp) -> U32:
  match x:
    case Gp{+lane, _, _}:
      lane

def gp_s0(+x: Gp) -> F32:
  match x:
    case Gp{_, +s0, _}:
      s0

def gp_s1(+x: Gp) -> F32:
  match x:
    case Gp{_, _, +s1}:
      s1

def to_u(+x: F32) -> U32:
  U32.from_nat(F32.to_nat(F32.floor(x)))

def dt() -> F32:
  (1.0 / 30.0 : F32)

def usub(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_lt(a, b), 0, (a - b : U32))

def choose_sign(+neg: Bool, +neg_zero: Bool) -> F32:
  match neg neg_zero:
    case True{} _:
      (0.0 - 1.0 : F32)
    case False{} True{}:
      (0.0 - 1.0 : F32)
    case False{} False{}:
      1.0

def fsignum(+v: F32) -> F32:
  choose_sign(F32.is_lt(v, 0.0), U32.is_eq(F32.bits(v), 2147483648))

def hyp(+a: F32, +b: F32) -> F32:
  F32.sqrt(((a * a : F32) + (b * b : F32) : F32))

def sn_roll() -> U32:
  0

def sn_clang() -> U32:
  1

def sn_gold() -> U32:
  2

def sn_fall() -> U32:
  3

def sn_pop() -> U32:
  4

def sn_rock() -> U32:
  5

def sn_land() -> U32:
  6

def sn_wet() -> U32:
  11

def sn_toll() -> U32:
  12

def sn_card() -> U32:
  14

def choose_wreck_sound(+fall: Bool, +gap: Bool, +bonk: Bool, +timber: Bool, +buffer: Bool) -> U32:
  match fall gap bonk timber buffer:
    case True{} _ _ _ _:
      sn_fall()
    case False{} True{} _ _ _:
      sn_fall()
    case False{} False{} True{} _ _:
      sn_pop()
    case False{} False{} False{} True{} _:
      sn_pop()
    case False{} False{} False{} False{} True{}:
      sn_clang()
    case False{} False{} False{} False{} False{}:
      sn_rock()

def wreck_sound(+what: U32) -> U32:
  choose_wreck_sound(U32.is_eq(what, 1), U32.is_eq(what, 4), U32.is_eq(what, 2), U32.is_eq(what, 5), U32.is_eq(what, 3))

def maybe_snd(+on: Bool, +code: U32) -> List<&2, U32>:
  match on:
    case True{}:
      Con{code, Nil{}}
    case False{}:
      Nil{}

def snd_cat(+a: List<&2, U32>, +b: List<&2, U32>) -> List<&2, U32>:
  match a:
    case Nil{}:
      b
    case Con{+head, +tail}:
      Con{head, snd_cat(tail, b)}

def reverse(+xs: List<&2, Rd>, +acc: List<&2, Rd>) -> List<&2, Rd>:
  match xs:
    case Nil{}:
      acc
    case Con{+head, +tail}:
      reverse(tail, Con{head, acc})

def len(+xs: List<&2, Rd>, +acc: U32) -> U32:
  match xs:
    case Nil{}:
      acc
    case Con{+head, +tail}:
      len(tail, (acc + 1 : U32))

def nth_smp(+xs: List<&2, Smp>, +i: U32, +zero: Bool) -> Smp:
  match xs zero:
    case Con{+head, +tail} True{}:
      head
    case Con{+head, +tail} False{}:
      nth_smp(tail, (i - 1 : U32), U32.is_zero((i - 1 : U32)))
    case Nil{} _:
      Smp{0.0, 0.0, 0.0, False{}, 0, 0, False{}}

def nth_it(+xs: List<&2, It>, +i: U32, +zero: Bool) -> It:
  match xs zero:
    case Con{+head, +tail} True{}:
      head
    case Con{+head, +tail} False{}:
      nth_it(tail, (i - 1 : U32), U32.is_zero((i - 1 : U32)))
    case Nil{} _:
      It{0.0, 0.0, 0.0, 0}

def post_one(+acc: F32, +p: F32, +s: F32) -> F32:
  Bool.pick(F32, F32.is_le(p, s), F32.max(acc, p), acc)

def post_go(+ps: List<&2, F32>, +s: F32, +acc: F32) -> F32:
  match ps:
    case Nil{}:
      acc
    case Con{+head, +tail}:
      post_go(tail, s, post_one(acc, head, s))

def wrap_if(+looped: Bool, +len: F32, +s: F32) -> F32:
  match looped:
    case True{}:
      (s - (len * F32.floor((s / len : F32)) : F32) : F32)
    case False{}:
      F32.clamp(s, 0.0, len)

def at(+t: Trk, +s: F32) -> Smp:
  match t:
    case Trk{+n, +looped, +finish, +samples, +posts, +items, +blocks, +gaps}:
      +w = wrap_if(looped, U32.to_f32((n - 1 : U32)), s)
      +i = U32.min(to_u(w), usub(n, 2))
      nth_smp(samples, i, U32.is_zero(i))

def tlen(+t: Trk) -> F32:
  match t:
    case Trk{+n, +looped, +finish, +samples, +posts, +items, +blocks, +gaps}:
      U32.to_f32((n - 1 : U32))

def tfinish(+t: Trk) -> F32:
  match t:
    case Trk{+n, +looped, +finish, +samples, +posts, +items, +blocks, +gaps}:
      finish

def titems(+t: Trk) -> List<&2, It>:
  match t:
    case Trk{+n, +looped, +finish, +samples, +posts, +items, +blocks, +gaps}:
      items

def post(+t: Trk, +s: F32) -> F32:
  match t:
    case Trk{+n, +looped, +finish, +samples, +posts, +items, +blocks, +gaps}:
      post_go(posts, s, 0.0)

def gap_hit(+head: Gp, +s: F32, +lane: U32) -> Bool:
  Bool.and(U32.is_eq(gp_lane(head), lane), Bool.and(F32.is_ge(s, gp_s0(head)), F32.is_lt(s, gp_s1(head))))

def broken_go(+gs: List<&2, Gp>, +s: F32, +lane: U32) -> Bool:
  match gs:
    case Nil{}:
      False{}
    case Con{+head, +tail}:
      Bool.or(gap_hit(head, s, lane), broken_go(tail, s, lane))

def broken(+t: Trk, +s: F32, +lane: U32) -> Bool:
  match t:
    case Trk{+n, +looped, +finish, +samples, +posts, +items, +blocks, +gaps}:
      broken_go(gaps, s, lane)

def laid(+t: Trk, +s: F32, +lane: U32) -> Bool:
  +sm = at(t, s)
  U32.is_ne(U32.and(s_lanes(sm), U32.shln(1, U32.to_nat(lane))), 0)

def rotten(+plank: U32) -> Bool:
  U32.is_eq(U32.mod(plank, 2), 1)

def taken_has(+taken: List<&2, U32>, +i: U32) -> Bool:
  match taken:
    case Nil{}:
      False{}
    case Con{+head, +tail}:
      Bool.or(U32.is_eq(head, i), taken_has(tail, i))

def taken_add(+taken: List<&2, U32>, +i: U32) -> List<&2, U32>:
  Con{i, taken}

def some_or(+m: Maybe<&2, U32>, +d: U32) -> U32:
  match m:
    case None{}:
      d
    case Some{+x}:
      x

def maybe_first(+hit: Bool, +when: U32, +rest: Maybe<&2, U32>) -> Maybe<&2, U32>:
  match hit:
    case True{}:
      Some{when}
    case False{}:
      rest

def plank_when(+ps: List<&2, U32>, +plank: U32) -> Maybe<&2, U32>:
  match ps:
    case Nil{}:
      None{}
    case Con{+key, +tail}:
      match tail:
        case Con{+when, +rest}:
          maybe_first(U32.is_eq(key, plank), when, plank_when(rest, plank))
        case Nil{}:
          None{}

def pair_has(+ps: List<&2, U32>, +plank: U32) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{+key, +tail}:
      match tail:
        case Con{+when, +rest}:
          Bool.or(U32.is_eq(key, plank), pair_has(rest, plank))
        case Nil{}:
          U32.is_eq(key, plank)

def append_pair(+ps: List<&2, U32>, +plank: U32, +t: U32) -> List<&2, U32>:
  match ps:
    case Nil{}:
      Con{plank, Con{t, Nil{}}}
    case Con{+key, +tail}:
      match tail:
        case Con{+when, +rest}:
          Con{key, Con{when, append_pair(rest, plank, t)}}
        case Nil{}:
          Con{key, Con{t, Nil{}}}

def plank_began(+planks: List<&2, U32>, +plank: U32, +t: U32) -> Pl:
  Pl{Bool.pick(List<&2, U32>, pair_has(planks, plank), planks, append_pair(planks, plank, t)), some_or(plank_when(planks, plank), t)}

def rs_of(+t: U32, +taken: List<&2, U32>, +planks: List<&2, U32>, +bolts: List<&2, Bolt>) -> Rs:
  Rs{t, taken, planks, bolts}

def step_of(+r: Rd, +rs: Rs, +snd: List<&2, U32>) -> Step:
  Step{r, rs, snd}

def pr_of(+stop: Bool, +r: Rd, +rs: Rs, +snd: List<&2, U32>) -> Pr:
  Pr{stop, r, rs, snd}

def d_default() -> Drift:
  Drift{0.0, 0.0, False{}, 0, 0, 0, 0, 0, 0, 0}

def rd0() -> Rd:
  Rd{0, d_default(), 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0, 0.0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}

def rider_new(+id: U32, +s: F32, +x: F32, +lane: U32) -> Rd:
  Rd{id, d_default(), s, x, 0.0, 0.0, 0.0, 0.0, lane, 0.0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}

def mot(+r: Rd, +s: F32, +x: F32, +z: F32, +vs: F32, +vx: F32, +vz: F32, +lane: U32, +spin: F32) -> Rd:
  Rd{rd_id(r), rd_drift(r), s, x, z, vs, vx, vz, lane, spin, rd_crash(r), rd_wreck(r), rd_crashes(r), rd_stun(r), rd_burst(r), rd_rolling(r), rd_rest(r), rd_reload(r), rd_haul(r), rd_rings_hi(r), rd_rings_lo(r), rd_fin(r), rd_keys(r)}

def cnt(+r: Rd, +crash: U32, +wreck: U32, +crashes: U32, +stun: U32, +burst: U32, +rolling: U32, +rest: U32, +reload: U32, +haul: U32) -> Rd:
  Rd{rd_id(r), rd_drift(r), rd_s(r), rd_x(r), rd_z(r), rd_vs(r), rd_vx(r), rd_vz(r), rd_lane(r), rd_spin(r), crash, wreck, crashes, stun, burst, rolling, rest, reload, haul, rd_rings_hi(r), rd_rings_lo(r), rd_fin(r), rd_keys(r)}

def bits(+r: Rd, +rings_hi: U32, +rings_lo: U32, +fin: U32, +keys: U32) -> Rd:
  Rd{rd_id(r), rd_drift(r), rd_s(r), rd_x(r), rd_z(r), rd_vs(r), rd_vx(r), rd_vz(r), rd_lane(r), rd_spin(r), rd_crash(r), rd_wreck(r), rd_crashes(r), rd_stun(r), rd_burst(r), rd_rolling(r), rd_rest(r), rd_reload(r), rd_haul(r), rings_hi, rings_lo, fin, keys}

def wrecked(+r: Rd, +what: U32) -> Rd:
  cnt(r, 45, what, (rd_crashes(r) + 1 : U32), rd_stun(r), 0, rd_rolling(r), rd_rest(r), rd_reload(r), rd_haul(r))

def vert_land_if(+z: F32, +vz: F32, +down: Bool) -> Vz:
  match down:
    case True{}:
      Vz{0.0, 0.0, Con{sn_land(), Nil{}}}
    case False{}:
      Vz{z, vz, Nil{}}

def vert(+z: F32, +vz: F32, +grav: F32, +active: Bool) -> Vz:
  match active:
    case False{}:
      Vz{z, vz, Nil{}}
    case True{}:
      +nz = (z + ((vz - (grav * dt() : F32) : F32) * dt() : F32) : F32)
      vert_land_if(nz, (vz - (grav * dt() : F32) : F32), F32.is_le(nz, 0.0))

def rope_back(+t: Trk, +r: Rd, +x: F32) -> Rd:
  mot(r, F32.max((post(t, rd_s(r)) - 6.0 : F32), 1.0), x, 0.0, 0.0, 0.0, 0.0, rd_lane(r), rd_spin(r))

def pick_hit(+head: It, +s: F32, +x: F32, +z: F32, +reach: F32, +taken: List<&2, U32>, +haul: U32, +i: U32) -> Pick:
  +yes = Bool.and(U32.is_eq(it_kind(head), 0), Bool.and(Bool.not(taken_has(taken, i)), Bool.and(F32.is_le(F32.abs((it_s(head) - s : F32)), reach), F32.is_le(hyp((it_x(head) - x : F32), (it_z(head) - (z + 0.5 : F32) : F32)), (reach + 0.2 : F32)))))
  Pick{Bool.pick(List<&2, U32>, yes, taken_add(taken, i), taken), Bool.pick(U32, yes, (haul + 1 : U32), haul), maybe_snd(yes, sn_gold())}

def pick_go(+items: List<&2, It>, +i: U32, +s: F32, +x: F32, +z: F32, +reach: F32, +taken: List<&2, U32>, +haul: U32) -> Pick:
  match items:
    case Nil{}:
      Pick{taken, haul, Nil{}}
    case Con{+head, +tail}:
      +got = pick_hit(head, s, x, z, reach, taken, haul, i)
      +rest = pick_go(tail, (i + 1 : U32), s, x, z, reach, pick_taken(got), pick_haul(got))
      Pick{pick_taken(rest), pick_haul(rest), snd_cat(pick_snd(got), pick_snd(rest))}

def lane_of(+left: Bool, +right: Bool) -> F32:
  match left right:
    case True{} _:
      (0.0 - 1.7 : F32)
    case False{} True{}:
      1.7
    case False{} False{}:
      0.0

def lane_x(+lane: U32) -> F32:
  lane_of(U32.is_eq(lane, 0), U32.is_eq(lane, 2))

def pair_grid(+k: U32, +step: F32, +across: F32) -> Gr:
  Gr{(2.0 + (U32.to_f32(U32.div(k, 2)) * step : F32) : F32), Bool.pick(F32, U32.is_eq(U32.mod(k, 2), 0), (0.0 - across : F32), across), 1}

def cart_rows(+k: U32, +i: U32, +lane: U32, +r0: Bool, +r1: Bool, +r3: Bool) -> Gr:
  match r0 r1 r3:
    case True{} _ _:
      Gr{6.0, lane_x(lane), lane}
    case False{} True{} _:
      Gr{4.5, lane_x(lane), lane}
    case False{} False{} True{}:
      Gr{2.5, lane_x(lane), lane}
    case False{} False{} False{}:
      Gr{Bool.pick(F32, U32.is_eq(i, 2), 4.5, 1.0), lane_x(lane), lane}

def lane_pick(+r0: Bool, +r2: Bool, +r3: Bool, +r5: Bool) -> U32:
  match r0 r2 r3 r5:
    case True{} _ _ _:
      1
    case False{} True{} _ _:
      2
    case False{} False{} True{} _:
      1
    case False{} False{} False{} True{}:
      2
    case False{} False{} False{} False{}:
      0

def gen_cart(+k: U32, +i: U32, +lane: U32) -> Gr:
  cart_rows(k, i, lane, U32.is_eq(i, 0), U32.is_eq(i, 1), U32.is_eq(i, 3))

def finish_of(+finish: F32, +touge: Bool) -> F32:
  match touge:
    case True{}:
      (finish * 3.0 : F32)
    case False{}:
      finish

def finish_distance(+t: Trk, +course: U32) -> F32:
  finish_of(tfinish(t), U32.is_eq(course, 3))

def limit_of(+touge: Bool, +crossroads: Bool) -> U32:
  match touge crossroads:
    case True{} False{}:
      9000
    case True{} True{}:
      2700
    case False{} _:
      2700

def time_limit(+course: U32, +crossroads: Bool) -> U32:
  limit_of(U32.is_eq(course, 3), crossroads)

def lane_row(+i: U32) -> U32:
  lane_pick(U32.is_eq(i, 0), U32.is_eq(i, 2), U32.is_eq(i, 3), U32.is_eq(i, 5))

def grid_of(+cart: Bool, +touge: Bool, +griffin: Bool, +k: U32) -> Gr:
  match cart touge griffin:
    case True{} _ _:
      gen_cart(k, U32.mod(k, 6), lane_row(U32.mod(k, 6)))
    case False{} True{} _:
      Gr{0.0, ((U32.to_f32(k) - 1.5 : F32) * 1.6 : F32), 1}
    case False{} False{} True{}:
      pair_grid(k, 2.0, 1.6)
    case False{} False{} False{}:
      pair_grid(k, 1.4, 1.0)

def grid(+course: U32, +k: U32) -> Gr:
  grid_of(U32.is_eq(course, 1), U32.is_eq(course, 3), U32.is_eq(course, 2), k)

def rs_taken(+x: Rs) -> List<&2, U32>:
  match x:
    case Rs{_, +taken, _, _}:
      taken

def rs_planks(+x: Rs) -> List<&2, U32>:
  match x:
    case Rs{_, _, +planks, _}:
      planks

def rs_bolts(+x: Rs) -> List<&2, Bolt>:
  match x:
    case Rs{_, _, _, +bolts}:
      bolts

def rs_t(+x: Rs) -> U32:
  match x:
    case Rs{+t, _, _, _}:
      t

def pl_planks(+x: Pl) -> List<&2, U32>:
  match x:
    case Pl{+planks, _}:
      planks

def pl_began(+x: Pl) -> U32:
  match x:
    case Pl{_, +began}:
      began

def v_z(+x: Vz) -> F32:
  match x:
    case Vz{+z, _, _}:
      z

def v_vz(+x: Vz) -> F32:
  match x:
    case Vz{_, +vz, _}:
      vz

def v_snd(+x: Vz) -> List<&2, U32>:
  match x:
    case Vz{_, _, +snd}:
      snd

def pick_up(+rs: Rs, +r: Rd, +t: Trk, +reach: F32) -> Pick:
  pick_go(titems(t), 0, rd_s(r), rd_x(r), rd_z(r), reach, rs_taken(rs), rd_haul(r))

# The course table Rust draws, handed in once and read by the tick.

def track(+n: U32, +looped: Bool, +finish: F32, +samples: List<&2, Smp>, +posts: List<&2, F32>, +items: List<&2, It>, +blocks: List<&2, Bl>, +gaps: List<&2, Gp>) -> Trk:
  Trk{n, looped, finish, samples, posts, items, blocks, gaps}

# ─── the race tick: the state in, the next state out ────────────────────────

# What a rider's held keys say this tick: the two presses it just made and the
# bits it carries into the next tick.
type Kp is Data:
  Kp{f: Bool, e: Bool, keys: U32}

# One rider's tick as Rust hands it over: their seat's move axes and the one
# sine the Shaft's sluice reads, taken from the position before the step. A
# transcendental is the adapter's to compute, so both legs hold the same bits.
type One is Data:
  One{seat: Seat, sin_s: F32}

# The tick in, and the tick out.
type Tick is Data:
  Tick{
    t: U32, course: U32, finish: F32, limit: U32, riders: List<&2, Rd>,
    ones: List<&2, One>, rs: Rs, trk: Trk
  }

type Tk is Data:
  Tk{
    riders: List<&2, Rd>, rs: Rs, snd: List<&2, U32>,
    all_home: Bool, timed_out: Bool
  }

# A rider's moving numbers as one tick walks them.
type Mv is Data:
  Mv{
    s: F32, x: F32, z: F32, vs: F32, vx: F32, vz: F32, spin: F32,
    stun: U32, rest: U32, haul: U32, keys: U32
  }

# A step in progress: the rider as the tick found it (any wreck already
# applied), its moving numbers, and the race's own state.
type Tx is Data:
  Tx{done: Bool, r: Rd, m: Mv, rs: Rs, snd: List<&2, U32>}

type Hit is Data:
  Hit{hit: Bool, cx: F32}

type Duo is Data:
  Duo{a: Rd, b: Rd, snd: List<&2, U32>}

type Ids is Data:
  Ids{aid: U32, bid: U32}

type Bs is Data:
  Bs{riders: List<&2, Rd>, snd: List<&2, U32>}

# The tick rate and the title card in ticks.
def hz() -> U32:
  30

def mk_mv(+s: F32, +x: F32, +z: F32, +vs: F32, +vx: F32, +vz: F32, +spin: F32, +stun: U32, +rest: U32, +haul: U32, +keys: U32) -> Mv:
  Mv{s, x, z, vs, vx, vz, spin, stun, rest, haul, keys}

def title_ticks() -> U32:
  (3 * hz() : U32)

def fall_g() -> F32:
  (9.0 : F32)

def push_a() -> F32:
  (3.2 : F32)

def brake_a() -> F32:
  (7.0 : F32)

def drag_v() -> F32:
  (0.12 : F32)

def top_v() -> F32:
  (11.0 : F32)

def steer_v() -> F32:
  (7.0 : F32)

def hop_v() -> F32:
  (4.6 : F32)

def hop_g() -> F32:
  (14.0 : F32)

def ball_r() -> F32:
  (0.5 : F32)

def rot_ticks() -> U32:
  hz()

def roll_rest() -> U32:
  45

def bonk_ticks() -> U32:
  20

def add_if(+on: Bool, +a: F32, +b: F32) -> F32:
  match on:
    case True{}:
      (a + b : F32)
    case False{}:
      a

def sub_if(+on: Bool, +a: F32, +b: F32) -> F32:
  match on:
    case True{}:
      (a - b : F32)
    case False{}:
      a

def one_if(+on: Bool) -> U32:
  match on:
    case True{}:
      1
    case False{}:
      0

def rd_if(+on: Bool, +yes: Rd, +no: Rd) -> Rd:
  match on:
    case True{}:
      yes
    case False{}:
      no

def one_seat(+o: One) -> Seat:
  match o:
    case One{+seat, _}:
      seat

def kp_f(+k: Kp) -> Bool:
  match k:
    case Kp{+f, _, _}:
      f

def kp_keys(+k: Kp) -> U32:
  match k:
    case Kp{_, _, +keys}:
      keys

def hit_yes(+h: Hit) -> Bool:
  match h:
    case Hit{+hit, _}:
      hit

def hit_cx(+h: Hit) -> F32:
  match h:
    case Hit{_, +cx}:
      cx

def tx_rs(+p: Tx) -> Rs:
  match p:
    case Tx{_, _, _, +rs, _}:
      rs

def tx_snd(+p: Tx) -> List<&2, U32>:
  match p:
    case Tx{_, _, _, _, +snd}:
      snd

# F and E as held last tick (bits 0 and 1), for presses.
def press(+s: Seat, +keys: U32) -> Kp:
  match s:
    case Seat{_, _, _, +fire, +interact, _, _}:
      Kp{
        Bool.and(fire, U32.is_eq(U32.and(keys, 1), 0)),
        Bool.and(interact, U32.is_eq(U32.and(keys, 2), 0)),
        U32.or(one_if(fire), Bool.pick(U32, interact, 2, 0))
      }

# The rider as the tick leaves it: its moving numbers over the rider it began
# as, so crash, burst, rings and finish keep whatever the step set.
def mv_rd(+m: Mv, +r: Rd) -> Rd:
  match m:
    case Mv{+s, +x, +z, +vs, +vx, +vz, +spin, +stun, +rest, +haul, +keys}:
      +moved = cnt(
        mot(r, s, x, z, vs, vx, vz, rd_lane(r), spin),
        rd_crash(r), rd_wreck(r), rd_crashes(r), stun, rd_burst(r),
        rd_rolling(r), rest, rd_reload(r), haul
      )
      bits(moved, rd_rings_hi(r), rd_rings_lo(r), rd_fin(r), keys)

def mv_after(+p: Tx) -> Rd:
  match p:
    case Tx{_, +r, +m, _, _}:
      mv_rd(m, r)

def keys_rd(+r: Rd, +keys: U32) -> Rd:
  bits(r, rd_rings_hi(r), rd_rings_lo(r), rd_fin(r), keys)

# A wrecked cart: the crash is the bonk's shorter clock and the ball is let
# through the rest of the crossing.
def bonk(+r: Rd) -> Rd:
  +w = wrecked(r, 2)
  cnt(w, bonk_ticks(), 2, rd_crashes(w), rd_stun(w), 0, rd_rolling(w), roll_rest(), rd_reload(w), rd_haul(w))

def rope_if(+on: Bool, +t: Trk, +r: Rd) -> Rd:
  match on:
    case True{}:
      rope_back(t, r, 0.0)
    case False{}:
      r

# ── the crossing carts ──

def cross_x_at(+p: F32, +under: Bool) -> F32:
  match under:
    case True{}:
      ((0.0 - 4.0 : F32) + (8.0 * p : F32) : F32)
    case False{}:
      (9.0 : F32)

def cross_x(+p: F32) -> F32:
  cross_x_at(p, F32.is_lt(p, 1.0))

def cross_phase(+t: U32, +offset: U32) -> F32:
  (U32.to_f32(U32.mod((t + offset : U32), 120)) / 60.0 : F32)

def cross_one(+p: F32, +t: U32) -> List<&2, Cart>:
  Con{
    Cart{(p + 6.0 : F32), cross_x(cross_phase(t, 0))},
    Con{Cart{(p + 13.0 : F32), F32.neg(cross_x(cross_phase(t, 60)))}, Nil{}}
  }

def cat_carts(+a: List<&2, Cart>, +b: List<&2, Cart>) -> List<&2, Cart>:
  match a:
    case Nil{}:
      b
    case Con{+head, +tail}:
      Con{head, cat_carts(tail, b)}

def post_push(+ps: List<&2, F32>, +p: F32) -> List<&2, F32>:
  match ps:
    case Nil{}:
      Con{p, Nil{}}
    case Con{+head, +tail}:
      Con{head, post_push(tail, p)}

def post_keep(+keep: Bool, +head: F32, +acc: List<&2, F32>) -> List<&2, F32>:
  match keep:
    case True{}:
      post_push(acc, head)
    case False{}:
      acc

def post_pick(+ps: List<&2, F32>, +trk: Trk, +acc: List<&2, F32>) -> List<&2, F32>:
  match ps:
    case Nil{}:
      acc
    case Con{+head, +tail}:
      post_pick(tail, trk, post_keep(U32.is_eq(s_ground(at(trk, head)), 2), head, acc))

def cross_gen(+ps: List<&2, F32>, +t: U32, +acc: List<&2, Cart>) -> List<&2, Cart>:
  match ps:
    case Nil{}:
      acc
    case Con{+head, +tail}:
      cross_gen(tail, t, cat_carts(acc, cross_one(head, t)))

def cross(+t: Trk, +tick: U32) -> List<&2, Cart>:
  match t:
    case Trk{+n, +looped, +finish, +samples, +posts, +items, +blocks, +gaps}:
      cross_gen(post_pick(posts, t, Nil{}), tick, Nil{})

def cart_one(+head: Cart, +s: F32, +x: F32) -> Bool:
  Bool.and(F32.is_lt(F32.abs((cart_s(head) - s : F32)), 1.3), F32.is_lt(F32.abs((cart_x(head) - x : F32)), 1.6))

def cart_scan(+cs: List<&2, Cart>, +s: F32, +x: F32, +hit: Bool, +cx: F32) -> Hit:
  match cs:
    case Nil{}:
      Hit{hit, cx}
    case Con{+head, +tail}:
      +now = Bool.and(Bool.not(hit), cart_one(head, s, x))
      cart_scan(tail, s, x, Bool.or(hit, now), Bool.pick(F32, now, cart_x(head), cx))

def cart_find(+cs: List<&2, Cart>, +s: F32, +x: F32) -> Hit:
  cart_scan(cs, s, x, False{}, 0.0)

# ── the Shaft Run ──

# Into the sorter: the ball rolls to a stop at the bottom.
def sh_home(+r: Rd, +rs: Rs, +trk: Trk) -> Pr:
  +vs1 = (rd_vs(r) * 0.9 : F32)
  +s1 = F32.min((rd_s(r) + (vs1 * dt() : F32) : F32), (tlen(trk) - 1.0 : F32))
  +spin1 = (rd_spin(r) + ((vs1 * dt() : F32) / ball_r() : F32) : F32)
  pr_of(True{}, mot(r, s1, rd_x(r), rd_z(r), vs1, rd_vx(r), rd_vz(r), rd_lane(r), spin1), rs, Nil{})

# Down the shaft, tumbling, until the rope comes for it; or out of it, slowing.
def sh_crash(+r: Rd, +rs: Rs, +trk: Trk) -> Pr:
  +fall = U32.is_eq(rd_wreck(r), 1)
  +crash1 = (rd_crash(r) - 1 : U32)
  +vs1 = Bool.pick(F32, fall, rd_vs(r), (rd_vs(r) * 0.85 : F32))
  +vz1 = Bool.pick(F32, fall, (rd_vz(r) - (hop_g() * dt() : F32) : F32), rd_vz(r))
  +z1 = Bool.pick(F32, fall, F32.max((rd_z(r) + (vz1 * dt() : F32) : F32), (0.0 - 14.0 : F32)), rd_z(r))
  +s1 = Bool.pick(F32, fall, (rd_s(r) + ((vs1 * dt() : F32) * 0.5 : F32) : F32), F32.max((rd_s(r) + (vs1 * dt() : F32) : F32), 0.5))
  +x1 = Bool.pick(F32, fall, (rd_x(r) + ((rd_vx(r) * dt() : F32) * 0.5 : F32) : F32), (rd_x(r) + (rd_vx(r) * dt() : F32) : F32))
  +vx1 = Bool.pick(F32, fall, rd_vx(r), (rd_vx(r) * 0.85 : F32))
  +spin1 = Bool.pick(F32, fall, (rd_spin(r) + 0.5 : F32), rd_spin(r))
  +done = U32.is_zero(crash1)
  +base = cnt(mot(r, s1, x1, z1, vs1, vx1, vz1, rd_lane(r), spin1), crash1, Bool.pick(U32, done, 0, rd_wreck(r)), rd_crashes(r), rd_stun(r), rd_burst(r), rd_rolling(r), rd_rest(r), rd_reload(r), rd_haul(r))
  pr_of(True{}, rope_if(Bool.and(done, fall), trk, base), rs, Nil{})

# The walls, or the edge.
def sh_walls_if(+done: Bool, +r: Rd, +m: Mv, +rs: Rs, +snd: List<&2, U32>, +trk: Trk) -> Tx:
  match done:
    case True{}:
      Tx{True{}, r, m, rs, snd}
    case False{}:
      match m:
        case Mv{+s, +x, +z, +vs, +vx, +vz, +spin, +stun, +rest, +haul, +keys}:
          +sm = at(trk, s)
          +half = (s_width(sm) * 0.5 : F32)
          +walls = s_walls(sm)
          +hit = Bool.and(walls, F32.is_gt(F32.abs(x), (half - ball_r() : F32)))
          +x1 = Bool.pick(F32, hit, (fsignum(x) * (half - ball_r() : F32) : F32), x)
          +vx1 = Bool.pick(F32, hit, Bool.pick(F32, s_funnel(sm), 0.0, (F32.neg(vx) * 0.45 : F32)), vx)
          +vs1 = Bool.pick(F32, hit, (vs * 0.94 : F32), vs)
          +snd1 = snd_cat(snd, maybe_snd(Bool.and(hit, F32.is_gt(F32.abs(vx1), 1.5)), sn_clang()))
          +fell = Bool.and(Bool.not(walls), Bool.and(F32.is_gt(F32.abs(x), half), F32.is_le(z, 0.05)))
          Tx{
            fell,
            rd_if(fell, wrecked(r, 1), r),
            Mv{s, x1, z, vs1, Bool.pick(F32, fell, (fsignum(x) * 2.0 : F32), vx1), Bool.pick(F32, fell, 0.0, vz), spin, stun, rest, haul, keys},
            rs,
            snd_cat(snd1, maybe_snd(fell, wreck_sound(1)))
          }

def sh_walls(+p: Tx, +trk: Trk) -> Tx:
  match p:
    case Tx{+done, +r, +m, +rs, +snd}:
      sh_walls_if(done, r, m, rs, snd, trk)

# Rotten planks: the first ball over one starts it going; a ball on a plank
# that has gone goes with it.
def sh_planks_if(+done: Bool, +r: Rd, +m: Mv, +rs: Rs, +snd: List<&2, U32>, +trk: Trk, +t: U32) -> Tx:
  match done:
    case True{}:
      Tx{True{}, r, m, rs, snd}
    case False{}:
      match m:
        case Mv{+s, +x, +z, +vs, +vx, +vz, +spin, +stun, +rest, +haul, +keys}:
          +plank = to_u((s / 2.0 : F32))
          +rot = Bool.and(U32.is_eq(s_ground(at(trk, s)), 5), rotten(plank))
          +pb = plank_began(rs_planks(rs), plank, t)
          +go = Bool.and(rot, Bool.and(U32.is_ge(t, (pl_began(pb) + rot_ticks() : U32)), F32.is_le(z, 0.05)))
          Tx{
            go,
            rd_if(go, wrecked(r, 1), r),
            Mv{s, x, z, vs, vx, Bool.pick(F32, go, 0.0, vz), spin, stun, rest, haul, keys},
            Rs{rs_t(rs), rs_taken(rs), Bool.pick(List<&2, U32>, rot, pl_planks(pb), rs_planks(rs)), rs_bolts(rs)},
            snd_cat(snd, maybe_snd(go, wreck_sound(1)))
          }

def sh_planks(+p: Tx, +trk: Trk, +t: U32) -> Tx:
  match p:
    case Tx{+done, +r, +m, +rs, +snd}:
      sh_planks_if(done, r, m, rs, snd, trk, t)

# The crossing ore carts.
def sh_carts_if(+done: Bool, +r: Rd, +m: Mv, +rs: Rs, +snd: List<&2, U32>, +trk: Trk, +t: U32) -> Tx:
  match done:
    case True{}:
      Tx{True{}, r, m, rs, snd}
    case False{}:
      match m:
        case Mv{+s, +x, +z, +vs, +vx, +vz, +spin, +stun, +rest, +haul, +keys}:
          +rest1 = usub(rest, 1)
          +check = Bool.and(Bool.and(U32.is_eq(s_ground(at(trk, s)), 2), F32.is_lt(z, 1.0)), U32.is_zero(rest1))
          +h = Bool.pick(Hit, check, cart_find(cross(trk, t), s, x), Hit{False{}, 0.0})
          +hit = hit_yes(h)
          Tx{
            hit,
            rd_if(hit, bonk(r), r),
            Mv{
              s, x, z,
              Bool.pick(F32, hit, (0.0 - 3.0 : F32), vs),
              Bool.pick(F32, hit, Bool.pick(F32, F32.is_lt(hit_cx(h), x), 5.0, (0.0 - 5.0 : F32)), vx),
              vz, spin, stun, Bool.pick(U32, hit, roll_rest(), rest1), haul, keys
            },
            rs,
            snd_cat(snd, maybe_snd(hit, wreck_sound(2)))
          }

def sh_carts(+p: Tx, +trk: Trk, +t: U32) -> Tx:
  match p:
    case Tx{+done, +r, +m, +rs, +snd}:
      sh_carts_if(done, r, m, rs, snd, trk, t)

# Gems on the way: the first ball to one takes it.
def sh_pick_if(+done: Bool, +r: Rd, +m: Mv, +rs: Rs, +snd: List<&2, U32>, +trk: Trk) -> Tx:
  match done:
    case True{}:
      Tx{True{}, r, m, rs, snd}
    case False{}:
      match m:
        case Mv{+s, +x, +z, +vs, +vx, +vz, +spin, +stun, +rest, +haul, +keys}:
          +got = pick_up(rs, mv_rd(m, r), trk, 0.9)
          Tx{
            False{}, r,
            Mv{s, x, z, vs, vx, vz, spin, stun, rest, pick_haul(got), keys},
            Rs{rs_t(rs), pick_taken(got), rs_planks(rs), rs_bolts(rs)},
            snd_cat(snd, pick_snd(got))
          }

def sh_pick(+p: Tx, +trk: Trk) -> Tx:
  match p:
    case Tx{+done, +r, +m, +rs, +snd}:
      sh_pick_if(done, r, m, rs, snd, trk)

# The ball racing: along, across, up, and then what it met.
def sh_live(+r: Rd, +seat: Seat, +kp: Kp, +sin_s: F32, +rs: Rs, +trk: Trk, +t: U32) -> Pr:
  match seat:
    case Seat{_, +mx, +my, _, +interact, _, _}:
      +pf = kp_f(kp)
      +awake = U32.is_zero(rd_stun(r))
      +sm = at(trk, rd_s(r))
      +grounded = F32.is_le(rd_z(r), 0.0)
      +sluice = U32.is_eq(s_ground(sm), 4)
      +lode = Bool.and(U32.is_eq(s_ground(sm), 3), grounded)
      +off = (1.3 - rd_x(r) : F32)
      +a0 = (fall_g() * s_slope(sm) : F32)
      +a1 = add_if(Bool.and(awake, F32.is_lt(my, 0.0)), a0, push_a())
      +a2 = sub_if(Bool.and(awake, Bool.or(F32.is_gt(my, 0.0), interact)), a1, (brake_a() * fsignum(rd_vs(r)) : F32))
      +a3 = add_if(sluice, a2, 2.0)
      +snd0 = maybe_snd(Bool.and(sluice, U32.is_eq(U32.mod(t, 20), 0)), sn_wet())
      +ax0 = Bool.pick(F32, Bool.and(lode, F32.is_lt(F32.abs(off), 1.8)), (3.2 * fsignum(off) : F32), 0.0)
      +a4 = add_if(Bool.and(lode, F32.is_lt(F32.abs(off), 0.35)), a3, 3.0)
      +ax1 = add_if(sluice, ax0, (4.0 * sin_s : F32))
      +vs1 = F32.clamp((rd_vs(r) + ((a4 - (drag_v() * rd_vs(r) : F32) : F32) * dt() : F32) : F32), (0.0 - 4.0 : F32), top_v())
      +ax2 = add_if(awake, ax1, (steer_v() * mx : F32))
      +stun1 = usub(rd_stun(r), 1)
      +ax3 = (ax2 - ((vs1 * vs1 : F32) * s_bend(sm) : F32) : F32)
      +vx1 = ((rd_vx(r) + (ax3 * dt() : F32) : F32) * Bool.pick(F32, grounded, 0.97, 1.0) : F32)
      +hop = Bool.and(pf, grounded)
      +vz1 = Bool.pick(F32, hop, hop_v(), rd_vz(r))
      +snd1 = snd_cat(snd0, maybe_snd(hop, sn_roll()))
      +air = Bool.or(Bool.not(grounded), F32.is_gt(vz1, 0.0))
      +vz2 = Bool.pick(F32, air, (vz1 - (hop_g() * dt() : F32) : F32), vz1)
      +z1 = Bool.pick(F32, air, (rd_z(r) + (vz2 * dt() : F32) : F32), rd_z(r))
      +landed = Bool.and(air, F32.is_le(z1, 0.0))
      +z2 = Bool.pick(F32, landed, 0.0, z1)
      +vz3 = Bool.pick(F32, landed, 0.0, vz2)
      +snd2 = snd_cat(snd1, maybe_snd(landed, sn_land()))
      +s1 = (rd_s(r) + (vs1 * dt() : F32) : F32)
      +x1 = (rd_x(r) + (vx1 * dt() : F32) : F32)
      +spin1 = (rd_spin(r) + ((vs1 * dt() : F32) / ball_r() : F32) : F32)
      +back = F32.is_lt(s1, 0.5)
      +m = mk_mv(
        Bool.pick(F32, back, (0.5 : F32), s1), x1, z2,
        Bool.pick(F32, back, (F32.abs(vs1) * 0.4 : F32), vs1), vx1, vz3,
        spin1, stun1, rd_rest(r), rd_haul(r), kp_keys(kp)
      )
      +after = sh_pick(sh_carts(sh_planks(sh_walls(Tx{False{}, r, m, rs, snd2}, trk), trk, t), trk, t), trk)
      pr_of(True{}, mv_after(after), tx_rs(after), tx_snd(after))

# A race's tick for one rider: the Shaft's ball today; the other three
# courses keep their seats until their own port.
def shaft_state(+r: Rd, +keyed: Rd, +seat: Seat, +kp: Kp, +sin_s: F32, +rs: Rs, +trk: Trk, +t: U32, +racing: Bool, +on_track: Bool) -> Pr:
  match racing on_track:
    case False{} _:
      sh_home(keyed, rs, trk)
    case True{} False{}:
      sh_crash(keyed, rs, trk)
    case True{} True{}:
      sh_live(r, seat, kp, sin_s, rs, trk, t)

def shaft(+r: Rd, +o: One, +rs: Rs, +trk: Trk, +t: U32) -> Pr:
  match o:
    case One{+seat, +sin_s}:
      +kp = press(seat, rd_keys(r))
      shaft_state(r, keys_rd(r, kp_keys(kp)), seat, kp, sin_s, rs, trk, t, U32.is_zero(rd_fin(r)), U32.is_zero(rd_crash(r)))

def adv_course(+r: Rd, +o: One, +rs: Rs, +trk: Trk, +t: U32, +shaft_run: Bool) -> Pr:
  match shaft_run:
    case True{}:
      shaft(r, o, rs, trk, t)
    case False{}:
      pr_of(True{}, r, rs, Nil{})

def adv(+live: Bool, +r: Rd, +o: One, +rs: Rs, +trk: Trk, +t: U32, +course: U32) -> Pr:
  match live:
    case False{}:
      pr_of(True{}, r, rs, Nil{})
    case True{}:
      adv_course(r, o, rs, trk, t, U32.is_eq(course, 0))

# The finish line: whoever crosses it while racing is home at this tick.
def after_step(+p: Pr, +t: U32, +finish: F32) -> Pr:
  match p:
    case Pr{+stop, +r, +rs, +snd}:
      +home = Bool.and(U32.is_zero(rd_fin(r)), Bool.and(U32.is_zero(rd_crash(r)), F32.is_ge(rd_s(r), finish)))
      pr_of(stop, Bool.pick(Rd, home, bits(r, rd_rings_hi(r), rd_rings_lo(r), t, rd_keys(r)), r), rs, snd_cat(snd, maybe_snd(home, sn_land())))

def run_riders(+riders: List<&2, Rd>, +ones: List<&2, One>, +rs: Rs, +trk: Trk, +t: U32, +finish: F32, +course: U32, +acc: List<&2, Rd>, +snd: List<&2, U32>) -> Loop:
  match riders ones:
    case Nil{} _:
      Loop{reverse(acc, Nil{}), rs, snd}
    case Con{+head, +tail} Con{+ohead, +otail}:
      +p = after_step(adv(seat_standing(one_seat(ohead)), head, ohead, rs, trk, t, course), t, finish)
      run_riders(tail, otail, pr_rs(p), trk, t, finish, course, Con{pr_r(p), acc}, snd_cat(snd, pr_snd(p)))
    case Con{_, _} Nil{}:
      Loop{reverse(acc, Nil{}), rs, snd}

def pair_a(+p: Duo) -> Rd:
  match p:
    case Duo{+a, _, _}:
      a

def pair_b(+p: Duo) -> Rd:
  match p:
    case Duo{_, +b, _}:
      b

def pair_snd(+p: Duo) -> List<&2, U32>:
  match p:
    case Duo{_, _, +snd}:
      snd

def id_a(+p: Ids) -> U32:
  match p:
    case Ids{+aid, _}:
      aid

def id_b(+p: Ids) -> U32:
  match p:
    case Ids{_, +bid}:
      bid

# The pairs Rust's nested bump loop visits, in its order.
def against(+id: U32, +xs: List<&2, Rd>) -> List<&2, Ids>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+head, +tail}:
      Con{Ids{id, rd_id(head)}, against(id, tail)}

def cat_ids(+a: List<&2, Ids>, +b: List<&2, Ids>) -> List<&2, Ids>:
  match a:
    case Nil{}:
      b
    case Con{+head, +tail}:
      Con{head, cat_ids(tail, b)}

def id_pairs(+xs: List<&2, Rd>, +acc: List<&2, Ids>) -> List<&2, Ids>:
  match xs:
    case Nil{}:
      acc
    case Con{+head, +tail}:
      id_pairs(tail, cat_ids(acc, against(rd_id(head), tail)))

def get_rd(+id: U32, +xs: List<&2, Rd>) -> Rd:
  match xs:
    case Nil{}:
      rd0()
    case Con{+head, +tail}:
      Bool.pick(Rd, U32.is_eq(rd_id(head), id), head, get_rd(id, tail))

def set_rd(+id: U32, +v: Rd, +xs: List<&2, Rd>) -> List<&2, Rd>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+head, +tail}:
      Bool.pick(
        List<&2, Rd>,
        U32.is_eq(rd_id(head), id),
        Con{v, set_rd(id, v, tail)},
        Con{head, set_rd(id, v, tail)}
      )

def racing(+r: Rd) -> Bool:
  Bool.and(U32.is_zero(rd_crash(r)), U32.is_zero(rd_fin(r)))

# Balls that meet bounce apart and trade their pace.
def bump_pair(+a: Rd, +b: Rd) -> Duo:
  +ds = (rd_s(b) - rd_s(a) : F32)
  +dx = (rd_x(b) - rd_x(a) : F32)
  +d = hyp(ds, dx)
  +on = Bool.and(
    Bool.and(racing(a), racing(b)),
    Bool.and(
      Bool.not(F32.is_gt(F32.abs((rd_z(a) - rd_z(b) : F32)), 0.8)),
      Bool.and(Bool.not(F32.is_ge(d, (2.0 * ball_r() : F32))), Bool.not(F32.is_lt(d, 0.0001)))
    )
  )
  +ns = (ds / d : F32)
  +nx = (dx / d : F32)
  +push = (((2.0 * ball_r() : F32) - d : F32) * 0.5 : F32)
  +along = (((rd_vs(b) - rd_vs(a) : F32) * ns : F32) + ((rd_vx(b) - rd_vx(a) : F32) * nx : F32) : F32)
  +bump = Bool.and(on, F32.is_lt(along, 0.0))
  +j = (F32.neg(along) * 0.9 : F32)
  Duo{
    Bool.pick(Rd, on, mot(a, (rd_s(a) - (ns * push : F32) : F32), (rd_x(a) - (nx * push : F32) : F32), rd_z(a), Bool.pick(F32, bump, (rd_vs(a) - (j * ns : F32) : F32), rd_vs(a)), Bool.pick(F32, bump, (rd_vx(a) - (j * nx : F32) : F32), rd_vx(a)), rd_vz(a), rd_lane(a), rd_spin(a)), a),
    Bool.pick(Rd, on, mot(b, (rd_s(b) + (ns * push : F32) : F32), (rd_x(b) + (nx * push : F32) : F32), rd_z(b), Bool.pick(F32, bump, (rd_vs(b) + (j * ns : F32) : F32), rd_vs(b)), Bool.pick(F32, bump, (rd_vx(b) + (j * nx : F32) : F32), rd_vx(b)), rd_vz(b), rd_lane(b), rd_spin(b)), b),
    maybe_snd(bump, sn_clang())
  }

def apply_pairs(+ps: List<&2, Ids>, +state: List<&2, Rd>, +snd: List<&2, U32>) -> Bs:
  match ps:
    case Nil{}:
      Bs{state, snd}
    case Con{+head, +tail}:
      +p = bump_pair(get_rd(id_a(head), state), get_rd(id_b(head), state))
      +s1 = set_rd(id_a(head), pair_a(p), state)
      +s2 = set_rd(id_b(head), pair_b(p), s1)
      apply_pairs(tail, s2, snd_cat(snd, pair_snd(p)))

def bump_all(+xs: List<&2, Rd>) -> Bs:
  apply_pairs(id_pairs(xs, Nil{}), xs, Nil{})

def bs_riders(+b: Bs) -> List<&2, Rd>:
  match b:
    case Bs{+riders, _}:
      riders

def bs_snd(+b: Bs) -> List<&2, U32>:
  match b:
    case Bs{_, +snd}:
      snd

def all_home_go(+xs: List<&2, Rd>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Con{+head, +tail}:
      Bool.and(U32.is_ne(rd_fin(head), 0), all_home_go(tail))

def all_home(+xs: List<&2, Rd>) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{_, _}:
      all_home_go(xs)

# One tick of a race: the field in, the field out.
def tick(+x: Tick) -> Tk:
  match x:
    case Tick{+t, +course, +finish, +limit, +riders, +ones, +rs, +trk}:
      +done = run_riders(riders, ones, rs, trk, t, finish, course, Nil{}, Nil{})
      +bumped = bump_all(loop_riders(done))
      Tk{
        bs_riders(bumped), loop_rs(done), snd_cat(loop_snd(done), bs_snd(bumped)),
        all_home(bs_riders(bumped)),
        U32.is_ge(t, (title_ticks() + limit : U32))
      }
