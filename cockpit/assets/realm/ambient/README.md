# Ambient work locations

`/world view ambient` (or `ANGEL_WORLD_VIEW=ambient`) shows still location art
following the current tool destination. New tool traffic changes the art and
caption together; entering a room uses the existing world lifecycle and does
not pin its art against later work.
`/world view top`, `3d`, and `raycast` retain their existing cameras. Presented
reports, images, and videos keep foreground priority.

Eight original images use the Realm material palette: dark slate, umber timber,
muted olive and warm ochre. The artwork contains no state beacons or readable
claims. Each admitted PNG is 256×224, opaque, and uses only palette banks 0–3;
bank 4 remains reserved for actual state. Full-resolution generated originals,
prompts, provenance and admission hashes are retained in
`docs/plans/release-20260907/miniviz-art/`.

The code lazily retains one decoded plate per location (1.75 MiB for all eight),
uses the existing resident image worker for graphics or ordinary-terminal half
blocks, and keeps a cached braille rendition visible while the image is prepared. The still scene has no animation clock or moving camera.

Admission uses the existing build-time palette tool:

```sh
python3 scripts/realm-palette.py --quantize SOURCE DEST --cover 256x224 --no-signal
```
