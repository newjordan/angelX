# Names other groups own that games_b calls. A stub answers a harmless default
# and drops out of the build as soon as a fragment defines the name.

# rules.rs
def rules.Run.floor(+run: J.Val) -> U32:
  gb.u(J.get(run, "dungeon"), "depth")

# depths.rs
def depths.Run.fight_on(+run: J.Val) -> Bool:
  False{}

# forest_road.rs
def forest_road.Run.forest_road_showing(+run: J.Val) -> Bool:
  False{}

# roundtable.rs
def roundtable.Run.roundtable_showing(+run: J.Val) -> Bool:
  False{}

# rules.rs: Grid { room, barred }.slide(from, delta, r, mover);
# mover: 0 Hero, 1 Walker, 2 Flier, 3 Wader.
def rules.Grid.slide(+room: J.Val, +barred: Bool, p: F32 & F32, d: F32 & F32, +r: F32, +mover: U32) -> F32 & F32:
  (+x, +y) = p
  (+dx, +dy) = d
  rules.Grid.slide.bog(room, barred, x, y, dx, dy, r, mover, rules.Grid.tile_at(room, x, y))

def rules.Grid.slide.bog(+room: J.Val, +barred: Bool, +x: F32, +y: F32, +dx: F32, +dy: F32, +r: F32, +mover: U32, +tile: String) -> F32 & F32:
  +walks = Bool.and(U32.is_le(mover, 1), String.eq(tile, "bog"))
  rules.Grid.slide.hero(room, barred, x, y, Bool.pick(F32, walks, (dx * 0.55 : F32), dx), Bool.pick(F32, walks, (dy * 0.55 : F32), dy), r, mover)

def rules.Grid.slide.hero(+room: J.Val, +barred: Bool, +x: F32, +y: F32, +dx: F32, +dy: F32, +r: F32, +mover: U32) -> F32 & F32:
  rules.Grid.slide.pick(Bool.and(U32.is_eq(mover, 0), Bool.not(rules.Grid.clear(room, barred, x, y, r, mover))), room, barred, x, y, dx, dy, r, mover)

def rules.Grid.slide.pick(stuck: Bool, +room: J.Val, +barred: Bool, +x: F32, +y: F32, +dx: F32, +dy: F32, +r: F32, +mover: U32) -> F32 & F32:
  match stuck:
    case True{}:
      (Bool.pick(F32, F32.is_gt((dx * (U32.to_f32(gb.u(room, "cols")) - x : F32) : F32), 0.0), (x + dx : F32), x), Bool.pick(F32, F32.is_gt((dy * (U32.to_f32(gb.u(room, "rows")) - y : F32) : F32), 0.0), (y + dy : F32), y))
    case False{}:
      rules.Grid.slide.axes(room, barred, x, y, dx, dy, r, mover)

def rules.Grid.slide.axes(+room: J.Val, +barred: Bool, +x: F32, +y: F32, +dx: F32, +dy: F32, +r: F32, +mover: U32) -> F32 & F32:
  +x2 = Bool.pick(F32, rules.Grid.clear(room, barred, (x + dx : F32), y, r, mover), (x + dx : F32), x)
  +y2 = Bool.pick(F32, rules.Grid.clear(room, barred, x2, (y + dy : F32), r, mover), (y + dy : F32), y)
  (x2, y2)

def rules.Grid.tile_at(+room: J.Val, +x: F32, +y: F32) -> String:
  +col = RS.i_clamp(RS.f_as_i32(F32.floor((x / 2.0 : F32))), 0, RS.i_sub(gb.u(room, "cols"), 1))
  +row = RS.i_clamp(RS.f_as_i32(F32.floor((y / 2.0 : F32))), 0, RS.i_sub(gb.u(room, "rows"), 1))
  J.str(layout.Room.tile(room, col, row))

def rules.Grid.open(+room: J.Val, +barred: Bool, +x: F32, +y: F32, +mover: U32) -> Bool:
  rules.Grid.open.tile(rules.Grid.tile_at(room, x, y), barred, mover)

def rules.Grid.open.tile(+t: String, +barred: Bool, +mover: U32) -> Bool:
  Bool.or(Bool.or(String.eq(t, "floor"), String.eq(t, "stairs")), Bool.or(String.eq(t, "ledge"), Bool.or(String.eq(t, "bog"), Bool.or(Bool.and(String.eq(t, "hazard"), U32.is_eq(mover, 2)), Bool.and(String.eq(t, "door"), Bool.and(U32.is_eq(mover, 0), Bool.not(barred)))))))

def rules.Grid.clear(+room: J.Val, +barred: Bool, +x: F32, +y: F32, +r: F32, +mover: U32) -> Bool:
  Bool.and(Bool.and(rules.Grid.open(room, barred, (x - r : F32), (y - r : F32), mover), rules.Grid.open(room, barred, (x + r : F32), (y - r : F32), mover)), Bool.and(rules.Grid.open(room, barred, (x - r : F32), (y + r : F32), mover), rules.Grid.open(room, barred, (x + r : F32), (y + r : F32), mover)))

def rules.Run.barred(+run: J.Val) -> Bool:
  False{}

# rules.rs: fn unit(x, y) -> (x / length, y / length), or (0, 0).
def rules.unit(+x: F32, +y: F32) -> F32 & F32:
  rules.unit.go(RS.f_hypot(x, y), x, y)

def rules.unit.go(+length: F32, +x: F32, +y: F32) -> F32 & F32:
  rules.unit.if(F32.is_gt(length, 0.0), length, x, y)

