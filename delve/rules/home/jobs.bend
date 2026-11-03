# The model-job contracts' policies: card_paint.rs, wish_jobs.rs, brew_jobs.rs,
# reforge_jobs.rs. A `Result<(), String>` is a `Maybe<&2, String>` (None is Ok).

# ---- shared ---------------------------------------------------------------------

# serde_json::to_vec(value).len() <= limit
def jobs.bounded(+v: J.Val, +limit: U32) -> Bool:
  U32.is_le(home.util.jlen(v), limit)

def jobs.refuse(ok: Bool, +why: String) -> Maybe<&2, String>:
  home.util.ite(Maybe<&2, String>, ok, u => None{}, u => Some{why})

# every item of a J array is a string within `n` bytes
def jobs.strs_within(+xs: List<&2, J.Val>, +n: U32) -> Bool:
  home.util.all_len(xs, n)

# a card's id is lowercase letters, digits and dashes
def card_paint.id_ok(+id: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(id)), Bool.and(home.util.blen_le(id, 128), scrolls.policy.id_chars(id)))

# ---- card_paint.rs --------------------------------------------------------------

# `looks` of a card as a value to compare: absent is empty
def card_paint.looks(+card: J.Val) -> J.Val:
  home.util.ite(J.Val, J.has(card, "looks"), u => J.get(card, "looks"), u => J.obj())

def card_paint.valid_card(+card: J.Val) -> Bool:
  Bool.and(card_paint.id_ok(J.str(J.get(card, "id"))),
  Bool.and(home.util.blen_le(J.str(J.get(card, "name")), 1024),
  Bool.and(home.util.blen_le(J.str(J.get(card, "by")), 1024),
  Bool.and(U32.is_le(J.len(J.get(card, "effects")), 128),
  Bool.and(jobs.bounded(card, 131072),
    card_paint.checks_back(card, cards.check(J.str(J.get(card, "id")), cards.Card.definition_file(card))))))))

def card_paint.checks_back(+card: J.Val, +r: J.Val) -> Bool:
  home.util.ite(Bool, J.has(r, "Ok"),
    u => Bool.and(J.eq(J.get(J.get(J.get(r, "Ok"), "card"), "art"), J.get(card, "art")), J.eq(card_paint.looks(J.get(J.get(r, "Ok"), "card")), card_paint.looks(card))),
    u => False{})

# Authorship metadata and visual progress are independent of game behavior.
def card_paint.same_definition(+current: J.Val, +authored: J.Val) -> Bool:
  J.eq(card_paint.bare(current), card_paint.bare(authored))

def card_paint.bare(+c: J.Val) -> J.Val:
  J.del(J.del(J.del(c, "by"), "art"), "looks")

# identity::piece_of: an ultimate, a weapon, magic, or a worn part at its place
def card_paint.piece_of(+card: J.Val) -> String:
  home.util.ite(String, Bool.not(J.is_null(J.get(card, "ult"))), u => "ultimate",
    u => card_paint.piece_of.kind(card, J.str(J.get(card, "kind"))))

def card_paint.piece_of.kind(+card: J.Val, +kind: String) -> String:
  home.util.ite(String, String.eq(kind, "arm"), u => "weapon",
    u => home.util.ite(String, String.eq(kind, "spell"), u => "magic",
      u => home.util.ite(String, Bool.or(String.eq(kind, "hold"), String.eq(kind, "guard")), u => card_paint.piece_of.worn(J.get(J.get(card, "looks"), "worn")), u => "none")))

def card_paint.piece_of.worn(+worn: J.Val) -> String:
  match worn:
    case J.JNull{}:
      "none"
    case _:
      "part:" ++ card_paint.piece_of.place(home.util.opt_str(worn, "at"))

def card_paint.piece_of.place(+at: Maybe<&2, String>) -> String:
  match at:
    case Some{+a}:
      card_paint.piece_of.canon(looks.place(a))
    case None{}:
      "body"

def card_paint.piece_of.canon(+m: Maybe<&2, String>) -> String:
  match m:
    case Some{+p}:
      p
    case None{}:
      "body"

def card_paint.retains_identity(+current: J.Val, +painted: J.Val) -> Bool:
  Bool.and(String.eq(card_paint.piece_of(current), card_paint.piece_of(painted)), Bool.not(Bool.xor(dead_wishes.colours_body(current), dead_wishes.colours_body(painted))))

