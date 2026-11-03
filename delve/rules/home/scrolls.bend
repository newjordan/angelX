# crates/delve/src/rules/scrolls.rs and scrolls/policy.rs — the wish scroll's
# retained-source checks and the `delve.scroll` policy (`Request::evaluate`).
# A Result<(), String> is a `Maybe<&2, String>`: None is Ok, Some(text) the Err.
# A Request, Offer, Choice or Entry is the J value in its serde shape.

# ---- scrolls.rs ------------------------------------------------------------

# One choice of a retained view is well formed (`valid_view`).
def scrolls.valid_choice(+c: J.Val, +great: Bool, +n: U32) -> Bool:
  +wish = J.get(c, "wish")
  +take = J.str(J.get(c, "take"))
  +steps = J.u32(J.get(c, "steps"))
  +tip = J.get(c, "tip")
  Bool.and(home.util.blen_le(J.str(J.get(c, "head")), 1024),
  Bool.and(home.util.blen_le(J.str(J.get(c, "gives")), 4096),
  Bool.and(home.util.blen_le(J.str(J.get(c, "cost")), 1024),
  Bool.and(scrolls.valid_choice.wish(wish),
  Bool.and(scrolls.valid_choice.tip(tip),
    Bool.pick(Bool, String.eq(take, "Free"),
      Bool.and(great, Bool.and(J.is_null(wish), Bool.and(U32.is_eq(steps, 0), Bool.and(J.is_null(tip), U32.is_eq(n, 1))))),
      Bool.and(home.util.rng(steps, 1, 5),
      Bool.and(Bool.or(Bool.not(String.eq(take, "Split")), Bool.and(great, U32.is_eq(steps, 1))),
        scrolls.valid_choice.spell(wish, take, steps, tip)))))))))

def scrolls.valid_choice.wish(+wish: J.Val) -> Bool:
  match wish:
    case J.JS{+id}:
      Bool.and(Bool.not(String.is_empty(id)), home.util.blen_le(id, 128))
    case _:
      True{}

def scrolls.valid_choice.tip(+tip: J.Val) -> Bool:
  match tip:
    case J.JNull{}:
      True{}
    case _:
      Bool.and(U32.is_le(J.u32(J.get(tip, "Purse")), 1000000), U32.is_le(J.u32(J.get(tip, "Mend")), 1000000))

def scrolls.valid_choice.spell(+wish: J.Val, +take: String, +steps: U32, +tip: J.Val) -> Bool:
  match wish:
    case J.JS{+id}:
      Bool.or(Bool.not(String.starts_with(id, "spell:")), Bool.and(String.eq(take, "Said"), Bool.and(U32.is_le(steps, 2), J.is_null(tip))))
    case _:
      True{}

def scrolls.valid_choices(+cs: List<&2, J.Val>, +great: Bool, +n: U32) -> Bool:
  match cs:
    case Nil{}:
      True{}
    case Con{+h, +t}:
      Bool.and(scrolls.valid_choice(h, great, n), scrolls.valid_choices(t, great, n))

def scrolls.valid_view(+offer: J.Val) -> Bool:
  scrolls.valid_view.go(offer, home.util.len(J.items(J.get(offer, "choices"))))

def scrolls.valid_view.go(+offer: J.Val, +n: U32) -> Bool:
  Bool.and(home.util.blen_le(J.str(J.get(offer, "said")), 4096),
  Bool.and(home.util.blen_le(J.str(J.get(offer, "line")), 4096),
  Bool.and(home.util.blen_le(J.str(J.get(offer, "ask")), 4096),
  Bool.and(home.util.rng(n, 1, 3),
    scrolls.valid_choices(J.items(J.get(offer, "choices")), J.bool(J.get(offer, "great")), n)))))

# ---- scrolls/policy.rs: Entry -----------------------------------------------

