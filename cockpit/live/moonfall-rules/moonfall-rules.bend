import Base
import ./model.bend as M
import ./town.bend as D

# Moonfall, the fen town's three days, as one rules module (crates/delve/src/rules/
# moonfall.rs and its folder held these rules in Rust). Rust keeps the session and
# its saves; the module takes the whole session each tick and answers the next one.
# Positions are pixels times 256, signed quantities are two's complement words,
# and every enum is a small number (the Rust adapter and index.js name them).

# --- the small helpers -----------------------------------------------------------
# Bend has no `if`: a fork is a call to `ite` with two thunks, or Bool.pick when
# both arms are cheap values.
def ite(-A: Type, c: Bool, t: Unit -> A, e: Unit -> A) -> A:
  match c:
    case True{}:
      t(Unit{})
    case False{}:
      e(Unit{})

def eq(+a: U32, +b: U32) -> Bool:
  U32.is_eq(a, b)

def ne(+a: U32, +b: U32) -> Bool:
  U32.is_ne(a, b)

def bit(+a: U32, +m: U32) -> Bool:
  U32.is_ne(U32.and(a, m), 0)

def bu(+c: Bool) -> U32:
  Bool.to_u32(c)

def sat(+a: U32, +b: U32) -> U32:
  Bool.pick(U32, U32.is_gt(a, b), (a - b : U32), 0)

def isneg(+a: U32) -> Bool:
  U32.is_ge(a, 2147483648)

def neg(+a: U32) -> U32:
  (0 - a : U32)

def sq(+a: U32) -> U32:
  (a * a : U32)

# The squared distance of two pixel points (any sign, small).
def dist2(+ax: U32, +ay: U32, +bx: U32, +by: U32) -> U32:
  (sq((ax - bx : U32)) + sq((ay - by : U32)) : U32)

# The sign of a word as -1, 0 or 1 (two's complement).
def sgn(+a: U32) -> U32:
  Bool.pick(U32, isneg(a), 4294967295, bu(U32.is_gt(a, 0)))

def sabs(+a: U32) -> U32:
  Bool.pick(U32, isneg(a), neg(a), a)

# a / b truncated toward zero, b positive.
def sdiv(+a: U32, +b: U32) -> U32:
  +q = (sabs(a) / b : U32)
  Bool.pick(U32, isneg(a), neg(q), q)

# The number of set bits.
def pop_go(n: Nat, +a: U32, +acc: U32) -> U32:
  match n:
    case 0n:
      acc
    case 1n+p:
      pop_go(p, U32.shr(a), (acc + U32.and(a, 1) : U32))

def pop(+a: U32) -> U32:
  pop_go(32n, a, 0)

# --- the cues (the Rust adapter holds their names) --------------------------------
def q_toll() -> U32:
  0

def q_roar() -> U32:
  1

def q_wall() -> U32:
  2

def q_card() -> U32:
  3

def q_door() -> U32:
  4

def q_wish() -> U32:
  5

def q_rock() -> U32:
  6

def q_pickup() -> U32:
  7

def q_empty() -> U32:
  8

def q_spoil() -> U32:
  9

def q_fall() -> U32:
  10

def q_hurt() -> U32:
  11

# --- the three-day clock (clock.rs) -------------------------------------------------
def c_end(+day: U32) -> U32:
  (day * 3 : U32)

def c_ticks(+day: U32, +h2: U32) -> U32:
  ((h2 * day + 24) / 48 : U32)

def c_half(+clock: U32, +day: U32) -> U32:
  ((clock * 48) / U32.max(day, 1) : U32)

def c_final(+day: U32) -> U32:
  c_ticks(day, 132)

def c_minutes(+clock: U32, +day: U32) -> U32:
  ((clock * 1440) / U32.max(day, 1) : U32)

def c_hour(+clock: U32, +day: U32) -> U32:
  ((((c_minutes(clock, day) + 360) / 60) % 24) : U32)

def c_night(+clock: U32, +day: U32) -> Bool:
  +h = c_hour(clock, day)
  Bool.not(Bool.and(U32.is_ge(h, 6), U32.is_lt(h, 18)))

def c_bridge_up(+clock: U32, +day: U32) -> Bool:
  +h = c_hour(clock, day)
  Bool.not(Bool.and(U32.is_ge(h, 6), U32.is_lt(h, 20)))

# The next tick at hour `phase` (mod 24, from dawn) strictly after `clock`.
def c_mark(+clock: U32, +day: U32, +phase: U32) -> U32:
  +t0 = c_ticks(day, (phase * 2 : U32))
  +t1 = c_ticks(day, ((phase + 24) * 2 : U32))
  +t2 = c_ticks(day, ((phase + 48) * 2 : U32))
  +t3 = c_ticks(day, ((phase + 72) * 2 : U32))
  +t4 = c_ticks(day, ((phase + 96) * 2 : U32))
  Bool.pick(U32, U32.is_gt(t0, clock), t0, Bool.pick(U32, U32.is_gt(t1, clock), t1, Bool.pick(U32, U32.is_gt(t2, clock), t2, Bool.pick(U32, Bool.or(U32.is_gt(t3, clock), U32.is_ge((phase + 72 : U32), 96)), t3, t4))))

def c_next_dusk(+clock: U32, +day: U32) -> U32:
  c_mark(clock, day, 12)

def c_next_dawn(+clock: U32, +day: U32) -> U32:
  c_mark(clock, day, 24)

# --- the town's ground (town.rs) ---------------------------------------------------
# The ground bits: 1 the bridge is up, 2 a swift knight, 4 the ditch is filled.
def g_open(+gb: U32, +c: U32) -> Bool:
  Bool.or(Bool.or(Bool.or(eq(c, 0), eq(c, 1)), Bool.or(eq(c, 5), eq(c, 8))), Bool.or(Bool.and(eq(c, 3), Bool.not(bit(gb, 1))), Bool.or(Bool.and(eq(c, 4), bit(gb, 2)), Bool.and(eq(c, 7), bit(gb, 4)))))

def g_cell_in(+ok: Bool, +gb: U32, +ax: U32, +ay: U32, +con: U32, +cx: U32, +cy: U32) -> Bool:
  match ok:
    case True{}:
      +tx = (ax / 16 : U32)
      +ty = (ay / 16 : U32)
      Bool.and(g_open(gb, D.cls(tx, ty)), Bool.not(Bool.and(eq(con, 1), Bool.and(eq(tx, cx), eq(ty, cy)))))
    case False{}:
      False{}

# One corner of a knight's feet: the pixel (ax, ay) may be off the map's west or north.
def g_cell(+gb: U32, +ax: U32, +ay: U32, +con: U32, +cx: U32, +cy: U32) -> Bool:
  g_cell_in(Bool.not(Bool.or(isneg(ax), isneg(ay))), gb, ax, ay, con, cx, cy)

def g_clear_in(+ok: Bool, +gb: U32, +x: U32, +y: U32, +con: U32, +cx: U32, +cy: U32) -> Bool:
  match ok:
    case True{}:
      +px = (x / 256 : U32)
      +py = (y / 256 : U32)
      Bool.and(Bool.and(g_cell(gb, (px - 4 : U32), (py - 3 : U32), con, cx, cy), g_cell(gb, (px + 3 : U32), (py - 3 : U32), con, cx, cy)), Bool.and(g_cell(gb, (px - 4 : U32), (py + 2 : U32), con, cx, cy), g_cell(gb, (px + 3 : U32), (py + 2 : U32), con, cx, cy)))
    case False{}:
      False{}

# Whether a knight's feet (an 8 by 6 box at x, y) stand clear of every solid tile
# and of the crate (`con`: there is one, at tile cx, cy).
def g_clear(+gb: U32, +x: U32, +y: U32, +con: U32, +cx: U32, +cy: U32) -> Bool:
  g_clear_in(Bool.not(Bool.or(isneg(x), isneg(y))), gb, x, y, con, cx, cy)

def spot_px_x(+i: U32) -> U32:
  (D.spot_tx(i) * 16 + 8 : U32)

def spot_px_y(+i: U32) -> U32:
  (D.spot_ty(i) * 16 + 8 : U32)

def rt_n(+r: D.Rt) -> U32:
  match r:
    case D.Rt{+n, _, _}:
      n

def rt_code(+r: D.Rt, +j: U32) -> U32:
  match r:
    case D.Rt{_, +w0, +w1}:
      U32.and(U32.shrn(Bool.pick(U32, U32.is_lt(j, 16), w0, w1), Nat.mul(U32.to_nat((j % 16 : U32)), 2n)), 3)

# --- the folk (folk.rs) --------------------------------------------------------------
# Folk: 0 Mira, 1 Tovin, 2 Nessa, 3 Orin, 4 Pella, 5 Aster. Spots, in the native order:
# 0 ArchDoor 1 ArchPorch 2 BakeryDoor 3 BakeryAt 4 ForgeAt 5 HeraldDoor 6 HeraldAt
# 7 Podium 8 WellSide 9 WellEast 10 ScriptDoor 11 ScriptDesk 12 ReedBank 13 HutDoor
# 14 BridgePost 15 EastGate 16 EastOut 17..22 Lamp1..Lamp6 23 NorthYard 24 SouthYard 25 Plaza.
# An entry is: half-hours after the first dawn, spot, behind the door, the lamp lit
# (255: none), and whether the Ledger keeps a line for it.
def f_mira() -> List<&2, M.St>:
  [M.St{0, 0, 1, 255, 1}, M.St{1, 1, 0, 255, 1}, M.St{13, 3, 0, 255, 1}, M.St{16, 1, 0, 255, 1}, M.St{22, 17, 0, 0, 1}, M.St{23, 18, 0, 1, 0}, M.St{24, 19, 0, 2, 0}, M.St{25, 20, 0, 3, 0}, M.St{26, 21, 0, 4, 0}, M.St{27, 22, 0, 5, 0}, M.St{30, 8, 0, 255, 1}, M.St{33, 0, 1, 255, 1}]

def f_pella() -> List<&2, M.St>:
  [M.St{0, 5, 1, 255, 1}, M.St{2, 6, 0, 255, 1}, M.St{12, 7, 0, 255, 1}, M.St{15, 6, 0, 255, 1}, M.St{24, 7, 0, 255, 1}, M.St{27, 6, 0, 255, 0}, M.St{29, 5, 1, 255, 1}]

def f_nessa() -> List<&2, M.St>:
  [M.St{0, 2, 1, 255, 1}, M.St{1, 3, 0, 255, 1}, M.St{12, 4, 0, 255, 1}, M.St{21, 3, 0, 255, 1}, M.St{25, 2, 1, 255, 1}]

def f_orin() -> List<&2, M.St>:
  [M.St{0, 10, 1, 255, 1}, M.St{2, 12, 0, 255, 1}, M.St{5, 11, 0, 255, 1}, M.St{23, 3, 0, 255, 1}, M.St{29, 10, 1, 255, 1}]

def f_tovin() -> List<&2, M.St>:
  [M.St{0, 13, 1, 255, 1}, M.St{1, 14, 0, 255, 1}, M.St{12, 13, 1, 255, 1}, M.St{14, 14, 0, 255, 1}, M.St{28, 13, 1, 255, 1}]

def f_day_of(+folk: U32) -> List<&2, M.St>:
  ite(List<&2, M.St>, eq(folk, 0), u => f_mira(), u => ite(List<&2, M.St>, eq(folk, 1), u => f_tovin(), u => ite(List<&2, M.St>, eq(folk, 2), u => f_nessa(), u => ite(List<&2, M.St>, eq(folk, 3), u => f_orin(), u => ite(List<&2, M.St>, eq(folk, 4), u => f_pella(), u => Nil{})))))

# A stop of day `d` (0 to 2) for the entry `i` of a villager's routine. The iron
# cart brings Nessa to the forge at dawn on the second day; on the third evening
# everyone who sleeps indoors hurries home before the Third Midnight.
def f_stop(+folk: U32, +d: U32, +i: U32, +e: M.St) -> M.Stop:
  M.Stop{(M.st_at(e) + d * 48 : U32), Bool.pick(U32, Bool.and(eq(folk, 2), Bool.and(eq(d, 1), eq(i, 1))), 4, M.st_spot(e)), M.st_inside(e), bu(Bool.and(eq(d, 2), Bool.and(eq(M.st_inside(e), 1), U32.is_ge(M.st_at(e), 26)))), Bool.pick(U32, eq(M.st_noted(e), 1), i, 255), M.st_lamp(e)}

def f_day_stops(+es: List<&2, M.St>, +folk: U32, +d: U32, +i: U32) -> List<&2, M.Stop>:
  match es:
    case Nil{}:
      Nil{}
    case Con{+e, +rest}:
      Con{f_stop(folk, d, i, e), f_day_stops(rest, folk, d, (i + 1 : U32))}

# Aster rides in on the second evening and out again: his program is his own.
def f_aster() -> List<&2, M.Stop>:
  [M.Stop{0, 16, 0, 0, 255, 255}, M.Stop{69, 15, 0, 0, 0, 255}, M.Stop{83, 16, 0, 0, 255, 255}]

def f_program(+folk: U32) -> List<&2, M.Stop>:
  +es = f_day_of(folk)
  ite(List<&2, M.Stop>, eq(folk, 5), u => f_aster(), u => List.append(&2, M.Stop, f_day_stops(es, folk, 0, 0), List.append(&2, M.Stop, f_day_stops(es, folk, 1, 0), f_day_stops(es, folk, 2, 0))))

# One leg of a day: set out so as to arrive at the stop's hour, never before the
# last leg is done. Villagers walk 1.5 pixels a tick, a hurrying one 2.4.
def f_leg(+prev: U32, +free: U32, +idx: U32, +day: U32, +s: M.Stop) -> M.Leg:
  +pace = Bool.pick(U32, eq(M.sp_fast(s), 1), 614, 384)
  +travel = ((rt_n(D.route((prev * 32 + M.sp_spot(s) : U32))) * 4096) / pace : U32)
  +at = c_ticks(day, M.sp_at(s))
  +depart = U32.max(sat(at, travel), free)
  M.Leg{idx, prev, depart, (depart + travel : U32), M.sp_spot(s), M.sp_inside(s), M.sp_routine(s), M.sp_lamp(s), at, pace}

def f_legs_go(+ss: List<&2, M.Stop>, +prev: U32, +free: U32, +idx: U32, +day: U32) -> List<&2, M.Leg>:
  match ss:
    case Nil{}:
      Nil{}
    case Con{+s, +rest}:
      +l = f_leg(prev, free, idx, day, s)
      Con{l, f_legs_go(rest, M.sp_spot(s), M.lg_arrive(l), (idx + 1 : U32), day)}

def f_legs_of(+ss: List<&2, M.Stop>, +day: U32) -> List<&2, M.Leg>:
  match ss:
    case Nil{}:
      Nil{}
    case Con{+s, +rest}:
      Con{M.Leg{0, M.sp_spot(s), 0, 0, M.sp_spot(s), M.sp_inside(s), M.sp_routine(s), M.sp_lamp(s), 0, 384}, f_legs_go(rest, M.sp_spot(s), 0, 1, day)}

def f_legs(+folk: U32, +day: U32) -> List<&2, M.Leg>:
  f_legs_of(f_program(folk), day)

def lg_head(+ls: List<&2, M.Leg>) -> M.Leg:
  match ls:
    case Nil{}:
      M.Leg{0, 0, 0, 0, 0, 0, 255, 255, 0, 384}
    case Con{+l, _}:
      l

def lg_nth(+ls: List<&2, M.Leg>, n: Nat) -> M.Leg:
  match ls:
    case Nil{}:
      M.Leg{0, 0, 0, 0, 0, 0, 255, 255, 0, 384}
    case Con{+l, +rest}:
      match n:
        case 0n:
          l
        case 1n+p:
          lg_nth(rest, p)

# The last leg that has set out by `clock` (legs set out in order).
def f_pick(+ls: List<&2, M.Leg>, +clock: U32, +best: M.Leg) -> M.Leg:
  match ls:
    case Nil{}:
      best
    case Con{+l, +rest}:
      f_pick(rest, clock, Bool.pick(M.Leg, U32.is_le(M.lg_depart(l), clock), l, best))

# --- walking a route ---------------------------------------------------------------
def f_dx(+code: U32) -> U32:
  Bool.pick(U32, eq(code, 2), 4294967295, bu(eq(code, 3)))

def f_dy(+code: U32) -> U32:
  Bool.pick(U32, eq(code, 0), 4294967295, bu(eq(code, 1)))

def f_face_of(+code: U32) -> U32:
  Bool.pick(U32, eq(code, 0), 1, Bool.pick(U32, eq(code, 1), 0, code))

def f_sum_go(n: Nat, +rt: D.Rt, +j: U32, +sx: U32, +sy: U32) -> M.P:
  match n:
    case 0n:
      M.P{sx, sy}
    case 1n+p:
      f_sum_go(p, rt, (j + 1 : U32), (sx + f_dx(rt_code(rt, j)) : U32), (sy + f_dy(rt_code(rt, j)) : U32))

def f_along_n(+empty: Bool, +rt: D.Rt, +sx: U32, +sy: U32, +travelled: U32) -> M.Al:
  match empty:
    case True{}:
      M.Al{sx, sy, 0}
    case False{}:
      +left = (travelled / 256 : U32)
      +n = rt_n(rt)
      +i = Bool.pick(U32, eq(left, 0), 0, ((left - 1 : U32) / 16 : U32))
      +beyond = U32.is_ge(i, n)
      +ie = Bool.pick(U32, beyond, (n - 1 : U32), i)
      +off = Bool.pick(U32, beyond, 16, (left - i * 16 : U32))
      +p = f_sum_go(U32.to_nat(ie), rt, 0, 0, 0)
      +code = rt_code(rt, ie)
      M.Al{(sx + M.pt_x(p) * 16 + f_dx(code) * off : U32), (sy + M.pt_y(p) * 16 + f_dy(code) * off : U32), f_face_of(code)}

# Where along `rt` (from the spot `from`) a walker is after `travelled` Q-pixels.
def f_along(+from: U32, +to: U32, +travelled: U32) -> M.Al:
  +rt = D.route((from * 32 + to : U32))
  f_along_n(eq(rt_n(rt), 0), rt, spot_px_x(from), spot_px_y(from), travelled)

def f_travel(+from: U32, +to: U32) -> U32:
  ((rt_n(D.route((from * 32 + to : U32))) * 4096) / 384 : U32)

# Aster after his horse is shod at `t`: from the gate to the well's east side,
# there until 22:00, then out of the east gate again.
def f_aster_after(+clock: U32, +day: U32, +t: U32) -> M.Fg:
  +travel = f_travel(15, 9)
  +leave = U32.max(c_ticks(day, 80), (t + travel + 1 : U32))
  +back = f_travel(9, 16)
  +a1 = f_along(15, 9, ((clock - t : U32) * 384 : U32))
  +a2 = f_along(9, 16, ((clock - leave : U32) * 384 : U32))
  +s1 = U32.is_lt(clock, (t + travel : U32))
  +s2 = U32.is_lt(clock, leave)
  +s3 = U32.is_lt(clock, (leave + back : U32))
  M.Fg{Bool.pick(U32, s1, M.al_x(a1), Bool.pick(U32, s2, spot_px_x(9), Bool.pick(U32, s3, M.al_x(a2), spot_px_x(16)))), Bool.pick(U32, s1, M.al_y(a1), Bool.pick(U32, s2, spot_px_y(9), Bool.pick(U32, s3, M.al_y(a2), spot_px_y(16)))), bu(s3), bu(Bool.or(s1, Bool.and(Bool.not(s2), s3))), Bool.pick(U32, s1, M.al_face(a1), Bool.pick(U32, s2, 2, Bool.pick(U32, s3, M.al_face(a2), 3))), 0, bu(Bool.and(Bool.not(s1), s2)), 255}

def f_figure_legs(+folk: U32, +clock: U32, +legs: List<&2, M.Leg>) -> M.Fg:
  +best = f_pick(legs, clock, lg_head(legs))
  +moving = U32.is_lt(clock, M.lg_arrive(best))
  +al = f_along(M.lg_from(best), M.lg_spot(best), ((clock - M.lg_depart(best) : U32) * M.lg_pace(best) : U32))
  +stage = Bool.or(Bool.not(eq(folk, 5)), Bool.and(U32.is_le(M.lg_depart(lg_nth(legs, 1n)), clock), U32.is_lt(clock, M.lg_arrive(lg_nth(legs, 2n)))))
  M.Fg{Bool.pick(U32, moving, M.al_x(al), spot_px_x(M.lg_spot(best))), Bool.pick(U32, moving, M.al_y(al), spot_px_y(M.lg_spot(best))), bu(Bool.and(stage, Bool.or(moving, eq(M.lg_inside(best), 0)))), bu(moving), Bool.pick(U32, moving, M.al_face(al), (bu(eq(M.lg_spot(best), 15)) * 3 : U32)), M.lg_idx(best), bu(Bool.and(Bool.not(moving), stage)), Bool.pick(U32, moving, 255, M.lg_routine(best))}

