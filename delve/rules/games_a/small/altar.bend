# crates/delve/src/rules/altar.rs — the wish altar and its lamp. Rooms, floors,
# lamps and the run are their serde J objects; `Option<(f32, f32)>` is null or
# a two-item array; the u64 tick is `W.W`.

def altar.REACH() -> F32:
  2.4

def altar.EDGE_SLIDE() -> F32:
  0.35

def altar.CORNER() -> F32:
  0.9

def altar.FLARE_TICKS() -> W.W:
  W.of_u32(60)

def altar.MOST_PENDING() -> U32:
  24

def altar.MOST_LANDED() -> U32:
  6

# --- the lamp ------------------------------------------------------------------

# Nothing has happened at the lamp.
def altar.Lamp.quiet(+lamp: J.Val) -> Bool:
  Bool.and(U32.is_zero(J.len(J.get(lamp, "rubbed"))), Bool.and(U32.is_zero(J.len(J.get(lamp, "wishing"))), U32.is_zero(J.len(J.get(lamp, "landed")))))

# Knight `id`'s wishes in flight, oldest first.
def altar.Lamp.pending(+lamp: J.Val, +id: U32) -> J.Val:
  J.JA{altar.Lamp.pending.go(J.items(J.get(lamp, "wishing")), id)}

def altar.Lamp.pending.go(+ps: List<&2, J.Val>, +id: U32) -> List<&2, J.Val>:
  match ps:
    case Nil{}:
      Nil{}
    case Con{+p, +t}:
      altar.Lamp.pending.keep(U32.is_eq(ga.u(p, "knight"), id), p, altar.Lamp.pending.go(t, id))

def altar.Lamp.pending.keep(+mine: Bool, +p: J.Val, +rest: List<&2, J.Val>) -> List<&2, J.Val>:
  match mine:
    case True{}:
      Con{p, rest}
    case False{}:
      rest

# The newest grant, while it still flares at `tick`.
def altar.Lamp.flaring(+lamp: J.Val, +tick: W.W) -> J.Val:
  altar.Lamp.flaring.last(List.reverse(&2, J.Val, J.items(J.get(lamp, "landed"))), tick)

def altar.Lamp.flaring.last(+rev: List<&2, J.Val>, +tick: W.W) -> J.Val:
  match rev:
    case Nil{}:
      J.JNull{}
    case Con{+l, +t}:
      altar.Lamp.flaring.pick(W.lt(W.sat_sub(tick, W.of_j(J.get(l, "tick"))), altar.FLARE_TICKS()), l)

def altar.Lamp.flaring.pick(+on: Bool, +l: J.Val) -> J.Val:
  ga.pv(on, l, J.JNull{})

# --- the stone's geometry --------------------------------------------------------

# Tiles a knight cannot cross once the altar stands at column `c`, row `r`.
def altar.stone_blocks(+c: U32, +r: U32, +tc: U32, +tr: U32) -> Bool:
  altar.stone_blocks.box(
    F32.mul(F32.add(RS.i_as_f32(c), 0.5), layout.TILE_UNITS()),
    F32.mul(F32.add(RS.i_as_f32(r), 1.0), layout.TILE_UNITS()),
    F32.mul(F32.add(RS.i_as_f32(tc), 0.5), layout.TILE_UNITS()),
    F32.mul(F32.add(RS.i_as_f32(tr), 0.5), layout.TILE_UNITS()))

def altar.stone_blocks.box(+ax: F32, +ay: F32, +cx: F32, +cy: F32) -> Bool:
  altar.stone_blocks.gap(
    F32.sub(ax, F32.mul(1.5, layout.TILE_UNITS())), F32.add(ax, F32.mul(1.5, layout.TILE_UNITS())),
    F32.sub(ay, F32.mul(1.4, layout.TILE_UNITS())), F32.add(ay, 0.6), cx, cy)

def altar.stone_blocks.gap(+left: F32, +right: F32, +top: F32, +foot: F32, +cx: F32, +cy: F32) -> Bool:
  altar.stone_blocks.pick(Bool.and(F32.is_gt(cx, left), F32.is_lt(cx, right)), Bool.and(F32.is_gt(cy, top), F32.is_lt(cy, foot)),
    ga.pf(F32.is_le(cy, top), F32.sub(top, cy), ga.pf(F32.is_ge(cy, foot), F32.sub(cy, foot), 0.0)),
    ga.pf(F32.is_le(cx, left), F32.sub(left, cx), ga.pf(F32.is_ge(cx, right), F32.sub(cx, right), 0.0)))

def altar.stone_blocks.pick(+xin: Bool, +yin: Bool, +gap_y: F32, +gap_x: F32) -> Bool:
  Bool.or(Bool.and(xin, yin),
    ga.pb(xin, F32.is_lt(gap_y, 0.45), ga.pb(yin, F32.is_lt(gap_x, 0.45), False{})))

# The four ways, in `layout::DIRS` order: north, east, south, west.
def altar.dir_dx(+i: U32) -> U32:
  ga.pu(U32.is_eq(i, 1), 1, ga.pu(U32.is_eq(i, 3), 4294967295, 0))

def altar.dir_dy(+i: U32) -> U32:
  ga.pu(U32.is_eq(i, 0), 4294967295, ga.pu(U32.is_eq(i, 2), 1, 0))

# Walkable ground: floor, door, stairs, bog.
def altar.ground(+t: J.Val) -> Bool:
  Bool.or(Bool.or(ga.is(t, "floor"), ga.is(t, "door")), Bool.or(ga.is(t, "stairs"), ga.is(t, "bog")))

# --- stone_cuts_a_passage ------------------------------------------------------------

# The window round the stone: the room, the stone, and the box (c0, r0)-(c1, r1).
type AltarWin is Data:
  AltarWin{room: J.Val, c: U32, r: U32, c0: U32, r0: U32, c1: U32, r1: U32}

def altar.win.walk(+win: AltarWin, +tc: U32, +tr: U32) -> Bool:
  match win:
    case AltarWin{+room, +c, +r, +c0, +r0, +c1, +r1}:
      altar.win.walk.in(
        Bool.and(Bool.and(RS.i_ge(tc, c0), RS.i_ge(tr, r0)), Bool.and(RS.i_le(tc, c1), RS.i_le(tr, r1))),
        Bool.and(Bool.and(RS.i_ge(tc, 0), RS.i_ge(tr, 0)), Bool.and(U32.is_lt(tc, ga.u(room, "cols")), U32.is_lt(tr, ga.u(room, "rows")))),
        room, tc, tr)

def altar.win.walk.in(+inside: Bool, +ok: Bool, +room: J.Val, +tc: U32, +tr: U32) -> Bool:
  altar.win.walk.in.m(Bool.and(inside, ok), inside, ok, room, tc, tr)

def altar.win.walk.in.m(+flag: Bool, +inside: Bool, +ok: Bool, +room: J.Val, +tc: U32, +tr: U32) -> Bool:
  match flag:
    case False{}:
      False{}
    case True{}:
      altar.ground(layout.Room.tile(room, tc, tr))

def altar.win.blocks(+win: AltarWin, +tc: U32, +tr: U32) -> Bool:
  match win:
    case AltarWin{+room, +c, +r, +c0, +r0, +c1, +r1}:
      altar.stone_blocks(c, r, tc, tr)

# A walkable neighbour that the stone blocks.
def altar.win.touches(+win: AltarWin, +tc: U32, +tr: U32, +i: U32) -> Bool:
  altar.win.touches.m(U32.is_eq(i, 4), win, tc, tr, i)

def altar.win.touches.m(+flag: Bool, +win: AltarWin, +tc: U32, +tr: U32, +i: U32) -> Bool:
  match flag:
    case True{}:
      False{}
    case False{}:
      altar.win.touches.at(win, tc, tr, i, RS.i_add(tc, altar.dir_dx(i)), RS.i_add(tr, altar.dir_dy(i)))

def altar.win.touches.at(+win: AltarWin, +tc: U32, +tr: U32, +i: U32, +nx: U32, +ny: U32) -> Bool:
  altar.win.touches.or(Bool.and(altar.win.walk(win, nx, ny), altar.win.blocks(win, nx, ny)), win, tc, tr, i)

def altar.win.touches.or(+hit: Bool, +win: AltarWin, +tc: U32, +tr: U32, +i: U32) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      altar.win.touches(win, tc, tr, (i + 1 : U32))

# The seed of the flood: the first walkable tile the stone touches, in the scan
# rows `r-2 ..= r+3`, columns `c-2 ..= c+2`; else the first walkable one.
def altar.win.seed(+win: AltarWin, +k: U32, +first: Maybe<&2, Sigma<&2, &2, U32, _ => U32>>) -> Maybe<&2, Sigma<&2, &2, U32, _ => U32>>:
  match win:
    case AltarWin{+room, +c, +r, +c0, +r0, +c1, +r1}:
      altar.win.seed.at(U32.is_ge(k, 30), win, k, RS.i_add(c, RS.i_sub(U32.mod(k, 5), 2)), RS.i_add(r, RS.i_sub(U32.div(k, 5), 2)), first)