def rules.unit.if(pos: Bool, +length: F32, +x: F32, +y: F32) -> F32 & F32:
  match pos:
    case True{}:
      ((x / length : F32), (y / length : F32))
    case False{}:
      (0.0, 0.0)

# home.rs: Home::level(station) over the `levels` map.
def home.Home.level(+home: J.Val, +station: String) -> U32:
  J.u32(J.get(J.get(home, "levels"), station))

# cards.rs: Book::get(id), the card with that id.
def cards.Book.get(+book: J.Val, +id: String) -> J.Val:
  cards.Book.get.go(J.items(J.get(book, "cards")), id)

def cards.Book.get.go(+cs: List<&2, J.Val>, +id: String) -> J.Val:
  match cs:
    case Nil{}:
      J.JNull{}
    case Con{+c, +t}:
      cards.Book.get.pick(String.eq(J.str(J.get(c, "id")), id), c, t, id)

def cards.Book.get.pick(hit: Bool, +c: J.Val, +t: List<&2, J.Val>, +id: String) -> J.Val:
  match hit:
    case True{}:
      c
    case False{}:
      cards.Book.get.go(t, id)

# hero.rs: Hero::spend(&mut self, card, score, nova): the hero, score, nova.
def hero.Hero.spend(+hero: J.Val, +card: J.Val, +score: U32, +nova: U32) -> J.Val & U32 & U32:
  (hero, score, nova)

# chatter.rs: clip(id, name, n) -> the sound's name.
def chatter.clip(+id: U32, +name: String, +n: U32) -> String:
  "knights/" ++ Bool.pick(String, U32.is_eq(U32.mod(gb.ssub(id, 1), 4), 0), "a", Bool.pick(String, U32.is_eq(U32.mod(gb.ssub(id, 1), 4), 1), "b", Bool.pick(String, U32.is_eq(U32.mod(gb.ssub(id, 1), 4), 2), "c", "d"))) ++ "_" ++ name ++ "_" ++ U32.show((U32.mod(n, 2) + 1 : U32))

# feats.rs: Run::notice(id).
def feats.Run.notice(+run: J.Val, +id: String) -> J.Val:
  feats.Run.notice.if(gb.any_eq(J.items(J.get(run, "feats")), id), run, id)

def feats.Run.notice.if(known: Bool, +run: J.Val, +id: String) -> J.Val:
  match known:
    case True{}:
      run
    case False{}:
      J.set(run, "feats", J.push(J.get(run, "feats"), J.JS{id}))

# monsters.rs: Enemy::radius().
def monsters.Enemy.radius(+e: J.Val) -> F32:
  0.5

# rules.rs: Input::valid().
def rules.Input.valid(+input: J.Val) -> Bool:
  True{}

# rules.rs: Run::room().
def rules.Run.room(+run: J.Val) -> J.Val:
  J.nth(J.get(J.get(run, "dungeon"), "rooms"), gb.u(run, "at"))

# realm.rs: Spoils::add(spoil, n): a spoil's count saturating up, n > 0.
def realm.Spoils.add(+spoils: J.Val, +spoil: String, +n: U32) -> J.Val:
  realm.Spoils.add.if(U32.is_gt(n, 0), spoils, spoil, n)

def realm.Spoils.add.if(go: Bool, +spoils: J.Val, +spoil: String, +n: U32) -> J.Val:
  match go:
    case True{}:
      J.set(spoils, spoil, J.JU{gb.sadd(J.u32(J.get(spoils, spoil)), n)})
    case False{}:
      spoils
    
# loot.rs: the run's generator (the combat group's port).
def loot.Rng.below(+r: W.W, +n: U32) -> W.W & U32:
  loot.Rng.below.go(loot.Rng.next(r), n)

def loot.Rng.below.go(rv: W.W & W.W, +n: U32) -> W.W & U32:
  (r2, o) = rv
  (r2, W.rem_u32(o, U32.max(n, 1)))

def loot.Rng.next(+r: W.W) -> W.W & W.W:
  +x1 = W.xor(r, W.shr(r, 12))
  +x2 = W.xor(x1, W.shl(x1, 25))
  +x3 = W.xor(x2, W.shr(x2, 27))
  (x3, W.mul(x3, W.W{625341585, 1332534557}))

# bowcountry.rs: LINE and ROPE (the popinjay line and its rope, in tiles).
def bowcountry.LINE() -> F32:
  10.0

def bowcountry.ROPE() -> F32:
  5.0


# layout.rs: the tile's serde name (the world group's, copied for the stubs).
def layout.tile_name(+t: LayTile) -> String:
  match t:
    case TileFloor{}:
      "floor"
    case TileWall{}:
      "wall"
    case TileBlock{}:
      "block"
    case TileHazard{}:
      "hazard"
    case TileDoor{}:
      "door"
    case TileStairs{}:
      "stairs"
    case TileLedge{}:
      "ledge"
    case TileBog{}:
      "bog"

# (unused here: Room::set reaches it)
def layout.tile_from_name(+s: String) -> LayTile:
  TileFloor{}

# pastimes/{powder,gander,siege}.rs ticks — until they are ported.

def pastimes.gander.Run.tick_gander(+run: J.Val, +round: J.Val, +inputs: J.Val, +elig: List<&2, J.Val>, +tapped: List<&2, U32>) -> J.Val & (J.Val & Bool):
  (run, (round, True{}))


# crossroads.rs: Run::finish_crossroads(seats).
def crossroads.Run.finish_crossroads(+run: J.Val, +seats: List<&2, U32>) -> J.Val:
  run

