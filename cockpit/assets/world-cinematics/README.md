# Cockpit world-cinematic assets

`location-atlas.png` is a terminal-sized deterministic derivative of
`../workshop/fantasy-loop-lab.png`. The ordinary cockpit miniviz uses eight
different focal regions as its first location set, then slowly tightens each
crop toward the relevant door or work surface.

The travel panorama is generated at runtime from the current workspace's real
procedural island. Its rider overlay reuses `../tourney/poses/approach.png` and
`../tourney/poses/canter.png`, alpha-composited into the lower-right corner of
every outdoor first-person plate (ride + settled vista) on the two-pose canter
cadence (`tick / 2`). Interiors omit the saddle overlay.

Regenerate the atlas without any image-generation service:

```sh
python3 scripts/process-world-cinematics.py
```
