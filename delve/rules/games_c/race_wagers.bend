# crates/delve/src/rules/race_wagers.rs — a finite spectator race card.

def race_wagers.HEAT_TICKS() -> U32:
  300

def race_wagers.STAKE() -> U32:
  10

def race_wagers.RETURN() -> U32:
  25

# pastimes::RIVALS
def race_wagers.RIVALS(+i: U32) -> String:
  gc.nth_s(["SIR RATTLE", "GOAT MEATLOAF", "ABBOT OVERDRIVE"], i)

# Heat::draw(): the stored draw, else heat % 3.
def race_wagers.Heat.draw(+h: J.Val) -> U32:
  gc.ifu(J.is_null(J.get(h, "draw")), _ => U32.mod(gc.u(h, "heat"), 3), _ => gc.u(h, "draw"))

# pub fn draw(seed, raid_id, heat) -> u8
def race_wagers.draw(+seed: W.W, +raid_id: W.W, +heat: U32) -> U32:
  race_wagers.draw.go(loot.Rng.below(loot.Rng.new(W.xor(W.xor(W.xor(seed, W.rotl(raid_id, 29)), W.rotl(W.of_u32((heat + 1 : U32)), 47)), W.W{30561, 1734701568})), 3))

def race_wagers.draw.go(rv: W.W & U32) -> U32:
  (+r, +v) = rv
  v

# pub fn distance(draw, racer, tick) -> u32
def race_wagers.distance(+draw: U32, +racer: U32, +tick: U32) -> U32:
  race_wagers.distance.go(U32.mod((racer + 3 - U32.mod(draw, 3) : U32), 3), gc.umin(tick, race_wagers.HEAT_TICKS()))

def race_wagers.speeds(+role: U32) -> List<&2, U32>:
  gc.ifj_us(U32.is_eq(role, 0), _ => [18, 19, 13], _ => gc.ifj_us(U32.is_eq(role, 1), _ => [13, 16, 22], _ => [16, 18, 14]))

def race_wagers.distance.go(+role: U32, +tick: U32) -> U32:
  (race_wagers.part(tick, 0, gc.nth_u(race_wagers.speeds(role), 0)) + race_wagers.part(tick, 1, gc.nth_u(race_wagers.speeds(role), 1)) + race_wagers.part(tick, 2, gc.nth_u(race_wagers.speeds(role), 2)) : U32)

# tick.saturating_sub(part * (HEAT_TICKS / 3)).min(HEAT_TICKS / 3) * speed
def race_wagers.part(+tick: U32, +part: U32, +speed: U32) -> U32:
  (gc.umin(gc.ssub(tick, (part * 100 : U32)), 100) * speed : U32)

# pub fn winner(draw) -> u8: the last of the farthest.
def race_wagers.winner(+draw: U32) -> U32:
  race_wagers.winner.go(race_wagers.distance(draw, 0, 300), race_wagers.distance(draw, 1, 300), race_wagers.distance(draw, 2, 300))

def race_wagers.winner.go(+d0: U32, +d1: U32, +d2: U32) -> U32:
  gc.pu(Bool.and(U32.is_ge(d2, d0), U32.is_ge(d2, d1)), 2, gc.pu(U32.is_ge(d1, d0), 1, 0))

# pub fn finished(heat, mark) -> u8
def race_wagers.finished(+heat: U32, +mark: U32) -> U32:
  gc.ifu(Bool.and(U32.is_ge(mark, 2), U32.is_le(mark, 4)), _ => (mark - 2 : U32), _ => race_wagers.winner(U32.mod(heat, 3)))

# pub fn next(country) -> Option<usize>: the first heat not yet in the mask, or 99.
def race_wagers.next(+country: J.Val) -> U32:
  gc.ifu(Bool.not(gc.has_bit(gc.u(country, "wager_mask"), 0)), _ => 0,
    _ => gc.ifu(Bool.not(gc.has_bit(gc.u(country, "wager_mask"), 1)), _ => 1,
      _ => gc.ifu(Bool.not(gc.has_bit(gc.u(country, "wager_mask"), 2)), _ => 2, _ => 99)))

