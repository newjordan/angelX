# River and clay frontier

`river-clay-frontier-concept.png` is a visual-development painting for a working
medieval river village linked to dangerous mineral delves. It shows a timber
yard, clay pit, drying bricks, kiln, waterwheel, unfinished keep and returning
adventurers. It is a concept, not a screenshot of finished gameplay.

`ground-atlas.png` contains four overhead pixel-art ground textures in exact
627 × 627 quadrants:

| Runtime ID | Quadrant | Material |
|---|---|---|
| `ground-clay` | upper left | Damp terracotta clay bank |
| `ground-dry-clay` | upper right | Cracked drying clay and silt |
| `ground-ford` | lower left | Shallow blue-green water with a visible pebble bed |
| `ground-mud` | lower right | Trampled muddy cart-road surface |

Both images were generated with the built-in ImageGen tool. The complete
prompts and source checksums are retained in `concept-manifest.json`,
`concept-prompt.txt`, `manifest.json`, and `ground-prompt.txt`. No source pixels
were manually changed. The game build reads the texture rectangles from the
manifest and samples them through the existing frontier ground renderer.

The associated geography is implemented in `crates/kingdom/src/world.rs`:
72 × 54 plots, nine 24 × 18 parcels, seeded forest/stone/clay/river sites, safe
fords, persistent tree clearance, and production- or claim-earned expansion.
Older saves preserve the original 24 × 18 core as unrestricted meadow. The
village's terrain and resource rules are simulation data; this painting does
not replace them.