# Where `folk` is at `clock`. Aster, once shod at `shod`, takes his own way.
def f_figure(+folk: U32, +clock: U32, +day: U32, +shod_on: U32, +shod: U32) -> M.Fg:
  ite(M.Fg, Bool.and(eq(folk, 5), Bool.and(eq(shod_on, 1), U32.is_ge(clock, shod))), u => f_aster_after(clock, day, shod), u => f_figure_legs(folk, clock, f_legs(folk, day)))

# The tick Aster stands at the well once he has been shod at `shod`.
def f_aster_at_well(+shod: U32) -> U32:
  (shod + f_travel(15, 9) : U32)

# The tick the lamplighter reaches lantern `n`, in the day's own ticks.
def f_lamp_go(+ls: List<&2, M.Leg>, +n: U32, +day: U32) -> U32:
  match ls:
    case Nil{}:
      0
    case Con{+l, +rest}:
      Bool.pick(U32, eq(M.lg_lamp(l), n), (M.lg_arrive(l) % U32.max(day, 1) : U32), f_lamp_go(rest, n, day))

def f_lamp_at(+n: U32, +day: U32) -> U32:
  f_lamp_go(f_legs(0, day), n, day)

# --- lists of words and of knights --------------------------------------------------
def lu_get(+l: List<&2, U32>, +i: U32) -> U32:
  match l:
    case Nil{}:
      0
    case Con{+h, +t}:
      Bool.pick(U32, eq(i, 0), h, lu_get(t, (i - 1 : U32)))

def lu_set(+l: List<&2, U32>, +i: U32, +v: U32) -> List<&2, U32>:
  match l:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Con{Bool.pick(U32, eq(i, 0), v, h), lu_set(t, (i - 1 : U32), v)}

def lu_push(+l: List<&2, U32>, +v: U32) -> List<&2, U32>:
  List.append(&2, U32, l, Con{v, Nil{}})

def lu_sum(+l: List<&2, U32>, +acc: U32) -> U32:
  match l:
    case Nil{}:
      acc
    case Con{+h, +t}:
      lu_sum(t, (acc + h : U32))

# Where the knights stand at each dawn, around the Moonwell: the tile for seat `id`.
def sp_tx(+id: U32) -> U32:
  +i = ((id + 5) % 6 : U32)
  Bool.pick(U32, eq(i, 0), 16, Bool.pick(U32, eq(i, 1), 19, Bool.pick(U32, eq(i, 2), 15, Bool.pick(U32, eq(i, 3), 20, Bool.pick(U32, eq(i, 4), 16, 19)))))

def sp_ty(+id: U32) -> U32:
  (15 + ((((id + 5) % 6) / 2) * 2) : U32)

# A knight on their feet at the dawn spawn, wearing `mask`.
def kn_new(+id: U32, +joined: U32, +mask: U32) -> M.Kn:
  M.k_set_candles(M.k_set_mask(M.k_set_y(M.k_set_x(M.k_set_joined(M.k_set_id(M.Kn{M.v_zero()}, id), joined), ((sp_tx(id) * 16 + 8) * 256 : U32)), ((sp_ty(id) * 16 + 8) * 256 : U32)), mask), 3)

def sl_get(+l: List<&2, M.Kn>, +id: U32) -> M.Kn:
  match l:
    case Nil{}:
      kn_new(0, 0, 0)
    case Con{+k, +t}:
      Bool.pick(M.Kn, eq(M.k_id(k), id), k, sl_get(t, id))

def sl_has(+l: List<&2, M.Kn>, +id: U32) -> Bool:
  match l:
    case Nil{}:
      False{}
    case Con{+k, +t}:
      Bool.or(eq(M.k_id(k), id), sl_has(t, id))

def sl_put(+l: List<&2, M.Kn>, +k: M.Kn) -> List<&2, M.Kn>:
  match l:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Con{Bool.pick(M.Kn, eq(M.k_id(h), M.k_id(k)), k, h), sl_put(t, k)}

def sl_ins(+l: List<&2, M.Kn>, +k: M.Kn) -> List<&2, M.Kn>:
  match l:
    case Nil{}:
      Con{k, Nil{}}
    case Con{+h, +t}:
      Bool.pick(List<&2, M.Kn>, U32.is_lt(M.k_id(k), M.k_id(h)), Con{k, Con{h, t}}, Bool.pick(List<&2, M.Kn>, eq(M.k_id(k), M.k_id(h)), Con{k, t}, Con{h, sl_ins(t, k)}))

def sl_del(+l: List<&2, M.Kn>, +id: U32) -> List<&2, M.Kn>:
  match l:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(List<&2, M.Kn>, eq(M.k_id(h), id), t, Con{h, sl_del(t, id)})

def sl_len(+l: List<&2, M.Kn>) -> U32:
  match l:
    case Nil{}:
      0
    case Con{_, +t}:
      (sl_len(t) + 1 : U32)

# --- the session's small verbs --------------------------------------------------------
def get_k(+s: M.Sess, +id: U32) -> M.Kn:
  sl_get(M.s_seats(s), id)

def put_k(+s: M.Sess, +k: M.Kn) -> M.Sess:
  M.s_set_seats(s, sl_put(M.s_seats(s), k))

def edit(+s: M.Sess, +id: U32, f: M.Kn -> M.Kn) -> M.Sess:
  put_k(s, f(get_k(s, id)))

def cue(+s: M.Sess, +c: U32) -> M.Sess:
  M.s_set_cues(s, lu_push(M.s_cues(s), c))

# Sounds the Ledger and the town owe, handed out after the tick's own.
def defer(+s: M.Sess, +c: U32) -> M.Sess:
  M.s_set_pending(s, lu_push(M.s_pending(s), c))

def nw_len(+l: List<&2, M.Nw>) -> U32:
  match l:
    case Nil{}:
      0
    case Con{_, +t}:
      (nw_len(t) + 1 : U32)

def nw_tail(+l: List<&2, M.Nw>) -> List<&2, M.Nw>:
  match l:
    case Nil{}:
      Nil{}
    case Con{_, +t}:
      t

# Notes: 0 Wrote(folk, routine), 1 Learned(fact), 2 Ticked(folk), 3 Remind(folk, id),
# 4 Sign(sign), 5 Got(item), 6 Mask(mask), 7 Candle(n), 8 Snuffed, 9 Lit(i), 10 Out, 11 Ring(id).
def announce_cue(+s: M.Sess, +kind: U32) -> M.Sess:
  ite(M.Sess, U32.is_le(kind, 1), u => defer(s, q_pickup()), u => ite(M.Sess, eq(kind, 2), u => defer(s, q_wish()), u => ite(M.Sess, eq(kind, 3), u => defer(s, q_card()), u => s)))

# Everyone's: the Ledger wrote, or an hour is near.
def announce(+s: M.Sess, +kind: U32, +a: U32, +b: U32) -> M.Sess:
  +t = announce_cue(s, kind)
  +l = List.append(&2, M.Nw, M.s_news(t), Con{M.Nw{kind, a, b, 120}, Nil{}})
  M.s_set_news(t, Bool.pick(List<&2, M.Nw>, U32.is_gt(nw_len(l), 3), nw_tail(l), l))

def kn_note(+k: M.Kn, +kind: U32, +a: U32, +b: U32, +left: U32) -> M.Kn:
  M.k_set_note_left(M.k_set_note_b(M.k_set_note_a(M.k_set_note_kind(M.k_set_note_on(k, 1), kind), a), b), left)

# A short word for one knight.
def note(+s: M.Sess, +who: U32, +kind: U32, +a: U32, +b: U32) -> M.Sess:
  edit(s, who, k => kn_note(k, kind, a, b, 90))

def sign(+s: M.Sess, +who: U32, +which: U32) -> M.Sess:
  note(s, who, 4, which, 0)

def pw(+n: U32) -> U32:
  U32.shln(1, U32.to_nat(n))

# --- the Ledger (ledger.rs) -----------------------------------------------------------
# Facts: 0 OrinLost 1 NessaCold 2 PellaScroll 3 TovinPass 4 MiraSeal 5 AsterComing 6 AsterLame 7 Verse.
# Masks: 1 swift, 2 stout. Keeps: 1 north key, 2 south seal, 4 oil.
def m_knows(+s: M.Sess, +f: U32) -> Bool:
  bit(M.s_facts(s), pw(f))

def m_witnessed(+s: M.Sess, +folk: U32, +r: U32) -> Bool:
  Bool.and(Bool.and(ne(r, 255), U32.is_lt(r, 32)), bit(lu_get(M.s_rt(s), folk), pw(r)))

def m_witness(+s: M.Sess, +folk: U32, +r: U32) -> M.Sess:
  M.s_set_rt(s, lu_set(M.s_rt(s), folk, U32.or(lu_get(M.s_rt(s), folk), pw(r))))

def m_ticked(+s: M.Sess, +folk: U32) -> Bool:
  bit(M.s_ticks(s), pw(folk))

def m_has_mask(+s: M.Sess, +m: U32) -> Bool:
  bit(M.s_masks(s), m)

def m_has_keep(+s: M.Sess, +k: U32) -> Bool:
  bit(M.s_keeps(s), k)

def rt_pop(+l: List<&2, U32>, +acc: U32) -> U32:
  match l:
    case Nil{}:
      acc
    case Con{+h, +t}:
      rt_pop(t, (acc + pop(h) : U32))

# How much the Ledger holds: a count that only grows.
def m_written(+s: M.Sess) -> U32:
  (pop(M.s_facts(s)) + rt_pop(M.s_rt(s), 0) + pop(M.s_ticks(s)) + pop(M.s_masks(s)) + pop(M.s_keeps(s)) : U32)

# A fact learned, written once, told to the table.
def learn(+s: M.Sess, +f: U32) -> M.Sess:
  ite(M.Sess, m_knows(s, f), u => s, u => announce(M.s_set_facts(s, U32.or(M.s_facts(s), pw(f))), 1, f, 0))

# A favour ticked: the first time it is, the table is told.
def tick_folk(+s: M.Sess, +folk: U32) -> M.Sess:
  ite(M.Sess, m_ticked(s, folk), u => s, u => announce(M.s_set_ticks(s, U32.or(M.s_ticks(s), pw(folk))), 2, folk, 0))

# The appointments the Ledger reminds the table of, an hour ahead (folk, routine, only day or 255).
def rem_go(+ss: List<&2, M.Stop>, +s: M.Sess, +who: U32, +id: U32, +only: U32, +clock: U32, +day: U32) -> M.Sess:
  match ss:
    case Nil{}:
      s
    case Con{+st, +rest}:
      +at = c_ticks(day, M.sp_at(st))
      +hit = Bool.and(Bool.and(eq(M.sp_routine(st), id), Bool.or(eq(only, 255), eq((at / U32.max(day, 1) : U32), only))), eq(sat(at, (day / 24 : U32)), clock))
      rem_go(rest, ite(M.Sess, hit, u => announce(s, 3, who, id), u => s), who, id, only, clock, day)

def rem_one(+s: M.Sess, +who: U32, +id: U32, +only: U32, +clock: U32, +day: U32) -> M.Sess:
  ite(M.Sess, Bool.or(m_witnessed(s, who, id), Bool.and(eq(who, 5), m_knows(s, 5))), u => rem_go(f_program(who), s, who, id, only, clock, day), u => s)

def reminders(+s: M.Sess, +clock: U32, +day: U32) -> M.Sess:
  +a = rem_one(s, 4, 2, 255, clock, day)
  +b = rem_one(a, 4, 4, 255, clock, day)
  +c = rem_one(b, 0, 4, 255, clock, day)
  +d = rem_one(c, 0, 10, 255, clock, day)
  +e = rem_one(d, 1, 4, 255, clock, day)
  rem_one(e, 5, 0, 1, clock, day)

# --- a knight on foot (walk.rs) ---------------------------------------------------------
# The key bits a tick's input is read as.
def key_use() -> U32:
  1

def key_ledger() -> U32:
  2

def key_bell() -> U32:
  4

def key_mask() -> U32:
  8

def key_drop() -> U32:
  16

def key_side() -> U32:
  32

def key_up() -> U32:
  64

def key_down() -> U32:
  128

def key_left() -> U32:
  256

def key_right() -> U32:
  512

def positive(+a: U32) -> Bool:
  Bool.and(U32.is_gt(a, 0), Bool.not(isneg(a)))

def eq_bit(+flag: U32, +m: U32) -> U32:
  (bu(eq(flag, 1)) * m : U32)

def keys_of(+i: M.In) -> U32:
  +up = Bool.or(isneg(M.i_my(i)), isneg(M.i_ay(i)))
  +down = Bool.or(positive(M.i_my(i)), positive(M.i_ay(i)))
  +left = Bool.or(isneg(M.i_mx(i)), isneg(M.i_ax(i)))
  +right = Bool.or(positive(M.i_mx(i)), positive(M.i_ax(i)))
  (bu(Bool.or(eq(M.i_interact(i), 1), eq(M.i_take(i), 1))) * 1 + eq_bit(M.i_swing(i), 2) + eq_bit(M.i_ult(i), 4) + eq_bit(M.i_play(i), 8) + eq_bit(M.i_bomb(i), 16) + eq_bit(M.i_dash(i), 32) + bu(up) * 64 + bu(down) * 128 + bu(left) * 256 + bu(right) * 512 : U32)

def pressed(+now: U32, +before: U32, +b: U32) -> Bool:
  Bool.and(bit(now, b), Bool.not(bit(before, b)))

# Q-pixels a tick a knight walks at: the folk's pace times the number, by mask.
def speed(+walk: U32, +mask: U32) -> U32:
  +base = ((384 * walk) / 100 : U32)
  Bool.pick(U32, eq(mask, 0), base, Bool.pick(U32, eq(mask, 1), ((base * 16) / 10 : U32), ((base * 7) / 10 : U32)))

# A facing as a step (two's complement).
def facing_x(+face: U32) -> U32:
  Bool.pick(U32, eq(face, 2), 4294967295, bu(eq(face, 3)))

def facing_y(+face: U32) -> U32:
  Bool.pick(U32, eq(face, 1), 4294967295, bu(eq(face, 0)))

def crate_corner(+cx: U32, +cy: U32, +ax: U32, +ay: U32) -> Bool:
  Bool.and(Bool.and(Bool.not(isneg(ax)), Bool.not(isneg(ay))), Bool.and(eq((ax / 16 : U32), cx), eq((ay / 16 : U32), cy)))

# Whether the knight's feet at (nx, ny) overlap the crate's tile.
def crate_hit(+con: U32, +cx: U32, +cy: U32, +nx: U32, +ny: U32) -> Bool:
  +px = (nx / 256 : U32)
  +py = (ny / 256 : U32)
  Bool.and(eq(con, 1), Bool.or(Bool.or(crate_corner(cx, cy, (px - 4 : U32), (py - 3 : U32)), crate_corner(cx, cy, (px + 3 : U32), (py - 3 : U32))), Bool.or(crate_corner(cx, cy, (px - 4 : U32), (py + 2 : U32)), crate_corner(cx, cy, (px + 3 : U32), (py + 2 : U32)))))

# A stout knight leans on the crate; ten ticks on, it moves a tile or falls into the ditch.
def lean_done(+w: M.Wk, +k: M.Kn, +ax: U32, +ay: U32, +gb: U32) -> M.Wk:
  +tx = (M.wk_cx(w) + sgn(ax) : U32)
  +ty = (M.wk_cy(w) + sgn(ay) : U32)
  +c = D.cls(tx, ty)
  +ditch = Bool.and(eq(c, 7), Bool.not(bit(gb, 4)))
  +open = g_open(gb, c)
  ite(M.Wk, ditch, u => M.wk_set_filled(M.wk_set_con(M.wk_set_k(w, M.k_set_push(k, 0)), 0), 1), u => ite(M.Wk, open, u => M.wk_set_moved(M.wk_set_cy(M.wk_set_cx(M.wk_set_k(w, M.k_set_push(k, 0)), tx), ty), 1), u => M.wk_set_k(w, M.k_set_push(k, 0))))

def lean(+w: M.Wk, +ax: U32, +ay: U32, +gb: U32) -> M.Wk:
  +k = M.k_set_push(M.wk_k(w), (M.k_push(M.wk_k(w)) + 1 : U32))
  +w1 = M.wk_set_leaning(M.wk_set_k(w, k), 1)
  ite(M.Wk, U32.is_ge(M.k_push(k), 10), u => lean_done(w1, k, ax, ay, gb), u => w1)

# One axis of a step: blocked ground stops it, a crate is leant on or too heavy.
def walk_axis_go(+w: M.Wk, +ax: U32, +ay: U32, +each: U32, +gb: U32) -> M.Wk:
  +k = M.wk_k(w)
  +nx = (M.k_x(k) + ax * each : U32)
  +ny = (M.k_y(k) + ay * each : U32)
  ite(M.Wk, Bool.not(g_clear(gb, nx, ny, 0, 0, 0)), u => w, u => ite(M.Wk, crate_hit(M.wk_con(w), M.wk_cx(w), M.wk_cy(w), nx, ny), u => ite(M.Wk, eq(M.k_mask(k), 2), u => lean(w, ax, ay, gb), u => M.wk_set_heavy(w, 1)), u => M.wk_set_k(w, M.k_set_y(M.k_set_x(k, nx), ny))))

def walk_axis(+w: M.Wk, +ax: U32, +ay: U32, +each: U32, +gb: U32) -> M.Wk:
  ite(M.Wk, Bool.and(eq(ax, 0), eq(ay, 0)), u => w, u => walk_axis_go(w, ax, ay, each, gb))

# The face after a step: sideways wins while facing north or south, else up and down.
def face_after(+face: U32, +dx: U32, +dy: U32) -> U32:
  Bool.pick(U32, Bool.and(ne(dx, 0), Bool.or(eq(dy, 0), U32.is_lt(face, 2))), Bool.pick(U32, isneg(dx), 2, 3), Bool.pick(U32, ne(dy, 0), Bool.pick(U32, isneg(dy), 1, 0), face))

def walk_moving(+k: M.Kn, +dx: U32, +dy: U32, +rolling: Bool, +pace: U32, +gb: U32, +con: U32, +cx: U32, +cy: U32) -> M.Wk:
  +speed1 = Bool.pick(U32, rolling, ((pace * 22) / 10 : U32), pace)
  +each = Bool.pick(U32, Bool.and(ne(dx, 0), ne(dy, 0)), ((speed1 * 181) / 256 : U32), speed1)
  +k1 = M.k_set_stride(k, U32.min((M.k_stride(k) + 1 : U32), 60000))
  +w0 = {M.Wk{k1, con, cx, cy, 0, 0, 0, 0} : M.Wk}
  +w1 = walk_axis(w0, dx, 0, each, gb)
  +w2 = walk_axis(w1, 0, dy, each, gb)
  +k2 = M.wk_k(w2)
  M.wk_set_k(w2, M.k_set_face(M.k_set_push(k2, Bool.pick(U32, eq(M.wk_leaning(w2), 1), M.k_push(k2), 0)), face_after(M.k_face(k2), dx, dy)))

# One tick of walking for a knight who is free to move: the sidestep, then up to
# two axes of stride. `rolled` is the sidestep key's press.
def walk_step(+k: M.Kn, +mx: U32, +my: U32, +pace: U32, +gb: U32, +rolled: Bool, +con: U32, +cx: U32, +cy: U32) -> M.Wk:
  +rolling0 = U32.is_gt(M.k_roll(k), 0)
  +begin = Bool.and(Bool.and(rolled, eq(M.k_roll_rest(k), 0)), Bool.and(eq(M.k_down(k), 0), Bool.not(rolling0)))
  +still = Bool.and(eq(mx, 0), eq(my, 0))
  +dirx = Bool.pick(U32, still, facing_x(M.k_face(k)), mx)
  +diry = Bool.pick(U32, still, facing_y(M.k_face(k)), my)
  +dx = Bool.pick(U32, rolling0, M.k_roll_dx(k), Bool.pick(U32, begin, dirx, mx))
  +dy = Bool.pick(U32, rolling0, M.k_roll_dy(k), Bool.pick(U32, begin, diry, my))
  +k1 = M.k_set_roll_dy(M.k_set_roll_dx(M.k_set_roll_rest(M.k_set_roll(k, Bool.pick(U32, begin, 6, M.k_roll(k))), Bool.pick(U32, begin, 40, M.k_roll_rest(k))), Bool.pick(U32, begin, dirx, M.k_roll_dx(k))), Bool.pick(U32, begin, diry, M.k_roll_dy(k)))
  +rolling = U32.is_gt(M.k_roll(k1), 0)
  +k2 = M.k_set_roll(M.k_set_roll_rest(k1, sat(M.k_roll_rest(k1), 1)), sat(M.k_roll(k1), 1))
  ite(M.Wk, Bool.and(eq(dx, 0), eq(dy, 0)), u => M.Wk{M.k_set_push(M.k_set_stride(k2, 0), 0), con, cx, cy, 0, 0, 0, 0}, u => walk_moving(k2, dx, dy, rolling, pace, gb, con, cx, cy))

