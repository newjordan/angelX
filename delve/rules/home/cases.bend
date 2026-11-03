# Case wrappers: `case.<def>` takes a recorded case's `in` and answers its `out`.
# (compute.* and cards.* wrappers sit beside their defs.)

def case.presence.Run.set_away(+input: J.Val) -> J.Val:
  presence.Run.set_away(J.get(input, "run"), J.u32(J.get(input, "id")), J.bool(J.get(input, "away")))

def case.presence.Run.set_absent(+input: J.Val) -> J.Val:
  presence.Run.set_absent(J.get(input, "run"), J.u32(J.get(input, "id")), J.bool(J.get(input, "absent")))

def case.presence.Run.rest_until_moved(+input: J.Val) -> J.Val:
  presence.Run.rest_until_moved(J.get(input, "run"))

def case.presence.Run.ward_while_held(+input: J.Val) -> J.Val:
  presence.Run.ward_while_held(J.get(input, "run"))

def case.presence.Run.hold(+input: J.Val) -> J.Val:
  presence.Run.hold(J.get(input, "run"), J.str(J.get(input, "hold")))

def case.presence.Run.wake_on_first_move(+input: J.Val) -> J.Val:
  presence.Run.wake_on_first_move(J.get(input, "hero"), J.get(input, "input"))

def case.fellowship.Run.call_fellowship(+input: J.Val) -> J.Val:
  fellowship.Run.call_fellowship(J.get(input, "run"))

def case.fellowship.Run.clear(+input: J.Val) -> J.Val:
  fellowship.Run.clear(J.get(input, "run"))

def case.fellowship.Run.wait(+input: J.Val) -> J.Val:
  fellowship.Run.wait(J.get(input, "run"), J.items(J.get(input, "ways")))

def case.fellowship.Run.fellowship_line(+input: J.Val) -> J.Val:
  fellowship.Run.fellowship_line(J.get(input, "run"), J.get(input, "focus"))

def case.fellowship.Run.counts_in_fellowship(+input: J.Val) -> J.Val:
  J.JBool{fellowship.Run.counts_in_fellowship(J.get(input, "run"), J.u32(J.get(input, "id")))}

def case.shield.turns(+input: J.Val) -> J.Val:
  J.JBool{shield.turns(J.get(input, "h"), J.f32(J.get(input, "dx")), J.f32(J.get(input, "dy")))}

def case.shield.entry(+input: J.Val) -> J.Val:
  casex.opt_f32(shield.entry(J.f32(J.get(input, "ax")), J.f32(J.get(input, "ay")), J.f32(J.get(input, "bx")), J.f32(J.get(input, "by")), J.f32(J.get(input, "cx")), J.f32(J.get(input, "cy")), J.f32(J.get(input, "r"))))

def casex.opt_f32(+m: Maybe<&2, F32>) -> J.Val:
  match m:
    case Some{+f}:
      J.JF{f}
    case None{}:
      J.JNull{}

def casex.opt_j(+m: Maybe<&2, J.Val>) -> J.Val:
  match m:
    case Some{+v}:
      v
    case None{}:
      J.JNull{}

def case.shield.meets(+input: J.Val) -> J.Val:
  casex.opt_j(shield.meets(J.get(input, "h"), J.f32(J.get(input, "cx")), J.f32(J.get(input, "cy")), J.f32(J.get(input, "ax")), J.f32(J.get(input, "ay")), J.f32(J.get(input, "bx")), J.f32(J.get(input, "by")), J.f32(J.get(input, "body"))))

def case.shield.Hero.shield_turned(+input: J.Val) -> J.Val:
  casex.pair_hk(shield.Hero.shield_turned(J.get(input, "h"), J.u32(J.get(input, "damage"))))

def casex.pair_hk(r: J.Val & Bool) -> J.Val:
  (h, k) = r
  home.util.obj([("h", h), ("knocked", J.JBool{k})])

def case.shield.Wear.rest(+input: J.Val) -> J.Val:
  shield.Hero.put_wear(J.get(input, "h"), shield.Wear.rest(J.get(J.get(input, "h"), "shield_wear"), J.u32(J.get(J.get(input, "h"), "max_hp"))))

