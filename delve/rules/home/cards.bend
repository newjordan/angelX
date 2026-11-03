# crates/delve/src/rules/cards.rs — the card file checker (`check`), the card
# as its file (`file`, `definition_file`), and the caps a held deck keeps.
# A Card is the J value in its serde shape. A `Result<T, E>` here is a J
# {"Ok": t} / {"Err": e}; a `Result<(), E>` a `Maybe<&2, E>` (None is Ok).

# ---- slug, safe-for-work ----------------------------------------------------

def cards.slug(+name: String) -> String:
  home.util.take_chars(String.reverse(home.util.trim_start_c(String.reverse(cards.slug.go(String.to_list(name), "")), 45)), 32)

def cards.slug.go(+cs: List<&2, Char>, +out: String) -> String:
  match cs:
    case Nil{}:
      out
    case Con{+h, +t}:
      cards.slug.step(Char.to_u32(h), t, out)

def cards.slug.step(+c: U32, +t: List<&2, Char>, +out: String) -> String:
  home.util.ite(String, home.util.c_alnum(c),
    u => cards.slug.go(t, String.append(out, SCon{Char.from_u32(home.util.c_lower_of(c)), SNil{}})),
    u => home.util.ite(String, Bool.and(Bool.not(String.ends_with(out, "-")), Bool.not(String.is_empty(out))),
      u => cards.slug.go(t, String.append(out, "-")),
      u => cards.slug.go(t, out)))

def cards.rot13.char(+c: U32) -> U32:
  home.util.ite(U32, home.util.rng(c, 97, 109), u => (c + 13 : U32),
    u => home.util.ite(U32, home.util.rng(c, 110, 122), u => (c - 13 : U32), u => c))

def cards.rot13.go(+cs: List<&2, Char>) -> List<&2, Char>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      Char.from_u32(cards.rot13.char(Bool.pick(U32, home.util.c_upper(Char.to_u32(h)), (Char.to_u32(h) + 32 : U32), Char.to_u32(h)))) <> cards.rot13.go(t)

def cards.rot13(+w: String) -> String:
  String.from_list(cards.rot13.go(String.to_list(w)))

def cards.NOT_FOR_WORK() -> List<&2, String>:
  ["frk", "frkl", "frkhny", "cravf", "cravfrf", "qvpx", "qvpxf", "pbpx", "pbpxf", "fpuybat", "crpxre", "jrvare", "jvrare", "grfgvpyr", "grfgvpyrf", "fpebghz", "onyyfnpx", "ahgfnpx", "obbo", "obbof", "obbovrf", "gvgf", "gvggl", "gvggvrf", "avccyr", "avccyrf", "intvan", "chffl", "pyvg", "fcrez", "frzra", "phz", "wvmm", "betnfz", "ubeal", "rerpgvba", "obare", "qvyqb", "ivoengbe", "pbaqbz", "cbea", "cbeab", "ahqr", "ahqrf", "fgevccre", "ubbxre", "juber", "fyhg", "zvys", "uragnv", "afsj", "nany", "nahf", "ohggubyr", "nffubyr", "cbjreobggbz", "oqfz", "srgvfu", "xvaxl", "yvatrevr", "cnagvrf", "gubat", "travgny", "travgnyf", "znfgheongr", "jnax", "oybjwbo", "unaqwbo", "evzwbo", "shpx", "shpxvat", "shpxre", "zbgureshpxre", "fuvg", "fuvggl", "ohyyfuvg", "ovgpu", "ovgpurf", "phag", "avttre", "avttn", "snttbg", "snt", "ergneq", "ergneqrq", "genaal", "fcvp", "puvax", "xvxr", "qlxr"]

# alphanumeric runs of the text
def cards.words_of(+s: String) -> List<&2, String>:
  cards.words_of.go(String.to_list(s), "", Nil{})

def cards.words_of.go(+cs: List<&2, Char>, +cur: String, +acc: List<&2, String>) -> List<&2, String>:
  match cs:
    case Nil{}:
      List.reverse(&2, String, home.util.ite(List<&2, String>, String.is_empty(cur), u => acc, u => cur <> acc))
    case Con{+h, +t}:
      home.util.ite(List<&2, String>, home.util.c_alnum_u(Char.to_u32(h)),
        u => cards.words_of.go(t, String.append(cur, SCon{h, SNil{}}), acc),
        u => cards.words_of.go(t, "", home.util.ite(List<&2, String>, String.is_empty(cur), u => acc, u => cur <> acc)))

def cards.not_for_work(+text: String) -> Maybe<&2, String>:
  cards.not_for_work.go(cards.words_of(text))

def cards.not_for_work.go(+ws: List<&2, String>) -> Maybe<&2, String>:
  match ws:
    case Nil{}:
      None{}
    case Con{+w, +t}:
      home.util.ite(Maybe<&2, String>, home.util.has_name(cards.NOT_FOR_WORK(), cards.rot13(w)),
        u => Some{home.util.lower(w)},
        u => cards.not_for_work.go(t))

# ---- script::read (the card file's lines, art and draw blocks) ----------------

def cards.ink_char(+c: Char) -> Bool:
  Bool.or(U32.is_eq(Char.to_u32(c), 46), String.contains("kKZXgjGJhiHfDFEelNmLAMyCYnbIBpPrRoOtTsxSuUvVWqQz0123wa4@56$c987", SCon{c, SNil{}}))

def cards.art_row(+s: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(s)), cards.all_inks(String.to_list(s), False{}))

def cards.sprite_row(+s: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(s)), cards.all_inks(String.to_list(s), True{}))

def cards.all_inks(+cs: List<&2, Char>, +bar: Bool) -> Bool:
  match cs:
    case Nil{}:
      True{}
    case Con{+h, +t}:
      Bool.and(Bool.or(cards.ink_char(h), Bool.and(bar, U32.is_eq(Char.to_u32(h), 124))), cards.all_inks(t, bar))

# {lines: [{n, word, rest}], art: [..], art_blocks, sprites: [{n, header, rows}], text_bytes}
def cards.script.read(+raw: String) -> J.Val:
  cards.script.go(home.util.lines(raw), 1, home.util.obj([("lines", J.JA{Nil{}}), ("art", J.JA{Nil{}}), ("art_blocks", J.JU{0}), ("sprites", J.JA{Nil{}}), ("text_bytes", J.JU{0}), ("in_art", J.JBool{False{}}), ("in_sprite", J.JBool{False{}})]))

def cards.script.go(+ls: List<&2, String>, +n: U32, +st: J.Val) -> J.Val:
  match ls:
    case Nil{}:
      st
    case Con{+l, +t}:
      cards.script.go(t, (n + 1 : U32), cards.script.line(st, n, home.util.trim(l)))

def cards.script.line(+st: J.Val, +n: U32, +line: String) -> J.Val:
  home.util.ite(J.Val, Bool.and(J.bool(J.get(st, "in_sprite")), Bool.and(Bool.not(String.eq(line, "art")), cards.sprite_row(line))),
    u => cards.script.sprite_row(st, line),
    u => cards.script.text(J.set(st, "in_sprite", J.JBool{False{}}), n, line))

def cards.script.sprite_row(+st: J.Val, +line: String) -> J.Val:
  J.set(st, "sprites", cards.script.add_row(J.get(st, "sprites"), line))

def cards.script.add_row(+sprites: J.Val, +line: String) -> J.Val:
  cards.script.add_row.of(J.items(sprites), line)

def cards.script.add_row.of(+xs: List<&2, J.Val>, +line: String) -> J.Val:
  cards.script.add_row.rev(List.reverse(&2, J.Val, xs), line)

def cards.script.add_row.rev(+xs: List<&2, J.Val>, +line: String) -> J.Val:
  match xs:
    case Con{+last, +rest}:
      J.JA{List.reverse(&2, J.Val, J.set(last, "rows", J.push(J.get(last, "rows"), J.JS{line})) <> rest)}
    case Nil{}:
      J.JA{Nil{}}

def cards.script.text(+st: J.Val, +n: U32, +line: String) -> J.Val:
  cards.script.text.art(J.set(st, "text_bytes", J.JU{(J.u32(J.get(st, "text_bytes")) + home.util.blen(line) + 1 : U32)}), n, line)

def cards.script.text.art(+st: J.Val, +n: U32, +line: String) -> J.Val:
  home.util.ite(J.Val, Bool.and(J.bool(J.get(st, "in_art")), cards.art_row(line)),
    u => J.set(st, "art", J.push(J.get(st, "art"), J.JS{line})),
    u => cards.script.text.word(J.set(st, "in_art", J.JBool{False{}}), n, line))

def cards.script.text.word(+st: J.Val, +n: U32, +line: String) -> J.Val:
  home.util.ite(J.Val, Bool.or(String.is_empty(line), Bool.or(String.starts_with(line, "#"), String.starts_with(line, "```"))),
    u => st,
    u => cards.script.split(st, n, home.util.split_ws1(line)))

def cards.script.split(+st: J.Val, +n: U32, +wr: Sigma<&2, &2, String, _ => String>) -> J.Val:
  match wr:
    case (+word, +rest):
      cards.script.kind(st, n, home.util.ascii_lower(word), rest)

def cards.script.kind(+st: J.Val, +n: U32, +word: String, +rest: String) -> J.Val:
  home.util.ite(J.Val, String.eq(word, "art"),
    u => J.set(J.set(st, "in_art", J.JBool{True{}}), "art_blocks", J.JU{(J.u32(J.get(st, "art_blocks")) + 1 : U32)}),
    u => home.util.ite(J.Val, String.eq(word, "draw"),
      u => J.set(J.set(st, "in_sprite", J.JBool{True{}}), "sprites", J.push(J.get(st, "sprites"), home.util.obj([("n", J.JU{n}), ("header", J.JS{home.util.ascii_lower(home.util.trim(rest))}), ("rows", J.JA{Nil{}})]))),
      u => J.set(st, "lines", J.push(J.get(st, "lines"), home.util.obj([("n", J.JU{n}), ("word", J.JS{word}), ("rest", J.JS{home.util.trim(home.util.before(rest, " #"))})])))))

# script::clean: no control characters, at most `max` chars
def cards.clean(+text: String, +max: U32) -> String:
  home.util.take_chars(home.util.no_control(text), max)

# ---- check -------------------------------------------------------------------

def cards.MAX_FILE_BYTES() -> U32:
  24576

def cards.MAX_BYTES() -> U32:
  4096

def cards.ck.err(+st: J.Val, +msg: String) -> J.Val:
  J.set(st, "errors", J.push(J.get(st, "errors"), J.JS{msg}))

def cards.ck.note(+st: J.Val, +msg: String) -> J.Val:
  J.set(st, "notes", J.push(J.get(st, "notes"), J.JS{msg}))

def cards.ck.fail(+errors: J.Val) -> J.Val:
  J.set(J.obj(), "Err", errors)

def cards.ck.start() -> J.Val:
  home.util.obj([("errors", J.JA{Nil{}}), ("notes", J.JA{Nil{}}), ("name", J.JNull{}), ("kind", J.JNull{}), ("rarity", J.JS{"common"}), ("text", J.JS{""}), ("by", J.JS{""}), ("drop", J.JNull{}), ("chest", J.JNull{}), ("effects", J.JA{Nil{}}), ("runes", J.JA{Nil{}}), ("plain", J.JNull{}), ("only_in", J.JNull{}), ("guard", J.JNull{}), ("ult", J.JNull{}), ("cooldown", J.JNull{}), ("style", J.JNull{}), ("looks", J.obj())])

# Check one card file. `id` usually comes from the file name. Answers
# {"Ok": {"card": card, "notes": [..]}} or {"Err": [errors]}.
def cards.check(+id: String, +raw: String) -> J.Val:
  home.util.ite(J.Val, U32.is_gt(home.util.blen(raw), 24576),
    u => cards.ck.fail(J.JA{[J.JS{"the file is over 24576 bytes with its sprites"}]}),
    u => cards.check.script(cards.slug(id), cards.script.read(raw)))

def cards.check.script(+id: String, +script: J.Val) -> J.Val:
  home.util.ite(J.Val, U32.is_gt(J.u32(J.get(script, "text_bytes")), 4096),
    u => cards.ck.fail(J.JA{[J.JS{"the card's lines are over 4096 bytes (sprite rows aside)"}]}),
    u => cards.check.sprites(id, script, home.util.ite(J.Val, U32.is_gt(J.u32(J.get(script, "art_blocks")), 1), u => cards.ck.err(cards.ck.start(), "only one art block"), u => cards.ck.start())))

def cards.check.sprites(+id: String, +script: J.Val, +st: J.Val) -> J.Val:
  cards.check.lines(id, script, cards.ck.sprites(st, J.items(J.get(script, "sprites"))))

def cards.check.lines(+id: String, +script: J.Val, +st: J.Val) -> J.Val:
  cards.check.finish(id, J.get(script, "art"), cards.ck.lines(st, J.items(J.get(script, "lines"))))

def cards.ck.lines(+st: J.Val, +ls: List<&2, J.Val>) -> J.Val:
  match ls:
    case Nil{}:
      st
    case Con{+l, +t}:
      cards.ck.lines(cards.ck.line(st, J.u32(J.get(l, "n")), J.str(J.get(l, "word")), J.str(J.get(l, "rest"))), t)

def cards.ck.eff(+n: U32, +word: String, +eff: J.Val) -> J.Val:
  home.util.obj([("n", J.JU{n}), ("word", J.JS{word}), ("eff", eff)])

def cards.ck.push_eff(+st: J.Val, +n: U32, +word: String, +eff: J.Val) -> J.Val:
  J.set(st, "effects", J.push(J.get(st, "effects"), cards.ck.eff(n, word, eff)))