def altar.win.seed.at(+done: Bool, +win: AltarWin, +k: U32, +tc: U32, +tr: U32, +first: Maybe<&2, Sigma<&2, &2, U32, _ => U32>>) -> Maybe<&2, Sigma<&2, &2, U32, _ => U32>>:
  match done:
    case True{}:
      first
    case False{}:
      altar.win.seed.cand(Bool.and(altar.win.walk(win, tc, tr), Bool.not(altar.win.blocks(win, tc, tr))), win, k, tc, tr, first)

def altar.win.seed.cand(+open: Bool, +win: AltarWin, +k: U32, +tc: U32, +tr: U32, +first: Maybe<&2, Sigma<&2, &2, U32, _ => U32>>) -> Maybe<&2, Sigma<&2, &2, U32, _ => U32>>:
  match open:
    case False{}:
      altar.win.seed(win, (k + 1 : U32), first)
    case True{}:
      altar.win.seed.touch(altar.win.touches(win, tc, tr, 0), win, k, tc, tr, first)

def altar.win.seed.touch(+touch: Bool, +win: AltarWin, +k: U32, +tc: U32, +tr: U32, +first: Maybe<&2, Sigma<&2, &2, U32, _ => U32>>) -> Maybe<&2, Sigma<&2, &2, U32, _ => U32>>:
  match touch:
    case True{}:
      Some{(tc, tr)}
    case False{}:
      altar.win.seed(win, (k + 1 : U32), altar.win.seed.keep(first, tc, tr))

def altar.win.seed.keep(+first: Maybe<&2, Sigma<&2, &2, U32, _ => U32>>, +tc: U32, +tr: U32) -> Maybe<&2, Sigma<&2, &2, U32, _ => U32>>:
  match first:
    case Some{+p}:
      Some{p}
    case None{}:
      Some{(tc, tr)}

# The cell index within the window: (tr - r0) * 25 + (tc - c0).
def altar.win.idx(+win: AltarWin, +tc: U32, +tr: U32) -> U32:
  match win:
    case AltarWin{+room, +c, +r, +c0, +r0, +c1, +r1}:
      (RS.i_sub(tr, r0) * 25 + RS.i_sub(tc, c0) : U32)

# The flood of walkable cells from the seed; `block` makes the stone a wall.
# The stack holds cell indices; `seen` is a set of index keys.
def altar.flood(+win: AltarWin, +block: Bool, +stack: List<&2, U32>, +seen: J.Val, +fuel: Nat) -> J.Val:
  match fuel:
    case 0n:
      seen
    case 1n+p:
      altar.flood.pop(stack, win, block, seen, p)

def altar.flood.pop(+stack: List<&2, U32>, +win: AltarWin, +block: Bool, +seen: J.Val, +p: Nat) -> J.Val:
  match stack:
    case Nil{}:
      seen
    case Con{+i, +t}:
      altar.flood.dirs(0, win, block, t, seen, p, RS.i_add(altar.win.c0(win), U32.mod(i, 25)), RS.i_add(altar.win.r0(win), U32.div(i, 25)))

def altar.win.c0(+win: AltarWin) -> U32:
  match win:
    case AltarWin{+room, +c, +r, +c0, +r0, +c1, +r1}:
      c0

def altar.win.r0(+win: AltarWin) -> U32:
  match win:
    case AltarWin{+room, +c, +r, +c0, +r0, +c1, +r1}:
      r0

def altar.flood.dirs(+d: U32, +win: AltarWin, +block: Bool, +stack: List<&2, U32>, +seen: J.Val, +p: Nat, +x: U32, +y: U32) -> J.Val:
  altar.flood.dirs.m(U32.is_eq(d, 4), d, win, block, stack, seen, p, x, y)

def altar.flood.dirs.m(+flag: Bool, +d: U32, +win: AltarWin, +block: Bool, +stack: List<&2, U32>, +seen: J.Val, +p: Nat, +x: U32, +y: U32) -> J.Val:
  match flag:
    case True{}:
      altar.flood(win, block, stack, seen, p)
    case False{}:
      altar.flood.step(d, win, block, stack, seen, p, x, y, RS.i_add(x, altar.dir_dx(d)), RS.i_add(y, altar.dir_dy(d)))

def altar.flood.step(+d: U32, +win: AltarWin, +block: Bool, +stack: List<&2, U32>, +seen: J.Val, +p: Nat, +x: U32, +y: U32, +nx: U32, +ny: U32) -> J.Val:
  altar.flood.visit(
    Bool.and(altar.win.walk(win, nx, ny), Bool.not(Bool.and(block, altar.win.blocks(win, nx, ny)))),
    d, win, block, stack, seen, p, x, y, nx, ny)

def altar.flood.visit(+ok: Bool, +d: U32, +win: AltarWin, +block: Bool, +stack: List<&2, U32>, +seen: J.Val, +p: Nat, +x: U32, +y: U32, +nx: U32, +ny: U32) -> J.Val:
  match ok:
    case False{}:
      altar.flood.dirs((d + 1 : U32), win, block, stack, seen, p, x, y)
    case True{}:
      altar.flood.mark(altar.win.idx(win, nx, ny), J.has(seen, U32.show(altar.win.idx(win, nx, ny))), d, win, block, stack, seen, p, x, y)

def altar.flood.mark(+j: U32, +was: Bool, +d: U32, +win: AltarWin, +block: Bool, +stack: List<&2, U32>, +seen: J.Val, +p: Nat, +x: U32, +y: U32) -> J.Val:
  match was:
    case True{}:
      altar.flood.dirs((d + 1 : U32), win, block, stack, seen, p, x, y)
    case False{}:
      altar.flood.dirs((d + 1 : U32), win, block, Con{j, stack}, J.set(seen, U32.show(j), J.JBool{True{}}), p, x, y)

# Some cell reachable without the stone that is no longer reachable around it.
def altar.cuts.any(+win: AltarWin, +open: J.Val, +around: J.Val, +i: U32) -> Bool:
  altar.cuts.any.m(U32.is_ge(i, 625), win, open, around, i)

def altar.cuts.any.m(+flag: Bool, +win: AltarWin, +open: J.Val, +around: J.Val, +i: U32) -> Bool:
  match flag:
    case True{}:
      False{}
    case False{}:
      altar.cuts.any.at(win, open, around, i, Bool.and(J.has(open, U32.show(i)), Bool.not(J.has(around, U32.show(i)))))

def altar.cuts.any.at(+win: AltarWin, +open: J.Val, +around: J.Val, +i: U32, +hit: Bool) -> Bool:
  match hit:
    case False{}:
      altar.cuts.any(win, open, around, (i + 1 : U32))
    case True{}:
      altar.cuts.any.free(Bool.not(altar.win.blocks(win, RS.i_add(altar.win.c0(win), U32.mod(i, 25)), RS.i_add(altar.win.r0(win), U32.div(i, 25)))), win, open, around, i)

def altar.cuts.any.free(+free: Bool, +win: AltarWin, +open: J.Val, +around: J.Val, +i: U32) -> Bool:
  match free:
    case True{}:
      True{}
    case False{}:
      altar.cuts.any(win, open, around, (i + 1 : U32))

# The stone at (`c`, `r`) would seal a walk the party could take.
def altar.stone_cuts_a_passage(+room: J.Val, +c: U32, +r: U32) -> Bool:
  altar.stone_cuts_a_passage.seed(AltarWin{room, c, r, RS.i_sub(c, 12), RS.i_sub(r, 12), RS.i_add(c, 12), RS.i_add(r, 12)})

def altar.stone_cuts_a_passage.seed(+win: AltarWin) -> Bool:
  altar.stone_cuts_a_passage.flood(altar.win.seed(win, 0, None{}), win)

def altar.stone_cuts_a_passage.flood(+seed: Maybe<&2, Sigma<&2, &2, U32, _ => U32>>, +win: AltarWin) -> Bool:
  match seed:
    case None{}:
      False{}
    case Some{(+tc, +tr)}:
      altar.stone_cuts_a_passage.sets(altar.win.idx(win, tc, tr), win)

def altar.stone_cuts_a_passage.sets(+s: U32, +win: AltarWin) -> Bool:
  altar.cuts.any(win,
    altar.flood(win, False{}, [s], J.set(J.obj(), U32.show(s), J.JBool{True{}}), 700n),
    altar.flood(win, True{}, [s], J.set(J.obj(), U32.show(s), J.JBool{True{}}), 700n), 0)

