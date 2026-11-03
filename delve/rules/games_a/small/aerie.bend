# crates/delve/src/rules/aerie.rs — the eastern high roads and the pigeon post.
# Rooms, runs and pigeons are their serde J objects; the u64 tick is `W.W`.

def aerie.LAP() -> U32:
  240

def aerie.BONK() -> F32:
  1.3

def aerie.POOP_EVERY() -> U32:
  90

def aerie.POOP_REACH() -> F32:
  1.2

def aerie.SWING_REACH() -> F32:
  2.2

def aerie.TAU() -> F32:
  6.2831855

# CLERK = (5.0, 10.0).
def aerie.CLERK_X() -> F32:
  5.0

def aerie.CLERK_Y() -> F32:
  10.0

# The paper whose room is `kind`: 0, 1, 2, else 3.
def aerie.doc_index(+kind: String) -> U32:
  ga.pu(String.eq(kind, "belfry"), 0, ga.pu(String.eq(kind, "aqueduct"), 1, ga.pu(String.eq(kind, "aviary"), 2, 3)))

def aerie.doc_x(+i: U32) -> F32:
  ga.pf(U32.is_eq(i, 0), 20.0, ga.pf(U32.is_eq(i, 1), 5.0, 19.0))

def aerie.doc_y(+i: U32) -> F32:
  ga.pf(U32.is_eq(i, 0), 10.0, ga.pf(U32.is_eq(i, 1), 8.5, 6.5))

def aerie.doc_name(+i: U32) -> String:
  ga.ps(U32.is_eq(i, 0), "BELL TAX", ga.ps(U32.is_eq(i, 1), "WATER TAX", "SEED TAX"))

def aerie.doc_room(+i: U32) -> String:
  ga.ps(U32.is_eq(i, 0), "belfry", ga.ps(U32.is_eq(i, 1), "aqueduct", "aviary"))

def aerie.quest_mark(+i: U32) -> String:
  ga.ps(U32.is_eq(i, 0), "country:minutes:accept", ga.ps(U32.is_eq(i, 1), "country:minutes:0",
    ga.ps(U32.is_eq(i, 2), "country:minutes:1", ga.ps(U32.is_eq(i, 3), "country:minutes:2", "country:minutes:deliver"))))

def aerie.visit_mark(+i: U32) -> String:
  ga.ps(U32.is_eq(i, 0), "country:aerie:0", ga.ps(U32.is_eq(i, 1), "country:aerie:1", "country:aerie:2"))

# The pigeons' state when there is none: `Pigeons::default()`.
def aerie.Pigeons.default() -> J.Val:
  ga.sv(ga.sv(ga.sv(J.obj(), "dropped", J.JNull{}), "bonks", J.obj()), "poop", J.obj())

def aerie.Pigeons.valid(+p: J.Val) -> Bool:
  Bool.and(Bool.and(U32.is_le(ga.nfields(J.get(p, "bonks")), rules.MAX_PLAYERS()), U32.is_le(ga.nfields(J.get(p, "poop")), rules.MAX_PLAYERS())),
    aerie.Pigeons.valid.dropped(J.get(p, "dropped")))

def aerie.Pigeons.valid.dropped(+d: J.Val) -> Bool:
  aerie.Pigeons.valid.dropped.in(J.is_null(d), d)

def aerie.Pigeons.valid.dropped.in(+none: Bool, +d: J.Val) -> Bool:
  match none:
    case True{}:
      True{}
    case False{}:
      Bool.and(U32.is_lt(ga.nu(d, 0), 3), Bool.and(
        Bool.and(F32.is_ge(ga.nf(d, 1), 0.0), F32.is_lt(ga.nf(d, 1), 200.0)),
        Bool.and(F32.is_ge(ga.nf(d, 2), 0.0), F32.is_lt(ga.nf(d, 2), 200.0))))

# Where paper `i`'s pigeon flies at `tick`, in units: a figure of eight over the
# middle of its room.
def aerie.pigeon_at(+room: J.Val, +i: U32, +tick: W.W) -> F32 & F32:
  aerie.pigeon_at.t(room, F32.mul(F32.div(RS.u_as_f32(W.rem_u32(W.add(tick, W.div_u32(W.mul_u32(W.of_u32(i), 240), 3)), 240)), 240.0), aerie.TAU()))