def card_paint.painted(+candidate: J.Val) -> J.Val:
  home.util.set_if(card_paint.has_looks(J.get(candidate, "authored")),
    J.set(J.get(candidate, "current"), "art", J.get(J.get(candidate, "authored"), "art")),
    "looks", J.get(J.get(candidate, "authored"), "looks"))

def card_paint.has_looks(+c: J.Val) -> Bool:
  J.has(c, "looks")

def card_paint.compatible_visuals(+candidate: J.Val) -> Bool:
  card_paint.compatible_visuals.go(J.get(candidate, "current"), card_paint.painted(candidate))

def card_paint.compatible_visuals.go(+current: J.Val, +painted: J.Val) -> Bool:
  Bool.and(card_paint.retains_identity(current, painted), card_paint.valid_card(painted))

def card_paint.cands_ok(+cs: List<&2, J.Val>, +seen: List<&2, String>) -> Bool:
  match cs:
    case Nil{}:
      True{}
    case Con{+c, +t}:
      Bool.and(
        Bool.and(String.eq(J.str(J.get(J.get(c, "current"), "id")), J.str(J.get(J.get(c, "authored"), "id"))),
          Bool.and(Bool.not(home.util.has_name(seen, J.str(J.get(J.get(c, "current"), "id")))),
            Bool.and(card_paint.valid_card(J.get(c, "current")), card_paint.valid_card(J.get(c, "authored"))))),
        card_paint.cands_ok(t, J.str(J.get(J.get(c, "current"), "id")) <> seen))

def card_paint.Request.validate(+req: J.Val) -> Maybe<&2, String>:
  jobs.refuse(Bool.and(U32.is_le(J.len(J.get(req, "candidates")), 8), Bool.and(card_paint.cands_ok(J.items(J.get(req, "candidates")), Nil{}), jobs.bounded(req, 1048576))),
    "invalid painted catalogue capture")

def card_paint.accepted(+cs: List<&2, J.Val>) -> List<&2, J.Val>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, +t}:
      home.util.ite(List<&2, J.Val>, Bool.and(card_paint.same_definition(J.get(c, "current"), J.get(c, "authored")), card_paint.compatible_visuals(c)),
        u => J.get(J.get(c, "current"), "id") <> card_paint.accepted(t),
        u => card_paint.accepted(t))

def card_paint.Request.evaluate(+req: J.Val) -> J.Val:
  home.util.obj([("accepted", J.JA{card_paint.accepted(J.items(J.get(req, "candidates")))})])

def card_paint.compat_id(+cs: List<&2, J.Val>, +id: String) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case Con{+c, +t}:
      Bool.or(Bool.and(String.eq(J.str(J.get(J.get(c, "current"), "id")), id), card_paint.compatible_visuals(c)), card_paint.compat_id(t, id))

def card_paint.result_ok(+cs: List<&2, J.Val>, +ids: List<&2, J.Val>, +seen: List<&2, String>) -> Bool:
  match ids:
    case Nil{}:
      True{}
    case Con{+id, +t}:
      Bool.and(Bool.and(Bool.not(home.util.has_name(seen, J.str(id))), card_paint.compat_id(cs, J.str(id))), card_paint.result_ok(cs, t, J.str(id) <> seen))

def card_paint.Request.validate_result(+req: J.Val, +decision: J.Val) -> Maybe<&2, String>:
  jobs.refuse(Bool.and(U32.is_le(J.len(J.get(decision, "accepted")), J.len(J.get(req, "candidates"))), card_paint.result_ok(J.items(J.get(req, "candidates")), J.items(J.get(decision, "accepted")), Nil{})),
    "paint policy returned an uncaptured, duplicate or identity-changing visual")

# ---- wish_jobs.rs ---------------------------------------------------------------

def wish_jobs.valid_ask(+ask: J.Val) -> Bool:
  Bool.and(home.util.rng(J.u32(J.get(ask, "knight")), 1, 6),
  Bool.and(Bool.not(home.util.is_blank(J.str(J.get(ask, "words")))),
  Bool.and(home.util.blen_le(J.str(J.get(ask, "words")), 4096),
  Bool.and(home.util.rng(J.u32(J.get(ask, "steps")), 1, 5),
    jobs.bounded(ask, 16384)))))

def wish_jobs.tiers_ok(+wish: J.Val, +t: U32) -> Bool:
  match t:
    case 0:
      True{}
    case _:
      Bool.and(J.has(phrasebook.Wish.card(wish, (t - 1 : U32)), "Ok"), wish_jobs.tiers_ok(wish, (t - 1 : U32)))