# The kinds of line a card file has, by its first word.
def cards.ck.line(+st: J.Val, +n: U32, +word: String, +rest: String) -> J.Val:
  home.util.ite(J.Val, String.eq(word, "name"), u => J.set(st, "name", J.JS{cards.clean(rest, 64)}),
  u => home.util.ite(J.Val, String.eq(word, "text"), u => J.set(st, "text", J.JS{cards.clean(rest, 256)}),
  u => home.util.ite(J.Val, String.eq(word, "by"), u => J.set(st, "by", J.JS{cards.clean(rest, 24)}),
  u => home.util.ite(J.Val, String.eq(word, "kind"), u => cards.ck.kind(st, n, home.util.ascii_lower(rest)),
  u => home.util.ite(J.Val, String.eq(word, "cooldown"), u => cards.ck.cooldown(st, n, rest),
  u => home.util.ite(J.Val, String.eq(word, "cast"), u => cards.ck.cast(st, n, home.util.ascii_lower(rest)),
  u => home.util.ite(J.Val, String.eq(word, "rarity"), u => cards.ck.rarity(st, n, home.util.ascii_lower(rest)),
  u => home.util.ite(J.Val, String.eq(word, "where"), u => cards.ck.where(st, n, home.util.ascii_lower(rest)),
  u => home.util.ite(J.Val, Bool.or(String.eq(word, "drop"), String.eq(word, "chest")), u => cards.ck.weight(st, n, word, rest),
  u => home.util.ite(J.Val, String.eq(word, "arm"), u => cards.ck.arm(st, n, home.util.ascii_lower(rest)),
  u => home.util.ite(J.Val, Bool.or(Bool.or(String.eq(word, "bolt"), String.eq(word, "spread")), Bool.or(String.eq(word, "melee"), String.eq(word, "throw"))), u => J.set(st, "runes", J.push(J.get(st, "runes"), J.JS{word ++ " " ++ rest})),
  u => home.util.ite(J.Val, String.eq(word, "look"), u => J.set(J.set(st, "runes", J.push(J.get(st, "runes"), J.JS{word ++ " " ++ rest})), "style", home.util.obj([("n", J.JU{n}), ("rest", J.JS{home.util.ascii_lower(rest)})])),
  u => home.util.ite(J.Val, String.eq(word, "sheet"), u => cards.ck.err(st, "line " ++ U32.show(n) ++ ": `sheet` names a picture in .angel/dungeon/sheets; it is cut into a `draw` block when the card loads from the workspace — a card sent on its own draws its sprite as rows"),
  u => home.util.ite(J.Val, String.eq(word, "summon"), u => cards.ck.summon(st, n, home.util.ascii_lower(rest)),
  u => home.util.ite(J.Val, String.eq(word, "ult"), u => cards.ck.ult(st, n, rest),
  u => home.util.ite(J.Val, String.eq(word, "guard"), u => cards.ck.guard(st, n, home.util.ascii_lower(rest)),
  u => cards.ck.spec(st, n, word, rest, cards.SPECS())))))))))))))))))

def cards.ck.kind(+st: J.Val, +n: U32, +lower: String) -> J.Val:
  home.util.ite(J.Val, home.util.has_name(["take", "hold", "play", "spell", "arm", "guard"], lower),
    u => J.set(st, "kind", J.JS{lower}),
    u => J.set(cards.ck.err(st, "line " ++ U32.show(n) ++ ": kind `" ++ lower ++ "` — use take, hold, play, arm, guard or spell"), "kind", J.JNull{}))

def cards.ck.cooldown(+st: J.Val, +n: U32, +rest: String) -> J.Val:
  cards.ck.cooldown.parse(home.util.ite(J.Val, Bool.not(J.is_null(J.get(st, "cooldown"))), u => cards.ck.err(st, "line " ++ U32.show(n) ++ ": cooldown appears twice"), u => st), n, home.util.parse_u32(home.util.trim_end_c(rest, 115)))

def cards.ck.cooldown.parse(+st: J.Val, +n: U32, +v: Maybe<&2, U32>) -> J.Val:
  match v:
    case Some{+value}:
      cards.ck.cooldown.set(st, value, U32.clamp(value, 1, 120))
    case None{}:
      cards.ck.err(st, "line " ++ U32.show(n) ++ ": cooldown takes whole seconds 1–120")

def cards.ck.cooldown.set(+st: J.Val, +value: U32, +clamped: U32) -> J.Val:
  J.set(home.util.ite(J.Val, Bool.not(U32.is_eq(clamped, value)), u => cards.ck.note(st, "cooldown " ++ U32.show(value) ++ " clamped to " ++ U32.show(clamped) ++ "s"), u => st), "cooldown", J.JU{clamped})

def cards.ck.cast(+st: J.Val, +n: U32, +lower: String) -> J.Val:
  home.util.ite(J.Val, home.util.has_name(["bolt", "ring", "meteor"], lower),
    u => cards.ck.push_eff(st, n, "cast", home.util.obj([("cast", J.JS{lower})])),
    u => cards.ck.err(st, "line " ++ U32.show(n) ++ ": cast takes bolt, ring or meteor"))

def cards.ck.rarity(+st: J.Val, +n: U32, +lower: String) -> J.Val:
  home.util.ite(J.Val, home.util.has_name(["common", "rare", "relic"], lower),
    u => J.set(st, "rarity", J.JS{lower}),
    u => cards.ck.err(st, "line " ++ U32.show(n) ++ ": rarity `" ++ lower ++ "` — use common, rare or relic"))

def cards.ck.where(+st: J.Val, +n: U32, +lower: String) -> J.Val:
  home.util.ite(J.Val, home.util.has_name(["crypt", "the crypt"], lower), u => J.set(st, "only_in", J.JS{"crypt"}),
  u => home.util.ite(J.Val, home.util.has_name(["mines", "the mines"], lower), u => J.set(st, "only_in", J.JS{"cavern"}),
  u => home.util.ite(J.Val, home.util.has_name(["keep", "dragon keep"], lower), u => J.set(st, "only_in", J.JS{"hellforge"}),
  u => home.util.ite(J.Val, home.util.has_name(["any", "anywhere"], lower), u => J.set(st, "only_in", J.JNull{}),
  u => cards.ck.err(st, "line " ++ U32.show(n) ++ ": where `" ++ lower ++ "` — use crypt, mines, keep or anywhere")))))

def cards.ck.weight(+st: J.Val, +n: U32, +word: String, +rest: String) -> J.Val:
  cards.ck.weight.parse(st, n, word, home.util.parse_u32(rest))

def cards.ck.weight.parse(+st: J.Val, +n: U32, +word: String, +v: Maybe<&2, U32>) -> J.Val:
  match v:
    case Some{+value}:
      cards.ck.weight.set(st, word, value, U32.min(value, 100))
    case None{}:
      cards.ck.err(st, "line " ++ U32.show(n) ++ ": " ++ word ++ " takes a weight 0–100")

def cards.ck.weight.set(+st: J.Val, +word: String, +value: U32, +clamped: U32) -> J.Val:
  J.set(home.util.ite(J.Val, Bool.not(U32.is_eq(clamped, value)), u => cards.ck.note(st, word ++ " " ++ U32.show(value) ++ " clamped to 100"), u => st), word, J.JU{clamped})

def cards.ck.arm(+st: J.Val, +n: U32, +lower: String) -> J.Val:
  home.util.ite(J.Val, home.util.has_name(["bow", "crossbow", "handgonne", "notes", "stones"], lower),
    u => J.set(st, "plain", J.JS{lower}),
    u => cards.ck.err(st, "line " ++ U32.show(n) ++ ": arm `" ++ lower ++ "` — use bow, crossbow, handgonne or notes, or Forge rune lines"))

# `summon N [swarm|guard|turret]`
def cards.ck.summon(+st: J.Val, +n: U32, +lower: String) -> J.Val:
  cards.ck.summon.args(st, n, home.util.words(lower))

def cards.ck.summon.args(+st: J.Val, +n: U32, +ws: List<&2, String>) -> J.Val:
  match ws:
    case Con{+c, +t}:
      cards.ck.summon.count(st, n, home.util.parse_u32(c), t)
    case Nil{}:
      cards.ck.err(st, "line " ++ U32.show(n) ++ ": summon takes a count and swarm, guard or turret (`summon 4 swarm`)")

def cards.ck.summon.count(+st: J.Val, +n: U32, +count: Maybe<&2, U32>, +rest: List<&2, String>) -> J.Val:
  match rest:
    case Nil{}:
      cards.ck.summon.done(st, n, count, Some{"swarm"}, True{})
    case Con{+w, +t}:
      cards.ck.summon.done(st, n, count, imps.Behaviour.parse(w), List.is_empty(&2, String, t))

# Behaviour::parse
def imps.Behaviour.parse(+w: String) -> Maybe<&2, String>:
  home.util.ite(Maybe<&2, String>, home.util.has_name(["swarm", "swarmer", "swarmers"], w), u => Some{"swarm"},
  u => home.util.ite(Maybe<&2, String>, home.util.has_name(["guard", "guards"], w), u => Some{"guard"},
  u => home.util.ite(Maybe<&2, String>, home.util.has_name(["turret", "turrets"], w), u => Some{"turret"}, u => None{})))

def imps.Behaviour.most(+b: String) -> U32:
  Bool.pick(U32, String.eq(b, "swarm"), 6, Bool.pick(U32, String.eq(b, "guard"), 4, 2))

def imps.Behaviour.seconds_each(+b: String) -> U32:
  Bool.pick(U32, String.eq(b, "swarm"), 4, Bool.pick(U32, String.eq(b, "guard"), 5, 10))

def cards.ck.summon.done(+st: J.Val, +n: U32, +count: Maybe<&2, U32>, +b: Maybe<&2, String>, +last: Bool) -> J.Val:
  match count:
    case Some{+c}:
      cards.ck.summon.done2(st, n, c, b, last)
    case None{}:
      cards.ck.err(st, "line " ++ U32.show(n) ++ ": summon takes a count and swarm, guard or turret (`summon 4 swarm`)")

def cards.ck.summon.done2(+st: J.Val, +n: U32, +c: U32, +b: Maybe<&2, String>, +last: Bool) -> J.Val:
  match b:
    case Some{+beh}:
      home.util.ite(J.Val, last,
        u => cards.ck.summon.set(st, n, c, beh, U32.clamp(c, 1, imps.Behaviour.most(beh))),
        u => cards.ck.err(st, "line " ++ U32.show(n) ++ ": summon takes a count and swarm, guard or turret (`summon 4 swarm`)"))
    case None{}:
      cards.ck.err(st, "line " ++ U32.show(n) ++ ": summon takes a count and swarm, guard or turret (`summon 4 swarm`)")

def cards.ck.summon.set(+st: J.Val, +n: U32, +count: U32, +beh: String, +clamped: U32) -> J.Val:
  cards.ck.push_eff(
    home.util.ite(J.Val, Bool.not(U32.is_eq(clamped, count)), u => cards.ck.note(st, "summon " ++ U32.show(count) ++ " " ++ beh ++ " clamped to " ++ U32.show(clamped)), u => st),
    n, "summon", home.util.obj([("summon", J.JA{[J.JU{clamped}, J.JS{beh}]})]))

def cards.ck.ult(+st: J.Val, +n: U32, +rest: String) -> J.Val:
  cards.ck.ult.found(st, n, rest, ults.Ult.parse(rest))

def cards.ck.ult.found(+st: J.Val, +n: U32, +rest: String, +found: J.Val) -> J.Val:
  match found:
    case J.JNull{}:
      cards.ck.err(st, "line " ++ U32.show(n) ++ ": ult `" ++ rest ++ "` — use " ++ ults.Ult.words())
    case _:
      home.util.ite(J.Val, J.is_null(J.get(st, "ult")), u => J.set(st, "ult", found), u => cards.ck.err(st, "line " ++ U32.show(n) ++ ": ult appears twice"))

# `guard roll|shield|spin|wall width=N drain=N empower=N`
def cards.ck.guard(+st: J.Val, +n: U32, +lower: String) -> J.Val:
  cards.ck.guard.kind(st, n, lower, home.util.words(lower))

def cards.ck.guard.kind(+st: J.Val, +n: U32, +lower: String, +ws: List<&2, String>) -> J.Val:
  match ws:
    case Con{+k, +t}:
      cards.ck.guard.which(st, n, lower, k)
    case Nil{}:
      cards.ck.guard.which(st, n, lower, "")

def cards.ck.guard.which(+st: J.Val, +n: U32, +lower: String, +k: String) -> J.Val:
  home.util.ite(J.Val, String.eq(k, "roll"), u => J.set(st, "guard", J.JS{"roll"}),
  u => home.util.ite(J.Val, String.eq(k, "shield"), u => J.set(st, "guard", J.JS{"shield"}),
  u => home.util.ite(J.Val, String.eq(k, "spin"), u => J.set(st, "guard", J.JS{"spin"}),
  u => home.util.ite(J.Val, String.eq(k, "wall"), u => cards.ck.wall(st, lower),
  u => cards.ck.err(st, "line " ++ U32.show(n) ++ ": guard `" ++ k ++ "` — roll, shield, spin, or wall width=2-8 drain=10-50 empower=0-40")))))

# the value of `key=` among the words, if any
def cards.ck.kv(+ws: List<&2, String>, +key: String) -> Maybe<&2, String>:
  match ws:
    case Nil{}:
      None{}
    case Con{+w, +t}:
      home.util.ite(Maybe<&2, String>, String.starts_with(w, key ++ "="), u => Some{String.drop(w, U32.to_nat((home.util.chars(key) + 1 : U32)))}, u => cards.ck.kv(t, key))

# `v.round().max(0.0) as u32` of the number written, else the default
def cards.ck.num(+raw: Maybe<&2, String>, +default: U32) -> U32:
  match raw:
    case Some{+s}:
      home.util.parse_round(home.util.trim_end_c(s, 37), default)
    case None{}:
      default

def cards.ck.wall(+st: J.Val, +lower: String) -> J.Val:
  cards.ck.wall.width(st, home.util.words(lower))

