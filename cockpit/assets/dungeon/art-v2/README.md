# Upgrade material and pose kit

Small local studies prepared from the user-selected UPGRADE references in
`/work/artifacts/art-concepts/upgrade`. Rebuild from the repository root with:

```sh
UV_CACHE_DIR=/work/build-cache/angelx-artv2/uv TMPDIR=/work/tmp \
  uv run --no-project --with Pillow python scripts/art-v2-kit.py
```

The extraction boxes and silhouette masks are checked into that script.
PNG samples decode once into transparent RGB sprites. The knight studies are
then brought into the hero the default kit draws: its cool steel, cream glint
and sky rim, the seat inks, and its gold-rimmed heater shield with a pale. Scenery is constructed
from the collision grid and settlement geography; a plate is never used as a
whole-scene backdrop. Tiles have quiet stone/wood/grass/gravel material windows;
actor studies use bounded keyed silhouettes with source-ground specks removed.

Gameplay figures use a 768×448 canvas, 32–44-sample knights, separate live aim
and equipment, contact shadows, gait offsets and independent lights. Horse and
boss studies establish anatomy and material separation. This is a prototype
pose kit: full eight-direction movement, compression/contact drawings, gear
variants and damage stages remain production conversion work. Refer to
`docs/ART-V2.md` for the accepted direction and conversion order.
