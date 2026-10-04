# Delve sound effects and music

Played by `cockpit/src/drive/together_audio.rs`. The game names a sound and
the mixer plays the file of that name. Each name is throttled, so a fast bow
never stacks into noise, and only a few sounds play at once.

| Sound | When |
|---|---|
| shot_bow / shot_crossbow / shot_handgonne / shot_bolt | a knight fires (bolt = forged or magic shots) |
| swing | a melee swing |
| hit / kill | a monster is struck / falls |
| hero_hurt | a knight takes a hit |
| roll / shield_block | a dodge roll / a shot glancing off a raised shield |
| bomb | a bomb clears the room |
| card_pickup / spoil_pickup / play_card | a card taken / spoils taken / a card played |
| door_seal / door_open | the gates bar when a fight starts / open when it ends |
| wave_gate | a doorway burns before a wave pours in |
| spike / rock_land / vent_fire | the Crypt's spikes, the Mines' rocks, Dragon Keep's vents |
| chest_open / mimic | a chest opens / a chest bites |
| boss_rise / boss_fall | a guardian appears / falls |
| descend | the party takes the stairs |
| wall_up / mana_empty | a held wall rises / mana runs dry |

Music tracks: `crypt`, `mines`, `keep`, `boss`, `sanctuary`, `menu`.
Sources and licences are in `../LICENSES.md`.

## Levels

The voices were overpowering because nothing else was playing. The mix is
now built by class.

| Class | File level | Runtime level | Net |
|---|---|---|---|
| Music | -20 LUFS integrated | 0.35 | about -29 LUFS |
| SFX | RMS -20 dB, -2 dBFS limited peaks | 0.5 | about -26 dB |
| Voices | -16 LUFS (`voices/voice.py`) | `VOICE_LEVEL` 0.28 | about -27 LUFS |

Music sits under play. Short SFX transients cut through. Voices land near
the music and a touch above it, so they stay clear without dominating. The
runtime levels are constants in `together_audio.rs`.

## Regenerating

- `level.py` re-levels everything offline, with no API calls.
- `pack.py` fetches and converts the CC0 music from OpenGameArt into
  `$TMPDIR/audio-packs`.
- `sound.py` (ElevenLabs) made the SFX and the sanctuary track. It skips any
  file that already exists. ElevenLabs credits are limited, so prefer CC0
  packs (Kenney.nl, OpenGameArt) for new sounds.