# --- route_tiles ---------------------------------------------------------------------------

# Whether tile index `k` (col = k % cols, row = k / cols) is walkable ground.
def altar.route.walk(+room: J.Val, +k: U32, +cols: U32) -> Bool:
  altar.ground(layout.Room.tile(room, U32.mod(k, cols), U32.div(k, cols)))

# The breadth-first search. `front`/`rear` are the queue (rear reversed); `back`
# maps a tile index to the one it was reached from; `goals` collects doors and
# stairs in the order met (reversed).
def altar.route.bfs(+room: J.Val, +cols: U32, +rows: U32, +start: U32, +front: List<&2, U32>, +rear: List<&2, U32>, +back: J.Val, +goals: List<&2, U32>, +fuel: Nat) -> J.Val & List<&2, U32>:
  match fuel:
    case 0n:
      (back, goals)
    case 1n+p:
      altar.route.bfs.pop(front, rear, room, cols, rows, start, back, goals, p)

def altar.route.bfs.pop(+front: List<&2, U32>, +rear: List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +back: J.Val, +goals: List<&2, U32>, +p: Nat) -> J.Val & List<&2, U32>:
  match front:
    case Con{+k, +t}:
      altar.route.bfs.visit(k, t, rear, room, cols, rows, start, back, goals, p)
    case Nil{}:
      altar.route.bfs.flip(List.reverse(&2, U32, rear), room, cols, rows, start, back, goals, p)

def altar.route.bfs.flip(+front: List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +back: J.Val, +goals: List<&2, U32>, +p: Nat) -> J.Val & List<&2, U32>:
  match front:
    case Nil{}:
      (back, goals)
    case Con{+k, +t}:
      altar.route.bfs.visit(k, t, Nil{}, room, cols, rows, start, back, goals, p)

def altar.route.bfs.visit(+k: U32, +front: List<&2, U32>, +rear: List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +back: J.Val, +goals: List<&2, U32>, +p: Nat) -> J.Val & List<&2, U32>:
  altar.route.bfs.goal(
    Bool.and(U32.is_ne(k, start), Bool.or(ga.is(layout.Room.tile(room, U32.mod(k, cols), U32.div(k, cols)), "door"), ga.is(layout.Room.tile(room, U32.mod(k, cols), U32.div(k, cols)), "stairs"))),
    k, front, rear, room, cols, rows, start, back, goals, p)

def altar.route.bfs.goal(+is_goal: Bool, +k: U32, +front: List<&2, U32>, +rear: List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +back: J.Val, +goals: List<&2, U32>, +p: Nat) -> J.Val & List<&2, U32>:
  match is_goal:
    case True{}:
      altar.route.bfs.dirs(0, k, front, rear, room, cols, rows, start, back, Con{k, goals}, p)
    case False{}:
      altar.route.bfs.dirs(0, k, front, rear, room, cols, rows, start, back, goals, p)

def altar.route.bfs.dirs(+d: U32, +k: U32, +front: List<&2, U32>, +rear: List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +back: J.Val, +goals: List<&2, U32>, +p: Nat) -> J.Val & List<&2, U32>:
  altar.route.bfs.dirs.m(U32.is_eq(d, 4), d, k, front, rear, room, cols, rows, start, back, goals, p)

def altar.route.bfs.dirs.m(+flag: Bool, +d: U32, +k: U32, +front: List<&2, U32>, +rear: List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +back: J.Val, +goals: List<&2, U32>, +p: Nat) -> J.Val & List<&2, U32>:
  match flag:
    case True{}:
      altar.route.bfs(room, cols, rows, start, front, rear, back, goals, p)
    case False{}:
      altar.route.bfs.next(RS.i_add(U32.mod(k, cols), altar.dir_dx(d)), RS.i_add(U32.div(k, cols), altar.dir_dy(d)), d, k, front, rear, room, cols, rows, start, back, goals, p)

def altar.route.bfs.next(+nx: U32, +ny: U32, +d: U32, +k: U32, +front: List<&2, U32>, +rear: List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +back: J.Val, +goals: List<&2, U32>, +p: Nat) -> J.Val & List<&2, U32>:
  altar.route.bfs.take(
    Bool.and(Bool.and(RS.i_ge(nx, 0), RS.i_lt(nx, cols)), Bool.and(RS.i_ge(ny, 0), RS.i_lt(ny, rows))),
    nx, ny, d, k, front, rear, room, cols, rows, start, back, goals, p)

def altar.route.bfs.take(+inb: Bool, +nx: U32, +ny: U32, +d: U32, +k: U32, +front: List<&2, U32>, +rear: List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +back: J.Val, +goals: List<&2, U32>, +p: Nat) -> J.Val & List<&2, U32>:
  match inb:
    case False{}:
      altar.route.bfs.dirs((d + 1 : U32), k, front, rear, room, cols, rows, start, back, goals, p)
    case True{}:
      altar.route.bfs.fresh(Bool.and(Bool.not(J.has(back, U32.show((ny * cols + nx : U32)))), altar.route.walk(room, (ny * cols + nx : U32), cols)),
        (ny * cols + nx : U32), d, k, front, rear, room, cols, rows, start, back, goals, p)

def altar.route.bfs.fresh(+new: Bool, +j: U32, +d: U32, +k: U32, +front: List<&2, U32>, +rear: List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +back: J.Val, +goals: List<&2, U32>, +p: Nat) -> J.Val & List<&2, U32>:
  match new:
    case False{}:
      altar.route.bfs.dirs((d + 1 : U32), k, front, rear, room, cols, rows, start, back, goals, p)
    case True{}:
      altar.route.bfs.dirs((d + 1 : U32), k, front, Con{j, rear}, room, cols, rows, start, J.set(back, U32.show(j), J.JU{k}), goals, p)

# Mark the walk from a goal back toward the start.
def altar.route.mark(+k: U32, +home: U32, +back: J.Val, +on: J.Val, +fuel: Nat) -> J.Val:
  match fuel:
    case 0n:
      on
    case 1n+p:
      altar.route.mark.at(U32.is_eq(k, home), k, home, back, on, p)

def altar.route.mark.at(+done: Bool, +k: U32, +home: U32, +back: J.Val, +on: J.Val, +p: Nat) -> J.Val:
  match done:
    case True{}:
      on
    case False{}:
      altar.route.mark.prev(J.has(back, U32.show(k)), k, home, back, J.set(on, U32.show(k), J.JBool{True{}}), p)

def altar.route.mark.prev(+has: Bool, +k: U32, +home: U32, +back: J.Val, +on: J.Val, +p: Nat) -> J.Val:
  match has:
    case False{}:
      on
    case True{}:
      altar.route.mark(J.u32(J.get(back, U32.show(k))), home, back, on, p)

def altar.route.goals(+gs: List<&2, U32>, +home: U32, +back: J.Val, +on: J.Val, +fuel: Nat) -> J.Val:
  match gs:
    case Nil{}:
      on
    case Con{+g, +t}:
      altar.route.goals(t, home, back, altar.route.mark(g, home, back, on, fuel), fuel)

# The extra goals (the open floor's chamber centres) that the search reached.
def altar.route.extra(+ex: List<&2, J.Val>, +room: J.Val, +cols: U32, +rows: U32, +back: J.Val, +goals: List<&2, U32>) -> List<&2, U32>:
  match ex:
    case Nil{}:
      goals
    case Con{+e, +t}:
      altar.route.extra(t, room, cols, rows, back, altar.route.extra.one(RS.f_as_i32(F32.div(J.f32(J.nth(e, 0)), layout.TILE_UNITS())), RS.f_as_i32(F32.div(J.f32(J.nth(e, 1)), layout.TILE_UNITS())), cols, rows, back, goals))

def altar.route.extra.one(+c: U32, +r: U32, +cols: U32, +rows: U32, +back: J.Val, +goals: List<&2, U32>) -> List<&2, U32>:
  altar.route.extra.in(Bool.and(Bool.and(RS.i_ge(c, 0), RS.i_lt(c, cols)), Bool.and(RS.i_ge(r, 0), RS.i_lt(r, rows))), c, r, cols, back, goals)

def altar.route.extra.in(+inb: Bool, +c: U32, +r: U32, +cols: U32, +back: J.Val, +goals: List<&2, U32>) -> List<&2, U32>:
  match inb:
    case False{}:
      goals
    case True{}:
      altar.route.extra.add(J.has(back, U32.show((r * cols + c : U32))), (r * cols + c : U32), goals)

def altar.route.extra.add(+reached: Bool, +j: U32, +goals: List<&2, U32>) -> List<&2, U32>:
  match reached:
    case True{}:
      Con{j, goals}
    case False{}:
      goals

