# Names other groups own that games_c calls. A stub answers a harmless default
# and drops out of the build as soon as a fragment defines the name.

# rules.rs
def rules.Run.room(+run: J.Val) -> J.Val:
  J.get(J.get(J.get(run, "dungeon"), "rooms"), "0")

def rules.Input.valid(+input: J.Val) -> Bool:
  True{}

# layout.rs: Room::set(col, row, tile) over the room's run-length tiles.
def layout.Room.set(+room: J.Val, +col: U32, +row: U32, +t: LayTile) -> J.Val:
  room

# realm.rs: Spoils::add(&mut self, spoil, n); the spoil as its serde name.
def realm.Spoils.add(+spoils: J.Val, +spoil: String, +n: U32) -> J.Val:
  spoils

# layout.rs
def layout.TILE_UNITS() -> F32:
  2.0

# rules.rs constants (core's rules.<NAME>())
def rules.HZ() -> U32:
  30

def rules.DT() -> F32:
  0.03333333507180214

def rules.HERO_RADIUS() -> F32:
  0.41999998688697815

def rules.MAX_PLAYERS() -> U32:
  6

def rules.MAX_PROJECTILES() -> U32:
  384

def realm.Spoils.get(+spoils: J.Val, +spoil: String) -> U32:
  J.u32(J.get(spoils, spoil))

def realm.Spoils.take(+spoils: J.Val, +price: J.Val) -> J.Val:
  spoils

# rules.rs Input
def rules.Input.default() -> J.Val:
  J.set(J.obj(), "move_x", J.JU{0})

# loot.rs Rng (the generator is a u64 in the state)
def loot.Rng.new(+seed: W.W) -> W.W:
  seed

def loot.Rng.next(+r: W.W) -> W.W & W.W:
  (r, r)

def loot.Rng.below(+r: W.W, +n: U32) -> W.W & U32:
  (r, 0)

def ults.ULT_FULL() -> U32:
  1000

# layout.rs Room::solid_rock(cell) and the grid size; world.rs
def layout.COLS() -> U32:
  24

def layout.ROWS() -> U32:
  14

def layout.Room.solid_rock(+cx: U32, +cy: U32) -> J.Val:
  J.obj()

def world.room_of(+dungeon: J.Val, +kind: String) -> Maybe<&2, U32>:
  None{}

def world.COURTYARD_ARRIVAL() -> F32 & F32:
  (0.0, 0.0)

# forest.rs Forest::leave(&mut self)
def forest.Forest.leave(+forest: J.Val) -> J.Val:
  forest

# crossroads.rs (games_c)
def crossroads.Run.crossroads_voting(+run: J.Val) -> Bool:
  False{}

def crossroads.Run.crossroads_command(+run: J.Val, +id: U32, +command: J.Val) -> J.Val:
  run

# rules.rs
def rules.mix(+h: W.W) -> W.W:
  h

def rules.Run.away(+run: J.Val, +id: U32) -> Bool:
  False{}

# hitstop.rs
def hitstop.State.put(+run: J.Val, +s: J.Val) -> J.Val:
  run

# fellowship.rs State::clear on the run's fellowship
def fellowship.State.clear(+run: J.Val) -> J.Val:
  run

# floors.rs
def floors.Run.finish_descent(+run: J.Val, +depth: U32, +pack: J.Val, +from_home: Bool) -> J.Val:
  run

# overclass.rs
def overclass.MOST_WISHES() -> U32:
  3

# pastimes.rs (games_b)
def pastimes.State.leave(+state: J.Val) -> J.Val:
  state

def pastimes.Run.start_crossroads_pastime(+run: J.Val, +game: String) -> J.Val:
  run

