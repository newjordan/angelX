# crates/delve/src/rules/combos.rs — wish families and the recipes that
# unlock a wish's super effects. A family set is the ascending list of the
# family indexes (Family::ALL order) a BTreeSet<Family> would hold.

# a sorted set with one more member
def combos.set_add(+xs: List<&2, U32>, +x: U32) -> List<&2, U32>:
  match xs:
    case Nil{}:
      [x]
    case Con{+h, +t}:
      home.util.ite(List<&2, U32>, U32.is_eq(h, x), u => xs, u => home.util.ite(List<&2, U32>, U32.is_lt(x, h), u => x <> (h <> t), u => h <> combos.set_add(t, x)))

def combos.set_has(+xs: List<&2, U32>, +x: U32) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(U32.is_eq(h, x), combos.set_has(t, x))

def combos.set_union(+a: List<&2, U32>, +b: List<&2, U32>) -> List<&2, U32>:
  match b:
    case Nil{}:
      a
    case Con{+h, +t}:
      combos.set_union(combos.set_add(a, h), t)

def combos.STAND_IN() -> U32:
  5

# A wish's card: one the phrasebook granted.
def combos.is_wish(+card: J.Val) -> Bool:
  String.starts_with(J.str(J.get(card, "id")), "wish-")

# The card word of a held effect's serde name, if it is a super effect.
def combos.effect_word(+name: String) -> Maybe<&2, String>:
  home.util.ite(Maybe<&2, String>, home.util.has_name(["homing", "chain", "burst", "pierce", "fork", "bounce", "orbit", "volley"], name), u => Some{name}, u => None{})

# the recipe of an effect word: [effect, name, family, family] or null
def combos.recipe(+effect: String, +rs: List<&2, List<&2, String>>) -> J.Val:
  match rs:
    case Nil{}:
      J.JNull{}
    case Con{+r, +t}:
      home.util.ite(J.Val, String.eq(J.str(J.JS{combos.nth_str(r, 0)}), effect), u => home.util.jstrs(r), u => combos.recipe(effect, t))

def combos.nth_str(+xs: List<&2, String>, +i: U32) -> String:
  match xs:
    case Nil{}:
      ""
    case Con{+h, +t}:
      home.util.ite(String, U32.is_eq(i, 0), u => h, u => combos.nth_str(t, (i - 1 : U32)))

def combos.num(+s: String) -> U32:
  home.util.or(home.util.parse_u32(s), 0)

# Whether a knight of `families` is given super effect `effect`.
def combos.unlocked(+effect: String, +families: List<&2, U32>) -> Bool:
  combos.unlocked.of(combos.recipe(effect, combos.RECIPES()), families)

def combos.unlocked.of(+r: J.Val, +families: List<&2, U32>) -> Bool:
  match r:
    case J.JNull{}:
      True{}
    case _:
      Bool.and(combos.set_has(families, combos.num(J.str(home.util.get(J.items(r), 2)))), combos.set_has(families, combos.num(J.str(home.util.get(J.items(r), 3)))))

# the words of a name/scroll as `said` splits them: a plural's `s` dropped
def combos.said(+text: String) -> List<&2, String>:
  combos.said.go(cards.words_of(home.util.lower(text)))

def combos.said.go(+ws: List<&2, String>) -> List<&2, String>:
  match ws:
    case Nil{}:
      Nil{}
    case Con{+w, +t}:
      home.util.ite(String, Bool.and(U32.is_gt(home.util.blen(w), 3), Bool.and(String.ends_with(w, "s"), Bool.not(String.ends_with(w, "ss")))), u => String.take(w, U32.to_nat((home.util.chars(w) - 1 : U32))), u => w) <> combos.said.go(t)

def combos.word_in(+w: String, +ws: List<&2, String>) -> Bool:
  home.util.has_name(ws, w)

def combos.any_word(+list: List<&2, String>, +said: List<&2, String>) -> Bool:
  match list:
    case Nil{}:
      False{}
    case Con{+w, +t}:
      Bool.or(home.util.has_name(said, w), combos.any_word(t, said))

# the families named by the words, ascending
def combos.by_words(+all: List<&2, List<&2, String>>, +said: List<&2, String>, +i: U32, +acc: List<&2, U32>) -> List<&2, U32>:
  match all:
    case Nil{}:
      acc
    case Con{+ws, +t}:
      combos.by_words(t, said, (i + 1 : U32), home.util.ite(List<&2, U32>, combos.any_word(ws, said), u => combos.set_add(acc, i), u => acc))

