# Town clerks

The archivist and courier are approved ImageGen originals, copied unchanged from
the 2026-09-25 modular town-asset exploration. Their exact generation prompts
are retained beside them as `archivist-prompt.txt` and `courier-prompt.txt`.

- `archivist.png`: the town's memory/archive keeper, stationed by the Scriptorium.
- `courier.png`: the town's delivery clerk, strolling a short town-road segment.

These residents are scenery. They do not claim that cache work or delivery is
running and do not start harness tasks, model calls, polling, or timers.

The default Realm overworld decodes them on its first composed frame, even if
the residents are offscreen. Its existing lazy image worker crops their
transparent margins and caches tiny
20-pixel-tall poses in the ordinary realm palette. The original PNGs stay intact.
Drawing uses the existing depth sorting, view culling, lighting, scene cache,
and paced frame tick. Reduced/off motion keeps both poses and positions still.
They do not add a redraw source or animate while the Realm is hidden.

Add future approved residents to the small `CLERKS` data array in
`cockpit/src/stage/world_viz/overworld/clerks.rs`; no actor engine is needed.
