# crates/delve/src/rules/griffinwing.rs — the session: its consts, the five
# numbers, the small mutators every part calls (sound, say, spark, next_id),
# the table and the formation. The tick (`step`) and its transitions are in
# step.bend; the Run's hooks in run.bend.

def griffinwing.SLOT() -> U32:
  3

# A run never runs past an hour; a longer one is not a run to play.
def griffinwing.LIMIT() -> U32:
  (60 * 60 * rules.HZ() : U32)

# A fall puts the beat back this many ticks after the last knight goes down.
def griffinwing.RETRY() -> U32:
  24

def griffinwing.DYING() -> U32:
  60

def griffinwing.VICTORY() -> U32:
  210

def griffinwing.TALLY_HOLD() -> U32:
  45

# A knight who falls rises again this many ticks later, at the formation.
def griffinwing.RISE() -> U32:
  75

def griffinwing.PURSE() -> U32:
  10

def griffinwing.PURSE_MEDAL() -> U32:
  5

def griffinwing.REPLAY() -> U32:
  5

def griffinwing.REPLAY_MEDAL() -> U32:
  2

def griffinwing.BEGUN() -> U32:
  1

def griffinwing.GOLD() -> U32:
  2

def griffinwing.CAMPAIGN_MARK() -> String:
  "griffinwing:campaign-begun"

def griffinwing.GOLD_MARK() -> String:
  "griffinwing:gold-medal"

# A seat's bit.
def griffinwing.bit(+id: U32) -> U32:
  U32.shln(1, U32.to_nat((RS.u_clamp(id, 1, 8) - 1 : U32)))

# ─── the five numbers ───────────────────────────────────────────────────────

# `Nums::default()`: where the knobs start.
def griffinwing.Nums.default() -> J.Val:
  griffinwing.Nums.make(40, 30, 64, 75, 50)

def griffinwing.Nums.make(+charge: U32, +roll: U32, +window: U32, +spur: U32, +tell: U32) -> J.Val:
  +n0 = J.obj()
  +n1 = ga.su(n0, "charge", charge)
  +n2 = ga.su(n1, "roll", roll)
  +n3 = ga.su(n2, "window", window)
  +n4 = ga.su(n3, "spur", spur)
  ga.su(n4, "tell", tell)

# `Nums::of(&tune)`: `Tune::roundtable[SLOT]`, each clamped to its bounds.
def griffinwing.Nums.of(+tune: J.Val) -> J.Val:
  griffinwing.Nums.of.row(J.nth(J.get(tune, "roundtable"), griffinwing.SLOT()))

def griffinwing.Nums.of.row(+row: J.Val) -> J.Val:
  griffinwing.Nums.make(
    RS.u_clamp(ga.nu(row, 0), 20, 90),
    RS.u_clamp(ga.nu(row, 1), 18, 60),
    RS.u_clamp(ga.nu(row, 2), 40, 90),
    RS.u_clamp(ga.nu(row, 3), 30, 180),
    RS.u_clamp(ga.nu(row, 4), 40, 120))

# The steer box's half width and half height.
def griffinwing.Nums.wx(+n: J.Val) -> U32:
  griffinwing.types.dm(ga.u(n, "window"))

def griffinwing.Nums.wz(+n: J.Val) -> U32:
  RS.i_div((griffinwing.types.dm(ga.u(n, "window")) * 5 : U32), 8)

# How much of a roll reflects: all but its last ten ticks.
def griffinwing.Nums.reflect(+n: J.Val) -> U32:
  U32.max(ga.ssub(ga.u(n, "roll"), 10), 8)

# ─── the session ────────────────────────────────────────────────────────────

# The rail's point the formation flies at now.
def griffinwing.Session.anchor(+s: J.Val) -> U32:
  (griffinwing.rail.ORIGIN(U32.min(ga.u(s, "stage"), 2)) + (ga.u(s, "t") * griffinwing.types.PACE() : U32) : U32)

# How many knights fly: the pressure on the foes.
def griffinwing.Session.pressure(+s: J.Val) -> U32:
  RS.u_clamp(gw.nseats(s), 1, rules.MAX_PLAYERS())

# A fresh id for a foe.
def griffinwing.Session.next_id(+s: J.Val) -> J.Val:
  gw.out(
    ga.su(s, "ids", Bool.pick(U32, U32.is_ge(ga.u(s, "ids"), 60000), 1, (ga.u(s, "ids") + 1 : U32))),
    J.JU{ga.u(s, "ids")})

