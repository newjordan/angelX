def activity.Run.tick_world_games(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def college.room(+kind: String, +cell: LayCell) -> J.Val:
  layout.cellar.walled(cell, kind)

def ranch.room(+kind: String, +cell: LayCell) -> J.Val:
  layout.cellar.walled(cell, kind)

def colosseum.room(+kind: String, +cx: U32, +cy: U32) -> J.Val:
  layout.cellar.walled(LayCell{cx, cy}, kind)

def alehouse.room(+cell: LayCell) -> J.Val:
  layout.cellar.walled(cell, "alehouse")

def bazaar.room(+cell: LayCell) -> J.Val:
  layout.cellar.walled(cell, "bazaar")

def barony.kings_hall_room(+cell: LayCell) -> J.Val:
  layout.cellar.walled(cell, "kings_hall")

def holdings.land_office_room(+cell: LayCell) -> J.Val:
  layout.cellar.walled(cell, "land_office")

def barony.Barony.built(+barony: J.Val, +work: String) -> Bool:
  False{}

def cinder.Run.cinder_showing(+run: J.Val) -> Bool:
  False{}

def mossbell.Run.mossbell_showing(+run: J.Val) -> Bool:
  False{}

def brasswake.Run.brasswake_showing(+run: J.Val) -> Bool:
  False{}

def rafterwake.Run.rafterwake_showing(+run: J.Val) -> Bool:
  False{}

def pennon.Run.pennon_showing(+run: J.Val) -> Bool:
  False{}

def colosseum.Run.arena_return(+run: J.Val) -> Maybe<&2, J.Val>:
  None{}

def chariot.Chariot.depart(+chariot: J.Val, +id: U32) -> J.Val:
  chariot

def college.Dialogue.default() -> J.Val:
  J.obj()

def hazards.Waves.default() -> J.Val:
  J.obj()

def hero.Hero.dress(+h: J.Val) -> J.Val:
  h

def hero.Hero.held_off(+h: J.Val, +input: J.Val) -> J.Val & J.Val:
  (h, input)

def hero.Pit.tick_hero(+h: J.Val) -> J.Val:
  h

def hireling.Run.hireling_follows(+run: J.Val) -> J.Val:
  run

def pastimes.Pastimes.depart(+p: J.Val, +id: U32, +remaining: List<&2, U32>) -> J.Val:
  p

def pastimes.Pastimes.leave(+p: J.Val) -> J.Val:
  p

def pitstop.Room.pit_box(+room: J.Val) -> J.Val:
  J.JNull{}

def rules.Run.depart_lists(+run: J.Val, +id: U32) -> J.Val:
  run

def rules.Run.end_outside_activities(+run: J.Val) -> J.Val:
  run

def works.Works.leave_room(+works: J.Val) -> J.Val:
  works

def barony.add_forge_hall(+dungeon: J.Val, +barony: J.Val) -> J.Val:
  dungeon

def holdings.add_holding_hall(+dungeon: J.Val, +holdings: J.Val) -> J.Val:
  dungeon

def boss_gates.Economy.has_camp(+gates: J.Val, +index: U32) -> Bool:
  False{}

def boss_gates.Gates.retire_all(+gates: J.Val, +enemies: J.Val) -> J.Val:
  gates

def boss_gates.Gates.unlocked(+gates: J.Val) -> Bool:
  True{}

def boss_gates.Run.boss_door_holds(+run: J.Val, +next: U32, +dir: U32) -> Bool:
  False{}

def boss_gates.Run.deploy_camp(+run: J.Val, +index: U32, +kind: String, +depth: U32) -> J.Val:
  run

def boss_gates.Run.guardian_locked(+run: J.Val) -> Bool:
  False{}

def boss_gates.Run.populate_boss_gates(+run: J.Val) -> J.Val:
  run

def boss_gates.Run.show_boss_lock(+run: J.Val) -> J.Val:
  run

def cauldron.Run.drink_round(+run: J.Val) -> J.Val:
  run

def champions.Field.clear(+f: J.Val) -> J.Val:
  f

def critters.Field.clear(+f: J.Val) -> J.Val:
  f

def gore.Gore.clear(+f: J.Val) -> J.Val:
  f

def feel.Feel.clear_room(+f: J.Val) -> J.Val:
  f

def fellowship.Fellowship.clear(+f: J.Val) -> J.Val:
  f

def curios.Hero.curio_enters(+h: J.Val, +delving: Bool) -> J.Val:
  h

def depths.Run.enter_open(+run: J.Val, +index: U32, +first_visit: Bool) -> J.Val:
  run

def depths.Run.open_doors(+run: J.Val) -> J.Val & Bool:
  (run, False{})

def depths.Run.open_guarded(+run: J.Val) -> Bool:
  False{}

def depths.Run.open_landing(+run: J.Val, +index: U32, +came_from: U32) -> J.Val:
  J.JNull{}

def depths.Run.open_spots(+run: J.Val, +index: U32, +at: J.Val, +n: U32) -> List<&2, Sigma<&2, &2, F32, _ => F32>>:
  Nil{}

def depths.Run.wants_open(+run: J.Val) -> Bool:
  False{}

def depths.open_floor(+depth: U32, +pack: String, +party: U32, +rng: W.W) -> J.Val & W.W:
  (J.JNull{}, rng)

def exhibits.Run.pips_map(+run: J.Val) -> J.Val:
  run

def fellowship.Fellowship.wait(+run: J.Val, +ids: List<&2, U32>, +way: U32) -> J.Val:
  run

def fellowship.Fellowship.wait_each(+run: J.Val, +xs: List<&2, Sigma<&2, &2, String, _ => J.Val>>) -> J.Val:
  run

def fellowship.LIGHT() -> U32:
  5

def fellowship.STAIRS() -> U32:
  4

def fellowship.Run.counts_in_fellowship(+run: J.Val, +id: U32) -> Bool:
  False{}

def floor_plates.Kind.of_pack(+pack: String) -> String:
  pack

def folly.Run.folly_running(+run: J.Val) -> Bool:
  False{}

def folly.Run.pick_folly(+run: J.Val) -> J.Val:
  run

def folly.Run.start_bat_shoot(+run: J.Val, +index: U32) -> J.Val:
  run

def folly.Run.start_wax_maze(+run: J.Val, +index: U32, +from: J.Val) -> J.Val:
  run

def frogs.Run.maybe_frog(+run: J.Val, +index: U32, +first_visit: Bool) -> J.Val:
  run

def gantry.Run.pick_gantry(+run: J.Val) -> J.Val:
  run

def hazards.Run.arm_room(+run: J.Val) -> J.Val:
  run

def hero.Hero.mend(+h: J.Val, +n: U32, +kind: String) -> J.Val:
  h

def hireling.Run.hire(+run: J.Val) -> J.Val:
  run

def home.Home.landing(+home: J.Val) -> U32:
  1

def home.Run.residents_help(+run: J.Val) -> J.Val:
  run

def home.Run.at_home_now(+run: J.Val) -> Bool:
  False{}

def identity.Run.creation_holds_stairs(+run: J.Val) -> Bool:
  False{}

def identity.Run.identity_room(+run: J.Val, +index: U32, +first_visit: Bool) -> J.Val:
  run

def identity.Run.simple_creation(+run: J.Val) -> Bool:
  False{}

def ledge.Run.arrive_side_on(+run: J.Val) -> J.Val:
  run

def outlaw.Run.raise_outlaw(+run: J.Val) -> J.Val:
  run

def overclass.Run.choose_altars(+run: J.Val) -> J.Val:
  run

def overclass.Run.floor_wishes(+run: J.Val) -> U32:
  2

def overclass.Run.give_wishes(+run: J.Val, +n: U32, +great: U32) -> J.Val:
  run

def rescues.Run.cage_someone(+run: J.Val) -> J.Val:
  run

def rules.Run.bank_coins(+run: J.Val) -> J.Val:
  run

def rules.Run.deepen(+run: J.Val) -> J.Val:
  run

def rules.Run.hallow(+run: J.Val) -> J.Val:
  run

def rules.Run.kit_out(+run: J.Val) -> J.Val:
  run

def rules.Run.lift_gifts(+run: J.Val) -> J.Val:
  run

def rules.Run.maybe_goblin(+run: J.Val) -> J.Val:
  run

def rules.Run.refill_winds(+run: J.Val) -> J.Val:
  run

def rules.Run.settle_dare(+run: J.Val) -> J.Val:
  run

def rules.Run.stock_gear_cache(+run: J.Val) -> J.Val:
  run

def rules.Run.swarm_up(+run: J.Val) -> J.Val:
  run

def rules.Run.unwashed_toll(+run: J.Val) -> J.Val:
  run

def rules.Run.bank_all(+run: J.Val, +done: Bool, +why: String) -> J.Val:
  run

def rules.Run.notice(+run: J.Val, +what: String) -> J.Val:
  run

def rules.Run.spawn(+run: J.Val, +kind: String) -> J.Val:
  run

def rules.Run.spawn_boss(+run: J.Val, +i: U32) -> J.Val:
  run

def secrets.Run.notice_crack(+run: J.Val, +first_visit: Bool) -> J.Val:
  run

def songs.Run.sing_song(+run: J.Val) -> J.Val:
  run

def spectacle.Run.step_into_gate(+run: J.Val) -> J.Val & Bool:
  (run, False{})

def tune.more_foes(+roster: List<&2, String>, +foe_count: U32) -> List<&2, String>:
  Nil{}

def ults.Run.deal_ults(+run: J.Val) -> J.Val:
  run

def works.Run.works_enter(+run: J.Val) -> J.Val:
  run

def yard.Run.raise_quintains(+run: J.Val) -> J.Val:
  run

def cat.Run.bring_tallow(+run: J.Val) -> J.Val:
  run

def dares.Run.call_dare(+run: J.Val) -> J.Val:
  run

def fortune.Run.half_charge(+run: J.Val) -> J.Val:
  run

def fortune.Run.harden(+run: J.Val) -> J.Val:
  run

def fortune.Run.iron(+run: J.Val) -> J.Val:
  run

def colosseum.Run.arena_heat_on(+run: J.Val) -> Bool:
  False{}

def race3d.Run.pick_race3d(+run: J.Val) -> J.Val:
  run

def race3d.Run.start_race3d(+run: J.Val, +index: U32) -> J.Val:
  run

def crossroads.Run.begin_crossroads(+run: J.Val, +depth: U32, +pack: String) -> J.Val:
  run

def fortune.Mode.collapse_secs(+depth: U32) -> U32:
  60

def floor_stats.Run.close_floor(+run: J.Val, +end: String) -> J.Val:
  run

def floor_stats.Tally.new_delve(+t: J.Val, +tick: J.Val) -> J.Val:
  t

def floor_stats.Tally.next_floor(+t: J.Val, +tick: J.Val) -> J.Val:
  t

def floor_stats.SHOWN() -> U32:
  200

def risen.FOLLOW() -> U32:
  240

def hero.Hero.mend2(+h: J.Val) -> J.Val:
  h

def countryside.Countryside.has_plot(+c: J.Val, +id: String) -> Bool:
  False{}

def countryside.room(+room: J.Val) -> J.Val:
  room

def gantry.Run.start_gantry(+run: J.Val, +index: U32) -> J.Val:
  run

def hop.Run.start_book_hop(+run: J.Val, +index: U32) -> J.Val:
  run

def powder.Run.start_powder(+run: J.Val, +index: U32) -> J.Val:
  run

def kingdom.Run.leave_kingdom_place(+run: J.Val) -> J.Val:
  run

def kingdom.Run.record_kingdom_visit(+run: J.Val) -> J.Val:
  run

def kingdom.Run.returns_to_kingdom(+run: J.Val, +entrance: String) -> Bool:
  False{}

def open.Open.door_on(+open: J.Val, +col: U32, +row: U32) -> J.Val:
  J.JNull{}

def open.SideDoor.landing(+door: J.Val) -> Sigma<&2, &2, F32, _ => F32>:
  (0.0, 0.0)

def roundtable.clear(+run: J.Val) -> J.Val:
  run

def runes.Run.place_rune(+run: J.Val) -> J.Val:
  run

def nooks.add_nook(+dungeon: J.Val) -> J.Val & Bool:
  (dungeon, False{})

def nooks.Run.ready_nook(+run: J.Val) -> J.Val:
  run

def kingdom.on_road(+run: J.Val, +x: F32, +y: F32) -> Bool:
  False{}

def folly.take_antes(+players: J.Val) -> J.Val:
  J.set(J.set(J.set(J.obj(), "players", players), "ante", J.obj()), "scores", J.obj())