# --- the favours (story.rs) ----------------------------------------------------------
# Flags of the loop: 1 north open, 2 south open, 4 ink taken, 8 ditch filled.
def final_hours(+s: M.Sess) -> Bool:
  U32.is_ge(M.s_clock(s), c_final(M.s_n_day(s)))

def day_index(+s: M.Sess) -> U32:
  U32.min((M.s_clock(s) / U32.max(M.s_n_day(s), 1) : U32), 2)

def peak1(+s: M.Sess) -> U32:
  U32.max(M.s_peak(s), 1)

# Ticks a flame burns once lit: tighter with every extra seat.
def burn(+s: M.Sess) -> U32:
  +hour = (M.s_n_day(s) / 24 : U32)
  U32.max(sat(((hour * 9) / 2 : U32), ((hour * (peak1(s) - 1 : U32)) / 4 : U32)), ((hour * 5) / 2 : U32))

def lit_get(+s: M.Sess, +i: U32) -> U32:
  Bool.pick(U32, eq(i, 0), M.s_lit0(s), Bool.pick(U32, eq(i, 1), M.s_lit1(s), M.s_lit2(s)))

def lit_set(+s: M.Sess, +i: U32, +v: U32) -> M.Sess:
  ite(M.Sess, eq(i, 0), u => M.s_set_lit0(s, v), u => ite(M.Sess, eq(i, 1), u => M.s_set_lit1(s, v), u => M.s_set_lit2(s, v)))

# A flame of the three towers: 0 north, 1 south, 2 the well.
def flame(+s: M.Sess, +i: U32) -> Bool:
  U32.is_gt(lit_get(s, i), M.s_clock(s))

def has_flag(+s: M.Sess, +f: U32) -> Bool:
  bit(M.s_flags(s), f)

def set_flag(+s: M.Sess, +f: U32) -> M.Sess:
  M.s_set_flags(s, U32.or(M.s_flags(s), f))

def clear_flag(+s: M.Sess, +f: U32) -> M.Sess:
  M.s_set_flags(s, U32.and(M.s_flags(s), U32.xor(f, 15)))

# The ground bits for a knight with or without the swift mask.
def ground(+s: M.Sess, +swift: Bool) -> U32:
  (bu(c_bridge_up(M.s_clock(s), M.s_n_day(s))) + bu(swift) * 2 + bu(has_flag(s, 8)) * 4 : U32)

def tower_ready(+s: M.Sess, +i: U32) -> Bool:
  Bool.and(Bool.and(m_knows(s, 7), final_hours(s)), Bool.pick(Bool, eq(i, 0), has_flag(s, 1), Bool.pick(Bool, eq(i, 1), has_flag(s, 2), m_has_keep(s, 4))))

# Whether it is day `d` (255: any) and within the half-hours [lo, hi) of its dawn.
def within(+s: M.Sess, +d: U32, +lo: U32, +hi: U32) -> Bool:
  +h = c_half(M.s_clock(s), M.s_n_day(s))
  Bool.and(Bool.or(eq(d, 255), eq((h / 48 : U32), d)), Bool.and(U32.is_ge((h % 48 : U32), lo), U32.is_lt((h % 48 : U32), hi)))

def figure_of(+s: M.Sess, +folk: U32) -> M.Fg:
  f_figure(folk, M.s_clock(s), M.s_n_day(s), M.s_shod_on(s), M.s_shod(s))

def lamp_tx(+n: U32) -> U32:
  Bool.pick(U32, eq(n, 0), 9, Bool.pick(U32, eq(n, 1), 13, Bool.pick(U32, eq(n, 2), 16, Bool.pick(U32, eq(n, 3), 20, Bool.pick(U32, eq(n, 4), 24, 28)))))

# The lantern lights by the lamplighter's round (`ls`: the lamplighter's legs), and stays lit
# unless the Gloam or the moon's shadow put it out.
def lantern_scheduled_l(+s: M.Sess, +ls: List<&2, M.Leg>, +n: U32) -> Bool:
  U32.is_ge((M.s_clock(s) % U32.max(M.s_n_day(s), 1) : U32), f_lamp_go(ls, n, M.s_n_day(s)))

def lantern_scheduled(+s: M.Sess, +n: U32) -> Bool:
  lantern_scheduled_l(s, f_legs(0, M.s_n_day(s)), n)

def lantern_lit_l(+s: M.Sess, +ls: List<&2, M.Leg>, +n: U32) -> Bool:
  Bool.and(Bool.and(lantern_scheduled_l(s, ls, n), Bool.not(bit(M.s_snuffed(s), pw(n)))), eq(M.s_done_on(s), 0))

# --- what E may mean where a knight stands -------------------------------------------
# 0 nothing, 1 to 6 a villager, 7 the ink pot, 8 the reeds, 9 the north door, 10 the
# south door, 11 the well, 12 the bell post, 13 to 18 a lantern.
def offer(+b: M.Bs, +d2: U32, +reach: U32, +u: U32) -> M.Bs:
  Bool.pick(M.Bs, Bool.and(U32.is_le(d2, (reach * reach : U32)), Bool.or(eq(M.bs_u(b), 0), U32.is_lt(d2, M.bs_d(b)))), {M.Bs{d2, u} : M.Bs}, b)

def offer_folk(+s: M.Sess, +b: M.Bs, +mx: U32, +my: U32, +f: U32) -> M.Bs:
  +fig = figure_of(s, f)
  Bool.pick(M.Bs, eq(M.fg_visible(fig), 1), offer(b, dist2(mx, my, M.fg_x(fig), M.fg_y(fig)), 26, (f + 1 : U32)), b)

def offer_lamp(+b: M.Bs, +mx: U32, +my: U32, +n: U32) -> M.Bs:
  offer(b, dist2(mx, my, (lamp_tx(n) * 16 + 8 : U32), 276), 18, (13 + n : U32))

def nearest_go(+s: M.Sess, +k: M.Kn) -> U32:
  +mx = (M.k_x(k) / 256 : U32)
  +my = (M.k_y(k) / 256 : U32)
  +b0 = {M.Bs{0, 0} : M.Bs}
  +b1 = offer_folk(s, b0, mx, my, 0)
  +b2 = offer_folk(s, b1, mx, my, 1)
  +b3 = offer_folk(s, b2, mx, my, 2)
  +b4 = offer_folk(s, b3, mx, my, 3)
  +b5 = offer_folk(s, b4, mx, my, 4)
  +b6 = offer_folk(s, b5, mx, my, 5)
  +b7 = offer(b6, dist2(mx, my, 184, 56), 22, 7)
  +b8 = offer(b7, dist2(mx, my, 216, 88), 22, 8)
  +b9 = offer(b8, dist2(mx, my, 296, 72), 26, 9)
  +b10 = offer(b9, dist2(mx, my, 280, 392), 26, 10)
  +b11 = offer(b10, dist2(mx, my, 288, 208), 38, 11)
  +b12 = offer(b11, dist2(mx, my, 216, 204), 22, 12)
  +b13 = offer_lamp(b12, mx, my, 0)
  +b14 = offer_lamp(b13, mx, my, 1)
  +b15 = offer_lamp(b14, mx, my, 2)
  +b16 = offer_lamp(b15, mx, my, 3)
  +b17 = offer_lamp(b16, mx, my, 4)
  M.bs_u(offer_lamp(b17, mx, my, 5))

def nearest_use(+s: M.Sess, +who: U32) -> U32:
  ite(U32, Bool.not(sl_has(M.s_seats(s), who)), u => 0, u => nearest_go(s, get_k(s, who)))

def held_of(+s: M.Sess, +who: U32) -> U32:
  M.k_held(get_k(s, who))

# What is taken is in hand, from this tick of the loop.
def take(+s: M.Sess, +who: U32, +item: U32) -> M.Sess:
  note(edit(s, who, k => M.k_set_held_at(M.k_set_held(k, item), M.s_clock(s))), who, 5, item, 0)

def give_away(+s: M.Sess, +who: U32) -> M.Sess:
  edit(s, who, k => M.k_set_held(k, 0))

# A mask is earned and kept, and the person's tick goes in the Ledger.
def keep_mask(+s: M.Sess, +who: U32, +bit_: U32, +mask: U32, +ticked: U32) -> M.Sess:
  note(tick_folk(M.s_set_masks(s, U32.or(M.s_masks(s), bit_)), ticked), who, 6, mask, 0)

def use_ink(+s: M.Sess, +who: U32) -> M.Sess:
  ite(M.Sess, ne(held_of(s, who), 0), u => sign(s, who, 3), u => ite(M.Sess, Bool.or(Bool.not(m_knows(s, 0)), has_flag(s, 4)), u => sign(s, who, 0), u => take(set_flag(s, 4), who, 1)))

def use_reeds(+s: M.Sess, +who: U32) -> M.Sess:
  ite(M.Sess, ne(held_of(s, who), 0), u => sign(s, who, 3), u => ite(M.Sess, Bool.not(m_knows(s, 1)), u => sign(s, who, 0), u => take(s, who, 2)))

# The tower doors open to the key and the seal the party keeps.
def door(+s: M.Sess, +who: U32, +north: Bool) -> M.Sess:
  +flag = Bool.pick(U32, north, 1, 2)
  ite(M.Sess, has_flag(s, flag), u => ite(M.Sess, final_hours(s), u => ite(M.Sess, Bool.not(m_knows(s, 7)), u => sign(s, who, 12), u => s), u => sign(s, who, 11)), u => ite(M.Sess, m_has_keep(s, flag), u => sign(set_flag(s, flag), who, Bool.pick(U32, north, 5, 7)), u => sign(s, who, Bool.pick(U32, north, 4, 6))))

def use_well(+s: M.Sess, +who: U32) -> M.Sess:
  sign(s, who, Bool.pick(U32, Bool.not(m_has_keep(s, 4)), 9, Bool.pick(U32, Bool.and(final_hours(s), Bool.not(m_knows(s, 7))), 12, 10)))

# E at the post is the bell's own menu, as R is.
def use_bell(+s: M.Sess, +who: U32) -> M.Sess:
  ite(M.Sess, Bool.and(eq(M.s_done_on(s), 0), eq(M.s_call_on(s), 0)), u => edit(s, who, k => M.k_set_say_on(M.k_set_parg(M.k_set_panel(k, 2), 0), 0)), u => s)

def relight(+s: M.Sess, +who: U32, +n: U32) -> M.Sess:
  ite(M.Sess, Bool.and(lantern_scheduled(s, n), bit(M.s_snuffed(s), pw(n))), u => sign(defer(M.s_set_snuffed(s, U32.and(M.s_snuffed(s), U32.xor(pw(n), 63))), q_spoil()), who, 15), u => s)

# E held: at a tower that is ready, light it when the hold is full.
def hold0(+s: M.Sess, +who: U32) -> M.Sess:
  edit(s, who, k => M.k_set_hold(k, 0))

def lit_tower(+s: M.Sess, +who: U32, +i: U32) -> M.Sess:
  +s1 = lit_set(s, i, (M.s_clock(s) + burn(s) : U32))
  cue(cue(announce(s1, 9, i, 0), q_wish()), q_toll())

def hold_full(+s: M.Sess, +who: U32, +i: U32) -> M.Sess:
  +s1 = hold0(s, who)
  ite(M.Sess, flame(s, i), u => s1, u => lit_tower(s1, who, i))

def hold_tower(+s: M.Sess, +who: U32, +i: U32) -> M.Sess:
  +h = (M.k_hold(get_k(s, who)) + 1 : U32)
  +s1 = edit(s, who, k => M.k_set_hold(k, h))
  ite(M.Sess, U32.is_ge(h, 24), u => hold_full(s1, who, i), u => s1)

def hold_use(+s: M.Sess, +who: U32) -> M.Sess:
  +u = nearest_use(s, who)
  +i = Bool.pick(U32, eq(u, 9), 0, Bool.pick(U32, eq(u, 10), 1, Bool.pick(U32, eq(u, 11), 2, 255)))
  ite(M.Sess, Bool.or(eq(i, 255), Bool.not(tower_ready(s, i))), u2 => hold0(s, who), u2 => hold_tower(s, who, i))

# --- a conversation: choose what is said, and do what it means ------------------------
# Topics: 0 Hello(band), 1 Request, 2 Give, 3 Thanks, 4 Done, 5 Nag(n), 6 Cry(n).
def says(+s: M.Sess, +tk: U32, +ta: U32) -> M.Ts:
  M.Ts{s, tk, ta}

# The part of the day by the clock, for small talk.
def band(+hour: U32) -> U32:
  Bool.pick(U32, Bool.and(U32.is_ge(hour, 6), U32.is_le(hour, 11)), 0, Bool.pick(U32, Bool.and(U32.is_ge(hour, 12), U32.is_le(hour, 16)), 1, Bool.pick(U32, Bool.and(U32.is_ge(hour, 17), U32.is_le(hour, 21)), 2, 3)))

def cry_active(+s: M.Sess, +who: U32) -> Bool:
  Bool.and(eq(M.s_cry_on(s), 1), eq(M.s_cry_folk(s), who))

def cry_number(+s: M.Sess) -> U32:
  Bool.pick(U32, eq(M.s_cry_tk(s), 6), M.s_cry_ta(s), 0)

# Whether the reeds in a knight's hand are dry: twelve hours in the hand.
def dry(+s: M.Sess, +k: M.Kn) -> Bool:
  Bool.and(eq(M.k_held(k), 2), U32.is_ge(M.s_clock(s), (M.k_held_at(k) + 12 * M.s_n_day(s) / 24 : U32)))

# The second day's iron cart: Nessa's rack of shoes is out.
def shoe_day(+s: M.Sess) -> Bool:
  Bool.and(within(s, 1, 1, 4), eq(M.fg_arrived(figure_of(s, 2)), 1))

def hands_full(+s: M.Sess, +who: U32, +band_: U32) -> M.Ts:
  says(sign(s, who, 3), 0, band_)

def talk_mira(+s: M.Sess, +who: U32, +held: U32, +band_: U32) -> M.Ts:
  +both = Bool.and(m_has_mask(s, 1), m_has_mask(s, 2))
  ite(M.Ts, eq(held, 4),
    u => ite(M.Ts, Bool.and(U32.is_ge(day_index(s), 1), eq(M.fg_routine(figure_of(s, 0)), 10)),
      u => says(keep_mask(give_away(s, who), who, 1, 1, 4), 3, 0),
      u => says(s, 5, 2)),
    u => ite(M.Ts, Bool.not(m_knows(s, 4)),
      u => says(learn(s, 4), 1, 0),
      u => ite(M.Ts, cry_active(s, 0),
        u => says(s, 6, 0),
        u => ite(M.Ts, m_has_keep(s, 2),
          u => says(s, 4, 0),
          u => ite(M.Ts, within(s, 255, 1, 6),
            u => ite(M.Ts, both,
              u => says(tick_folk(sign(M.s_set_keeps(s, U32.or(M.s_keeps(s), 2)), who, 13), 0), 2, 0),
              u => says(s, 5, 0)),
            u => ite(M.Ts, both,
              u => says(s, 5, 1),
              u => says(s, 0, band_)))))))

def talk_pella(+s: M.Sess, +who: U32, +held: U32, +band_: U32) -> M.Ts:
  ite(M.Ts, Bool.not(m_knows(s, 2)),
    u => says(learn(s, 2), 1, 0),
    u => ite(M.Ts, cry_active(s, 4),
      u => says(s, 6, cry_number(s)),
      u => ite(M.Ts, m_has_mask(s, 1),
        u => says(s, 4, 0),
        u => ite(M.Ts, eq(held, 4),
          u => says(s, 5, 0),
          u => ite(M.Ts, Bool.not(within(s, 0, 2, 5)),
            u => says(s, 5, 1),
            u => ite(M.Ts, eq(held, 0),
              u => says(take(s, who, 4), 2, 0),
              u => hands_full(s, who, band_)))))))

def talk_nessa_reeds(+s: M.Sess, +who: U32, +me: M.Kn) -> M.Ts:
  ite(M.Ts, Bool.not(dry(s, me)),
    u => says(s, 5, 2),
    u => ite(M.Ts, Bool.not(within(s, 1, 5, 9)),
      u => says(s, 5, 3),
      u => says(keep_mask(give_away(s, who), who, 2, 2, 2), 3, 0)))

def talk_nessa_shoe(+s: M.Sess, +who: U32, +held: U32, +band_: U32) -> M.Ts:
  ite(M.Ts, Bool.not(m_knows(s, 6)),
    u => says(s, 5, 0),
    u => ite(M.Ts, eq(M.s_shod_on(s), 1),
      u => says(s, 4, 0),
      u => ite(M.Ts, eq(held, 0),
        u => says(take(s, who, 5), 2, 0),
        u => hands_full(s, who, band_))))

def talk_nessa(+s: M.Sess, +who: U32, +held: U32, +band_: U32, +me: M.Kn) -> M.Ts:
  ite(M.Ts, Bool.not(m_knows(s, 1)),
    u => says(learn(s, 1), 1, 0),
    u => ite(M.Ts, eq(held, 2),
      u => talk_nessa_reeds(s, who, me),
      u => ite(M.Ts, shoe_day(s),
        u => talk_nessa_shoe(s, who, held, band_),
        u => ite(M.Ts, m_has_mask(s, 2),
          u => ite(M.Ts, Bool.and(m_knows(s, 6), Bool.not(m_has_keep(s, 4))),
            u => says(s, 5, 1),
            u => says(s, 4, 0)),
          u => says(s, 0, band_)))))

def orin_ink(+s: M.Sess, +who: U32, +held: U32, +band_: U32) -> M.Ts:
  +ready = (M.s_ink_given(s) + 5 * M.s_n_day(s) / 24 : U32)
  ite(M.Ts, U32.is_lt(M.s_clock(s), ready),
    u => says(s, 5, 0),
    u => ite(M.Ts, Bool.and(m_ticked(s, 3), m_has_keep(s, 1)),
      u => says(s, 4, 0),
      u => ite(M.Ts, U32.is_gt(M.s_clock(s), (ready + 2 * M.s_n_day(s) / 24 : U32)),
        u => says(s, 5, 2),
        u => ite(M.Ts, eq(held, 0),
          u => says(tick_folk(take(s, who, 3), 3), 2, 0),
          u => hands_full(s, who, band_)))))

def talk_orin(+s: M.Sess, +who: U32, +held: U32, +band_: U32) -> M.Ts:
  ite(M.Ts, Bool.not(m_knows(s, 0)),
    u => says(learn(s, 0), 1, 0),
    u => ite(M.Ts, eq(held, 1),
      u => ite(M.Ts, within(s, 0, 2, 5),
        u => says(M.s_set_ink_given(M.s_set_ink_on(give_away(s, who), 1), M.s_clock(s)), 3, 0),
        u => says(s, 5, 1)),
      u => ite(M.Ts, eq(M.s_ink_on(s), 1),
        u => orin_ink(s, who, held, band_),
        u => says(s, 0, band_))))

def talk_tovin(+s: M.Sess, +who: U32, +held: U32, +band_: U32, +hour: U32) -> M.Ts:
  ite(M.Ts, Bool.not(m_knows(s, 3)),
    u => says(learn(s, 3), 1, 0),
    u => ite(M.Ts, eq(held, 3),
      u => ite(M.Ts, within(s, 1, 22, 28),
        u => says(tick_folk(sign(M.s_set_keeps(give_away(s, who), U32.or(M.s_keeps(s), 1)), who, 14), 1), 3, 0),
        u => says(s, 5, 1)),
      u => ite(M.Ts, m_has_keep(s, 1),
        u => says(s, 4, 0),
        u => ite(M.Ts, Bool.or(U32.is_ge(hour, 20), U32.is_lt(hour, 6)),
          u => says(s, 0, 3),
          u => says(s, 0, band_)))))