def case.shield.Wear.left(+input: J.Val) -> J.Val:
  J.JF{shield.Wear.left(J.get(J.get(input, "h"), "shield_wear"), J.u32(J.get(J.get(input, "h"), "max_hp")))}

def case.shield.Wear.back(+input: J.Val) -> J.Val:
  J.JF{shield.Wear.back(J.get(J.get(input, "h"), "shield_wear"))}

def case.shield.Wear.fresh(+input: J.Val) -> J.Val:
  J.JBool{shield.Wear.fresh(J.get(J.get(input, "h"), "shield_wear"))}

def case.spell_upgrades.Hero.spell_tier(+input: J.Val) -> J.Val:
  J.JU{spell_upgrades.Hero.spell_tier(J.get(input, "h"), J.str(J.get(input, "id")))}

def case.spell_upgrades.Hero.spell_power(+input: J.Val) -> J.Val:
  J.JU{spell_upgrades.Hero.spell_power(J.get(input, "h"), J.str(J.get(input, "id")), J.u32(J.get(input, "base")))}

def case.spell_upgrades.Hero.spell_step(+input: J.Val) -> J.Val:
  spell_upgrades.Hero.spell_step(J.get(input, "h"), J.str(J.get(input, "id")))

def case.spell_upgrades.Hero.spell_wait(+input: J.Val) -> J.Val:
  J.JU{spell_upgrades.Hero.spell_wait(J.get(input, "h"), J.get(input, "card"))}

def case.spell_upgrades.Hero.spell_name(+input: J.Val) -> J.Val:
  J.JS{spell_upgrades.Hero.spell_name(J.get(input, "h"), J.get(input, "card"))}

def case.overclass.Run.give_wishes(+input: J.Val) -> J.Val:
  overclass.Run.give_wishes(J.get(input, "run"), J.u32(J.get(input, "plain")), J.u32(J.get(input, "great")))

def case.overclass.Run.can_wish(+input: J.Val) -> J.Val:
  J.JBool{overclass.Run.can_wish(J.get(input, "run"), J.u32(J.get(input, "id")))}

def case.overclass.Run.spend_wish_kind(+input: J.Val) -> J.Val:
  casex.pair_run_spent(overclass.Run.spend_wish_kind(J.get(input, "run"), J.u32(J.get(input, "id")), J.bool(J.get(input, "great"))))

def casex.pair_run_spent(r: J.Val & Bool) -> J.Val:
  (run, spent) = r
  home.util.obj([("run", run), ("spent", J.JBool{spent})])

def case.overclass.Run.tell(+input: J.Val) -> J.Val:
  overclass.Run.tell(J.get(input, "run"), J.u32(J.get(input, "id")), J.str(J.get(input, "line")))

def case.overclass.Run.refund_told(+input: J.Val) -> J.Val:
  overclass.Run.refund_told(J.get(input, "run"), J.u32(J.get(input, "id")), J.bool(J.get(input, "great")), J.str(J.get(input, "why")))

def case.overclass.Run.wishes_for_newcomer(+input: J.Val) -> J.Val:
  overclass.Run.wishes_for_newcomer(J.get(input, "run"), J.u32(J.get(input, "id")))

def case.overclass.Run.made(+input: J.Val) -> J.Val:
  overclass.Run.made(J.get(input, "run"), J.u32(J.get(input, "id")), J.str(J.get(input, "wish")), J.str(J.get(input, "name")))

def case.overclass.wish_returned(+input: J.Val) -> J.Val:
  J.JS{overclass.wish_returned(J.str(J.get(input, "why")))}

def case.overclass.wish_kept(+input: J.Val) -> J.Val:
  J.JS{overclass.wish_kept(J.str(J.get(input, "why")))}

def case.dead_wishes.Gifts.hit(+input: J.Val) -> J.Val:
  J.JU{dead_wishes.Gifts.hit(J.get(input, "g"), J.u32(J.get(input, "weight")))}

