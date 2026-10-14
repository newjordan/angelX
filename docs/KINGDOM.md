# The walking kingdom

## Architecture decision

Use the mini-viz realm itself as the continuous surface, at its native
sixteen pixels per tile. It is 112 by 121 tiles (224 by 242 simulation
units). A new home-floor Kingdom map carries its collision; walking across
the old overview screen boundaries never calls room entry, rebuilds a
floor, or moves another knight. The overview and walking landscape share
the same authored geography, castle positions, College and wish lots.

Stitching the 24 by 14 Delve rooms would preserve their grid but produce a
different kingdom: the model castles and most overview landmarks have no
place in that grid, and its College is deliberately in a remote reserved
cell. Instead those rooms remain destinations behind surface doors. This
keeps their games, NPCs, plots, construction, tower floors, ranch breeding,
and existing save masks. Ordinary countryside destinations are free to
enter. A return marker records the actual surface door and every knight's
previous surface position. Leaving a visit restores those positions,
including after checkpoint restoration. A newly joined knight returns at
the door. If new construction has occupied an old position, arrival moves
the knight to the nearest open tile. The underground settlement stays
byte-for-byte independent of the surface.

The simulation still advances at 30 ticks per second, with at most four
knights and the existing projectile/entity limits. Knights may separate
across the entire bounded surface; there is no party-distance tether.
Entering a separate map remains a party transition, as in the existing
Delve. Every seat renders around its own knight, including the host,
joined terminals and browser seats. Camera state belongs to each hero:
a 5 by 3 simulation-unit dead zone,
22 percent easing per tick (about 180 milliseconds), and an edge clamp.
The renderer has no shared camera state. The overview remains available
when the walking view closes.

Model castles and landmarks without a previous playable room open a
shared stone hall with the visited place's name. The King's Hall retains
its original construction requirement and mine-head entrance; the College
retains its full school and tower. Country transport plates still travel
directly between their authored visit rooms.

Click the overview, press P with it focused, or use `/world walk` to enter
the walking kingdom. WASD walks, E enters nearby doors, F fires, and Esc
returns to the overview. Starting a delve places the party outside its
gate. Starting overview walking places a new walker at the Keep; reopening
the same walking map keeps their position.

Landscape painting is shared and cached. The 1792 by 1936 landscape and
its terminal reduction ranks take about 28 MB total, shared across seats.
They are prepared at surface admission and rebuilt when built wishes
change. Roads and bridges connect every door; added buildings avoid the
original roads, water, landmarks and wish lots. Each countryside door is
built in its region (`overworld::kingdom::REGIONS`): a fixed patch of
ground, a spot for each place in it, and a cleared row (or open water)
where the region's name is lettered in grey. Every door gets a lane to the
nearest road, drawn as road, so the regions hang off one network. An open
plot in the town core (`OPEN_PLOT`, tiles 33 to 37 across and 35 to 39
down, between the Keep and the Village on the north road) is kept clear
for the King's court. The foreground adds twelve
road walkers, eight birds and nearby chimney smoke. A walking frame crops
one native 384 by 224 view and paints only visible live figures, ambient life and
short nearby door labels. It must never allocate a full-map live figure
layer or repaint the kingdom every tick.

Saves default missing camera and return fields. Legacy surface and College
visits gain a return door without moving the current party. Cellar saves
join the new surface when they emerge through the gate. Settlement cells
remain unchanged. Wish art travels in the durable world snapshot; ordinary
live packets carry the small return marker and each hero's own camera.

## Validation

Run from `cockpit/` with `CARGO_TARGET_DIR=/work/build-cache/angelx-integration`
and `TMPDIR=/work/tmp`.

The targeted suites cover continuous movement over more than one old
screen width without room/revision changes; independent, eased and clamped
cameras; every door's open arrival and return; held E; distant friend entry;
legacy surface, College and cellar saves; built wish lots; checkpoint
round trips; live mirrors; actual browser-seat PNG views; overview controls;
and existing countryside, school, ranch, barony and dungeon behavior.

The integrated results on October 12 are 850 passing `cargo test together`
tests (56 ignored), 378 passing `cargo test --bin angel world_viz` tests
(80 ignored), four passing `cargo test overworld_walk` tests, and 55 passing
`cargo test --bin angel app::control::dungeon::tests` tests (five ignored).
No tests are excluded. The College lamp is included in the resting realm's
light count. The shot writer passed separately with `--ignored`.
`cargo build --bin angel` passed for the production binary.

The College retains its room palettes and students. Its prompts stay in
the footer, with the courtyard exit naming the Kingdom when it returns
there. Kingdom entrance prompts also use a reserved opaque footer, painted
after world figures and edge markers. Building names and other landscape
labels never share the prompt's space. A regression test checks every
entrance while the camera moves, another knight crowds the footer, and
an offscreen knight adds an edge marker.

The save retry fixture now includes the College's 36 reserved journal
entries; previously it filled only the country's 104 entries while expecting
the combined 140-entry bound. The host return test now checks that a surface
statue wakes at its own position.

### Frame timing

The ignored `kingdom_frame_times` test alternates the existing gate room
and the kingdom, after cache warmup, for 499 samples at each size. These
are local unoptimized debug test-build measurements, excluding PNG encoding
and transport. The room's periodic torch repaint remains included.

| View | Room median | Kingdom median | Room p95 | Kingdom p95 |
|---|---:|---:|---:|---:|
| Browser 384 by 224 | 2.308 ms | 2.065 ms | 16.422 ms | 2.167 ms |
| Terminal 135 by 78 | 13.415 ms | 3.710 ms | 27.561 ms | 3.872 ms |

The kingdom simulation averaged 0.020 ms per tick over 3,000 ticks, well
within its 33.33 ms budget. Terminal frames use cached palette reduction
ranks with the same signal-priority and ink-coverage rules as room frames.
There is no render regression in this comparison. Initial atlas preparation
and a construction-driven atlas rebuild are admission/update costs, not
part of continuous camera travel.

### Render review

The ignored `write_kingdom_shots` test uses
`ANGEL_ARENA_SHOTS=/work/tmp/review-shots/kingdom-integrated` and writes nine frames from a real
collision-routed walk from the Keep through the southern model castles to
the College, its arrival frame, College/ranch/gate door and return sequences,
the College hall, the Artisan Quarter entrance, two distant seats, a terminal
frame and the full overview: 25 frames in total.
Every frame was opened for review. The ranch uses its authored red barn;
landmark silhouettes vary; nearby signs use short names; the College and
pocket exits identify the kingdom. The contact sheets are
`/work/tmp/review-shots/kingdom-integrated/contact-sheet-01.png` through
`contact-sheet-03.png`.

### Remaining boundaries

Separate-map entry is still a party transition. Independent simultaneous
interiors would require a different room simulation and replication model.
Model castles and new wish landmarks currently share the stone hall layout.
Walking terrain is the cached overview at dusk; live harness activity remains
visible in the overview.
