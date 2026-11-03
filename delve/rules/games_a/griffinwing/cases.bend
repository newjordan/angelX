# Case wrappers: `case.<bend name>(input) -> output`, the JSON shape the Rust
# recorder (crates/delve/tests/rules/griffinwing_bend_cases__tests.rs) writes.

def case.griffinwing.types.norm(+i: J.Val) -> J.Val:
  J.JU{griffinwing.types.norm(ga.nu(i, 0), ga.nu(i, 1), ga.nu(i, 2))}

def case.griffinwing.types.flat(+i: J.Val) -> J.Val:
  J.JU{griffinwing.types.flat(ga.nu(i, 0), ga.nu(i, 1), ga.nu(i, 2), ga.nu(i, 3))}

def case.griffinwing.types.scale3(+i: J.Val) -> J.Val:
  gw.v3j(griffinwing.types.scale3(ga.nu(J.get(i, "d"), 0), ga.nu(J.get(i, "d"), 1), ga.nu(J.get(i, "d"), 2), ga.u(i, "len")))

def case.griffinwing.types.tri(+i: J.Val) -> J.Val:
  J.of_i32(griffinwing.types.tri(ga.u(i, "t"), ga.u(i, "period"), ga.u(i, "amp")))

def case.griffinwing.types.near(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.types.near(
    ga.nu(J.get(i, "a"), 0), ga.nu(J.get(i, "a"), 1), ga.nu(J.get(i, "a"), 2),
    ga.nu(J.get(i, "b"), 0), ga.nu(J.get(i, "b"), 1), ga.nu(J.get(i, "b"), 2),
    ga.nu(J.get(i, "p"), 0), ga.nu(J.get(i, "p"), 1), ga.nu(J.get(i, "p"), 2), ga.u(i, "r"))}

def case.griffinwing.types.Foe.alive(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.types.Foe.alive(i)}

def case.griffinwing.types.Foe.lockable(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.types.Foe.lockable(i)}

def case.griffinwing.types.Craft.new(+i: J.Val) -> J.Val:
  griffinwing.types.Craft.new(ga.u(i, "x"), ga.u(i, "z"))

def case.griffinwing.types.Craft.valid(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.types.Craft.valid(i)}

def case.griffinwing.flight.Craft.reflecting(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.flight.Craft.reflecting(J.get(i, "c"), ga.u(i, "reflect"))}

def case.griffinwing.types.Beast.valid(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.types.Beast.valid(i)}

def case.griffinwing.types.Tally.valid(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.types.Tally.valid(i)}

def case.griffinwing.types.Tally.bank(+i: J.Val) -> J.Val:
  griffinwing.types.Tally.bank(J.get(i, "a"), J.get(i, "b"))

def case.griffinwing.things.shutter_gap(+i: J.Val) -> J.Val:
  J.of_i32(griffinwing.things.shutter_gap(J.get(i, "p"), ga.u(i, "t"), ga.u(i, "wx")))

def case.griffinwing.flight.bits(+i: J.Val) -> J.Val:
  J.JU{griffinwing.flight.bits(i)}

def case.griffinwing.flight.slot(+i: J.Val) -> J.Val:
  griffinwing.flight.slot(J.u32(i))

def case.griffinwing.flight.rider_slot(+i: J.Val) -> J.Val:
  griffinwing.flight.rider_slot(J.u32(i))

def case.griffinwing.bit(+i: J.Val) -> J.Val:
  J.JU{griffinwing.bit(J.u32(i))}

def case.griffinwing.Nums.of(+i: J.Val) -> J.Val:
  griffinwing.Nums.of(J.get(i, "tune"))

def case.griffinwing.Nums.wx(+i: J.Val) -> J.Val:
  J.of_i32(griffinwing.Nums.wx(i))

def case.griffinwing.Nums.wz(+i: J.Val) -> J.Val:
  J.of_i32(griffinwing.Nums.wz(i))

def case.griffinwing.Nums.reflect(+i: J.Val) -> J.Val:
  J.JU{griffinwing.Nums.reflect(i)}

def case.griffinwing.Session.new(+i: J.Val) -> J.Val:
  griffinwing.Session.new(J.get(i, "players"))

def case.griffinwing.Session.sit(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.sit(J.get(i, "s"), ga.u(i, "id"), ga.u(i, "joined")))