def talk_aster(+s: M.Sess, +who: U32, +held: U32, +band_: U32) -> M.Ts:
  ite(M.Ts, Bool.and(eq(held, 5), eq(M.s_shod_on(s), 0)),
    u => says(M.s_set_shod(M.s_set_shod_on(give_away(s, who), 1), M.s_clock(s)), 3, 0),
    u => ite(M.Ts, m_has_keep(s, 4),
      u => says(s, 4, 0),
      u => ite(M.Ts, eq(M.s_shod_on(s), 1),
        u => says(s, 2, 0),
        u => says(learn(s, 6), 1, 0))))

# Start a conversation: the words chosen and what they mean done, then the talk opens.
def talk(+s: M.Sess, +who: U32, +to: U32) -> M.Sess:
  +held = held_of(s, who)
  +hour = c_hour(M.s_clock(s), M.s_n_day(s))
  +band_ = band(hour)
  +me = get_k(s, who)
  +r = ite(M.Ts, eq(to, 0),
    u => talk_mira(s, who, held, band_),
    u => ite(M.Ts, eq(to, 4),
      u => talk_pella(s, who, held, band_),
      u => ite(M.Ts, eq(to, 2),
        u => talk_nessa(s, who, held, band_, me),
        u => ite(M.Ts, eq(to, 3),
          u => talk_orin(s, who, held, band_),
          u => ite(M.Ts, eq(to, 1),
            u => talk_tovin(s, who, held, band_, hour),
            u => talk_aster(s, who, held, band_))))))
  edit(M.ts_s(r), who, k => M.k_set_say_age(M.k_set_say_page(M.k_set_say_ta(M.k_set_say_tk(M.k_set_say_folk(M.k_set_say_on(k, 1), to), M.ts_tk(r)), M.ts_ta(r)), 0), 0))

# E pressed: do the one thing it means here.
def press_use_u(+s: M.Sess, +who: U32, +u: U32) -> M.Sess:
  ite(M.Sess, Bool.and(U32.is_ge(u, 1), U32.is_le(u, 6)),
    u2 => talk(s, who, (u - 1 : U32)),
    u2 => ite(M.Sess, eq(u, 7),
      u2 => use_ink(s, who),
      u2 => ite(M.Sess, eq(u, 8),
        u2 => use_reeds(s, who),
        u2 => ite(M.Sess, eq(u, 9),
          u2 => door(s, who, True{}),
          u2 => ite(M.Sess, eq(u, 10),
            u2 => door(s, who, False{}),
            u2 => ite(M.Sess, eq(u, 11),
              u2 => use_well(s, who),
              u2 => ite(M.Sess, eq(u, 12),
                u2 => use_bell(s, who),
                u2 => ite(M.Sess, U32.is_ge(u, 13),
                  u2 => relight(s, who, (u - 13 : U32)),
                  u2 => s))))))))

def press_use(+s: M.Sess, +who: U32) -> M.Sess:
  press_use_u(s, who, nearest_use(s, who))

# --- the Gloam's wisps (wisps.rs) -------------------------------------------------------
# Wisp modes: 0 hunting, 1 putting a light out (ticks left), 2 driven off (ticks left).
# The length of (dx, dy) in integers, the same answer on every machine.
def isqrt_go(it: Nat, +lo: Nat, +hi: Nat, +n: Nat) -> Nat:
  match it:
    case 0n:
      lo
    case 1n+p:
      +mid = Nat.div(Nat.add(Nat.add(lo, hi), 1n), 2n)
      +ok = Nat.is_le(Nat.mul(mid, mid), n)
      isqrt_go(p, Bool.pick(Nat, ok, mid, lo), Bool.pick(Nat, ok, hi, Nat.sub(mid, 1n)), n)

def isqrt(+dx: U32, +dy: U32) -> U32:
  +ax = U32.to_nat(sabs(dx))
  +ay = U32.to_nat(sabs(dy))
  U32.from_nat(isqrt_go(24n, 0n, 1048576n, Nat.add(Nat.mul(ax, ax), Nat.mul(ay, ay))))

# Wisps come in from around the edge of the town, in pixels.
def coming_x(+i: U32) -> U32:
  Bool.pick(U32, eq(i, 0), 4294967276, Bool.pick(U32, eq(i, 1), 150, Bool.pick(U32, eq(i, 2), 300, Bool.pick(U32, eq(i, 3), 470, Bool.pick(U32, eq(i, 4), 630, Bool.pick(U32, eq(i, 5), 630, Bool.pick(U32, eq(i, 6), 330, 4294967276)))))))

def coming_y(+i: U32) -> U32:
  Bool.pick(U32, eq(i, 0), 400, Bool.pick(U32, eq(i, 1), 440, Bool.pick(U32, eq(i, 2), 450, Bool.pick(U32, eq(i, 3), 440, Bool.pick(U32, eq(i, 4), 300, Bool.pick(U32, eq(i, 5), 120, Bool.pick(U32, eq(i, 6), 4294967276, 180)))))))

def wl_len(+l: List<&2, M.Wp>) -> U32:
  match l:
    case Nil{}:
      0
    case Con{_, +t}:
      (wl_len(t) + 1 : U32)

# How many wisps the night holds for the table: the number, two more for every
# knight beyond the first, and one more in the Final Hours.
def wisp_want(+s: M.Sess) -> U32:
  ite(U32, Bool.or(Bool.not(c_night(M.s_clock(s), M.s_n_day(s))), eq(M.s_done_on(s), 1)),
    u => 0,
    u => U32.min((M.s_n_wisps(s) + 2 * (peak1(s) - 1 : U32) + bu(final_hours(s)) : U32), 24))

# The lit lights, by id and place, in pixels: lanterns 0 to 5, then the three towers.
def lt_if(+c: Bool, +l: M.Lt) -> List<&2, M.Lt>:
  Bool.pick(List<&2, M.Lt>, c, Con{l, Nil{}}, Nil{})

def lamp_light(+s: M.Sess, +ls: List<&2, M.Leg>, +n: U32) -> List<&2, M.Lt>:
  lt_if(lantern_lit_l(s, ls, n), M.Lt{n, (lamp_tx(n) * 16 + 8 : U32), 272, 0})

def flame_light(+s: M.Sess, +i: U32, +x: U32, +y: U32) -> List<&2, M.Lt>:
  lt_if(flame(s, i), M.Lt{(6 + i : U32), x, y, 0})

def lights(+s: M.Sess, +ls: List<&2, M.Leg>) -> List<&2, M.Lt>:
  List.append(&2, M.Lt, lamp_light(s, ls, 0), List.append(&2, M.Lt, lamp_light(s, ls, 1), List.append(&2, M.Lt, lamp_light(s, ls, 2), List.append(&2, M.Lt, lamp_light(s, ls, 3), List.append(&2, M.Lt, lamp_light(s, ls, 4), List.append(&2, M.Lt, lamp_light(s, ls, 5), List.append(&2, M.Lt, flame_light(s, 0, 296, 72), List.append(&2, M.Lt, flame_light(s, 1, 280, 392), flame_light(s, 2, 288, 208)))))))))

# The seats on their feet, as pixel points.
def up_points(+l: List<&2, M.Kn>) -> List<&2, M.P>:
  match l:
    case Nil{}:
      Nil{}
    case Con{+k, +t}:
      Bool.pick(List<&2, M.P>, eq(M.k_down(k), 0), Con{M.P{(M.k_x(k) / 256 : U32), (M.k_y(k) / 256 : U32)}, up_points(t)}, up_points(t))

# Whether a knight stands close enough to a light to keep wisps off it (40 pixels).
def guarded(+ks: List<&2, M.P>, +x: U32, +y: U32) -> Bool:
  match ks:
    case Nil{}:
      False{}
    case Con{+p, +t}:
      Bool.or(U32.is_le(dist2(M.pt_x(p), M.pt_y(p), x, y), 1600), guarded(t, x, y))

def lt_before(+a: M.Lt, +b: M.Lt) -> Bool:
  Bool.or(U32.is_lt(M.lt_d2(a), M.lt_d2(b)), Bool.and(eq(M.lt_d2(a), M.lt_d2(b)), U32.is_lt(M.lt_id(a), M.lt_id(b))))

def lt_ins(+l: M.Lt, +sorted: List<&2, M.Lt>) -> List<&2, M.Lt>:
  match sorted:
    case Nil{}:
      Con{l, Nil{}}
    case Con{+h, +t}:
      Bool.pick(List<&2, M.Lt>, lt_before(l, h), Con{l, Con{h, t}}, Con{h, lt_ins(l, t)})

# The lights ordered by distance from (hx, hy), then by id.
def lt_sort(+l: List<&2, M.Lt>, +hx: U32, +hy: U32, +acc: List<&2, M.Lt>) -> List<&2, M.Lt>:
  match l:
    case Nil{}:
      acc
    case Con{+h, +t}:
      lt_sort(t, hx, hy, lt_ins(M.lt_set_d2(h, dist2(hx, hy, M.lt_x(h), M.lt_y(h))), acc))

def lt_nth(+l: List<&2, M.Lt>, n: Nat) -> M.Lt:
  match l:
    case Nil{}:
      M.Lt{255, 0, 0, 0}
    case Con{+h, +t}:
      match n:
        case 0n:
          h
        case 1n+p:
          lt_nth(t, p)

# The light with id `id` (id 255 when it is not lit).
def lt_find(+l: List<&2, M.Lt>, +id: U32) -> M.Lt:
  match l:
    case Nil{}:
      M.Lt{255, 0, 0, 0}
    case Con{+h, +t}:
      Bool.pick(M.Lt, eq(M.lt_id(h), id), h, lt_find(t, id))

def lt_len(+l: List<&2, M.Lt>) -> U32:
  match l:
    case Nil{}:
      0
    case Con{_, +t}:
      (lt_len(t) + 1 : U32)

def wisp_hunt_go(+w: M.Wp, +ls: List<&2, M.Lt>, +ks: List<&2, M.P>, +drift: U32, +hx: U32, +hy: U32) -> M.Wr:
  +sorted = lt_sort(ls, hx, hy, Nil{})
  +t = lt_nth(sorted, U32.to_nat((M.w_born(w) % U32.min(lt_len(ls), 3) : U32)))
  +dx = (M.lt_x(t) * 256 - M.w_x(w) : U32)
  +dy = (M.lt_y(t) * 256 - M.w_y(w) : U32)
  +len = isqrt(dx, dy)
  +w1 = M.w_set_aim(w, M.lt_id(t))
  ite(M.Wr, U32.is_le(len, 1280),
    u => M.Wr{Bool.pick(M.Wp, guarded(ks, M.lt_x(t), M.lt_y(t)), M.w_set_t(M.w_set_mode(w1, 2), 90), M.w_set_t(M.w_set_mode(w1, 1), 60)), 255},
    u => M.Wr{M.w_set_y(M.w_set_x(w1, (M.w_x(w) + sdiv((dx * drift : U32), len) : U32)), (M.w_y(w) + sdiv((dy * drift : U32), len) : U32)), 255})

def wisp_hunt(+w: M.Wp, +ls: List<&2, M.Lt>, +ks: List<&2, M.P>, +drift: U32, +hx: U32, +hy: U32) -> M.Wr:
  ite(M.Wr, eq(lt_len(ls), 0),
    u => M.Wr{w, 255},
    u => wisp_hunt_go(w, ls, ks, drift, hx, hy))

def wisp_snuff(+w: M.Wp, +ls: List<&2, M.Lt>, +ks: List<&2, M.P>) -> M.Wr:
  +l = lt_find(ls, M.w_aim(w))
  ite(M.Wr, eq(M.lt_id(l), 255),
    u => M.Wr{M.w_set_t(M.w_set_mode(w, 0), 0), 255},
    u => ite(M.Wr, guarded(ks, M.lt_x(l), M.lt_y(l)),
      u => M.Wr{M.w_set_t(M.w_set_mode(w, 2), 90), 255},
      u => ite(M.Wr, U32.is_le(M.w_t(w), 1),
        u => M.Wr{M.w_set_t(M.w_set_mode(w, 0), 0), M.w_aim(w)},
        u => M.Wr{M.w_set_t(w, (M.w_t(w) - 1 : U32)), 255})))

# The nearest knight (first on a tie), as a light record with id 1, or id 0 for none.
def near_knight(+ks: List<&2, M.P>, +hx: U32, +hy: U32, +best: M.Lt) -> M.Lt:
  match ks:
    case Nil{}:
      best
    case Con{+p, +t}:
      +d = dist2(hx, hy, M.pt_x(p), M.pt_y(p))
      near_knight(t, hx, hy, Bool.pick(M.Lt, Bool.or(eq(M.lt_id(best), 0), U32.is_lt(d, M.lt_d2(best))), M.Lt{1, M.pt_x(p), M.pt_y(p), d}, best))

def wisp_shy(+w: M.Wp, +ls: List<&2, M.Lt>, +ks: List<&2, M.P>, +drift: U32, +hx: U32, +hy: U32) -> M.Wr:
  +nk = near_knight(ks, hx, hy, M.Lt{0, 0, 0, 0})
  +lit = lt_find(ls, M.w_aim(w))
  +fx = Bool.pick(U32, eq(M.lt_id(nk), 1), M.lt_x(nk), Bool.pick(U32, ne(M.lt_id(lit), 255), M.lt_x(lit), hx))
  +fy = Bool.pick(U32, eq(M.lt_id(nk), 1), M.lt_y(nk), Bool.pick(U32, ne(M.lt_id(lit), 255), M.lt_y(lit), hy))
  +dx = (hx - fx : U32)
  +dy = (hy - fy : U32)
  +len = U32.max(isqrt(dx, dy), 1)
  +step = (drift * 3 / 2 : U32)
  +w1 = M.w_set_y(M.w_set_x(w, (M.w_x(w) + sdiv((dx * step : U32), len) : U32)), (M.w_y(w) + sdiv((dy * step : U32), len) : U32))
  M.Wr{Bool.pick(M.Wp, U32.is_le(M.w_t(w), 1), M.w_set_t(M.w_set_mode(w1, 0), 0), M.w_set_t(w1, (M.w_t(w) - 1 : U32))), 255}

def wisp_step(+w: M.Wp, +ls: List<&2, M.Lt>, +ks: List<&2, M.P>, +drift: U32) -> M.Wr:
  +hx = sdiv(M.w_x(w), 256)
  +hy = sdiv(M.w_y(w), 256)
  ite(M.Wr, eq(M.w_mode(w), 0),
    u => wisp_hunt(w, ls, ks, drift, hx, hy),
    u => ite(M.Wr, eq(M.w_mode(w), 1),
      u => wisp_snuff(w, ls, ks),
      u => wisp_shy(w, ls, ks, drift, hx, hy)))

def wisps_go(+ws: List<&2, M.Wp>, +ls: List<&2, M.Lt>, +ks: List<&2, M.P>, +drift: U32) -> M.Wl:
  match ws:
    case Nil{}:
      M.Wl{Nil{}, Nil{}}
    case Con{+w, +rest}:
      +r = wisp_step(w, ls, ks, drift)
      +tail = wisps_go(rest, ls, ks, drift)
      M.Wl{Con{M.wr_w(r), M.wl_ws(tail)}, Bool.pick(List<&2, U32>, ne(M.wr_out(r), 255), Con{M.wr_out(r), M.wl_outs(tail)}, M.wl_outs(tail))}

def lu_has(+l: List<&2, U32>, +v: U32) -> Bool:
  match l:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(eq(h, v), lu_has(t, v))

# The knights a wisp at (hx, hy) touches: on their feet, unwarded, not mid-sidestep,
# no menu open, within nine pixels. Each is listed once.
def touch_seats(+seats: List<&2, M.Kn>, +hx: U32, +hy: U32, +h: List<&2, U32>) -> List<&2, U32>:
  match seats:
    case Nil{}:
      h
    case Con{+k, +t}:
      +hit = Bool.and(Bool.and(Bool.and(eq(M.k_down(k), 0), eq(M.k_hurt(k), 0)), Bool.and(eq(M.k_roll(k), 0), eq(M.k_panel(k), 0))), Bool.and(Bool.not(lu_has(h, M.k_id(k))), U32.is_le(dist2((M.k_x(k) / 256 : U32), (M.k_y(k) / 256 : U32), hx, hy), 81)))
      touch_seats(t, hx, hy, Bool.pick(List<&2, U32>, hit, lu_push(h, M.k_id(k)), h))

def touches(+ws: List<&2, M.Wp>, +seats: List<&2, M.Kn>, +h: List<&2, U32>) -> List<&2, U32>:
  match ws:
    case Nil{}:
      h
    case Con{+w, +rest}:
      touches(rest, seats, touch_seats(seats, sdiv(M.w_x(w), 256), sdiv(M.w_y(w), 256), h))

# A light goes out: a lantern's bit, or a tower's flame.
def put_out(+s: M.Sess, +aim: U32) -> M.Sess:
  announce(defer(ite(M.Sess, U32.is_lt(aim, 6),
    u => M.s_set_snuffed(s, U32.or(M.s_snuffed(s), pw(aim))),
    u => lit_set(s, (aim - 6 : U32), 0)), q_empty()), 10, 0, 0)

def put_outs(+l: List<&2, U32>, +s: M.Sess) -> M.Sess:
  match l:
    case Nil{}:
      s
    case Con{+a, +t}:
      put_outs(t, put_out(s, a))

# The wisp nearest a point (first on a tie), as a light record with id 1, or id 0 for none.
def near_wisp(+ws: List<&2, M.Wp>, +mx: U32, +my: U32, +best: M.Lt) -> M.Lt:
  match ws:
    case Nil{}:
      best
    case Con{+w, +t}:
      +wx = sdiv(M.w_x(w), 256)
      +wy = sdiv(M.w_y(w), 256)
      +d = dist2(mx, my, wx, wy)
      near_wisp(t, mx, my, Bool.pick(M.Lt, Bool.or(eq(M.lt_id(best), 0), U32.is_lt(d, M.lt_d2(best))), M.Lt{1, wx, wy, d}, best))

# A touch costs a candle; the knight is pushed away from the nearest wisp.
def hurt_one(+s: M.Sess, +id: U32) -> M.Sess:
  +k = get_k(s, id)
  +mx = (M.k_x(k) / 256 : U32)
  +my = (M.k_y(k) / 256 : U32)
  +nw = near_wisp(M.s_wisps(s), mx, my, M.Lt{0, 0, 0, 0})
  +fx = Bool.pick(U32, eq(M.lt_id(nw), 1), M.lt_x(nw), mx)
  +fy = Bool.pick(U32, eq(M.lt_id(nw), 1), M.lt_y(nw), my)
  +dx = sgn((mx - fx : U32))
  +dy = sgn((my - fy : U32))
  +gb = ground(s, False{})
  +x3 = (M.k_x(k) + dx * 3072 : U32)
  +y3 = (M.k_y(k) + dy * 3072 : U32)
  +x2 = (M.k_x(k) + dx * 2048 : U32)
  +y2 = (M.k_y(k) + dy * 2048 : U32)
  +x1 = (M.k_x(k) + dx * 1024 : U32)
  +y1 = (M.k_y(k) + dy * 1024 : U32)
  +c3 = g_clear(gb, x3, y3, M.s_crate_on(s), M.s_crate_x(s), M.s_crate_y(s))
  +c2 = g_clear(gb, x2, y2, M.s_crate_on(s), M.s_crate_x(s), M.s_crate_y(s))
  +c1 = g_clear(gb, x1, y1, M.s_crate_on(s), M.s_crate_x(s), M.s_crate_y(s))
  +nx = Bool.pick(U32, c3, x3, Bool.pick(U32, c2, x2, Bool.pick(U32, c1, x1, M.k_x(k))))
  +ny = Bool.pick(U32, c3, y3, Bool.pick(U32, c2, y2, Bool.pick(U32, c1, y1, M.k_y(k))))
  +candles = sat(M.k_candles(k), 1)
  +k1 = M.k_set_say_on(M.k_set_y(M.k_set_x(M.k_set_hurt(M.k_set_candles(k, candles), 75), nx), ny), 0)
  ite(M.Sess, eq(candles, 0),
    u => cue(put_k(ite(M.Sess, eq(M.k_held(k), 1), u => clear_flag(s, 4), u => s), kn_note(M.k_set_panel(M.k_set_held(M.k_set_down(k1, 90), 0), 0), 8, 0, 0, 90)), q_fall()),
    u => cue(put_k(s, kn_note(k1, 7, candles, 0, 90)), q_hurt()))

def hurt_all(+l: List<&2, U32>, +s: M.Sess) -> M.Sess:
  match l:
    case Nil{}:
      s
    case Con{+id, +t}:
      hurt_all(t, hurt_one(s, id))

def wisp_arrive(+s: M.Sess) -> M.Sess:
  +born = M.s_elapsed(s)
  +i = ((born / 45) % 8 : U32)
  M.s_set_wisps(s, List.append(&2, M.Wp, M.s_wisps(s), Con{M.Wp{(coming_x(i) * 256 : U32), (coming_y(i) * 256 : U32), 0, 0, 0, born}, Nil{}}))