# The route tiles as the set of tile indices on a shortest walk (before it is
# turned into the flag vector).
def altar.route.set(+room: J.Val, +from: J.Val, +extra: J.Val) -> J.Val:
  altar.route.set.start(room, ga.u(room, "cols"), ga.u(room, "rows"), RS.f_as_i32(F32.div(J.f32(J.nth(from, 0)), layout.TILE_UNITS())), RS.f_as_i32(F32.div(J.f32(J.nth(from, 1)), layout.TILE_UNITS())), extra)

def altar.route.set.start(+room: J.Val, +cols: U32, +rows: U32, +sx: U32, +sy: U32, +extra: J.Val) -> J.Val:
  altar.route.set.ok(Bool.and(Bool.and(RS.i_ge(sx, 0), RS.i_lt(sx, cols)), Bool.and(RS.i_ge(sy, 0), RS.i_lt(sy, rows))), room, cols, rows, sx, sy, extra)

def altar.route.set.ok(+inb: Bool, +room: J.Val, +cols: U32, +rows: U32, +sx: U32, +sy: U32, +extra: J.Val) -> J.Val:
  match inb:
    case False{}:
      J.obj()
    case True{}:
      altar.route.set.walk(altar.route.walk(room, (sy * cols + sx : U32), cols), room, cols, rows, (sy * cols + sx : U32), extra)

def altar.route.set.walk(+ok: Bool, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +extra: J.Val) -> J.Val:
  match ok:
    case False{}:
      J.obj()
    case True{}:
      altar.route.set.done(altar.route.bfs(room, cols, rows, start, [start], Nil{}, J.set(J.obj(), U32.show(start), J.JU{start}), Nil{}, U32.to_nat((cols * rows + 1 : U32))), room, cols, rows, start, extra)

def altar.route.set.done(+r: J.Val & List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +extra: J.Val) -> J.Val:
  (back, goals) = r
  altar.route.set.mark(List.reverse(&2, U32, goals), room, cols, rows, start, back, J.items(extra))

def altar.route.set.mark(+goals: List<&2, U32>, +room: J.Val, +cols: U32, +rows: U32, +start: U32, +back: J.Val, +ex: List<&2, J.Val>) -> J.Val:
  altar.route.goals(altar.route.extra(ex, room, cols, rows, back, goals) , start, back, J.obj(), U32.to_nat((cols * rows + 1 : U32)))

def altar.route.flags(+on: J.Val, +i: U32, +n: U32) -> List<&2, J.Val>:
  altar.route.flags.m(U32.is_ge(i, n), on, i, n)

def altar.route.flags.m(+flag: Bool, +on: J.Val, +i: U32, +n: U32) -> List<&2, J.Val>:
  match flag:
    case True{}:
      Nil{}
    case False{}:
      Con{J.JBool{J.has(on, U32.show(i))}, altar.route.flags(on, (i + 1 : U32), n)}

# Tiles that lie on a shortest walk from `from` to a door, the stairs, or one
# of `extra`: the flag vector.
def altar.route_tiles(+room: J.Val, +from: J.Val, +extra: J.Val) -> J.Val:
  J.JA{altar.route.flags(altar.route.set(room, from, extra), 0, (ga.u(room, "cols") * ga.u(room, "rows") : U32))}

def altar.stone_stands_on_a_route(+on: J.Val, +cols: U32, +c: U32, +r: U32) -> Bool:
  altar.route.stands(on, cols, c, r, 0)

def altar.route.stands(+on: J.Val, +cols: U32, +c: U32, +r: U32, +k: U32) -> Bool:
  altar.route.stands.m(U32.is_ge(k, 9), on, cols, c, r, k)

def altar.route.stands.m(+flag: Bool, +on: J.Val, +cols: U32, +c: U32, +r: U32, +k: U32) -> Bool:
  match flag:
    case True{}:
      False{}
    case False{}:
      altar.route.stands.at(on, cols, c, r, k, RS.i_add(c, RS.i_sub(U32.mod(k, 3), 1)), RS.i_add(r, RS.i_sub(U32.div(k, 3), 1)))

def altar.route.stands.at(+on: J.Val, +cols: U32, +c: U32, +r: U32, +k: U32, +tc: U32, +tr: U32) -> Bool:
  altar.route.stands.hit(
    Bool.and(Bool.and(altar.stone_blocks(c, r, tc, tr), Bool.not(RS.i_lt(tc, 0))), Bool.not(RS.i_lt(tr, 0))),
    on, cols, c, r, k, (tr * cols + tc : U32))

def altar.route.stands.hit(+live: Bool, +on: J.Val, +cols: U32, +c: U32, +r: U32, +k: U32, +i: U32) -> Bool:
  match live:
    case False{}:
      altar.route.stands(on, cols, c, r, (k + 1 : U32))
    case True{}:
      altar.route.stands.on(Bool.and(U32.is_lt(i, J.len(on)), J.bool(J.nth(on, i))), on, cols, c, r, k)

def altar.route.stands.on(+on_route: Bool, +on: J.Val, +cols: U32, +c: U32, +r: U32, +k: U32) -> Bool:
  match on_route:
    case True{}:
      True{}
    case False{}:
      altar.route.stands(on, cols, c, r, (k + 1 : U32))

# --- where the altar stands -------------------------------------------------------

# A tile as a number of tiles: `(u / TILE_UNITS).floor() as i32`.
def altar.tile(+u: F32) -> U32:
  RS.f_as_i32(F32.floor(F32.div(u, layout.TILE_UNITS())))

# The anchor of the search: (col, row), or null for a room that keeps none.
def altar.place.anchor(+room: J.Val, +arrival: J.Val) -> J.Val:
  altar.place.anchor.in(J.is_null(arrival), ga.s(room, "kind"), RS.i_div(ga.u(room, "cols"), 2), RS.i_div(ga.u(room, "rows"), 2), arrival)

def altar.place.anchor.in(+open: Bool, +kind: String, +hc: U32, +hr: U32, +arrival: J.Val) -> J.Val:
  altar.place.anchor.pick(open, String.eq(kind, "start"), String.eq(kind, "sanctuary"), hc, hr, arrival)

def altar.place.anchor.pick(+no_arrival: Bool, +start: Bool, +sanct: Bool, +hc: U32, +hr: U32, +arrival: J.Val) -> J.Val:
  match no_arrival:
    case False{}:
      ga.pair_i(RS.i_sub(altar.tile(J.f32(J.nth(arrival, 0))), 6), RS.i_sub(altar.tile(J.f32(J.nth(arrival, 1))), 2))
    case True{}:
      altar.place.anchor.kind(start, sanct, hc, hr)

def altar.place.anchor.kind(+start: Bool, +sanct: Bool, +hc: U32, +hr: U32) -> J.Val:
  match start:
    case True{}:
      ga.pair_i(RS.i_sub(hc, 7), RS.i_sub(hr, 3))
    case False{}:
      altar.place.anchor.sanct(sanct, hc, hr)

def altar.place.anchor.sanct(+sanct: Bool, +hc: U32, +hr: U32) -> J.Val:
  match sanct:
    case True{}:
      ga.pair_i(RS.i_sub(hc, 8), hr)
    case False{}:
      J.JNull{}

# The 3x2 patch (c-1..=c+1, r..=r+1) is all floor.
def altar.place.floor3(+room: J.Val, +c: U32, +r: U32, +k: U32) -> Bool:
  altar.place.floor3.m(U32.is_ge(k, 6), room, c, r, k)

def altar.place.floor3.m(+flag: Bool, +room: J.Val, +c: U32, +r: U32, +k: U32) -> Bool:
  match flag:
    case True{}:
      True{}
    case False{}:
      altar.place.floor3.at(ga.is(layout.Room.tile(room, RS.i_add(c, RS.i_sub(U32.mod(k, 3), 1)), RS.i_add(r, U32.div(k, 3))), "floor"), room, c, r, k)

def altar.place.floor3.at(+f: Bool, +room: J.Val, +c: U32, +r: U32, +k: U32) -> Bool:
  match f:
    case True{}:
      altar.place.floor3(room, c, r, (k + 1 : U32))
    case False{}:
      False{}

# No door in c-3..=c+3 x r-2..=r+3.
def altar.place.nodoor(+room: J.Val, +c: U32, +r: U32, +k: U32) -> Bool:
  altar.place.nodoor.m(U32.is_ge(k, 42), room, c, r, k)

def altar.place.nodoor.m(+flag: Bool, +room: J.Val, +c: U32, +r: U32, +k: U32) -> Bool:
  match flag:
    case True{}:
      True{}
    case False{}:
      altar.place.nodoor.at(ga.is(layout.Room.tile(room, RS.i_add(c, RS.i_sub(U32.mod(k, 7), 3)), RS.i_add(r, RS.i_sub(U32.div(k, 7), 2))), "door"), room, c, r, k)

