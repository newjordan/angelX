# The Mage's spells

Strips for the weave's spells (`rules::weave`), drawn by
`render/overworld/arena/spellfx.rs`. Rendered from
[pixelfx](https://github.com/patch-gg/pixelfx) (commit `fe7103c`; MIT, Copyright
(c) 2026 Patch and Copyright (c) 2026 Hugo Duprez, whose Sprite Fusion effects
some of them adapt) by `node scripts/delve/spellfx.mjs [pixelfx checkout]`.

Each spell is played at 15 frames a second (two ticks a frame) and sized to its
reach in the game. Every pixel is snapped to an existing ink and is opaque:
there is no dithered glow. A spell cast along the aim (Scald, Wildfire) has
eight rows, east first and then clockwise. `spellfx.json` gives each strip's
frame size, the floor point it stands on, and its frame count.
`weave::SHOWN` must stay two ticks for every frame; a test checks it.

| Strip | pixelfx effect | Spell |
|---|---|---|
| sunfall | fire/meteor | Sunfall |
| hoarfrost | ice/frost-nova | Hoarfrost |
| thunderhead | lightning/lightning-strike | Thunderhead, one on each foe struck |
| scald | ice/frost-breath, steam greys | Scald |
| wildfire | fire/flamethrower | Wildfire |
| frost-ring | ice/snowflake-burst | Frost Ring |
| sleet | ice/blizzard-vortex | Sleet's field, looping |
| static-ward | lightning/thunderclap | Static Ward |
| cataclysm | explosions/plasma-explosion | Cataclysm |
| burn | fire/fire-ring | a Mage's fire on the floor |