def wish_jobs.valid_wish(+wish: J.Val) -> Bool:
  Bool.and(Bool.not(String.is_empty(J.str(J.get(wish, "id")))),
  Bool.and(home.util.blen_le(J.str(J.get(wish, "id")), 24),
  Bool.and(scrolls.policy.id_chars(J.str(J.get(wish, "id"))),
  Bool.and(Bool.not(home.util.is_blank(J.str(J.get(wish, "name")))),
  Bool.and(home.util.blen_le(J.str(J.get(wish, "name")), 1024),
  Bool.and(home.util.rng(J.len(J.get(wish, "tiers")), 1, 5),
  Bool.and(jobs.bounded(wish, 131072),
  Bool.and(wish_jobs.tiers_ok(wish, J.len(J.get(wish, "tiers"))),
    wish_jobs.reads_back(wish)))))))))

def wish_jobs.reads_back(+wish: J.Val) -> Bool:
  wish_jobs.reads_back.of(wish, home.util.get(J.items(phrasebook.read_checked(phrasebook.Wish.definition_file(wish))), 0))

def wish_jobs.reads_back.of(+wish: J.Val, +read: J.Val) -> Bool:
  J.eq(read, J.JA{[wish]})

def wish_jobs.Work.valid(+work: J.Val) -> Bool:
  Bool.and(wish_jobs.valid_ask(J.get(work, "ask")),
  Bool.and(home.util.blen_le(J.str(J.get(work, "maker")), 1024),
  Bool.and(String.starts_with(J.str(J.get(work, "binding")), "wish-"),
    home.util.blen_le(J.str(J.get(work, "binding")), 192))))

def wish_jobs.PolicyRequest.validate(+req: J.Val) -> Maybe<&2, String>:
  jobs.refuse(Bool.and(wish_jobs.Work.valid(J.get(req, "work")), wish_jobs.valid_wish(J.get(req, "wish"))), "invalid wish policy inputs")

# `result` is the PolicyResult as serialised: {"Ok": wish} or {"Err": [text]}
def wish_jobs.PolicyRequest.validate_result(+req: J.Val, +result: J.Val) -> Maybe<&2, String>:
  jobs.refuse(
    home.util.ite(Bool, J.has(result, "Ok"),
      u => Bool.and(String.eq(J.str(J.get(J.get(result, "Ok"), "id")), J.str(J.get(J.get(req, "wish"), "id"))), wish_jobs.valid_wish(J.get(result, "Ok"))),
      u => jobs.errors_ok(J.items(J.get(result, "Err")))),
    "invalid wish policy result")

def jobs.errors_ok(+es: List<&2, J.Val>) -> Bool:
  Bool.and(Bool.not(List.is_empty(&2, J.Val, es)), Bool.and(U32.is_le(home.util.len(es), 64), home.util.all_len(es, 8192)))

def wish_jobs.PolicyRequest.evaluate(+req: J.Val) -> J.Val:
  home.util.ok(J.get(req, "wish"))

# ---- brew_jobs.rs ---------------------------------------------------------------

def brew_jobs.id(+v: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(v)), Bool.and(home.util.blen_le(v, 128), home.util.all_in(v, [48, 57, 65, 90, 97, 122], "-_")))

