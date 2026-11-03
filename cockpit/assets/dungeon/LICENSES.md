# Delve audio: sources and licences

## Music (`music/`)

| Track | Source | Author | Licence |
|---|---|---|---|
| crypt | [Dark Shrine Loop](https://opengameart.org/content/dark-shrine-loop) | qubodup | CC0 |
| mines | "Level 2", [5 Chiptunes (Action)](https://opengameart.org/content/5-chiptunes-action) — Retro Game Music Pack | Juhani Junkala (SubspaceAudio) | CC0 |
| keep | "Level 3", [5 Chiptunes (Action)](https://opengameart.org/content/5-chiptunes-action) | Juhani Junkala (SubspaceAudio) | CC0 |
| boss | [Epic Boss Battle (Seamlessly Looping)](https://opengameart.org/content/boss-battle-music) | Juhani Junkala (SubspaceAudio) | CC0 |
| menu | [Dark Intro](https://opengameart.org/content/dark-intro) | Nikke | CC0 |
| sanctuary | generated for angelX with ElevenLabs music (`sound.py`) | — | per the operator's ElevenLabs plan |
| village-wakes | "The Village Wakes II", ACE-Step 1.5 Turbo (early_flutes_v1), generated for angelX on the operator's rig ("The Quiet Roads", 2026-10-15) | — | angelX's own, non-commercial research |
| village-green | "Dew on the Village Green", YuE2-3B, generated for angelX on the operator's rig ("The Quiet Roads", 2026-10-15) | — | angelX's own, non-commercial research |
| quiet-hills | "Seven Quiet Hills", MiniMax Music 3, generated for angelX on the operator's rig ("The Quiet Roads", 2026-10-15) | — | angelX's own, non-commercial research |
| bluebells | "Bluebells beneath the Oaks", MiniMax Music 3, generated for angelX on the operator's rig ("The Quiet Roads", 2026-10-15) | — | angelX's own, non-commercial research |
| heather | "Paths through the Heather", MiniMax Music 3, generated for angelX on the operator's rig ("The Quiet Roads", 2026-10-15) | — | angelX's own, non-commercial research |
| pear-tree | "The Pear Tree in Blossom", MiniMax Music 3, generated for angelX on the operator's rig ("The Quiet Roads", 2026-10-15) | — | angelX's own, non-commercial research |
| candles | "Candles for the Wanderer", MiniMax Music 3, generated for angelX on the operator's rig ("The Quiet Roads", 2026-10-15) | — | angelX's own, non-commercial research |
| last-light | "The Last Light in the Inn", MiniMax Music 3, generated for angelX on the operator's rig ("The Quiet Roads", 2026-10-15) | — | angelX's own, non-commercial research |
| fireside-cup | "A Cup beside the Fire", MiniMax Music 3, generated for angelX on the operator's rig ("The Quiet Roads", 2026-10-15) | — | angelX's own, non-commercial research |
| small-hours | "The Small Hours", MiniMax Music 3, generated for angelX on the operator's rig ("The Quiet Roads", 2026-10-15) | — | angelX's own, non-commercial research |
| library-rain | "Rain on the Library Roof", MiniMax Music 3, generated for angelX on the operator's rig ("The Quiet Roads", 2026-10-15) | — | angelX's own, non-commercial research |
| keepers-cottage | "The Old Keeper's Cottage", MiniMax Music 3, generated for angelX on the operator's rig ("The Quiet Roads", 2026-10-15) | — | angelX's own, non-commercial research |
| roadside-lantern | "A Lantern beside the Road", YuE2-3B, generated for angelX on the operator's rig ("The Quiet Roads", 2026-10-15) | — | angelX's own, non-commercial research |

CC0 needs no credit; the authors are thanked here anyway. Licences were read
on each OpenGameArt page on 2026-10-02. The tracks were converted to mono
MP3, trimmed and loudness-matched (`pack.py`, `level.py`). The Quiet Roads tracks are
stereo 112 kbps MP3 at -20 LUFS, from their 48 kHz masters.

## Sound effects (`sfx/`)

The first 27 sound effects were generated for angelX with the ElevenLabs
sound-effects API (`sound.py`), then trimmed and levelled (`level.py`).
`wheel_spin` and `wheel_land` are synthesized from scratch by `synth.py`
(numpy: pegged clicks timed to the wheel's own ease-out, and an inharmonic
bell), levelled the same way; they are part of angelX and carry its licence.

The spectacle's effects (`knight_hit`, the footsteps, the guardians' roars, the
hand and the hell gate) and the finishers' (`gore_kick`, `gore_thunk`,
`gore_chop`, `gore_splat`, `gore_bones`, `gore_burst`) are synthesized from
nothing by `synth_sfx.py` (numpy and scipy: noise bursts, filtered sweeps and
decaying sine modes, every source seeded), encoded with ffmpeg and levelled
as the rest. No recording or sound pack is in them; they are part of angelX
and carry its licence.

## The knights' voices (`sfx/knights/`)

The knights' grunts and their few words of made-up chatter were generated
for angelX with ElevenLabs text-to-speech (`knights.py`), per the operator's
ElevenLabs plan, in four library voices (Ember, Guy, Jessica and Callum),
then trimmed, lifted a touch in pitch and levelled by the same script.

## Art packs (`packs/`)

| Pack | Source | Author | Licence |
|---|---|---|---|
| demonic-dungeon (v2.3.3, the no-background sheets) | [Demonic Dungeon](https://v3x3d.itch.io/demonic-dungeon) | VEXED; palette Mystic-16 by Polyphrog | CC BY 4.0, bought by the operator 2026-10-30 |
| Mage spell strips (`crates/delve/assets/spellfx/`, rendered, not a pack) | [pixelfx](https://github.com/patch-gg/pixelfx) | Patch; Hugo Duprez (Sprite Fusion effects) | MIT |