def wisp_live(+s: M.Sess, +want: U32, +mira: List<&2, M.Leg>) -> M.Sess:
  +s1 = ite(M.Sess, Bool.and(U32.is_lt(wl_len(M.s_wisps(s)), want), eq((M.s_clock(s) % 45 : U32), 0)), u => wisp_arrive(s), u => s)
  +drift = (256 + 256 * (peak1(s) - 1 : U32) / 10 : U32)
  +ks = up_points(M.s_seats(s1))
  +r = wisps_go(M.s_wisps(s1), lights(s1, mira), ks, drift)
  +hurts = touches(M.wl_ws(r), M.s_seats(s1), Nil{})
  hurt_all(hurts, put_outs(M.wl_outs(r), M.s_set_wisps(s1, M.wl_ws(r))))

# One tick of the Gloam (`mira`: the lamplighter's legs).
def tick_wisps(+s: M.Sess, +mira: List<&2, M.Leg>) -> M.Sess:
  +want = wisp_want(s)
  ite(M.Sess, eq(want, 0), u => M.s_set_wisps(s, Nil{}), u => wisp_live(s, want, mira))

# --- the bell and the phases (moonfall.rs) -----------------------------------------------
# Rings: 0 back to the first dawn, 1 forward to the next dusk, 2 forward to the next dawn.
# Phases: 0 play, 1 the rewind ritual, 2 time skipped, 3 the moon turns back.
def in_zero() -> M.In:
  M.In{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}

def in_find(+l: List<&2, M.In>, +id: U32) -> M.In:
  match l:
    case Nil{}:
      in_zero()
    case Con{+i, +t}:
      Bool.pick(M.In, eq(M.i_id(i), id), i, in_find(t, id))

# The keys a seat holds this tick: none unless their input is one the game may read.
def in_eff(+s: M.Sess, +id: U32) -> M.In:
  +i = in_find(M.s_inputs(s), id)
  Bool.pick(M.In, eq(M.i_valid(i), 1), i, in_zero())

def pg_find(+l: List<&2, M.Pg>, +id: U32) -> U32:
  match l:
    case Nil{}:
      1
    case Con{+p, +t}:
      Bool.pick(U32, eq(M.p_id(p), id), M.p_pages(p), pg_find(t, id))

def he_find(+l: List<&2, M.He>, +id: U32) -> U32:
  match l:
    case Nil{}:
      2
    case Con{+h, +t}:
      Bool.pick(U32, eq(M.h_id(h), id), M.h_awake(h), he_find(t, id))

# Whether a knight is awake in the Delve (the hero exists, is not away or stone).
def awake(+s: M.Sess, +id: U32) -> Bool:
  eq(he_find(M.s_heroes(s), id), 1)

def hero_here(+s: M.Sess, +id: U32) -> Bool:
  ne(he_find(M.s_heroes(s), id), 2)

def bell_need(+s: M.Sess) -> U32:
  (sl_len(M.s_seats(s)) / 2 + 1 : U32)

def ring_ok(+s: M.Sess, +ring: U32) -> Bool:
  Bool.or(eq(ring, 0), Bool.pick(Bool, eq(ring, 1), U32.is_le(c_next_dusk(M.s_clock(s), M.s_n_day(s)), c_final(M.s_n_day(s))), U32.is_le(c_next_dawn(M.s_clock(s), M.s_n_day(s)), c_final(M.s_n_day(s)))))

# Menus shut and talk ended, for a change of time.
def calm_seats(+l: List<&2, M.Kn>) -> List<&2, M.Kn>:
  match l:
    case Nil{}:
      Nil{}
    case Con{+k, +t}:
      Con{M.k_set_hold(M.k_set_say_on(M.k_set_panel(k, 0), 0), 0), calm_seats(t)}

def calm(+s: M.Sess) -> M.Sess:
  M.s_set_seats(M.s_set_call_on(s, 0), calm_seats(M.s_seats(s)))

def begin_rewind(+s: M.Sess, +fell: Bool) -> M.Sess:
  +s1 = M.s_set_p_fell(M.s_set_p_from(M.s_set_p_t(M.s_set_phase(s, 1), 0), U32.min(M.s_clock(s), c_end(M.s_n_day(s)))), bu(fell))
  ite(M.Sess, fell, u => cue(cue(calm(s1), q_toll()), q_roar()), u => cue(calm(s1), q_toll()))

# Strike the bell: the ritual, or time run on.
def ring_it(+s: M.Sess, +ring: U32) -> M.Sess:
  ite(M.Sess, eq(ring, 0),
    u => begin_rewind(s, False{}),
    u => ite(M.Sess, Bool.not(ring_ok(s, ring)),
      u => s,
      u => cue(calm(M.s_set_p_to(M.s_set_p_from(M.s_set_p_t(M.s_set_phase(s, 2), 0), M.s_clock(s)), Bool.pick(U32, eq(ring, 1), c_next_dusk(M.s_clock(s), M.s_n_day(s)), c_next_dawn(M.s_clock(s), M.s_n_day(s))))), q_toll())))

def put_hand(+s: M.Sess, +id: U32) -> M.Sess:
  ite(M.Sess, eq(M.s_call_on(s), 1), u => M.s_set_call_ayes(s, U32.or(M.s_call_ayes(s), pw((id - 1 : U32)))), u => s)

def ask_new(+s: M.Sess, +id: U32, +ring: U32) -> M.Sess:
  +s1 = announce(s, 11, id, 0)
  ite(M.Sess, U32.is_ge(1, bell_need(s1)),
    u => ring_it(s1, ring),
    u => M.s_set_call_left(M.s_set_call_ayes(M.s_set_call_by(M.s_set_call_ring(M.s_set_call_on(s1, 1), ring), id), pw((id - 1 : U32))), 150))

def ask_bell(+s: M.Sess, +id: U32, +ring: U32) -> M.Sess:
  ite(M.Sess, Bool.not(ring_ok(s, ring)),
    u => s,
    u => ite(M.Sess, eq(M.s_call_on(s), 1),
      u => ite(M.Sess, eq(M.s_call_ring(s), ring), u => put_hand(s, id), u => s),
      u => ask_new(s, id, ring)))

def seated_mask(+l: List<&2, M.Kn>, +m: U32) -> U32:
  match l:
    case Nil{}:
      m
    case Con{+k, +t}:
      seated_mask(t, U32.or(m, pw((M.k_id(k) - 1 : U32))))

def bell_count(+s: M.Sess, +ayes: U32, +left: U32) -> M.Sess:
  +s1 = M.s_set_call_left(M.s_set_call_ayes(s, ayes), left)
  ite(M.Sess, U32.is_ge(pop(ayes), bell_need(s)),
    u => ring_it(M.s_set_call_on(s1, 0), M.s_call_ring(s)),
    u => ite(M.Sess, Bool.or(eq(left, 0), eq(ayes, 0)), u => M.s_set_call_on(s1, 0), u => s1))

# A knight who left takes their hand off the rope; a lapsed call does nothing.
def bell_tick(+s: M.Sess) -> M.Sess:
  ite(M.Sess, eq(M.s_call_on(s), 0),
    u => s,
    u => bell_count(s, U32.and(M.s_call_ayes(s), seated_mask(M.s_seats(s), 0)), sat(M.s_call_left(s), 1)))

# The next mask the Ledger lets a knight wear.
def owned_mask(+s: M.Sess, +m: U32) -> Bool:
  Bool.or(eq(m, 0), Bool.or(Bool.and(eq(m, 1), m_has_mask(s, 1)), Bool.and(eq(m, 2), m_has_mask(s, 2))))

def cycle_mask(+s: M.Sess, +id: U32) -> M.Sess:
  +now = M.k_mask(get_k(s, id))
  +a1 = ((now + 1) % 3 : U32)
  +a2 = ((now + 2) % 3 : U32)
  edit(s, id, k => M.k_set_mask(k, Bool.pick(U32, owned_mask(s, a1), a1, Bool.pick(U32, owned_mask(s, a2), a2, Bool.pick(U32, owned_mask(s, now), now, 0)))))

# --- one knight's tick -------------------------------------------------------------------
def act_down(+s: M.Sess, +k1: M.Kn) -> M.Sess:
  +d = (M.k_down(k1) - 1 : U32)
  +k2 = M.k_set_down(k1, d)
  put_k(s, Bool.pick(M.Kn, eq(d, 0), M.k_set_hurt(M.k_set_candles(M.k_set_y(M.k_set_x(k2, ((sp_tx(M.k_id(k1)) * 16 + 8) * 256 : U32)), ((sp_ty(M.k_id(k1)) * 16 + 8) * 256 : U32)), 1), 90), k2))

def act_ledger(+s: M.Sess, +id: U32, +keys: U32, +before: U32) -> M.Sess:
  +k = get_k(s, id)
  +page = M.k_parg(k)
  +k1 = M.k_set_read(k, m_written(s))
  put_k(s, ite(M.Kn, Bool.or(Bool.or(pressed(keys, before, key_ledger()), pressed(keys, before, key_use())), pressed(keys, before, key_side())),
    u => M.k_set_panel(k1, 0),
    u => ite(M.Kn, pressed(keys, before, key_right()),
      u => M.k_set_parg(k1, ((page + 1) % 7 : U32)),
      u => ite(M.Kn, pressed(keys, before, key_left()),
        u => M.k_set_parg(k1, ((page + 6) % 7 : U32)),
        u => k1))))

def act_bell(+s: M.Sess, +id: U32, +keys: U32, +before: U32) -> M.Sess:
  +k = get_k(s, id)
  +line = M.k_parg(k)
  ite(M.Sess, pressed(keys, before, key_up()),
    u => put_k(s, M.k_set_parg(k, ((line + 2) % 3 : U32))),
    u => ite(M.Sess, pressed(keys, before, key_down()),
      u => put_k(s, M.k_set_parg(k, ((line + 1) % 3 : U32))),
      u => ite(M.Sess, Bool.or(Bool.or(pressed(keys, before, key_ledger()), pressed(keys, before, key_side())), pressed(keys, before, key_mask())),
        u => put_k(s, M.k_set_panel(k, 0)),
        u => ite(M.Sess, Bool.or(pressed(keys, before, key_use()), pressed(keys, before, key_bell())),
          u => ask_bell(put_k(s, M.k_set_panel(k, 0)), id, line),
          u => s))))

# E: the next page of a talk.
def talk_page(+s: M.Sess, +id: U32) -> M.Sess:
  +k = get_k(s, id)
  ite(M.Sess, U32.is_ge((M.k_say_page(k) + 1 : U32), pg_find(M.s_pages(s), id)),
    u => put_k(s, M.k_set_say_on(k, 0)),
    u => put_k(s, M.k_set_say_page(k, (M.k_say_page(k) + 1 : U32))))

# E: the next page of a talk, or what E means here; held, E lights a tower.
def act_use(+s: M.Sess, +id: U32, +keys: U32, +before: U32) -> M.Sess:
  +talking = eq(M.k_say_on(get_k(s, id)), 1)
  ite(M.Sess, pressed(keys, before, key_use()),
    u => ite(M.Sess, talking, u => talk_page(s, id), u => press_use(s, id)),
    u => ite(M.Sess, Bool.and(bit(keys, key_use()), Bool.not(talking)),
      u => hold_use(s, id),
      u => ite(M.Sess, eq(M.k_hold(get_k(s, id)), 0), u => s, u => hold0(s, id))))

def act_drop(+s: M.Sess, +id: U32) -> M.Sess:
  +held = M.k_held(get_k(s, id))
  ite(M.Sess, ne(held, 0),
    u => edit(ite(M.Sess, eq(held, 1), u => clear_flag(s, 4), u => s), id, k => kn_note(M.k_set_held(k, 0), 4, 16, 0, 60)),
    u => s)

# Walking. Moving away from someone ends the talk.
def act_walk(+s: M.Sess, +id: U32, +inp: M.In, +keys: U32, +before: U32) -> M.Sess:
  +k = get_k(s, id)
  +gb = ground(s, eq(M.k_mask(k), 1))
  +w = walk_step(k, M.i_mx(inp), M.i_my(inp), speed(M.s_n_walk(s), M.k_mask(k)), gb, pressed(keys, before, key_side()), M.s_crate_on(s), M.s_crate_x(s), M.s_crate_y(s))
  +k1 = ite(M.Kn, Bool.and(eq(M.i_mx(inp), 0), eq(M.i_my(inp), 0)), u => M.wk_k(w), u => M.k_set_say_on(M.wk_k(w), 0))
  +crate = Bool.or(eq(M.wk_filled(w), 1), eq(M.wk_moved(w), 1))
  +s1 = ite(M.Sess, crate, u => M.s_set_crate_y(M.s_set_crate_x(M.s_set_crate_on(put_k(s, k1), M.wk_con(w)), M.wk_cx(w)), M.wk_cy(w)), u => put_k(s, k1))
  +s2 = ite(M.Sess, eq(M.wk_filled(w), 1), u => cue(set_flag(s1, 8), q_rock()), u => s1)
  +s3 = ite(M.Sess, eq(M.wk_moved(w), 1), u => cue(s2, q_rock()), u => s2)
  ite(M.Sess, Bool.and(eq(M.wk_heavy(w), 1), eq(M.k_note_on(k1), 0)), u => edit(s3, id, k => kn_note(k, 4, 8, 0, 60)), u => s3)

def act_rest(+s: M.Sess, +id: U32, +inp: M.In, +keys: U32, +before: U32) -> M.Sess:
  +s1 = ite(M.Sess, pressed(keys, before, key_mask()), u => cycle_mask(s, id), u => s)
  +s2 = ite(M.Sess, pressed(keys, before, key_drop()), u => act_drop(s1, id), u => s1)
  act_walk(act_use(s2, id, keys, before), id, inp, keys, before)

def act_shut(+s: M.Sess, +id: U32, +inp: M.In, +keys: U32, +before: U32) -> M.Sess:
  ite(M.Sess, pressed(keys, before, key_ledger()),
    u => edit(s, id, k => M.k_set_read(M.k_set_say_on(M.k_set_parg(M.k_set_panel(k, 1), 0), 0), m_written(s))),
    u => ite(M.Sess, Bool.and(pressed(keys, before, key_bell()), eq(M.s_done_on(s), 0)),
      u => ite(M.Sess, Bool.and(eq(M.s_call_on(s), 1), Bool.not(bit(M.s_call_ayes(s), pw((id - 1 : U32))))),
        u => put_hand(s, id),
        u => ite(M.Sess, eq(M.s_call_on(s), 0),
          u => edit(s, id, k => M.k_set_say_on(M.k_set_parg(M.k_set_panel(k, 2), 0), 0)),
          u => s)),
      u => act_rest(s, id, inp, keys, before)))

def act_panel(+s: M.Sess, +id: U32, +inp: M.In, +keys: U32, +before: U32) -> M.Sess:
  +panel = M.k_panel(get_k(s, id))
  ite(M.Sess, eq(panel, 1),
    u => act_ledger(s, id, keys, before),
    u => ite(M.Sess, eq(panel, 2),
      u => act_bell(s, id, keys, before),
      u => act_shut(s, id, inp, keys, before)))

# One knight's tick.
def act_in(+s: M.Sess, +id: U32, +inp: M.In, +keys: U32, +k0: M.Kn) -> M.Sess:
  +k1 = M.k_set_hurt(M.k_set_keys(k0, keys), sat(M.k_hurt(k0), 1))
  ite(M.Sess, U32.is_gt(M.k_down(k1), 0),
    u => act_down(put_k(s, k1), k1),
    u => act_panel(put_k(s, k1), id, inp, keys, M.k_keys(k0)))

def act(+s: M.Sess, +id: U32, +inp: M.In) -> M.Sess:
  ite(M.Sess, sl_has(M.s_seats(s), id),
    u => act_in(s, id, inp, keys_of(inp), get_k(s, id)),
    u => s)

# --- the loop's turns --------------------------------------------------------------------
# The town returns to the first dawn. The Ledger and the masks stay; everything else,
# carried items and opened doors and the rest, is wound back.
def reset_seats(+l: List<&2, M.Kn>) -> List<&2, M.Kn>:
  match l:
    case Nil{}:
      Nil{}
    case Con{+k, +t}:
      Con{kn_new(M.k_id(k), M.k_joined(k), M.k_mask(k)), reset_seats(t)}

def control(+s: M.Sess) -> U32:
  U32.max(((M.s_n_rewind(s) * 7) / 10 : U32), 1)

def reset(+s: M.Sess) -> M.Sess:
  +s1 = M.s_set_news(M.s_set_cry_on(M.s_set_call_on(M.s_set_card(M.s_set_phase(M.s_set_clock(M.s_set_loops(s, (M.s_loops(s) + 1 : U32)), 0), 0), (sat(M.s_n_rewind(s), control(s)) + 24 : U32)), 0), 0), Nil{})
  +s2 = M.s_set_wisps(M.s_set_lit2(M.s_set_lit1(M.s_set_lit0(M.s_set_snuffed(M.s_set_crate_y(M.s_set_crate_x(M.s_set_crate_on(M.s_set_shod(M.s_set_shod_on(M.s_set_ink_given(M.s_set_ink_on(M.s_set_flags(s1, 0), 0), 0), 0), 0), 1), 17), 20), 0), 0), 0), 0), Nil{})
  M.s_set_parked(M.s_set_seats(s2, reset_seats(M.s_seats(s2))), reset_seats(M.s_parked(s2)))

def rewinding(+s: M.Sess, +t0: U32) -> M.Sess:
  +t = (t0 + 1 : U32)
  +c = control(s)
  +s1 = ite(M.Sess, eq(t, (c / 2 : U32)), u => cue(s, q_wall()), u => s)
  ite(M.Sess, U32.is_ge(t, c), u => cue(reset(s1), q_card()), u => M.s_set_p_t(s1, t))

def skipping(+s: M.Sess, +t0: U32) -> M.Sess:
  +t = (t0 + 1 : U32)
  +day = M.s_n_day(s)
  ite(M.Sess, U32.is_ge(t, 36),
    u => cue(M.s_set_phase(M.s_set_wisps(ite(M.Sess, Bool.or(ne((M.s_p_to(s) / day : U32), (M.s_p_from(s) / day : U32)), eq((M.s_p_to(s) % day : U32), 0)), u => M.s_set_snuffed(M.s_set_clock(s, M.s_p_to(s)), 0), u => M.s_set_clock(s, M.s_p_to(s))), Nil{}), 0), q_toll()),
    u => M.s_set_p_t(s, t))

def winning(+s: M.Sess, +t0: U32) -> M.Sess:
  +t = (t0 + 1 : U32)
  ite(M.Sess, U32.is_ge(t, 120),
    u => cue(M.s_set_phase(M.s_set_wisps(M.s_set_lit2(M.s_set_lit1(M.s_set_lit0(M.s_set_clock(M.s_set_done_at(M.s_set_done_on(s, 1), M.s_elapsed(s)), c_end(M.s_n_day(s))), (c_end(M.s_n_day(s)) * 2 : U32)), (c_end(M.s_n_day(s)) * 2 : U32)), (c_end(M.s_n_day(s)) * 2 : U32)), Nil{}), 0), q_wish()),
    u => M.s_set_p_t(s, t))

# --- the town's own tick -------------------------------------------------------------------
# Pella cries the hours and Mira says her verse when they reach their places.
def cry_set(+s: M.Sess, +who: U32, +ta: U32) -> M.Sess:
  M.s_set_cry_age(M.s_set_cry_ta(M.s_set_cry_tk(M.s_set_cry_folk(M.s_set_cry_on(s, 1), who), 6), ta), 0)

def cry_scan(+ls: List<&2, M.Leg>, +s: M.Sess, +who: U32, +routine: U32, +ta: U32, +fires: Bool) -> M.Sess:
  match ls:
    case Nil{}:
      s
    case Con{+l, +t}:
      cry_scan(t, ite(M.Sess, Bool.and(Bool.and(Bool.and(eq(M.lg_routine(l), routine), eq(M.lg_arrive(l), M.s_clock(s))), ne(M.lg_arrive(l), 0)), fires), u => cry_set(s, who, ta), u => s), who, routine, ta, fires)

def cries(+s: M.Sess, +pella: List<&2, M.Leg>, +mira: List<&2, M.Leg>) -> M.Sess:
  +d = day_index(s)
  +a = cry_scan(pella, s, 4, 2, 0, True{})
  +b = cry_scan(pella, a, 4, 4, (d + 1 : U32), True{})
  cry_scan(mira, b, 0, 10, 0, eq(d, 2))