def brew_jobs.ids_ok(+xs: List<&2, J.Val>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Con{+h, +t}:
      Bool.and(brew_jobs.id(J.str(h)), brew_jobs.ids_ok(t))

def brew_jobs.valid_outbox(+asks: J.Val, +raid: J.Val) -> Bool:
  Bool.and(U32.is_le(J.len(asks), 4096), Bool.and(brew_jobs.asks_ok(J.items(asks), raid), jobs.bounded(asks, 1048576)))

def brew_jobs.asks_ok(+xs: List<&2, J.Val>, +raid: J.Val) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case Con{+ask, +t}:
      Bool.and(
        Bool.and(J.eq(J.get(ask, "raid"), raid), Bool.and(home.util.rng(J.u32(J.get(ask, "knight")), 1, 6), Bool.and(U32.is_gt(J.u32(J.get(ask, "depth")), 0), Bool.and(brew_jobs.ids_ok(J.items(J.get(ask, "cards"))), brew_jobs.id(J.str(J.get(ask, "placeholder"))))))),
        brew_jobs.asks_ok(t, raid))

def brew_jobs.inputs_match(+inputs: List<&2, J.Val>, +ids: List<&2, J.Val>) -> Bool:
  match inputs ids:
    case Con{+c, +ct} Con{+id, +it}:
      Bool.and(String.eq(J.str(J.get(c, "id")), J.str(id)), brew_jobs.inputs_match(ct, it))
    case _ _:
      True{}

def brew_jobs.Work.valid(+work: J.Val) -> Bool:
  +ask = J.get(work, "ask")
  Bool.and(brew_jobs.valid_outbox(J.JA{[ask]}, J.get(ask, "raid")),
  Bool.and(brew_jobs.inputs_match(J.items(J.get(work, "inputs")), J.items(J.get(ask, "cards"))),
  Bool.and(String.eq(J.str(J.get(J.get(work, "placeholder"), "id")), J.str(J.get(ask, "placeholder"))),
  Bool.and(brew_jobs.id(J.str(J.get(work, "result_id"))),
  Bool.and(home.util.blen_le(J.str(J.get(work, "player_name")), 1024),
    jobs.bounded(work, 262144))))))

def brew_jobs.PolicyRequest.validate(+req: J.Val) -> Maybe<&2, String>:
  jobs.refuse(Bool.and(brew_jobs.Work.valid(J.get(req, "work")), Bool.and(U32.is_le(J.len(J.get(req, "taken")), 4096), jobs.bounded(req, 1048576))), "invalid brew policy inputs")

# the card re-checked from its file is the card
def brew_jobs.same_card(+card: J.Val, +r: J.Val) -> Bool:
  home.util.ite(Bool, J.has(r, "Ok"), u => J.eq(J.get(J.get(r, "Ok"), "card"), card), u => False{})

# `result` is the PolicyResult as serialised: {"Ok": card} or {"Err": [text]}
def brew_jobs.PolicyRequest.validate_result(+req: J.Val, +result: J.Val) -> Maybe<&2, String>:
  jobs.refuse(
    Bool.and(
      home.util.ite(Bool, J.has(result, "Ok"),
        u => Bool.and(String.eq(J.str(J.get(J.get(result, "Ok"), "id")), J.str(J.get(J.get(req, "work"), "result_id"))),
          Bool.and(String.eq(J.str(J.get(J.get(result, "Ok"), "kind")), "spell"),
            brew_jobs.same_card(J.get(result, "Ok"), cards.check(J.str(J.get(J.get(result, "Ok"), "id")), cards.Card.file(J.get(result, "Ok")))))),
        u => jobs.errors_ok(J.items(J.get(result, "Err")))),
      jobs.bounded(result, 262144)),
    "invalid brew policy result")

# Answers {"Ok": PolicyResult} or {"Err": text}.
def brew_jobs.PolicyRequest.evaluate(+req: J.Val) -> J.Val:
  brew_jobs.PolicyRequest.evaluate.card(req, J.set(J.get(req, "card"), "id", J.get(J.get(req, "work"), "result_id")))

def brew_jobs.PolicyRequest.evaluate.card(+req: J.Val, +card: J.Val) -> J.Val:
  brew_jobs.PolicyRequest.evaluate.faults(req, card, cauldron.faults(card, home.util.get(J.items(J.get(J.get(req, "work"), "inputs")), 0), home.util.get(J.items(J.get(J.get(req, "work"), "inputs")), 1), J.get(req, "taken")))

def brew_jobs.PolicyRequest.evaluate.faults(+req: J.Val, +card: J.Val, +errors: List<&2, String>) -> J.Val:
  brew_jobs.PolicyRequest.evaluate.fin(req, home.util.ite(J.Val, List.is_empty(&2, String, errors), u => home.util.ok(card), u => home.util.err_list(errors)))

def brew_jobs.PolicyRequest.evaluate.fin(+req: J.Val, +result: J.Val) -> J.Val:
  brew_jobs.PolicyRequest.evaluate.checked(brew_jobs.PolicyRequest.validate_result(req, result), result)

def brew_jobs.PolicyRequest.evaluate.checked(+m: Maybe<&2, String>, +result: J.Val) -> J.Val:
  match m:
    case Some{+why}:
      home.util.err(why)
    case None{}:
      home.util.ok(result)

# ---- reforge_jobs.rs --------------------------------------------------------------

def reforge.part_kind(+part: String) -> String:
  Bool.pick(String, String.eq(part, "offense"), "arm", "guard")

def reforge.part_word(+part: String) -> String:
  Bool.pick(String, String.eq(part, "offense"), "offence", "defence")

def reforge.kit_card(+kit: J.Val) -> J.Val:
  home.util.ite(J.Val, String.eq(J.str(J.get(kit, "part")), "offense"), u => J.get(kit, "arm"), u => J.get(kit, "card"))

def reforge_jobs.Work.valid(+w: J.Val) -> Bool:
  Bool.and(home.util.rng(J.u32(J.get(w, "player")), 1, 6),
  Bool.and(U32.is_gt(J.u32(J.get(w, "depth")), 0),
  Bool.and(Bool.not(home.util.is_blank(J.str(J.get(w, "words")))),
  Bool.and(home.util.blen_le(J.str(J.get(w, "words")), 4096),
  Bool.and(home.util.blen_le(J.str(J.get(w, "player_name")), 1024),
  Bool.and(String.starts_with(J.str(J.get(w, "result_id")), "reforge-"),
  Bool.and(home.util.blen_le(J.str(J.get(w, "result_id")), 128),
  Bool.and(home.util.all_in(J.str(J.get(w, "result_id")), [48, 57, 65, 90, 97, 122], "-"),
  Bool.and(String.eq(J.str(J.get(J.get(w, "kit"), "part")), J.str(J.get(w, "part"))),
  Bool.and(reforge_jobs.original_ok(J.get(w, "original"), reforge.kit_card(J.get(w, "kit"))),
    jobs.bounded(w, 262144)))))))))))

def reforge_jobs.original_ok(+original: J.Val, +kit_card: J.Val) -> Bool:
  match original:
    case J.JNull{}:
      True{}
    case _:
      Bool.and(Bool.not(J.is_null(kit_card)), String.eq(J.str(J.get(original, "id")), J.str(kit_card)))

def reforge_jobs.PolicyRequest.validate(+req: J.Val) -> Maybe<&2, String>:
  jobs.refuse(Bool.and(reforge_jobs.Work.valid(J.get(req, "work")), jobs.bounded(req, 524288)), "invalid reforge policy inputs")

def reforge_jobs.same_card(+card: J.Val, +r: J.Val) -> Bool:
  home.util.ite(Bool, J.has(r, "Ok"), u => J.eq(J.get(J.get(r, "Ok"), "card"), card), u => False{})

def reforge_jobs.PolicyRequest.validate_result(+req: J.Val, +result: J.Val) -> Maybe<&2, String>:
  jobs.refuse(
    Bool.and(
      home.util.ite(Bool, J.has(result, "Ok"),
        u => Bool.and(String.eq(J.str(J.get(J.get(result, "Ok"), "id")), J.str(J.get(J.get(req, "work"), "result_id"))),
          Bool.and(String.eq(J.str(J.get(J.get(result, "Ok"), "kind")), reforge.part_kind(J.str(J.get(J.get(req, "work"), "part")))),
            reforge_jobs.same_card(J.get(result, "Ok"), cards.check(J.str(J.get(J.get(result, "Ok"), "id")), cards.Card.definition_file(J.get(result, "Ok")))))),
        u => jobs.errors_ok(J.items(J.get(result, "Err")))),
      jobs.bounded(result, 262144)),
    "invalid reforge policy result")

def reforge_jobs.PolicyRequest.evaluate(+req: J.Val) -> J.Val:
  reforge_jobs.PolicyRequest.evaluate.card(req, J.set(J.get(req, "card"), "id", J.get(J.get(req, "work"), "result_id")))

def reforge_jobs.PolicyRequest.evaluate.card(+req: J.Val, +card: J.Val) -> J.Val:
  reforge_jobs.PolicyRequest.evaluate.fin(req,
    home.util.ite(J.Val, Bool.not(String.eq(J.str(J.get(card, "kind")), reforge.part_kind(J.str(J.get(J.get(req, "work"), "part"))))),
      u => home.util.err_list(["a " ++ reforge.part_word(J.str(J.get(J.get(req, "work"), "part"))) ++ " reforge needs kind " ++ reforge.part_kind(J.str(J.get(J.get(req, "work"), "part")))]),
      u => home.util.ok(card)))

def reforge_jobs.PolicyRequest.evaluate.fin(+req: J.Val, +result: J.Val) -> J.Val:
  brew_jobs.PolicyRequest.evaluate.checkedr(reforge_jobs.PolicyRequest.validate_result(req, result), result)

def brew_jobs.PolicyRequest.evaluate.checkedr(+m: Maybe<&2, String>, +result: J.Val) -> J.Val:
  match m:
    case Some{+why}:
      home.util.err(why)
    case None{}:
      home.util.ok(result)