def altar.place.nodoor.at(+d: Bool, +room: J.Val, +c: U32, +r: U32, +k: U32) -> Bool:
  match d:
    case True{}:
      False{}
    case False{}:
      altar.place.nodoor(room, c, r, (k + 1 : U32))

# A spot the stone may stand: clear floor, off the doors, sealing no walk.
def altar.place.clear(+room: J.Val, +c: U32, +r: U32) -> Bool:
  altar.place.clear.a(altar.place.floor3(room, c, r, 0), room, c, r)

def altar.place.clear.a(+f: Bool, +room: J.Val, +c: U32, +r: U32) -> Bool:
  match f:
    case False{}:
      False{}
    case True{}:
      altar.place.clear.b(altar.place.nodoor(room, c, r, 0), room, c, r)

def altar.place.clear.b(+f: Bool, +room: J.Val, +c: U32, +r: U32) -> Bool:
  match f:
    case False{}:
      False{}
    case True{}:
      Bool.not(altar.stone_cuts_a_passage(room, c, r))

# The spiral: rings 0..12, rows then columns within a ring. `aside` is the first
# spot that sat on a route (null when none). Answers the place, or null.
def altar.place.scan(+room: J.Val, +routes: J.Val, +cols: U32, +rows: U32, +ac: U32, +ar: U32, +ring: U32, +dr: U32, +dc: U32, +aside: J.Val) -> J.Val:
  altar.place.scan.m(U32.is_ge(ring, 12), room, routes, cols, rows, ac, ar, ring, dr, dc, aside)

def altar.place.scan.m(+flag: Bool, +room: J.Val, +routes: J.Val, +cols: U32, +rows: U32, +ac: U32, +ar: U32, +ring: U32, +dr: U32, +dc: U32, +aside: J.Val) -> J.Val:
  match flag:
    case True{}:
      altar.place.at(aside)
    case False{}:
      altar.place.scan.row(RS.i_gt(dr, ring), room, routes, cols, rows, ac, ar, ring, dr, dc, aside)

def altar.place.scan.row(+past: Bool, +room: J.Val, +routes: J.Val, +cols: U32, +rows: U32, +ac: U32, +ar: U32, +ring: U32, +dr: U32, +dc: U32, +aside: J.Val) -> J.Val:
  match past:
    case True{}:
      altar.place.scan(room, routes, cols, rows, ac, ar, (ring + 1 : U32), RS.i_neg((ring + 1 : U32)), RS.i_neg((ring + 1 : U32)), aside)
    case False{}:
      altar.place.scan.col(RS.i_gt(dc, ring), room, routes, cols, rows, ac, ar, ring, dr, dc, aside)

def altar.place.scan.col(+past: Bool, +room: J.Val, +routes: J.Val, +cols: U32, +rows: U32, +ac: U32, +ar: U32, +ring: U32, +dr: U32, +dc: U32, +aside: J.Val) -> J.Val:
  match past:
    case True{}:
      altar.place.scan(room, routes, cols, rows, ac, ar, ring, RS.i_add(dr, 1), RS.i_neg(ring), aside)
    case False{}:
      altar.place.scan.edge(U32.is_eq(RS.i_max(RS.i_abs(dr), RS.i_abs(dc)), ring), room, routes, cols, rows, ac, ar, ring, dr, dc, aside)

def altar.place.scan.edge(+on_ring: Bool, +room: J.Val, +routes: J.Val, +cols: U32, +rows: U32, +ac: U32, +ar: U32, +ring: U32, +dr: U32, +dc: U32, +aside: J.Val) -> J.Val:
  match on_ring:
    case False{}:
      altar.place.scan(room, routes, cols, rows, ac, ar, ring, dr, RS.i_add(dc, 1), aside)
    case True{}:
      altar.place.scan.cand(RS.i_add(ac, dc), RS.i_add(ar, dr), room, routes, cols, rows, ac, ar, ring, dr, dc, aside)

def altar.place.scan.cand(+c: U32, +r: U32, +room: J.Val, +routes: J.Val, +cols: U32, +rows: U32, +ac: U32, +ar: U32, +ring: U32, +dr: U32, +dc: U32, +aside: J.Val) -> J.Val:
  altar.place.scan.ok(
    Bool.or(Bool.or(RS.i_lt(c, 2), RS.i_lt(r, 2)), Bool.or(RS.i_gt(c, RS.i_sub(cols, 3)), RS.i_gt(r, RS.i_sub(rows, 3)))),
    c, r, room, routes, cols, rows, ac, ar, ring, dr, dc, aside)

def altar.place.scan.ok(+skip: Bool, +c: U32, +r: U32, +room: J.Val, +routes: J.Val, +cols: U32, +rows: U32, +ac: U32, +ar: U32, +ring: U32, +dr: U32, +dc: U32, +aside: J.Val) -> J.Val:
  match skip:
    case True{}:
      altar.place.scan(room, routes, cols, rows, ac, ar, ring, dr, RS.i_add(dc, 1), aside)
    case False{}:
      altar.place.scan.clear(altar.place.clear(room, c, r), c, r, room, routes, cols, rows, ac, ar, ring, dr, dc, aside)

def altar.place.scan.clear(+clear: Bool, +c: U32, +r: U32, +room: J.Val, +routes: J.Val, +cols: U32, +rows: U32, +ac: U32, +ar: U32, +ring: U32, +dr: U32, +dc: U32, +aside: J.Val) -> J.Val:
  match clear:
    case False{}:
      altar.place.scan(room, routes, cols, rows, ac, ar, ring, dr, RS.i_add(dc, 1), aside)
    case True{}:
      altar.place.scan.route(Bool.and(Bool.not(J.is_null(routes)), altar.stone_stands_on_a_route(routes, cols, c, r)), c, r, room, routes, cols, rows, ac, ar, ring, dr, dc, aside)

def altar.place.scan.route(+on_route: Bool, +c: U32, +r: U32, +room: J.Val, +routes: J.Val, +cols: U32, +rows: U32, +ac: U32, +ar: U32, +ring: U32, +dr: U32, +dc: U32, +aside: J.Val) -> J.Val:
  match on_route:
    case False{}:
      altar.place.at(ga.pair_i(c, r))
    case True{}:
      altar.place.scan(room, routes, cols, rows, ac, ar, ring, dr, RS.i_add(dc, 1), altar.place.first(aside, c, r))

def altar.place.first(+aside: J.Val, +c: U32, +r: U32) -> J.Val:
  ga.pv(J.is_null(aside), ga.pair_i(c, r), aside)

# The stone's spot for tile (c, r): (c + 0.5, r + 1.0) tiles in units.
def altar.place.at(+cr: J.Val) -> J.Val:
  altar.place.at.in(J.is_null(cr), cr)

def altar.place.at.in(+none: Bool, +cr: J.Val) -> J.Val:
  match none:
    case True{}:
      J.JNull{}
    case False{}:
      ga.pair_f(F32.mul(F32.add(RS.i_as_f32(J.i32(J.nth(cr, 0))), 0.5), layout.TILE_UNITS()), F32.mul(F32.add(RS.i_as_f32(J.i32(J.nth(cr, 1))), 1.0), layout.TILE_UNITS()))

# Where the altar stands in `room`, if it has one: a clear stretch of floor
# three tiles wide and two deep, found by a spiral out from the spot that suits
# the room. `arrival` is null or [x, y]; `extra` an array of [x, y].
def altar.place(+room: J.Val, +arrival: J.Val, +extra: J.Val) -> J.Val:
  altar.place.go(altar.place.anchor(room, arrival), room, arrival, extra)

def altar.place.go(+anchor: J.Val, +room: J.Val, +arrival: J.Val, +extra: J.Val) -> J.Val:
  altar.place.go.in(J.is_null(anchor), anchor, room, arrival, extra)

def altar.place.go.in(+none: Bool, +anchor: J.Val, +room: J.Val, +arrival: J.Val, +extra: J.Val) -> J.Val:
  match none:
    case True{}:
      J.JNull{}
    case False{}:
      altar.place.go.routes(ga.pv(J.is_null(arrival), J.JNull{}, altar.route_tiles(room, arrival, extra)), anchor, room)

def altar.place.go.routes(+routes: J.Val, +anchor: J.Val, +room: J.Val) -> J.Val:
  altar.place.scan(room, routes, ga.u(room, "cols"), ga.u(room, "rows"), J.i32(J.nth(anchor, 0)), J.i32(J.nth(anchor, 1)), 0, 0, 0, J.JNull{})

def altar.spot(+room: J.Val, +arrival: J.Val) -> J.Val:
  altar.place(room, arrival, J.JA{[]})