# fn tick_race_wagers(&mut self, inputs)
def race_wagers.Run.tick_race_wagers(+run: J.Val, +inputs: J.Val) -> J.Val:
  gc.ifj(Bool.or(Bool.not(String.eq(J.str(J.get(rules.Run.room(run), "kind")), "hippodrome")), Bool.not(J.is_null(J.get(J.get(run, "pastimes"), "round")))),
    _ => race_wagers.clear(run),
    _ => race_wagers.Run.tick_race_wagers.a(run, inputs, J.get(J.get(run, "pastimes"), "wager")))

def race_wagers.pastimes(+run: J.Val, +f: String, +v: J.Val) -> J.Val:
  J.set(run, "pastimes", J.set(J.get(run, "pastimes"), f, v))

def race_wagers.clear(+run: J.Val) -> J.Val:
  race_wagers.pastimes(race_wagers.pastimes(race_wagers.pastimes(run, "wager", J.JNull{}), "wager_key", J.JBool{False{}}), "wager_owner", J.JNull{})

def race_wagers.Run.tick_race_wagers.a(+run: J.Val, +inputs: J.Val, +race: J.Val) -> J.Val:
  gc.ifj(J.is_null(race),
    _ => race_wagers.Run.tick_race_wagers.idle(run, inputs),
    _ => race_wagers.Run.tick_race_wagers.running(run, inputs, race))

# A heat under way: the key observed, the clock one tick on, the result at the line.
def race_wagers.Run.tick_race_wagers.running(+run: J.Val, +inputs: J.Val, +race: J.Val) -> J.Val:
  race_wagers.Run.tick_race_wagers.line(
    race_wagers.pastimes(race_wagers.pastimes(run, "wager", J.JNull{}), "wager_key",
      J.JBool{gc.ifb(J.is_null(J.get(J.get(run, "pastimes"), "wager_owner")), _ => False{}, _ => gc.pressed_interact(inputs, J.u32(J.get(J.get(run, "pastimes"), "wager_owner"))))}),
    gc.su(race, "elapsed", (gc.u(race, "elapsed") + 1 : U32)))

def race_wagers.Run.tick_race_wagers.line(+run: J.Val, +race: J.Val) -> J.Val:
  gc.ifj(U32.is_eq(gc.u(race, "elapsed"), race_wagers.HEAT_TICKS()),
    _ => race_wagers.won(run, race, race_wagers.winner(race_wagers.Heat.draw(race))),
    _ => race_wagers.pastimes(run, "wager", race))

def race_wagers.won(+run: J.Val, +race: J.Val, +won: U32) -> J.Val:
  gc.found(gc.mark(run, hippodrome.WAGER_MARKS((gc.u(race, "heat") + 3 : U32)), (won + 2 : U32)), 0,
    race_wagers.RIVALS(won) ++ " wins heat " ++ U32.show((gc.u(race, "heat") + 1 : U32)) ++ "! Ticket settles on save; winning return 25 gold.")

def race_wagers.Run.tick_race_wagers.idle(+run: J.Val, +inputs: J.Val) -> J.Val:
  race_wagers.Run.tick_race_wagers.next(run, inputs, race_wagers.next(gc.hcountry(run)))

def race_wagers.Run.tick_race_wagers.next(+run: J.Val, +inputs: J.Val, +heat: U32) -> J.Val:
  gc.ifj(Bool.or(U32.is_eq(heat, 99), gc.marked(run, hippodrome.WAGER_MARKS((heat + 3 : U32)))),
    _ => run,
    _ => race_wagers.Run.tick_race_wagers.owner(run, inputs, heat, race_wagers.owner(run, inputs)))

# visitors: knights at the booth, in id order.
def race_wagers.visitors(+run: J.Val, +ids: List<&2, U32>) -> List<&2, U32>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      gc.ifx_us(race_wagers.visiting(gc.seat(J.get(run, "players"), id)), id, race_wagers.visitors(run, t))

def gc.ifx_us(+yes: Bool, +id: U32, rest: List<&2, U32>) -> List<&2, U32>:
  match yes:
    case True{}:
      id <> rest
    case False{}:
      rest