def cards.ck.wall.clamp(+st: J.Val, +key: String, +v: U32, +lo: U32, +hi: U32) -> J.Val & U32:
  (home.util.ite(J.Val, Bool.not(U32.is_eq(U32.clamp(v, lo, hi), v)), u => cards.ck.note(st, "wall " ++ key ++ " " ++ U32.show(v) ++ " clamped to " ++ U32.show(U32.clamp(v, lo, hi))), u => st), U32.clamp(v, lo, hi))

def cards.ck.wall.width(+st: J.Val, +ws: List<&2, String>) -> J.Val:
  cards.ck.wall.drain(cards.ck.wall.clamp(st, "width", cards.ck.num(cards.ck.kv(ws, "width"), 4), 2, 8), ws)

def cards.ck.wall.drain(+r: J.Val & U32, +ws: List<&2, String>) -> J.Val:
  match r:
    case (+st, +width):
      cards.ck.wall.empower(cards.ck.wall.clamp(st, "drain", cards.ck.num(cards.ck.kv(ws, "drain"), 25), 10, 50), ws, width)

def cards.ck.wall.empower(+r: J.Val & U32, +ws: List<&2, String>, +width: U32) -> J.Val:
  match r:
    case (+st, +drain):
      cards.ck.wall.raise(cards.ck.wall.clamp(st, "empower", cards.ck.num(cards.ck.kv(ws, "empower"), 0), 0, 40), width, drain)

def cards.ck.wall.raise(+r: J.Val & U32, +width: U32, +drain: U32) -> J.Val:
  match r:
    case (+st, +empower):
      cards.ck.wall.set(st, width, drain, empower, (width * 5 + U32.div(empower, 3) : U32))

def cards.ck.wall.set(+st: J.Val, +width: U32, +drain: U32, +empower: U32, +floor: U32) -> J.Val:
  home.util.ite(J.Val, U32.is_lt(drain, floor),
    u => J.set(cards.ck.note(st, "wall drain raised to " ++ U32.show(floor) ++ ": a wall that wide and strong costs more to hold"), "guard", cards.ck.wall.obj(width, U32.min(floor, 50), empower)),
    u => J.set(st, "guard", cards.ck.wall.obj(width, drain, empower)))

def cards.ck.wall.obj(+width: U32, +drain: U32, +empower: U32) -> J.Val:
  home.util.obj([("wall", home.util.obj([("width", J.JU{width}), ("drain", J.JU{drain}), ("empower", J.JU{empower})]))])

# The spec table's word, or the unknown-word refusal.
def cards.ck.spec(+st: J.Val, +n: U32, +word: String, +rest: String, +specs: List<&2, CardsSpec>) -> J.Val:
  match specs:
    case Nil{}:
      cards.ck.err(st, "line " ++ U32.show(n) ++ ": unknown word `" ++ word ++ "`; a card has name, kind, rarity, text, by, drop, chest, art, cooldown, cast, effects (" ++ home.util.join(cards.spec_words(cards.SPECS()), ", ") ++ "), and for arm cards `arm <weapon>` or Forge runes")
    case Con{+s, +t}:
      home.util.ite(J.Val, String.eq(cards.spec.word(s), word), u => cards.ck.spec.value(st, n, s, rest), u => cards.ck.spec(st, n, word, rest, t))

def cards.spec.word(+s: CardsSpec) -> String:
  match s:
    case CardsSpec{+word, +name, +low, +high, +cap, +hold, +spend}:
      word

def cards.spec_words(+specs: List<&2, CardsSpec>) -> List<&2, String>:
  match specs:
    case Nil{}:
      Nil{}
    case Con{+s, +t}:
      cards.spec.word(s) <> cards.spec_words(t)

def cards.ck.spec.value(+st: J.Val, +n: U32, +s: CardsSpec, +rest: String) -> J.Val:
  match s:
    case CardsSpec{+word, +name, +low, +high, +cap, +hold, +spend}:
      cards.ck.spec.parse(st, n, word, name, low, high, home.util.ite(Maybe<&2, U32>, Bool.and(home.util.is_blank(rest), U32.is_eq(low, high)), u => Some{low}, u => home.util.parse_u32(home.util.trim_end_set(home.util.trim_start_c(rest, 43)))))

# trim_end_matches(['%', 's'])
def home.util.trim_end_set(+s: String) -> String:
  String.reverse(home.util.trim_start_set(String.reverse(s)))

def home.util.trim_start_set(+s: String) -> String:
  match s:
    case SNil{}:
      ""
    case SCon{+h, +t}:
      home.util.ite(String, Bool.or(U32.is_eq(Char.to_u32(h), 37), U32.is_eq(Char.to_u32(h), 115)), u => home.util.trim_start_set(t), u => SCon{h, t})

def cards.ck.spec.parse(+st: J.Val, +n: U32, +word: String, +name: String, +low: U32, +high: U32, +v: Maybe<&2, U32>) -> J.Val:
  match v:
    case Some{+value}:
      cards.ck.spec.set(st, n, word, name, low, high, value, U32.clamp(value, low, high))
    case None{}:
      cards.ck.err(st, "line " ++ U32.show(n) ++ ": " ++ word ++ " takes a whole number " ++ U32.show(low) ++ "–" ++ U32.show(high))

def cards.ck.spec.set(+st: J.Val, +n: U32, +word: String, +name: String, +low: U32, +high: U32, +value: U32, +clamped: U32) -> J.Val:
  cards.ck.push_eff(
    home.util.ite(J.Val, Bool.not(U32.is_eq(clamped, value)), u => cards.ck.note(st, word ++ " " ++ U32.show(value) ++ " is outside " ++ U32.show(low) ++ "–" ++ U32.show(high) ++ ", clamped to " ++ U32.show(clamped)), u => st),
    n, word, home.util.obj([(name, J.JU{clamped})]))

# ---- the draw blocks ------------------------------------------------------------

# key=value split at the first `=`: (key, value, has `=`)
def home.util.split_eq(+s: String) -> Sigma<&2, &2, String, _ => Sigma<&2, &2, String, _ => Bool>>:
  home.util.split_eq.go(s, "")

def home.util.split_eq.go(+s: String, +acc: String) -> Sigma<&2, &2, String, _ => Sigma<&2, &2, String, _ => Bool>>:
  match s:
    case SNil{}:
      (acc, ("", False{}))
    case SCon{+h, +t}:
      home.util.ite(Sigma<&2, &2, String, _ => Sigma<&2, &2, String, _ => Bool>>, U32.is_eq(Char.to_u32(h), 61),
        u => (acc, (t, True{})),
        u => home.util.split_eq.go(t, String.append(acc, SCon{h, SNil{}})))

def cards.slot_word(+word: String) -> J.Val:
  home.util.ite(J.Val, home.util.has_name(["worn", "shot", "orbit", "aura"], word), u => J.JS{word},
    u => home.util.ite(J.Val, home.util.has_name(["summon", "foe"], word), u => J.JS{"summon"}, u => J.JNull{}))

def cards.ck.sprites(+st: J.Val, +sps: List<&2, J.Val>) -> J.Val:
  match sps:
    case Nil{}:
      st
    case Con{+sp, +t}:
      cards.ck.sprites(cards.ck.sprite(st, sp), t)

def cards.ck.sprite(+st: J.Val, +sp: J.Val) -> J.Val:
  cards.ck.sprite.opts(st, J.u32(J.get(sp, "n")), J.str(J.get(sp, "header")), J.get(sp, "rows"))

def cards.ck.sprite.opts(+st: J.Val, +n: U32, +header: String, +rows: J.Val) -> J.Val:
  cards.ck.sprite.head(st, n, header, rows, home.util.words(header))

def cards.ck.sprite.head(+st: J.Val, +n: U32, +header: String, +rows: J.Val, +ws: List<&2, String>) -> J.Val:
  match ws:
    case Con{+word, +parts}:
      cards.ck.sprite.checked(st, n, header, rows, word, cards.ck.opts(home.util.obj([("bad", J.JNull{})]), n, word, header, parts))
    case Nil{}:
      cards.ck.sprite.checked(st, n, header, rows, "", cards.ck.opts(home.util.obj([("bad", J.JNull{})]), n, "", header, Nil{}))

def cards.ck.opts(+h: J.Val, +n: U32, +word: String, +header: String, +parts: List<&2, String>) -> J.Val:
  match parts:
    case Nil{}:
      h
    case Con{+part, +t}:
      home.util.ite(J.Val, Bool.not(J.is_null(J.get(h, "bad"))), u => h, u => cards.ck.opts(cards.ck.opt(h, n, word, header, part, home.util.split_eq(part)), n, word, header, t))

def cards.ck.bad(+h: J.Val, +msg: String) -> J.Val:
  J.set(h, "bad", J.JS{msg})

# set key to v; a second set of the same key is the refusal
def cards.ck.put(+h: J.Val, +n: U32, +word: String, +key: String, +v: J.Val) -> J.Val:
  home.util.ite(J.Val, J.has(h, key),
    u => cards.ck.bad(h, "line " ++ U32.show(n) ++ ": `" ++ key ++ "=` appears twice on `draw " ++ word ++ "`"),
    u => J.set(h, key, v))

def cards.ck.opt(+h: J.Val, +n: U32, +word: String, +header: String, +part: String, +kv: Sigma<&2, &2, String, _ => Sigma<&2, &2, String, _ => Bool>>) -> J.Val:
  match kv:
    case (+key, (+value, +has_eq)):
      cards.ck.opt.key(h, n, word, header, part, key, value, has_eq)

def cards.ck.opt.key(+h: J.Val, +n: U32, +word: String, +header: String, +part: String, +key: String, +value: String, +has_eq: Bool) -> J.Val:
  home.util.ite(J.Val, Bool.and(String.eq(key, "at"), String.eq(word, "worn")), u => cards.ck.put(h, n, word, "at", J.JS{value}),
  u => home.util.ite(J.Val, Bool.and(String.eq(key, "placed"), Bool.and(String.eq(word, "worn"), Bool.not(home.util.opt_none(looks.place(value))))), u => cards.ck.put(h, n, word, "placed", J.JS{value}),
  u => home.util.ite(J.Val, String.eq(key, "paint"),
    u => home.util.ite(J.Val, looks.paint_name(value), u => cards.ck.put(h, n, word, "paint", J.JS{value}), u => cards.ck.bad(h, "line " ++ U32.show(n) ++ ": paint=`" ++ value ++ "` — a painting is named by its sha256, 64 hex digits")),
  u => home.util.ite(J.Val, String.eq(key, "normal"),
    u => home.util.ite(J.Val, looks.paint_name(value), u => cards.ck.put(h, n, word, "normal", J.JS{value}), u => cards.ck.bad(h, "line " ++ U32.show(n) ++ ": normal=`" ++ value ++ "` — a normal map is named by its sha256, 64 hex digits")),
  u => home.util.ite(J.Val, String.eq(key, "hd"),
    u => home.util.ite(J.Val, looks.paint_name(value), u => cards.ck.put(h, n, word, "hd", J.JS{value}), u => cards.ck.bad(h, "line " ++ U32.show(n) ++ ": hd=`" ++ value ++ "` — a painting is named by its sha256, 64 hex digits")),
  u => home.util.ite(J.Val, Bool.and(String.eq(key, "pixel"), Bool.and(String.is_empty(value), Bool.not(has_eq))), u => cards.ck.put(h, n, word, "pixel", J.JBool{True{}}),
  u => home.util.ite(J.Val, String.eq(key, "move"), u => cards.ck.opt.move(h, n, word, value, looks.Move.parse_list(value)),
  u => cards.ck.bad(h, "line " ++ U32.show(n) ++ ": `draw " ++ header ++ "` — a sprite line is `draw <slot>`, and may add `paint=<sha256>` (and `pixel`) and `move=<words>`; a worn one `at=<place>`"))))))))

def home.util.opt_none(+m: Maybe<&2, String>) -> Bool:
  match m:
    case Some{+s}:
      False{}
    case None{}:
      True{}

def cards.ck.opt.move(+h: J.Val, +n: U32, +word: String, +value: String, +r: J.Val) -> J.Val:
  home.util.ite(J.Val, J.has(r, "Ok"),
    u => cards.ck.put(h, n, word, "move", J.get(r, "Ok")),
    u => cards.ck.bad(h, "line " ++ U32.show(n) ++ ": " ++ J.str(J.get(r, "Err"))))

def cards.ck.sprite.checked(+st: J.Val, +n: U32, +header: String, +rows: J.Val, +word: String, +h: J.Val) -> J.Val:
  cards.ck.sprite.pair(st, n, rows, word, home.util.ite(J.Val, Bool.and(J.is_null(J.get(h, "bad")), Bool.and(J.has(h, "normal"), Bool.not(J.has(h, "paint")))),
    u => cards.ck.bad(h, "line " ++ U32.show(n) ++ ": `normal=` is a painting's, beside its `paint=`"),
    u => home.util.ite(J.Val, Bool.and(J.is_null(J.get(h, "bad")), Bool.and(J.has(h, "pixel"), Bool.not(J.has(h, "paint")))),
      u => cards.ck.bad(h, "line " ++ U32.show(n) ++ ": `pixel` is a painting's, beside its `paint=`"),
      u => h)))

def cards.ck.sprite.pair(+st: J.Val, +n: U32, +rows: J.Val, +word: String, +h: J.Val) -> J.Val:
  home.util.ite(J.Val, Bool.not(J.is_null(J.get(h, "bad"))),
    u => cards.ck.err(st, J.str(J.get(h, "bad"))),
    u => cards.ck.sprite.at(st, n, rows, word, h))

def cards.ck.strs(+a: J.Val) -> List<&2, String>:
  cards.ck.strs.go(J.items(a))