# {"s": s}
def gw.out_only(+s: J.Val) -> J.Val:
  ga.sv(J.obj(), "s", s)

def case.griffinwing.Session.anchor(+i: J.Val) -> J.Val:
  J.of_i32(griffinwing.Session.anchor(i))

def case.griffinwing.Session.pressure(+i: J.Val) -> J.Val:
  J.JU{griffinwing.Session.pressure(i)}

def case.griffinwing.Session.next_id(+i: J.Val) -> J.Val:
  griffinwing.Session.next_id(J.get(i, "s"))

def case.griffinwing.Session.sound(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.sound(J.get(i, "s"), ga.s(i, "name")))

def case.griffinwing.Session.say(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.say(J.get(i, "s"), ga.u(i, "who"), ga.s(i, "line")))

def case.griffinwing.Session.spark(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.spark(J.get(i, "s"), ga.s(i, "what"), ga.nu(J.get(i, "at"), 0), ga.nu(J.get(i, "at"), 1), ga.nu(J.get(i, "at"), 2), ga.u(i, "n")))

def case.griffinwing.Session.voice(+i: J.Val) -> J.Val:
  J.JU{griffinwing.Session.voice(i)}

def case.griffinwing.Session.fill_wing(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.fill_wing(J.get(i, "s")))

def case.griffinwing.Session.muster(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.muster(J.get(i, "s"), J.get(i, "heroes")))

def case.griffinwing.Session.step(+i: J.Val) -> J.Val:
  griffinwing.Session.step(J.get(i, "s"), J.get(i, "inputs"), J.get(i, "heroes"), J.get(i, "n"))

def case.griffinwing.Session.tick_lists(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.tick_lists(J.get(i, "s")))

def case.griffinwing.Session.end_beat(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.end_beat(J.get(i, "s")))

def case.griffinwing.Session.next_beat(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.next_beat(J.get(i, "s")))

def case.griffinwing.Session.restart(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.restart(J.get(i, "s")))

def case.griffinwing.Session.finish(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.Session.finish(J.get(i, "s")))

def case.griffinwing.Session.medal_now(+i: J.Val) -> J.Val:
  J.JS{griffinwing.Session.medal_now(i)}

def case.griffinwing.Session.purse(+i: J.Val) -> J.Val:
  J.JU{griffinwing.Session.purse(i)}

def case.griffinwing.Session.finished(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.Session.finished(i)}

def case.griffinwing.Session.permille(+i: J.Val) -> J.Val:
  J.JU{griffinwing.Session.permille(i)}

def case.griffinwing.flight.Session.craft_s(+i: J.Val) -> J.Val:
  J.of_i32(griffinwing.flight.Session.craft_s(J.get(i, "s"), J.get(i, "c")))

def case.griffinwing.flight.Session.craft_at(+i: J.Val) -> J.Val:
  griffinwing.flight.Session.craft_at(J.get(i, "s"), ga.u(i, "id"))

def case.griffinwing.flight.Session.fly(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.flight.Session.fly(J.get(i, "s"), ga.u(i, "id"), J.get(i, "i"), J.get(i, "n")))

def case.griffinwing.flight.Session.wing_fly(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.flight.Session.wing_fly(J.get(i, "s"), J.get(i, "n")))

def case.griffinwing.flight.Session.step_craft(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.flight.Session.step_craft(J.get(i, "s"), J.get(i, "n")))

def case.griffinwing.combat.Session.step_foes(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.combat.Session.step_foes(J.get(i, "s"), J.get(i, "n")))

def case.griffinwing.combat.Session.step_bolts(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.combat.Session.step_bolts(J.get(i, "s"), J.get(i, "n")))

def case.griffinwing.boss.Session.step_boss(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.boss.Session.step_boss(J.get(i, "s"), J.get(i, "n")))

def case.griffinwing.script.Session.run_script(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.script.Session.run_script(J.get(i, "s"), J.get(i, "n")))

def case.griffinwing.things.Session.step_props(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.things.Session.step_props(J.get(i, "s"), J.get(i, "n")))

def case.griffinwing.Session.valid(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.Session.valid(i)}

def case.griffinwing.Session.repair(+i: J.Val) -> J.Val:
  griffinwing.Session.repair(i)

def case.griffinwing.bot.Session.bot_with(+i: J.Val) -> J.Val:
  griffinwing.bot.Session.bot_with(J.get(i, "s"), ga.u(i, "id"), J.get(i, "n"), ga.b(i, "novice"))