# A room that keeps a wish altar: the open floor's map, a start room, or a
# Sanctuary. The identity start and the home floor do not.
def altar.keeps_an_altar(+floor: J.Val, +index: U32, +identity: Bool) -> Bool:
  altar.keeps_an_altar.in(Bool.or(Bool.or(identity, U32.is_zero(ga.u(floor, "depth"))), U32.is_ge(index, J.len(J.get(floor, "rooms")))), floor, index)

def altar.keeps_an_altar.in(+no: Bool, +floor: J.Val, +index: U32) -> Bool:
  match no:
    case True{}:
      False{}
    case False{}:
      Bool.or(
        Bool.and(Bool.not(J.is_null(J.get(floor, "open"))), U32.is_eq(ga.u(J.get(floor, "open"), "map"), index)),
        Bool.or(ga.is(J.get(J.nth(J.get(floor, "rooms"), index), "kind"), "start"), ga.is(J.get(J.nth(J.get(floor, "rooms"), index), "kind"), "sanctuary")))

# The middle of an open map's chamber (rect = (col, row, cols, rows), u16).
def altar.chamber.centre(+ch: J.Val) -> J.Val:
  ga.pair_f(
    F32.div(F32.add(F32.mul(RS.u_as_f32(ga.nu(J.get(ch, "rect"), 0)), layout.TILE_UNITS()), F32.mul(RS.u_as_f32(RS.as_u16((ga.nu(J.get(ch, "rect"), 0) + ga.nu(J.get(ch, "rect"), 2) : U32))), layout.TILE_UNITS())), 2.0),
    F32.div(F32.add(F32.mul(RS.u_as_f32(ga.nu(J.get(ch, "rect"), 1)), layout.TILE_UNITS()), F32.mul(RS.u_as_f32(RS.as_u16((ga.nu(J.get(ch, "rect"), 1) + ga.nu(J.get(ch, "rect"), 3) : U32))), layout.TILE_UNITS())), 2.0))

def altar.centres(+chs: List<&2, J.Val>) -> List<&2, J.Val>:
  match chs:
    case Nil{}:
      Nil{}
    case Con{+c, +t}:
      Con{altar.chamber.centre(c), altar.centres(t)}

# Choose the wish altar's spot for every room on `floor` that keeps one and
# does not yet have it. A room already kept is left where it was stood.
def altar.place_on(+floor: J.Val, +identity: Bool) -> J.Val:
  altar.place_on.go(floor, identity, 0, J.len(J.get(floor, "rooms")))

def altar.place_on.go(+floor: J.Val, +identity: Bool, +i: U32, +n: U32) -> J.Val:
  altar.place_on.go.m(U32.is_ge(i, n), floor, identity, i, n)

def altar.place_on.go.m(+flag: Bool, +floor: J.Val, +identity: Bool, +i: U32, +n: U32) -> J.Val:
  match flag:
    case True{}:
      floor
    case False{}:
      altar.place_on.room(Bool.or(J.has(J.get(floor, "altar_at"), U32.show(i)), Bool.not(altar.keeps_an_altar(floor, i, identity))), floor, identity, i, n)

def altar.place_on.room(+skip: Bool, +floor: J.Val, +identity: Bool, +i: U32, +n: U32) -> J.Val:
  match skip:
    case True{}:
      altar.place_on.go(floor, identity, (i + 1 : U32), n)
    case False{}:
      altar.place_on.put(altar.place_on.args(floor, i), floor, identity, i, n)

# (arrival, extra) for room `i`: the open map's arrival and chamber centres.
def altar.place_on.args(+floor: J.Val, +i: U32) -> J.Val:
  altar.place_on.args.in(Bool.and(Bool.not(J.is_null(J.get(floor, "open"))), U32.is_eq(ga.u(J.get(floor, "open"), "map"), i)), floor)

def altar.place_on.args.in(+open: Bool, +floor: J.Val) -> J.Val:
  match open:
    case True{}:
      ga.pair(J.get(J.get(floor, "open"), "arrival"), J.JA{altar.centres(J.items(J.get(J.get(floor, "open"), "chambers")))})
    case False{}:
      ga.pair(J.JNull{}, J.JA{[]})

def altar.place_on.put(+args: J.Val, +floor: J.Val, +identity: Bool, +i: U32, +n: U32) -> J.Val:
  altar.place_on.go(
    J.set(floor, "altar_at", J.set(ga.pv(J.has(floor, "altar_at"), J.get(floor, "altar_at"), J.obj()), U32.show(i),
      altar.place(J.nth(J.get(floor, "rooms"), i), J.nth(args, 0), J.nth(args, 1)))),
    identity, (i + 1 : U32), n)

# --- the Run ---------------------------------------------------------------------------

# Stand the wish altar in every room that has one and does not yet keep a spot.
def altar.Run.choose_altars(+run: J.Val) -> J.Val:
  J.set(run, "dungeon", altar.place_on(J.get(run, "dungeon"), ga.b(run, "identity")))

# The room the party is in keeps an altar whose spot is not yet chosen.
def altar.Run.altar_due(+run: J.Val) -> Bool:
  Bool.and(altar.keeps_an_altar(J.get(run, "dungeon"), ga.u(run, "at"), ga.b(run, "identity")),
    Bool.not(J.has(J.get(J.get(run, "dungeon"), "altar_at"), U32.show(ga.u(run, "at")))))

# Where the wish altar stands in the room the party is in: none in a room
# without one, or under the identity start. Chosen once, and kept on the floor.
def altar.Run.wish_altar(+run: J.Val) -> J.Val:
  altar.Run.wish_altar.in(altar.keeps_an_altar(J.get(run, "dungeon"), ga.u(run, "at"), ga.b(run, "identity")), run)

def altar.Run.wish_altar.in(+keeps: Bool, +run: J.Val) -> J.Val:
  match keeps:
    case False{}:
      J.JNull{}
    case True{}:
      altar.Run.wish_altar.kept(J.has(J.get(J.get(run, "dungeon"), "altar_at"), U32.show(ga.u(run, "at"))), run)

def altar.Run.wish_altar.kept(+has: Bool, +run: J.Val) -> J.Val:
  match has:
    case True{}:
      J.get(J.get(J.get(run, "dungeon"), "altar_at"), U32.show(ga.u(run, "at")))
    case False{}:
      altar.Run.wish_altar.open(J.get(J.get(run, "dungeon"), "open"), run)

def altar.Run.wish_altar.open(+open: J.Val, +run: J.Val) -> J.Val:
  altar.Run.wish_altar.here(Bool.and(Bool.not(J.is_null(open)), U32.is_eq(ga.u(open, "map"), ga.u(run, "at"))), open, run)

def altar.Run.wish_altar.here(+here: Bool, +open: J.Val, +run: J.Val) -> J.Val:
  match here:
    case True{}:
      altar.place(rules.Run.room(run), J.get(open, "arrival"), J.JA{altar.centres(J.items(J.get(open, "chambers")))})
    case False{}:
      altar.place(rules.Run.room(run), J.JNull{}, J.JA{[]})

# Knight `id` stands within reach of the lamp.
def altar.Run.near_altar(+run: J.Val, +id: U32) -> Bool:
  altar.Run.near_altar.in(altar.Run.wish_altar(run), ga.at(J.get(run, "players"), id))

def altar.Run.near_altar.in(+spot: J.Val, +hero: J.Val) -> Bool:
  altar.Run.near_altar.go(Bool.or(J.is_null(spot), J.is_null(hero)), spot, hero)

def altar.Run.near_altar.go(+no: Bool, +spot: J.Val, +hero: J.Val) -> Bool:
  match no:
    case True{}:
      False{}
    case False{}:
      F32.is_lt(
        RS.f_hypot(F32.sub(ga.f(hero, "x"), J.f32(J.nth(spot, 0))), F32.sub(ga.f(hero, "y"), F32.sub(J.f32(J.nth(spot, 1)), F32.mul(0.5, layout.TILE_UNITS())))),
        F32.mul(altar.REACH(), layout.TILE_UNITS()))

# --- the lamp's hooks from the host -------------------------------------------------------

# A lamp field that serde skips when empty: an empty array is left out.
def altar.Lamp.put(+lamp: J.Val, +k: String, +items: List<&2, J.Val>) -> J.Val:
  altar.Lamp.put.in(U32.is_zero(ga.count(items)), lamp, k, items)

def altar.Lamp.put.in(+empty: Bool, +lamp: J.Val, +k: String, +items: List<&2, J.Val>) -> J.Val:
  match empty:
    case True{}:
      J.del(lamp, k)
    case False{}:
      J.set(lamp, k, J.JA{items})

def altar.Lamp.has_words(+ps: List<&2, J.Val>, +id: U32, +words: String) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case Con{+p, +t}:
      Bool.or(Bool.and(U32.is_eq(ga.u(p, "knight"), id), String.eq(ga.s(p, "words"), words)), altar.Lamp.has_words(t, id, words))