def cards.ck.strs.go(+xs: List<&2, J.Val>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      J.str(h) <> cards.ck.strs.go(t)

# a worn piece drawn small is put on the knight at the place named
def cards.ck.sprite.at(+st: J.Val, +n: U32, +rows: J.Val, +word: String, +h: J.Val) -> J.Val:
  home.util.ite(J.Val, J.has(h, "at"),
    u => cards.ck.sprite.placed(st, n, word, h, looks.Sprite.from_strip("worn", cards.ck.strs(rows))),
    u => cards.ck.sprite.slot(st, n, rows, word, h))

def cards.ck.sprite.placed(+st: J.Val, +n: U32, +word: String, +h: J.Val, +small: J.Val) -> J.Val:
  home.util.ite(J.Val, J.has(small, "Ok"),
    u => cards.ck.sprite.placed2(st, n, word, h, looks.place_on_body(J.get(J.get(small, "Ok"), "frames"), J.str(J.get(h, "at")))),
    u => cards.ck.err(st, "line " ++ U32.show(n) ++ ": " ++ J.str(J.get(small, "Err"))))

def cards.ck.sprite.placed2(+st: J.Val, +n: U32, +word: String, +h: J.Val, +r: J.Val) -> J.Val:
  home.util.ite(J.Val, J.has(r, "Ok"),
    u => cards.ck.sprite.slot(st, n, J.JA{home.util.tail(home.util.jstrs.go(home.util.lines(looks.Sprite.to_block(home.util.obj([("frames", J.get(r, "Ok"))]), "worn"))))}, word, h),
    u => cards.ck.err(st, "line " ++ U32.show(n) ++ ": " ++ J.str(J.get(r, "Err"))))

# skip(1)
def home.util.tail(+xs: List<&2, J.Val>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      t

def cards.ck.sprite.slot(+st: J.Val, +n: U32, +rows: J.Val, +word: String, +h: J.Val) -> J.Val:
  cards.ck.sprite.known(st, n, rows, word, h, cards.slot_word(word))

def cards.ck.sprite.known(+st: J.Val, +n: U32, +rows: J.Val, +word: String, +h: J.Val, +slot: J.Val) -> J.Val:
  match slot:
    case J.JNull{}:
      cards.ck.err(st, "line " ++ U32.show(n) ++ ": draw `" ++ word ++ "` — use worn, shot, orbit, summon or aura")
    case _:
      home.util.ite(J.Val, J.has(J.get(st, "looks"), J.str(slot)),
        u => cards.ck.err(st, "line " ++ U32.show(n) ++ ": draw " ++ J.str(slot) ++ " appears twice"),
        u => cards.ck.sprite.drawn(st, n, h, J.str(slot), looks.Sprite.from_strip(J.str(slot), cards.ck.strs(rows))))

def cards.ck.sprite.drawn(+st: J.Val, +n: U32, +h: J.Val, +slot: String, +r: J.Val) -> J.Val:
  home.util.ite(J.Val, J.has(r, "Ok"),
    u => cards.ck.sprite.fit(st, n, h, slot, J.get(r, "Ok")),
    u => cards.ck.err(st, "line " ++ U32.show(n) ++ ": " ++ J.str(J.get(r, "Err"))))

# the place a sprite sits at: `at`, else `placed`
def cards.ck.place_of(+h: J.Val) -> J.Val:
  home.util.ite(J.Val, J.has(h, "at"), u => J.get(h, "at"), u => J.get(h, "placed"))

def cards.ck.sprite.fit(+st: J.Val, +n: U32, +h: J.Val, +slot: String, +sprite: J.Val) -> J.Val:
  home.util.ite(J.Val, String.eq(slot, "worn"),
    u => cards.ck.sprite.fitted(st, n, h, slot, sprite, artistry.fit_worn(sprite, home.util.ite(String, J.is_null(cards.ck.place_of(h)), u => "whole", u => J.str(cards.ck.place_of(h))))),
    u => cards.ck.sprite.set(st, h, slot, sprite))

def cards.ck.sprite.fitted(+st: J.Val, +n: U32, +h: J.Val, +slot: String, +sprite: J.Val, +r: J.Val) -> J.Val:
  home.util.ite(J.Val, J.has(r, "Ok"),
    u => cards.ck.sprite.set(st, h, slot, J.get(r, "Ok")),
    u => cards.ck.err(st, "line " ++ U32.show(n) ++ ": " ++ J.str(J.get(r, "Err"))))

# Sprite{hd, paint, normal, pixel, moves, at, ..sprite} into the looks
def cards.ck.sprite.set(+st: J.Val, +h: J.Val, +slot: String, +sprite: J.Val) -> J.Val:
  J.set(st, "looks", J.set(J.get(st, "looks"), slot, cards.ck.sprite.dress(h, sprite)))

def cards.ck.sprite.dress(+h: J.Val, +s: J.Val) -> J.Val:
  home.util.set_if(Bool.not(J.is_null(cards.ck.place_canon(cards.ck.place_of(h)))),
    home.util.set_if(Bool.and(J.has(h, "move"), Bool.not(U32.is_eq(J.len(J.get(h, "move")), 0))),
      home.util.set_if(J.has(h, "pixel"),
        home.util.set_if(J.has(h, "normal"),
          home.util.set_if(J.has(h, "paint"),
            home.util.set_if(Bool.and(J.has(h, "hd"), Bool.not(J.has(h, "paint"))),
              cards.ck.clear(s),
              "hd", J.get(h, "hd")),
            "paint", J.get(h, "paint")),
          "normal", J.get(h, "normal")),
        "pixel", J.JBool{True{}}),
      "moves", J.get(h, "move")),
    "at", cards.ck.place_canon(cards.ck.place_of(h)))

# the sprite as from_strip made it, without the optional parts this overrides
def cards.ck.clear(+s: J.Val) -> J.Val:
  J.del(J.del(J.del(J.del(J.del(J.del(s, "hd"), "paint"), "normal"), "pixel"), "moves"), "at")

def cards.ck.place_canon(+p: J.Val) -> J.Val:
  match p:
    case J.JNull{}:
      J.JNull{}
    case _:
      home.util.jopt(looks.place(J.str(p)))

# ---- the checks that need the whole file -----------------------------------------

def cards.kind_is(+st: J.Val, +k: String) -> Bool:
  String.eq(J.str(J.get(st, "kind")), k)

def cards.kind_in(+st: J.Val, +ks: List<&2, String>) -> Bool:
  home.util.has_name(ks, J.str(J.get(st, "kind")))

def cards.check.finish(+id: String, +art: J.Val, +st: J.Val) -> J.Val:
  cards.fin.kind(cards.fin.text(cards.fin.nsfw(cards.fin.id(cards.fin.name(J.set(J.set(st, "id", J.JS{id}), "art", art))))))

# name: trimmed and cut to 22 characters
def cards.fin.name(+st: J.Val) -> J.Val:
  cards.fin.name.of(st, J.get(st, "name"))

def cards.fin.name.of(+st: J.Val, +name: J.Val) -> J.Val:
  match name:
    case J.JS{+s}:
      home.util.ite(J.Val, Bool.not(home.util.is_blank(s)),
        u => cards.fin.name.cut(st, home.util.trim(s)),
        u => cards.fin.name.none(st))
    case _:
      cards.fin.name.none(st)

def cards.fin.name.none(+st: J.Val) -> J.Val:
  J.set(cards.ck.err(st, "the card needs a `name`"), "name", J.JS{""})

def cards.fin.name.cut(+st: J.Val, +name: String) -> J.Val:
  J.set(home.util.ite(J.Val, U32.is_gt(home.util.chars(name), 22), u => cards.ck.note(st, "name cut to 22 characters"), u => st), "name", J.JS{home.util.take_chars(name, 22)})

# A card sent without a file takes its id from its name.
def cards.fin.id(+st: J.Val) -> J.Val:
  cards.fin.id.of(home.util.ite(J.Val, String.is_empty(J.str(J.get(st, "id"))), u => J.set(st, "id", J.JS{cards.slug(J.str(J.get(st, "name")))}), u => st))

def cards.fin.id.of(+st: J.Val) -> J.Val:
  home.util.ite(J.Val, Bool.and(String.is_empty(J.str(J.get(st, "id"))), Bool.not(String.is_empty(J.str(J.get(st, "name"))))),
    u => cards.ck.err(st, "the card's name needs a letter or digit"),
    u => st)

def cards.fin.nsfw(+st: J.Val) -> J.Val:
  cards.fin.nsfw.of(st, cards.nsfw3(J.str(J.get(st, "id")), J.str(J.get(st, "name")), J.str(J.get(st, "text"))))

def cards.nsfw3(+a: String, +b: String, +c: String) -> Maybe<&2, String>:
  cards.nsfw3.go(cards.not_for_work(a), b, c)

def cards.nsfw3.go(+m: Maybe<&2, String>, +b: String, +c: String) -> Maybe<&2, String>:
  match m:
    case Some{+w}:
      Some{w}
    case None{}:
      cards.nsfw3.go2(cards.not_for_work(b), c)

def cards.nsfw3.go2(+m: Maybe<&2, String>, +c: String) -> Maybe<&2, String>:
  match m:
    case Some{+w}:
      Some{w}
    case None{}:
      cards.not_for_work(c)

def cards.fin.nsfw.of(+st: J.Val, +m: Maybe<&2, String>) -> J.Val:
  match m:
    case Some{+word}:
      cards.ck.err(st, "`" ++ word ++ "` is not for this game: every card stays safe for work (a rude ask is made a harmless, funny stand-in)")
    case None{}:
      st

def cards.fin.text(+st: J.Val) -> J.Val:
  home.util.ite(J.Val, U32.is_gt(home.util.chars(J.str(J.get(st, "text"))), 64),
    u => J.set(cards.ck.note(st, "text cut to 64 characters"), "text", J.JS{home.util.take_chars(J.str(J.get(st, "text")), 64)}),
    u => st)

# No kind: the card is turned away at once.
def cards.fin.kind(+st: J.Val) -> J.Val:
  home.util.ite(J.Val, J.is_null(J.get(st, "kind")),
    u => cards.ck.fail(J.get(cards.ck.err(st, "the card needs a `kind`: take, hold, play, arm, guard or spell"), "errors")),
    u => cards.fin.hold(st))

# A held card's number counts no higher than the deck may add up to.
def cards.fin.hold(+st: J.Val) -> J.Val:
  cards.fin.looks(home.util.ite(J.Val, cards.kind_is(st, "hold"), u => cards.fin.hold.go(st, J.items(J.get(st, "effects")), Nil{}), u => st))

def cards.fin.hold.go(+st: J.Val, +es: List<&2, J.Val>, +acc: List<&2, J.Val>) -> J.Val:
  match es:
    case Nil{}:
      J.set(st, "effects", J.JA{List.reverse(&2, J.Val, acc)})
    case Con{+e, +t}:
      cards.fin.hold.eff(st, e, t, acc, cards.spec_of(J.str(J.get(e, "word")), cards.SPECS()), cards.eff_value(J.get(e, "eff")))

def cards.fin.hold.eff(+st: J.Val, +e: J.Val, +t: List<&2, J.Val>, +acc: List<&2, J.Val>, +spec: Maybe<&2, CardsSpec>, +value: U32) -> J.Val:
  match spec:
    case Some{+s}:
      cards.fin.hold.eff2(st, e, t, acc, s, value)
    case None{}:
      cards.fin.hold.go(st, t, e <> acc)

def cards.fin.hold.eff2(+st: J.Val, +e: J.Val, +t: List<&2, J.Val>, +acc: List<&2, J.Val>, +s: CardsSpec, +value: U32) -> J.Val:
  match s:
    case CardsSpec{+word, +name, +low, +high, +cap, +hold, +spend}:
      home.util.ite(J.Val, Bool.and(U32.is_gt(cap, 0), U32.is_gt(value, cap)),
        u => cards.fin.hold.go(cards.ck.note(st, word ++ " " ++ U32.show(value) ++ " is over what a deck may hold (" ++ U32.show(cap) ++ "), clamped to " ++ U32.show(cap)), t, J.set(e, "eff", home.util.obj([(name, J.JU{cap})])) <> acc),
        u => cards.fin.hold.go(st, t, e <> acc))

def cards.spec_of(+word: String, +specs: List<&2, CardsSpec>) -> Maybe<&2, CardsSpec>:
  match specs:
    case Nil{}:
      None{}
    case Con{+s, +t}:
      home.util.ite(Maybe<&2, CardsSpec>, String.eq(cards.spec.word(s), word), u => Some{s}, u => cards.spec_of(word, t))

# An effect J {name: value}'s number (Cast carries none).
def cards.eff_value(+eff: J.Val) -> U32:
  cards.eff_value.of(J.fields(eff))

def cards.eff_value.of(+fs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> U32:
  match fs:
    case Con{(+k, +v), +t}:
      cards.eff_value.v(v)
    case Nil{}:
      0

def cards.eff_value.v(+v: J.Val) -> U32:
  match v:
    case J.JA{+xs}:
      J.u32(home.util.get(xs, 0))
    case J.JS{+s}:
      0
    case _:
      J.u32(v)

# `look <colour> <shape>` off a weapon is the knight's shots' colour and shape.
def cards.fin.looks(+st: J.Val) -> J.Val:
  cards.fin.slots(home.util.ite(J.Val, Bool.not(cards.kind_is(st, "arm")), u => cards.fin.style(J.set(st, "runes", cards.ck.no_look(J.get(st, "runes"))), J.get(st, "style")), u => st))

def cards.ck.no_look(+runes: J.Val) -> J.Val:
  J.JA{cards.ck.no_look.go(J.items(runes))}

def cards.ck.no_look.go(+xs: List<&2, J.Val>) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      home.util.ite(List<&2, J.Val>, String.starts_with(J.str(h), "look "), u => cards.ck.no_look.go(t), u => h <> cards.ck.no_look.go(t))

def cards.fin.style(+st: J.Val, +style: J.Val) -> J.Val:
  match style:
    case J.JNull{}:
      st
    case _:
      cards.fin.style.set(st, J.u32(J.get(style, "n")), home.util.words(J.str(J.get(style, "rest"))))

def cards.fin.style.set(+st: J.Val, +n: U32, +ws: List<&2, String>) -> J.Val:
  match ws:
    case Con{+colour, Con{+shape, Nil{}}}:
      home.util.ite(J.Val, forge.valid_look(colour, shape),
        u => J.set(st, "looks", J.set(J.get(st, "looks"), "style", J.JA{[J.JS{colour}, J.JS{shape}]})),
        u => cards.fin.style.bad(st, n))
    case _:
      cards.fin.style.bad(st, n)

def cards.fin.style.bad(+st: J.Val, +n: U32) -> J.Val:
  cards.ck.err(st, "line " ++ U32.show(n) ++ ": look takes a colour (" ++ home.util.join(forge.COLOURS(), " ") ++ ") and a shape (" ++ home.util.join(forge.SHAPES(), " ") ++ ")")

# What a card shows must be something its kind keeps on the field.
def cards.fin.slots(+st: J.Val) -> J.Val:
  cards.fin.style_kind(cards.fin.slot(cards.fin.slot(cards.fin.slot(cards.fin.slot(cards.fin.slot(st, "worn"), "shot"), "orbit"), "summon"), "aura"))

def cards.has_summon(+st: J.Val) -> Bool:
  cards.has_summon.go(J.items(J.get(st, "effects")))

def cards.has_summon.go(+es: List<&2, J.Val>) -> Bool:
  match es:
    case Nil{}:
      False{}
    case Con{+e, +t}:
      Bool.or(String.eq(J.str(J.get(e, "word")), "summon"), cards.has_summon.go(t))

def cards.slot_fits(+st: J.Val, +slot: String) -> Bool:
  home.util.ite(Bool, Bool.or(String.eq(slot, "worn"), String.eq(slot, "aura")), u => cards.kind_in(st, ["hold", "guard", "arm"]),
  u => home.util.ite(Bool, String.eq(slot, "shot"), u => cards.kind_in(st, ["hold", "arm", "spell"]),
  u => home.util.ite(Bool, String.eq(slot, "orbit"), u => cards.kind_is(st, "hold"), u => cards.has_summon(st))))

def cards.fin.slot(+st: J.Val, +slot: String) -> J.Val:
  home.util.ite(J.Val, Bool.and(J.has(J.get(st, "looks"), slot), Bool.not(cards.slot_fits(st, slot))),
    u => cards.ck.err(st, home.util.ite(String, String.eq(slot, "summon"), u => "a `draw summon` needs a `summon` effect on the card",
      u => home.util.ite(String, String.eq(slot, "orbit"), u => "`draw orbit` belongs on a hold card", u => "`draw " ++ slot ++ "` does not show on a " ++ J.str(J.get(st, "kind")) ++ " card"))),
    u => st)

def cards.fin.style_kind(+st: J.Val) -> J.Val:
  cards.fin.cooldown(home.util.ite(J.Val, Bool.and(J.has(J.get(st, "looks"), "style"), Bool.not(cards.kind_in(st, ["hold", "spell"]))),
    u => cards.ck.err(st, "`look` does not show on a " ++ J.str(J.get(st, "kind")) ++ " card"),
    u => st))

# A summoning spell recharges no faster than its creatures earn.
def cards.fin.floor(+es: List<&2, J.Val>, +acc: U32) -> U32:
  match es:
    case Nil{}:
      U32.min(acc, 120)
    case Con{+e, +t}:
      cards.fin.floor(t, U32.max(acc, cards.summon_secs(J.get(e, "eff"))))

def cards.summon_secs(+eff: J.Val) -> U32:
  home.util.ite(U32, J.has(eff, "summon"),
    u => (J.u32(home.util.get(J.items(J.get(eff, "summon")), 0)) * imps.Behaviour.seconds_each(J.str(home.util.get(J.items(J.get(eff, "summon")), 1))) : U32),
    u => 0)

def cards.fin.cooldown(+st: J.Val) -> J.Val:
  cards.fin.cooldown.rule(home.util.ite(J.Val, Bool.and(cards.kind_is(st, "spell"), Bool.not(J.is_null(J.get(st, "cooldown")))),
    u => cards.fin.cooldown.raise(st, J.u32(J.get(st, "cooldown")), cards.fin.floor(J.items(J.get(st, "effects")), 0)),
    u => st))

def cards.fin.cooldown.raise(+st: J.Val, +seconds: U32, +floor: U32) -> J.Val:
  home.util.ite(J.Val, U32.is_lt(seconds, floor),
    u => J.set(cards.ck.note(st, "cooldown raised to " ++ U32.show(floor) ++ "s: that many creatures take longer to call"), "cooldown", J.JU{floor}),
    u => st)

def cards.fin.cooldown.rule(+st: J.Val) -> J.Val:
  cards.fin.ult(home.util.ite(J.Val, Bool.and(cards.kind_is(st, "spell"), J.is_null(J.get(st, "cooldown"))),
    u => cards.ck.err(st, "a spell needs `cooldown <seconds>` (1–120)"),
    u => home.util.ite(J.Val, Bool.and(Bool.not(cards.kind_is(st, "spell")), Bool.not(J.is_null(J.get(st, "cooldown")))),
      u => cards.ck.err(st, "`cooldown` belongs on `kind spell`"),
      u => st)))

def cards.fin.ult(+st: J.Val) -> J.Val:
  cards.fin.body(home.util.ite(J.Val, Bool.and(Bool.not(J.is_null(J.get(st, "ult"))), Bool.not(cards.kind_is(st, "hold"))),
    u => cards.ck.err(st, "`ult` belongs on `kind hold`"),
    u => st))

def cards.fin.body(+st: J.Val) -> J.Val:
  cards.fin.art(
    home.util.ite(J.Val, cards.kind_is(st, "guard"), u => cards.fin.guard(st),
    u => home.util.ite(J.Val, Bool.not(J.is_null(J.get(st, "guard"))), u => cards.ck.err(st, "`guard` belongs on `kind guard`"),
    u => home.util.ite(J.Val, cards.kind_is(st, "arm"), u => cards.fin.arm(st), u => cards.fin.effects(st)))))

def cards.fin.guard(+st: J.Val) -> J.Val:
  cards.fin.guard.only(home.util.ite(J.Val, J.is_null(J.get(st, "guard")), u => cards.ck.err(st, "a guard card needs `guard roll` or `guard shield`"), u => st))

def cards.fin.guard.only(+st: J.Val) -> J.Val:
  home.util.ite(J.Val, Bool.or(Bool.not(U32.is_eq(J.len(J.get(st, "effects")), 0)), Bool.or(Bool.not(J.is_null(J.get(st, "plain"))), Bool.not(U32.is_eq(J.len(J.get(st, "runes")), 0)))),
    u => cards.ck.err(st, "a guard card only says what the guard key does"),
    u => st)

def cards.fin.arm(+st: J.Val) -> J.Val:
  cards.fin.arm.pick(home.util.ite(J.Val, Bool.not(U32.is_eq(J.len(J.get(st, "effects")), 0)), u => cards.ck.err(st, "an arm card is a weapon; put effects on a hold card"), u => st))

def cards.fin.arm.pick(+st: J.Val) -> J.Val:
  cards.fin.arm.pair(st, Bool.not(J.is_null(J.get(st, "plain"))), U32.is_eq(J.len(J.get(st, "runes")), 0))

def cards.fin.arm.pair(+st: J.Val, +plain: Bool, +none_runes: Bool) -> J.Val:
  home.util.ite(J.Val, Bool.and(plain, Bool.not(none_runes)), u => cards.ck.err(st, "use `arm <weapon>` or rune lines, not both"),
  u => home.util.ite(J.Val, plain, u => J.set(st, "arm", home.util.obj([("plain", J.get(st, "plain"))])),
  u => home.util.ite(J.Val, Bool.not(none_runes), u => cards.fin.arm.forged(st, forge.check("name " ++ J.str(J.get(st, "name")) ++ "\n" ++ home.util.join(cards.ck.strs(J.get(st, "runes")), "\n"))),
  u => cards.ck.err(st, "an arm card needs `arm bow|crossbow|handgonne|notes` or Forge runes (bolt/melee/spread/look)"))))

def cards.fin.arm.forged(+st: J.Val, +r: J.Val) -> J.Val:
  home.util.ite(J.Val, J.has(r, "Ok"),
    u => J.set(cards.ck.notes_add(st, J.get(J.get(r, "Ok"), "notes")), "arm", home.util.obj([("forged", cards.fin.speed(J.get(J.get(r, "Ok"), "weapon")))])),
    u => cards.ck.errs_add(st, J.items(J.get(r, "Err"))))

def cards.ck.notes_add(+st: J.Val, +ns: J.Val) -> J.Val:
  J.set(st, "notes", J.JA{List.append(&2, J.Val, J.items(J.get(st, "notes")), J.items(ns))})

def cards.ck.errs_add(+st: J.Val, +es: List<&2, J.Val>) -> J.Val:
  J.set(st, "errors", J.JA{List.append(&2, J.Val, J.items(J.get(st, "errors")), es)})

# A homemade arm is never slow: its shots fly at least as fast as the stock arms'.
def cards.fin.speed(+w: J.Val) -> J.Val:
  cards.fin.speed.of(w, J.get(w, "bolt"))

def cards.fin.speed.of(+w: J.Val, +bolt: J.Val) -> J.Val:
  match bolt:
    case J.JNull{}:
      w
    case _:
      J.set(w, "bolt", J.set(bolt, "speed", J.JF{home.util.fmax(J.f32(J.get(bolt, "speed")), 36.0)}))

def home.util.fmax(+a: F32, +b: F32) -> F32:
  home.util.ite(F32, F32.is_lt(a, b), u => b, u => a)

def cards.fin.effects(+st: J.Val) -> J.Val:
  cards.fin.dups(cards.fin.each(cards.fin.count(cards.fin.need(cards.fin.weapon_lines(st)))), J.items(J.get(st, "effects")), Nil{})

def cards.fin.weapon_lines(+st: J.Val) -> J.Val:
  home.util.ite(J.Val, Bool.or(Bool.not(J.is_null(J.get(st, "plain"))), Bool.not(U32.is_eq(J.len(J.get(st, "runes")), 0))),
    u => cards.ck.err(st, "weapon lines belong on `kind arm`, not `kind " ++ J.str(J.get(st, "kind")) ++ "`"),
    u => st)

def cards.fin.need(+st: J.Val) -> J.Val:
  home.util.ite(J.Val, Bool.and(U32.is_eq(J.len(J.get(st, "effects")), 0), J.is_null(J.get(st, "ult"))),
    u => cards.ck.err(st, "a " ++ J.str(J.get(st, "kind")) ++ " card needs at least one effect"),
    u => st)

def cards.fin.count(+st: J.Val) -> J.Val:
  home.util.ite(J.Val, U32.is_gt(J.len(J.get(st, "effects")), 3),
    u => cards.ck.err(st, "at most 3 effects per card (" ++ U32.show(J.len(J.get(st, "effects"))) ++ " given)"),
    u => st)

def cards.fin.each(+st: J.Val) -> J.Val:
  cards.fin.each.go(st, J.items(J.get(st, "effects")))

def cards.fin.each.go(+st: J.Val, +es: List<&2, J.Val>) -> J.Val:
  match es:
    case Nil{}:
      st
    case Con{+e, +t}:
      cards.fin.each.go(cards.fin.fit(st, J.u32(J.get(e, "n")), J.str(J.get(e, "word")), cards.spec_of(J.str(J.get(e, "word")), cards.SPECS())), t)

def cards.is_minion(+word: String) -> Bool:
  home.util.has_name(["brownie", "moth", "wisp", "mite", "sprite", "mote", "newt", "linnet", "leech", "ghoul", "warden", "totem"], word)

def cards.word_hold(+word: String, +spec: Maybe<&2, CardsSpec>) -> Bool:
  match spec:
    case Some{CardsSpec{+w, +name, +low, +high, +cap, +hold, +spend}}:
      hold
    case None{}:
      False{}

def cards.word_spend(+word: String, +spec: Maybe<&2, CardsSpec>) -> Bool:
  home.util.ite(Bool, String.eq(word, "summon"), u => True{},
    u => match_spend(spec))

def match_spend(+spec: Maybe<&2, CardsSpec>) -> Bool:
  match spec:
    case Some{CardsSpec{+w, +name, +low, +high, +cap, +hold, +spend}}:
      spend
    case None{}:
      False{}

def cards.fin.fit(+st: J.Val, +n: U32, +word: String, +spec: Maybe<&2, CardsSpec>) -> J.Val:
  home.util.ite(J.Val, cards.fin.fits(st, word, spec),
    u => st,
    u => cards.ck.err(st, "line " ++ U32.show(n) ++ ": " ++ word ++ " works on " ++ cards.fin.works(word, spec) ++ " cards, not " ++ J.str(J.get(st, "kind"))))

def cards.fin.fits(+st: J.Val, +word: String, +spec: Maybe<&2, CardsSpec>) -> Bool:
  home.util.ite(Bool, cards.kind_is(st, "spell"),
    u => home.util.has_name(["heal", "bombs", "nova", "ward", "cast", "cabbages", "imposture", "hedge", "summon", "feat", "ghoul", "warden", "totem", "rip"], word),
    u => home.util.ite(Bool, String.eq(word, "summon"), u => cards.kind_is(st, "play"),
      u => home.util.ite(Bool, cards.kind_is(st, "hold"), u => cards.word_hold(word, spec),
        u => home.util.ite(Bool, cards.is_minion(word), u => cards.kind_is(st, "play"), u => cards.word_spend(word, spec)))))

def cards.fin.works(+word: String, +spec: Maybe<&2, CardsSpec>) -> String:
  home.util.ite(String, Bool.or(String.eq(word, "cast"), String.eq(word, "rip")), u => "spell",
    u => home.util.ite(String, home.util.has_name(["summon", "ghoul", "warden", "totem"], word), u => "spell or play",
      u => home.util.ite(String, cards.word_hold(word, spec), u => "hold",
        u => home.util.ite(String, cards.is_minion(word), u => "play", u => "take or play"))))

def cards.fin.dups(+st: J.Val, +es: List<&2, J.Val>, +seen: List<&2, String>) -> J.Val:
  match es:
    case Nil{}:
      st
    case Con{+e, +t}:
      home.util.ite(J.Val, home.util.has_name(seen, J.str(J.get(e, "word"))),
        u => cards.fin.dups(cards.ck.err(st, "line " ++ U32.show(J.u32(J.get(e, "n"))) ++ ": " ++ J.str(J.get(e, "word")) ++ " appears twice"), t, J.str(J.get(e, "word")) <> seen),
        u => cards.fin.dups(st, t, J.str(J.get(e, "word")) <> seen))

# The card's art rows.
def cards.fin.art(+st: J.Val) -> J.Val:
  cards.fin.worn(cards.fin.art.ink(cards.fin.art.size(st)))

def cards.fin.art.size(+st: J.Val) -> J.Val:
  home.util.ite(J.Val, Bool.or(U32.is_gt(J.len(J.get(st, "art")), 16), cards.rows_wide(J.items(J.get(st, "art")))),
    u => cards.ck.err(st, "art is at most 16 by 16 inks"),
    u => st)

def cards.rows_wide(+rows: List<&2, J.Val>) -> Bool:
  match rows:
    case Nil{}:
      False{}
    case Con{+r, +t}:
      Bool.or(U32.is_gt(home.util.chars(J.str(r)), 16), cards.rows_wide(t))

def cards.rows_blank(+rows: List<&2, J.Val>) -> Bool:
  match rows:
    case Nil{}:
      True{}
    case Con{+r, +t}:
      Bool.and(cards.all_dots(String.to_list(J.str(r))), cards.rows_blank(t))

def cards.all_dots(+cs: List<&2, Char>) -> Bool:
  match cs:
    case Nil{}:
      True{}
    case Con{+h, +t}:
      Bool.and(U32.is_eq(Char.to_u32(h), 46), cards.all_dots(t))

def cards.fin.art.ink(+st: J.Val) -> J.Val:
  home.util.ite(J.Val, Bool.and(Bool.not(U32.is_eq(J.len(J.get(st, "art")), 0)), cards.rows_blank(J.items(J.get(st, "art")))),
    u => cards.ck.err(st, "art has no inked pixel; use realm inks (see /dungeon cards)"),
    u => st)

# A worn sprite that names no place takes the one its card's name implies.
def cards.fin.worn(+st: J.Val) -> J.Val:
  cards.fin.worn.at(st, J.get(J.get(st, "looks"), "worn"))

def cards.fin.worn.at(+st: J.Val, +worn: J.Val) -> J.Val:
  match worn:
    case J.JNull{}:
      cards.fin.done(st)
    case _:
      home.util.ite(J.Val, J.has(worn, "at"), u => cards.fin.done(st), u => cards.fin.worn.part(st, worn, looks.default_worn_part(J.str(J.get(st, "name")))))

def cards.fin.worn.part(+st: J.Val, +worn: J.Val, +at: String) -> J.Val:
  home.util.ite(J.Val, String.eq(at, "whole"), u => cards.fin.done(st),
    u => cards.fin.worn.fit(st, worn, at, artistry.fit_worn(J.set(worn, "at", J.JS{at}), at)))

def cards.fin.worn.fit(+st: J.Val, +worn: J.Val, +at: String, +r: J.Val) -> J.Val:
  home.util.ite(J.Val, J.has(r, "Ok"),
    u => cards.fin.done(J.set(st, "looks", J.set(J.get(st, "looks"), "worn", J.get(r, "Ok")))),
    u => cards.fin.done(J.set(cards.ck.err(st, J.str(J.get(r, "Err"))), "looks", J.set(J.get(st, "looks"), "worn", J.set(worn, "at", J.JS{at})))))

# Rejected with every error, or the card checked.
def cards.fin.done(+st: J.Val) -> J.Val:
  home.util.ite(J.Val, Bool.not(U32.is_eq(J.len(J.get(st, "errors")), 0)),
    u => cards.ck.fail(J.get(st, "errors")),
    u => cards.fin.card(st))

def cards.rarity_drop(+r: String) -> U32:
  Bool.pick(U32, String.eq(r, "common"), 10, Bool.pick(U32, String.eq(r, "rare"), 3, 0))

def cards.rarity_chest(+r: String) -> U32:
  Bool.pick(U32, String.eq(r, "rare"), 4, 2)

def cards.fin.effs(+es: List<&2, J.Val>) -> List<&2, J.Val>:
  match es:
    case Nil{}:
      Nil{}
    case Con{+e, +t}:
      J.get(e, "eff") <> cards.fin.effs(t)

def cards.fin.card(+st: J.Val) -> J.Val:
  home.util.ok(home.util.obj([("card", cards.fin.card.of(st)), ("notes", J.get(st, "notes"))]))

def cards.fin.card.of(+st: J.Val) -> J.Val:
  home.util.set_if(Bool.not(home.util.obj_empty(J.get(st, "looks"))),
    home.util.set_if(Bool.not(J.is_null(J.get(st, "ult"))),
      home.util.obj([
        ("id", J.get(st, "id")), ("name", J.get(st, "name")), ("kind", J.get(st, "kind")), ("rarity", J.get(st, "rarity")),
        ("text", J.get(st, "text")), ("by", J.get(st, "by")),
        ("drop", home.util.ite(J.Val, J.is_null(J.get(st, "drop")), u => J.JU{cards.rarity_drop(J.str(J.get(st, "rarity")))}, u => J.get(st, "drop"))),
        ("chest", home.util.ite(J.Val, J.is_null(J.get(st, "chest")), u => J.JU{cards.rarity_chest(J.str(J.get(st, "rarity")))}, u => J.get(st, "chest"))),
        ("effects", J.JA{cards.fin.effs(J.items(J.get(st, "effects")))}), ("arm", J.get(st, "arm")), ("art", J.get(st, "art")),
        ("only_in", J.get(st, "only_in")), ("guard", J.get(st, "guard")),
        ("cooldown", home.util.ite(J.Val, J.is_null(J.get(st, "cooldown")), u => J.JU{0}, u => J.get(st, "cooldown")))]),
      "ult", J.get(st, "ult")),
    "looks", J.get(st, "looks"))

# ---- Card::file / definition_file --------------------------------------------------

# The word of an effect's serde name: `cast`, `summon` and the spec table's.
def cards.word_of_name(+name: String) -> String:
  cards.word_of_name.go(name, cards.SPECS())

def cards.word_of_name.go(+name: String, +specs: List<&2, CardsSpec>) -> String:
  match specs:
    case Nil{}:
      "?"
    case Con{CardsSpec{+word, +n, +low, +high, +cap, +hold, +spend}, +t}:
      home.util.ite(String, String.eq(n, name), u => word, u => cards.word_of_name.go(name, t))

# Card::line: one effect as the card line that says it
def cards.Card.line(+eff: J.Val) -> String:
  cards.Card.line.of(J.fields(eff))

def cards.Card.line.of(+fs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> String:
  match fs:
    case Con{(+name, +v), +t}:
      cards.Card.line.named(name, v)
    case Nil{}:
      ""

def cards.Card.line.named(+name: String, +v: J.Val) -> String:
  home.util.ite(String, String.eq(name, "cast"), u => "cast " ++ J.str(v),
    u => home.util.ite(String, String.eq(name, "summon"), u => "summon " ++ U32.show(J.u32(home.util.get(J.items(v), 0))) ++ " " ++ J.str(home.util.get(J.items(v), 1)),
      u => cards.word_of_name(name) ++ " " ++ U32.show(J.u32(v))))

def cards.lines_of(+effects: List<&2, J.Val>) -> String:
  match effects:
    case Nil{}:
      ""
    case Con{+e, +t}:
      cards.Card.line(e) ++ "\n" ++ cards.lines_of(t)

def cards.rows_text(+rows: List<&2, J.Val>) -> String:
  match rows:
    case Nil{}:
      ""
    case Con{+r, +t}:
      J.str(r) ++ "\n" ++ cards.rows_text(t)

def cards.sprites_text(+looks: J.Val) -> String:
  cards.sprite_text(looks, "worn") ++ cards.sprite_text(looks, "shot") ++ cards.sprite_text(looks, "orbit") ++ cards.sprite_text(looks, "summon") ++ cards.sprite_text(looks, "aura")

def cards.sprite_text(+looks: J.Val, +slot: String) -> String:
  home.util.ite(String, J.has(looks, slot), u => looks.Sprite.to_block(J.get(looks, slot), slot) ++ "\n", u => "")

def cards.Card.file(+card: J.Val) -> String:
  "name " ++ J.str(J.get(card, "name")) ++ "\nkind " ++ J.str(J.get(card, "kind")) ++ "\nrarity " ++ J.str(J.get(card, "rarity")) ++ "\ntext " ++ J.str(J.get(card, "text")) ++ "\n"
    ++ home.util.ite(String, String.is_empty(J.str(J.get(card, "by"))), u => "", u => "by " ++ J.str(J.get(card, "by")) ++ "\n")
    ++ "drop " ++ U32.show(J.u32(J.get(card, "drop"))) ++ "\nchest " ++ U32.show(J.u32(J.get(card, "chest"))) ++ "\n"
    ++ home.util.ite(String, String.eq(J.str(J.get(card, "kind")), "spell"), u => "cooldown " ++ U32.show(J.u32(J.get(card, "cooldown"))) ++ "\n", u => "")
    ++ cards.lines_of(J.items(J.get(card, "effects")))
    ++ home.util.ite(String, J.is_null(J.get(card, "ult")), u => "", u => "ult " ++ ults.Ult.word(J.get(card, "ult")) ++ "\n")
    ++ cards.style_text(J.get(J.get(card, "looks"), "style"))
    ++ cards.sprites_text(J.get(card, "looks"))
    ++ home.util.ite(String, U32.is_eq(J.len(J.get(card, "art")), 0), u => "", u => "art\n" ++ cards.rows_text(J.items(J.get(card, "art"))))

def cards.style_text(+style: J.Val) -> String:
  match style:
    case J.JA{+xs}:
      "look " ++ J.str(home.util.get(xs, 0)) ++ " " ++ J.str(home.util.get(xs, 1)) ++ "\n"
    case _:
      ""

def cards.arm_text(+arm: J.Val) -> String:
  home.util.ite(String, J.has(arm, "plain"), u => "arm " ++ J.str(J.get(arm, "plain")) ++ "\n",
    u => home.util.ite(String, J.has(arm, "forged"), u => cards.runes_text(home.util.lines(J.str(J.get(J.get(arm, "forged"), "runes")))), u => ""))

def cards.runes_text(+ls: List<&2, String>) -> String:
  match ls:
    case Nil{}:
      ""
    case Con{+l, +t}:
      home.util.ite(String, String.starts_with(l, "name "), u => cards.runes_text(t), u => l ++ "\n" ++ cards.runes_text(t))

def cards.guard_text(+g: J.Val) -> String:
  match g:
    case J.JS{+s}:
      "guard " ++ s ++ "\n"
    case J.JO{+m}:
      "guard wall width=" ++ U32.show(J.u32(J.get(J.get(g, "wall"), "width"))) ++ " drain=" ++ U32.show(J.u32(J.get(J.get(g, "wall"), "drain"))) ++ " empower=" ++ U32.show(J.u32(J.get(J.get(g, "wall"), "empower"))) ++ "\n"
    case _:
      ""

def cards.where_text(+pack: J.Val) -> String:
  match pack:
    case J.JS{+p}:
      "where " ++ home.util.ite(String, String.eq(p, "crypt"), u => "crypt",
        u => home.util.ite(String, String.eq(p, "cavern"), u => "mines",
          u => home.util.ite(String, String.eq(p, "hellforge"), u => "keep", u => layout.Pack.name(p)))) ++ "\n"
    case _:
      ""

# Complete authored definition for validation and implementation handoff.
def cards.Card.definition_file(+card: J.Val) -> String:
  cards.arm_text(J.get(card, "arm")) ++ cards.guard_text(J.get(card, "guard")) ++ cards.where_text(J.get(card, "only_in")) ++ cards.Card.file(card)

# ---- caps and the deck's bonus ------------------------------------------------------

def cards.range(+word: String) -> J.Val:
  cards.range.of(cards.spec_of(word, cards.SPECS()))

def cards.range.of(+s: Maybe<&2, CardsSpec>) -> J.Val:
  match s:
    case Some{CardsSpec{+w, +n, +low, +high, +cap, +hold, +spend}}:
      J.JA{[J.JU{low}, J.JU{high}]}
    case None{}:
      J.JNull{}

# The most a knight's held cards may add up to of effect `word`.
def cards.cap(+word: String) -> U32:
  cards.cap.of(cards.spec_of(word, cards.SPECS()))

def cards.cap.of(+s: Maybe<&2, CardsSpec>) -> U32:
  match s:
    case Some{CardsSpec{+w, +n, +low, +high, +cap, +hold, +spend}}:
      cap
    case None{}:
      0

def cards.WORDS() -> List<&2, String>:
  ["damage", "rate", "speed", "pierce", "shots", "armor", "vamp", "bounce", "homing", "chain", "mend", "orbit", "burst", "fork", "fart", "size", "knock", "burn", "chill", "poison", "boomerang", "velocity"]

def cards.index_of(+word: String, +ws: List<&2, String>, +i: U32) -> U32:
  match ws:
    case Nil{}:
      99
    case Con{+h, +t}:
      home.util.ite(U32, String.eq(h, word), u => i, u => cards.index_of(word, t, (i + 1 : U32)))

# The most a knight's wishes add up to of each word, under the card cap.
def cards.wish_cap(+word: String) -> U32:
  home.util.ite(U32, String.eq(word, "damage"), u => 25,
  u => home.util.ite(U32, Bool.or(String.eq(word, "rate"), String.eq(word, "speed")), u => 20,
  u => home.util.ite(U32, String.eq(word, "velocity"), u => 40,
  u => home.util.ite(U32, String.eq(word, "burst"), u => 15,
  u => home.util.ite(U32, String.eq(word, "shots"), u => 1, u => U32.min(cards.cap(word), 2))))))

# The most a wish can ever give of held word `word`, or null for other words.
def cards.wish_most(+word: String) -> J.Val:
  home.util.ite(J.Val, home.util.has_name(cards.WORDS(), word), u => J.JU{U32.min(cards.wish_cap(word), cards.cap(word))}, u => J.JNull{})

def cards.whole(+word: String) -> Bool:
  U32.is_le(cards.cap(word), 3)

def cards.fget(+xs: List<&2, F32>, +i: U32) -> F32:
  cards.fget.of(List.get(&2, F32, xs, U32.to_nat(i)))

def cards.fget.of(+m: Maybe<&2, F32>) -> F32:
  match m:
    case Some{+v}:
      v
    case None{}:
      0.0

def cards.fset(+xs: List<&2, F32>, +i: U32, +v: F32) -> List<&2, F32>:
  List.set(&2, F32, xs, U32.to_nat(i), v)

def cards.zeros(+n: U32) -> List<&2, F32>:
  List.replicate(F32, U32.to_nat(n), 0.0)

def cards.empties(+n: U32) -> List<&2, List<&2, F32>>:
  match n:
    case 0:
      Nil{}
    case _:
      Nil{} <> cards.empties((n - 1 : U32))

# the number of earlier cards with this id: how many copies are already held
def cards.copies(+seen: List<&2, String>, +id: String) -> U32:
  match seen:
    case Nil{}:
      0
    case Con{+h, +t}:
      (Bool.to_u32(String.eq(h, id)) + cards.copies(t, id) : U32)

def cards.copy_worth(+copy: U32) -> F32:
  home.util.ite(F32, U32.is_eq(copy, 0), u => 1.0, u => home.util.ite(F32, U32.is_eq(copy, 1), u => 0.5, u => home.util.ite(F32, U32.is_eq(copy, 2), u => 0.25, u => 0.0)))

def cards.Bonus.of(+held: List<&2, J.Val>) -> J.Val:
  cards.Bonus.within(held, None{})

def cards.Bonus.within(+held: List<&2, J.Val>, +floor: Maybe<&2, U32>) -> J.Val:
  cards.Bonus.within.go(combos.held_families(held), held, BonusAcc{cards.zeros(22), cards.empties(22), 0, Nil{}}, floor)

def cards.Bonus.within.go(+families: List<&2, U32>, +held: List<&2, J.Val>, +acc: BonusAcc, +floor: Maybe<&2, U32>) -> J.Val:
  cards.Bonus.shares(cards.Bonus.walk(families, held, acc), floor)

def cards.Bonus.walk(+families: List<&2, U32>, +held: List<&2, J.Val>, +acc: BonusAcc) -> BonusAcc:
  match held:
    case Nil{}:
      acc
    case Con{+card, +t}:
      cards.Bonus.walk(families, t, cards.Bonus.card(families, card, acc))

def cards.Bonus.card(+families: List<&2, U32>, +card: J.Val, +acc: BonusAcc) -> BonusAcc:
  match acc:
    case BonusAcc{+sums, +wished, +volley, +seen}:
      cards.Bonus.effects(families, J.items(J.get(card, "effects")), combos.is_wish(card), cards.copy_worth(cards.copies(seen, J.str(J.get(card, "id")))), BonusAcc{sums, wished, volley, J.str(J.get(card, "id")) <> seen})

def cards.Bonus.effects(+families: List<&2, U32>, +es: List<&2, J.Val>, +wish: Bool, +worth: F32, +acc: BonusAcc) -> BonusAcc:
  match es:
    case Nil{}:
      acc
    case Con{+e, +t}:
      cards.Bonus.effects(families, t, wish, worth, cards.Bonus.effect(families, home.util.get(home.util.keys_of(e), 0), J.get(e, J.str(home.util.get(home.util.keys_of(e), 0))), wish, worth, acc))

def cards.Bonus.effect(+families: List<&2, U32>, +name_j: J.Val, +value_j: J.Val, +wish: Bool, +worth: F32, +acc: BonusAcc) -> BonusAcc:
  cards.Bonus.effect.go(families, J.str(name_j), J.u32(value_j), wish, worth, acc)

def cards.Bonus.effect.go(+families: List<&2, U32>, +name: String, +value: U32, +wish: Bool, +worth: F32, +acc: BonusAcc) -> BonusAcc:
  home.util.ite(BonusAcc, Bool.and(wish, cards.Bonus.locked(name, families)),
    u => cards.Bonus.push_wished(acc, 0, U32.to_f32(combos.STAND_IN())),
    u => home.util.ite(BonusAcc, String.eq(name, "volley"),
      u => cards.Bonus.volley(acc, value),
      u => cards.Bonus.slot(acc, cards.index_of(name, cards.WORDS(), 0), value, wish, worth)))

# a super effect on a wish whose recipe is not yet held
def cards.Bonus.locked(+name: String, +families: List<&2, U32>) -> Bool:
  cards.Bonus.locked.of(combos.effect_word(name), families)

def cards.Bonus.locked.of(+w: Maybe<&2, String>, +families: List<&2, U32>) -> Bool:
  match w:
    case Some{+word}:
      Bool.not(combos.unlocked(word, families))
    case None{}:
      False{}

def cards.Bonus.volley(+acc: BonusAcc, +v: U32) -> BonusAcc:
  match acc:
    case BonusAcc{+sums, +wished, +volley, +seen}:
      BonusAcc{sums, wished, home.util.ite(U32, U32.is_eq(volley, 0), u => U32.max(v, 5), u => U32.min(volley, U32.max(v, 5))), seen}

def cards.Bonus.push_wished(+acc: BonusAcc, +i: U32, +v: F32) -> BonusAcc:
  match acc:
    case BonusAcc{+sums, +wished, +volley, +seen}:
      BonusAcc{sums, cards.wset(wished, i, v <> cards.wget(wished, i)), volley, seen}

def cards.wget(+xs: List<&2, List<&2, F32>>, +i: U32) -> List<&2, F32>:
  cards.wget.of(List.get(&2, List<&2, F32>, xs, U32.to_nat(i)))

def cards.wget.of(+m: Maybe<&2, List<&2, F32>>) -> List<&2, F32>:
  match m:
    case Some{+v}:
      v
    case None{}:
      Nil{}

def cards.wset(+xs: List<&2, List<&2, F32>>, +i: U32, +v: List<&2, F32>) -> List<&2, List<&2, F32>>:
  List.set(&2, List<&2, F32>, xs, U32.to_nat(i), v)

def cards.Bonus.slot(+acc: BonusAcc, +i: U32, +value: U32, +wish: Bool, +worth: F32) -> BonusAcc:
  home.util.ite(BonusAcc, U32.is_eq(i, 99), u => acc,
    u => home.util.ite(BonusAcc, wish,
      u => cards.Bonus.push_wished(acc, i, U32.to_f32(value)),
      u => cards.Bonus.add_sum(acc, i, F32.mul(U32.to_f32(value), worth))))

def cards.Bonus.add_sum(+acc: BonusAcc, +i: U32, +x: F32) -> BonusAcc:
  match acc:
    case BonusAcc{+sums, +wished, +volley, +seen}:
      BonusAcc{cards.fset(sums, i, F32.add(cards.fget(sums, i), x)), wished, volley, seen}

# descending insertion sort
def cards.fsort(+xs: List<&2, F32>) -> List<&2, F32>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      cards.fsort.ins(h, cards.fsort(t))

def cards.fsort.ins(+x: F32, +xs: List<&2, F32>) -> List<&2, F32>:
  match xs:
    case Nil{}:
      [x]
    case Con{+h, +t}:
      home.util.ite(List<&2, F32>, F32.is_ge(x, h), u => x <> (h <> t), u => h <> cards.fsort.ins(x, t))

# strongest whole, next half, next a quarter
def cards.wish_sum(+v: List<&2, F32>) -> F32:
  match v:
    case Nil{}:
      0.0
    case Con{+a, +r1}:
      cards.wish_sum.b(F32.mul(a, 1.0), r1)

def cards.wish_sum.b(+s: F32, +r: List<&2, F32>) -> F32:
  match r:
    case Nil{}:
      s
    case Con{+b, +r2}:
      cards.wish_sum.c(F32.add(s, F32.mul(b, 0.5)), r2)

def cards.wish_sum.c(+s: F32, +r: List<&2, F32>) -> F32:
  match r:
    case Nil{}:
      s
    case Con{+c, +r3}:
      F32.add(s, F32.mul(c, 0.25))

def cards.fmin(+a: F32, +b: F32) -> F32:
  home.util.ite(F32, F32.is_lt(b, a), u => b, u => a)

# each word's wishes into a share, and the power they come to
def cards.Bonus.shares(+acc: BonusAcc, +floor: Maybe<&2, U32>) -> J.Val:
  match acc:
    case BonusAcc{+sums, +wished, +volley, +seen}:
      cards.Bonus.shares.go(0, wished, Nil{}, 0.0, sums, volley, floor)

def cards.Bonus.shares.go(+i: U32, +wished: List<&2, List<&2, F32>>, +share: List<&2, F32>, +power: F32, +sums: List<&2, F32>, +volley: U32, +floor: Maybe<&2, U32>) -> J.Val:
  home.util.ite(J.Val, U32.is_ge(i, 22),
    u => cards.Bonus.budget(List.reverse(&2, F32, share), power, sums, volley, floor),
    u => cards.Bonus.shares.step(i, wished, share, power, sums, volley, floor, U32.to_f32(cards.wish_cap(home.util.nth_word(cards.WORDS(), i))), cards.wish_sum(cards.fsort(cards.wget(wished, i)))))

def home.util.nth_word(+ws: List<&2, String>, +i: U32) -> String:
  combos.nth_str(ws, i)

def cards.Bonus.shares.step(+i: U32, +wished: List<&2, List<&2, F32>>, +share: List<&2, F32>, +power: F32, +sums: List<&2, F32>, +volley: U32, +floor: Maybe<&2, U32>, +top: F32, +sum: F32) -> J.Val:
  cards.Bonus.shares.next(i, wished, share, power, sums, volley, floor, top, cards.fmin(sum, top))

def cards.Bonus.shares.next(+i: U32, +wished: List<&2, List<&2, F32>>, +share: List<&2, F32>, +power: F32, +sums: List<&2, F32>, +volley: U32, +floor: Maybe<&2, U32>, +top: F32, +s: F32) -> J.Val:
  cards.Bonus.shares.go((i + 1 : U32), wished, s <> share, home.util.ite(F32, F32.is_gt(top, 0.0), u => F32.add(power, F32.div(s, top)), u => power), sums, volley, floor)

def cards.WISH_BUDGET(+i: U32) -> F32:
  home.util.ite(F32, U32.is_le(i, 1), u => 3.0, u => home.util.ite(F32, U32.is_eq(i, 2), u => 4.0, u => home.util.ite(F32, U32.is_eq(i, 3), u => 5.0, u => home.util.ite(F32, U32.is_eq(i, 4), u => 6.0, u => home.util.ite(F32, U32.is_eq(i, 5), u => 7.0, u => 8.0)))))

def cards.Bonus.scale(+share: List<&2, F32>, +k: F32) -> List<&2, F32>:
  match share:
    case Nil{}:
      Nil{}
    case Con{+s, +t}:
      F32.mul(s, k) <> cards.Bonus.scale(t, k)

def cards.Bonus.budget(+share: List<&2, F32>, +power: F32, +sums: List<&2, F32>, +volley: U32, +floor: Maybe<&2, U32>) -> J.Val:
  match floor:
    case Some{+f}:
      cards.Bonus.budget.over(share, power, sums, volley, cards.WISH_BUDGET(U32.min(f, 6)))
    case None{}:
      cards.Bonus.capped(share, sums, volley)

def cards.Bonus.budget.over(+share: List<&2, F32>, +power: F32, +sums: List<&2, F32>, +volley: U32, +budget: F32) -> J.Val:
  home.util.ite(J.Val, F32.is_gt(power, budget),
    u => cards.Bonus.capped(cards.Bonus.scale(share, F32.div(budget, power)), sums, volley),
    u => cards.Bonus.capped(share, sums, volley))

# f32::round for a non-negative value
def cards.round(+x: F32) -> F32:
  cards.round.t(F32.trunc(x), x)

def cards.round.t(+t: F32, +x: F32) -> F32:
  home.util.ite(F32, F32.is_ge(F32.sub(x, t), 0.5), u => F32.add(t, 1.0), u => t)

# `(value as u32).min(cap(word))`
def cards.Bonus.cap_of(+i: U32, +sums: List<&2, F32>, +share: List<&2, F32>) -> U32:
  cards.Bonus.cap_word(combos.nth_str(cards.WORDS(), i), cards.fget(sums, i), cards.fget(share, i))

def cards.Bonus.cap_word(+word: String, +sum: F32, +share: F32) -> U32:
  U32.min(
    home.util.round_u32.nn(home.util.ite(F32, cards.whole(word),
      u => F32.add(home.util.ite(F32, String.eq(word, "armor"), u => F32.floor(sum), u => cards.round(sum)), F32.floor(F32.add(share, 0.25))),
      u => cards.round(F32.add(sum, share)))),
    cards.cap(word))

# value as u32 for a non-negative f32 (saturating)
def home.util.round_u32.nn(+r: F32) -> U32:
  home.util.ite(U32, F32.is_ge(r, 4294967040.0), u => 4294967295, u => F32.to_u32(r))

def cards.Bonus.capped(+share: List<&2, F32>, +sums: List<&2, F32>, +volley: U32) -> J.Val:
  cards.Bonus.build(share, sums, volley, 0, J.obj())

def cards.Bonus.build(+share: List<&2, F32>, +sums: List<&2, F32>, +volley: U32, +i: U32, +o: J.Val) -> J.Val:
  home.util.ite(J.Val, U32.is_ge(i, 22),
    u => cards.Bonus.fin(o, volley),
    u => cards.Bonus.build(share, sums, volley, (i + 1 : U32), cards.Bonus.put(o, combos.nth_str(cards.WORDS(), i), home.util.ite(U32, U32.is_eq(i, 20),
      u => U32.min(Bool.to_u32(F32.is_gt(F32.add(cards.fget(sums, 20), cards.fget(share, 20)), 0.0)), cards.cap("boomerang")),
      u => cards.Bonus.cap_of(i, sums, share)))))

# the words wishes made literal stay off the wire at zero
def cards.Bonus.put(+o: J.Val, +word: String, +v: U32) -> J.Val:
  home.util.ite(J.Val, Bool.and(U32.is_eq(v, 0), cards.Bonus.optional(word)), u => o, u => J.set(o, word, J.JU{v}))

def cards.Bonus.optional(+word: String) -> Bool:
  home.util.has_name(["size", "knock", "burn", "chill", "poison", "boomerang", "velocity"], word)

def cards.Bonus.fin(+o: J.Val, +volley: U32) -> J.Val:
  J.set(o, "volley", J.JU{volley})

def case.cards.Bonus.within(+input: J.Val) -> J.Val:
  cards.Bonus.within(J.items(J.get(input, "held")), cards.case.floor(J.get(input, "floor")))

def cards.case.floor(+f: J.Val) -> Maybe<&2, U32>:
  match f:
    case J.JNull{}:
      None{}
    case _:
      Some{J.u32(f)}

def case.cards.cap(+input: J.Val) -> J.Val:
  J.JU{cards.cap(J.str(J.get(input, "word")))}

def case.cards.range(+input: J.Val) -> J.Val:
  cards.range(J.str(J.get(input, "word")))

def case.cards.wish_cap(+input: J.Val) -> J.Val:
  J.JU{cards.wish_cap(J.str(J.get(input, "word")))}

def case.cards.wish_most(+input: J.Val) -> J.Val:
  cards.wish_most(J.str(J.get(input, "word")))

# ---- Card helpers and the Book ------------------------------------------------------

# A material card: it carries spoils home and drops on its own roll.
def cards.Card.is_spoil(+card: J.Val) -> Bool:
  cards.Card.is_spoil.go(J.items(J.get(card, "effects")))

def cards.Card.is_spoil.go(+es: List<&2, J.Val>) -> Bool:
  match es:
    case Nil{}:
      False{}
    case Con{+e, +t}:
      Bool.or(Bool.or(Bool.or(J.has(e, "bone"), J.has(e, "wax")), Bool.or(J.has(e, "ore"), J.has(e, "gem"))), Bool.or(Bool.or(J.has(e, "ember"), J.has(e, "scale")), cards.Card.is_spoil.go(t)))

# A bottle: a card that mends or wards whoever drinks it.
def cards.Card.drinks(+card: J.Val) -> Bool:
  Bool.or(cauldron.has_effect(card, "heal"), cauldron.has_effect(card, "ward"))

# Written magic: a spell, or a card that says it is a scroll.
def cards.Card.scroll(+card: J.Val) -> Bool:
  Bool.or(String.eq(J.str(J.get(card, "kind")), "spell"), String.contains(home.util.ascii_lower(J.str(J.get(card, "id")) ++ " " ++ J.str(J.get(card, "name"))), "scroll"))

# A deep delve sheds the materials of its kin.
def cards.Card.drops_in(+card: J.Val, +pack: String) -> Bool:
  cards.Card.drops_in.of(J.get(card, "only_in"), pack)

def cards.Card.drops_in.of(+only: J.Val, +pack: String) -> Bool:
  match only:
    case J.JNull{}:
      True{}
    case _:
      Bool.or(String.eq(J.str(only), pack), String.eq(J.str(only), layout.Pack.kin(pack)))

def cards.Card.art_rows(+card: J.Val) -> J.Val:
  J.get(card, "art")

# The card with this id in the sorted book, or null.
def cards.Book.get(+book: J.Val, +id: String) -> J.Val:
  cards.Book.get.go(J.items(J.get(book, "cards")), id)

def cards.Book.get.go(+cs: List<&2, J.Val>, +id: String) -> J.Val:
  match cs:
    case Nil{}:
      J.JNull{}
    case Con{+c, +t}:
      home.util.ite(J.Val, String.eq(J.str(J.get(c, "id")), id), u => c, u => cards.Book.get.go(t, id))

# Add or replace a card; false when the book is full (1024 cards).
def cards.Book.insert(+book: J.Val, +card: J.Val) -> J.Val & Bool:
  cards.Book.insert.go(J.get(book, "revision"), J.items(J.get(book, "cards")), card, Nil{}, book)

def cards.Book.insert.go(+rev: J.Val, +cs: List<&2, J.Val>, +card: J.Val, +before: List<&2, J.Val>, +book: J.Val) -> J.Val & Bool:
  match cs:
    case Nil{}:
      cards.Book.insert.end(rev, before, card, book)
    case Con{+c, +t}:
      home.util.ite(J.Val & Bool, String.eq(J.str(J.get(c, "id")), J.str(J.get(card, "id"))),
        u => (cards.Book.rebuild(book, home.util.ite(J.Val, J.eq(c, card), u => rev, u => J.JU{(J.u32(rev) + 1 : U32)}), List.append(&2, J.Val, List.reverse(&2, J.Val, before), card <> t)), True{}),
        u => home.util.ite(J.Val & Bool, String.is_gt(J.str(J.get(c, "id")), J.str(J.get(card, "id"))),
          u => cards.Book.insert.full(book, rev, List.reverse(&2, J.Val, before), card, c <> t),
          u => cards.Book.insert.go(rev, t, card, c <> before, book)))

def cards.Book.insert.end(+rev: J.Val, +before: List<&2, J.Val>, +card: J.Val, +book: J.Val) -> J.Val & Bool:
  cards.Book.insert.full(book, rev, List.reverse(&2, J.Val, before), card, Nil{})

def cards.Book.insert.full(+book: J.Val, +rev: J.Val, +head: List<&2, J.Val>, +card: J.Val, +tail: List<&2, J.Val>) -> J.Val & Bool:
  home.util.ite(J.Val & Bool, U32.is_lt(home.util.len(List.append(&2, J.Val, head, tail)), 1024),
    u => (cards.Book.rebuild(book, J.JU{(J.u32(rev) + 1 : U32)}, List.append(&2, J.Val, head, card <> tail)), True{}),
    u => (book, False{}))

def cards.Book.rebuild(+book: J.Val, +rev: J.Val, +cs: List<&2, J.Val>) -> J.Val:
  J.set(J.set(book, "revision", rev), "cards", J.JA{cs})

# The weighted table of a roll: the card indexes with weight and the weights.
def cards.Book.table(+cs: List<&2, J.Val>, +pack: String, +mode: U32, +i: U32) -> List<&2, Sigma<&2, &2, U32, _ => U32>>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+c, +t}:
      home.util.ite(List<&2, Sigma<&2, &2, U32, _ => U32>>, U32.is_gt(cards.Book.weight(c, pack, mode), 0),
        u => (i, cards.Book.weight(c, pack, mode)) <> cards.Book.table(t, pack, mode, (i + 1 : U32)),
        u => cards.Book.table(t, pack, mode, (i + 1 : U32)))

# mode 0 an ordinary drop, 1 the delve's material, 2 a chest's prize
def cards.Book.weight(+c: J.Val, +pack: String, +mode: U32) -> U32:
  home.util.ite(U32, U32.is_eq(mode, 0),
    u => home.util.ite(U32, Bool.or(cards.Card.is_spoil(c), Bool.not(cards.Card.drops_in(c, pack))), u => 0, u => J.u32(J.get(c, "drop"))),
    u => home.util.ite(U32, U32.is_eq(mode, 1),
      u => home.util.ite(U32, Bool.and(cards.Card.is_spoil(c), cards.Card.drops_in(c, pack)), u => J.u32(J.get(c, "drop")), u => 0),
      u => home.util.ite(U32, cards.Card.drops_in(c, pack), u => J.u32(J.get(c, "chest")), u => 0)))

def cards.Book.weights(+t: List<&2, Sigma<&2, &2, U32, _ => U32>>) -> List<&2, U32>:
  match t:
    case Nil{}:
      Nil{}
    case Con{(+i, +w), +r}:
      w <> cards.Book.weights(r)

def cards.Book.index_at(+t: List<&2, Sigma<&2, &2, U32, _ => U32>>, +n: U32) -> U32:
  match t:
    case Nil{}:
      0
    case Con{(+i, +w), +r}:
      home.util.ite(U32, U32.is_eq(n, 0), u => i, u => cards.Book.index_at(r, (n - 1 : U32)))

# `roll`: the id picked off the weighted table, if the table is not empty.
def cards.Book.roll(+book: J.Val, +rng: W.W, +pack: String, +mode: U32) -> W.W & Maybe<&2, String>:
  cards.Book.roll.go(J.items(J.get(book, "cards")), rng, cards.Book.table(J.items(J.get(book, "cards")), pack, mode, 0))

def cards.Book.roll.go(+cs: List<&2, J.Val>, +rng: W.W, +table: List<&2, Sigma<&2, &2, U32, _ => U32>>) -> W.W & Maybe<&2, String>:
  match table:
    case Nil{}:
      (rng, None{})
    case Con{+h, +t}:
      cards.Book.roll.pick(cs, table, loot.Rng.pick(rng, cards.Book.weights(table)))

def cards.Book.roll.pick(+cs: List<&2, J.Val>, +table: List<&2, Sigma<&2, &2, U32, _ => U32>>, +r: W.W & U32) -> W.W & Maybe<&2, String>:
  match r:
    case (+rng, +n):
      (rng, Some{J.str(J.get(home.util.get(cs, cards.Book.index_at(table, n)), "id"))})

def cards.Book.roll_drop(+book: J.Val, +rng: W.W, +pack: String) -> W.W & Maybe<&2, String>:
  cards.Book.roll(book, rng, pack, 0)

def cards.Book.roll_spoil(+book: J.Val, +rng: W.W, +pack: String) -> W.W & Maybe<&2, String>:
  cards.Book.roll(book, rng, pack, 1)

# A chest holds one prize plus two ordinary drops: the generator and the ids.
def cards.Book.roll_chest(+book: J.Val, +rng: W.W, +pack: String) -> W.W & List<&2, String>:
  cards.Book.roll_chest.prize(book, pack, cards.Book.roll(book, rng, pack, 2))

def cards.Book.roll_chest.prize(+book: J.Val, +pack: String, +r: W.W & Maybe<&2, String>) -> W.W & List<&2, String>:
  match r:
    case (+rng, +prize):
      cards.Book.roll_chest.rest(book, pack, rng, prize)

def cards.Book.roll_chest.rest(+book: J.Val, +pack: String, +rng: W.W, +prize: Maybe<&2, String>) -> W.W & List<&2, String>:
  cards.Book.roll_chest.second(book, pack, prize, cards.Book.roll(book, rng, pack, 0))

def cards.Book.roll_chest.second(+book: J.Val, +pack: String, +prize: Maybe<&2, String>, +r: W.W & Maybe<&2, String>) -> W.W & List<&2, String>:
  match r:
    case (+rng, +first):
      cards.Book.roll_chest.third(prize, first, cards.Book.roll(book, rng, pack, 0))

def cards.Book.roll_chest.third(+prize: Maybe<&2, String>, +first: Maybe<&2, String>, +r: W.W & Maybe<&2, String>) -> W.W & List<&2, String>:
  match r:
    case (+rng, +second):
      (rng, List.concat(&2, String, [cauldron.one(prize), cauldron.one(first), cauldron.one(second)]))

def cards.case.ok(+input: J.Val) -> J.Val:
  input

# Play cards a knight can carry; quaff plays the first.
def cards.HAND() -> U32:
  4

# Spells a knight can learn; keys 1-4 cast them.
def cards.SPELL_SLOTS() -> U32:
  4

# home.rs: the party stands at home, at the top of the Winding Stair
def home.Run.at_home_now(+run: J.Val) -> Bool:
  U32.is_eq(J.u32(J.get(J.get(run, "dungeon"), "depth")), 0)

# `Home::level`: how far up a station's ladder the realm has built (0 unbuilt).
def home.Home.level(+home: J.Val, +station: String) -> U32:
  J.u32(J.get(J.get(home, "levels"), station))

# ---- yard.rs: Grubbins' stall ---------------------------------------------------

# What a card of each rarity costs at Grubbins' stall, in gold.
def yard.stall_price(+card: J.Val) -> U32:
  home.util.ite(U32, String.eq(J.str(J.get(card, "rarity")), "common"), u => 60,
    u => home.util.ite(U32, String.eq(J.str(J.get(card, "rarity")), "rare"), u => 150, u => 300))

# The stall's plate a point stands on: 0, 1 or 2, else 9.
def yard.stall_plate(+x: F32, +y: F32) -> U32:
  home.util.ite(U32, talents.inside(17, 11, 2, 1, x, y), u => 0,
    u => home.util.ite(U32, talents.inside(19, 11, 2, 1, x, y), u => 1,
      u => home.util.ite(U32, talents.inside(21, 11, 2, 1, x, y), u => 2, u => 9)))
