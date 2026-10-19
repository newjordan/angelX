# The fourth wall: things that float in front of the terminal

The operator's words, from the sessions of 2026-10-16:

- "we can have nice, cool looking 3D buttons ... that glow, on the terminal, whatever we want"
- "make the buttons 3D rotating objects, and add a glowing border, break the fourth wall and UI system"
- "i want the shapes to float up and out organically on their own, and ethereally float on terminal space, no borders, gently over the other windows, thats the whole point"
- "change the coin that flies up to a nice little gold coin, and on every million flip up that many coins"
- "a 3D scroll that rolls down and back up, with proper physics, completely flying on top of the ui and looking gorgeous ... use the local trellis model and export the proper assets, then animate them"

Several efforts built toward that on 2026-10-16/17. This is where each one landed, how they fit together, and what was fixed on the way in (`synth/fourth-wall`, 2026-10-17).

## What came in

| Piece | From | Where it lives now |
|---|---|---|
| **Object dock.** The mini-viz's seven actions as turning solids that float up and out of the pane, each a different shape. The expand arrow reveals them; a click on the solid itself runs its action, and the empty space between solids falls through. Labels stay text, paging keeps all seven reachable, and reduced motion snaps. | angel, main tree | `cockpit/src/ui/object_dock.rs`. Its solids are `LayerKind::Primitive` layers, ray-marched by the window. |
| **Turning solids as a layer kind.** Cube, sphere, pyramid, cylinder, cone, torus and octahedron, lit, with a rim and a turn. A host that only draws quads gets a bounded projection. | angel, main tree | `angel-stage` (`LayerKind::Primitive`, `primitive.rs`), `angel-gpu` (kinds 16 to 22 in `glass.wgsl`, one instance per solid). |
| **Gold coins.** One toss per whole million input tokens, staggered, at most eight live. A milestone during a toss extends the burst rather than restarting it. | angel, main tree | `cockpit/src/ui/viz/spend_viz.rs`. In the window, coins are cached flip frames placed over the cockpit. |
| **The `/setup` scroll.** A foreground paper that unrolls with fixed 1/120 s physics. The rollers turn by released length over radius, and it reverses mid-motion. Its text is native atlas glyphs bent with the paper. The rollers are the TRELLIS mesh exported on Spark. | angel, main tree | glass: `scroll.rs`, `scroll_mesh.rs`, `render_scroll*.rs`. `angel-gpu`: parchment (kind 11) and mesh triangles (kind 12). Cockpit: `setup_menu.rs`, `views/setup_view.rs`. The roller texture is an `angel-gpu` picture. |
| **Floor pickups as solids.** Dropped cards and runes as lit solids with a contact shadow; a rune keeps its sign. | main tree | `delve`: `cards::pickup_image`, `runes::solid_orb`, `RoomScene::solids`. Drawn beside the play line's 3D pickups (below). |
| **The Orrery.** The live agent graph as a leaning holo-panel in the window's top right, on its own thread. While no graph runs it holds a gold sun and orbits. `ANGEL_GLASS_ORRERY=1`. | GLM, `feat/fourth-wall` | `cockpit/src/ui/orrery.rs`, on dev. |
| **The Stereopticon.** The artifacts pane's videos play as a glass plate in the window, with a progress track, pause bars and an ended replay that restarts. | GLM, `feat/fourth-wall` | `cockpit/src/ui/stereopticon.rs`, on dev. |
| **Documents as pages, video streams, the browser's alpha, the render scorecard.** | Muse, `render/kitty-scorecard` | `present.rs`, `angel_glass::page`, the web shell, `docs/RENDER_SCORECARD.md`, on dev. |
| **Transcript links.** Underlined links in the transcript open on click-release and never on a drag. | angel, main tree | `cockpit/src/ui/transcript_links.rs`. |
| **The flash fix.** A paced wake-up that drew nothing no longer sweeps the displayed stage away. | angel, main tree | `cockpit/src/ui/frame_lifecycle.rs`. |
| **The mini-viz click fix.** Only the dungeon's sprite on the map is the Delve's entrance, not the whole pane, and visible controls get their clicks first. | angel, main tree | `dungeon_intro.rs` (`delve_map_hit`), `turn_io.rs`. |

