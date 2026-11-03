# crates/delve/src/rules/forest.rs — the clearing's firefly hunt and rune song.

def forest.hunt_time() -> U32:
  900

def forest.song_len() -> U32:
  4

def forest.show_time() -> U32:
  150

# The pads in index order: lantern, memory stone, the three runes (tiles).
def forest.pad(+i: U32) -> F32 & F32:
  match i:
    case 0:
      (4.0, 10.5)
    case 1:
      (18.0, 10.5)
    case 2:
      (15.0, 5.5)
    case 3:
      (18.5, 3.5)
    case _:
      (21.0, 7.0)

# The fireflies' home points (tiles).
def forest.fly(+i: U32) -> F32 & F32:
  match i:
    case 0:
      (3.5, 4.0)
    case 1:
      (6.0, 3.5)
    case 2:
      (8.5, 4.5)
    case 3:
      (7.5, 6.5)
    case 4:
      (4.5, 6.0)
    case 5:
      (3.5, 8.5)
    case 6:
      (6.5, 9.0)
    case _:
      (9.0, 8.5)

# Forest::leave
def forest.Forest.leave(+f: J.Val) -> J.Val:
  gb.su(gb.su(gb.su(gb.sb(gb.su(gb.su(f, "hunt_left", 0), "caught", 0), "singing", False{}), "show_left", 0), "note", 0), "held", 0)

# Forest::melody: the rune a note of this song strikes.
def forest.Forest.melody(+f: J.Val, +note: U32) -> U32:
  U32.mod((forest.melody.base(U32.min(note, 3)) + U32.mod(gb.u(f, "songs"), 3) : U32), 3)

def forest.melody.base(+n: U32) -> U32:
  match n:
    case 0:
      0
    case 1:
      2
    case 2:
      1
    case _:
      2

# forest::firefly: a firefly's place at a tick, in arena units.
def forest.firefly(+index: U32, +tick: W.W) -> F32 & F32:
  forest.firefly.go(forest.fly(index), forest.firefly.t(index, tick))

def forest.firefly.t(+index: U32, +tick: W.W) -> F32:
  a = (U32.to_f32(W.rem_u32(tick, 360)) / 30.0 : F32)
  b = (a * 6.2831855 : F32)
  c = (b / 12.0 : F32)
  (c + U32.to_f32(index) : F32)

def forest.firefly.go(p: F32 & F32, +t: F32) -> F32 & F32:
  (x, y) = p
  ((x * 2.0 + F32.cos(t) * 0.6 : F32), (y * 2.0 + F32.sin(t) * 0.6 : F32))

# Whether a standing knight is within 1.25 of the point (arena units).
def forest.near(+players: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +x: F32, +y: F32) -> Bool:
  match players:
    case Nil{}:
      False{}
    case Con{(+id, +h), +t}:
      Bool.or(forest.near.one(h, x, y), forest.near(t, x, y))

def forest.near.one(+h: J.Val, +x: F32, +y: F32) -> Bool:
  Bool.and(Bool.and(U32.is_gt(gb.u(h, "hp"), 0), Bool.not(gb.b(h, "stone"))), Bool.and(U32.is_eq(gb.u(h, "privy"), 0), F32.is_lt(RS.f_hypot((gb.f(h, "x") - x : F32), (gb.f(h, "y") - y : F32)), 1.25)))