# The host handed knight `id`'s words to a grant runner: the lamp smokes until
# it lands or is let go.
def altar.Run.lamp_sent(+run: J.Val, +id: U32, +words: String) -> J.Val:
  altar.Run.lamp_sent.in(altar.Lamp.has_words(J.items(J.get(J.get(run, "lamp"), "wishing")), id, words), run, id, words)

def altar.Run.lamp_sent.in(+same: Bool, +run: J.Val, +id: U32, +words: String) -> J.Val:
  match same:
    case True{}:
      run
    case False{}:
      altar.Run.lamp_sent.push(J.items(J.get(J.get(run, "lamp"), "wishing")), run, id, words)

def altar.Run.lamp_sent.push(+ws: List<&2, J.Val>, +run: J.Val, +id: U32, +words: String) -> J.Val:
  J.set(run, "lamp", altar.Lamp.put(J.get(run, "lamp"), "wishing",
    List.append(&2, J.Val, altar.Lamp.cap(ws, altar.MOST_PENDING()), [ga.sv(ga.ss(ga.su(J.obj(), "knight", id), "words", words), "since", J.get(run, "tick"))])))

# `if len >= most { remove(0) }`.
def altar.Lamp.cap(+ws: List<&2, J.Val>, +most: U32) -> List<&2, J.Val>:
  altar.Lamp.cap.in(U32.is_ge(ga.count(ws), most), ws)

def altar.Lamp.cap.in(+full: Bool, +ws: List<&2, J.Val>) -> List<&2, J.Val>:
  match full:
    case True{}:
      ga.remove_nth(ws, 0n)
    case False{}:
      ws

# Knight `id`'s wish in those words is no longer in flight (the first match).
def altar.Lamp.drop(+ps: List<&2, J.Val>, +id: U32, +words: String) -> List<&2, J.Val>:
  match ps:
    case Nil{}:
      Nil{}
    case Con{+p, +t}:
      altar.Lamp.drop.at(Bool.and(U32.is_eq(ga.u(p, "knight"), id), String.eq(ga.s(p, "words"), words)), p, t, id, words)

def altar.Lamp.drop.at(+hit: Bool, +p: J.Val, +t: List<&2, J.Val>, +id: U32, +words: String) -> List<&2, J.Val>:
  match hit:
    case True{}:
      t
    case False{}:
      Con{p, altar.Lamp.drop(t, id, words)}

def altar.Run.lamp_dropped(+run: J.Val, +id: U32, +words: String) -> J.Val:
  J.set(run, "lamp", altar.Lamp.put(J.get(run, "lamp"), "wishing", altar.Lamp.drop(J.items(J.get(J.get(run, "lamp"), "wishing")), id, words)))

# Knight `id`'s wish in those words came back granted as `name`: the lamp
# flares and the knight is told, wherever they are.
def altar.Run.lamp_landed(+run: J.Val, +id: U32, +words: String, +name: String) -> J.Val:
  altar.Run.lamp_landed.in(altar.Run.lamp_dropped(run, id, words), id, name)

def altar.Run.lamp_landed.in(+run: J.Val, +id: U32, +name: String) -> J.Val:
  J.set(run, "lamp", altar.Lamp.put(J.get(run, "lamp"), "landed",
    List.append(&2, J.Val, altar.Lamp.cap(J.items(J.get(J.get(run, "lamp"), "landed")), altar.MOST_LANDED()),
      [ga.sv(ga.ss(ga.su(J.obj(), "knight", id), "name", name), "tick", J.get(run, "tick"))])))

# --- E at the lamp ---------------------------------------------------------------------------

# `held` is a BTreeSet<u32>: a sorted array of ids.
def altar.held.has(+held: J.Val, +id: U32) -> Bool:
  altar.held.has.go(J.items(held), id)

def altar.held.has.go(+xs: List<&2, J.Val>, +id: U32) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case Con{+x, +t}:
      Bool.or(U32.is_eq(J.u32(x), id), altar.held.has.go(t, id))

def altar.held.insert(+xs: List<&2, J.Val>, +id: U32) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      [J.JU{id}]
    case Con{+x, +t}:
      altar.held.insert.at(U32.is_lt(id, J.u32(x)), x, t, id)

def altar.held.insert.at(+before: Bool, +x: J.Val, +t: List<&2, J.Val>, +id: U32) -> List<&2, J.Val>:
  match before:
    case True{}:
      Con{J.JU{id}, Con{x, t}}
    case False{}:
      Con{x, altar.held.insert(t, id)}

def altar.held.remove(+xs: List<&2, J.Val>, +id: U32) -> List<&2, J.Val>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+x, +t}:
      altar.held.remove.at(U32.is_eq(J.u32(x), id), x, t, id)

def altar.held.remove.at(+same: Bool, +x: J.Val, +t: List<&2, J.Val>, +id: U32) -> List<&2, J.Val>:
  match same:
    case True{}:
      t
    case False{}:
      Con{x, altar.held.remove(t, id)}

def altar.Run.set_held(+run: J.Val, +held: List<&2, J.Val>) -> J.Val:
  J.set(run, "lamp", J.set(J.get(run, "lamp"), "held", J.JA{held}))

# The stone is stone: a knight standing in it is set on the nearest edge, and
# a walk into a corner slides round the end. One hero.
def altar.slide(+hero: J.Val, +input: J.Val, +left: F32, +right: F32, +top: F32, +foot: F32) -> J.Val:
  altar.slide.in(
    Bool.and(Bool.and(F32.is_gt(ga.f(hero, "x"), left), F32.is_lt(ga.f(hero, "x"), right)), Bool.and(F32.is_gt(ga.f(hero, "y"), top), F32.is_lt(ga.f(hero, "y"), foot))),
    hero, input, left, right, top, foot)

def altar.slide.in(+inside: Bool, +hero: J.Val, +input: J.Val, +left: F32, +right: F32, +top: F32, +foot: F32) -> J.Val:
  match inside:
    case False{}:
      hero
    case True{}:
      altar.slide.side(
        altar.slide.nearest(F32.sub(ga.f(hero, "x"), left), F32.sub(right, ga.f(hero, "x")), F32.sub(ga.f(hero, "y"), top), F32.sub(foot, ga.f(hero, "y"))),
        hero, RS.i_as_f32(ga.u(input, "move_x")), RS.i_as_f32(ga.u(input, "move_y")), left, right, top, foot)

# The side whose distance is least, the first of equals: 0 west, 1 east,
# 2 north, 3 south.
def altar.slide.nearest(+d0: F32, +d1: F32, +d2: F32, +d3: F32) -> U32:
  altar.slide.nearest.keep(F32.is_gt(d0, d1), d0, d1, d2, d3)

def altar.slide.nearest.keep(+gt: Bool, +d0: F32, +d1: F32, +d2: F32, +d3: F32) -> U32:
  altar.slide.nearest.two(ga.pu(gt, 1, 0), ga.pf(gt, d1, d0), d2, d3)

def altar.slide.nearest.two(+side: U32, +best: F32, +d2: F32, +d3: F32) -> U32:
  altar.slide.nearest.three(ga.pu(F32.is_gt(best, d2), 2, side), ga.pf(F32.is_gt(best, d2), d2, best), d3)

def altar.slide.nearest.three(+side: U32, +best: F32, +d3: F32) -> U32:
  ga.pu(F32.is_gt(best, d3), 3, side)

def altar.slide.side(+side: U32, +hero: J.Val, +mx: F32, +my: F32, +left: F32, +right: F32, +top: F32, +foot: F32) -> J.Val:
  altar.slide.put(side,
    ga.pf(U32.is_lt(side, 2), my, mx),
    ga.pb(U32.is_eq(side, 0), F32.is_gt(mx, 0.0), ga.pb(U32.is_eq(side, 1), F32.is_lt(mx, 0.0), ga.pb(U32.is_eq(side, 2), F32.is_gt(my, 0.0), F32.is_lt(my, 0.0)))),
    ga.pf(U32.is_lt(side, 2), F32.sub(ga.f(hero, "y"), top), F32.sub(ga.f(hero, "x"), left)),
    ga.pf(U32.is_lt(side, 2), F32.sub(foot, ga.f(hero, "y")), F32.sub(right, ga.f(hero, "x"))),
    hero, left, right, top, foot)

def altar.slide.put(+side: U32, +along: F32, +deeper: Bool, +span_near: F32, +span_far: F32, +hero: J.Val, +left: F32, +right: F32, +top: F32, +foot: F32) -> J.Val:
  altar.slide.edge(side,
    ga.pf(F32.is_ne(along, 0.0), F32.mul(RS.f_signum(along), altar.EDGE_SLIDE()),
      ga.pf(Bool.and(deeper, F32.is_lt(RS.f_min(span_near, span_far), altar.CORNER())),
        ga.pf(F32.is_lt(span_near, span_far), F32.neg(altar.EDGE_SLIDE()), altar.EDGE_SLIDE()), 0.0)),
    hero, left, right, top, foot)