def aerie.pigeon_at.t(+room: J.Val, +t: F32) -> F32 & F32:
  (F32.mul(F32.add(F32.div(RS.u_as_f32(ga.u(room, "cols")), 2.0), F32.mul(F32.mul(F32.sin(t), RS.u_as_f32(ga.u(room, "cols"))), 0.32)), layout.TILE_UNITS()),
    F32.mul(F32.add(F32.div(RS.u_as_f32(ga.u(room, "rows")), 2.0), F32.mul(F32.mul(F32.sin(F32.mul(2.0, t)), RS.u_as_f32(ga.u(room, "rows"))), 0.22)), layout.TILE_UNITS()))

# The paper whose pigeon is up in this room, if one is: its index, or 3.
def aerie.flying(+kind: String, +stage: U32, +papers: U32, +pigeons: J.Val) -> U32:
  aerie.flying.in(aerie.doc_index(kind), stage, papers, pigeons)

def aerie.flying.in(+i: U32, +stage: U32, +papers: U32, +pigeons: J.Val) -> U32:
  aerie.flying.up(Bool.and(U32.is_lt(i, 3), Bool.and(U32.is_eq(stage, 1),
    Bool.and(U32.is_zero(U32.and(papers, RS.u_shl(1, i))), aerie.flying.free(pigeons, i)))), i)

def aerie.flying.free(+pigeons: J.Val, +i: U32) -> Bool:
  aerie.flying.free.in(J.is_null(pigeons), J.get(pigeons, "dropped"), i)

def aerie.flying.free.in(+none: Bool, +dropped: J.Val, +i: U32) -> Bool:
  match none:
    case True{}:
      True{}
    case False{}:
      Bool.or(J.is_null(dropped), U32.is_ne(ga.nu(dropped, 0), i))

def aerie.flying.up(+yes: Bool, +i: U32) -> U32:
  ga.pu(yes, i, 3)

# The commission stage and found-paper mask, with pending private intents.
def aerie.progress(+country: J.Val, +marks: J.Val) -> U32 & U32:
  aerie.progress.mask(
    ga.pu(Bool.and(U32.is_zero(ga.u(country, "post")), J.has(marks, aerie.quest_mark(0))), 1, ga.u(country, "post")),
    ga.u(country, "minutes"), marks)

def aerie.progress.mask(+stage: U32, +papers: U32, +marks: J.Val) -> U32 & U32:
  aerie.progress.deliver(stage,
    ga.pu(U32.is_gt(stage, 0),
      U32.or(U32.or(U32.or(papers, ga.pu(J.has(marks, aerie.quest_mark(1)), 1, 0)), ga.pu(J.has(marks, aerie.quest_mark(2)), 2, 0)), ga.pu(J.has(marks, aerie.quest_mark(3)), 4, 0)),
      papers), marks)

def aerie.progress.deliver(+stage: U32, +papers: U32, +marks: J.Val) -> U32 & U32:
  (ga.pu(Bool.and(U32.is_gt(stage, 0), Bool.and(U32.is_eq(papers, 7), J.has(marks, aerie.quest_mark(4)))), 2, stage), papers)

# --- shape: the sculptures and the stairs ------------------------------------------

# The block rectangles (col, row, w, h) of a kind, as a flat list.
def aerie.blocks(+kind: String) -> List<&2, U32>:
  aerie.blocks.in(String.eq(kind, "belfry"), String.eq(kind, "aqueduct"), String.eq(kind, "aviary"))

def aerie.blocks.in(+belfry: Bool, +aqueduct: Bool, +aviary: Bool) -> List<&2, U32>:
  match belfry:
    case True{}:
      [3, 2, 2, 3, 19, 2, 2, 3]
    case False{}:
      aerie.blocks.aq(aqueduct, aviary)

def aerie.blocks.aq(+aqueduct: Bool, +aviary: Bool) -> List<&2, U32>:
  match aqueduct:
    case True{}:
      [7, 2, 2, 3, 15, 2, 2, 3, 7, 10, 2, 2, 15, 10, 2, 2]
    case False{}:
      aerie.blocks.av(aviary)

def aerie.blocks.av(+aviary: Bool) -> List<&2, U32>:
  match aviary:
    case True{}:
      [3, 2, 5, 2]
    case False{}:
      Nil{}

# Fill the rectangle with `tile`.
def aerie.fill(+room: J.Val, +c: U32, +r: U32, +w: U32, +h: U32, +tile: String) -> J.Val:
  aerie.fill.row(room, c, r, w, r, (r + h : U32), tile)

