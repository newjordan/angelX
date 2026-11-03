# crates/delve/src/rules/hippodrome.rs — the kingdom's chariot circuit.

def hippodrome.PLACES(+i: U32) -> String:
  gc.nth_s(["hippodrome", "wheelwright", "toll_bridge"], i)

def hippodrome.VISIT_MARKS(+i: U32) -> String:
  gc.nth_s(["country:race:0", "country:race:1", "country:race:2"], i)

def hippodrome.QUEST_MARKS(+i: U32) -> String:
  gc.nth_s(["country:axle:accept", "country:axle:0", "country:axle:1", "country:axle:2", "country:axle:deliver"], i)

def hippodrome.WAGER_MARKS(+i: U32) -> String:
  gc.nth_s(["country:wager:0", "country:wager:1", "country:wager:2", "country:wager:finish:0", "country:wager:finish:1", "country:wager:finish:2"], i)

# The shared booth and the mechanic stand (tile coordinates).
def hippodrome.MECHANIC() -> F32 & F32:
  (5.0, 9.5)

def hippodrome.BOOTH() -> F32 & F32:
  (17.0, 4.5)

# PARTS[i] = (kind, at, name).
def hippodrome.PARTS.kind(+i: U32) -> String:
  hippodrome.PLACES(i)

def hippodrome.PARTS.at(+i: U32) -> F32 & F32:
  (gc.nth_f([20.0, 19.0, 5.0], i), gc.nth_f([5.0, 5.0, 9.0], i))

def hippodrome.PARTS.name(+i: U32) -> String:
  gc.nth_s(["SQUARE WHEEL", "GOAT-PROOF AXLE", "TOLL RECEIPT"], i)

# The quest's stage and parts from the country's record and the marks.
def hippodrome.progress(+country: J.Val, +marks: J.Val) -> U32 & U32:
  hippodrome.progress.a(gc.u(country, "axle"), gc.u(country, "axle_parts"), marks)

def hippodrome.progress.a(+stage: U32, +parts: U32, +marks: J.Val) -> U32 & U32:
  hippodrome.progress.b(gc.pu(Bool.and(U32.is_zero(stage), J.has(marks, "country:axle:accept")), 1, stage), parts, marks)

def hippodrome.progress.b(+stage: U32, +parts: U32, +marks: J.Val) -> U32 & U32:
  hippodrome.progress.c(stage, gc.pu(U32.is_eq(stage, 1), U32.or(parts, hippodrome.parts_bits(marks)), parts), marks)

def hippodrome.parts_bits(+marks: J.Val) -> U32:
  U32.or(gc.pu(J.has(marks, "country:axle:0"), 1, 0), U32.or(gc.pu(J.has(marks, "country:axle:1"), 2, 0), gc.pu(J.has(marks, "country:axle:2"), 4, 0)))

def hippodrome.progress.c(+stage: U32, +parts: U32, +marks: J.Val) -> U32 & U32:
  (gc.pu(Bool.and(U32.is_eq(stage, 1), Bool.and(U32.is_eq(parts, 7), J.has(marks, "country:axle:deliver"))), 2, stage), parts)

# The kinds' blocked rectangles (col, row, w, h) and the stair patch.
def hippodrome.shape(+room: J.Val) -> J.Val:
  hippodrome.shape.stairs(hippodrome.shape.rects(room, J.str(J.get(room, "kind"))), J.str(J.get(room, "kind")))

def hippodrome.shape.rects(+room: J.Val, +kind: String) -> J.Val:
  gc.ifj(String.eq(kind, "hippodrome"),
    _ => hippodrome.rect(hippodrome.rect(hippodrome.rect(room, 3, 2, 6, 2), 15, 2, 6, 2), 8, 9, 2, 2),
    _ => gc.ifj(String.eq(kind, "wheelwright"),
      _ => hippodrome.rect(hippodrome.rect(hippodrome.rect(hippodrome.rect(room, 3, 2, 5, 3), 15, 2, 6, 2), 7, 10, 3, 2), 15, 11, 3, 1),
      _ => gc.ifj(String.eq(kind, "toll_bridge"),
        _ => hippodrome.rect(hippodrome.rect(hippodrome.rect(hippodrome.rect(hippodrome.rect(room, 3, 2, 6, 3), 15, 2, 6, 3), 8, 9, 2, 3), 14, 10, 2, 2), 3, 11, 4, 1),
        _ => room)))