# the family one effect (by serde name) belongs to, 99 for none
def combos.effect_family(+name: String) -> U32:
  home.util.ite(U32, home.util.has_name(["shots", "size", "velocity", "knock", "boomerang"], name), u => 1,
  u => home.util.ite(U32, String.eq(name, "burn"), u => 2,
  u => home.util.ite(U32, String.eq(name, "chill"), u => 3,
  u => home.util.ite(U32, String.eq(name, "chain"), u => 4,
  u => home.util.ite(U32, home.util.has_name(["poison", "fart"], name), u => 5,
  u => home.util.ite(U32, home.util.has_name(["armor", "max_hp"], name), u => 6,
  u => home.util.ite(U32, home.util.has_name(["speed", "rate"], name), u => 7,
  u => home.util.ite(U32, home.util.has_name(["vamp", "mend"], name), u => 8,
  u => home.util.ite(U32, home.util.has_name(["summon", "brownie", "moth", "wisp", "mite", "ash_sprite", "mote", "newt", "linnet", "leech", "ghoul", "warden", "totem"], name), u => 0, u => 99)))))))))

def combos.by_effects(+es: List<&2, J.Val>, +acc: List<&2, U32>) -> List<&2, U32>:
  match es:
    case Nil{}:
      acc
    case Con{+e, +t}:
      combos.by_effects(t, combos.add_family(acc, combos.effect_family(J.str(home.util.get(home.util.keys_of(e), 0)))))

def combos.add_family(+acc: List<&2, U32>, +f: U32) -> List<&2, U32>:
  home.util.ite(List<&2, U32>, U32.is_eq(f, 99), u => acc, u => combos.set_add(acc, f))

# The families one wish's card is of.
def combos.families(+card: J.Val) -> List<&2, U32>:
  combos.families.looks(card, combos.by_effects(J.items(J.get(card, "effects")), combos.by_words(combos.WORDS(), combos.said(J.str(J.get(card, "name")) ++ " " ++ J.str(J.get(card, "text"))), 0, Nil{})))

def combos.families.looks(+card: J.Val, +acc: List<&2, U32>) -> List<&2, U32>:
  combos.add_family(
    combos.add_family(
      combos.add_family(acc, home.util.ite(U32, J.has(J.get(card, "looks"), "summon"), u => 0, u => 99)),
      home.util.ite(U32, J.has(J.get(card, "looks"), "worn"), u => 6, u => 99)),
    home.util.ite(U32, Bool.and(J.has(J.get(card, "looks"), "shot"), String.eq(J.str(J.get(card, "kind")), "hold")), u => 1, u => 99))

# The families of every wish among `held`.
def combos.held_families(+held: List<&2, J.Val>) -> List<&2, U32>:
  combos.held_families.go(held, Nil{})

def combos.held_families.go(+held: List<&2, J.Val>, +acc: List<&2, U32>) -> List<&2, U32>:
  match held:
    case Nil{}:
      acc
    case Con{+c, +t}:
      combos.held_families.go(t, home.util.ite(List<&2, U32>, combos.is_wish(c), u => combos.set_union(acc, combos.families(c)), u => acc))

# The recipes `families` make: effect words in recipe order.
def combos.made(+families: List<&2, U32>) -> List<&2, String>:
  combos.made.go(combos.RECIPES(), families)

def combos.made.go(+rs: List<&2, List<&2, String>>, +families: List<&2, U32>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case Con{+r, +t}:
      home.util.ite(List<&2, String>, Bool.and(combos.set_has(families, combos.num(combos.nth_str(r, 2))), combos.set_has(families, combos.num(combos.nth_str(r, 3)))), u => combos.nth_str(r, 0) <> combos.made.go(t, families), u => combos.made.go(t, families))

# A short list: "Creature, Projectile".
def combos.list(+families: List<&2, U32>) -> String:
  home.util.join(combos.list.names(families), ", ")

def combos.list.names(+families: List<&2, U32>) -> List<&2, String>:
  match families:
    case Nil{}:
      Nil{}
    case Con{+f, +t}:
      combos.nth_str(combos.NAMES(), f) <> combos.list.names(t)

# the family set as a J array of indexes (for cases)
def combos.set_json(+xs: List<&2, U32>) -> J.Val:
  J.JA{combos.set_json.go(xs)}

def combos.set_json.go(+xs: List<&2, U32>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      J.JU{h} <> combos.set_json.go(t)

def combos.set_of(+a: J.Val) -> List<&2, U32>:
  combos.set_of.go(J.items(a))

def combos.set_of.go(+xs: List<&2, J.Val>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      J.u32(h) <> combos.set_of.go(t)

def case.combos.held_families(+input: J.Val) -> J.Val:
  combos.set_json(combos.held_families(J.items(J.get(input, "held"))))

def case.combos.families(+input: J.Val) -> J.Val:
  combos.set_json(combos.families(J.get(input, "card")))

def case.combos.unlocked(+input: J.Val) -> J.Val:
  J.JBool{combos.unlocked(J.str(J.get(input, "effect")), combos.set_of(J.get(input, "families")))}