def race_wagers.visiting(+h: J.Val) -> Bool:
  Bool.and(U32.is_gt(gc.u(h, "hp"), 0), Bool.and(Bool.not(gc.b(h, "stone")), Bool.and(U32.is_zero(gc.u(h, "privy")), race_wagers.at_booth(gc.f(h, "x"), gc.f(h, "y")))))

def race_wagers.at_booth(+x: F32, +y: F32) -> Bool:
  F32.is_lt(RS.f_hypot((x - 17.0 * layout.TILE_UNITS() : F32), (y - 4.5 * layout.TILE_UNITS() : F32)), 1.4)

# The lowest-id visitor pressing a valid control, else the first visitor; 0 for none.
def race_wagers.owner(+run: J.Val, +inputs: J.Val) -> U32:
  race_wagers.owner.go(race_wagers.visitors(run, gc.ids(J.get(run, "players"))), inputs, race_wagers.first(race_wagers.visitors(run, gc.ids(J.get(run, "players")))))

def race_wagers.first(+ids: List<&2, U32>) -> U32:
  match ids:
    case Nil{}:
      0
    case Con{+h, +t}:
      h

def race_wagers.owner.go(+ids: List<&2, U32>, +inputs: J.Val, +dflt: U32) -> U32:
  match ids:
    case Nil{}:
      dflt
    case Con{+id, +t}:
      gc.ifu(race_wagers.active(gc.seat(inputs, id)), _ => id, _ => race_wagers.owner.go(t, inputs, dflt))

# a valid input pressing interact or any axis
def race_wagers.active(+i: J.Val) -> Bool:
  gc.ifb(J.is_null(i), _ => False{},
    _ => Bool.and(rules.Input.valid(i), Bool.or(gc.b(i, "interact"), Bool.or(U32.is_ne(gc.u(i, "aim_x"), 0), Bool.or(U32.is_ne(gc.u(i, "aim_y"), 0), Bool.or(U32.is_ne(gc.u(i, "move_x"), 0), U32.is_ne(gc.u(i, "move_y"), 0)))))))

def race_wagers.Run.tick_race_wagers.owner(+run: J.Val, +inputs: J.Val, +heat: U32, +id: U32) -> J.Val:
  gc.ifj(U32.is_zero(id),
    _ => race_wagers.pastimes(race_wagers.pastimes(run, "wager_key", J.JBool{False{}}), "wager_owner", J.JNull{}),
    _ => race_wagers.Run.tick_race_wagers.seat(
      race_wagers.pastimes(race_wagers.pastimes(run, "wager_key", J.JBool{gc.ifb(gc.opt_is(J.get(J.get(run, "pastimes"), "wager_owner"), id), _ => gc.b(J.get(run, "pastimes"), "wager_key"), _ => False{})}), "wager_owner", J.JU{id}),
      inputs, heat, id, gc.input(inputs, id)))

def race_wagers.Run.tick_race_wagers.seat(+run: J.Val, +inputs: J.Val, +heat: U32, +id: U32, +input: J.Val) -> J.Val:
  race_wagers.Run.tick_race_wagers.tap(
    race_wagers.pastimes(run, "wager_key", J.JBool{gc.b(input, "interact")}),
    heat, id, input, Bool.and(gc.b(input, "interact"), Bool.not(gc.b(J.get(run, "pastimes"), "wager_key"))),
    J.u32(J.nth(J.get(gc.hcountry(run), "wager_tickets"), heat)),
    J.u32(J.get(J.get(run, "marks"), hippodrome.WAGER_MARKS(heat))))

# The pick for the heat: the ticket's, a pending mark's, else the stick moves the pick.
def race_wagers.Run.tick_race_wagers.tap(+run: J.Val, +heat: U32, +id: U32, +input: J.Val, +tapped: Bool, +ticket: U32, +pending: U32) -> J.Val:
  gc.ifj(U32.is_gt(ticket, 0),
    _ => race_wagers.Run.tick_race_wagers.pay(run, heat, id, tapped, ticket, pending, (ticket - 1 : U32)),
    _ => gc.ifj(Bool.and(U32.is_ge(pending, 1), U32.is_le(pending, 3)),
      _ => race_wagers.Run.tick_race_wagers.pay(run, heat, id, tapped, ticket, pending, (pending - 1 : U32)),
      _ => race_wagers.Run.tick_race_wagers.aim(run, heat, id, tapped, ticket, pending, input)))

