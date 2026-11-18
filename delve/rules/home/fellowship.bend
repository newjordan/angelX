def fellowship.STAIRS() -> U32:
  4

def fellowship.LIGHT() -> U32:
  5

def fellowship.CALL_EVERY() -> U32:
  120

def fellowship.State.is_empty(+s: J.Val) -> Bool:
  Bool.and(U32.is_eq(J.len(J.get(s, "waiting")), 0), Bool.and(U32.is_eq(J.len(J.get(s, "called")), 0), home.util.obj_empty(J.get(s, "rung"))))

def fellowship.Run.put(+run: J.Val, +state: J.Val) -> J.Val:
  home.util.ite(J.Val, fellowship.State.is_empty(state), u => J.del(run, "fellowship"), u => J.set(run, "fellowship", fellowship.State.shape(state)))

def fellowship.State.shape(+s: J.Val) -> J.Val:
  home.util.set_if(Bool.not(home.util.obj_empty(J.get(s, "rung"))),
    home.util.set_if(Bool.not(U32.is_eq(J.len(J.get(s, "called")), 0)),
      J.set(J.obj(), "waiting", home.util.ite(J.Val, J.has(s, "waiting"), u => J.get(s, "waiting"), u => J.JA{Nil{}})),
      "called", J.get(s, "called")),
    "rung", J.get(s, "rung"))

def fellowship.Run.clear(+run: J.Val) -> J.Val:
  fellowship.Run.put(run, J.set(J.set(J.get(run, "fellowship"), "waiting", J.JA{Nil{}}), "called", J.JA{Nil{}}))

def fellowship.Run.wait(+run: J.Val, +ways: List<&2, J.Val>) -> J.Val:
  fellowship.Run.put(run, J.set(J.get(run, "fellowship"), "waiting", J.JA{List.append(&2, J.Val, J.items(J.get(J.get(run, "fellowship"), "waiting")), ways)}))

def fellowship.Run.waiting_for_fellowship(+run: J.Val) -> Bool:
  Bool.not(U32.is_eq(J.len(J.get(J.get(run, "fellowship"), "waiting")), 0))

def fellowship.State.is_waiting(+s: J.Val, +id: U32) -> Bool:
  fellowship.is_waiting.go(J.items(J.get(s, "waiting")), id)

def fellowship.is_waiting.go(+ws: List<&2, J.Val>, +id: U32) -> Bool:
  match ws:
    case Nil{}:
      False{}
    case Con{+w, +t}:
      Bool.or(U32.is_eq(J.u32(home.util.get(J.items(w), 0)), id), fellowship.is_waiting.go(t, id))

def fellowship.Run.counts_in_fellowship(+run: J.Val, +id: U32) -> Bool:
  fellowship.counts.of(core.hero(run, id), rules.Run.away(run, id))

def fellowship.counts.of(+hero: J.Val, +away: Bool) -> Bool:
  match hero:
    case J.JNull{}:
      False{}
    case _:
      Bool.and(Bool.and(U32.is_gt(J.u32(J.get(hero, "hp")), 0), Bool.and(Bool.not(J.bool(J.get(hero, "stone"))), Bool.not(J.bool(J.get(hero, "absent"))))), Bool.not(away))

def fellowship.waiting_in(+ws: List<&2, J.Val>, +way: U32) -> U32:
  match ws:
    case Nil{}:
      0
    case Con{+w, +t}:
      (Bool.to_u32(U32.is_eq(J.u32(home.util.get(J.items(w), 1)), way)) + fellowship.waiting_in(t, way) : U32)

def fellowship.Run.gathered(+run: J.Val) -> J.Val:
  fellowship.gathered.go(J.items(J.get(J.get(run, "fellowship"), "waiting")), J.items(J.get(J.get(run, "fellowship"), "waiting")), J.JNull{})

def fellowship.gathered.go(+rest: List<&2, J.Val>, +all: List<&2, J.Val>, +best: J.Val) -> J.Val:
  match rest:
    case Nil{}:
      best
    case Con{+w, +t}:
      fellowship.gathered.go(t, all, fellowship.gathered.better(best, J.u32(home.util.get(J.items(w), 1)), fellowship.waiting_in(all, J.u32(home.util.get(J.items(w), 1)))))

def fellowship.gathered.better(+best: J.Val, +way: U32, +n: U32) -> J.Val:
  home.util.ite(J.Val, Bool.or(J.is_null(best), U32.is_gt(n, J.u32(home.util.get(J.items(best), 1)))), u => J.JA{[J.JU{way}, J.JU{n}]}, u => best)

def fellowship.counted(+hs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +run: J.Val) -> List<&2, U32>:
  match hs:
    case Nil{}:
      Nil{}
    case Con{(+k, +h), +t}:
      home.util.ite(List<&2, U32>, fellowship.Run.counts_in_fellowship(run, combos.num(k)), u => combos.num(k) <> fellowship.counted(t, run), u => fellowship.counted(t, run))

def fellowship.stands_in(+waiting: List<&2, J.Val>, +id: U32, +way: U32) -> Bool:
  match waiting:
    case Nil{}:
      False{}
    case Con{+w, +t}:
      Bool.or(Bool.and(U32.is_eq(J.u32(home.util.get(J.items(w), 0)), id), U32.is_eq(J.u32(home.util.get(J.items(w), 1)), way)), fellowship.stands_in(t, id, way))

