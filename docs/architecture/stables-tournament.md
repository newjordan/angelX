# Stables and knights' practice tournament

[Architecture index](README.md) · [Commands](../COMMANDS.md)

This is a **local playable practice game**, shared by the owner's mini-world and
Delve home. It is not the `/tourney` benchmark/calibration system, and has no
model calls, paid resources, loot, trophies, research authority or verifier hooks.

## Controls

| Command | Effect |
|---|---|
| `/world visit stables` (also `stable`) | Focus the actual 3D stable district, including when the optional realm map is enabled. |
| `/world visit tournament` (also `knights`) | Focus the same district from the fenced lists. |
| `/world enter`, `/world leave`, `/world follow` | Enter the staged aisle/lists, leave to their exterior, return to the working knight. In the focused world pane, Enter enters and arrows move between four connected aisle/promenade stations. |
| `/dungeon stable [enter\|status]` | Open a 3D stable side stage if a solo Delve exists; otherwise open it in the mini-world. `status` does not open a stage. |
| `/dungeon stable select Bramble\|Cinder\|Mist` | Select a named mount. Bramble specializes in guard, Cinder in charge, Mist in aim. Selection is locked during an underway tournament. |
| `/dungeon stable tend` | Brush, water and check tack. Preparation does not stack; starting consumes it for one first-pass point. |
| `/dungeon tournament [enter\|status\|start]` | Open/check/start a three-pass local match. A second start during a match is rejected. |
| `/dungeon tournament round <1..3> guard\|aim\|charge` | Resolve exactly the next numbered pass. Repeats, skipped passes, invalid numbers/choices do nothing. |
| `/dungeon stable leave`, `/dungeon tournament leave` | Close practice; tournament leave marks an underway match left, without refunding preparation. Stable leave only closes the stable. Finished results remain until another start. |

In the **Delve practice overlay**: `1/2/3` select mounts, `E` tends, Enter starts a
match, `G/A/C` choose the current pass, arrows move the viewpoint, Esc leaves.
OS key-repeat cannot consume another pass. Slash commands are always available.
The home and yard have two walkable `E ENTER` approaches, with horse/stall and
lists/pennant projections reflecting the same owner state.

The rival announces charge, guard, aim on passes 1, 2, 3 respectively. Guard beats
charge; charge beats aim; aim beats guard. A win earns 3 practice points, a tie 1,
a loss 0, with +1 for a mount's specialty and the optional first-pass tending
point. The opponent's announced par is 2 points/pass (6 total). After three
choices the actual score produces WIN/DRAW/LOSS. This deliberately small,
deterministic game can be replayed; it is not a combat simulation or a benchmark.

## Actual runtime paths and ownership

```text
/dungeon stable|tournament       /world visit stables|tournament
        │                                   │
 app/control/dungeon_chivalry.rs  ← realm load/projection → commands.rs
        │                                   │
 drive/together_realm::Realm.chivalry         World.chivalry + Visit
        │ same-owner realm.json              │
        ├─ Run.chivalry (serde skipped)       ├─ ride::scryglass_frame_with_motion
        │   → overworld/arena/chivalry.rs     │   → world_viz/chivalry.rs
        │   → normal home/yard native +      │   → world3d/chivalry.rs meshes
        │     half-block Delve frames        │   → deterministic raster → Dotmax
        │                                    │
        └─ DungeonView.chivalry_visit ────────┘
            → ui::draw → chivalry_viz (ordinary Delve overlay)
```

`drive/chivalry.rs` owns only mount preparation and three fixed pass slots.
`Realm.chivalry` defaults when missing from old saves, and uses the existing
same-owner rewards/realm path. Valid changes compute a cloned game candidate, then use
that writer before installing its projection; save failure restores the old game.
No general-purpose writer or cross-owner import was added. The practice visit,
viewpoint and notice are ephemeral, not part of the run checkpoint.

A side visit clears held combat controls and elapsed-step debt, pauses **solo**
play and preserves the current floor/players. Closing/collapsing, restarting,
world travel and shutdown clear visit ownership/cache. It never generates or
replaces a Delve floor. Checkpoint restore rebuilds the private projection from
the realm on the next normal advance, without reopening a side stage.

## Rendering and privacy limits

- Three stall bays: modeled four-legged horses with neck/muzzle/ears, saddle,
  reins, hay racks, trough water, tack and amber lanterns. Entering cuts the roof
  away for the staged aisle, rather than replacing any of the eight existing
  Building paintings.
- Fenced lists: two mounted knights, lances, spectators, a herald, colored
  pennants and pass/score lamps; choices move competitors and results raise the
  result standard. Both places share a warm exterior lane with cottages/well.
  The existing ordinary court receives only additive lanterns, benches/hay and
  village bunting. The old road sweep stays clear.
- Four bounded camera stations per interior. They are discrete inspection
  positions along connected clear geometry, **not** an unbounded free-roaming
  stable collision world. Game choices are turn-based, not animated jousting.
- One cached Dotmax picture, keyed by full practice state, place, inside/outside,
  station, dimensions, quantized finite camera inputs and motion mode. Maximum
  256×80 terminal cells / 512×320 sampled pixels. No idle prop animation. Off and
  Reduced are deterministic; hidden surfaces don't compose. Zero and tiny sizes
  are bounded, with text/chrome taking precedence in very small Delve overlays.
- Home/yard practice props are on the live figure layer, not static scenery.
  Native transport keys include the owner projection, so same-tick selection or
  tending cannot reuse a stale image. Delve practice uses braille on all
  terminals and never queues a Kitty/other native image worker under its UI.
- Host-only. Commands/world practice visits are explicitly refused while joined
  to a remote Delve or while a host invitation is open. The guest protocol does
  not implement shared practice. `Run.chivalry` is not serialized, and the guest
  painter receives a **sanitized clone** as well (serialization alone would not
  protect PNGs). Practice notices never enter broadcast notice/HUD/chorus.

## Verification and captures

Feature tests live in `tests/cockpit/drive/chivalry__tests.rs`,
`tests/cockpit/app/dungeon_chivalry__tests.rs` and
`tests/cockpit/world_viz/chivalry__tests.rs`, plus targeted existing guest and
native cache test modules. They cover progression, invalid/repeated actions,
old-save defaults, owner/save-failure boundaries, no rewards, floor/control
preservation, guest JSON **and painter** isolation, connected geometry,
motion/cache transitions, zero/tiny sizes and live surface routing.

`ANGEL_CHIVALRY_REVIEW=<directory>` makes the ordinary tests retain deterministic
normal-world Dotmax raw/braille pictures, real `ui::draw` mini-world and Delve
cell dumps, and native home/yard before/after frames. These do **not** depend on
opt-in illustration fixture modes. PNGs/cell dumps use explicit deterministic
states and Motion Off; they are render evidence, not proof of a physical terminal
image-protocol session. The village before image removes only the additive
mesh dressing from the same ordinary court scene, keeping the camera identical.

### Malformed practice saves

The optional practice subtree is decoded independently. Missing or malformed
practice data resets only the stable/tournament state; treasury, home, wishes and
Delve progression remain intact when the realm is next saved. This is not a
repair mechanism for syntactically invalid JSON or corruption elsewhere in a
realm save.
