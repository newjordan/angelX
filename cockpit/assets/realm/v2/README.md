# Authored interior plates

The outdoor backdrop, facade and actor PNGs are retired. All `interiors/`
room-shell and station graphics and source images are retained unchanged.
Scriptorium keeps its whole approved `../scriptorium-baseline*` plate.

Every station was generated as top-down-oblique SNES pixel art on a flat
`#ff00ff` key, without room architecture, characters, readable text, UI, or
watermarks.

Process:

```sh
python3 scripts/realm-palette.py \
  --quantize cockpit/assets/realm/v2/realm-backdrop-source.png \
  cockpit/assets/realm/v2/realm-backdrop.png \
  --cover 256x224 --no-signal

python3 scripts/realm-assets.py validate \
  cockpit/assets/realm/v2/landmark-*.png \
  cockpit/assets/realm/v2/knight-*.png
```