## How the 3D paths fit together

There are now several ways something 3D reaches the window. Each has its place:

1. **`angel-gpu` scenes** (`docs/GLASS_CORE.md`): meshes, depth, a camera and the world's look. They draw the Delve's shots and pickups as props, the cube and the shore.
2. **Ray-marched solids** (`LayerKind::Primitive`): one quad each, shaded in the fragment shader. They draw the dock's buttons, and the pickups on a stage that draws no props.
3. **The scroll's paper and rollers**: instances in the window's own pass, drawn over everything the cockpit drew.
4. **Leaned 2D panels** (the Orrery, the Stereopticon): layers turned together about a panel's centre.
5. **The CPU rasterizer**: the terminal's own path, unchanged.

A floor pickup shows how they stack. A page that draws props gets the play line's meshes. A frame handed out in parts to a page that draws none gets one ray-marched solid. A painted frame stamps the same solid, projected once.

## Fixed on the way in

- **Leaned panels grew.** `orrery::tilted_rect` placed the turned rectangle's bounding box and turned that too, so every leaned quad grew by its lean. It also turned cell rects in cell units, where a cell is 12 × 25 px. A 40-column Stereopticon plate came out about two rows taller and ran over its pane's path line and controls. Now a leaned quad keeps its size, and its centre turns about the panel's in pixels (`tilted_rect_in`).
- **The Stereopticon's plate leaves its pane its lines.** The screen is fit inside the pane less the plate's own reach, the brackets and the lean (`room_for_plate`). The path above and the controls below stay readable and clickable.
- **The Orrery stays in the window.** It is anchored two cells in from the right and below the header's three rows. It no longer runs off the edge or over the session's name.
- **Scroll clicks landed on the wrong item.** Both glass's window and the cockpit's `setup_mouse` mapped the pointer through the curled page. Each mapping took off the paper's top offset (about 1.2 rows on a 30-row scroll), so a click chose an item a row or two above the one under it. Now only the cockpit maps it.
- **The scroll's key bar was under the roller.** The page put its key bar at `height - 4`, where the paper already curls under the lower roller (from about 87% of its height). It now sits two rows above `scroll::readable_rows`.
- **The dock replaced rows it shouldn't.**
  - It stood in for every world-pane footer, so an arrival floated all seven actions, and Enter floated where there was no interior.
  - It now floats exactly the actions of the row it stands in for: the ordinary world's or an arrival's, before any were trimmed for width.
  - A walk's, a lesson's or a catalog's row keeps its own footer.
  - Under 40 columns, where Back would sit pages away, the pane keeps its classic row.
- **Tests follow the floating world actions.** The world's row of bracketed actions now floats from the dock's handle. The tests that read `[Library]`, `[Back]`, `[Enter]` or `[Map]` off the world's footer now open the dock and page to the action (`float_dock_action`), or ask the dock whether it carries the action. The PTY smokes see the Realm by its pixels and the dock's handle. The Delve test no longer expects a click on the middle of the mini-viz to open the Delve; only the gate on the map does.
- **Ported onto the split renderer.** The scroll and the primitives were written against the glass from before `angel-gpu` existed. Kind 4 is now the premultiplied picture, so the primitive kinds moved to 16 to 22. The rollers' texture rides the picture store, handed over again if the budget evicted it.
- **One animation predicate.** `Layer::animating` is shared by the window and the stages, replacing the inline copies the render-present handoff flagged.
- **Pickups, both ways.** The main tree's solid pickups would have replaced the play line's 3D props; they now sit beside them as the fallback. The old card plate stays as the source the 3D props take their colours from.
- **Tests that look at frames in parts.** The parts switch is per test thread under `cfg(test)`, so one test's parts no longer leak into another's painted frame.

## Not done

- The Orrery has no command yet: it is `ANGEL_GLASS_ORRERY=1` only, with no "show while a graph runs" policy.
- The scroll still draws its rollers in painter's order inside the window's pass, not through an `angel-gpu` scene with depth. Moving it there would let the scroll share the scenes' light and the world's look.
- The coins are flip frames, not a mesh.
