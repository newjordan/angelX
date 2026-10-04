# Live mini-world adventures

The default Realm pane follows an active quest into a native 3D scene. The
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
