# Village construction and machinery

Eleven full-resolution generated source masters supply 27 asset groups and 49 frames. The palette follows [the village target](../concepts/village-vision.png): terracotta, cream limestone, ochre timber, blue slate, blue-and-gold heraldry and restrained teal accents. The retained [brickworks concept](brickworks-quarter-concept.png) establishes the extraction → drying → firing → construction district.

The exact source rectangles, local ground anchors, shared reference sizes, source dimensions and SHA-256 checksums are in [manifest.json](manifest.json). Complete generation prompts and original output locations are in [prompts.json](prompts.json). Source masters remain separate from the derived runtime banks and review captures.

| Source | Contents |
| --- | --- |
| brickworks-modules-atlas.png | Clay pit, brickworks, kiln housing, masonry yard |
| brick-cottage-stages-atlas.png | Footings, half walls, roof scaffold, finished cottage |
| brick-walls-gates-atlas.png | Straight wall and closed/open gate |
| bridges-connectors-atlas.png | Horizontal/vertical timber bridges, post, vertical wall |
| brick-corners-atlas.png | Four screen-axis wall corners |
| brick-junctions-atlas.png | Four three-arm wall junctions |
| walls-gate-stages-atlas.png | Four wall and four gate construction stages |
| power-housings-atlas.png | Fixed waterwheel and line-shaft housings |
| rotors-animation-atlas.png | Four wheel rotations and four shaft/crank rotations |
| fire-press-animation-atlas.png | Four kiln flames and four brick press strokes |

`village_art_build.rs` compiles structure banks at 48, 64, 96 and 128 reference pixels. It also reads the separately owned [citizen manifest](../citizens/manifest.json), producing 32px and 48px body-height banks. A group uses one common scale and anchor-relative union canvas across its frames. Foundations stay low, tools can rise beyond an actor's head, and changing poses cannot resize the actor. Resize filtering uses premultiplied colour to avoid opaque halos around transparent edges.

`village_art.rs` accepts a ground contact `(x,y)`. Structure `size` is its reference span; citizen size is body height. The animation clock uses the game's authoritative `rules::HZ`. Walking takes priority on a citizen's interpolated arrival tick. An idle wheel, shaft or press retains frame zero; kiln fire appears only while active. These are four separately generated moving images composited over unchanged housings.

The runtime API exposes `paint_module`, `paint_construction`, `paint_wall` (N1/E2/S4/W8), `paint_gate`, `paint_bridge`, `paint_blueprint` and `paint_citizen`. It returns false for missing art so callers can retain their normal fallback. Cottage, wall and gate have authored construction sequences; other construction plans retain the caller's site marker. Wall and gate stages use the same tile-sized span as their finished modules. The catalogue and map renderer consume the brick structures, bridges, waterwheel and drive shafts. Brickworks press art remains available for a later production module.

The [review folder](review/) contains captures from the actual sprite compiler and painter in a standalone fixture, including a two-second machinery/citizen loop. They demonstrate source cropping, pose alignment, stable building housings and visible animation. They are art/render evidence, not proof of production economics or a completed player campaign. Actual integrated game captures are indexed separately under [kingdom works](https://github.com/newjordan/angelX/blob/main/docs/images/kingdom-works/README.md).