# Whether any knight on their feet is within `notice` pixels of (x, y).
def any_near(+ks: List<&2, M.P>, +x: U32, +y: U32, +r2: U32) -> Bool:
  match ks:
    case Nil{}:
      False{}
    case Con{+p, +t}:
      Bool.or(U32.is_le(dist2(M.pt_x(p), M.pt_y(p), x, y), r2), any_near(t, x, y, r2))

def witness_fact(+s: M.Sess, +who: U32, +routine: U32) -> M.Sess:
  +d = day_index(s)
  ite(M.Sess, Bool.and(Bool.and(eq(who, 4), eq(routine, 4)), eq(d, 0)),
    u => learn(s, 5),
    u => ite(M.Sess, Bool.and(eq(who, 5), eq(routine, 0)),
      u => learn(s, 6),
      u => ite(M.Sess, Bool.and(Bool.and(eq(who, 0), eq(routine, 10)), eq(d, 2)),
        u => learn(s, 7),
        u => s)))

def witness_one(+s: M.Sess, +who: U32, +ks: List<&2, M.P>) -> M.Sess:
  +fig = figure_of(s, who)
  +n = M.s_n_notice(s)
  ite(M.Sess, Bool.and(eq(M.fg_arrived(fig), 1), any_near(ks, M.fg_x(fig), M.fg_y(fig), (n * n : U32))),
    u => witness_fact(ite(M.Sess, Bool.and(Bool.and(ne(M.fg_routine(fig), 255), U32.is_lt(M.fg_routine(fig), 32)), Bool.not(m_witnessed(s, who, M.fg_routine(fig)))),
      u => announce(m_witness(s, who, M.fg_routine(fig)), 0, who, M.fg_routine(fig)),
      u => s), who, M.fg_routine(fig)),
    u => s)

# The Ledger writes what a knight is near enough to see.
def witness(+s: M.Sess) -> M.Sess:
  +ks = up_points(M.s_seats(s))
  +a = witness_one(s, 0, ks)
  +b = witness_one(a, 1, ks)
  +c = witness_one(b, 2, ks)
  +d = witness_one(c, 3, ks)
  +e = witness_one(d, 4, ks)
  witness_one(e, 5, ks)

# The moon's shadow puts a street lantern out: lantern `n` goes at fin + n hours / 2.
def snuff_one(+s: M.Sess, +n: U32, +fin: U32, +hour: U32) -> M.Sess:
  ite(M.Sess, Bool.and(eq(M.s_clock(s), (fin + n * hour / 2 : U32)), Bool.not(bit(M.s_snuffed(s), pw(n)))),
    u => announce(M.s_set_snuffed(s, U32.or(M.s_snuffed(s), pw(n))), 10, 0, 0),
    u => s)

def snuff_lamps(+s: M.Sess, +fin: U32, +hour: U32) -> M.Sess:
  snuff_one(snuff_one(snuff_one(snuff_one(snuff_one(snuff_one(s, 0, fin, hour), 1, fin, hour), 2, fin, hour), 3, fin, hour), 4, fin, hour), 5, fin, hour)

# Aster's oil reaches the well.
def oil_arrives(+s: M.Sess) -> M.Sess:
  ite(M.Sess, Bool.and(eq(M.s_shod_on(s), 1), Bool.and(Bool.not(m_has_keep(s, 4)), U32.is_ge(M.s_clock(s), f_aster_at_well(M.s_shod(s))))),
    u => cue(tick_folk(M.s_set_keeps(s, U32.or(M.s_keeps(s), 4)), 5), q_wish()),
    u => s)

def world_tick(+s: M.Sess) -> M.Sess:
  +day = M.s_n_day(s)
  +hour = U32.max((day / 24 : U32), 1)
  +before = c_bridge_up(M.s_clock(s), day)
  +s1 = M.s_set_clock(s, U32.min((M.s_clock(s) + 1 : U32), c_end(day)))
  +c1 = M.s_clock(s1)
  +s2 = ite(M.Sess, eq((c1 % hour : U32), 0), u => ite(M.Sess, Bool.or(eq((c_hour(c1, day) % 6 : U32), 0), final_hours(s1)), u => cue(s1, q_toll()), u => s1), u => s1)
  +s3 = ite(M.Sess, Bool.not(U32.is_eq(bu(c_bridge_up(c1, day)), bu(before))), u => cue(s2, q_door()), u => s2)
  +s4 = ite(M.Sess, eq((c1 % day : U32), 0), u => M.s_set_snuffed(s3, 0), u => s3)
  +s5 = ite(M.Sess, final_hours(s4), u => snuff_lamps(s4, c_final(day), hour), u => s4)
  +s6 = oil_arrives(s5)
  +mira = f_legs(0, day)
  +s7 = cries(s6, f_legs(4, day), mira)
  +s8 = ite(M.Sess, eq((c1 % 5 : U32), 0), u => witness(s7), u => s7)
  tick_wisps(reminders(s8, c1, day), mira)

def all_flames(+s: M.Sess) -> Bool:
  Bool.and(flame(s, 0), Bool.and(flame(s, 1), flame(s, 2)))

def play_after(+s: M.Sess) -> M.Sess:
  +s1 = world_tick(s)
  ite(M.Sess, all_flames(s1),
    u => cue(cue(M.s_set_p_t(M.s_set_phase(s1, 3), 0), q_toll()), q_wish()),
    u => ite(M.Sess, U32.is_ge(M.s_clock(s1), c_end(M.s_n_day(s1))), u => begin_rewind(s1, True{}), u => s1))

def play_go(+l: List<&2, M.Kn>, +s: M.Sess) -> M.Sess:
  match l:
    case Nil{}:
      s
    case Con{+k, +t}:
      play_go(t, act(s, M.k_id(k), in_eff(s, M.k_id(k))))

def play(+s: M.Sess) -> M.Sess:
  +s1 = bell_tick(play_go(M.s_seats(s), s))
  ite(M.Sess, eq(M.s_done_on(s1), 1), u => s1, u => play_after(s1))

# --- the session's tick (Session::step) ---------------------------------------------------
# The timers that only run down.
def news_age(+l: List<&2, M.Nw>) -> List<&2, M.Nw>:
  match l:
    case Nil{}:
      Nil{}
    case Con{+n, +t}:
      Bool.pick(List<&2, M.Nw>, U32.is_gt(sat(M.n_left(n), 1), 0), Con{M.n_set_left(n, sat(M.n_left(n), 1)), news_age(t)}, news_age(t))

def age_seats(+l: List<&2, M.Kn>) -> List<&2, M.Kn>:
  match l:
    case Nil{}:
      Nil{}
    case Con{+k, +t}:
      +left = sat(M.k_note_left(k), 1)
      +age = (M.k_say_age(k) + 1 : U32)
      Con{M.k_set_say_on(M.k_set_say_age(M.k_set_note_on(M.k_set_note_left(k, Bool.pick(U32, eq(M.k_note_on(k), 1), left, M.k_note_left(k))), Bool.pick(U32, Bool.and(eq(M.k_note_on(k), 1), eq(left, 0)), 0, M.k_note_on(k))), Bool.pick(U32, eq(M.k_say_on(k), 1), age, M.k_say_age(k))), Bool.pick(U32, Bool.and(eq(M.k_say_on(k), 1), U32.is_gt(age, 360)), 0, M.k_say_on(k))), age_seats(t)}

def age(+s: M.Sess) -> M.Sess:
  +cry_age = (M.s_cry_age(s) + 1 : U32)
  +s1 = M.s_set_seats(M.s_set_news(M.s_set_card(s, sat(M.s_card(s), 1)), news_age(M.s_news(s))), age_seats(M.s_seats(s)))
  ite(M.Sess, eq(M.s_cry_on(s), 1),
    u => ite(M.Sess, U32.is_gt(cry_age, 150), u => M.s_set_cry_age(M.s_set_cry_on(s1, 0), cry_age), u => M.s_set_cry_age(s1, cry_age)),
    u => s1)

# A knight who stepped away stands up; one who is still in the party is parked for later.
def drop_gone(+l: List<&2, M.Kn>, +s: M.Sess) -> M.Sess:
  match l:
    case Nil{}:
      s
    case Con{+k, +t}:
      drop_gone(t, ite(M.Sess, awake(s, M.k_id(k)),
        u => s,
        u => ite(M.Sess, hero_here(s, M.k_id(k)),
          u => M.s_set_parked(M.s_set_seats(s, sl_del(M.s_seats(s), M.k_id(k))), sl_ins(M.s_parked(s), k)),
          u => M.s_set_seats(s, sl_del(M.s_seats(s), M.k_id(k))))))

def keep_parked(+l: List<&2, M.Kn>, +s: M.Sess) -> List<&2, M.Kn>:
  match l:
    case Nil{}:
      Nil{}
    case Con{+k, +t}:
      Bool.pick(List<&2, M.Kn>, hero_here(s, M.k_id(k)), Con{k, keep_parked(t, s)}, keep_parked(t, s))

# A knight who joins late sits down at the well (or where they stood, if parked).
def sit(+s: M.Sess, +id: U32) -> M.Sess:
  +k = Bool.pick(M.Kn, sl_has(M.s_parked(s), id), sl_get(M.s_parked(s), id), kn_new(id, M.s_elapsed(s), 0))
  +s1 = M.s_set_parked(M.s_set_seats(s, sl_ins(M.s_seats(s), k)), sl_del(M.s_parked(s), id))
  M.s_set_peak(s1, U32.max(M.s_peak(s1), sl_len(M.s_seats(s1))))

def late_go(+hs: List<&2, M.He>, +s: M.Sess) -> M.Sess:
  match hs:
    case Nil{}:
      s
    case Con{+h, +t}:
      late_go(t, ite(M.Sess, Bool.and(eq(M.h_awake(h), 1), Bool.not(sl_has(M.s_seats(s), M.h_id(h)))), u => sit(s, M.h_id(h)), u => s))

def keep_parked_s(+s: M.Sess) -> M.Sess:
  M.s_set_parked(s, keep_parked(M.s_parked(s), s))

# One tick: the seats are whoever is here and awake.
def step_core(+s: M.Sess) -> M.Sess:
  +s1 = M.s_set_elapsed(s, U32.min((M.s_elapsed(s) + 1 : U32), 108000))
  +s2 = keep_parked_s(drop_gone(M.s_seats(s1), s1))
  +s3 = age(late_go(M.s_heroes(s2), s2))
  +s4 = M.s_set_clock(s3, U32.min(M.s_clock(s3), c_end(M.s_n_day(s3))))
  +phase = M.s_phase(s4)
  +s5 = ite(M.Sess, eq(phase, 0),
    u => play(s4),
    u => ite(M.Sess, eq(phase, 1),
      u => rewinding(s4, M.s_p_t(s4)),
      u => ite(M.Sess, eq(phase, 2), u => skipping(s4, M.s_p_t(s4)), u => winning(s4, M.s_p_t(s4)))))
  M.s_set_pending(M.s_set_cues(s5, List.append(&2, U32, M.s_cues(s5), M.s_pending(s5))), Nil{})


# The knights who walk out of the town after the moon has turned back: E pressed with
# nothing to mean anything to (any key row, whether or not the seat is at the table).
def leave_go(+l: List<&2, M.In>, +s: M.Sess, +acc: List<&2, U32>) -> List<&2, U32>:
  match l:
    case Nil{}:
      acc
    case Con{+i, +t}:
      leave_go(t, s, Bool.pick(List<&2, U32>, Bool.and(eq(M.i_interact(i), 1), eq(nearest_use(s, M.i_id(i)), 0)), lu_push(acc, M.i_id(i)), acc))

def main_step(+s: M.Sess) -> M.Out:
  +s1 = step_core(s)
  +fresh = Bool.and(eq(M.s_done_on(s1), 1), eq(M.s_done_on(s), 0))
  +freed = Bool.and(eq(M.s_done_on(s1), 1), eq(M.s_phase(s1), 0))
  M.Out{s1, ite(List<&2, U32>, freed, u => leave_go(M.s_inputs(s1), s1, Nil{}), u => Nil{}), Bool.pick(U32, fresh, U32.clamp(sl_len(M.s_seats(s1)), 1, 6), 0), U32.clamp(sat(1000, (150 * M.s_loops(s1) : U32)), 100, 1000)}

# --- what a bot presses (plan.rs) ----------------------------------------------------------
# A bot is a stand-in for a player who has read the Ledger. It is stateless: each tick it
# works out the table's tasks from the session, gives each task to the knight best placed
# for it (the same answer for every bot at the table), and does its own.

# The tile to stand on for each of the places E means something.
# Tasks: light 1 if a tower's flame; hand 0 free, 1 holds the item, 2 any; aim 0 a villager
# (the folk number), 1 a tile; act 0 tap, 1 hold, 2 stand, 3 the south route.

def cat_tk(+a: List<&2, M.Tk>, +b: List<&2, M.Tk>) -> List<&2, M.Tk>:
  List.append(&2, M.Tk, a, b)

def tk_if(+c: Bool, +t: M.Tk) -> List<&2, M.Tk>:
  Bool.pick(List<&2, M.Tk>, c, Con{t, Nil{}}, Nil{})

# A tick on a day (0 to 2) at a label hour and minute.
def at_(+day: U32, +d: U32, +hh: U32, +mm: U32) -> U32:
  +since = Bool.pick(U32, U32.is_lt(hh, 6), (hh + 18 : U32), (hh - 6 : U32))
  ((((d * 24 + since) * 60 + mm) * day) / 1440 : U32)

# The last of the intervals a villager can be seen.
def iv_merge(+out: List<&2, M.Iv>, +folk: U32, +a: U32, +b: U32) -> List<&2, M.Iv>:
  match out:
    case Nil{}:
      Con{M.Iv{folk, a, b}, Nil{}}
    case Con{+l, +rest}:
      Bool.pick(List<&2, M.Iv>, U32.is_ge(M.iv_b(l), a), Con{M.Iv{folk, M.iv_a(l), U32.max(M.iv_b(l), b)}, rest}, Con{M.Iv{folk, a, b}, out})

def iv_add(+out: List<&2, M.Iv>, +folk: U32, +a: U32, +b: U32) -> List<&2, M.Iv>:
  Bool.pick(List<&2, M.Iv>, U32.is_gt(b, a), iv_merge(out, folk, a, b), out)

def next_depart(+ls: List<&2, M.Leg>, +end: U32) -> U32:
  match ls:
    case Nil{}:
      end
    case Con{+l, _}:
      M.lg_depart(l)

def seen_go(+ls: List<&2, M.Leg>, +folk: U32, +end: U32, +i: U32, +acc: List<&2, M.Iv>) -> List<&2, M.Iv>:
  match ls:
    case Nil{}:
      acc
    case Con{+l, +rest}:
      +a1 = Bool.pick(List<&2, M.Iv>, U32.is_gt(i, 0), iv_add(acc, folk, M.lg_depart(l), M.lg_arrive(l)), acc)
      +a2 = Bool.pick(List<&2, M.Iv>, eq(M.lg_inside(l), 0), iv_add(a1, folk, M.lg_arrive(l), next_depart(rest, end)), a1)
      seen_go(rest, folk, end, (i + 1 : U32), a2)

# When a villager can be seen: Aster only between riding in and riding out.
def seen_of(+folk: U32, +ls: List<&2, M.Leg>, +end: U32) -> List<&2, M.Iv>:
  List.reverse(&2, M.Iv, ite(List<&2, M.Iv>, eq(folk, 5),
    u => iv_add(Nil{}, 5, M.lg_depart(lg_nth(ls, 1n)), M.lg_arrive(lg_nth(ls, 2n))),
    u => seen_go(ls, folk, end, 0, Nil{})))

def fig_core(+s: M.Sess, +folk: U32, +legs: List<&2, M.Leg>, +clock: U32, +shod_on: U32, +shod: U32) -> M.Fg:
  ite(M.Fg, Bool.and(eq(folk, 5), Bool.and(eq(shod_on, 1), U32.is_ge(clock, shod))),
    u => f_aster_after(clock, M.s_n_day(s), shod),
    u => f_figure_legs(folk, clock, legs))

# The legs of every villager, when each can be seen, and where each stands now.
def cx_new(+s: M.Sess) -> M.Cx:
  +day = M.s_n_day(s)
  +end = c_end(day)
  +l0 = f_legs(0, day)
  +l1 = f_legs(1, day)
  +l2 = f_legs(2, day)
  +l3 = f_legs(3, day)
  +l4 = f_legs(4, day)
  +l5 = f_legs(5, day)
  +now = M.s_clock(s)
  +so = M.s_shod_on(s)
  +sh = M.s_shod(s)
  M.Cx{l0, l1, l2, l3, l4, l5, List.append(&2, M.Iv, seen_of(0, l0, end), List.append(&2, M.Iv, seen_of(1, l1, end), List.append(&2, M.Iv, seen_of(2, l2, end), List.append(&2, M.Iv, seen_of(3, l3, end), List.append(&2, M.Iv, seen_of(4, l4, end), seen_of(5, l5, end)))))), fig_core(s, 0, l0, now, so, sh), fig_core(s, 1, l1, now, so, sh), fig_core(s, 2, l2, now, so, sh), fig_core(s, 3, l3, now, so, sh), fig_core(s, 4, l4, now, so, sh), fig_core(s, 5, l5, now, so, sh)}

def cx_legs(+cx: M.Cx, +folk: U32) -> List<&2, M.Leg>:
  Bool.pick(List<&2, M.Leg>, eq(folk, 0), M.cx_l0(cx), Bool.pick(List<&2, M.Leg>, eq(folk, 1), M.cx_l1(cx), Bool.pick(List<&2, M.Leg>, eq(folk, 2), M.cx_l2(cx), Bool.pick(List<&2, M.Leg>, eq(folk, 3), M.cx_l3(cx), Bool.pick(List<&2, M.Leg>, eq(folk, 4), M.cx_l4(cx), M.cx_l5(cx))))))

def fig_at(+s: M.Sess, +cx: M.Cx, +folk: U32, +clock: U32, +shod_on: U32, +shod: U32) -> M.Fg:
  fig_core(s, folk, cx_legs(cx, folk), clock, shod_on, shod)

def fig_now(+s: M.Sess, +cx: M.Cx, +folk: U32) -> M.Fg:
  Bool.pick(M.Fg, eq(folk, 0), M.cx_g0(cx), Bool.pick(M.Fg, eq(folk, 1), M.cx_g1(cx), Bool.pick(M.Fg, eq(folk, 2), M.cx_g2(cx), Bool.pick(M.Fg, eq(folk, 3), M.cx_g3(cx), Bool.pick(M.Fg, eq(folk, 4), M.cx_g4(cx), M.cx_g5(cx))))))

# The next tick at or after `clock` that `folk` can be seen (the schedule only).
def ns_go(+sn: List<&2, M.Iv>, +folk: U32, +clock: U32) -> M.Pf:
  match sn:
    case Nil{}:
      M.Pf{0, 0, 0}
    case Con{+iv, +rest}:
      Bool.pick(M.Pf, Bool.and(eq(M.iv_folk(iv), folk), U32.is_gt(M.iv_b(iv), clock)), M.Pf{1, U32.max(M.iv_a(iv), clock), 0}, ns_go(rest, folk, clock))

def aim_folk_later(+s: M.Sess, +cx: M.Cx, +folk: U32, +ns: M.Pf) -> M.Pf:
  ite(M.Pf, eq(M.pf_ok(ns), 0),
    u => M.Pf{0, 0, 0},
    u => M.Pf{1, M.fg_x(fig_at(s, cx, folk, (M.pf_x(ns) + 2 : U32), M.s_shod_on(s), M.s_shod(s))), M.fg_y(fig_at(s, cx, folk, (M.pf_x(ns) + 2 : U32), M.s_shod_on(s), M.s_shod(s)))})

# Where a task is done, as pixels (ok 0: nowhere to be had).
def aim_px(+s: M.Sess, +cx: M.Cx, +ak: U32, +aa: U32, +ab: U32, +when: U32) -> M.Pf:
  ite(M.Pf, eq(ak, 1),
    u => M.Pf{1, (aa * 16 + 8 : U32), (ab * 16 + 8 : U32)},
    u => ite(M.Pf, Bool.and(eq(M.fg_visible(fig_now(s, cx, aa)), 1), U32.is_le(when, M.s_clock(s))),
      u => M.Pf{1, M.fg_x(fig_now(s, cx, aa)), M.fg_y(fig_now(s, cx, aa))},
      u => aim_folk_later(s, cx, aa, ns_go(M.cx_sn(cx), aa, U32.max(when, M.s_clock(s))))))