def pastimes.Run.tick_pastimes(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

# race3d.rs and deathmatch.rs (games_c; drop out when ported)
def race3d.Race3d.order(+race: J.Val) -> List<&2, U32>:
  Nil{}

def race3d.Run.tick_race3d(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def race3d.Run.start_race3d_course(+run: J.Val, +index: U32, +course: String) -> J.Val:
  run

def deathmatch.Run.tick_deathmatch(+run: J.Val, +inputs: J.Val) -> J.Val:
  run

def deathmatch.Session.order(+s: J.Val) -> List<&2, U32>:
  Nil{}

def deathmatch.Session.new(+players: J.Val, +book: J.Val) -> J.Val:
  J.obj()

# ledge.rs, identity.rs, hazards.rs, monsters.rs, hero.rs, skills.rs, layout.rs
def ledge.Run.side_on(+run: J.Val) -> Bool:
  False{}

def identity.Run.simple_creation(+run: J.Val) -> Bool:
  False{}

def hazards.doorway.at(+room: J.Val, +side: U32) -> F32 & F32:
  (0.0, 0.0)

def monsters.EnemyKind.flies(+kind: String) -> Bool:
  False{}

def monsters.Enemy.radius(+e: J.Val) -> F32:
  0.5

def hero.Hero.hurt(+hero: J.Val, +damage: U32) -> J.Val:
  hero

def skills.Run.boom(+run: J.Val, +x: F32, +y: F32, +kind: String, +reach: F32) -> J.Val:
  run

def skills.GAS_LIFE() -> U32:
  240

def layout.Tile.ground(+t: LayTile) -> Bool:
  True{}

def layout.Room.tile(+room: J.Val, +col: U32, +row: U32) -> LayTile:
  TileFloor{}

def loot.Rng.chance(+r: W.W, +percent: U32) -> W.W & Bool:
  (r, False{})

def loot.Rng.pick(+r: W.W, +weights: List<&2, U32>) -> W.W & U32:
  (r, 0)

# layout.rs
def layout.Room.open_door(+room: J.Val, +dir: U32) -> J.Val:
  room

def layout.Room.great(+room: J.Val) -> Bool:
  False{}

def layout.connected(+room: J.Val) -> Bool:
  True{}

# rules.rs / hexer.rs / cards.rs / feel.rs / party_len
def rules.Run.spawn_at(+run: J.Val, +kind: String, +x: F32, +y: F32) -> J.Val & Maybe<&2, U32>:
  (run, None{})

def rules.Run.party_len(+run: J.Val) -> U32:
  1

def hexer.Run.hex(+run: J.Val, +id: U32) -> J.Val:
  run

def cards.HAND() -> U32:
  4

def feel.TEMPER_MAX() -> U32:
  3

# hero.rs
def hero.Hero.rebonus(+h: J.Val, +book: J.Val) -> J.Val:
  h

# gear.rs
def gear.Hero.dress(+h: J.Val) -> J.Val:
  h

# rules.rs Grid (the room's tiles as movement and shots see them; mover 0 Hero, 1 Walker, 2 Flier, 3 Wader)
def rules.Grid.clear(+room: J.Val, +barred: Bool, +x: F32, +y: F32, +r: F32, +mover: U32) -> Bool:
  True{}

def rules.Grid.open(+room: J.Val, +barred: Bool, +x: F32, +y: F32, +mover: U32) -> Bool:
  True{}

def hero.Hero.own_keys(+h: J.Val, +input: J.Val, +side: Bool, +shield_free: Bool) -> J.Val & J.Val:
  (h, J.obj())

def hero.Hero.own_step(+h: J.Val, +mx: F32, +my: F32, +room: J.Val, +barred: Bool, +roll_speed: F32, +pace: F32) -> J.Val & (F32 & F32 & F32):
  (h, (0.0, (0.0, 0.0)))

def rules.unit(+x: F32, +y: F32) -> F32 & F32:
  (x, y)

def rules.Grid.slide(+room: J.Val, +barred: Bool, +p: F32 & F32, +d: F32 & F32, +r: F32, +mover: U32) -> F32 & F32:
  p

def lich.CHILL_PACE() -> F32:
  0.5

def rules.ROLL_TICKS() -> U32:
  10

# hero / shield / aoe / cards / wishcraft / spells (core, home)
def hero.Hero.scaled(+h: J.Val, +damage: U32) -> U32:
  damage

def hero.Hero.cooldown(+h: J.Val, +ticks: U32) -> U32:
  ticks

def aoe.part(+damage: U32, +rank: U32) -> U32:
  damage

def cards.cap(+word: String) -> U32:
  8

def rules.ShotTraits.of(+bonus: J.Val) -> J.Val:
  J.obj()

def shield.turns(+h: J.Val, +dx: F32, +dy: F32) -> Bool:
  False{}

def shield.Hero.shield_turned(+h: J.Val, +damage: U32) -> J.Val & Bool:
  (h, False{})

def hero.Hero.cycle_hand(+h: J.Val) -> J.Val & Bool:
  (h, False{})

def spell_upgrades.Hero.spell_wait(+h: J.Val, +card: J.Val) -> U32:
  0

def spell_upgrades.Hero.spell_step(+h: J.Val, +id: String) -> J.Val:
  J.obj()

def spell_upgrades.Hero.spell_damage(+h: J.Val, +id: String, +base: U32) -> U32:
  base

def spell_upgrades.Hero.spell_power(+h: J.Val, +id: String, +base: U32) -> U32:
  base

def spells.Hero.spell_shots(+h: J.Val, +shape: String, +spell: String, +owner: U32, +shots: J.Val) -> J.Val:
  shots

def hero.Hero.spend(+h: J.Val, +card: J.Val) -> J.Val & (U32 & U32):
  (h, (0, 0))

def hero.Hero.rolling(+h: J.Val) -> Maybe<&2, U32>:
  None{}

def hero.Hero.loose(+h: J.Val, +id: U32, +book: J.Val, +burning: Bool, +rng: W.W, +flying: U32, +shots: J.Val) -> J.Val & J.Val:
  (h, shots)

def stakes.Hero.mend(+h: J.Val, +amount: U32, +how: U32) -> J.Val & U32:
  (h, 0)

def stakes.Hero.leech(+h: J.Val, +amount: U32, +how: U32) -> J.Val & U32:
  (h, 0)

def champions.of(+h: J.Val) -> String:
  ""

def wishcraft.shape(+shots: J.Val) -> J.Val:
  shots

def wishcraft.knock(+p: J.Val) -> F32:
  0.0

def wishcraft.turn_home(+p: J.Val) -> J.Val & Bool:
  (p, False{})

def spin.sweeping(+age: U32) -> Bool:
  False{}

def cards.SPELL_SLOTS() -> U32:
  4

def rules.Grid.halted(+room: J.Val, +barred: Bool, +from: F32 & F32, +d: F32 & F32) -> F32 & F32:
  from

def shield.entry(+ax: F32, +ay: F32, +bx: F32, +by: F32, +cx: F32, +cy: F32, +r: F32) -> Maybe<&2, F32>:
  None{}

def shield.meets(+h: J.Val, +cx: F32, +cy: F32, +ax: F32, +ay: F32, +bx: F32, +by: F32, +body: F32) -> Maybe<&2, J.Val>:
  None{}

def rules.Grid.shot_stop(+room: J.Val, +barred: Bool, +from: F32 & F32, +to: F32 & F32) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  None{}

def rules.Grid.shot_stop_body(+room: J.Val, +barred: Bool, +from: F32 & F32, +to: F32 & F32, +body: F32) -> Maybe<&2, Sigma<&2, &2, F32, _ => F32>>:
  None{}

def classes.shot_sound(+h: J.Val) -> Maybe<&2, String>:
  None{}

def gear.Hero.unarmed(+h: J.Val) -> Bool:
  False{}

# activity.rs
def activity.Run.leave_activity(+run: J.Val, +id: U32) -> J.Val & Bool:
  (run, False{})

def home.Run.at_home_now(+run: J.Val) -> Bool:
  False{}

def monsters.Enemy.is_boss(+e: J.Val) -> Bool:
  False{}

def monsters.EnemyKind.radius(+kind: String) -> F32:
  1.0

def classes.shot_look_some(+h: J.Val) -> Bool:
  False{}

def floors.Run.descend(+run: J.Val) -> J.Val:
  run

def layout.Room.width(+room: J.Val) -> F32:
  48.0

def layout.Room.height(+room: J.Val) -> F32:
  28.0

def gear.POP_TICKS() -> U32:
  30

def hero.ONE_HIT_MOST() -> U32:
  90
