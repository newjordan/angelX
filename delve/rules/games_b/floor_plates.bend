# crates/delve/src/rules/floor_plates.rs — seat-local floor title plates.

def floor_plates.Playing.length(+p: J.Val) -> U32:
  gb.sat8(gb.u(p, "results"), 54)

# Run::begin_floor_plate: a plate at age 0 for every seat.
def floor_plates.Run.begin_floor_plate(+run: J.Val, +kind: String) -> J.Val:
  gb.put_nonempty(run, "floor_plates", floor_plates.begin.go(J.fields(J.get(run, "players")), kind, rules.Run.floor(run), J.obj()))

def floor_plates.begin.go(+ps: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +kind: String, +depth: U32, m: J.Val) -> J.Val:
  match ps:
    case Nil{}:
      m
    case Con{(+id, +h), +t}:
      floor_plates.begin.go(t, kind, depth, J.set(m, id, floor_plates.Playing.new(kind, depth)))

def floor_plates.Playing.new(+kind: String, +depth: U32) -> J.Val:
  gb.ss(gb.su(gb.su(J.obj(), "depth", depth), "age", 0), "kind", kind)

# Run::skip_floor_plate: whether the seat had a plate (and now has none).
def floor_plates.Run.skip_floor_plate(+run: J.Val, +seat: U32) -> J.Val & Bool:
  floor_plates.skip.go(J.get(run, "floor_plates"), seat, run)

def floor_plates.skip.go(+plates: J.Val, +seat: U32, run: J.Val) -> J.Val & Bool:
  (gb.put_nonempty(run, "floor_plates", J.del(plates, gb.key(seat))), J.has(plates, gb.key(seat)))

# Run::tick_floor_plates: age every plate one step; one whose title is done
# (or whose seat pressed skip) goes away.
def floor_plates.Run.tick_floor_plates(+run: J.Val, +inputs: J.Val) -> J.Val:
  gb.put_nonempty(run, "floor_plates", floor_plates.tick.go(J.fields(J.get(run, "floor_plates")), inputs, depths.Run.fight_on(run), J.obj()))

def floor_plates.tick.go(+ps: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +inputs: J.Val, +fight: Bool, m: J.Val) -> J.Val:
  match ps:
    case Nil{}:
      m
    case Con{(+id, +p), +t}:
      floor_plates.tick.go(t, inputs, fight, floor_plates.tick.keep(floor_plates.tick.age(p, fight), id, gb.b(J.get(inputs, id), "skip_plate"), m))

def floor_plates.tick.age(+p: J.Val, +fight: Bool) -> J.Val:
  gb.su(p, "age", gb.sat8(Bool.pick(U32, Bool.and(fight, U32.is_lt(gb.u(p, "age"), gb.u(p, "results"))), gb.u(p, "results"), gb.u(p, "age")), 1))

def floor_plates.tick.keep(+p: J.Val, +id: String, +skip: Bool, m: J.Val) -> J.Val:
  floor_plates.tick.keep.if(Bool.and(U32.is_lt(gb.u(p, "age"), floor_plates.Playing.length(p)), Bool.not(skip)), id, p, m)

def floor_plates.tick.keep.if(keep: Bool, +id: String, p: J.Val, m: J.Val) -> J.Val:
  match keep:
    case True{}:
      J.set(m, id, p)
    case False{}:
      m

# Cases: the recorder's {run, ...} in, {run, ret?} out.
def case.floor_plates.Run.begin_floor_plate(+input: J.Val) -> J.Val:
  J.set(J.obj(), "run", floor_plates.Run.begin_floor_plate(J.get(input, "run"), J.str(J.get(input, "kind"))))

def case.floor_plates.Run.tick_floor_plates(+input: J.Val) -> J.Val:
  J.set(J.obj(), "run", floor_plates.Run.tick_floor_plates(J.get(input, "run"), J.get(input, "inputs")))

def case.floor_plates.Run.skip_floor_plate(+input: J.Val) -> J.Val:
  floor_plates.skip.out(floor_plates.Run.skip_floor_plate(J.get(input, "run"), J.u32(J.get(input, "seat"))))

def floor_plates.skip.out(r: J.Val & Bool) -> J.Val:
  (run, had) = r
  J.set(J.set(J.obj(), "run", run), "ret", J.JBool{had})

# Run::floor_plate: the seat's plate while it is still playing, or null.
def floor_plates.Run.floor_plate(+run: J.Val, +seat: U32) -> J.Val:
  floor_plates.floor_plate.go(J.get(J.get(run, "floor_plates"), gb.key(seat)))

def floor_plates.floor_plate.go(+p: J.Val) -> J.Val:
  floor_plates.floor_plate.if(Bool.and(Bool.not(J.is_null(p)), U32.is_lt(gb.u(p, "age"), floor_plates.Playing.length(p))), p)

def floor_plates.floor_plate.if(playing: Bool, +p: J.Val) -> J.Val:
  match playing:
    case True{}:
      p
    case False{}:
      J.JNull{}