def aerie.fill.row(+room: J.Val, +c: U32, +r: U32, +w: U32, +y: U32, +end: U32, +tile: String) -> J.Val:
  aerie.fill.row.m(U32.is_ge(y, end), room, c, r, w, y, end, tile)

def aerie.fill.row.m(+flag: Bool, +room: J.Val, +c: U32, +r: U32, +w: U32, +y: U32, +end: U32, +tile: String) -> J.Val:
  match flag:
    case True{}:
      room
    case False{}:
      aerie.fill.row(aerie.fill.col(room, c, w, y, tile, c, (c + w : U32)), c, r, w, (y + 1 : U32), end, tile)

def aerie.fill.col(+room: J.Val, +c: U32, +w: U32, +y: U32, +tile: String, +x: U32, +end: U32) -> J.Val:
  aerie.fill.col.m(U32.is_ge(x, end), room, c, w, y, tile, x, end)

def aerie.fill.col.m(+flag: Bool, +room: J.Val, +c: U32, +w: U32, +y: U32, +tile: String, +x: U32, +end: U32) -> J.Val:
  match flag:
    case True{}:
      room
    case False{}:
      aerie.fill.col(layout.Room.set(room, x, y, J.JS{tile}), c, w, y, tile, (x + 1 : U32), end)

def aerie.shape.blocks(+room: J.Val, +bs: List<&2, U32>) -> J.Val:
  match bs:
    case Nil{}:
      room
    case Con{+c, Con{+r, Con{+w, Con{+h, +t}}}}:
      aerie.shape.blocks(aerie.fill(room, c, r, w, h, "block"), t)
    case _:
      room

def aerie.shape(+room: J.Val) -> J.Val:
  aerie.shape.stairs(aerie.shape.blocks(room, aerie.blocks(ga.s(room, "kind"))), ga.s(room, "kind"))

def aerie.shape.stairs(+room: J.Val, +kind: String) -> J.Val:
  aerie.shape.stairs.in(String.eq(kind, "aviary"), String.eq(kind, "rookery"), room)

def aerie.shape.stairs.in(+aviary: Bool, +rookery: Bool, +room: J.Val) -> J.Val:
  match aviary:
    case True{}:
      aerie.fill(room, 19, 10, 2, 1, "stairs")
    case False{}:
      aerie.shape.stairs.rook(rookery, room)

def aerie.shape.stairs.rook(+rookery: Bool, +room: J.Val) -> J.Val:
  match rookery:
    case True{}:
      aerie.fill(room, 4, 10, 2, 1, "stairs")
    case False{}:
      room

# --- the Run -------------------------------------------------------------------------------