# `snd.contains(name)`.
def gw.has_str(+xs: List<&2, J.Val>, +name: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(String.eq(J.str(h), name), gw.has_str(t, name))

def griffinwing.Session.sound(+s: J.Val, +name: String) -> J.Val:
  gw.ifj(Bool.and(U32.is_lt(gw.len(s, "snd"), 12), Bool.not(gw.has_str(ga.items(s, "snd"), name))),
    _ => ga.push(s, "snd", J.JS{name}),
    _ => s)

# `calls.iter().any(|c| c.line == line && c.ticks > 40)`.
def griffinwing.Session.said(+cs: List<&2, J.Val>, +line: String) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case Con{+c, +t}:
      Bool.or(Bool.and(String.eq(ga.s(c, "line"), line), U32.is_gt(ga.u(c, "ticks"), 40)), griffinwing.Session.said(t, line))

# A rider's short shout, shown as a portrait for a moment. The same line is
# not said twice in a row, and four show at most.
def griffinwing.Session.say(+s: J.Val, +who: U32, +line: String) -> J.Val:
  gw.ifj(griffinwing.Session.said(ga.items(s, "calls"), line),
    _ => s,
    _ => griffinwing.Session.say.push(s, who, line))

def griffinwing.Session.say.push(+s: J.Val, +who: U32, +line: String) -> J.Val:
  ga.push(
    gw.ifj(U32.is_ge(gw.len(s, "calls"), griffinwing.types.MAX_CALLS()), _ => gw.remove(s, "calls", 0), _ => s),
    "calls", griffinwing.Session.Call.make(who, line, 100))

def griffinwing.Session.Call.make(+who: U32, +line: String, +ticks: U32) -> J.Val:
  +c0 = J.obj()
  +c1 = ga.su(c0, "who", who)
  +c2 = ga.ss(c1, "line", line)
  ga.su(c2, "ticks", ticks)

def griffinwing.Session.spark(+s: J.Val, +what: String, +at: U32, +x: U32, +z: U32, +n: U32) -> J.Val:
  griffinwing.Session.spark.push(
    gw.ifj(U32.is_ge(gw.len(s, "fx"), griffinwing.types.MAX_FX()), _ => gw.remove(s, "fx", 0), _ => s),
    what, at, x, z, n)

def griffinwing.Session.spark.push(+s: J.Val, +what: String, +at: U32, +x: U32, +z: U32, +n: U32) -> J.Val:
  ga.push(s, "fx", griffinwing.Session.Flare.make(what, at, x, z, griffinwing.Session.spark.ticks(what), n))

def griffinwing.Session.spark.ticks(+what: String) -> U32:
  Bool.pick(U32, String.eq(what, "Starfall"), 36,
    Bool.pick(U32, String.eq(what, "Fall"), 40,
      Bool.pick(U32, String.eq(what, "Lock"), 8, 14)))

def griffinwing.Session.Flare.make(+what: String, +at: U32, +x: U32, +z: U32, +ticks: U32, +n: U32) -> J.Val:
  +f0 = J.obj()
  +f1 = ga.ss(f0, "what", what)
  +f2 = ga.si(f1, "s", at)
  +f3 = ga.si(f2, "x", x)
  +f4 = ga.si(f3, "z", z)
  +f5 = ga.su(f4, "ticks", ticks)
  ga.su(f5, "n", n)

# Who calls the hints: the first of the riders still flying, and when a full
# table leaves none (or all are lost), Aster, the wing's lead.
def griffinwing.Session.voice(+s: J.Val) -> U32:
  griffinwing.Session.voice.go(ga.items(s, "wing"))

def griffinwing.Session.voice.go(+ws: List<&2, J.Val>) -> U32:
  match ws:
    case Nil{}:
      7
    case Con{+w, +t}:
      Bool.pick(U32, ga.b(w, "lost"), griffinwing.Session.voice.go(t), ga.u(w, "id"))

# Fill the wing up to four with riders: the seats first, riders in the seats
# nobody sits in.
def griffinwing.Session.fill_wing(+s: J.Val) -> J.Val:
  griffinwing.Session.fill_wing.add(
    griffinwing.Session.fill_wing.pop(s, ga.ssub(griffinwing.types.WING(), gw.nseats(s))),
    ga.ssub(griffinwing.types.WING(), gw.nseats(s)))

# while wing.len() > want { wing.pop() }
def griffinwing.Session.fill_wing.pop(+s: J.Val, +want: U32) -> J.Val:
  gw.ifj(U32.is_gt(gw.len(s, "wing"), want),
    _ => griffinwing.Session.fill_wing.pop(gw.trunc(s, "wing", (gw.len(s, "wing") - 1 : U32)), want),
    _ => s)

# while wing.len() < want { push a rider }
def griffinwing.Session.fill_wing.add(+s: J.Val, +want: U32) -> J.Val:
  gw.ifj(U32.is_lt(gw.len(s, "wing"), want),
    _ => griffinwing.Session.fill_wing.add(ga.push(s, "wing", griffinwing.Session.Wingman.make(gw.len(s, "wing"))), want),
    _ => s)

def griffinwing.Session.Wingman.make(+k: U32) -> J.Val:
  griffinwing.Session.Wingman.of(k, griffinwing.flight.rider_slot.x(k), griffinwing.flight.rider_slot.z(k), griffinwing.flight.rider_slot.back(k))

def griffinwing.Session.Wingman.of(+k: U32, +x: U32, +z: U32, +back: U32) -> J.Val:
  +w0 = J.obj()
  +w1 = ga.su(w0, "id", (7 + k : U32))
  +w2 = ga.sv(w1, "craft", ga.si(griffinwing.types.Craft.new(x, z), "back", back))
  +w3 = ga.sb(w2, "lost", False{})
  ga.su(w3, "fire", (12 * k : U32))

# Seat a knight at the formation, unhurt for a moment.
def griffinwing.Session.sit(+s: J.Val, +id: U32, +joined: U32) -> J.Val:
  gw.ifj(Bool.or(Bool.not(U32.is_eq(RS.u_clamp(id, 1, rules.MAX_PLAYERS()), id)), ga.has_key(J.get(s, "seats"), id)),
    _ => s,
    _ => griffinwing.Session.sit.in(s, id, joined))

def griffinwing.Session.sit.in(+s: J.Val, +id: U32, +joined: U32) -> J.Val:
  ga.sv(s, "seats", ga.put_at(J.get(s, "seats"), id,
    griffinwing.Session.Seat.make(joined,
      ga.su(griffinwing.types.Craft.new(griffinwing.flight.slot.x(id), griffinwing.flight.slot.z(id)), "hurt", (2 * rules.HZ() : U32)))))

def griffinwing.Session.Seat.make(+joined: U32, +craft: J.Val) -> J.Val:
  ga.sv(ga.su(J.obj(), "joined", joined), "craft", craft)

# The kingdom tile its door stands on: walk up and press E.
def griffinwing.ENTRY() -> J.Val:
  ga.pair_i(105, 9)

# Every mark this adventure may write.
def griffinwing.MARKS() -> J.Val:
  J.JA{[J.JS{griffinwing.CAMPAIGN_MARK()}, J.JS{griffinwing.GOLD_MARK()}]}

# The five numbers (charge, roll, window, spur, tell) as the knob panel shows
# them: {name, label, constant, min, max, start}.
def griffinwing.NUMBERS() -> J.Val:
  J.JA{[
    griffinwing.NUMBERS.row("griffinwing_lock", "Charge ticks", "griffinwing::NUMBERS[0]", 20, 90, 40),
    griffinwing.NUMBERS.row("griffinwing_roll", "Roll ticks", "griffinwing::NUMBERS[1]", 18, 60, 30),
    griffinwing.NUMBERS.row("griffinwing_window", "Steer box", "griffinwing::NUMBERS[2]", 40, 90, 64),
    griffinwing.NUMBERS.row("griffinwing_spur", "Spur ticks", "griffinwing::NUMBERS[3]", 30, 180, 75),
    griffinwing.NUMBERS.row("griffinwing_tell", "Tell ticks", "griffinwing::NUMBERS[4]", 40, 120, 50)]}

def griffinwing.NUMBERS.row(+name: String, +label: String, +constant: String, +min: U32, +max: U32, +start: U32) -> J.Val:
  +n0 = ga.ss(J.obj(), "name", name)
  +n1 = ga.ss(n0, "label", label)
  +n2 = ga.ss(n1, "constant", constant)
  +n3 = ga.su(n2, "min", min)
  +n4 = ga.su(n3, "max", max)
  ga.su(n4, "start", start)