def fellowship.not_standing(+ids: List<&2, U32>, +waiting: List<&2, J.Val>, +way: U32) -> List<&2, U32>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+id, +t}:
      home.util.ite(List<&2, U32>, fellowship.stands_in(waiting, id, way), u => fellowship.not_standing(t, waiting, way), u => id <> fellowship.not_standing(t, waiting, way))

def fellowship.Run.call_fellowship(+run: J.Val) -> J.Val:
  fellowship.call.go(run, fellowship.counted(J.int_fields(J.get(run, "players")), run), fellowship.Run.gathered(run))

def fellowship.call.go(+run: J.Val, +counted: List<&2, U32>, +gathered: J.Val) -> J.Val:
  fellowship.call.ring(run, home.util.ite(List<&2, U32>, Bool.and(Bool.not(J.is_null(gathered)), U32.is_gt((J.u32(home.util.get(J.items(gathered), 1)) * 2 : U32), U32.from_nat(List.length(&2, U32, counted)))),
    u => fellowship.not_standing(counted, J.items(J.get(J.get(run, "fellowship"), "waiting")), J.u32(home.util.get(J.items(gathered), 0))),
    u => Nil{}))

def fellowship.ids_has(+ids: List<&2, U32>, +id: U32) -> Bool:
  combos.set_has(ids, id)

def fellowship.call.ring(+run: J.Val, +called: List<&2, U32>) -> J.Val:
  fellowship.ring.each(run, called, called, fellowship.rung_kept(J.fields(J.get(J.get(run, "fellowship"), "rung")), called, J.obj()))

def fellowship.rung_kept(+fs: List<&2, Sigma<&2, &2, String, _ => J.Val>>, +called: List<&2, U32>, +acc: J.Val) -> J.Val:
  match fs:
    case Nil{}:
      acc
    case Con{(+k, +v), +t}:
      fellowship.rung_kept(t, called, home.util.ite(J.Val, fellowship.ids_has(called, combos.num(k)), u => J.set(acc, k, v), u => acc))

def fellowship.ring.each(+run: J.Val, +ids: List<&2, U32>, +called: List<&2, U32>, +rung: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      fellowship.ring.fin(run, called, rung)
    case Con{+id, +t}:
      fellowship.ring.one(run, id, t, called, rung, J.get(rung, U32.show(id)))

def fellowship.ring.one(+run: J.Val, +id: U32, +t: List<&2, U32>, +called: List<&2, U32>, +rung: J.Val, +at: J.Val) -> J.Val:
  home.util.ite(J.Val, Bool.or(J.is_null(at), W.ge(W.sat_sub(W.of_j(J.get(run, "tick")), W.of_j(at)), W.of_u32(120))),
    u => fellowship.ring.each(J.set(run, "seat_sounds", J.push(J.get(run, "seat_sounds"), J.JA{[J.JU{id}, J.JS{"circle_toll"}]})), t, called, J.set(rung, U32.show(id), J.get(run, "tick"))),
    u => fellowship.ring.each(run, t, called, rung))

def fellowship.ring.fin(+run: J.Val, +called: List<&2, U32>, +rung: J.Val) -> J.Val:
  fellowship.Run.put(run, J.set(J.set(J.get(run, "fellowship"), "rung", rung), "called", J.JA{fellowship.ids_json(called)}))

def fellowship.ids_json(+ids: List<&2, U32>) -> List<&2, J.Val>:
  match ids:
    case Nil{}:
      Nil{}
    case Con{+h, +t}:
      J.JU{h} <> fellowship.ids_json(t)

def fellowship.Run.fellowship_line(+run: J.Val, +focus: J.Val) -> J.Val:
  fellowship.line.of(J.items(J.get(J.get(run, "fellowship"), "waiting")), J.get(run, "fellowship"), focus)

def fellowship.line.of(+waiting: List<&2, J.Val>, +state: J.Val, +focus: J.Val) -> J.Val:
  match waiting:
    case Nil{}:
      J.JNull{}
    case Con{+w, +t}:
      home.util.ite(J.Val, Bool.and(Bool.not(J.is_null(focus)), fellowship.State.is_waiting(state, J.u32(focus))),
        u => J.JA{[J.JS{"WAITING FOR FELLOWSHIP"}, J.JBool{True{}}]},
        u => J.JA{[J.JS{"FRIENDS WAIT AT " ++ fellowship.way_name(J.u32(home.util.get(J.items(w), 1)))}, J.JBool{False{}}]})

def fellowship.way_name(+way: U32) -> String:
  home.util.ite(String, U32.is_eq(way, 0), u => "THE NORTH DOOR", u => home.util.ite(String, U32.is_eq(way, 1), u => "THE EAST DOOR", u => home.util.ite(String, U32.is_eq(way, 2), u => "THE SOUTH DOOR", u => home.util.ite(String, U32.is_eq(way, 3), u => "THE WEST DOOR", u => home.util.ite(String, U32.is_eq(way, 4), u => "THE STAIRS", u => "THE LIGHT HOME")))))

def fellowship.State.clear(+run: J.Val) -> J.Val:
  fellowship.Run.clear(run)