def scrolls.policy.id_chars(+s: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{+h, +t}:
      Bool.and(Bool.or(home.util.c_lower(Char.to_u32(h)), Bool.or(home.util.c_digit(Char.to_u32(h)), U32.is_eq(Char.to_u32(h), 45))), scrolls.policy.id_chars(t))

def scrolls.policy.Entry.valid(+e: J.Val) -> Bool:
  +id = J.str(J.get(e, "id"))
  +n = home.util.len(J.items(J.get(e, "tiers")))
  Bool.and(Bool.not(String.is_empty(id)),
  Bool.and(home.util.blen_le(id, 24),
  Bool.and(scrolls.policy.id_chars(id),
  Bool.and(Bool.not(home.util.is_blank(J.str(J.get(e, "name")))),
  Bool.and(home.util.blen_le(J.str(J.get(e, "name")), 1024),
  Bool.and(home.util.rng(n, 1, 5),
    home.util.all_len(J.items(J.get(e, "tiers")), 4096)))))))

def scrolls.policy.Entry.card_id(+e: J.Val, +tier: U32) -> String:
  "wish-" ++ J.str(J.get(e, "id")) ++ "-" ++ U32.show((tier + 1 : U32))

# The next tier of the ladder the deck has not reached, if any.
def scrolls.policy.Entry.next_tier(+e: J.Val, +deck: J.Val) -> Maybe<&2, U32>:
  scrolls.policy.Entry.next_tier.fin(scrolls.policy.Entry.top(e, deck, home.util.len(J.items(J.get(e, "tiers")))), home.util.len(J.items(J.get(e, "tiers"))))

# 0 when no tier is held, else the highest held tier + 1
def scrolls.policy.Entry.top(+e: J.Val, +deck: J.Val, +t: U32) -> U32:
  match t:
    case 0:
      0
    case _:
      scrolls.policy.Entry.top.if(home.util.str_has(deck, scrolls.policy.Entry.card_id(e, (t - 1 : U32))), e, deck, t)

def scrolls.policy.Entry.top.if(held: Bool, +e: J.Val, +deck: J.Val, +t: U32) -> U32:
  match held:
    case True{}:
      t
    case False{}:
      scrolls.policy.Entry.top(e, deck, (t - 1 : U32))

def scrolls.policy.Entry.next_tier.fin(+next: U32, +len: U32) -> Maybe<&2, U32>:
  home.util.ite(Maybe<&2, U32>, U32.is_lt(next, len), u => Some{next}, u => None{})

# The ask a Making has open first, as its word (none when it is whole).
def scrolls.policy.made(+m: J.Val) -> Bool:
  Bool.or(J.bool(J.get(m, "made")), Bool.and(U32.is_ge(J.len(J.get(m, "parts")), 1), Bool.and(J.bool(J.get(m, "weapon")), J.bool(J.get(m, "ultimate")))))

def scrolls.policy.asks(+m: J.Val) -> List<&2, String>:
  home.util.ite(List<&2, String>, scrolls.policy.made(m), u => Nil{}, u =>
    List.append(&2, String,
      Bool.pick(List<&2, String>, U32.is_lt(J.len(J.get(m, "parts")), 1), ["body"], Nil{}),
      List.append(&2, String,
        Bool.pick(List<&2, String>, J.bool(J.get(m, "weapon")), Nil{}, ["weapon"]),
        Bool.pick(List<&2, String>, J.bool(J.get(m, "ultimate")), Nil{}, ["ultimate"]))))

def scrolls.policy.first_ask(+m: J.Val) -> Maybe<&2, String>:
  scrolls.policy.first_ask.of(scrolls.policy.asks(m))

def scrolls.policy.first_ask.of(+xs: List<&2, String>) -> Maybe<&2, String>:
  match xs:
    case Con{+h, +t}:
      Some{h}
    case Nil{}:
      None{}

def scrolls.policy.question(+ask: String) -> String:
  Bool.pick(String, String.eq(ask, "body"), "What are you?",
    Bool.pick(String, String.eq(ask, "weapon"), "What do you fight with?", "What is your ultimate?"))

# Making::heading
def scrolls.policy.heading(+m: J.Val) -> J.Val:
  scrolls.policy.heading.of(scrolls.policy.first_ask(m))

def scrolls.policy.heading.of(+a: Maybe<&2, String>) -> J.Val:
  match a:
    case Some{+a}:
      J.JS{scrolls.policy.question(a)}
    case None{}:
      J.JNull{}

# `making.is_none_or(|m| m.asks().first() == entry.answers)`
def scrolls.policy.Entry.fits(+e: J.Val, +making: J.Val) -> Bool:
  match making:
    case J.JNull{}:
      True{}
    case _:
      scrolls.policy.same_ask(scrolls.policy.first_ask(making), J.get(e, "answers"))

def scrolls.policy.same_ask(+a: Maybe<&2, String>, +b: J.Val) -> Bool:
  match a:
    case Some{+x}:
      String.eq(x, J.str(b))
    case None{}:
      J.is_null(b)

# ---- scrolls/policy.rs: Request::validate -----------------------------------

def scrolls.policy.spells_ok(+xs: List<&2, J.Val>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Con{+s, +t}:
      Bool.and(Bool.and(home.util.blen_le(J.str(J.get(s, "id")), 128), Bool.and(home.util.blen_le(J.str(J.get(s, "name")), 1024), U32.is_le(J.u32(J.get(s, "tier")), 2))), scrolls.policy.spells_ok(t))

# every whim is valid, unlearned, and has an id not seen before
def scrolls.policy.whims_ok(+xs: List<&2, J.Val>, +seen: List<&2, String>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Con{+w, +t}:
      Bool.and(Bool.and(scrolls.policy.Entry.valid(w), Bool.and(Bool.not(J.bool(J.get(w, "learned"))), Bool.not(home.util.has_name(seen, J.str(J.get(w, "id")))))), scrolls.policy.whims_ok(t, J.str(J.get(w, "id")) <> seen))

def scrolls.policy.known_ok(+k: J.Val) -> Bool:
  match k:
    case J.JNull{}:
      True{}
    case _:
      scrolls.policy.Entry.valid(k)

def scrolls.policy.heard_ok(+h: J.Val) -> Bool:
  match h:
    case J.JS{+s}:
      home.util.blen_le(s, 4096)
    case _:
      True{}

def scrolls.policy.previous_ok(+p: J.Val) -> Bool:
  match p:
    case J.JNull{}:
      True{}
    case _:
      Bool.and(J.is_null(J.get(p, "binding")), scrolls.valid_view(p))

def scrolls.policy.Request.validate(+req: J.Val) -> Maybe<&2, String>:
  scrolls.policy.Request.validate.fin(scrolls.policy.Request.valid(req))

def scrolls.policy.Request.validate.fin(ok: Bool) -> Maybe<&2, String>:
  match ok:
    case True{}:
      None{}
    case False{}:
      Some{"invalid scroll policy capture"}

def scrolls.policy.Request.valid(+req: J.Val) -> Bool:
  +f = J.get(req, "facts")
  Bool.and(home.util.rng(J.u32(J.get(f, "player")), 1, 6),
  Bool.and(home.util.blen_le(J.str(J.get(req, "said")), 4096),
  Bool.and(scrolls.policy.heard_ok(J.get(req, "heard")),
  Bool.and(U32.is_le(J.len(J.get(f, "deck")), 1024),
  Bool.and(home.util.all_len(J.items(J.get(f, "deck")), 128),
  Bool.and(U32.is_le(J.len(J.get(f, "spells")), 4),
  Bool.and(scrolls.policy.spells_ok(J.items(J.get(f, "spells"))),
  Bool.and(scrolls.policy.previous_ok(J.get(f, "previous")),
  Bool.and(scrolls.policy.known_ok(J.get(req, "known")),
  Bool.and(U32.is_le(J.len(J.get(req, "whims")), 512),
  Bool.and(scrolls.policy.whims_ok(J.items(J.get(req, "whims")), Nil{}),
    U32.is_le(home.util.jlen(req), 1048576))))))))))))

# ---- scrolls/policy.rs: Request::validate_result ------------------------------

# The spell a choice names is held at the tier it climbs from, or the wish
# is one the capture carries.
def scrolls.policy.choice_bad(+req: J.Val, +c: J.Val) -> Bool:
  scrolls.policy.choice_bad.of(req, c, J.get(c, "wish"))

def scrolls.policy.choice_bad.of(+req: J.Val, +c: J.Val, +wish: J.Val) -> Bool:
  match wish:
    case J.JS{+id}:
      Bool.pick(Bool, String.starts_with(id, "spell:"),
        Bool.not(scrolls.policy.spell_held(J.items(J.get(J.get(req, "facts"), "spells")), String.drop(id, 6n), J.u32(J.get(c, "steps")))),
        Bool.not(scrolls.policy.wish_captured(req, id)))
    case _:
      False{}

def scrolls.policy.spell_held(+spells: List<&2, J.Val>, +id: String, +steps: U32) -> Bool:
  match spells:
    case Nil{}:
      False{}
    case Con{+s, +t}:
      Bool.or(Bool.and(String.eq(J.str(J.get(s, "id")), id), U32.is_eq(steps, (J.u32(J.get(s, "tier")) + 1 : U32))), scrolls.policy.spell_held(t, id, steps))

def scrolls.policy.wish_captured(+req: J.Val, +id: String) -> Bool:
  Bool.or(String.eq(J.str(J.get(J.get(req, "known"), "id")), id), scrolls.policy.whim_named(J.items(J.get(req, "whims")), id))

def scrolls.policy.whim_named(+ws: List<&2, J.Val>, +id: String) -> Bool:
  match ws:
    case Nil{}:
      False{}
    case Con{+w, +t}:
      Bool.or(String.eq(J.str(J.get(w, "id")), id), scrolls.policy.whim_named(t, id))

def scrolls.policy.any_bad(+req: J.Val, +cs: List<&2, J.Val>) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case Con{+c, +t}:
      Bool.or(scrolls.policy.choice_bad(req, c), scrolls.policy.any_bad(req, t))

# `result` is the ResultValue as serialised: {"Ok": decision} or {"Err": text}.
def scrolls.policy.Request.validate_result(+req: J.Val, +result: J.Val) -> Maybe<&2, String>:
  home.util.ite(Maybe<&2, String>, J.has(result, "Ok"),
    u => scrolls.policy.Request.validate_result.ok(req, result, J.get(result, "Ok")),
    u => scrolls.policy.Request.validate_result.err(J.str(J.get(result, "Err"))))

def scrolls.policy.Request.validate_result.err(+error: String) -> Maybe<&2, String>:
  home.util.ite(Maybe<&2, String>, home.util.blen_le(error, 4096), u => None{}, u => Some{"scroll policy error exceeds bound"})

def scrolls.policy.Request.validate_result.ok(+req: J.Val, +result: J.Val, +decision: J.Val) -> Maybe<&2, String>:
  scrolls.policy.Request.validate_result.fin(scrolls.policy.Request.result_ok(req, result, decision, J.get(decision, "offer")))

def scrolls.policy.Request.validate_result.fin(ok: Bool) -> Maybe<&2, String>:
  match ok:
    case True{}:
      None{}
    case False{}:
      Some{"scroll policy returned an invalid or uncaptured choice"}

def scrolls.policy.Request.result_ok(+req: J.Val, +result: J.Val, +decision: J.Val, +offer: J.Val) -> Bool:
  +f = J.get(req, "facts")
  +price = J.u32(J.get(decision, "blood_price"))
  +great = J.bool(J.get(offer, "great"))
  Bool.and(J.is_null(J.get(offer, "binding")),
  Bool.and(scrolls.valid_view(offer),
  Bool.and(J.eq(J.get(offer, "opened"), J.get(f, "tick")),
  Bool.and(Bool.not(U32.is_eq(price, 0)),
  Bool.and(U32.is_le(price, U32.max(J.u32(J.get(f, "max_hp")), 1)),
  Bool.and(Bool.or(Bool.not(great), Bool.not(U32.is_eq(J.u32(J.get(f, "great_wishes")), 0))),
  Bool.and(Bool.or(great, Bool.not(U32.is_eq(J.u32(J.get(f, "wishes")), 0))),
  Bool.and(Bool.not(U32.is_eq(J.u32(J.get(f, "hp")), 0)),
  Bool.and(Bool.not(scrolls.policy.any_bad(req, J.items(J.get(offer, "choices")))),
    U32.is_le(home.util.jlen(result), 32768))))))))))

# ---- scrolls/policy.rs: Request::decide ---------------------------------------

def scrolls.policy.numeral(+tier: U32) -> String:
  Bool.pick(String, U32.is_eq(tier, 0), "I",
  Bool.pick(String, U32.is_eq(tier, 1), "II",
  Bool.pick(String, U32.is_eq(tier, 2), "III",
  Bool.pick(String, U32.is_eq(tier, 3), "IV",
  Bool.pick(String, U32.is_eq(tier, 4), "V", "+")))))

# max(max_hp / BLOOD_PRICE, 1)
def scrolls.policy.price(+max_hp: U32) -> U32:
  U32.max(U32.div(max_hp, 3), 1)

# `genie::frees`
def genie.frees(+words: String) -> Bool:
  genie.frees.go(home.util.lower(words))

def genie.frees.go(+w: String) -> Bool:
  Bool.and(String.contains(w, "genie"),
    Bool.or(String.contains(w, "free"), Bool.or(String.contains(w, "freedom"), Bool.or(String.contains(w, "release"), Bool.or(String.contains(w, "let him go"), Bool.or(String.contains(w, "let the genie go"), Bool.or(String.contains(w, "liberat"), Bool.or(String.contains(w, "unchain"), String.contains(w, "unshackle")))))))))

# `dead_wishes::named`
def dead_wishes.named(+words: String) -> Bool:
  dead_wishes.named.go(" " ++ home.util.join(home.util.words(String.from_list(dead_wishes.named.plain(String.to_list(home.util.lower(words))))), " ") ++ " ")

def dead_wishes.named.plain(+cs: List<&2, Char>) -> List<&2, Char>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Bool.pick(Char, home.util.c_alnum_u(Char.to_u32(h)), h, Char.from_u32(32)) <> dead_wishes.named.plain(t)

def dead_wishes.named.go(+plain: String) -> Bool:
  dead_wishes.named.any(["ghoul", "ghouls", "zombie", "zombies", "skeleton", "skeletons", "skelly", "skellies", "minion", "minions", "warden", "wardens", "totem", "totems", "undead", "army", "armies", "horde", "summons", "pet", "pets", "thrall", "thralls", "servants", "my dead", "the dead", "his dead", "our dead", "raised dead", "corpses"], plain)

def dead_wishes.named.any(+ws: List<&2, String>, +plain: String) -> Bool:
  match ws:
    case Nil{}:
      False{}
    case Con{+w, +t}:
      Bool.or(String.contains(plain, " " ++ w ++ " "), dead_wishes.named.any(t, plain))

# `rules::mix`
def scrolls.policy.mix(+h: W.W) -> W.W:
  +a = W.xor(h, W.shr(h, 33))
  +b = W.mul(a, W.W{4283543511, 3981806797})
  +c = W.xor(b, W.shr(b, 33))
  +d = W.mul(c, W.W{3301882366, 444984403})
  W.xor(d, W.shr(d, 33))

def scrolls.policy.choice(+take: String, +wish: J.Val, +steps: U32, +head: String, +gives: String, +cost: String, +tip: J.Val) -> J.Val:
  home.util.set_if(Bool.not(J.is_null(tip)),
    home.util.set_if(Bool.not(String.is_empty(cost)),
      home.util.set_if(Bool.not(J.is_null(wish)),
        home.util.obj([("take", J.JS{take}), ("steps", J.JU{steps}), ("head", J.JS{head}), ("gives", J.JS{gives})]),
        "wish", wish),
      "cost", J.JS{cost}),
    "tip", tip)

def scrolls.policy.offer(+said: String, +great: Bool, +choices: List<&2, J.Val>, +line: String, +opened: J.Val, +ask: J.Val, +target: J.Val) -> J.Val:
  home.util.set_if(Bool.not(J.is_null(target)),
    home.util.set_if(Bool.not(J.is_null(ask)),
      home.util.obj([("said", J.JS{said}), ("great", J.JBool{great}), ("choices", J.JA{choices}), ("line", J.JS{line}), ("opened", opened)]),
      "ask", ask),
    "target", target)

def scrolls.policy.decision(+offer: J.Val, +price: U32) -> J.Val:
  home.util.ok(home.util.obj([("offer", offer), ("blood_price", J.JU{price})]))

def scrolls.policy.Request.decide(+req: J.Val) -> J.Val:
  +f = J.get(req, "facts")
  home.util.ite(J.Val, Bool.or(U32.is_eq(J.u32(J.get(f, "hp")), 0), U32.is_eq(U32.min((J.u32(J.get(f, "wishes")) + J.u32(J.get(f, "great_wishes")) : U32), 255), 0)),
    u => home.util.err("No wishes held: each floor brings two, and its boss a super wish."),
    u => scrolls.policy.Request.decide.upgrade(req, scrolls.policy.Request.upgrade_spell(J.str(J.get(req, "said")), J.items(J.get(f, "spells")))))

# The captured spell an "upgrade <name>" wish names, or null.
def scrolls.policy.Request.upgrade_spell(+said: String, +spells: List<&2, J.Val>) -> J.Val:
  scrolls.policy.Request.upgrade_spell.cut(home.util.cut(home.util.trim(said), 8), spells)

def scrolls.policy.Request.upgrade_spell.cut(+m: Maybe<&2, Sigma<&2, &2, String, _ => String>>, +spells: List<&2, J.Val>) -> J.Val:
  match m:
    case Some{(+head, +rest)}:
      scrolls.policy.Request.upgrade_spell.hit(home.util.eq_ic(head, "upgrade "), home.util.trim(rest), spells)
    case None{}:
      J.JNull{}

def scrolls.policy.Request.upgrade_spell.hit(hit: Bool, +name: String, +spells: List<&2, J.Val>) -> J.Val:
  match hit:
    case True{}:
      scrolls.policy.Request.spell_named(spells, name)
    case False{}:
      J.JNull{}

def scrolls.policy.Request.spell_named(+spells: List<&2, J.Val>, +name: String) -> J.Val:
  match spells:
    case Nil{}:
      J.JNull{}
    case Con{+s, +t}:
      scrolls.policy.Request.spell_named.if(home.util.eq_ic(J.str(J.get(s, "name")), name), s, t, name)

def scrolls.policy.Request.spell_named.if(hit: Bool, +s: J.Val, +t: List<&2, J.Val>, +name: String) -> J.Val:
  match hit:
    case True{}:
      s
    case False{}:
      scrolls.policy.Request.spell_named(t, name)

def scrolls.policy.Request.decide.upgrade(+req: J.Val, +spell: J.Val) -> J.Val:
  match spell:
    case J.JNull{}:
      scrolls.policy.Request.decide.making(req)
    case _:
      scrolls.policy.Request.decide.upgrade.tier(req, spell, J.u32(J.get(spell, "tier")))

def scrolls.policy.Request.decide.upgrade.tier(+req: J.Val, +spell: J.Val, +tier: U32) -> J.Val:
  home.util.ite(J.Val, U32.is_eq(tier, 2),
    u => home.util.err(J.str(J.get(spell, "name")) ++ " is already granted in full"),
    u => scrolls.policy.Request.decide.upgrade.offer(req, spell, tier))

def scrolls.policy.Request.decide.upgrade.offer(+req: J.Val, +spell: J.Val, +tier: U32) -> J.Val:
  +f = J.get(req, "facts")
  scrolls.policy.decision(
    scrolls.policy.offer(home.util.trim(J.str(J.get(req, "said"))),
      Bool.and(Bool.not(U32.is_eq(J.u32(J.get(f, "great_wishes")), 0)), Bool.or(J.is_null(J.get(f, "making")), U32.is_eq(J.u32(J.get(f, "wishes")), 0))),
      [scrolls.policy.choice("Said", J.JS{"spell:" ++ J.str(J.get(spell, "id"))}, (tier + 1 : U32), "Upgrade spell", J.str(J.get(spell, "name")) ++ " " ++ scrolls.policy.numeral((tier + 1 : U32)), "spends one wish", J.JNull{})],
      "Your own magic grows with you.", J.get(f, "tick"), J.JNull{}, J.JNull{}),
    scrolls.policy.price(J.u32(J.get(f, "max_hp"))))

# A knight still unmade wishes only themselves, with plain wishes.
def scrolls.policy.Request.decide.making(+req: J.Val) -> J.Val:
  scrolls.policy.Request.decide.making.of(req, J.get(J.get(req, "facts"), "making"))

def scrolls.policy.Request.decide.making.of(+req: J.Val, +making: J.Val) -> J.Val:
  match making:
    case J.JNull{}:
      scrolls.policy.Request.decide.free(req, making)
    case _:
      scrolls.policy.Request.decide.unmade(req, making, scrolls.policy.heading_text(making))

def scrolls.policy.heading_text(+m: J.Val) -> String:
  scrolls.policy.heading_text.of(scrolls.policy.first_ask(m))

def scrolls.policy.heading_text.of(+a: Maybe<&2, String>) -> String:
  match a:
    case Some{+a}:
      scrolls.policy.question(a)
    case None{}:
      "What are you?"

def scrolls.policy.Request.decide.unmade(+req: J.Val, +making: J.Val, +ask: String) -> J.Val:
  +f = J.get(req, "facts")
  +wish = J.get(req, "known")
  home.util.ite(J.Val, U32.is_eq(J.u32(J.get(f, "wishes")), 0),
    u => home.util.err("Your super wish waits until you are whole. " ++ ask),
    u => scrolls.policy.Request.decide.unmade.fit(req, making, ask, Bool.and(Bool.not(J.is_null(wish)), Bool.not(scrolls.policy.Entry.fits(wish, making)))))

def scrolls.policy.Request.decide.unmade.fit(+req: J.Val, +making: J.Val, +ask: String, misfit: Bool) -> J.Val:
  match misfit:
    case True{}:
      home.util.err("First, wish yourself into being. " ++ ask ++ " " ++ J.str(J.get(J.get(req, "known"), "name")) ++ " doesn't answer it.")
    case False{}:
      scrolls.policy.Request.decide.unmade.free(req, making, ask, Bool.and(genie.frees(J.str(J.get(req, "said"))), J.is_null(J.get(req, "known"))))

def scrolls.policy.Request.decide.unmade.free(+req: J.Val, +making: J.Val, +ask: String, frees: Bool) -> J.Val:
  match frees:
    case True{}:
      home.util.err("First, wish yourself into being. " ++ ask)
    case False{}:
      scrolls.policy.Request.decide.free(req, making)

# The genie's freedom is asked for, with no wish named and no making open.
def scrolls.policy.Request.decide.free(+req: J.Val, +making: J.Val) -> J.Val:
  +great = Bool.and(Bool.not(U32.is_eq(J.u32(J.get(J.get(req, "facts"), "great_wishes")), 0)), J.is_null(making))
  home.util.ite(J.Val, Bool.and(genie.frees(J.str(J.get(req, "said"))), Bool.and(J.is_null(J.get(req, "known")), J.is_null(making))),
    u => scrolls.policy.Request.freedom(req, great),
    u => scrolls.policy.Request.decide.roll(req, making, great))

def scrolls.policy.Request.freedom(+req: J.Val, +great: Bool) -> J.Val:
  +f = J.get(req, "facts")
  +g = J.get(f, "genie")
  +freedom = J.u32(J.get(g, "freedom"))
  home.util.ite(J.Val, Bool.not(J.is_null(J.get(g, "freed"))),
    u => home.util.err("The genie is already free, and stays as your friend."),
    u => home.util.ite(J.Val, U32.is_lt(freedom, 100),
      u => home.util.err("The shackle holds: the gauge is at " ++ U32.show(U32.min(freedom, 100)) ++ "/100. Seal greater wishes to fill it."),
      u => scrolls.policy.Request.freedom.great(f, great)))

def scrolls.policy.Request.freedom.great(+f: J.Val, great: Bool) -> J.Val:
  match great:
    case False{}:
      home.util.err("Only a super wish breaks the shackle: a floor's boss gives one.")
    case True{}:
      scrolls.policy.decision(
        scrolls.policy.offer("I wish the genie were free", True{},
          [scrolls.policy.choice("Free", J.JNull{}, 0, "Set him free", "the genie stays on as your friend", "spends your super wish", J.JNull{})],
          "You'd... really do that? For me?", J.get(f, "tick"), J.JNull{}, J.JNull{}),
        scrolls.policy.price(J.u32(J.get(f, "max_hp"))))

# A summoner's scroll asks whom it is for, his dead first when the words name them.
def scrolls.policy.Request.target(+req: J.Val) -> J.Val:
  home.util.ite(J.Val, J.bool(J.get(J.get(req, "facts"), "summoner")),
    u => home.util.ite(J.Val, Bool.or(dead_wishes.named(J.str(J.get(req, "said"))), dead_wishes.named(J.str(J.get(req, "heard")))),
      u => J.JS{"dead"}, u => J.JS{"you"}),
    u => J.JNull{})

# roll = mix(seed ^ player.rotate_left(29) ^ sealed.wrapping_mul(0x9e3779b9))
def scrolls.policy.Request.roll(+f: J.Val) -> W.W:
  scrolls.policy.mix(W.xor(W.xor(W.of_j(J.get(f, "seed")), W.rotl(W.of_u32(J.u32(J.get(f, "player"))), 29)), W.mul(W.of_u32(J.u32(J.get(J.get(f, "genie"), "sealed"))), W.of_u32(2654435769))))

def scrolls.policy.Request.decide.roll(+req: J.Val, +making: J.Val, great: Bool) -> J.Val:
  scrolls.policy.Request.decide.ladder(req, making, great, scrolls.policy.Request.roll(J.get(req, "facts")), scrolls.policy.Request.target(req), J.get(req, "known"))

def scrolls.policy.Request.decide.ladder(+req: J.Val, +making: J.Val, +great: Bool, +roll: W.W, +target: J.Val, +known: J.Val) -> J.Val:
  match known:
    case J.JNull{}:
      scrolls.policy.Request.decide.build(req, making, great, roll, target, home.util.trim(J.str(J.get(req, "said"))), known, 0)
    case _:
      scrolls.policy.Request.decide.ladder.tier(req, making, great, roll, target, known, scrolls.policy.Entry.next_tier(known, J.get(J.get(req, "facts"), "deck")))

def scrolls.policy.Request.decide.ladder.tier(+req: J.Val, +making: J.Val, +great: Bool, +roll: W.W, +target: J.Val, +known: J.Val, +tier: Maybe<&2, U32>) -> J.Val:
  match tier:
    case Some{+t}:
      scrolls.policy.Request.decide.build(req, making, great, roll, target, J.str(J.get(known, "name")), known, t)
    case None{}:
      home.util.err(J.str(J.get(known, "name")) ++ " is already granted in full")

# min(tier + steps, len) - 1
def scrolls.policy.reach(+tier: U32, +steps: U32, +len: U32) -> U32:
  (U32.min((tier + steps : U32), len) - 1 : U32)

# What a take gives, as the scroll writes it.
def scrolls.policy.gives(+l: J.Val, +name: String, +tier: U32, +steps: U32) -> String:
  match l:
    case J.JNull{}:
      home.util.ite(String, U32.is_gt(steps, 1),
        u => "'" ++ name ++ "', " ++ U32.show(steps) ++ " steps when learned",
        u => "'" ++ name ++ "', once learned")
    case _:
      J.str(J.get(l, "name")) ++ " " ++ scrolls.policy.numeral(scrolls.policy.reach(tier, steps, J.len(J.get(l, "tiers"))))

# The whims that fit: not the wish asked, answering the open question, with
# the step to climb.
def scrolls.policy.Request.whim.fits(+ws: List<&2, J.Val>, +deck: J.Val, +asked: J.Val, +steps: U32, +making: J.Val) -> List<&2, J.Val>:
  match ws:
    case Nil{}:
      Nil{}
    case Con{+w, +t}:
      home.util.ite(List<&2, J.Val>,
        Bool.and(Bool.not(Bool.and(Bool.not(J.is_null(asked)), String.eq(J.str(asked), J.str(J.get(w, "id"))))),
          Bool.and(scrolls.policy.Entry.fits(w, making), scrolls.policy.Entry.climbs(w, deck, steps))),
        u => w <> scrolls.policy.Request.whim.fits(t, deck, asked, steps, making),
        u => scrolls.policy.Request.whim.fits(t, deck, asked, steps, making))

def scrolls.policy.Entry.climbs(+w: J.Val, +deck: J.Val, +steps: U32) -> Bool:
  scrolls.policy.Entry.climbs.of(scrolls.policy.Entry.next_tier(w, deck), steps, J.len(J.get(w, "tiers")))

def scrolls.policy.Entry.climbs.of(+t: Maybe<&2, U32>, +steps: U32, +len: U32) -> Bool:
  match t:
    case Some{+n}:
      U32.is_lt((n + steps - 1 : U32), len)
    case None{}:
      False{}

# fits[(roll >> 17) % fits.len()], or null when nothing fits
def scrolls.policy.Request.whim(+req: J.Val, +deck: J.Val, +asked: J.Val, +steps: U32, +roll: W.W, +making: J.Val) -> J.Val:
  scrolls.policy.Request.whim.pick(scrolls.policy.Request.whim.fits(J.items(J.get(req, "whims")), deck, asked, steps, making), roll)

def scrolls.policy.Request.whim.pick(+fits: List<&2, J.Val>, +roll: W.W) -> J.Val:
  home.util.ite(J.Val, List.is_empty(&2, J.Val, fits), u => J.JNull{}, u => home.util.get(fits, W.rem_u32(W.shr(roll, 17), home.util.len(fits))))

def scrolls.policy.Request.decide.build(+req: J.Val, +making: J.Val, +great: Bool, +roll: W.W, +target: J.Val, +name: String, +l: J.Val, +tier: U32) -> J.Val:
  scrolls.policy.Request.decide.whims(req, making, great, roll, target, name, l, tier, (1 + Bool.to_u32(great) : U32))

# The genie's whim: a step further when a wish has the step, else as strong.
def scrolls.policy.Request.decide.whims(+req: J.Val, +making: J.Val, +great: Bool, +roll: W.W, +target: J.Val, +name: String, +l: J.Val, +tier: U32, +steps: U32) -> J.Val:
  +asked = J.get(l, "id")
  +deck = J.get(J.get(req, "facts"), "deck")
  +more = scrolls.policy.Request.whim(req, deck, asked, (steps + 1 : U32), roll, making)
  scrolls.policy.Request.decide.whims.pick(req, making, great, roll, target, name, l, tier, steps, more,
    home.util.ite(J.Val, J.is_null(more), u => scrolls.policy.Request.whim(req, deck, asked, steps, roll, making), u => J.JNull{}))

def scrolls.policy.Request.decide.whims.pick(+req: J.Val, +making: J.Val, +great: Bool, +roll: W.W, +target: J.Val, +name: String, +l: J.Val, +tier: U32, +steps: U32, +more: J.Val, +same: J.Val) -> J.Val:
  scrolls.policy.Request.decide.mid(req, making, great, roll, target, name, l, tier, steps,
    home.util.ite(J.Val, J.is_null(more), u => same, u => more),
    home.util.ite(U32, J.is_null(more), u => steps, u => (steps + 1 : U32)))

# `other` is the whim (null when none), `wsteps` the steps it climbs.
def scrolls.policy.Request.decide.mid(+req: J.Val, +making: J.Val, +great: Bool, +roll: W.W, +target: J.Val, +name: String, +l: J.Val, +tier: U32, +steps: U32, +other: J.Val, +wsteps: U32) -> J.Val:
  +f = J.get(req, "facts")
  +price = scrolls.policy.price(J.u32(J.get(f, "max_hp")))
  +id_of = J.get(l, "id")
  +bolder = Bool.or(J.is_null(l), U32.is_lt((tier + steps : U32), J.len(J.get(l, "tiers"))))
  +calm = Bool.or(J.is_null(other), U32.is_le(wsteps, steps))
  +whim = scrolls.policy.Request.whim_choice(other, J.get(f, "deck"), wsteps)
  +middle = home.util.ite(J.Val, Bool.and(bolder, Bool.or(calm, Bool.not(U32.is_eq(W.rem_u32(roll, 3), 0)))),
    u => scrolls.policy.choice("Bolder", id_of, (steps + 1 : U32), "Bolder", scrolls.policy.gives(l, name, tier, (steps + 1 : U32)), "costs " ++ U32.show(price) ++ " health", J.JNull{}),
    u => home.util.ite(J.Val, Bool.and(great, calm),
      u => scrolls.policy.choice("Split", id_of, 1, "Save some", scrolls.policy.gives(l, name, tier, 1) ++ ", and keep a wish", "one step, not two", J.JNull{}),
      u => whim))
  scrolls.policy.Request.decide.deal(req, making, great, roll, target, name, l, tier, steps, price, middle)

def scrolls.policy.Request.whim_choice(+other: J.Val, +deck: J.Val, +more: U32) -> J.Val:
  match other:
    case J.JNull{}:
      J.JNull{}
    case _:
      scrolls.policy.Request.whim_choice.of(other, scrolls.policy.Entry.next_tier(other, deck), more)

def scrolls.policy.Request.whim_choice.of(+other: J.Val, +t: Maybe<&2, U32>, +more: U32) -> J.Val:
  scrolls.policy.choice("Whim", J.get(other, "id"), more, "The genie's whim",
    J.str(J.get(other, "name")) ++ " " ++ scrolls.policy.numeral(scrolls.policy.reach(home.util.or(t, 0), more, J.len(J.get(other, "tiers")))) ++ " instead",
    "not what you asked", J.JNull{})

def scrolls.policy.Request.decide.deal(+req: J.Val, +making: J.Val, +great: Bool, +roll: W.W, +target: J.Val, +name: String, +l: J.Val, +tier: U32, +steps: U32, +price: U32, +middle: J.Val) -> J.Val:
  +purse = U32.is_eq(W.rem_u32(W.div_u32(roll, 3), 2), 0)
  +gold = (30 + 10 * W.rem_u32(W.div_u32(roll, 7), 3) : U32)
  +tip = home.util.ite(J.Val, purse, u => home.util.obj([("Purse", J.JU{gold})]), u => home.util.obj([("Mend", J.JU{20})]))
  +gives = scrolls.policy.gives(l, name, tier, steps)
  +deal = scrolls.policy.choice("Deal", J.get(l, "id"), steps, "The genie's deal",
    home.util.ite(String, purse, u => gives ++ " + " ++ U32.show(gold) ++ " gold", u => gives ++ " + mending + 20 gold"),
    "fills no freedom", tip)
  +first = scrolls.policy.choice("Said", J.get(l, "id"), steps, "As you said", scrolls.policy.gives(l, name, tier, steps), "", J.JNull{})
  scrolls.policy.Request.decide.line(req, making, great, roll, target, name, price, home.util.ite(List<&2, J.Val>, J.is_null(middle), u => [first, deal], u => [first, middle, deal]))

def scrolls.policy.Request.decide.line(+req: J.Val, +making: J.Val, +great: Bool, +roll: W.W, +target: J.Val, +name: String, +price: U32, +choices: List<&2, J.Val>) -> J.Val:
  +f = J.get(req, "facts")
  +idx = W.rem_u32(W.shr(roll, 11), Bool.pick(U32, J.is_null(making), 4, 3))
  +line = home.util.ite(String, J.is_null(making),
    u => scrolls.policy.pick_line(idx, ["Ooh, a good one. How do you want it?", "Your wish! Pick how, friend.", "I could do it straight... or not.", "Psst. Number three is a bargain."]),
    u => scrolls.policy.pick_line(idx, ["Ooh, a fresh one! What will you be?", "Let's make you. Pick how, friend.", "A shape with a wish. My favourite."]))
  scrolls.policy.decision(
    scrolls.policy.offer(name, great, choices, line, J.get(f, "tick"), scrolls.policy.heading_of(making), target),
    price)

def scrolls.policy.heading_of(+making: J.Val) -> J.Val:
  match making:
    case J.JNull{}:
      J.JNull{}
    case _:
      scrolls.policy.heading(making)

def scrolls.policy.pick_line(+i: U32, +xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case Con{+h, +t}:
      home.util.ite(String, U32.is_eq(i, 0), u => h, u => scrolls.policy.pick_line((i - 1 : U32), t))