# The bits of the pads some knight stands on.
def forest.occupied(+players: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> U32:
  forest.occupied.go(players, 0, 0)

def forest.occupied.go(+players: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +i: U32, +acc: U32) -> U32:
  match i:
    case 5:
      acc
    case _:
      forest.occupied.go(players, (i + 1 : U32), forest.occupied.set(forest.occupied.at(players, forest.pad(i)), i, acc))

def forest.occupied.at(+players: List<&2, Sigma<&2, &2, String, _ => J.Val>>, p: F32 & F32) -> Bool:
  (x, y) = p
  forest.near(players, (x * 2.0 : F32), (y * 2.0 : F32))

def forest.occupied.set(+here: Bool, +i: U32, +acc: U32) -> U32:
  match here:
    case True{}:
      U32.or(acc, U32.shln(1, U32.to_nat(i)))
    case False{}:
      acc

# Run::tick_forest
def forest.Run.tick_forest(+run: J.Val) -> J.Val:
  forest.tick.gate(Bool.or(forest_road.Run.forest_road_showing(run), roundtable.Run.roundtable_showing(run)), run)

def forest.tick.gate(+skip: Bool, +run: J.Val) -> J.Val:
  match skip:
    case True{}:
      run
    case False{}:
      forest.tick.pads(run, forest.occupied(gb.hero_list(run)))

def forest.tick.pads(+run: J.Val, +occupied: U32) -> J.Val:
  forest.tick.go(forest.tick.held(run, occupied), U32.and(occupied, U32.not(gb.u(J.get(run, "forest"), "held"))))

def forest.tick.held(+run: J.Val, +occupied: U32) -> J.Val:
  J.set(run, "forest", gb.su(J.get(run, "forest"), "held", occupied))

def forest.tick.go(+run: J.Val, +pressed: U32) -> J.Val:
  forest.tick.song(forest.tick.hunt(forest.tick.song_start(forest.tick.hunt_start(run, pressed), pressed)), pressed)

def forest.f(+run: J.Val) -> J.Val:
  J.get(run, "forest")

def forest.with(+run: J.Val, f: J.Val) -> J.Val:
  J.set(run, "forest", f)

def forest.tick.hunt_start(+run: J.Val, +pressed: U32) -> J.Val:
  forest.tick.hunt_start.if(Bool.and(U32.is_ne(U32.and(pressed, 1), 0), U32.is_eq(gb.u(forest.f(run), "hunt_left"), 0)), run)

def forest.tick.hunt_start.if(go: Bool, +run: J.Val) -> J.Val:
  match go:
    case False{}:
      run
    case True{}:
      gb.sound(forest.with(run, gb.su(gb.su(forest.f(run), "hunt_left", 900), "caught", 0)), "play_card")

def forest.tick.song_start(+run: J.Val, +pressed: U32) -> J.Val:
  forest.tick.song_start.if(Bool.and(U32.is_ne(U32.and(pressed, 2), 0), Bool.not(gb.b(forest.f(run), "singing"))), run)

def forest.tick.song_start.if(go: Bool, +run: J.Val) -> J.Val:
  match go:
    case False{}:
      run
    case True{}:
      gb.sound(forest.with(run, gb.su(gb.su(gb.sb(forest.f(run), "singing", True{}), "show_left", 150), "note", 0)), "play_card")

# The hunt's tick: a step, the catches, the finish.
def forest.tick.hunt(+run: J.Val) -> J.Val:
  forest.tick.hunt.if(U32.is_gt(gb.u(forest.f(run), "hunt_left"), 0), run)

def forest.tick.hunt.if(go: Bool, +run: J.Val) -> J.Val:
  match go:
    case False{}:
      run
    case True{}:
      forest.hunt.finish(forest.hunt.catch(forest.with(run, gb.su(forest.f(run), "hunt_left", (gb.u(forest.f(run), "hunt_left") - 1 : U32))), 0))

def forest.hunt.catch(+run: J.Val, +i: U32) -> J.Val:
  match i:
    case 8:
      run
    case _:
      forest.hunt.catch(forest.hunt.catch.one(run, i, forest.firefly(i, gb.tick(run))), (i + 1 : U32))

def forest.hunt.catch.one(+run: J.Val, +i: U32, p: F32 & F32) -> J.Val:
  (x, y) = p
  forest.hunt.catch.if(Bool.and(Bool.not(gb.bit(gb.u(forest.f(run), "caught"), i)), forest.near(gb.hero_list(run), x, y)), run, i)

def forest.hunt.catch.if(go: Bool, +run: J.Val, +i: U32) -> J.Val:
  match go:
    case False{}:
      run
    case True{}:
      gb.sound(forest.with(run, gb.su(forest.f(run), "caught", U32.or(gb.u(forest.f(run), "caught"), U32.shln(1, U32.to_nat(i))))), "spoil_pickup")

def forest.hunt.finish(+run: J.Val) -> J.Val:
  forest.hunt.finish.go(run, gb.popcount(gb.u(forest.f(run), "caught")))

def forest.hunt.finish.go(+run: J.Val, +caught: U32) -> J.Val:
  forest.hunt.finish.all(forest.with(run, gb.su(forest.f(run), "best", U32.max(gb.u(forest.f(run), "best"), caught))), caught)

def forest.hunt.finish.all(+run: J.Val, +caught: U32) -> J.Val:
  forest.hunt.finish.pick(U32.is_eq(gb.u(forest.f(run), "caught"), 255), run, caught)

def forest.hunt.finish.pick(all: Bool, +run: J.Val, +caught: U32) -> J.Val:
  match all:
    case True{}:
      gb.sound(gb.found(J.put(J.set(forest.hunt.won(run), "score", J.JU{gb.sadd(gb.u(run, "score"), 100)}), ["marks", "country:fireflies"], J.JU{1}), 0, "All eight fireflies! +40 gold"), "play_card")
    case False{}:
      forest.hunt.finish.out(U32.is_eq(gb.u(forest.f(run), "hunt_left"), 0), run, caught)

def forest.hunt.won(+run: J.Val) -> J.Val:
  forest.with(run, gb.su(gb.su(forest.f(run), "hunt_left", 0), "hunts", gb.sadd(gb.u(forest.f(run), "hunts"), 1)))

def forest.hunt.finish.out(over: Bool, +run: J.Val, +caught: U32) -> J.Val:
  match over:
    case True{}:
      gb.found(run, 0, "Fireflies: " ++ U32.show(caught) ++ "/8. Step off to retry.")
    case False{}:
      run

# The song's tick: the show counts down, then a struck rune is judged.
def forest.tick.song(+run: J.Val, +pressed: U32) -> J.Val:
  forest.tick.song.if(gb.b(forest.f(run), "singing"), run, pressed)

def forest.tick.song.if(singing: Bool, +run: J.Val, +pressed: U32) -> J.Val:
  match singing:
    case False{}:
      run
    case True{}:
      forest.song.show(U32.is_gt(gb.u(forest.f(run), "show_left"), 0), run, U32.shrn(pressed, 2n))

def forest.song.show(showing: Bool, +run: J.Val, +notes: U32) -> J.Val:
  match showing:
    case True{}:
      forest.with(run, gb.su(forest.f(run), "show_left", (gb.u(forest.f(run), "show_left") - 1 : U32)))
    case False{}:
      forest.song.struck(U32.is_eq(notes, 0), run, notes)

def forest.song.struck(none: Bool, +run: J.Val, +notes: U32) -> J.Val:
  match none:
    case True{}:
      run
    case False{}:
      forest.song.single(U32.is_gt(gb.popcount(notes), 1), run, notes)

def forest.song.single(many: Bool, +run: J.Val, +notes: U32) -> J.Val:
  match many:
    case True{}:
      gb.found(run, 0, "One rune at a time! Step off and try that note again.")
    case False{}:
      forest.song.judge(U32.is_eq(notes, U32.shln(1, U32.to_nat(forest.Forest.melody(forest.f(run), gb.u(forest.f(run), "note"))))), run)

def forest.song.judge(right: Bool, +run: J.Val) -> J.Val:
  match right:
    case False{}:
      gb.found(forest.with(run, gb.sb(forest.f(run), "singing", False{})), 0, "The grove falls quiet. Return to the memory stone to retry.")
    case True{}:
      forest.song.next(gb.sound(forest.with(run, gb.su(forest.f(run), "note", (gb.u(forest.f(run), "note") + 1 : U32))), "spoil_pickup"))

def forest.song.next(+run: J.Val) -> J.Val:
  forest.song.done(U32.is_eq(gb.u(forest.f(run), "note"), 4), run)

def forest.song.done(done: Bool, +run: J.Val) -> J.Val:
  match done:
    case False{}:
      run
    case True{}:
      gb.sound(gb.found(J.put(J.set(forest.song.won(run), "score", J.JU{gb.sadd(gb.u(run, "score"), 100)}), ["marks", "country:song"], J.JU{1}), 0, "The woodland remembers your song! +40 gold"), "play_card")

def forest.song.won(+run: J.Val) -> J.Val:
  forest.with(run, gb.su(gb.sb(forest.f(run), "singing", False{}), "songs", gb.sadd(gb.u(forest.f(run), "songs"), 1)))

def case.forest.Run.tick_forest(+input: J.Val) -> J.Val:
  J.set(J.obj(), "run", forest.Run.tick_forest(J.get(input, "run")))
