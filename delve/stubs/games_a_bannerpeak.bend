def rules.Input.valid(+i: J.Val) -> Bool:
  True{}

def rules.Run.room(+run: J.Val) -> J.Val:
  J.nth(J.get(J.get(run, "dungeon"), "rooms"), ga.u(run, "at"))

def roundtable.number(+tune: J.Val, +slot: U32, +i: U32) -> U32:
  J.u32(J.nth(J.nth(J.get(tune, "roundtable"), slot), i))

def forest.Forest.leave(+f: J.Val) -> J.Val:
  +f1 = ga.su(f, "hunt_left", 0)
  +f2 = ga.su(f1, "caught", 0)
  +f3 = ga.sb(f2, "singing", False{})
  +f4 = ga.su(f3, "show_left", 0)
  +f5 = ga.su(f4, "note", 0)
  ga.su(f5, "held", 0)

def exits.Run.leave_activity(+run: J.Val, +id: U32) -> Sigma<&2, &2, J.Val, _ => Bool>:
  (run, False{})

def realm.Spoils.add(+sp: J.Val, +spoil: String, +n: U32) -> J.Val:
  Bool.pick(J.Val, U32.is_zero(n), sp, J.set(sp, spoil, J.JU{ga.sadd32(ga.u(sp, spoil), n)}))
