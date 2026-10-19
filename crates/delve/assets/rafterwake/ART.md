# Rafterwake art

One native pixel grain. Knights occupy 48x48 cells, the keeper 64x80, effects
48x32, and scaffold props 24x10 / 20x36 / 24x18 / 14x36. Nothing scales in play.
The 74px keeper is 1.72 times the 43px knight; the stage is 254x144 native pixels.
The play area occupies 66% of frame width; the knight occupies 30% of stage height.

Oak uses I/r/o/t; iron uses x/S/U/v from the existing ink palette. Each material
has four hue-shifted tones. Seat trim and plumes use four-tone seat ramps baked
with the complete body poses. An additional one-pixel seat contour separates
YOU from the keeper. All mirroring, damage seams, glyphs, scenery and recolours
are cached. There is no per-pixel floating-point work on the frame path.

The chain-drop has an overhead arm tell and a marked vertical landing column.
The sweep winds an arm behind the body and marks a low horizontal floor band.
Their rule rectangles and rendered marks share the seat's camera translation.
Wind-ups are 33/29/25 ticks across the three stages, never less than 0.75 seconds.
Falling bricks have their own 30-tick column tell. Impacts use solid body layers.
The contact puff, chain crown and low wedge each have four native frames.

The native recipe is authoritative; the PNGs are reviewable copies. Reproduce:
`python scripts/rafterwake-art.py SOURCE_DIRECTORY REVIEW_DIRECTORY`
Sources are keeper.png, knight.png and effects.png in the task's source folder.
Requests and source images: /work/artifacts/boss-art-20261015/game-5/source/.
The converter uses nearest sampling, keeps the principal silhouette, merges
small tone islands, places coloured contours and authors curved 3x2 lens lights.
It runs the shared singleton and signature gates. Minimum floor edge 15.6 L*;
idle head/body 24.0 L*. Source texture, cross-cell fragments, floating defeat
lenses, rectangular climb hands and the first clipped starting floor were rejected.

Review combines native room states, 1x/3x character/prop/effect sheets and actual
96x28 cockpit buffers. `scripts/rafterwake-review.py` composes those captures.
Originality review: a pulley-headed oak hoist with a small bell chest; no borrowed
character, name, map, music or recognizable commercial silhouette.