# room.set(x, y, Tile::Block) over y in r..r+h, x in c..c+w.
def hippodrome.rect(+room: J.Val, +c: U32, +r: U32, +w: U32, +h: U32) -> J.Val:
  hippodrome.rect.rows(room, c, r, w, (r + h : U32), TileBlock{})

def hippodrome.rect.rows(+room: J.Val, +c: U32, +y: U32, +w: U32, +end: U32, +tile: LayTile) -> J.Val:
  gc.ifj(U32.is_lt(y, end),
    _ => hippodrome.rect.rows(hippodrome.rect.cols(room, c, y, w, tile), c, (y + 1 : U32), w, end, tile),
    _ => room)

def hippodrome.rect.cols(+room: J.Val, +c: U32, +y: U32, +w: U32, +tile: LayTile) -> J.Val:
  gc.ifj(U32.is_zero(w),
    _ => room,
    _ => hippodrome.rect.cols(layout.Room.set(room, c, y, tile), (c + 1 : U32), y, (w - 1 : U32), tile))

def hippodrome.shape.stairs(+room: J.Val, +kind: String) -> J.Val:
  gc.ifj(String.eq(kind, "hippodrome"),
    _ => hippodrome.rect.rows(room, 19, 12, 2, 13, TileStairs{}),
    _ => gc.ifj(String.eq(kind, "stables"),
      _ => hippodrome.rect.rows(room, 19, 10, 2, 11, TileStairs{}),
      _ => room))

# Is knight `h` within 1.4 of the tile spot `at`?
def hippodrome.near(+h: J.Val, +at: F32 & F32) -> Bool:
  hippodrome.near.go(gc.f(h, "x"), gc.f(h, "y"), at)

def hippodrome.near.go(+hx: F32, +hy: F32, at: F32 & F32) -> Bool:
  (ax, ay) = at
  F32.is_lt(RS.f_hypot((hx - ax * layout.TILE_UNITS() : F32), (hy - ay * layout.TILE_UNITS() : F32)), 1.4)

# Which place of PLACES this kind is, or 3.
def hippodrome.place(+kind: String) -> U32:
  gc.pu(String.eq(kind, "hippodrome"), 0, gc.pu(String.eq(kind, "wheelwright"), 1, gc.pu(String.eq(kind, "toll_bridge"), 2, 3)))

# fn tick_hippodrome(&mut self, inputs)
def hippodrome.Run.tick_hippodrome(+run: J.Val, +inputs: J.Val) -> J.Val:
  hippodrome.Run.tick_hippodrome.a(run, inputs, J.str(J.get(rules.Run.room(run), "kind")))

def hippodrome.Run.tick_hippodrome.a(+run: J.Val, +inputs: J.Val, +kind: String) -> J.Val:
  hippodrome.Run.tick_hippodrome.b(run, inputs, kind, hippodrome.place(kind))

def hippodrome.Run.tick_hippodrome.b(+run: J.Val, +inputs: J.Val, +kind: String, +i: U32) -> J.Val:
  gc.ifj(Bool.or(U32.is_ge(i, 3), Bool.not(hippodrome.anyone(gc.vals(J.get(run, "players"))))),
    _ => run,
    _ => hippodrome.Run.tick_hippodrome.c(hippodrome.visit(run, i), inputs, kind))