def case.dead_wishes.Gifts.rest(+input: J.Val) -> J.Val:
  J.JU{dead_wishes.Gifts.rest(J.get(input, "g"), J.u32(J.get(input, "ticks")))}

def case.dead_wishes.Gifts.pace(+input: J.Val) -> J.Val:
  J.JF{dead_wishes.Gifts.pace(J.get(input, "g"))}

def case.dead_wishes.Gifts.taken(+input: J.Val) -> J.Val:
  J.JU{dead_wishes.Gifts.taken(J.get(input, "g"), J.u32(J.get(input, "damage")))}

def case.cards.Book.get(+input: J.Val) -> J.Val:
  cards.Book.get(J.get(input, "book"), J.str(J.get(input, "id")))

def case.cards.Book.insert(+input: J.Val) -> J.Val:
  casex.pair_book_ok(cards.Book.insert(J.get(input, "book"), J.get(input, "card")))

def casex.pair_book_ok(r: J.Val & Bool) -> J.Val:
  (book, ok) = r
  home.util.obj([("book", book), ("ok", J.JBool{ok})])

def case.cards.Card.is_spoil(+input: J.Val) -> J.Val:
  J.JBool{cards.Card.is_spoil(J.get(input, "card"))}

def case.cards.Card.drinks(+input: J.Val) -> J.Val:
  J.JBool{cards.Card.drinks(J.get(input, "card"))}

def case.cards.Card.scroll(+input: J.Val) -> J.Val:
  J.JBool{cards.Card.scroll(J.get(input, "card"))}

def case.home.Home.prowess(+input: J.Val) -> J.Val:
  home.Home.prowess(J.get(input, "hm"), J.str(J.get(input, "who")))

def case.home.Home.learn(+input: J.Val) -> J.Val:
  casex.pair_hm_r(home.Home.learn(J.get(input, "hm"), J.str(J.get(input, "who")), J.u32(J.get(input, "side"))))

def casex.pair_hm_r(r: J.Val & J.Val) -> J.Val:
  (hm, res) = r
  home.util.obj([("hm", home.util.obj([("knights", J.get(hm, "knights"))])), ("r", res)])

def case.talents.Prowess.level(+input: J.Val) -> J.Val:
  J.JU{talents.Prowess.level(J.get(input, "p"))}

def case.talents.Prowess.waiting(+input: J.Val) -> J.Val:
  J.JU{talents.Prowess.waiting(J.get(input, "p"))}

def case.talents.knight_key(+input: J.Val) -> J.Val:
  J.JS{talents.knight_key(J.get(input, "h"))}

def case.talents.lectern_at(+input: J.Val) -> J.Val:
  J.JU{talents.lectern_at(J.f32(J.get(input, "x")), J.f32(J.get(input, "y")))}

def case.talents.Run.tick_lessons(+input: J.Val) -> J.Val:
  talents.Run.tick_lessons(J.get(input, "run"), J.get(input, "inputs"))

def case.rescues.Run.tick_cage(+input: J.Val) -> J.Val:
  rescues.Run.tick_cage(J.get(input, "run"), J.get(input, "inputs"))

def case.rescues.Run.residents_help(+input: J.Val) -> J.Val:
  rescues.Run.residents_help(J.get(input, "run"))

def case.rescues.Run.greet_residents(+input: J.Val) -> J.Val:
  rescues.Run.greet_residents(J.get(input, "run"))

def case.rescues.Run.pips_map(+input: J.Val) -> J.Val:
  rescues.Run.pips_map(J.get(input, "run"))

def case.rescues.resident(+input: J.Val) -> J.Val:
  rescues.resident(J.str(J.get(input, "id")))

def case.yard.stall_price(+input: J.Val) -> J.Val:
  J.JU{yard.stall_price(J.get(input, "card"))}

def case.yard.stall_plate(+input: J.Val) -> J.Val:
  casex.plate(yard.stall_plate(J.f32(J.get(input, "x")), J.f32(J.get(input, "y"))))

def casex.plate(+n: U32) -> J.Val:
  home.util.ite(J.Val, U32.is_eq(n, 9), u => J.JNull{}, u => J.JU{n})
