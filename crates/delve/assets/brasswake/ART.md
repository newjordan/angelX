# Brasswake art contract

Source sheets: keeper, porter, knight and effects. Each source uses four columns
and three rows on a flat neutral background. The keeper's logical cell is 64px,
the porter and knight 48px, and effects 32px. All use the same one-pixel grain in
the room. One source scale per sheet retains physical size across raised arms,
walking, attacks and defeat. No runtime enlargement of bodies.

The source prompts specify: front three-quarter view; rounded, chunky readable
forms; four-tone hue-shifted ramps; a one-pixel coloured contour; no dithering,
near-white cores, black outlines, commercial likenesses, text or panel borders.
The knight faces right, the creatures left. The keeper is a copper pear-shaped
brewing vat with a small teal face, ladle arms and short wooden legs. The porter
wears a folded hood over an oak-cask body and has mitten fists and wooden boots.
The knight has slate armour, seat-coloured cloth and plume, fists and jointed legs.

The complete creature sets are idle, two walk poses, slam tell/contact, rush
tell/contact, flinch, two damage states, defeat and recovery. The knight set is
idle, two walk poses, jab, cross, kick, grab, throw, flinch, down, guard and rise.
Effects are four connected frames each of contact petals, a grounded slam crown
and a low curling steam front. Bread and casks are native-pixel props.

`scripts/brasswake-art.py` removes only background-connected neutral pixels,
keeps the largest silhouette, downsamples with nearest neighbour, snaps to at
most sixteen existing realm inks and removes isolated interior specks. It maps
the porter's hood to cool slate, then restores two shaped warm eye lenses with
dark one-pixel falloff. The renderer bakes dark-around-light damage seams,
mirrors, seat palettes, environment and large combo glyphs into OnceLock caches.
Terminal variants keep the same footprint, strengthen dark-around-light seams
and paired curved lenses, and give X3 a half-block-sized glyph in the top band.
The master ink palette remains unchanged. Impacts stay on the solid layer;
braille is reserved for atmosphere. Danger borders use the exact rules rectangle.

Rejected conversion one: neutral-colour removal punched holes in steel armour;
converted plum hood merged into oak; debris and scattered source highlights
became specks. Flood-fill extraction, the cool hood and local cleanup retain the
body shapes. Rejected unprotected tiny eyes: dark surrounds and shaped lenses
now survive the native sprite. Review both at 1x in-room and in actual cells.

Rebuild: `python scripts/brasswake-art.py SOURCE --review REVIEW/art`.
Review: `ANGEL_ARENA_SHOTS=REVIEW/rooms cargo test -p angel-delve
write_brasswake_review -- --ignored`, then the cockpit's
`write_brasswake_terminal_review` with `ANGEL_ART_SHOTS=REVIEW/terminal`.
Compose with `python scripts/brasswake-review.py REVIEW`.