# Whether a living, awake knight is here and not in the privy.
def aerie.living(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> Bool:
  match hs:
    case Nil{}:
      False{}
    case Con{(+k, +h), +t}:
      Bool.or(Bool.and(U32.is_gt(ga.u(h, "hp"), 0), Bool.and(Bool.not(ga.b(h, "stone")), U32.is_zero(ga.u(h, "privy")))), aerie.living(t))

def aerie.Run.tick_aerie(+run: J.Val, +inputs: J.Val) -> J.Val:
  aerie.Run.tick_aerie.kind(ga.s(rules.Run.room(run), "kind"), run, inputs)

def aerie.Run.tick_aerie.kind(+kind: String, +run: J.Val, +inputs: J.Val) -> J.Val:
  aerie.Run.tick_aerie.place(aerie.doc_index(kind), kind, run, inputs)

def aerie.Run.tick_aerie.place(+i: U32, +kind: String, +run: J.Val, +inputs: J.Val) -> J.Val:
  aerie.Run.tick_aerie.place.m(U32.is_ge(i, 3), i, kind, run, inputs)

def aerie.Run.tick_aerie.place.m(+flag: Bool, +i: U32, +kind: String, +run: J.Val, +inputs: J.Val) -> J.Val:
  match flag:
    case True{}:
      run
    case False{}:
      aerie.Run.tick_aerie.awake(aerie.living(J.int_fields(J.get(run, "players"))), i, kind, run, inputs)

def aerie.Run.tick_aerie.awake(+awake: Bool, +i: U32, +kind: String, +run: J.Val, +inputs: J.Val) -> J.Val:
  match awake:
    case False{}:
      run
    case True{}:
      aerie.Run.tick_aerie.round(
        aerie.Run.tick_aerie.visit(U32.is_zero(U32.and(ga.u(J.get(J.get(run, "home"), "countryside"), "aerie_visited"), RS.u_shl(1, i))), i, run),
        i, kind, inputs)

# A first visit marks the place.
def aerie.Run.tick_aerie.visit(+first: Bool, +i: U32, +run: J.Val) -> J.Val:
  match first:
    case True{}:
      J.set(run, "marks", J.set(J.get(run, "marks"), aerie.visit_mark(i), J.JU{1}))
    case False{}:
      run

# No quest actions while an arcade round is playing.
def aerie.Run.tick_aerie.round(+run: J.Val, +i: U32, +kind: String, +inputs: J.Val) -> J.Val:
  aerie.Run.tick_aerie.round.m(J.is_null(J.get(J.get(run, "pastimes"), "round")), run, i, kind, inputs)

def aerie.Run.tick_aerie.round.m(+flag: Bool, +run: J.Val, +i: U32, +kind: String, +inputs: J.Val) -> J.Val:
  match flag:
    case False{}:
      run
    case True{}:
      aerie.Run.tick_aerie.stage(aerie.progress(J.get(J.get(run, "home"), "countryside"), J.get(run, "marks")), kind, run, inputs)

def aerie.Run.tick_aerie.stage(+sp: U32 & U32, +kind: String, +run: J.Val, +inputs: J.Val) -> J.Val:
  (stage, papers) = sp
  aerie.Run.tick_aerie.stage.in(U32.is_eq(stage, 2), stage, papers, kind, run, inputs)

def aerie.Run.tick_aerie.stage.in(+done: Bool, +stage: U32, +papers: U32, +kind: String, +run: J.Val, +inputs: J.Val) -> J.Val:
  match done:
    case True{}:
      J.set(run, "pigeons", J.JNull{})
    case False{}:
      aerie.Run.tick_aerie.act(stage, papers, kind, aerie.Run.tick_aerie.pigeon(U32.is_eq(stage, 1), kind, papers, run), inputs)

def aerie.Run.tick_aerie.pigeon(+on: Bool, +kind: String, +papers: U32, +run: J.Val) -> J.Val:
  match on:
    case True{}:
      aerie.Run.tick_pigeon(run, kind, papers)
    case False{}:
      run

# The first knight to press E at the clerk or on a dropped paper.
def aerie.Run.tick_aerie.act(+stage: U32, +papers: U32, +kind: String, +run: J.Val, +inputs: J.Val) -> J.Val:
  aerie.Run.tick_aerie.done(aerie.action(J.int_fields(J.get(run, "players")), stage, papers, kind, J.get(run, "pigeons"), inputs), run)

# (id, action) or null.
def aerie.action(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +stage: U32, +papers: U32, +kind: String, +pigeons: J.Val, +inputs: J.Val) -> J.Val:
  match hs:
    case Nil{}:
      J.JNull{}
    case Con{(+k, +h), +t}:
      aerie.action.knight(Maybe.default(&2, U32, U32.read(k), 0), h, t, stage, papers, kind, pigeons, inputs)

def aerie.action.knight(+id: U32, +h: J.Val, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +stage: U32, +papers: U32, +kind: String, +pigeons: J.Val, +inputs: J.Val) -> J.Val:
  aerie.action.press(
    Bool.or(Bool.or(U32.is_zero(ga.u(h, "hp")), ga.b(h, "stone")), Bool.or(U32.is_ne(ga.u(h, "privy"), 0),
      Bool.not(Bool.and(rules.Input.valid(J.get(inputs, U32.show(id))), ga.b(J.get(inputs, U32.show(id)), "interact"))))),
    id, h, t, stage, papers, kind, pigeons, inputs)

def aerie.action.press(+skip: Bool, +id: U32, +h: J.Val, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +stage: U32, +papers: U32, +kind: String, +pigeons: J.Val, +inputs: J.Val) -> J.Val:
  match skip:
    case True{}:
      aerie.action(t, stage, papers, kind, pigeons, inputs)
    case False{}:
      aerie.action.clerk(
        Bool.and(String.eq(kind, "aviary"), Bool.and(
          F32.is_lt(RS.f_hypot(F32.sub(ga.f(h, "x"), F32.mul(aerie.CLERK_X(), layout.TILE_UNITS())), F32.sub(ga.f(h, "y"), F32.mul(aerie.CLERK_Y(), layout.TILE_UNITS()))), 1.4),
          Bool.or(U32.is_zero(stage), U32.is_eq(papers, 7)))),
        id, h, t, stage, papers, kind, pigeons, inputs)

def aerie.action.clerk(+at_clerk: Bool, +id: U32, +h: J.Val, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +stage: U32, +papers: U32, +kind: String, +pigeons: J.Val, +inputs: J.Val) -> J.Val:
  match at_clerk:
    case True{}:
      ga.pair(J.JU{id}, J.JU{ga.pu(U32.is_zero(stage), 0, 4)})
    case False{}:
      aerie.action.paper(J.get(pigeons, "dropped"), id, h, t, stage, papers, kind, pigeons, inputs)

def aerie.action.paper(+dropped: J.Val, +id: U32, +h: J.Val, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +stage: U32, +papers: U32, +kind: String, +pigeons: J.Val, +inputs: J.Val) -> J.Val:
  aerie.action.paper.m(J.is_null(dropped), dropped, id, h, t, stage, papers, kind, pigeons, inputs)

def aerie.action.paper.m(+flag: Bool, +dropped: J.Val, +id: U32, +h: J.Val, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +stage: U32, +papers: U32, +kind: String, +pigeons: J.Val, +inputs: J.Val) -> J.Val:
  match flag:
    case True{}:
      aerie.action(t, stage, papers, kind, pigeons, inputs)
    case False{}:
      aerie.action.pick(
        Bool.and(String.eq(aerie.doc_room(ga.nu(dropped, 0)), kind),
          F32.is_lt(RS.f_hypot(F32.sub(ga.f(h, "x"), ga.nf(dropped, 1)), F32.sub(ga.f(h, "y"), ga.nf(dropped, 2))), F32.mul(1.4, layout.TILE_UNITS()))),
        ga.nu(dropped, 0), id, t, stage, papers, kind, pigeons, inputs)

def aerie.action.pick(+hit: Bool, +i: U32, +id: U32, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +stage: U32, +papers: U32, +kind: String, +pigeons: J.Val, +inputs: J.Val) -> J.Val:
  match hit:
    case True{}:
      ga.pair(J.JU{id}, J.JU{(i + 1 : U32)})
    case False{}:
      aerie.action(t, stage, papers, kind, pigeons, inputs)

def aerie.Run.tick_aerie.done(+act: J.Val, +run: J.Val) -> J.Val:
  aerie.Run.tick_aerie.done.m(J.is_null(act), act, run)

def aerie.Run.tick_aerie.done.m(+flag: Bool, +act: J.Val, +run: J.Val) -> J.Val:
  match flag:
    case True{}:
      run
    case False{}:
      aerie.Run.tick_aerie.marked(ga.nu(act, 0), ga.nu(act, 1), run)

def aerie.Run.tick_aerie.marked(+id: U32, +action: U32, +run: J.Val) -> J.Val:
  aerie.Run.tick_aerie.said(id, action,
    aerie.Run.tick_aerie.drop(Bool.and(U32.is_ge(action, 1), U32.is_lt(action, 4)),
      ga.push(J.set(run, "marks", J.set(J.get(run, "marks"), aerie.quest_mark(action), J.JU{1})), "sounds", J.JS{"spoil_pickup"})))

# A paper picked up: the pigeons' dropped paper is cleared.
def aerie.Run.tick_aerie.drop(+pick: Bool, +run: J.Val) -> J.Val:
  aerie.Run.tick_aerie.drop.m(Bool.and(pick, Bool.not(J.is_null(J.get(run, "pigeons")))), pick, run)

def aerie.Run.tick_aerie.drop.m(+flag: Bool, +pick: Bool, +run: J.Val) -> J.Val:
  match flag:
    case True{}:
      J.set(run, "pigeons", ga.sv(J.get(run, "pigeons"), "dropped", J.JNull{}))
    case False{}:
      run

def aerie.Run.tick_aerie.said(+id: U32, +action: U32, +run: J.Val) -> J.Val:
  aerie.Run.tick_aerie.said.m(U32.is_ge(action, 4), id, action, run)

def aerie.Run.tick_aerie.said.m(+flag: Bool, +id: U32, +action: U32, +run: J.Val) -> J.Val:
  match flag:
    case True{}:
      aerie.Run.tick_aerie.crown(aerie.Run.crown_the_bonker(run, id), id)
    case False{}:
      aerie.Run.tick_aerie.found(run, id, aerie.message(action))

def aerie.message(+action: U32) -> String:
  ga.ps(U32.is_eq(action, 0), "Lost minutes! Pigeons took them. Bonk them.",
    ga.ps(U32.is_eq(action, 1), "BELL TAX back. It taxes complaints too.",
      ga.ps(U32.is_eq(action, 2), "WATER TAX back. Damp paperwork.", "SEED TAX back. Slightly eaten.")))

def aerie.Run.tick_aerie.crown(+rm: J.Val & String, +id: U32) -> J.Val:
  (run, message) = rm
  aerie.Run.tick_aerie.found(run, id, message)

def aerie.Run.tick_aerie.found(+run: J.Val, +id: U32, +message: String) -> J.Val:
  J.set(run, "found", J.JA{[J.get(run, "tick"), J.JU{id}, J.JS{message}]})

# The minutes in: the best bonker is crowned and everyone is cleaned.
def aerie.Run.crown_the_bonker(+run: J.Val, +giver: U32) -> J.Val & String:
  aerie.Run.crown_the_bonker.best(aerie.best(J.int_fields(J.get(ga.pv(J.is_null(J.get(run, "pigeons")), aerie.Pigeons.default(), J.get(run, "pigeons")), "bonks")), 0, 0, False{}), J.set(run, "pigeons", J.JNull{}), giver)

# The id with the most bonks, the lowest id of equals: (found, id).
def aerie.best(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +best_id: U32, +best_n: U32, +any: Bool) -> U32 & Bool:
  match hs:
    case Nil{}:
      (best_id, any)
    case Con{(+k, +n), +t}:
      aerie.best.pick(Bool.or(Bool.not(any), U32.is_gt(J.u32(n), best_n)), Maybe.default(&2, U32, U32.read(k), 0), J.u32(n), t, best_id, best_n)

def aerie.best.pick(+better: Bool, +id: U32, +n: U32, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +best_id: U32, +best_n: U32) -> U32 & Bool:
  aerie.best(t, ga.pu(better, id, best_id), ga.pu(better, n, best_n), True{})

def aerie.Run.crown_the_bonker.best(+r: U32 & Bool, +run: J.Val, +giver: U32) -> J.Val & String:
  (id, any) = r
  aerie.Run.crown_the_bonker.name(ga.pu(any, id, giver), run)

def aerie.Run.crown_the_bonker.name(+best: U32, +run: J.Val) -> J.Val & String:
  aerie.Run.crown_the_bonker.laurel(U32.is_gt(rules.Run.party_len(run), 1), best, run)

def aerie.Run.crown_the_bonker.laurel(+crown: Bool, +best: U32, +run: J.Val) -> J.Val & String:
  match crown:
    case True{}:
      aerie.Run.crown_the_bonker.say(best, J.set(run, "players", aerie.laurels(J.int_fields(J.get(run, "players")), J.get(run, "players"), best)))
    case False{}:
      aerie.Run.crown_the_bonker.say(best, run)

def aerie.Run.crown_the_bonker.say(+best: U32, +run: J.Val) -> J.Val & String:
  (run, "MINUTES IN! " ++ String.to_upper(ga.s(ga.at(J.get(run, "players"), best), "name")) ++ " BONKED BEST.")

def aerie.laurels(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +players: J.Val, +best: U32) -> J.Val:
  match hs:
    case Nil{}:
      players
    case Con{(+k, +h), +t}:
      aerie.laurels(t, J.set(players, k, ga.sb(h, "laurel", U32.is_eq(Maybe.default(&2, U32, U32.read(k), 0), best))), best)

# --- this room's pigeon ---------------------------------------------------------------------

# The first hostile-free shot with life left that is on the pigeon: it is spent,
# and its owner answers.
def aerie.shoot(+ps: List<&2, J.Val>, +x: F32, +y: F32) -> List<&2, J.Val> & Maybe<&2, U32>:
  match ps:
    case Nil{}:
      (Nil{}, None{})
    case Con{+s, +t}:
      aerie.shoot.hit(
        Bool.and(Bool.and(Bool.not(ga.b(s, "hostile")), U32.is_gt(ga.u(s, "ttl"), 0)),
          F32.is_lt(RS.f_hypot(F32.sub(ga.f(s, "x"), x), F32.sub(ga.f(s, "y"), y)), aerie.BONK())),
        s, t, x, y)

def aerie.shoot.hit(+hit: Bool, +s: J.Val, +t: List<&2, J.Val>, +x: F32, +y: F32) -> List<&2, J.Val> & Maybe<&2, U32>:
  match hit:
    case True{}:
      (Con{ga.su(s, "ttl", 0), t}, Some{ga.u(J.get(s, "traits"), "owner")})
    case False{}:
      aerie.shoot.rest(s, aerie.shoot(t, x, y))

def aerie.shoot.rest(+s: J.Val, +r: List<&2, J.Val> & Maybe<&2, U32>) -> List<&2, J.Val> & Maybe<&2, U32>:
  (t, owner) = r
  (Con{s, t}, owner)

# The first living knight swinging within reach.
def aerie.swung(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +x: F32, +y: F32) -> Maybe<&2, U32>:
  match hs:
    case Nil{}:
      None{}
    case Con{(+k, +h), +t}:
      aerie.swung.hit(Bool.and(Bool.and(U32.is_gt(ga.u(h, "hp"), 0), U32.is_gt(ga.u(h, "swing"), 0)),
        F32.is_lt(RS.f_hypot(F32.sub(ga.f(h, "x"), x), F32.sub(ga.f(h, "y"), y)), aerie.SWING_REACH())), k, t, x, y)

def aerie.swung.hit(+hit: Bool, +k: String, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +x: F32, +y: F32) -> Maybe<&2, U32>:
  match hit:
    case True{}:
      Some{Maybe.default(&2, U32, U32.read(k), 0)}
    case False{}:
      aerie.swung(t, x, y)

# The nearest knight to (x, y), the first of equals; 0 for an empty table.
def aerie.nearest(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +x: F32, +y: F32, +best: U32, +bd: F32, +any: Bool) -> U32:
  match hs:
    case Nil{}:
      best
    case Con{(+k, +h), +t}:
      aerie.nearest.pick(Bool.or(Bool.not(any), F32.is_gt(bd, RS.f_hypot(F32.sub(ga.f(h, "x"), x), F32.sub(ga.f(h, "y"), y)))),
        Maybe.default(&2, U32, U32.read(k), 0), RS.f_hypot(F32.sub(ga.f(h, "x"), x), F32.sub(ga.f(h, "y"), y)), t, x, y, best, bd)

def aerie.nearest.pick(+better: Bool, +id: U32, +d: F32, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +x: F32, +y: F32, +best: U32, +bd: F32) -> U32:
  aerie.nearest(t, x, y, ga.pu(better, id, best), ga.pf(better, d, bd), True{})

# Knights under the pigeon (hp > 0, within POOP_REACH), in seat order.
def aerie.under(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +x: F32, +y: F32) -> List<&2, U32>:
  match hs:
    case Nil{}:
      Nil{}
    case Con{(+k, +h), +t}:
      aerie.under.keep(Bool.and(U32.is_gt(ga.u(h, "hp"), 0), F32.is_lt(RS.f_hypot(F32.sub(ga.f(h, "x"), x), F32.sub(ga.f(h, "y"), y)), aerie.POOP_REACH())),
        Maybe.default(&2, U32, U32.read(k), 0), aerie.under(t, x, y))

def aerie.under.keep(+hit: Bool, +id: U32, +rest: List<&2, U32>) -> List<&2, U32>:
  match hit:
    case True{}:
      Con{id, rest}
    case False{}:
      rest

# A poop on each knight underneath: their count (capped at 9) and the line.
def aerie.splat(+run: J.Val, +p: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      J.set(run, "pigeons", p)
    case Con{+id, +t}:
      aerie.splat(
        J.set(ga.push(run, "sounds", J.JS{"hit_magic"}), "found", J.JA{[J.get(run, "tick"), J.JU{id}, J.JS{"SPLAT! THE PIGEON VOTED."}]}),
        ga.sv(p, "poop", J.set(J.get(p, "poop"), U32.show(id), J.JU{U32.min(ga.sadd8(ga.u(J.get(p, "poop"), U32.show(id)), 1), 9)})),
        t)

def aerie.Run.tick_pigeon(+run: J.Val, +kind: String, +papers: U32) -> J.Val:
  aerie.Run.tick_pigeon.in(run, kind, papers, ga.pv(J.is_null(J.get(run, "pigeons")), aerie.Pigeons.default(), J.get(run, "pigeons")))

def aerie.Run.tick_pigeon.in(+run: J.Val, +kind: String, +papers: U32, +p: J.Val) -> J.Val:
  aerie.Run.tick_pigeon.up(aerie.flying(kind, 1, papers, p), run, p)

def aerie.Run.tick_pigeon.up(+i: U32, +run: J.Val, +p: J.Val) -> J.Val:
  aerie.Run.tick_pigeon.up.m(U32.is_ge(i, 3), i, run, p)

def aerie.Run.tick_pigeon.up.m(+flag: Bool, +i: U32, +run: J.Val, +p: J.Val) -> J.Val:
  match flag:
    case True{}:
      J.set(run, "pigeons", p)
    case False{}:
      aerie.Run.tick_pigeon.at(aerie.pigeon_at(rules.Run.room(run), i, W.of_j(J.get(run, "tick"))), i, run, p)

def aerie.Run.tick_pigeon.at(+xy: F32 & F32, +i: U32, +run: J.Val, +p: J.Val) -> J.Val:
  (+x, +y) = xy
  aerie.Run.tick_pigeon.shot(aerie.shoot(J.items(J.get(run, "projectiles")), x, y), x, y, i, run, p)

def aerie.Run.tick_pigeon.shot(+sh: List<&2, J.Val> & Maybe<&2, U32>, +x: F32, +y: F32, +i: U32, +run: J.Val, +p: J.Val) -> J.Val:
  (ps, shot) = sh
  aerie.Run.tick_pigeon.owner(aerie.first_some(shot, aerie.swung(J.int_fields(J.get(run, "players")), x, y)), x, y, i, J.set(run, "projectiles", J.JA{ps}), p)

def aerie.first_some(+a: Maybe<&2, U32>, +b: Maybe<&2, U32>) -> Maybe<&2, U32>:
  match a:
    case Some{+v}:
      Some{v}
    case None{}:
      b

def aerie.Run.tick_pigeon.owner(+o: Maybe<&2, U32>, +x: F32, +y: F32, +i: U32, +run: J.Val, +p: J.Val) -> J.Val:
  match o:
    case Some{+owner}:
      aerie.Run.tick_pigeon.bonk(ga.pu(ga.has_key(J.get(run, "players"), owner), owner, aerie.nearest(J.int_fields(J.get(run, "players")), x, y, 0, 0.0, False{})), x, y, i, run, p)
    case None{}:
      aerie.Run.tick_pigeon.poop(U32.is_zero(W.rem_u32(W.of_j(J.get(run, "tick")), aerie.POOP_EVERY())), x, y, run, p)

def aerie.Run.tick_pigeon.bonk(+by: U32, +x: F32, +y: F32, +i: U32, +run: J.Val, +p: J.Val) -> J.Val:
  aerie.Run.tick_pigeon.land(ga.is(layout.Room.tile(rules.Run.room(run), RS.f_as_i32(F32.div(x, layout.TILE_UNITS())), RS.f_as_i32(F32.div(y, layout.TILE_UNITS()))), "floor"), by, x, y, i, run,
    ga.sv(p, "bonks", J.set(J.get(p, "bonks"), U32.show(by), J.JU{RS.as_u8((ga.u(J.get(p, "bonks"), U32.show(by)) + 1 : U32))})))

def aerie.Run.tick_pigeon.land(+floor: Bool, +by: U32, +x: F32, +y: F32, +i: U32, +run: J.Val, +p: J.Val) -> J.Val:
  aerie.Run.tick_pigeon.dropped(by, i, run, p, ga.pf(floor, x, F32.mul(aerie.doc_x(i), layout.TILE_UNITS())), ga.pf(floor, y, F32.mul(aerie.doc_y(i), layout.TILE_UNITS())))

def aerie.Run.tick_pigeon.dropped(+by: U32, +i: U32, +run: J.Val, +p: J.Val, +ax: F32, +ay: F32) -> J.Val:
  J.set(J.set(ga.push(run, "sounds", J.JS{"hit_arrow"}), "found", J.JA{[J.get(run, "tick"), J.JU{by}, J.JS{"BONK! " ++ aerie.doc_name(i) ++ " DROPPED."}]}),
    "pigeons", ga.sv(p, "dropped", J.JA{[J.JU{i}, J.JF{ax}, J.JF{ay}]}))

def aerie.Run.tick_pigeon.poop(+poop: Bool, +x: F32, +y: F32, +run: J.Val, +p: J.Val) -> J.Val:
  match poop:
    case True{}:
      aerie.splat(run, p, aerie.under(J.int_fields(J.get(run, "players")), x, y))
    case False{}:
      J.set(run, "pigeons", p)