def race_wagers.Run.tick_race_wagers.aim(+run: J.Val, +heat: U32, +id: U32, +tapped: Bool, +ticket: U32, +pending: U32, +input: J.Val) -> J.Val:
  race_wagers.Run.tick_race_wagers.aim.go(run, heat, id, tapped, ticket, pending, gc.ifu(U32.is_ne(gc.u(input, "aim_x"), 0), _ => gc.u(input, "aim_x"), _ => gc.u(input, "move_x")), gc.neg(gc.u(input, "aim_y")), gc.neg(gc.u(input, "move_y")))

def race_wagers.Run.tick_race_wagers.aim.go(+run: J.Val, +heat: U32, +id: U32, +tapped: Bool, +ticket: U32, +pending: U32, +axis: U32, +up_a: Bool, +up_m: Bool) -> J.Val:
  race_wagers.Run.tick_race_wagers.aim.set(run, heat, id, tapped, ticket, pending,
    gc.ifu(gc.neg(axis), _ => 0, _ => gc.ifu(Bool.and(U32.is_ne(axis, 0), Bool.not(gc.neg(axis))), _ => 2,
      _ => gc.ifu(Bool.or(up_a, up_m), _ => 1, _ => gc.u(J.get(run, "pastimes"), "wager_pick")))))

def race_wagers.Run.tick_race_wagers.aim.set(+run: J.Val, +heat: U32, +id: U32, +tapped: Bool, +ticket: U32, +pending: U32, +pick: U32) -> J.Val:
  race_wagers.Run.tick_race_wagers.pay(race_wagers.pastimes(run, "wager_pick", J.JU{pick}), heat, id, tapped, ticket, pending, pick)

def race_wagers.Run.tick_race_wagers.pay(+run: J.Val, +heat: U32, +id: U32, +tapped: Bool, +ticket: U32, +pending: U32, +pick: U32) -> J.Val:
  gc.ifj(Bool.not(tapped),
    _ => run,
    _ => gc.ifj(Bool.and(U32.is_zero(ticket), Bool.and(U32.is_zero(pending), U32.is_lt(realm.Spoils.get(J.get(run, "treasury"), "gold"), race_wagers.STAKE()))),
      _ => gc.found(run, id, "A ticket costs 10 gold. Free practice is at C; footpaths are free."),
      _ => race_wagers.Run.tick_race_wagers.book(run, heat, id, pick)))

def race_wagers.Run.tick_race_wagers.book(+run: J.Val, +heat: U32, +id: U32, +pick: U32) -> J.Val:
  gc.found(
    race_wagers.pastimes(
      gc.mark(run, hippodrome.WAGER_MARKS(heat), gc.ifu(gc.marked(run, hippodrome.WAGER_MARKS(heat)), _ => J.u32(J.get(J.get(run, "marks"), hippodrome.WAGER_MARKS(heat))), _ => (pick + 1 : U32))),
      "wager",
      race_wagers.heat_new(heat, pick, race_wagers.draw(W.of_j(J.get(run, "seed")), W.of_j(J.get(run, "raid_id")), heat))),
    id, "Heat " ++ U32.show((heat + 1 : U32)) ++ ": backing " ++ race_wagers.RIVALS(pick) ++ ". 10 gold, wins 25.")

def race_wagers.heat_new(+heat: U32, +pick: U32, +draw: U32) -> J.Val:
  gc.su(gc.su(gc.su(gc.su(J.obj(), "heat", heat), "pick", pick), "elapsed", 0), "draw", draw)

# pub fn settle(country, treasury, marks, news) -> (country, treasury, news)
def race_wagers.settle(+country: J.Val, +treasury: J.Val, +marks: J.Val, +news: J.Val) -> J.Val & J.Val & J.Val:
  race_wagers.settle.go(0, country, treasury, marks, news)

