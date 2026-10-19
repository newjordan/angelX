# Boss flight readability

The seven delivered flight banks are prepared by `scripts/boss-effect-import.py`.
Source crops and their SHA-256 records stay unchanged. Collision, warning masks,
flight ticks, impacts and boss definitions remain authoritative.

`scripts/boss-effect-slots.json` owns native display sizes, core selection and
exposure. Flight exposure is at least 1.0. Value is lifted before reduction;
core and leading-rim pigments come from the full-resolution source, so a BOX
average cannot erase a small furnace glint. No flight stamp is room-graded.
The shared 75 world inks do not contain pink. Mother's delivered orange/green
flight drawing therefore uses the existing `projectile.png` spore hue as its
explicit colour reference. Its source silhouette, socket and two poses remain.

Each native flight frame has at most 12 source-derived local inks, including
two exact signal pigments. These do not expand the realm or knight palettes.
Source and prepared fidelity are both retained in `effect-import.json`; the
prepared comparison must pass the original import thresholds. Signal peaks are
bounded at 242, with no near-white core (minimum channel below 198). The importer
reproduces the local bank, and tests check hue, exposure, alpha and provenance.
Tell and impact preparation stays byte-exact.

The terminal keeps the seat's own predicted client picture and camera.
Only cells containing exact flight pigments change: their compact bright cores
use braille's finer dot grid, at full hue and exposure. The light rim remains
native detail; it cannot expand a dot into a solid half-cell band. Other cells
retain their existing picture sampling. The layered review painter uses the
same emission policy. PNG decode and quarter turns stay cached; native live
stamps perform integer copies. The terminal mask uses integer sampling.

Review at four seats in both kits, native size and actual 96x28 cells. Inspect
dense release arcs as well as separated travelling shots. Physically overlapping
shots and positions closer than the terminal dot pitch still share a mark.
Run `scripts/test-boss-effect-import.py`, normal Delve tests, the arcade cell
exporter and normal WASM builds; compare simulation hashes and smoke timing.
