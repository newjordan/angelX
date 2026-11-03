# Delve sound effects and music

Played by `crates/realm-host/src/together_audio.rs`. The game names a sound and
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
| area_leave | a door shut behind the party leaving a place: an entrance taken, a place of the kingdom or an activity left |
| wave_gate | a doorway burns before a wave pours in |
| spike / rock_land / vent_fire | the Crypt's spikes, the Mines' rocks, Dragon Keep's vents |
| chest_open / mimic | a chest opens / a chest bites |
| boss_rise / boss_fall | a guardian appears / falls |
| descend | the party takes the stairs |
| wall_up / mana_empty | a held wall rises / mana runs dry |
| wheel_spin / wheel_land | Fortune's wheel clicks down to a stop / its bell rings the delve's mode |
| wish_grant | a scroll or a spell landing on the floor, a brew, a scroll learned (a wish granted now lands with `wish_land`) |

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

The finishers' sounds (a minion's last blow, `rules::gore`; made offline by `../synth_sfx.py`):

| Sound | When |
|---|---|
| gore_kick | a knight kicks a point-blank kill back: a boot into the ribs |
| gore_thunk | the kicked body, shaft in it, arrives in the stone (or the floor) |
| gore_chop | a blade takes a head or cuts a monster in half |
| gore_splat | a slime flattens, a shot knocks a body back wet |
| gore_bones | a skeleton comes apart |
| gore_burst | splash or an explosive blow bursts a monster into chunks |

The blades, magic by family and the wish moments (`rules::melee`,
`rules::gore`, `rules::vfx::Voice`, `altar`, `overclass`, `nooks::gnome`;
made offline by `../synth_sfx.py`, each a transient, a body and a tail):

