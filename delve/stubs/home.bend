# Names the home group calls that other groups own (delve/GROUPS.md). Each
# answers a harmless refusal until its group's fragment defines the name; the
# build drops a stub the moment a fragment defines it. Conventions the home
# group assumed when it wrote the calls (a `Result<T, E>` is a J
# {"Ok": t} / {"Err": e}; a sprite, a weapon, a Looks are their serde J):

# looks.rs
def looks.Sprite.from_strip(+slot: String, +rows: List<&2, String>) -> J.Val:
  home.util.err("looks are not ported yet")

def looks.Sprite.to_block(+sprite: J.Val, +slot: String) -> String:
  ""

def looks.place_on_body(+frames: J.Val, +place: String) -> J.Val:
  home.util.err("looks are not ported yet")

def looks.place(+s: String) -> Maybe<&2, String>:
  None{}

def looks.paint_name(+s: String) -> Bool:
  False{}

def looks.Move.parse_list(+s: String) -> J.Val:
  home.util.err("looks are not ported yet")

def looks.default_worn_part(+name: String) -> String:
  "whole"

# artistry.rs: fit_worn(&mut sprite, at) answers the fitted sprite or why not
def artistry.fit_worn(+sprite: J.Val, +at: String) -> J.Val:
  home.util.ok(sprite)

# realm/forge.rs
def forge.valid_look(+colour: String, +shape: String) -> Bool:
  Bool.and(home.util.has_name(forge.COLOURS(), colour), home.util.has_name(forge.SHAPES(), shape))

def forge.COLOURS() -> List<&2, String>:
  ["ember", "ice", "gold", "venom", "void", "rose", "steel"]

def forge.SHAPES() -> List<&2, String>:
  ["orb", "shard", "arrow", "star", "blade", "wave"]

# forge::check(&file, START_LOC): {"Ok": {"weapon": w, "notes": [..]}} or {"Err": [errors]}
def forge.check(+file: String) -> J.Val:
  home.util.err("the forge is not ported yet")

# ults.rs: parse answers the Ult as serde writes it, or null
def ults.Ult.parse(+rest: String) -> J.Val:
  J.JNull{}

def ults.Ult.words() -> String:
  ""

def ults.Ult.word(+ult: J.Val) -> String:
  J.str(ult)

# layout.rs: Pack::name(self) for a serde pack name
def layout.Pack.name(+pack: String) -> String:
  pack

# phrasebook.rs (core): Wish::card(tier) answers {"Ok": card} / {"Err": text};
# read_checked(text) answers [wishes, errors] (the Rust tuple as an array)
def phrasebook.Wish.card(+wish: J.Val, +tier: U32) -> J.Val:
  home.util.err("the phrasebook is not ported yet")

def phrasebook.Wish.definition_file(+wish: J.Val) -> String:
  ""

def phrasebook.read_checked(+text: String) -> J.Val:
  J.JA{[J.JA{Nil{}}, J.JA{Nil{}}]}

# layout.rs: Pack::kin(self) for a serde pack name
def layout.Pack.kin(+pack: String) -> String:
  pack

# loot.rs (combat): the generator's weighted pick answers the generator and
# the chosen index
def loot.Rng.pick(+r: W.W, +weights: List<&2, U32>) -> W.W & U32:
  (r, 0)

# rules.rs / hero.rs (core): the seat's knight, and whether it is away
def core.hero(+run: J.Val, +id: U32) -> J.Val:
  J.get(J.get(run, "players"), U32.show(id))

def rules.Run.away(+run: J.Val, +id: U32) -> Bool:
  rules.Run.away.go(J.get(J.get(run, "players"), U32.show(id)), J.get(J.get(run, "idle"), U32.show(id)))

def rules.Run.away.go(+hero: J.Val, +idle: J.Val) -> Bool:
  Bool.or(J.bool(J.get(hero, "away")), Bool.and(Bool.not(J.is_null(idle)), U32.is_ge(J.u32(idle), rules.AWAY_TICKS())))

# core/u.bend, as it stands in the core group (dropped once its fragment defines the names)
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

# core/run.bend
def core.set_hero(+run: J.Val, +id: U32, +hero: J.Val) -> J.Val:
  J.set(run, "players", J.set(J.get(run, "players"), U32.show(id), hero))

def core.phase_is(+run: J.Val, +name: String) -> Bool:
  String.eq(J.str(J.get(run, "phase")), name)

# rules.rs (core): unit and hit_segment
def rules.unit(+x: F32, +y: F32) -> F32 & F32:
  rules.unit.go(RS.f_hypot(x, y), x, y)

def rules.unit.go(+l: F32, +x: F32, +y: F32) -> F32 & F32:
  rules.unit.if(F32.is_gt(l, 0.0), l, x, y)

def rules.unit.if(pos: Bool, +l: F32, +x: F32, +y: F32) -> F32 & F32:
  match pos:
    case True{}:
      (F32.div(x, l), F32.div(y, l))
    case False{}:
      (0.0, 0.0)

def rules.hit_segment(+ax: F32, +ay: F32, +bx: F32, +by: F32, +x: F32, +y: F32, +radius: F32) -> Bool:
  F32.is_le(RS.f_hypot(F32.sub(F32.sub(x, ax), F32.mul(rules.hit_segment.t(ax, ay, bx, by, x, y), F32.sub(bx, ax))), F32.sub(F32.sub(y, ay), F32.mul(rules.hit_segment.t(ax, ay, bx, by, x, y), F32.sub(by, ay)))), radius)

def rules.hit_segment.t(+ax: F32, +ay: F32, +bx: F32, +by: F32, +x: F32, +y: F32) -> F32:
  rules.hit_segment.t.go(F32.add(F32.mul(F32.sub(bx, ax), F32.sub(bx, ax)), F32.mul(F32.sub(by, ay), F32.sub(by, ay))), ax, ay, bx, by, x, y)

def rules.hit_segment.t.go(+length: F32, +ax: F32, +ay: F32, +bx: F32, +by: F32, +x: F32, +y: F32) -> F32:
  Bool.pick(F32, F32.is_gt(length, 0.0), F32.max(0.0, F32.min(F32.div(F32.add(F32.mul(F32.sub(x, ax), F32.sub(bx, ax)), F32.mul(F32.sub(y, ay), F32.sub(by, ay))), length), 1.0)), 0.0)

# hero.rs (core): Hero::scaled(damage)
def hero.Hero.scaled(+h: J.Val, +damage: U32) -> U32:
  damage

def rules.AWAY_TICKS() -> U32:
  600

# hero.rs (core): the share of a hit `armor` pieces of mail turn aside
def hero.armor_turns(+armor: U32) -> U32:
  Bool.pick(U32, U32.is_eq(U32.min(armor, 2), 0), 0, Bool.pick(U32, U32.is_eq(U32.min(armor, 2), 1), 6, 11))

# layout.rs (world): the tile's side in arena units
def layout.TILE_UNITS() -> F32:
  2.0

# rules.rs (core): Input::valid; monsters.rs (combat): EnemyKind::bounty
def rules.Input.valid(+i: J.Val) -> Bool:
  True{}

def monsters.EnemyKind.bounty(+k: String) -> U32:
  25

# rules.rs / swamp.rs (core): notice(what) and the horse freed
def rules.Run.notice(+run: J.Val, +what: String) -> J.Val:
  run

def swamp.Run.horse_freed(+run: J.Val, +by: U32) -> J.Val:
  run

def swamp.HORSE() -> String:
  "white-horse"

def swamp.PEN_HELD() -> String:
  "THE MIRE KING HOLDS THE PEN."