# players.values().any(|h| h.hp > 0 && !h.stone && h.privy == 0)
def hippodrome.anyone(+hs: List<&2, J.Val>) -> Bool:
  match hs:
    case Nil{}:
      False{}
    case Con{+h, +t}:
      Bool.or(Bool.and(U32.is_gt(gc.u(h, "hp"), 0), Bool.and(Bool.not(gc.b(h, "stone")), U32.is_zero(gc.u(h, "privy")))), hippodrome.anyone(t))

# if race_visited & (1<<i) == 0 { marks.insert(VISIT_MARKS[i], 1) }
def hippodrome.visit(+run: J.Val, +i: U32) -> J.Val:
  gc.ifj(Bool.not(gc.has_bit(gc.u(gc.hcountry(run), "race_visited"), i)),
    _ => gc.mark(run, hippodrome.VISIT_MARKS(i), 1),
    _ => run)

def gc.hcountry(+run: J.Val) -> J.Val:
  J.get(J.get(run, "home"), "countryside")

def hippodrome.Run.tick_hippodrome.c(+run: J.Val, +inputs: J.Val, +kind: String) -> J.Val:
  gc.ifj(Bool.not(J.is_null(J.get(J.get(run, "pastimes"), "round"))),
    _ => run,
    _ => hippodrome.Run.tick_hippodrome.e(run, inputs, kind, hippodrome.progress(gc.hcountry(run), J.get(run, "marks"))))

def hippodrome.Run.tick_hippodrome.e(+run: J.Val, +inputs: J.Val, +kind: String, sp: U32 & U32) -> J.Val:
  (+stage, +parts) = sp
  gc.ifj(U32.is_eq(stage, 2),
    _ => run,
    _ => hippodrome.Run.tick_hippodrome.go(run, inputs, kind, stage, parts, gc.ids(J.get(run, "players"))))

