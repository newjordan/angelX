# Candidate review

## Pieces in reading order

1. North barred door: strong readable front elevation; rock, timber, iron, copper and malachite match the biome.
2. North open door: passage is clear and shares the same connected rock-and-timber frame.
3. North half-open door: recognizable transition with a barred panel swung aside; panel has visible perspective/diagonal edges against the strict axis-aligned requirement.
4. West barred door: posts are visible in the opening, but the surrounding timber/rock reads with slanted geometry and is not a clean axis-aligned cap strip.
5. West open door: opening is clear; wall cap and framing still include slanted geometry.
6. East barred door: top-down bars are visible, but the wall geometry is not fully axis-aligned.
7. East open door: gap reads as open; same slanted wall geometry issue.
8. South barred door: not a true south top-down cap strip; reads too much like a front-facing doorway.
9. South open door: not a true south cap strip/open gap; reads like a front-facing passage.
10. Stairs down: north-descending stairwell reads clearly, with axis-aligned steps.
11. Boss seal door: ornate barred front elevation, distinct and biome-consistent.
12. Floor seal marker: compact copper/malachite seal on rock; restrained center highlight.

## Weaknesses and next changes

The generation tool returned 1254x1254 rather than 1536x1536. The final crop measurement found 10 isolated connected components instead of 12; some intended cells merged or failed to separate under the alpha-mask crop check. The final candidate has transparent pixels, but its alpha channel contains 256 distinct values rather than only 0 and 255, so it also fails the hard-alpha requirement. The reported fully opaque fraction is 0.0014 and there is no no-transparency warning, but piece count and requested canvas size fail. Side-door geometry remains inconsistent with the strict axis-aligned specification, especially the south states. This candidate is not accepted. A future run should make each side/south door a compact, independent horizontal/vertical cap-strip glyph with generous gutters and no connecting diagonals, then verify the crop count and dimensions before acceptance.