def doable(+s: M.Sess, +cx: M.Cx, +t: M.Tk) -> Bool:
  Bool.and(Bool.and(U32.is_ge(M.s_clock(s), M.tk_opens(t)), U32.is_lt(M.s_clock(s), M.tk_closes(t))), Bool.or(eq(M.tk_ak(t), 1), eq(M.fg_visible(fig_now(s, cx, M.tk_aa(t))), 1)))

# When a task will next be doable, if it still can be this loop.
def when_ok(+s: M.Sess, +cx: M.Cx, +t: M.Tk) -> M.Pf:
  +opens = U32.max(M.tk_opens(t), M.s_clock(s))
  +ns = ns_go(M.cx_sn(cx), M.tk_aa(t), opens)
  ite(M.Pf, U32.is_ge(M.s_clock(s), M.tk_closes(t)),
    u => M.Pf{0, 0, 0},
    u => ite(M.Pf, eq(M.tk_ak(t), 1),
      u => M.Pf{1, opens, 0},
      u => Bool.pick(M.Pf, Bool.and(eq(M.pf_ok(ns), 1), U32.is_lt(M.pf_x(ns), M.tk_closes(t))), M.Pf{1, M.pf_x(ns), 0}, M.Pf{0, 0, 0})))

def held_by_any(+l: List<&2, M.Kn>, +item: U32) -> Bool:
  match l:
    case Nil{}:
      False{}
    case Con{+k, +t}:
      Bool.or(eq(M.k_held(k), item), held_by_any(t, item))

def up_ids(+l: List<&2, M.Kn>) -> List<&2, U32>:
  match l:
    case Nil{}:
      Nil{}
    case Con{+k, +t}:
      Bool.pick(List<&2, U32>, eq(M.k_down(k), 0), Con{M.k_id(k), up_ids(t)}, up_ids(t))

def manhattan(+ax: U32, +ay: U32, +bx: U32, +by: U32) -> U32:
  (sabs((ax - bx : U32)) + sabs((ay - by : U32)) : U32)

# The asks, nearest the first up knight first (then by who): (distance, folk).
def ask_ins(+d: U32, +who: U32, +sorted: List<&2, M.Lt>) -> List<&2, M.Lt>:
  match sorted:
    case Nil{}:
      Con{M.Lt{who, 0, 0, d}, Nil{}}
    case Con{+h, +t}:
      Bool.pick(List<&2, M.Lt>, lt_before(M.Lt{who, 0, 0, d}, h), Con{M.Lt{who, 0, 0, d}, Con{h, t}}, Con{h, ask_ins(d, who, t)})

def ask_add(+s: M.Sess, +cx: M.Cx, +sorted: List<&2, M.Lt>, +fx: U32, +fy: U32, +fact: U32, +who: U32) -> List<&2, M.Lt>:
  +fig = fig_at(s, cx, who, U32.max(M.s_clock(s), 1), 0, 0)
  Bool.pick(List<&2, M.Lt>, m_knows(s, fact), sorted, ask_ins(manhattan(fx, fy, M.fg_x(fig), M.fg_y(fig)), who, sorted))

def ask_tasks(+l: List<&2, M.Lt>, +end: U32) -> List<&2, M.Tk>:
  match l:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Con{M.Tk{0, 0, 0, 0, M.lt_id(h), 0, 0, 0, end}, ask_tasks(t, end)}

def first_up_x(+l: List<&2, M.Kn>) -> U32:
  match l:
    case Nil{}:
      280
    case Con{+k, +t}:
      Bool.pick(U32, eq(M.k_down(k), 0), (M.k_x(k) / 256 : U32), first_up_x(t))

def first_up_y(+l: List<&2, M.Kn>) -> U32:
  match l:
    case Nil{}:
      248
    case Con{+k, +t}:
      Bool.pick(U32, eq(M.k_down(k), 0), (M.k_y(k) / 256 : U32), first_up_y(t))

def ready_soon(+s: M.Sess, +i: U32) -> Bool:
  Bool.and(m_has_keep(s, Bool.pick(U32, eq(i, 0), 1, Bool.pick(U32, eq(i, 1), 2, 4))), m_knows(s, 7))

def light_task(+s: M.Sess, +i: U32, +tx: U32, +ty: U32, +fin: U32, +end: U32) -> List<&2, M.Tk>:
  tk_if(Bool.and(ready_soon(s, i), Bool.not(flame(s, i))), M.Tk{1, 2, 0, 1, tx, ty, 1, fin, end})

def guard_task(+s: M.Sess, +i: U32, +tx: U32, +ty: U32, +end: U32) -> List<&2, M.Tk>:
  tk_if(flame(s, i), M.Tk{0, 2, 0, 1, tx, ty, 2, 0, end})

def relight_task(+s: M.Sess, +cx: M.Cx, +n: U32, +end: U32) -> List<&2, M.Tk>:
  tk_if(Bool.and(lantern_scheduled_l(s, cx_legs(cx, 0), n), bit(M.s_snuffed(s), pw(n))), M.Tk{0, 0, 0, 1, lamp_tx(n), 18, 0, 0, end})

# The table's tasks in the order they matter, whether they can be done yet or not.
def tasks(+s: M.Sess, +cx: M.Cx) -> List<&2, M.Tk>:
  +day = M.s_n_day(s)
  +end = c_end(day)
  +fin = c_final(day)
  +hour = (day / 24 : U32)
  +now = M.s_clock(s)
  +seats = M.s_seats(s)
  +swift = m_has_mask(s, 1)
  +stout = m_has_mask(s, 2)
  +lights = cat_tk(light_task(s, 0, 18, 4, fin, end), cat_tk(light_task(s, 2, 17, 14, fin, end), light_task(s, 1, 17, 24, fin, end)))
  +guards = cat_tk(guard_task(s, 2, 17, 14, end), cat_tk(guard_task(s, 1, 17, 24, end), guard_task(s, 0, 18, 4, end)))
  +carry = cat_tk(cat_tk(cat_tk(Con{M.Tk{0, 1, 4, 0, 0, 0, 0, at_(day, 1, 20, 55), at_(day, 1, 22, 15)}, Con{M.Tk{0, 1, 4, 0, 0, 0, 0, at_(day, 2, 20, 55), at_(day, 2, 22, 15)}, Nil{}}}, Con{M.Tk{0, 1, 2, 0, 2, 0, 0, at_(day, 1, 8, 30), at_(day, 1, 10, 30)}, Nil{}}), Con{M.Tk{0, 1, 3, 0, 1, 0, 0, at_(day, 1, 17, 0), at_(day, 1, 19, 40)}, Nil{}}), cat_tk(Con{M.Tk{0, 1, 5, 0, 5, 0, 0, at_(day, 1, 16, 35), at_(day, 1, 23, 0)}, Nil{}}, Con{M.Tk{0, 1, 1, 0, 3, 0, 0, at_(day, 0, 7, 0), at_(day, 0, 8, 25)}, Nil{}}))
  +witness = cat_tk(tk_if(Bool.not(m_knows(s, 5)), M.Tk{0, 2, 0, 1, 21, 16, 2, at_(day, 0, 17, 40), at_(day, 0, 18, 10)}), cat_tk(tk_if(Bool.not(m_knows(s, 6)), M.Tk{0, 2, 0, 1, 34, 15, 2, at_(day, 1, 16, 20), at_(day, 1, 23, 0)}), tk_if(Bool.not(m_knows(s, 7)), M.Tk{0, 2, 0, 1, 15, 15, 2, at_(day, 2, 20, 45), at_(day, 2, 22, 0)})))
  +sealing = Bool.and(Bool.and(m_knows(s, 4), Bool.and(swift, stout)), Bool.not(m_has_keep(s, 2)))
  +seal = cat_tk(tk_if(sealing, M.Tk{0, 2, 0, 0, 0, 0, 0, at_(day, 0, 6, 50), at_(day, 0, 9, 0)}), cat_tk(tk_if(sealing, M.Tk{0, 2, 0, 0, 0, 0, 0, at_(day, 1, 6, 50), at_(day, 1, 9, 0)}), tk_if(sealing, M.Tk{0, 2, 0, 0, 0, 0, 0, at_(day, 2, 6, 50), at_(day, 2, 9, 0)})))
  +shoe = tk_if(Bool.and(Bool.and(m_knows(s, 6), eq(M.s_shod_on(s), 0)), Bool.and(Bool.not(m_has_keep(s, 4)), Bool.not(held_by_any(seats, 5)))), M.Tk{0, 0, 0, 0, 2, 0, 0, at_(day, 1, 6, 35), at_(day, 1, 7, 55)})
  +pass = tk_if(Bool.and(Bool.and(eq(M.s_ink_on(s), 1), Bool.not(m_has_keep(s, 1))), Bool.not(held_by_any(seats, 3))), M.Tk{0, 0, 0, 0, 3, 0, 0, (M.s_ink_given(s) + 5 * hour : U32), (M.s_ink_given(s) + 7 * hour : U32)})
  +favours = cat_tk(tk_if(Bool.and(Bool.and(m_knows(s, 0), eq(M.s_ink_on(s), 0)), Bool.and(Bool.not(has_flag(s, 4)), Bool.not(m_has_keep(s, 1)))), M.Tk{0, 0, 0, 1, 11, 3, 0, at_(day, 0, 6, 15), at_(day, 0, 8, 0)}), cat_tk(tk_if(Bool.and(Bool.and(m_knows(s, 2), Bool.not(swift)), Bool.not(held_by_any(seats, 4))), M.Tk{0, 0, 0, 0, 4, 0, 0, at_(day, 0, 7, 0), at_(day, 0, 8, 30)}), tk_if(Bool.and(Bool.and(m_knows(s, 1), Bool.not(stout)), Bool.not(held_by_any(seats, 2))), M.Tk{0, 0, 0, 1, 13, 5, 0, at_(day, 0, 18, 0), at_(day, 0, 22, 0)})))
  +fx = first_up_x(seats)
  +fy = first_up_y(seats)
  +asks0 = ask_add(s, cx, Nil{}, fx, fy, 0, 3)
  +asks1 = ask_add(s, cx, asks0, fx, fy, 1, 2)
  +asks2 = ask_add(s, cx, asks1, fx, fy, 2, 4)
  +asks3 = ask_add(s, cx, asks2, fx, fy, 3, 1)
  +asks4 = ask_add(s, cx, asks3, fx, fy, 4, 0)
  +doors = cat_tk(tk_if(Bool.and(m_has_keep(s, 2), Bool.not(has_flag(s, 2))), M.Tk{0, 2, 0, 1, 17, 24, 3, at_(day, 2, 8, 0), end}), tk_if(Bool.and(m_has_keep(s, 1), Bool.not(has_flag(s, 1))), M.Tk{0, 2, 0, 1, 18, 4, 0, at_(day, 2, 17, 0), end}))
  +lamps = Bool.pick(List<&2, M.Tk>, Bool.and(c_night(now, day), Bool.or(U32.is_lt((now + 2 * hour : U32), fin), U32.is_ge(now, fin))), cat_tk(relight_task(s, cx, 0, end), cat_tk(relight_task(s, cx, 1, end), cat_tk(relight_task(s, cx, 2, end), cat_tk(relight_task(s, cx, 3, end), cat_tk(relight_task(s, cx, 4, end), relight_task(s, cx, 5, end)))))), Nil{})
  cat_tk(lights, cat_tk(guards, cat_tk(carry, cat_tk(witness, cat_tk(seal, cat_tk(shoe, cat_tk(pass, cat_tk(favours, cat_tk(ask_tasks(asks4, end), cat_tk(doors, lamps))))))))))

# --- who does what ------------------------------------------------------------------------
def hand_ok(+held: U32, +hk: U32, +hi: U32) -> Bool:
  Bool.or(eq(hk, 2), Bool.or(Bool.and(eq(hk, 0), eq(held, 0)), Bool.and(eq(hk, 1), eq(held, hi))))

def lu_without(+l: List<&2, U32>, +v: U32) -> List<&2, U32>:
  match l:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(List<&2, U32>, eq(h, v), t, Con{h, lu_without(t, v)})

# The knight nearest a target who can do the task (smallest distance, then smallest id).
def nearest_free(+free: List<&2, U32>, +seats: List<&2, M.Kn>, +hk: U32, +hi: U32, +tx: U32, +ty: U32, +best: M.Lt) -> M.Lt:
  match free:
    case Nil{}:
      best
    case Con{+id, +rest}:
      +k = sl_get(seats, id)
      +d = manhattan((M.k_x(k) / 256 : U32), (M.k_y(k) / 256 : U32), tx, ty)
      nearest_free(rest, seats, hk, hi, tx, ty, Bool.pick(M.Lt, Bool.and(hand_ok(M.k_held(k), hk, hi), Bool.or(eq(M.lt_id(best), 0), U32.is_lt(d, M.lt_d2(best)))), M.Lt{1, id, 0, d}, best))

def lu_nth(+l: List<&2, U32>, n: Nat) -> U32:
  match l:
    case Nil{}:
      0
    case Con{+h, +t}:
      match n:
        case 0n:
          h
        case 1n+p:
          lu_nth(t, p)

def lu_len(+l: List<&2, U32>) -> U32:
  match l:
    case Nil{}:
      0
    case Con{_, +t}:
      (lu_len(t) + 1 : U32)

# Each flame has its knight, by seat order, however they have wandered.
def light_role(+t: M.Tk) -> U32:
  Bool.pick(U32, Bool.and(eq(M.tk_aa(t), 18), eq(M.tk_ab(t), 4)), 0, Bool.pick(U32, Bool.and(eq(M.tk_aa(t), 17), eq(M.tk_ab(t), 14)), 1, 2))

def pick_role(+free: List<&2, U32>, +id: U32) -> M.Lt:
  Bool.pick(M.Lt, lu_has(free, id), M.Lt{1, id, 0, 0}, M.Lt{0, 0, 0, 0})

# The knight a task goes to, as a light record (id 1 found, x the knight).
def pick_knight(+s: M.Sess, +ids: List<&2, U32>, +free: List<&2, U32>, +t: M.Tk, +tx: U32, +ty: U32) -> M.Lt:
  ite(M.Lt, eq(M.tk_light(t), 1),
    u => pick_role(free, lu_nth(ids, U32.to_nat((light_role(t) % U32.max(lu_len(ids), 1) : U32)))),
    u => nearest_free(free, M.s_seats(s), M.tk_hk(t), M.tk_hi(t), tx, ty, M.Lt{0, 0, 0, 0}))

def assign_best(+st: M.Ar, +t: M.Tk, +best: M.Lt) -> M.Ar:
  Bool.pick(M.Ar, eq(M.lt_id(best), 1), M.Ar{lu_without(M.ar_free(st), M.lt_x(best)), List.append(&2, M.As, M.ar_given(st), Con{M.As{M.lt_x(best), 1, t}, Nil{}})}, st)

def assign_one(+s: M.Sess, +cx: M.Cx, +ids: List<&2, U32>, +st: M.Ar, +t: M.Tk) -> M.Ar:
  +target = aim_px(s, cx, M.tk_ak(t), M.tk_aa(t), M.tk_ab(t), M.tk_opens(t))
  ite(M.Ar, eq(M.pf_ok(target), 0),
    u => st,
    u => assign_best(st, t, pick_knight(s, ids, M.ar_free(st), t, M.pf_x(target), M.pf_y(target))))

def assign_go(+order: List<&2, M.Tk>, +s: M.Sess, +cx: M.Cx, +ids: List<&2, U32>, +st: M.Ar) -> M.Ar:
  match order:
    case Nil{}:
      st
    case Con{+t, +rest}:
      assign_go(rest, s, cx, ids, assign_one(s, cx, ids, st, t))

def doable_tasks(+all: List<&2, M.Tk>, +s: M.Sess, +cx: M.Cx) -> List<&2, M.Tk>:
  match all:
    case Nil{}:
      Nil{}
    case Con{+t, +rest}:
      Bool.pick(List<&2, M.Tk>, doable(s, cx, t), Con{t, doable_tasks(rest, s, cx)}, doable_tasks(rest, s, cx))

# Tasks not doable yet that will be, as (when, task) in the order they open (ties in task order).
def later_ins(+w: U32, +t: M.Tk, +sorted: List<&2, M.Ly>) -> List<&2, M.Ly>:
  match sorted:
    case Nil{}:
      Con{M.Ly{w, t}, Nil{}}
    case Con{+h, +rest}:
      Bool.pick(List<&2, M.Ly>, U32.is_lt(w, M.ly_w(h)), Con{M.Ly{w, t}, sorted}, Con{h, later_ins(w, t, rest)})

def later_go(+all: List<&2, M.Tk>, +s: M.Sess, +cx: M.Cx, +acc: List<&2, M.Ly>) -> List<&2, M.Ly>:
  match all:
    case Nil{}:
      acc
    case Con{+t, +rest}:
      +w = when_ok(s, cx, t)
      later_go(rest, s, cx, Bool.pick(List<&2, M.Ly>, Bool.and(Bool.not(doable(s, cx, t)), eq(M.pf_ok(w), 1)), later_ins(M.pf_x(w), t, acc), acc))

def pt_tasks(+l: List<&2, M.Ly>) -> List<&2, M.Tk>:
  match l:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Con{M.ly_t(h), pt_tasks(t)}

def idle_rows(+free: List<&2, U32>) -> List<&2, M.As>:
  match free:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      Con{M.As{id, 0, M.Tk{0, 0, 0, 0, 0, 0, 0, 0, 0}}, idle_rows(t)}

def assign_end(+st: M.Ar) -> List<&2, M.As>:
  List.append(&2, M.As, M.ar_given(st), idle_rows(M.ar_free(st)))

# Each task, in order of importance, to the knight nearest it who can do it.
def assign(+s: M.Sess, +cx: M.Cx, +all: List<&2, M.Tk>) -> List<&2, M.As>:
  +ids = up_ids(M.s_seats(s))
  ite(List<&2, M.As>, eq(lu_len(ids), 0),
    u => Nil{},
    u => assign_end(assign_go(List.append(&2, M.Tk, doable_tasks(all, s, cx), pt_tasks(later_go(all, s, cx, Nil{}))), s, cx, ids, M.Ar{ids, Nil{}})))

# --- walking to a tile: a flood of distances from the goal ---------------------------------------
# The walking distance from every open tile to the goal (65535: not reachable), over the
# ground the knight stands on. The array holds a distance per tile (tile index = row * 38 + column).
type BR is Type:
  BR{a: Array<U32>, next: List<&2, U32>}

def tile_free(+gb: U32, +con: U32, +cx: U32, +cy: U32, +tx: U32, +ty: U32) -> Bool:
  Bool.and(Bool.and(Bool.and(U32.is_lt(tx, 38), U32.is_lt(ty, 28)), g_open(gb, D.cls(tx, ty))), Bool.not(Bool.and(eq(con, 1), Bool.and(eq(cx, tx), eq(cy, ty)))))

def nb_set(+go: Bool, a: Array<U32>, +next: List<&2, U32>, +idx: U32, +d1: U32) -> BR:
  match go:
    case True{}:
      BR{Array.set(U32, a, idx, d1), Con{idx, next}}
    case False{}:
      BR{a, next}

def nb_get(g: Array<U32> & U32, +next: List<&2, U32>, +ok: Bool, +idx: U32, +d1: U32) -> BR:
  (a, +v) = g
  nb_set(Bool.and(ok, eq(v, 65535)), a, next, idx, d1)

def nb(r: BR, +ok: Bool, +idx: U32, +d1: U32) -> BR:
  match r:
    case BR{a, +next}:
      nb_get(Array.get(U32, a, idx), next, ok, idx, d1)

def ix(+ok: Bool, +tx: U32, +ty: U32) -> U32:
  Bool.pick(U32, ok, (ty * 38 + tx : U32), 0)

def expand(r: BR, +n: U32, +d1: U32, +gb: U32, +con: U32, +cx: U32, +cy: U32) -> BR:
  +tx = (n % 38 : U32)
  +ty = (n / 38 : U32)
  +okn = tile_free(gb, con, cx, cy, tx, (ty - 1 : U32))
  +oks = tile_free(gb, con, cx, cy, tx, (ty + 1 : U32))
  +okw = tile_free(gb, con, cx, cy, (tx - 1 : U32), ty)
  +oke = tile_free(gb, con, cx, cy, (tx + 1 : U32), ty)
  nb(nb(nb(nb(r, okn, ix(okn, tx, (ty - 1 : U32)), d1), oks, ix(oks, tx, (ty + 1 : U32)), d1), okw, ix(okw, (tx - 1 : U32), ty), d1), oke, ix(oke, (tx + 1 : U32), ty), d1)

def layer(front: List<&2, U32>, r: BR, +d1: U32, +gb: U32, +con: U32, +cx: U32, +cy: U32) -> BR:
  match front:
    case Nil{}:
      r
    case Con{+n, rest}:
      layer(rest, expand(r, n, d1, gb, con, cx, cy), d1, gb, con, cx, cy)