# The first knight with an action wins it; the loop breaks there.
def hippodrome.Run.tick_hippodrome.go(+run: J.Val, +inputs: J.Val, +kind: String, +stage: U32, +parts: U32, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      run
    case Con{+id, +t}:
      hippodrome.Run.tick_hippodrome.step(run, inputs, kind, stage, parts, id, t, gc.seat(J.get(run, "players"), id))

def hippodrome.Run.tick_hippodrome.step(+run: J.Val, +inputs: J.Val, +kind: String, +stage: U32, +parts: U32, +id: U32, +t: List<&2, U32>, +h: J.Val) -> J.Val:
  gc.ifj(Bool.not(hippodrome.pressing(h, gc.seat(inputs, id))),
    _ => hippodrome.Run.tick_hippodrome.go(run, inputs, kind, stage, parts, t),
    _ => hippodrome.Run.tick_hippodrome.act(run, inputs, kind, stage, parts, id, t, hippodrome.action(h, kind, stage, parts)))

# Alive (hp > 0, not stone, not in the privy) and a valid input pressing interact.
def hippodrome.pressing(+h: J.Val, +input: J.Val) -> Bool:
  Bool.and(U32.is_gt(gc.u(h, "hp"), 0), Bool.and(Bool.not(gc.b(h, "stone")), Bool.and(U32.is_zero(gc.u(h, "privy")), hippodrome.pressing.input(input))))

def hippodrome.pressing.input(+input: J.Val) -> Bool:
  gc.ifb(J.is_null(input), _ => False{}, _ => Bool.and(rules.Input.valid(input), gc.b(input, "interact")))

# The action number (0 accept, 1..3 parts, 4 deliver) or 99 for none.
def hippodrome.action(+h: J.Val, +kind: String, +stage: U32, +parts: U32) -> U32:
  gc.ifu(Bool.and(String.eq(kind, "wheelwright"), Bool.and(hippodrome.near(h, hippodrome.MECHANIC()), Bool.or(U32.is_zero(stage), U32.is_eq(parts, 7)))),
    _ => gc.pu(U32.is_zero(stage), 0, 4),
    _ => gc.ifu(U32.is_eq(stage, 1), _ => hippodrome.action.pos(h, kind, parts, 0), _ => 99))

# PARTS.iter().position(k == kind && near(at)).filter(parts & (1<<i) == 0).map(i+1)
def hippodrome.action.pos(+h: J.Val, +kind: String, +parts: U32, +i: U32) -> U32:
  gc.ifu(U32.is_ge(i, 3),
    _ => 99,
    _ => gc.ifu(Bool.and(String.eq(kind, hippodrome.PARTS.kind(i)), hippodrome.near(h, hippodrome.PARTS.at(i))),
      _ => gc.pu(gc.has_bit(parts, i), 99, (i + 1 : U32)),
      _ => hippodrome.action.pos(h, kind, parts, (i + 1 : U32))))

def hippodrome.Run.tick_hippodrome.act(+run: J.Val, +inputs: J.Val, +kind: String, +stage: U32, +parts: U32, +id: U32, +t: List<&2, U32>, +action: U32) -> J.Val:
  gc.ifj(U32.is_eq(action, 99),
    _ => hippodrome.Run.tick_hippodrome.go(run, inputs, kind, stage, parts, t),
    _ => gc.found(gc.sound(gc.mark(run, hippodrome.QUEST_MARKS(action), 1), "spoil_pickup"), id, hippodrome.said(action)))

def hippodrome.said(+action: U32) -> String:
  gc.ps(U32.is_eq(action, 0), "Wanted: SQUARE WHEEL, AXLE, RECEIPT.", gc.ps(U32.is_eq(action, 4), "Coach fixed! The goat signed it off.", "Recovered " ++ hippodrome.PARTS.name((action - 1 : U32)) ++ ". The wheelwright denies responsibility."))

# pub fn settle(country, treasury, marks, news) -> (country, treasury, news).
def hippodrome.settle(+country: J.Val, +treasury: J.Val, +marks: J.Val, +news: J.Val) -> J.Val & J.Val & J.Val:
  hippodrome.settle.visits(country, treasury, marks, news, 0)

def hippodrome.settle.visits(+country: J.Val, +treasury: J.Val, +marks: J.Val, +news: J.Val, +i: U32) -> J.Val & J.Val & J.Val:
  gc.if(J.Val & J.Val & J.Val, U32.is_ge(i, 3),
    _ => hippodrome.settle.axle(country, treasury, marks, news),
    _ => hippodrome.settle.visits(hippodrome.settle.visit(country, J.has(marks, hippodrome.VISIT_MARKS(i)), i), treasury, marks, news, (i + 1 : U32)))

def hippodrome.settle.visit(+country: J.Val, +seen: Bool, +i: U32) -> J.Val:
  gc.su(country, "race_visited", gc.pu(seen, U32.or(gc.u(country, "race_visited"), gc.bit(i)), gc.u(country, "race_visited")))

def hippodrome.settle.axle(+country: J.Val, +treasury: J.Val, +marks: J.Val, +news: J.Val) -> J.Val & J.Val & J.Val:
  hippodrome.settle.paid(hippodrome.settle.progress(country, marks), treasury, news)

def hippodrome.settle.progress.go(sp: U32 & U32, +country: J.Val) -> J.Val:
  (+stage, +parts) = sp
  gc.su(gc.su(country, "axle", stage), "axle_parts", parts)

def hippodrome.settle.progress(+country: J.Val, +marks: J.Val) -> J.Val:
  hippodrome.settle.progress.go(hippodrome.progress(country, marks), country)

def hippodrome.settle.paid(+country: J.Val, +treasury: J.Val, +news: J.Val) -> J.Val & J.Val & J.Val:
  gc.if(J.Val & J.Val & J.Val, Bool.and(U32.is_eq(gc.u(country, "axle"), 2), Bool.not(gc.b(country, "axle_paid"))),
    _ => (gc.sb(country, "axle_paid", True{}), realm.Spoils.add(realm.Spoils.add(treasury, "gold", 85), "ore", 2), J.push(news, J.JS{"Coach commission: 85 gold, 2 ore."})),
    _ => (country, treasury, news))
