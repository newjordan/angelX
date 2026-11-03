# crates/delve/src/rules/cauldron.rs — the pot's checks on a brewed spell
# (`faults` and what it measures with), as the delve.brew policy runs them.

def cauldron.POWER_BUDGET() -> F32:
  29.5

# `power`: what a spell does at once over its recharge
def cauldron.power(+card: J.Val) -> F32:
  F32.div(cauldron.points(J.items(J.get(card, "effects")), 0.0), U32.to_f32(U32.max(J.u32(J.get(card, "cooldown")), 1)))

def cauldron.points(+es: List<&2, J.Val>, +acc: F32) -> F32:
  match es:
    case Nil{}:
      acc
    case Con{+e, +t}:
      cauldron.points(t, F32.add(acc, cauldron.points.of(J.fields(e))))

def cauldron.points.of(+fs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> F32:
  match fs:
    case Con{(+name, +v), +t}:
      cauldron.points.named(name, v)
    case Nil{}:
      0.0

def cauldron.points.named(+name: String, +v: J.Val) -> F32:
  home.util.ite(F32, String.eq(name, "nova"), u => F32.mul(3.0, U32.to_f32(J.u32(v))),
  u => home.util.ite(F32, String.eq(name, "cabbages"), u => F32.mul(34.0, U32.to_f32(J.u32(v))),
  u => home.util.ite(F32, String.eq(name, "cast"), u => cauldron.points.cast(J.str(v)),
  u => home.util.ite(F32, String.eq(name, "heal"), u => F32.mul(2.0, U32.to_f32(J.u32(v))),
  u => home.util.ite(F32, String.eq(name, "ward"), u => F32.mul(25.0, U32.to_f32(J.u32(v))),
  u => home.util.ite(F32, String.eq(name, "bombs"), u => F32.mul(40.0, U32.to_f32(J.u32(v))),
  u => home.util.ite(F32, Bool.or(String.eq(name, "hedge"), String.eq(name, "imposture")), u => 90.0,
  u => home.util.ite(F32, String.eq(name, "summon"), u => cauldron.points.summon(J.u32(home.util.get(J.items(v), 0)), J.str(home.util.get(J.items(v), 1))),
  u => 0.0))))))))

def cauldron.points.cast(+shape: String) -> F32:
  Bool.pick(F32, String.eq(shape, "bolt"), 32.0, 96.0)

def cauldron.points.summon(+n: U32, +b: String) -> F32:
  F32.mul(U32.to_f32(n), Bool.pick(F32, String.eq(b, "swarm"), 30.0, Bool.pick(F32, String.eq(b, "guard"), 25.0, 80.0)))

# `effect_mix`: what a spell does in words, without its numbers, sorted
def cauldron.effect_mix(+card: J.Val) -> List<&2, String>:
  home.util.dedup_str(home.util.sort_str(cauldron.mix_words(J.items(J.get(card, "effects")))))

def cauldron.mix_words(+es: List<&2, J.Val>) -> List<&2, String>:
  match es:
    case Nil{}:
      Nil{}
    case Con{+e, +t}:
      cauldron.mix_word(J.fields(e)) <> cauldron.mix_words(t)

def cauldron.mix_word(+fs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> String:
  match fs:
    case Con{(+name, +v), +t}:
      home.util.ite(String, String.eq(name, "cast"), u => "cast " ++ J.str(v),
        u => home.util.ite(String, String.eq(name, "summon"), u => "summon " ++ J.str(home.util.get(J.items(v), 1)), u => cards.word_of_name(name)))
    case Nil{}:
      ""

# `bare`: a name as a brew compares it
def cauldron.bare(+name: String) -> String:
  cauldron.bare.trim(cauldron.bare.words(home.util.words(name)))

def cauldron.bare.words(+ws: List<&2, String>) -> List<&2, String>:
  match ws:
    case Nil{}:
      Nil{}
    case Con{+w, +t}:
      cauldron.bare.keep(home.util.lower(cauldron.bare.alnum(w)), t)

def cauldron.bare.alnum(+w: String) -> String:
  String.from_list(cauldron.bare.alnum.go(String.to_list(w)))

def cauldron.bare.alnum.go(+cs: List<&2, Char>) -> List<&2, Char>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      home.util.ite(List<&2, Char>, home.util.c_alnum_u(Char.to_u32(h)), u => h <> cauldron.bare.alnum.go(t), u => cauldron.bare.alnum.go(t))

def cauldron.bare.keep(+w: String, +t: List<&2, String>) -> List<&2, String>:
  home.util.ite(List<&2, String>, String.is_empty(w), u => cauldron.bare.words(t), u => w <> cauldron.bare.words(t))

# drop a leading potent/perfect and a trailing tier numeral, then join
def cauldron.bare.trim(+ws: List<&2, String>) -> String:
  String.concat(cauldron.bare.last(cauldron.bare.first(ws)))

def cauldron.bare.first(+ws: List<&2, String>) -> List<&2, String>:
  match ws:
    case Con{+f, +rest}:
      home.util.ite(List<&2, String>, Bool.or(String.eq(f, "potent"), String.eq(f, "perfect")), u => rest, u => f <> rest)
    case Nil{}:
      Nil{}

def cauldron.bare.last(+ws: List<&2, String>) -> List<&2, String>:
  cauldron.bare.last.of(List.reverse(&2, String, ws))

def cauldron.bare.last.of(+rev: List<&2, String>) -> List<&2, String>:
  match rev:
    case Con{+l, +rest}:
      List.reverse(&2, String, home.util.ite(List<&2, String>, home.util.has_name(["i", "ii", "iii", "iv", "v"], l), u => rest, u => l <> rest))
    case Nil{}:
      Nil{}

def cauldron.CREATURES() -> List<&2, String>:
  ["cat", "kitten", "kitty", "dog", "puppy", "pup", "hound", "pet", "crab", "rat", "mouse", "bird", "owl", "frog", "toad", "snake", "bee", "wasp", "spider", "wolf", "fox", "beast", "critter", "monkey", "pig", "goose", "chicken", "duck", "bat", "lizard", "hamster", "ferret"]

def cauldron.has_effect(+card: J.Val, +name: String) -> Bool:
  cauldron.has_effect.go(J.items(J.get(card, "effects")), name)

def cauldron.has_effect.go(+es: List<&2, J.Val>, +name: String) -> Bool:
  match es:
    case Nil{}:
      False{}
    case Con{+e, +t}:
      Bool.or(J.has(e, name), cauldron.has_effect.go(t, name))

# alphabetic runs of ASCII letters
def cauldron.alpha_words(+s: String) -> List<&2, String>:
  cauldron.alpha_words.go(String.to_list(s), "", Nil{})

def cauldron.alpha_words.go(+cs: List<&2, Char>, +cur: String, +acc: List<&2, String>) -> List<&2, String>:
  match cs:
    case Nil{}:
      List.reverse(&2, String, home.util.ite(List<&2, String>, String.is_empty(cur), u => acc, u => cur <> acc))
    case Con{+h, +t}:
      home.util.ite(List<&2, String>, Bool.or(home.util.c_lower(Char.to_u32(h)), home.util.c_upper(Char.to_u32(h))),
        u => cauldron.alpha_words.go(t, String.append(cur, SCon{h, SNil{}}), acc),
        u => cauldron.alpha_words.go(t, "", home.util.ite(List<&2, String>, String.is_empty(cur), u => acc, u => cur <> acc)))

def cauldron.creature_word(+w: String) -> Bool:
  Bool.or(home.util.has_name(cauldron.CREATURES(), w), Bool.and(String.ends_with(w, "s"), home.util.has_name(cauldron.CREATURES(), String.take(w, U32.to_nat((home.util.chars(w) - 1 : U32))))))

def cauldron.any_creature(+ws: List<&2, String>) -> Bool:
  match ws:
    case Nil{}:
      False{}
    case Con{+w, +t}:
      Bool.or(cauldron.creature_word(w), cauldron.any_creature(t))

def cauldron.creature(+c: J.Val) -> Bool:
  Bool.or(cauldron.has_effect(c, "summon"),
    cauldron.any_creature(cauldron.alpha_words(home.util.ascii_lower(J.str(J.get(c, "id")) ++ " " ++ J.str(J.get(c, "name")) ++ " " ++ J.str(J.get(c, "text"))))))

def cauldron.shoots(+c: J.Val) -> Bool:
  Bool.or(cauldron.has_effect(c, "cast"), J.has(J.get(c, "looks"), "shot"))

# A creature and a shot in the pot: the creature card, when one input is a
# creature and the other shoots (null when none).
def cauldron.pet_shot(+a: J.Val, +b: J.Val) -> J.Val:
  home.util.ite(J.Val, Bool.and(cauldron.creature(a), Bool.and(cauldron.shoots(b), Bool.not(cauldron.creature(b)))), u => a,
    u => home.util.ite(J.Val, Bool.and(cauldron.creature(b), Bool.and(cauldron.shoots(a), Bool.not(cauldron.creature(a)))), u => b, u => J.JNull{}))

# Why a spell the host's model brewed from `inputs` is turned away.
def cauldron.faults(+card: J.Val, +a: J.Val, +b: J.Val, +taken: J.Val) -> List<&2, String>:
  List.concat(&2, String, [
    cauldron.fault.kind(card),
    cauldron.fault.power(card),
    cauldron.fault.name(card, a, b, J.items(taken)),
    cauldron.fault.mix(card, a, b, J.items(taken)),
    cauldron.fault.cabbages(card, a, b),
    cauldron.fault.art(card),
    cauldron.fault.pet(card, a, b)])

def cauldron.one(+m: Maybe<&2, String>) -> List<&2, String>:
  match m:
    case Some{+s}:
      [s]
    case None{}:
      Nil{}

def cauldron.fault.kind(+card: J.Val) -> List<&2, String>:
  home.util.ite(List<&2, String>, String.eq(J.str(J.get(card, "kind")), "spell"), u => Nil{},
    u => ["a brew is a spell: `kind spell`, not `kind " ++ J.str(J.get(card, "kind")) ++ "`"])

def cauldron.fault.power(+card: J.Val) -> List<&2, String>:
  cauldron.fault.power.of(cauldron.power(card))

def cauldron.fault.power.of(+p: F32) -> List<&2, String>:
  home.util.ite(List<&2, String>, F32.is_gt(p, 29.5),
    u => ["it comes to " ++ home.util.fmt1(p) ++ " power a second; a brew may come to at most 29.5: lower its numbers or lengthen its cooldown"],
    u => Nil{})

# the first (id, name) among the inputs and then the taken whose bare name is `name`
def cauldron.name_hit(+id: String, +name: String, +cands: List<&2, J.Val>) -> J.Val:
  match cands:
    case Nil{}:
      J.JNull{}
    case Con{+c, +t}:
      home.util.ite(J.Val, Bool.and(Bool.not(String.eq(J.str(J.get(c, "id")), id)), String.eq(cauldron.bare(J.str(J.get(c, "name"))), name)), u => c, u => cauldron.name_hit(id, name, t))

def cauldron.fault.name(+card: J.Val, +a: J.Val, +b: J.Val, +taken: List<&2, J.Val>) -> List<&2, String>:
  cauldron.fault.name.hit(card, cauldron.name_hit(J.str(J.get(card, "id")), cauldron.bare(J.str(J.get(card, "name"))), a <> (b <> taken)))

def cauldron.fault.name.hit(+card: J.Val, +same: J.Val) -> List<&2, String>:
  match same:
    case J.JNull{}:
      Nil{}
    case _:
      ["the name " ++ J.str(J.get(card, "name")) ++ " is taken (" ++ J.str(J.get(same, "name")) ++ "): give it a name of its own"]

# (id, name, mix) of an input card
def cauldron.mix_of(+c: J.Val) -> J.Val:
  home.util.obj([("id", J.get(c, "id")), ("name", J.get(c, "name")), ("mix", home.util.jstrs(cauldron.effect_mix(c)))])

def cauldron.taken_mix(+t: J.Val) -> J.Val:
  t

def cauldron.mix_hit(+id: String, +own: List<&2, String>, +cands: List<&2, J.Val>) -> J.Val:
  match cands:
    case Nil{}:
      J.JNull{}
    case Con{+c, +t}:
      home.util.ite(J.Val, Bool.and(Bool.not(String.eq(J.str(J.get(c, "id")), id)), home.util.eq_strs(cauldron.ck_strs(J.get(c, "mix")), own)), u => c, u => cauldron.mix_hit(id, own, t))

def cauldron.ck_strs(+a: J.Val) -> List<&2, String>:
  cards.ck.strs(a)

def cauldron.fault.mix(+card: J.Val, +a: J.Val, +b: J.Val, +taken: List<&2, J.Val>) -> List<&2, String>:
  cauldron.fault.mix.hit(cauldron.effect_mix(card), cauldron.mix_hit(J.str(J.get(card, "id")), cauldron.effect_mix(card), cauldron.mix_of(a) <> (cauldron.mix_of(b) <> taken)))

def cauldron.fault.mix.hit(+own: List<&2, String>, +same: J.Val) -> List<&2, String>:
  match same:
    case J.JNull{}:
      Nil{}
    case _:
      ["its effects (" ++ home.util.join(own, ", ") ++ ") are " ++ J.str(J.get(same, "name")) ++ "'s mix: fuse the two into a combination no spell has"]

def cauldron.fault.cabbages(+card: J.Val, +a: J.Val, +b: J.Val) -> List<&2, String>:
  home.util.ite(List<&2, String>, Bool.and(cauldron.has_effect(card, "cabbages"), Bool.not(Bool.or(cauldron.has_effect(a, "cabbages"), cauldron.has_effect(b, "cabbages")))),
    u => ["no cabbages: no cabbage went in"], u => Nil{})

def cauldron.fault.art(+card: J.Val) -> List<&2, String>:
  home.util.ite(List<&2, String>, U32.is_eq(J.len(J.get(card, "art")), 0), u => ["draw its card icon under `art`"], u => Nil{})

def cauldron.fault.pet(+card: J.Val, +a: J.Val, +b: J.Val) -> List<&2, String>:
  cauldron.fault.pet.of(card, cauldron.pet_shot(a, b))

def cauldron.fault.pet.of(+card: J.Val, +pet: J.Val) -> List<&2, String>:
  match pet:
    case J.JNull{}:
      Nil{}
    case _:
      home.util.ite(List<&2, String>, Bool.or(Bool.not(cauldron.swarms(J.items(J.get(card, "effects")))), Bool.not(J.has(J.get(card, "looks"), "summon"))),
        u => [J.str(J.get(pet, "name")) ++ " and a shot went in, so the brew fires homing " ++ J.str(J.get(pet, "name")) ++ ": `summon N swarm` and a `draw summon` of it"],
        u => Nil{})

def cauldron.swarms(+es: List<&2, J.Val>) -> Bool:
  match es:
    case Nil{}:
      False{}
    case Con{+e, +t}:
      Bool.or(Bool.and(J.has(e, "summon"), String.eq(J.str(home.util.get(J.items(J.get(e, "summon")), 1)), "swarm")), cauldron.swarms(t))
