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
| clang | a thrown shield strikes a foe or glances off stone (synthesised locally with ffmpeg: inharmonic partials, fast decay; no generated audio) |
| bomb | a bomb clears the room |
| card_pickup / spoil_pickup / play_card | a card taken / spoils taken / a card played |
| door_seal / door_open | the gates bar when a fight starts / open when it ends |
| wave_gate | a doorway burns before a wave pours in |
| spike / rock_land / vent_fire | the Crypt's spikes, the Mines' rocks, Dragon Keep's vents |
| chest_open / mimic | a chest opens / a chest bites |
| boss_rise / boss_fall | a guardian appears / falls |
| descend | the party takes the stairs |
| wall_up / mana_empty | a held wall rises / mana runs dry |
| wheel_spin / wheel_land | Fortune's wheel clicks down to a stop / its bell rings the delve's mode |

The spectacle's sounds (made offline by `../synth_sfx.py`):

| Sound | When |
|---|---|
| shot_arcane / shot_spark | a Mage's arcane bolt leaves the staff (a rising glassy zap) / a Healer's spark leaves the censer (a swung whoosh, a small bell, embers) |
| knight_hit / knight_hit_big | a knight is hit / hit hard (a fifth of their health or more) |
| hit_blade / hit_axe / hit_arrow / hit_bolt / hit_blast / hit_magic | a hit lands: the sword, the dwarf's axes, a bow, a crossbow, a handgonne, a forged bolt |
| step_stone / step_gravel / step_ash / step_wet / step_moss / step_wood | footsteps on the Crypt, the Mines, Dragon Keep and the Unknown, the Drowned Archive, the Fungal Deep, the home's boards |
| door_pass | the party walks through a doorway |
| gold / potion / holy_water | gold is picked up; a draught is drunk; holy water is splashed |
| bomb_toss / shield_up / axe_spin | a bomb is thrown; a shield comes up; the dwarf spins |
| helmet_pop / armour_fall | the helmet flies off at two-fifths health; the armour comes off at fifteen in the hundred |
| engine_whistle | Grinwick lights a rail before charging; locally synthesised steam harmonics and hiss |
| boss_roar / boss_rage / boss_death | a guardian appears; it turns desperate; it falls |
| knight_fall / bones_clatter | a knight falls; the bones settle |
| crack_open / hand_rise / hand_grab / hand_drag / ground_close | the demon hand claims a fallen guardian: the ground cracks, the hand climbs out, closes, drags the body down, and the ground shuts |
| hell_gate_rise / hell_gate_enter / circle_toll | the hell gate rises at a dungeon's end; the party goes through; the new circle's bell |

Music tracks: `crypt`, `mines`, `keep`, `boss`, `sanctuary`, `menu`.
Sources and licences are in `../LICENSES.md`.

## Levels

The voices were overpowering because nothing else was playing. The mix is
now built by class.

| Class | File level | Runtime level | Net |
|---|---|---|---|
| Music | -20 LUFS integrated | 0.35 | about -29 LUFS |
| SFX | RMS -20 dB, -2 dBFS limited peaks | 0.5 | about -26 dB |
| Knights' chatter | RMS -23 dB, -3 dBFS limited peaks (`../knights.py`) | `VOICE_LEVEL` 0.5 | about -29 dB |
| Knights' grunts | RMS -26 dB, -3 dBFS limited peaks (`../knights.py`) | `VOICE_LEVEL` 0.5 | about -32 dB |

Music sits under play. Short SFX transients cut through. The knights' voices
sit under the effects: small and rare, never over the fight. Nobody
narrates. The runtime levels are constants in `together_audio.rs`;
`/dungeon voices <0-100>%` and **V** turn the knights up, down or off.

## The knights' voices (`knights/`)

Each knight speaks with their seat's voice (`a` to `d`), two takes of every
clip: `<voice>_<clip>_<take>.mp3`. Chatter is a few words of the knights' own
made-up tongue, never English: `cheer`, `ooh`, `uhoh`, `aww`, `thanks`, `go`,
`hello`, `hehe`. Grunts are one sound each: `hup` (a roll), `oof` (a hit),
`hyah` (an ultimate), `waah` (a fall). `together_shooter/chatter.rs` picks
the moments: big ones (a guardian rising or falling, a knight down, a
revive, the way down) after a three-second breath, ordinary ones only after
25 quiet seconds, and grunts one knight at a time, at least four seconds
apart (a fall always sounds). In a hard three-knight fight that is about one
noise every five seconds (`knights_noise_rate`, an ignored test, measures it).

## Regenerating

- `../synth_sfx.py` makes the spectacle's sounds from noise bursts, filtered
  sweeps and decaying sine modes (numpy and scipy, no API, no sound pack),
  encodes them as mono 44.1 kHz 96 kbit/s mp3 and levels them itself: RMS
  -20 dB with peaks a hair under -2 dBFS like the rest, and the footsteps six
  decibels lower (RMS -26 dB) because they come many a second.
  `python3 synth_sfx.py [name ...]` makes them; `--stats` lists every file's
  length, peak and RMS.
- Browser guests hear the same sounds: the page fetches `/sfx` and
  `/sfx/<name>.mp3` from the host and plays the names the host's mixer pushes.

- `../knights.py` (ElevenLabs text-to-speech) records the knights' voices
  into `knights/`, trimmed, lifted a touch in pitch and levelled. It keeps
  existing files: delete one to re-record it.
- `level.py` re-levels everything offline, with no API calls.
- `pack.py` fetches and converts the CC0 music from OpenGameArt into
  `$TMPDIR/audio-packs`.
- `sound.py` (ElevenLabs) made the SFX and the sanctuary track. It skips any
  file that already exists. ElevenLabs credits are limited, so prefer CC0
  packs (Kenney.nl, OpenGameArt) for new sounds.
