# Wished casts

Strips for every wished shape and element (`rules::zones`), drawn by
`render/overworld/arena/wishfx.rs`. Rendered from
[pixelfx](https://github.com/patch-gg/pixelfx) (commit `fe7103c`; MIT, Copyright
(c) 2026 Patch and Copyright (c) 2026 Hugo Duprez, whose Sprite Fusion effects
some of them adapt) by `node scripts/delve/wishfx.mjs [pixelfx checkout]`.

Each cast is played at 15 frames a second (two ticks a frame) and sized to the
level-2 reach of its shape (8 pixels a world unit). Levels 1 and 3 are the same
strip sampled to that size's reach. Every pixel is snapped to an existing ink
and is opaque. A cast along the aim (wave, cone, line) has eight rows, east
first and then clockwise. `wishfx.json` gives each strip's frame size, the floor
point it stands on, its frame count, its bake reach and whether it loops.

| Strip | pixelfx effect | Cast |
|---|---|---|
| burst-fire | explosions/big-explosion | Burst of fire |
| burst-frost | ice/ice-shatter | Burst of frost |
| burst-storm | lightning/thunderclap | Burst of storm |
| burst-poison | nature/spore-burst | Burst of poison |
| burst-holy | holy/sunburst | Burst of holy |
| burst-arcane | arcane/mana-burst | Burst of arcane |
| burst-earth | explosions/dirt-explosion | Burst of earth |
| cone-fire | fire/fire-breath | Cone of fire |
| cone-frost | ice/frost-breath | Cone of frost |
| cone-storm | fire/fire-breath, electric ramp | Cone of storm |
| cone-poison | fire/fire-breath, poison ramp | Cone of poison |
| cone-holy | fire/fire-breath, holy ramp | Cone of holy |
| cone-arcane | ice/frost-breath, arcane ramp | Cone of arcane |
| cone-earth | fire/fire-breath, dust ramp | Cone of earth |
| line-fire | fire/flamethrower | Line of fire |
| line-frost | ice/ice-lance | Line of frost |
| line-storm | lightning/chain-lightning | Line of storm |
| line-poison | water/water-jet, poison ramp | Line of poison |
| line-holy | arcane/arcane-beam, holy ramp | Line of holy |
| line-arcane | arcane/arcane-beam | Line of arcane |
| line-earth | nature/vine-whip | Line of earth (vine-whip; earth-spike reads as a pillar, not a line) |
| field-fire | fire/fire-ring | Field of fire |
| field-frost | ice/blizzard-vortex | Field of frost |
| field-storm | lightning/storm-cloud | Field of storm |
| field-poison | nature/poison-cloud | Field of poison |
| field-holy | holy/heal-aoe | Field of holy |
| field-arcane | arcane/rune-circle | Field of arcane |
| field-earth | nature/quake | Field of earth |
| pillar-fire | fire/flame-pillar | Pillar of fire |
| pillar-frost | ice/ice-spikes | Pillar of frost |
| pillar-storm | lightning/lightning-strike | Pillar of storm |
| pillar-poison | water/geyser, poison ramp | Pillar of poison |
| pillar-holy | holy/holy-pillar | Pillar of holy |
| pillar-arcane | arcane/star-fall | Pillar of arcane |
| pillar-earth | nature/earth-spike | Pillar of earth |
| nova-fire | explosions/plasma-explosion, fire ramp | Nova of fire |
| nova-frost | ice/frost-nova | Nova of frost |
| nova-storm | lightning/thunderclap | Nova of storm |
| nova-poison | ice/frost-nova, poison ramp | Nova of poison |
| nova-holy | holy/smite | Nova of holy |
| nova-arcane | shadow/dark-explosion, arcane ramp | Nova of arcane |
| nova-earth | impacts/ground-slam | Nova of earth |
| wave-fire | water/wave-crash, lava ramp | Wave of fire |
| wave-frost | water/wave-crash, ice ramp | Wave of frost |
| wave-storm | water/wave-crash, electric ramp | Wave of storm |
| wave-poison | water/wave-crash, poison ramp | Wave of poison |
| wave-holy | water/wave-crash, holy ramp | Wave of holy |
| wave-arcane | water/wave-crash, arcane ramp | Wave of arcane |
| wave-earth | nature/quake | Wave of earth |