def altar.slide.edge(+side: U32, +slide: F32, +hero: J.Val, +left: F32, +right: F32, +top: F32, +foot: F32) -> J.Val:
  altar.slide.along(side, slide,
    ga.sf(ga.sf(hero, "x", ga.pf(U32.is_eq(side, 0), left, ga.pf(U32.is_eq(side, 1), right, ga.f(hero, "x")))),
      "y", ga.pf(U32.is_eq(side, 2), top, ga.pf(U32.is_eq(side, 3), foot, ga.f(hero, "y")))))

def altar.slide.along(+side: U32, +slide: F32, +hero: J.Val) -> J.Val:
  altar.slide.along.in(U32.is_lt(side, 2), slide, hero)

def altar.slide.along.in(+vertical: Bool, +slide: F32, +hero: J.Val) -> J.Val:
  match vertical:
    case True{}:
      ga.sf(hero, "y", F32.add(ga.f(hero, "y"), slide))
    case False{}:
      ga.sf(hero, "x", F32.add(ga.f(hero, "x"), slide))

# Every knight standing in the stone, set on its edge.
def altar.slides(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +players: J.Val, +inputs: J.Val, +left: F32, +right: F32, +top: F32, +foot: F32) -> J.Val:
  match hs:
    case Nil{}:
      players
    case Con{(+k, +h), +t}:
      altar.slides(t, J.set(players, k, altar.slide(h, altar.input_of(J.get(inputs, k)), left, right, top, foot)), inputs, left, right, top, foot)

# An absent seat's keys are `Input::default()`.
def altar.input_of(+i: J.Val) -> J.Val:
  ga.pv(J.is_null(i), rules.Input.default(), i)

# Who rubs: the knights newly pressing E in reach of the lamp, in seat order.
# The run (with `held` updated) and the rubbers.
def altar.rubs(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +run: J.Val, +inputs: J.Val, +acc: List<&2, U32>) -> J.Val & List<&2, U32>:
  match hs:
    case Nil{}:
      (run, List.reverse(&2, U32, acc))
    case Con{(+k, +h), +t}:
      altar.rubs.id(Maybe.default(&2, U32, U32.read(k), 0), h, t, run, inputs, acc)

def altar.rubs.id(+id: U32, +h: J.Val, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +run: J.Val, +inputs: J.Val, +acc: List<&2, U32>) -> J.Val & List<&2, U32>:
  altar.rubs.down(Bool.and(rules.Input.valid(J.get(inputs, U32.show(id))), ga.b(J.get(inputs, U32.show(id)), "interact")), id, h, t, run, inputs, acc)

def altar.rubs.down(+down: Bool, +id: U32, +h: J.Val, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +run: J.Val, +inputs: J.Val, +acc: List<&2, U32>) -> J.Val & List<&2, U32>:
  match down:
    case False{}:
      altar.rubs(t, altar.Run.set_held(run, altar.held.remove(J.items(J.get(J.get(run, "lamp"), "held")), id)), inputs, acc)
    case True{}:
      altar.rubs.live(Bool.or(U32.is_zero(ga.u(h, "hp")), ga.b(h, "absent")), id, t, run, inputs, acc)

def altar.rubs.live(+dead: Bool, +id: U32, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +run: J.Val, +inputs: J.Val, +acc: List<&2, U32>) -> J.Val & List<&2, U32>:
  match dead:
    case True{}:
      altar.rubs(t, run, inputs, acc)
    case False{}:
      altar.rubs.fresh(altar.held.has(J.get(J.get(run, "lamp"), "held"), id), id, t, run, inputs, acc)

def altar.rubs.fresh(+held: Bool, +id: U32, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +run: J.Val, +inputs: J.Val, +acc: List<&2, U32>) -> J.Val & List<&2, U32>:
  match held:
    case True{}:
      altar.rubs(t, run, inputs, acc)
    case False{}:
      altar.rubs.near(altar.Run.near_altar(run, id), id, t, altar.Run.set_held(run, altar.held.insert(J.items(J.get(J.get(run, "lamp"), "held")), id)), inputs, acc)

def altar.rubs.near(+near: Bool, +id: U32, +t: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +run: J.Val, +inputs: J.Val, +acc: List<&2, U32>) -> J.Val & List<&2, U32>:
  match near:
    case True{}:
      altar.rubs(t, run, inputs, Con{id, acc})
    case False{}:
      altar.rubs(t, run, inputs, acc)

# Each rub: the lamp rubbed, or the knight told they hold no wish.
def altar.rub(+run: J.Val, +ids: List<&2, U32>) -> J.Val:
  match ids:
    case Nil{}:
      run
    case Con{+id, +t}:
      altar.rub(altar.rub.one(J.is_null(J.get(ga.at(J.get(run, "players"), id), "offer")), run, id), t)

def altar.rub.one(+open: Bool, +run: J.Val, +id: U32) -> J.Val:
  match open:
    case False{}:
      run
    case True{}:
      altar.rub.wish(overclass.Run.can_wish(run, id), run, id)

def altar.rub.wish(+can: Bool, +run: J.Val, +id: U32) -> J.Val:
  match can:
    case True{}:
      ga.push(J.set(run, "lamp", J.set(J.get(run, "lamp"), "rubbed", J.set(ga.pv(J.has(J.get(run, "lamp"), "rubbed"), J.get(J.get(run, "lamp"), "rubbed"), J.obj()), U32.show(id), J.get(run, "tick")))), "sounds", J.JS{"spell_cast"})
    case False{}:
      J.set(run, "found", J.JA{[J.get(run, "tick"), J.JU{id}, J.JS{"NO WISH HELD: EACH FLOOR BRINGS TWO"}]})

def altar.Run.tick_wish_altar.stone(+run: J.Val, +spot: J.Val, +inputs: J.Val) -> J.Val:
  altar.Run.tick_wish_altar.rubs(
    altar.Run.tick_wish_altar.slid(run, inputs,
      F32.sub(J.f32(J.nth(spot, 0)), F32.mul(1.5, layout.TILE_UNITS())), F32.add(J.f32(J.nth(spot, 0)), F32.mul(1.5, layout.TILE_UNITS())),
      F32.sub(J.f32(J.nth(spot, 1)), F32.mul(1.4, layout.TILE_UNITS())), F32.add(J.f32(J.nth(spot, 1)), 0.6)),
    inputs)

def altar.Run.tick_wish_altar.slid(+run: J.Val, +inputs: J.Val, +left: F32, +right: F32, +top: F32, +foot: F32) -> J.Val:
  J.set(run, "players", altar.slides(J.int_fields(J.get(run, "players")), J.get(run, "players"), inputs, left, right, top, foot))

def altar.Run.tick_wish_altar.rubs(+run: J.Val, +inputs: J.Val) -> J.Val:
  altar.Run.tick_wish_altar.rubbed(altar.rubs(J.int_fields(J.get(run, "players")), run, inputs, Nil{}))

def altar.Run.tick_wish_altar.rubbed(+r: J.Val & List<&2, U32>) -> J.Val:
  (run, ids) = r
  altar.rub(run, ids)

# E at the lamp: a knight who holds a wish rubs it, and their wish line opens;
# one who holds none is told so. `inputs` is the object of seat id -> Input.
def altar.Run.tick_wish_altar(+run: J.Val, +inputs: J.Val) -> J.Val:
  altar.Run.tick_wish_altar.due(altar.Run.altar_due(run), run, inputs)

def altar.Run.tick_wish_altar.due(+due: Bool, +run: J.Val, +inputs: J.Val) -> J.Val:
  match due:
    case True{}:
      altar.Run.tick_wish_altar.spot(altar.Run.choose_altars(run), inputs)
    case False{}:
      altar.Run.tick_wish_altar.spot(run, inputs)

def altar.Run.tick_wish_altar.spot(+run: J.Val, +inputs: J.Val) -> J.Val:
  altar.Run.tick_wish_altar.have(altar.Run.wish_altar(run), run, inputs)

def altar.Run.tick_wish_altar.have(+spot: J.Val, +run: J.Val, +inputs: J.Val) -> J.Val:
  altar.Run.tick_wish_altar.have.m(J.is_null(spot), spot, run, inputs)

def altar.Run.tick_wish_altar.have.m(+flag: Bool, +spot: J.Val, +run: J.Val, +inputs: J.Val) -> J.Val:
  match flag:
    case True{}:
      altar.Run.set_held(run, Nil{})
    case False{}:
      altar.Run.tick_wish_altar.stone(run, spot, inputs)