def case.griffinwing.bot.Session.bot_input(+i: J.Val) -> J.Val:
  griffinwing.bot.Session.bot_input(J.get(i, "s"), ga.u(i, "id"))

def case.griffinwing.bot.noise(+i: J.Val) -> J.Val:
  J.of_i32(griffinwing.bot.noise(ga.u(i, "id"), ga.u(i, "t"), ga.u(i, "salt"), ga.u(i, "span")))

def case.griffinwing.bot.Session.reflector(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.bot.Session.reflector(J.get(i, "s"), ga.u(i, "id"))}

def case.griffinwing.bot.Session.intercept(+i: J.Val) -> J.Val:
  griffinwing.bot.Session.intercept(J.get(i, "s"), ga.u(i, "s_c"))

def case.griffinwing.bot.Session.threat(+i: J.Val) -> J.Val:
  griffinwing.bot.Session.threat(J.get(i, "s"), J.get(i, "c"), ga.u(i, "s_c"))

def case.griffinwing.bot.Session.room_from(+i: J.Val) -> J.Val:
  griffinwing.bot.Session.room_from(J.get(i, "c"), ga.u(i, "ox"), ga.u(i, "oz"), J.get(i, "n"))

def case.griffinwing.bot.Session.footprint_away(+i: J.Val) -> J.Val:
  griffinwing.bot.Session.footprint_away(J.get(i, "s"), J.get(i, "c"), J.get(i, "n"))

def case.griffinwing.bot.Session.boss_corridor(+i: J.Val) -> J.Val:
  griffinwing.bot.Session.boss_corridor(J.get(i, "s"), J.get(i, "c"), J.get(i, "n"))

# {"s": ws.s, "c": ws.c} with the thing under another name.
def gw.named(+w: J.Val, +k: String) -> J.Val:
  ga.sv(ga.sv(J.obj(), "s", gw.ps(w)), k, gw.pc(w))

# ─── flight ─────────────────────────────────────────────────────────────────

def case.griffinwing.flight.Session.push_bolt(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.flight.Session.push_bolt(J.get(i, "s"), J.get(i, "b")))

def case.griffinwing.flight.Session.tap(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.flight.Session.tap(J.get(i, "s"), ga.u(i, "owner"), J.get(i, "c"), ga.u(i, "s_c")))

def case.griffinwing.flight.Session.acquire(+i: J.Val) -> J.Val:
  griffinwing.flight.Session.acquire(J.get(i, "s"), J.get(i, "c"), ga.u(i, "s_c"))

def case.griffinwing.flight.Session.release(+i: J.Val) -> J.Val:
  griffinwing.flight.Session.release(J.get(i, "s"), ga.u(i, "owner"), J.get(i, "c"), ga.u(i, "s_c"), J.get(i, "n"))

# ─── combat ─────────────────────────────────────────────────────────────────

def case.griffinwing.combat.Session.spawn_foe(+i: J.Val) -> J.Val:
  griffinwing.combat.Session.spawn_foe(J.get(i, "s"), ga.s(i, "kind"),
    ga.nu(J.get(i, "at"), 0), ga.nu(J.get(i, "at"), 1), ga.nu(J.get(i, "at"), 2),
    ga.nu(J.get(i, "vel"), 0), ga.nu(J.get(i, "vel"), 1), ga.nu(J.get(i, "vel"), 2),
    ga.s(i, "st"), ga.u(i, "mode"), ga.u(i, "phase"))

def case.griffinwing.combat.Session.hurt_craft(+i: J.Val) -> J.Val:
  griffinwing.combat.Session.hurt_craft(J.get(i, "s"), ga.u(i, "id"), ga.u(i, "dmg"))

def case.griffinwing.combat.Session.drain(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.combat.Session.drain(J.get(i, "s"), ga.u(i, "id"), ga.u(i, "amount")))

def case.griffinwing.combat.Session.latched(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.combat.Session.latched(J.get(i, "s"), ga.u(i, "id"))}

def case.griffinwing.combat.Session.victim(+i: J.Val) -> J.Val:
  griffinwing.combat.Session.victim(J.get(i, "s"), ga.u(i, "hunter"))