| Sound | When |
|---|---|
| swing_spear / swing_sword / swing_axe / swing_cleaver / swing_maul | a knight's stroke, by the arm it swings (`melee::Blade`, the arm the picture draws): a spear's quick high flick, a sword's clean cut with its edge singing, an axe's chunky chop with its head fluttering round, a cleaver's flat "whum", a maul's deep "vwoom". Fastest in the first 50 to 100 ms, where the steel is drawn, the follow-through after; the Hammerer's maul still lands with `hit_blast` |
| hit_flesh_light / hit_flesh_medium / hit_flesh_heavy | the stroke lands, once however many it struck, as heavy as its heaviest contact (`melee::Tier`): a tick and a slap; a crack, a meaty thwack with bone in it and the wet of the cut; a crack, bone crunching, a deep driven thud (the third of a chain, a maul's overhead, 45 damage and up) |
| parry / parry_whizz | a bullet turned off the blade: a sharp tick and a high clean ting ringing on (two of its modes a hair apart, so it shimmers; well above the shield's `clang`), and the turned shot whining back, falling in pitch as it goes |
| gore_behead / gore_head_thud | a blade takes a head: the thwack through the neck with the vertebra's crack and the spray pulsing out; then, when the picture lands the head (`gore::HOLD` + its span), a dull knock of skull, a slap and a bounce |
| gore_bisect / gore_slump | a blade cuts a body through: a long wet rip, the spine's crack, the weight parting, the spill; then the halves hitting the floor, one and then the other |
| cast_fire / cast_frost / cast_holy / cast_storm / cast_poison / cast_arcane / cast_earth | a spell cast, by its family (`vfx::Palette::voice`, the colours the magic pass draws it in): a wind-up while the rune opens (a seventh of a second), then the release. Fire a roar drawn up and a whoomph with fluttering flame; frost a glassy climb, a crystal chime and crackling ice; holy a choir swelling into a bell's bloom; storm a charge buzzing up, a snap, static and a rumble (also the Thunder Scroll's and a nova's crackle); poison a hiss, a wet glorp and bubbles popping; arcane a glassy sweep, a deep "vwom" and a shimmering chord; earth stone grinding, then rock crunching and pebbles |
| impact_fire / impact_frost / impact_holy / impact_storm / impact_poison / impact_arcane / impact_earth | a spell's bolt or falling star breaks on a monster, in its family's way (at most one a family every four ticks): a boom of flame and sizzling embers; a crack and a shatter of tinkling shards; a radiant chime; a whip-crack of thunder rolling away (also the Thunder Scroll's thunder); a wet acid splat; a phasing zap; a rock boom and debris |
| lamp_rub / genie_appear | the lamp rubbed: three strokes of palm on brass, a breath rising and a magic climbing; then, as the genie rises out of the smoke (`altar::GENIE_AT`, 14 ticks on), a soft poof, a shimmering chord and sparkles running up it |
| wish_land / wish_land_grand / wish_land_mythic | a wish lands on its knight, by tier (`overclass::LAND_SOUNDS`): light drawn in for a fifth of a second, then a sparkling D major chord swelling (I and II); with its lower voices, a boom and sparkles climbing (III and IV); and a bell tolling over it all (V). A great wish climbing several tiers at once lands once, at the highest |
| fairy_wish | the Gnome's house holds and the fairies pay: a run up through fairy dust, then giggles in chimes (quick bursts of high notes, each bent down at its end), dust tinkling all through |
| level_up | a knight reaches a level (`rules::rewards`): a bright brass fanfare climbing G major, its last note held under sparkles |
| rung_up | a knight climbs a rung of the resolution ladder: a lens's click and a glassy run of chimes upward, the last ringing clear |
| relic_find | a relic is taken off the floor: a bell struck deep, a choir's chord opening, sparkles falling |
| deepest_yet | the party stands deeper than the realm ever has: a boom far down and a horn calling twice, a fourth apart |
| sky_lit | a constellation of the realm's sky is lit: its stars ringing in one after another, then all of it together |

These sit by loudness rather than body RMS (`synth_sfx.py`'s `LEVEL`):
looped and measured with ebur128, a swing about -21 LUFS (the old
`swing`), the contacts -21, -20 and -17.5 (the old `hit_blade` -18), a
cast or a break -18.5 to -20.5 with the shots, a landing -17, -16.5 and
-16 by tier; true peaks a decibel under full scale at least.

A sound the rules name before its own take is made plays a stand-in
(`angel_delve::wire::STAND_INS`): the shoulder ride's `shoulder_climb` plays
`hand_grab`, `shoulder_hop` plays `roll`, `shoulder_throw` plays
`bomb_toss` and `shoulder_slam` plays `hit_blast`. A file of the hook's own
name takes over once the stand-in is dropped from that table.

The Bard's sounds (`rules::bard`; made offline by `../synth_sfx.py`):

| Sound | When |
|---|---|
| sax_g2 … sax_a3 | each volley of the Bard's notes blows the next stab of its riff (`bard::RIFF`: two bars of sixteenths in A minor, bouncing off the low A), so a held stream plays a deep techno sax line: tongued hard, scooped up into pitch, the tone closing down like a filter, a sine an octave under |
| bard_kick | under every fourth volley (`bard::pulse`): a deep techno kick, so a held stream is four on the floor at 112.5 BPM |
| hit_note | a note lands: an open hi-hat over the groove, so hitting fills it in |
| bard_lullaby | the Bard sings (R): the same sax slow and breathy, falling home through E major over a soft swell, two heartbeats slowing under it (every other ultimate keeps `boss_rise`) |
| bard_wub | held two seconds without a break (`bard::WUB_AT`), the bass drops in under every kick: an A an octave under the riff, its filter opening and shutting twice a beat (wub wub) |
| bard_overload | held a whole riff (`bard::GLITCH_AT`), the horn overloads once: a crushed zap screaming up, a boom through the floor, a crash and a buzz (the banner says "Wub wub!") |
| bard_glitch_0 … bard_glitch_3 | after the overload, one on each off-beat: the stab caught in a loop, a machine's blips, a tape stop, crushed static |

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

Measured as loudness (ffmpeg `ebur128`, integrated over each clip), the
effects sit at a median of -19.3 LUFS, the middle half between -22.3 and
-16.6, the footsteps about eight under. On 2026-10-25 the ten outliers were
brought in by gain alone, each re-encoded in its own format (mp3, its own
rate and channels): `kill` down to -16, just under `hero_hurt` so a knight's
hurt still reads over the kills; `hit`, `hit_magic`, `hit_axe`, `swing` and
`gore_bones` up to -25, still under every shot; `vent_fire` and `rock_land`,
the furnace's and the falling stone's cues, up to the set's middle; and
`armour_fall` and `wheel_spin`, whose true peaks touched 0 dB, down to -1.

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