def bfs_done(r: BR) -> Array<U32>:
  match r:
    case BR{a, _}:
      a

def bfs(p: Nat, r: BR, +d: U32, +gb: U32, +con: U32, +cx: U32, +cy: U32) -> Array<U32>:
  match p:
    case 0n:
      bfs_done(r)
    case 1n+q:
      match r:
        case BR{a, next}:
          match next:
            case Nil{}:
              a
            case Con{+h, +t}:
              bfs(q, layer(Con{h, t}, BR{a, Nil{}}, (d + 1 : U32), gb, con, cx, cy), (d + 1 : U32), gb, con, cx, cy)

def flood_go(+ok: Bool, a: Array<U32>, +gi: U32, +gb: U32, +con: U32, +cx: U32, +cy: U32) -> Array<U32>:
  match ok:
    case True{}:
      bfs(255n, BR{Array.set(U32, a, gi, 0), Con{gi, Nil{}}}, 0, gb, con, cx, cy)
    case False{}:
      a

def flood(+gx: U32, +gy: U32, +gb: U32, +con: U32, +cx: U32, +cy: U32) -> Array<U32>:
  +ok = tile_free(gb, con, cx, cy, gx, gy)
  flood_go(ok, [65535 : U32^11n], ix(ok, gx, gy), gb, con, cx, cy)

def r5e(g: Array<U32> & U32, +vc: U32, +vn: U32, +vs: U32, +vw: U32) -> M.V5:
  (a, +ve) = g
  M.V5{vc, vn, vs, vw, ve}

def r5d(g: Array<U32> & U32, +vc: U32, +vn: U32, +vs: U32, +e: U32) -> M.V5:
  (a, +vw) = g
  r5e(Array.get(U32, a, e), vc, vn, vs, vw)

def r5c(g: Array<U32> & U32, +vc: U32, +vn: U32, +w: U32, +e: U32) -> M.V5:
  (a, +vs) = g
  r5d(Array.get(U32, a, w), vc, vn, vs, e)

def r5b(g: Array<U32> & U32, +vc: U32, +s: U32, +w: U32, +e: U32) -> M.V5:
  (a, +vn) = g
  r5c(Array.get(U32, a, s), vc, vn, w, e)

def r5a(g: Array<U32> & U32, +n: U32, +s: U32, +w: U32, +e: U32) -> M.V5:
  (a, +vc) = g
  r5b(Array.get(U32, a, n), vc, s, w, e)

def read5(a: Array<U32>, +c: U32, +n: U32, +s: U32, +w: U32, +e: U32) -> M.V5:
  r5a(Array.get(U32, a, c), n, s, w, e)

def in_map(+tx: U32, +ty: U32) -> Bool:
  Bool.and(U32.is_lt(tx, 38), U32.is_lt(ty, 28))

def mapv(+ok: Bool, +v: U32) -> U32:
  Bool.pick(U32, ok, v, 65535)

# The step toward a tile, one tile at a time, centred in each so the feet never catch a corner.
def step_pick(+best: U32, +dxy: M.P, +v: U32, +dx: U32, +dy: U32) -> M.P:
  Bool.pick(M.P, U32.is_lt(v, best), M.P{dx, dy}, dxy)

def walk_dir(+v: M.V5, +hn: Bool, +hs: Bool, +hw: Bool, +he: Bool) -> M.P:
  +vn = mapv(hn, M.v5_n(v))
  +vs = mapv(hs, M.v5_s(v))
  +vw = mapv(hw, M.v5_w(v))
  +ve = mapv(he, M.v5_e(v))
  +b0 = M.v5_c(v)
  +bn = U32.min(b0, vn)
  +bs = U32.min(bn, vs)
  +bw = U32.min(bs, vw)
  Bool.pick(M.P, U32.is_lt(ve, bw), M.P{1, 0}, Bool.pick(M.P, U32.is_lt(vw, bs), M.P{4294967295, 0}, Bool.pick(M.P, U32.is_lt(vs, bn), M.P{0, 1}, Bool.pick(M.P, U32.is_lt(vn, b0), M.P{0, 4294967295}, M.P{0, 0}))))

def walk_to_go(+v: M.V5, +hx: U32, +hy: U32, +px: U32, +py: U32, +gx: U32, +gy: U32) -> M.P:
  +cenx = (hx * 16 + 8 : U32)
  +ceny = (hy * 16 + 8 : U32)
  +dir = walk_dir(v, in_map(hx, (hy - 1 : U32)), in_map(hx, (hy + 1 : U32)), in_map((hx - 1 : U32), hy), in_map((hx + 1 : U32), hy))
  +offy = (ceny - py : U32)
  +offx = (cenx - px : U32)
  ite(M.P, Bool.and(eq(hx, gx), eq(hy, gy)),
    u => M.P{Bool.pick(U32, U32.is_gt(sabs(offx), 2), sgn(offx), 0), Bool.pick(U32, U32.is_gt(sabs(offy), 2), sgn(offy), 0)},
    u => ite(M.P, Bool.and(eq(M.pt_x(dir), 0), eq(M.pt_y(dir), 0)),
      u => M.P{0, 0},
      u => ite(M.P, ne(M.pt_x(dir), 0),
        u => Bool.pick(M.P, U32.is_gt(sabs(offy), 3), M.P{0, sgn(offy)}, dir),
        u => Bool.pick(M.P, U32.is_gt(sabs(offx), 3), M.P{sgn(offx), 0}, dir))))

# What a bot at (px, py) presses to reach tile (gx, gy).
def walk_to(+s: M.Sess, +k: M.Kn, +gx: U32, +gy: U32, +con: U32, +cx: U32, +cy: U32) -> M.P:
  +gb = ground(s, eq(M.k_mask(k), 1))
  +px = (M.k_x(k) / 256 : U32)
  +py = (M.k_y(k) / 256 : U32)
  +hx = (px / 16 : U32)
  +hy = (py / 16 : U32)
  walk_to_go(read5(flood(gx, gy, gb, con, cx, cy), ix(in_map(hx, hy), hx, hy), ix(in_map(hx, (hy - 1 : U32)), hx, (hy - 1 : U32)), ix(in_map(hx, (hy + 1 : U32)), hx, (hy + 1 : U32)), ix(in_map((hx - 1 : U32), hy), (hx - 1 : U32), hy), ix(in_map((hx + 1 : U32), hy), (hx + 1 : U32), hy)), hx, hy, px, py, gx, gy)

# --- the bot's hands ------------------------------------------------------------------------------
def ip_zero() -> M.Ip:
  M.Ip{0, 0, 0, 0, 0, 0, 0, 0}

# Alternating presses: a key is down on one tick and up on the next.
def tap(+k: M.Kn, +b: U32) -> Bool:
  Bool.not(bit(M.k_keys(k), b))

def go_walk(+p: M.P) -> M.Ip:
  M.Ip{M.pt_x(p), M.pt_y(p), 0, 0, 0, 0, 0, 0}

def idle_spare(+ids: List<&2, U32>, +id: U32, +i: U32) -> U32:
  match ids:
    case Nil{}:
      0
    case Con{+h, +t}:
      Bool.pick(U32, eq(h, id), i, idle_spare(t, id, (i + 1 : U32)))

def ly_len(+l: List<&2, M.Ly>) -> U32:
  match l:
    case Nil{}:
      0
    case Con{_, +t}:
      (ly_len(t) + 1 : U32)

def ly_nth(+l: List<&2, M.Ly>, n: Nat) -> M.Ly:
  match l:
    case Nil{}:
      M.Ly{0, M.Tk{0, 0, 0, 0, 0, 0, 0, 0, 0}}
    case Con{+h, +t}:
      match n:
        case 0n:
          h
        case 1n+p:
          ly_nth(t, p)

def go(+s: M.Sess, +k: M.Kn, +gx: U32, +gy: U32, +con: U32, +cx: U32, +cy: U32, +reach: U32) -> M.Ip:
  ite(M.Ip, U32.is_le(manhattan((M.k_x(k) / 256 : U32), (M.k_y(k) / 256 : U32), (gx * 16 + 8 : U32), (gy * 16 + 8 : U32)), reach),
    u => ip_zero(),
    u => go_walk(walk_to(s, k, gx, gy, con, cx, cy)))

# A knight with nothing to do waits near the next thing to happen.
def idle(+s: M.Sess, +cx: M.Cx, +all: List<&2, M.Tk>, +k: M.Kn, +id: U32) -> M.Ip:
  +later = later_go(all, s, cx, Nil{})
  +ids = up_ids(M.s_seats(s))
  +spare = idle_spare(ids, id, 0)
  +n = ly_len(later)
  +pick = ly_nth(later, U32.to_nat((spare % U32.max(n, 1) : U32)))
  +pf = Bool.pick(M.Pf, eq(n, 0), M.Pf{0, 0, 0}, aim_px(s, cx, M.tk_ak(M.ly_t(pick)), M.tk_aa(M.ly_t(pick)), M.tk_ab(M.ly_t(pick)), M.ly_w(pick)))
  +sx = Bool.pick(U32, eq(M.pf_ok(pf), 1), M.pf_x(pf), 280)
  +sy = Bool.pick(U32, eq(M.pf_ok(pf), 1), M.pf_y(pf), 248)
  go(s, k, (sx / 16 : U32), (sy / 16 : U32), 0, 0, 0, 8)

# The south route: the crate into the ditch, then the seal to the door.
def south_lean(+k: M.Kn, +centre: U32, +px: U32) -> M.Ip:
  ite(M.Ip, U32.is_gt(sabs((px - centre : U32)), 2),
    u => M.Ip{sgn((centre - px : U32)), 0, 0, 0, 0, 0, 0, 0},
    u => Bool.pick(M.Ip, eq(M.k_mask(k), 2), M.Ip{0, 1, 0, 0, 0, 0, 0, 0}, ip_zero()))

def south_door(+s: M.Sess, +k: M.Kn) -> M.Ip:
  ite(M.Ip, U32.is_gt(manhattan((M.k_x(k) / 256 : U32), (M.k_y(k) / 256 : U32), 280, 392), 8),
    u => go(s, k, 17, 24, 0, 0, 0, 6),
    u => M.Ip{0, 0, bu(tap(k, key_use())), 0, 0, 0, 0, 0})

def south_crate(+s: M.Sess, +k: M.Kn) -> M.Ip:
  +cx = M.s_crate_x(s)
  +cy = M.s_crate_y(s)
  +px = (M.k_x(k) / 256 : U32)
  +py = (M.k_y(k) / 256 : U32)
  +hx = (px / 16 : U32)
  +hy = (py / 16 : U32)
  ite(M.Ip, Bool.or(Bool.and(eq(hx, cx), eq(hy, (cy - 1 : U32))), Bool.and(Bool.and(eq(hx, cx), U32.is_lt(hy, cy)), U32.is_le((cy - hy : U32), 1))),
    u => south_lean(k, (cx * 16 + 8 : U32), px),
    u => go(s, k, cx, (cy - 1 : U32), 1, cx, cy, 3))

def south(+s: M.Sess, +k: M.Kn) -> M.Ip:
  ite(M.Ip, has_flag(s, 2),
    u => ip_zero(),
    u => ite(M.Ip, Bool.and(Bool.not(has_flag(s, 8)), eq(M.s_crate_on(s), 1)),
      u => south_crate(s, k),
      u => south_door(s, k)))

def do_tap(+k: M.Kn, +act: U32) -> M.Ip:
  Bool.pick(M.Ip, eq(act, 0), M.Ip{0, 0, bu(tap(k, key_use())), 0, 0, 0, 0, 0}, Bool.pick(M.Ip, eq(act, 1), M.Ip{0, 0, 1, 0, 0, 0, 0, 0}, ip_zero()))

def do_task_at(+s: M.Sess, +cx: M.Cx, +all: List<&2, M.Tk>, +k: M.Kn, +t: M.Tk, +tgt: M.Pf) -> M.Ip:
  +px = (M.k_x(k) / 256 : U32)
  +py = (M.k_y(k) / 256 : U32)
  +near = manhattan(px, py, M.pf_x(tgt), M.pf_y(tgt))
  +gx = (M.pf_x(tgt) / 16 : U32)
  +gy = (M.pf_y(tgt) / 16 : U32)
  +con = M.s_crate_on(s)
  +ccx = M.s_crate_x(s)
  +ccy = M.s_crate_y(s)
  +reach = Bool.pick(U32, eq(M.tk_ak(t), 0), 18, 5)
  ite(M.Ip, eq(M.tk_act(t), 3),
    u => south(s, k),
    u => ite(M.Ip, Bool.not(doable(s, cx, t)),
      u => go(s, k, gx, gy, con, ccx, ccy, 6),
      u => ite(M.Ip, U32.is_gt(near, reach),
        u => go(s, k, gx, gy, con, ccx, ccy, U32.min(reach, 5)),
        u => do_tap(k, M.tk_act(t)))))

def do_task(+s: M.Sess, +cx: M.Cx, +all: List<&2, M.Tk>, +k: M.Kn, +t: M.Tk) -> M.Ip:
  +tgt = aim_px(s, cx, M.tk_ak(t), M.tk_aa(t), M.tk_ab(t), U32.max(M.s_clock(s), M.tk_opens(t)))
  ite(M.Ip, eq(M.pf_ok(tgt), 0),
    u => idle(s, cx, all, k, 0),
    u => do_task_at(s, cx, all, k, t, tgt))

# A wisp close enough to touch: sidestep through it, or step away.
def near_wisp_m(+ws: List<&2, M.Wp>, +mx: U32, +my: U32, +best: M.Lt) -> M.Lt:
  match ws:
    case Nil{}:
      best
    case Con{+w, +t}:
      +wx = sdiv(M.w_x(w), 256)
      +wy = sdiv(M.w_y(w), 256)
      +d = manhattan(wx, wy, mx, my)
      near_wisp_m(t, mx, my, Bool.pick(M.Lt, Bool.and(U32.is_lt(d, 16), Bool.or(eq(M.lt_id(best), 0), U32.is_lt(d, M.lt_d2(best)))), M.Lt{1, wx, wy, d}, best))

def danger(+s: M.Sess, +k: M.Kn) -> M.Dg:
  +mx = (M.k_x(k) / 256 : U32)
  +my = (M.k_y(k) / 256 : U32)
  +nw = near_wisp_m(M.s_wisps(s), mx, my, M.Lt{0, 0, 0, 0})
  Bool.pick(M.Dg, Bool.and(Bool.and(Bool.and(eq(M.k_hurt(k), 0), eq(M.k_roll(k), 0)), Bool.and(eq(M.k_panel(k), 0), eq(M.k_down(k), 0))), eq(M.lt_id(nw), 1)), M.Dg{1, (mx - M.lt_x(nw) : U32), (my - M.lt_y(nw) : U32), bu(eq(M.k_roll_rest(k), 0))}, M.Dg{0, 0, 0, 0})

# Something is about to touch us: roll, in the way we were going or away.
def dodge(+i: M.Ip, +d: M.Dg) -> M.Ip:
  +moving = Bool.not(Bool.and(eq(M.ip_mx(i), 0), eq(M.ip_my(i), 0)))
  +horiz = U32.is_ge(sabs(M.dg_dx(d)), sabs(M.dg_dy(d)))
  ite(M.Ip, eq(M.dg_on(d), 1),
    u => M.Ip{Bool.pick(U32, moving, M.ip_mx(i), Bool.pick(U32, horiz, sgn(M.dg_dx(d)), 0)), Bool.pick(U32, moving, M.ip_my(i), Bool.pick(U32, horiz, 0, sgn(M.dg_dy(d)))), M.ip_interact(i), M.ip_ult(i), M.ip_swing(i), M.ip_play(i), M.ip_bomb(i), Bool.pick(U32, eq(M.dg_can(d), 1), 1, M.ip_dash(i))},
    u => i)

def as_find(+l: List<&2, M.As>, +id: U32) -> M.As:
  match l:
    case Nil{}:
      M.As{0, 0, M.Tk{0, 0, 0, 0, 0, 0, 0, 0, 0}}
    case Con{+a, +t}:
      Bool.pick(M.As, eq(M.as_id(a), id), a, as_find(t, id))

def as_any(+l: List<&2, M.As>) -> Bool:
  match l:
    case Nil{}:
      False{}
    case Con{+a, +t}:
      Bool.or(eq(M.as_has(a), 1), as_any(t))

def any_when(+all: List<&2, M.Tk>, +s: M.Sess, +cx: M.Cx) -> Bool:
  match all:
    case Nil{}:
      False{}
    case Con{+t, +rest}:
      Bool.or(eq(M.pf_ok(when_ok(s, cx, t)), 1), any_when(rest, s, cx))

def holds_wanted(+all: List<&2, M.Tk>, +item: U32) -> Bool:
  match all:
    case Nil{}:
      False{}
    case Con{+t, +rest}:
      Bool.or(Bool.and(eq(M.tk_hk(t), 1), eq(M.tk_hi(t), item)), holds_wanted(rest, item))

def bot_busy(+s: M.Sess, +cx: M.Cx, +all: List<&2, M.Tk>, +k: M.Kn, +id: U32, +mine: M.As) -> M.Ip:
  +task = Bool.and(eq(M.as_has(mine), 1), eq(M.tk_act(M.as_t(mine)), 3))
  +want_stout = Bool.and(task, Bool.not(has_flag(s, 8)))
  +want = Bool.pick(U32, Bool.and(want_stout, m_has_mask(s, 2)), 2, Bool.pick(U32, m_has_mask(s, 1), 1, 0))
  ite(M.Ip, Bool.and(ne(M.k_mask(k), want), tap(k, key_mask())),
    u => M.Ip{0, 0, 0, 0, 0, 1, 0, 0},
    u => ite(M.Ip, eq(M.k_say_on(k), 1),
      u => M.Ip{0, 0, bu(tap(k, key_use())), 0, 0, 0, 0, 0},
      u => ite(M.Ip, Bool.and(ne(M.k_held(k), 0), Bool.not(holds_wanted(all, M.k_held(k)))),
        u => M.Ip{0, 0, 0, 0, 0, 0, bu(tap(k, key_drop())), 0},
        u => dodge(ite(M.Ip, eq(M.as_has(mine), 1), u => do_task(s, cx, all, k, M.as_t(mine)), u => idle(s, cx, all, k, id)), danger(s, k)))))

# Nothing left that anyone can do this loop: ring the bell, together.
def bot_ring(+s: M.Sess, +k: M.Kn, +id: U32) -> M.Ip:
  ite(M.Ip, Bool.and(eq(M.s_call_on(s), 1), Bool.not(bit(M.s_call_ayes(s), pw((id - 1 : U32))))),
    u => M.Ip{0, 0, 0, bu(tap(k, key_bell())), 0, 0, 0, 0},
    u => Bool.pick(M.Ip, eq(M.s_call_on(s), 1), ip_zero(), M.Ip{0, 0, 0, bu(tap(k, key_bell())), 0, 0, 0, 0}))

def bot_plan(+s: M.Sess, +k: M.Kn, +id: U32) -> M.Ip:
  +cx = cx_new(s)
  +all = tasks(s, cx)
  +plan = assign(s, cx, all)
  ite(M.Ip, Bool.and(Bool.not(as_any(plan)), Bool.not(Bool.or(final_hours(s), any_when(all, s, cx)))),
    u => bot_ring(s, k, id),
    u => bot_busy(s, cx, all, k, id, as_find(plan, id)))

def bot_panel(+s: M.Sess, +k: M.Kn, +id: U32) -> M.Ip:
  +panel = M.k_panel(k)
  ite(M.Ip, eq(panel, 1),
    u => M.Ip{0, 0, 0, 0, bu(tap(k, key_ledger())), 0, 0, 0},
    u => ite(M.Ip, eq(panel, 2),
      u => Bool.pick(M.Ip, U32.is_gt(M.k_parg(k), 0), M.Ip{0, Bool.pick(U32, tap(k, key_up()), 4294967295, 0), 0, 0, 0, 0, 0, 0}, M.Ip{0, 0, bu(tap(k, key_use())), 0, 0, 0, 0, 0}),
      u => ite(M.Ip, eq(M.s_done_on(s), 1), u => ip_zero(), u => bot_plan(s, k, id))))

# What `id` presses this tick.
def main_bot(+s: M.Sess, +id: U32) -> M.Ip:
  ite(M.Ip, Bool.or(Bool.not(sl_has(M.s_seats(s), id)), Bool.or(ne(M.s_phase(s), 0), ne(M.k_down(get_k(s, id)), 0))),
    u => ip_zero(),
    u => bot_panel(s, get_k(s, id), id))