def race_wagers.settle.go(+i: U32, +country: J.Val, +treasury: J.Val, +marks: J.Val, +news: J.Val) -> J.Val & J.Val & J.Val:
  gc.if(J.Val & J.Val & J.Val, U32.is_ge(i, 3),
    _ => (country, treasury, news),
    _ => gc.if(J.Val & J.Val & J.Val, gc.has_bit(gc.u(country, "wager_mask"), i),
      _ => race_wagers.settle.go((i + 1 : U32), country, treasury, marks, news),
      _ => race_wagers.settle.ticket(i, country, treasury, marks, news)))

def race_wagers.settle.ticket(+i: U32, +country: J.Val, +treasury: J.Val, +marks: J.Val, +news: J.Val) -> J.Val & J.Val & J.Val:
  gc.if(J.Val & J.Val & J.Val, Bool.and(U32.is_zero(J.u32(J.nth(J.get(country, "wager_tickets"), i))), Bool.and(U32.is_ge(J.u32(J.get(marks, hippodrome.WAGER_MARKS(i))), 1), U32.is_le(J.u32(J.get(marks, hippodrome.WAGER_MARKS(i))), 3))),
    _ => gc.if(J.Val & J.Val & J.Val, U32.is_lt(realm.Spoils.get(treasury, "gold"), 10),
      _ => race_wagers.settle.go((i + 1 : U32), country, treasury, marks, J.push(news, J.JS{"The bookie refuses an unfunded ticket. No stake taken and no winnings paid."})),
      _ => race_wagers.settle.result(i, J.set(country, "wager_tickets", J.set_nth(J.get(country, "wager_tickets"), i, J.JU{J.u32(J.get(marks, hippodrome.WAGER_MARKS(i)))})), realm.Spoils.take(treasury, J.set(J.obj(), "gold", J.JU{10})), marks, J.push(news, J.JS{"Heat " ++ U32.show((i + 1 : U32)) ++ " ticket: 10 gold paid once."}))),
    _ => race_wagers.settle.result(i, country, treasury, marks, news))

def race_wagers.settle.result(+i: U32, +country: J.Val, +treasury: J.Val, +marks: J.Val, +news: J.Val) -> J.Val & J.Val & J.Val:
  gc.if(J.Val & J.Val & J.Val, Bool.and(U32.is_gt(J.u32(J.nth(J.get(country, "wager_tickets"), i)), 0), J.has(marks, hippodrome.WAGER_MARKS((i + 3 : U32)))),
    _ => race_wagers.settle.paid(i, J.u32(J.get(marks, hippodrome.WAGER_MARKS((i + 3 : U32)))), J.u32(J.nth(J.get(country, "wager_tickets"), i)), gc.su(country, "wager_mask", U32.or(gc.u(country, "wager_mask"), gc.bit(i))), treasury, marks, news),
    _ => race_wagers.settle.go((i + 1 : U32), country, treasury, marks, news))

def race_wagers.settle.paid(+i: U32, +mark: U32, +ticket: U32, +country: J.Val, +treasury: J.Val, +marks: J.Val, +news: J.Val) -> J.Val & J.Val & J.Val:
  race_wagers.settle.paid.go(i, race_wagers.finished(i, mark), U32.is_eq((ticket - 1 : U32), race_wagers.finished(i, mark)), country, treasury, marks, news)

def race_wagers.settle.paid.go(+i: U32, +first: U32, +won: Bool, +country: J.Val, +treasury: J.Val, +marks: J.Val, +news: J.Val) -> J.Val & J.Val & J.Val:
  race_wagers.settle.go((i + 1 : U32), country,
    gc.ifj(won, _ => realm.Spoils.add(treasury, "gold", 25), _ => treasury), marks,
    J.push(news, J.JS{"Heat " ++ U32.show((i + 1 : U32)) ++ ": " ++ race_wagers.RIVALS(first) ++ " wins. " ++ gc.ps(won, "25 gold returned", "ticket lost") ++ "."}))
