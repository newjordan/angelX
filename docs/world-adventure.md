# Live mini-world adventures

## The crawl (the default)

While a `/loop` runs, the Realm pane is a first-person dungeon crawl through
a floor of the [Delve](DELVE.md), in the old way (Eye of the Beholder,
Dungeon Master): a step at a time, quarter turns at the corners, a torch in
the left hand, a sword in the right, and the dark. Two lines under the
picture say what's happening. When the knight steps up to the quintain and
the pane follows into the loop's own view, that view is the crawl too, with
the loop's numbers (iterations, time, tokens) kept under it.

- **Where:** the loop decides. A coding or competition loop is down the
  Mines, research is in the Fungal Deep, a stalled loop wades the Drowned
  Archive, and a submission being judged stands in Dragon Keep. Floors are
  the Delve's own layouts, each room set in rock with corridors cut between.
- **The walk:** each turn of the loop walks the party on to the next hall,
  and after the stairs hall, down a stair to a new floor.
- **In a hall,** what the agent's tools are doing is what the party does:

  | Tools | The party |
  | --- | --- |
  | Editing, building | Fights the hall's monsters (the guardian at the stairs) |
  | Reading, research | Studies at a lectern, a tome open on one of the floor's monsters |
  | Asking, planning | Has a word with one of the realm's folk (Tobbin, Merlin, Maud, Sir Kay and others) |
  | Dispatching | Scouts the dark |
  | Nothing in flight | Rests by a campfire that lights the hall |

- **Guardian fights:** a competition submission settling brings the floor's
  guardian out of the stone, and the crawl stops for it. The fight goes back
  and forth like the old games: the guardian's health runs down a bar across
  the top, it rallies and its wounds close, the party gives ground a cell at
  a time with the knights' own health bars dipping in the corner, then
  presses again.
  - **A promotion** (accepted as an improvement: a record beaten) is a fight
    the party wins. The guardian rages, staggers and sinks, and the party
    stands in gold light. Three minutes, to sit back and enjoy.
  - **A submission that settles without one** (rejected, timed out, or no
    record) is a retreat. The guardian comes back whole and the party backs
    away down the hall, shields up. Two minutes.
  - A result still waiting on its promotion brings nothing yet, and one
    already there when the session starts is history, not news.
  - The fight keeps the pane for its length even if the loop is home by
    then.

Like the expedition before it, the crawl is presentation only: time creates
no iterations, measurements or rewards, and a frame is a pure function of
the loop's state and its clock. `ANGEL_LOOP_VIEW=expedition` brings back the
expedition walk described below, and the quintain's trench run. The wall, door, floor and vault textures,
the hands and the props were painted from a brief; the notes and
rebuild tools are in `cockpit/assets/crawl/`.

## The expedition

With `ANGEL_LOOP_VIEW=expedition`, the Realm pane follows an active quest into a native 3D scene. The
Mines use a roofed timber gallery with rails, torch pools, crystal workings,
and a side stairwell reaching 2.6 world units below the main floor. Camera
panning reveals that depth; the side rooms are scenery, not independent
player-controlled levels. The [School of Magic](world-school.md) has explicit
study and underground archive locations.

A real loop iteration starts a short walk between authored camera stations.
After arriving, the camera observes the current work. In-flight read/research
tools give the knight a book; write/build activity gives him a hammer and
field anvil. Returned tools clear that pose. The cast follows the actual party
size, displaying at most three members. Solo work shows one knight.

Region, danger, submissions, and homecoming still come from the existing quest
events. Animation time supplies movement only: it creates no iterations,
measurements, success receipts, treasure, or model calls. The mine scene no
longer stamps mounted horse sprites over its interior.

## Controls

Focus Scryglass with an empty composer to use the camera keys:

| Control | Effect |
| --- | --- |
| Arrow keys or `h j k l` | Look around |
| `+` / `-` | Zoom |
| `0` or **Follow** | Reset the camera |
| `m` or **Map** | Show the current region on the map |
| `m` or **Explore** from that map | Return to the live journey |
| Mouse drag / wheel | Look / zoom |
| `/world visit school` | Visit the school grounds |
| `/world follow` | Leave an explicit visit and follow work again |

Explicit visits, entered rooms, and selected non-world routes retain their
operator choice. School entry uses **Enter**; a Mines map does not offer entry
to an unrelated town building. `r` on Realm retains the Research shortcut.
Existing global stop behavior is unchanged.

## Rendering and review

Animation uses ten samples per second on the existing world clock. During an
active turn the visual lane reuses frames for eight world ticks, limiting
recompositions to five per second. Reduced motion and motion off use a settled
camera and fixed animation phase while still reflecting actual event changes.
Region meshes use the existing bounded cache; only the small cast is rebuilt
for a frame. Drawing performs no file, network, or model work.

The ignored production exporter creates a 48-frame travel/read/write sequence,
96×72 raw mini frames, 48×18-cell braille previews, every mine station including
the station wrap, panned views, and the other adventure regions:

```sh
ANGEL_EXPEDITION_REVIEW=/tmp/angel-adventure-review \
  cargo test --manifest-path cockpit/Cargo.toml \
  stage::world_viz::world3d::expedition::tests::write_expedition_review_sequence \
  -- --ignored --exact
```

These previews are labelled event fixtures, not recordings of a live model or
benchmark results. Review the mini braille outputs alongside enlarged frames:
side workings and crystals are intentionally subtle at that resolution.

The development review retains a 9.6-second fixture clip, second-round contact
sheet, and station/pan sheet under the ignored
`artifacts/world-adventure/20260930/` directory. The exporter above reproduces
the frames from a source release. These demonstrate rendering and event routing,
not live provider performance.