def case.griffinwing.combat.Session.target(+i: J.Val) -> J.Val:
  griffinwing.combat.Session.target(J.get(i, "s"), ga.u(i, "turn"))

def case.griffinwing.combat.Session.plane_shot(+i: J.Val) -> J.Val:
  gw.v3j(griffinwing.combat.Session.plane_shot(J.get(i, "s"),
    ga.nu(J.get(i, "o"), 0), ga.nu(J.get(i, "o"), 1), ga.nu(J.get(i, "o"), 2),
    ga.nu(J.get(i, "to"), 0), ga.nu(J.get(i, "to"), 1), ga.u(i, "closing")))

def case.griffinwing.combat.Session.marks(+i: J.Val) -> J.Val:
  griffinwing.combat.Session.marks(J.get(i, "f"), ga.u(i, "seats"))

def case.griffinwing.combat.Session.fire_marks(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.combat.Session.fire_marks(J.get(i, "s"), J.get(i, "f")))

def case.griffinwing.combat.Session.step_foe(+i: J.Val) -> J.Val:
  gw.named(griffinwing.combat.Session.step_foe(J.get(i, "s"), J.get(i, "f"), ga.u(i, "anchor"), J.get(i, "n")), "f")

def case.griffinwing.combat.Session.hunt(+i: J.Val) -> J.Val:
  gw.named(griffinwing.combat.Session.hunt(J.get(i, "s"), J.get(i, "f")), "f")

def case.griffinwing.combat.Session.hang(+i: J.Val) -> J.Val:
  gw.named(griffinwing.combat.Session.hang(J.get(i, "s"), J.get(i, "f")), "f")

def case.griffinwing.combat.Session.bombers_ram(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.combat.Session.bombers_ram(J.get(i, "s")))

def case.griffinwing.combat.Session.hit_foe(+i: J.Val) -> J.Val:
  griffinwing.combat.Session.hit_foe(J.get(i, "s"), ga.u(i, "fi"), ga.u(i, "dmg"), ga.u(i, "by"))

def case.griffinwing.combat.Session.kill(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.combat.Session.kill(J.get(i, "s"), ga.u(i, "fi"), ga.u(i, "by")))

def case.griffinwing.combat.Session.break_lamp(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.combat.Session.break_lamp(J.get(i, "s"), ga.u(i, "fi"), ga.u(i, "by")))

def case.griffinwing.combat.Session.starfall(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.combat.Session.starfall(J.get(i, "s"), ga.u(i, "by"), ga.u(i, "s_c"), ga.u(i, "x"), ga.u(i, "z")))

def case.griffinwing.combat.Session.friendly_bolt(+i: J.Val) -> J.Val:
  griffinwing.combat.Session.friendly_bolt(J.get(i, "s"), J.get(i, "b"),
    ga.nu(J.get(i, "from"), 0), ga.nu(J.get(i, "from"), 1), ga.nu(J.get(i, "from"), 2),
    ga.nu(J.get(i, "to"), 0), ga.nu(J.get(i, "to"), 1), ga.nu(J.get(i, "to"), 2), J.get(i, "dead"))

def case.griffinwing.combat.Session.land(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.combat.Session.land(J.get(i, "s"), ga.u(i, "i"), J.get(i, "b")))

def case.griffinwing.combat.Session.hostile_bolt(+i: J.Val) -> J.Val:
  griffinwing.combat.Session.hostile_bolt(J.get(i, "s"), J.get(i, "b"),
    ga.nu(J.get(i, "from"), 0), ga.nu(J.get(i, "from"), 1), ga.nu(J.get(i, "from"), 2),
    ga.nu(J.get(i, "to"), 0), ga.nu(J.get(i, "to"), 1), ga.nu(J.get(i, "to"), 2), J.get(i, "n"))

def case.griffinwing.combat.Session.turn(+i: J.Val) -> J.Val:
  case.griffinwing.combat.Session.turn.drop(griffinwing.combat.Session.turn(J.get(i, "s"), J.get(i, "b"), ga.u(i, "by"),
    ga.nu(J.get(i, "at"), 0), ga.nu(J.get(i, "at"), 1), ga.nu(J.get(i, "at"), 2)))

def case.griffinwing.combat.Session.turn.drop(+r: J.Val) -> J.Val:
  J.del(r, "r")

# ─── boss ───────────────────────────────────────────────────────────────────

def case.griffinwing.boss.Session.rise(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.boss.Session.rise(J.get(i, "s")))

