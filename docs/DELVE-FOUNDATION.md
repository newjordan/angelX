# The Delve's foundation

How the Delve steps, draws and talks to friends: the server and its
snapshots, the presentation pipeline, the wire, the activity registry, and
how to add a new game on top of it. The engine underneath (`angel-serve`,
`angel-net`, the browser runtime) is `docs/ENGINE.md`; its section "The
Delve on the engine" is the long form of 1 to 3 here.

The rule it exists for: frame rate is never again a reason to park a feature.

## 1. The server (`cockpit/src/drive/together_sim.rs`)

```
 terminal keys ─► host's seat (in process) ─┐                     ┌─► every seat's own client draws its screen
 couch partner ─► its seat (in process) ────┼─► angel-serve ─► DelveServer ─► tick tree ─┤   (the host's too)
 friends ─► /ws (browser wasm, angelX) ─────┘   30 Hz  Run::step     per seat    └─► cockpit: latched snapshot,
 cockpit edits ─► Sim::as_mut: the run under its lock, between ticks                 news, settle, checkpoint
```

- **One server for every delve.** A delve's run lives with its
  `DelveServer` (`angel_serve::GameServer`, document mode, 30 Hz). A solo
  delve's server listens on loopback with a key nobody is told; `/dungeon
  host` moves the same run onto a server at the host's door, and
  `/dungeon kick` back.
- **One clock.** The server steps on its own thread, or with
  `ANGEL_DELVE_SIM_THREAD=0` on the terminal's, by hand once a frame
  (`Manual::run_to`); both are the engine's one beat. A beat more
  than five ticks late (a laptop that slept) starts over. Paused, the beat
  goes on and the run stands, so friends still hear the host.
- **Commands.** Each seat sends `DelveCmd` (the keys held, and pressed since
  the last command) once a tick; the server takes one a seat a tick and
  acknowledges it. No leases, no wall-clock stamps, no sub-tick back-dating.
- **Snapshots.** After every tick, and after an edit, the run is copied
  once into an immutable `Snapshot` for the cockpit; the tick's tree is
  built from that copy, outside the lock.
- **The terminal only reads.** Once a frame `advance_shooter_at` hands the
  server this frame's controls (the composer, the couch, a wish being
  forged), samples its own seats' keys, latches the newest snapshot, plays
  its own seat's sounds and the chorus's lines, and settles what the run
  banked into the realm. Every read (`dungeon.shooter.as_ref()`) is the
  latched snapshot. An edit (`as_mut()`) takes the run under its lock,
  between ticks; the next read sees it.
- **Off the terminal's thread.** The spell cards are read, the feel ledger
  and the three-second checkpoint written, on the `delve-service` thread
  (inline in the pump when stepped by hand), from a published snapshot:
  nothing does IO holding the run.
- **Tests** step the same server by hand with their own clock;
  `Sim::edit()` (tests only) edits without a lock.

## 2. Presentation

- **Every seat is a client.** The host's own knight plays through an
  in-process connection (`Server::connect_local`) and a
  `NativeLink<DelveClient>`, as a friend's angelX does over a socket and a
  browser does in wasm. Each draws its own screen from the replicated run:
  everyone else between the two ticks around its render tick, its own
  knight from its unanswered commands and the keys held now.
- **A render worker.** The host's picture is drawn, scaled and PNG-encoded
  on the viewer's render worker (Kitty), or on the seat's own worker (half
  blocks). The terminal asks for this frame's picture and places the newest
  finished one; it waits at most `ANGEL_DELVE_RENDER_WAIT_MS` (4 ms) for
  its own, then shows the last.
- **Display rate.** Frames pace at 60 a second, or `ANGEL_DELVE_FPS`
  (30 to 240). The server keeps 30 Hz.
- **Images** are packed RGBA words (`0xAARRGGBB`, alpha 0 transparent): a
  stamp, a compose, a copy or a conversion is a pass over plain words.
- **The sprite atlas.** A sprite read from ink rows is packed once into its
  render thread's atlas sheet (shelf-packed, 1024 wide); asking for it again
  copies it back out of the sheet.
- **The draw list** (`arena/draw_list.rs`). A scene's painters each name
  their layer: `Ground`, `Figures`, `Over`, `Boards`, then `Screen` after the
  camera's crop. The list paints layer by layer, in the order painters were
  added within a layer, so where a call sits no longer decides what covers
  what. Inside `Figures` the crowd is painted in three ranks (guardians,
  other monsters, knights) and within a rank from the back of the room to
  the front, so whoever stands lower is in front. Knights are never lost
  behind a monster.
- **What slides.** Between two ticks the pose (`mirror::Pose`) slides
  knights, monsters, shots and cameras. Everything else a game moves
  (riders, chariots, crabs, pigeons, critters) steps at the tick rate: the
  generic float motion that slid them cost a whole serialize and
  deserialize of the run a frame, on every client (`angel_delve::client`).

## 3. The wire (`angel_delve::wire`, carried by `angel-serve`)

- **One port, one socket.** The host serves the Delve's page (the angel-web
  shell and the Delve's wasm, built by the cockpit's `build.rs` and embedded)
  at `/` and the engine's protocol at `/ws`. A browser opens the link; a
  friend's angelX pastes the same link into `/dungeon join` (`http`, or
  `https` through a tunnel or a relay).
- **Trees, not structs.** The run is serialized into a `value::Value` tree
  (field and variant names kept) by its own `Serialize`, and read back by its
  own `Deserialize`. A field added to `Run`, or anything in it, is on the wire
  the moment it exists. The server's dice and next id stay home; the marks
  and pot shards the renderer reads ride beside the run's fields.
- **Patches.** Each seat gets its tree whole, then patches against the tick
  it acknowledged: a struct's changed fields, a map's changed entries, and a
  list of things with ids by id.
- **Per-seat interest.** Each seat gets its own share of the tree
  (`wire::seat_tree`): another knight's own heat is left out, and a seat
  outside the party's room gets none of the shots flying in it.
- **Events.** Sounds, the chorus's lines and notices go as events, each to
  whoever hears it; each client makes its own sounds of them.
- **Actions.** What a friend sends that is not keys (a boon chosen, a spell
  learned, a reforged card, a wish, a card file in pieces) is an engine
  action (`together_gear`), handed to the cockpit as news.

## 4. Games as a registry (`crates/delve/src/rules/activity.rs`)

Every game played in a room is an `Activity` with one lifecycle:

| hook | what it is |
| --- | --- |
| `name` | its name (shown by `/dungeon status`) |
| `on` | whether it is under way in this run |
| `ride` | a ride's tick: it plays, then gives the keys the rest of the tick sees (the lists, the Hippodrome, a race hall) |
| `tick` | everything else's tick, at its place in the step |
| `end` | over: its state cleared |
| `ends_outside` | ended when a knight's view leaves its room (`clear_activity`) |
| `settle` | what it pays into the realm at the save (`App::settle_home`) |
| `fields` | the `Run` fields that hold it (the wire leaves them out for a seat outside the party's room) |

The registry has five lists, walked in order: `RIDES` (the first under way
takes the keys), `FOLLIES` (the follies and rooms' small games), `HOME_ROOMS`
(the Undercroft room the party stands in), `WORLD` (the kingdom and its
rooms' games) and `PLACES` (the Undercroft, barony, holdings, College, ranch
and merchant). The if-chains that dispatched them (`advance`'s reins,
`tick_home`'s match, `tick_world`, `clear_activity`, the settle calls) now
walk these lists.

**Presentation is an event stream.** The step emits what it shows rather
than setting it: `self.fx.push(Fx::Shake(n))`, chain sparks as
`Fx::Spark`, sounds into `sounds`, the chorus's moments into `cues`.
`Run::present` (after each tick's rules, `fx.rs`) turns the tick's shakes
and sparks into what the picture draws, ages them and keeps the caps; the
sim thread hands sounds and cues to the cockpit and the friends as events.
Nothing in the rules reads presentation back.

**The sim reads the world through one door.** `rules/map.rs` is the only
module through which rules reach the world's map data (places, roads,
gates, castles, houses: `angel_delve::world`); the sim no longer warms the
renderer's kingdom atlas (the picture does it, on its own thread).

## How to add a new game

1. **State.** An `Option` field on `Run` with `#[serde(default)]`, set to
   `None` in `Run::new`, checked in `valid_snapshot`. That is all the wire
   needs: every seat gets it from the run's own `Serialize`, and its
   positions slide between ticks by themselves.
2. **Rules.** `impl Run { pub(super) fn tick_my_game(&mut self, inputs) }`,
   doing nothing when the state is `None`. Start it where the party walks
   into its room.
3. **Register it.** One row in `activity::FOLLIES` (or `RIDES` if it takes
   the keys of the knights it seats, `WORLD` for a room of the world):
   name, `on`, `tick`, `end`, `fields`, and `settle` if it pays the realm.
4. **Effects.** Shakes and sparks as `fx` events, sounds in `sounds`, the
   chorus's moments in `cues`.
5. **Picture.** A painter in `arena/my_game.rs`, added in
   `scene_layers_with` with the layer it belongs to:
   `scene.add(Layer::Ground, |cv| my_game::figures(cv, run))`. Its place in
   the function does not matter; its layer does. Each seat's client draws
   it, on a render worker or in the browser, never on the terminal's thread.
6. **Look.** An ignored shot writer under `ANGEL_ARENA_SHOTS`, and open every
   render.