def case.griffinwing.boss.Session.rest(+i: J.Val) -> J.Val:
  J.JU{griffinwing.boss.Session.rest(J.get(i, "s"), ga.u(i, "phase"))}

def case.griffinwing.boss.Session.begin_tell(+i: J.Val) -> J.Val:
  griffinwing.boss.Session.begin_tell(J.get(i, "s"), J.get(i, "b"), J.get(i, "n"))

def case.griffinwing.boss.Session.spit(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.boss.Session.spit(J.get(i, "s"), J.get(i, "b")))

# ─── script ─────────────────────────────────────────────────────────────────

def case.griffinwing.script.script(+i: J.Val) -> J.Val:
  griffinwing.script.script(J.u32(i))

def case.griffinwing.script.resume(+i: J.Val) -> J.Val:
  J.JU{griffinwing.script.resume(ga.u(i, "stage"), ga.u(i, "cp"))}

def case.griffinwing.script.Session.spread(+i: J.Val) -> J.Val:
  J.of_i32(griffinwing.script.Session.spread(ga.u(i, "k"), ga.u(i, "count")))

def case.griffinwing.script.Session.act(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.script.Session.act(J.get(i, "s"), J.get(i, "act")))

def case.griffinwing.script.Session.victory_gates(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.script.Session.victory_gates(J.get(i, "s")))

# ─── things ─────────────────────────────────────────────────────────────────

def case.griffinwing.things.Session.take(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.things.Session.take(J.get(i, "s"), ga.u(i, "id"), J.get(i, "p"), ga.b(i, "boosting")))

def case.griffinwing.things.Session.scrape(+i: J.Val) -> J.Val:
  gw.out_only(griffinwing.things.Session.scrape(J.get(i, "s"), ga.u(i, "id"), ga.u(i, "at"), ga.u(i, "x"), ga.u(i, "z")))

# ─── the Run's hooks ────────────────────────────────────────────────────────

def case.griffinwing.Run.griffinwing_showing(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.Run.griffinwing_showing(J.get(i, "run"))}

def case.griffinwing.Run.start_griffinwing(+i: J.Val) -> J.Val:
  ga.sv(J.obj(), "run", griffinwing.Run.start_griffinwing(J.get(i, "run")))

def case.griffinwing.Run.tick_griffinwing(+i: J.Val) -> J.Val:
  ga.sv(J.obj(), "run", griffinwing.Run.tick_griffinwing(J.get(i, "run"), J.get(i, "inputs")))

def case.griffinwing.Run.settle_griffinwing(+i: J.Val) -> J.Val:
  case.griffinwing.Run.settle.pack(griffinwing.Run.settle_griffinwing(J.get(i, "run"), J.get(i, "s")))

def case.griffinwing.Run.settle.pack(+w: J.Val) -> J.Val:
  ga.sv(ga.sv(J.obj(), "run", gw.ps(w)), "s", gw.pc(w))

def case.griffinwing.ADVENTURE.valid(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.ADVENTURE.valid(J.get(i, "run"))}

def case.griffinwing.ADVENTURE.clear(+i: J.Val) -> J.Val:
  ga.sv(J.obj(), "run", griffinwing.ADVENTURE.clear(J.get(i, "run")))

def case.griffinwing.ADVENTURE.seated(+i: J.Val) -> J.Val:
  griffinwing.ADVENTURE.seated(J.get(i, "run"))

def case.griffinwing.ADVENTURE.bot(+i: J.Val) -> J.Val:
  griffinwing.ADVENTURE.bot(J.get(i, "run"), ga.u(i, "id"))

def case.griffinwing.ACTIVITY.on(+i: J.Val) -> J.Val:
  J.JBool{griffinwing.ACTIVITY.on(J.get(i, "run"))}

def case.griffinwing.ACTIVITY.ride(+i: J.Val) -> J.Val:
  griffinwing.ACTIVITY.ride(J.get(i, "run"), J.get(i, "inputs"))

def case.griffinwing.ACTIVITY.end(+i: J.Val) -> J.Val:
  ga.sv(J.obj(), "run", griffinwing.ACTIVITY.end(J.get(i, "run")))

def case.griffinwing.settle(+i: J.Val) -> J.Val:
  griffinwing.settle(J.get(i, "realm"), J.get(i, "marks"))
