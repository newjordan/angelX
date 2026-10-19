# Lanternward: art requests for the second wave

Blocks first. Every actor and prop on the stage is a registered slot
(`slots.json`: canvas, bottom-centre anchor, collision geometry, animation
timing) drawn as a labelled block until a painted frame arrives. **Art never
sets a hitbox**: the rules (`rules/lanternward/*`) say where a body is and how
big it is; a slot's `collision` is documentation of that, not an input. The
stage stamps a frame by `lanternward.<id>` and `<state>-<index>.png`; a native
PNG must match the slot's `size` exactly. Concept sheets (painted, read-only
references, not to be copied into the repo) are in
`~/angelX_minigames/games/lanternward/assets/raw/`.

The world is seen from above at 16 px to the tile, 384 by 224 native (24 by 14
tiles). The Delve's own knight is 16 by 16 and stays the scale reference:
a door or a gate is one tile, a pot is a little under half a knight, the
Warden about three knights wide. Palette: the realm master palette only
(`render/overworld/ink.rs`); signal inks (`4 5 6 @ w 0 1 2 3 7 8`) for light,
state and tells, never for decoration.

## Identity before pixels

| Thing | Silhouette | Material | One signature feature |
|---|---|---|---|
| The knight (all six seats) | The Delve's own knight, seat-coloured | steel, a red field and a gold star on the shield | a small lantern at the hip that burns once the Lantern is theirs |
| Snuffer | a hood-shaped lump, point up, a tail curling out the back | cold smoke over frayed dark cloth | two pale eyes in the hood and a wick-stub at the end of the tail, lit when it is awake |
| Warden | a tall hooded giant, wide shoulders, long ragged hem | black iron plate over dark cloth, chains hung with small lanterns | the **lamp-heart**: a caged lamp in the chest, pale aqua when shut, amber and burning when open |
| Turret | a squat iron drum on a plinth with one barrel | riveted iron, a dull eye | the eye warms to amber through the tell and the barrel lines up with the footprint |
| Brazier | a spiked iron bowl on three legs | iron, ash, ember | the flame (out, lit, guttering) |
| Great brazier | a lantern-cage on a stone plinth, a glass orb inside | stone, iron, glass | the orb, dull aqua out and bright when lit |
| Crystal switch | a pointed crystal on a stone plinth | quartz, stone | aqua glow when it wakes |
| Chest | a barrelled chest banded in iron | oak, iron | the crescent-moon plate; open, it pours gold light |
| Doors | an arched door in a stone frame | oak, iron, stone | the small-key door wears a pale crescent ring, the Boss Key door a red one and chains |
| Wick-wisp | a drop of light with a short tail | light | it bobs and leans toward what to do next |

## Slots (all in `slots.json`, `placeholder: true`)

Actors: `hero` (32 by 32; idle, run, strafe, hop, flip, slash-1..3, lunge,
charge, spin, jump-attack, block, parry, hurt, item-get, down, fall; every
state faces right, the stage flips for left; until painted the Delve's knight
stands in), `snuffer` (18 by 20; idle, drift, tell, lunge, recover, hit,
stagger, die, hooded), `turret` (22 by 24; idle, tell, fire, recover, stun),
`warden` (48 by 56; sleep, idle, walk, beam-tell, beam, slam-tell, slam,
summon, hit, rear, stagger, die, heart-shut, heart-open: the lamp-heart is a
separate overlay at the chest).

Props: `brazier` `rest` `ember` `great` `chest` `pot` `grass` `sign` `folk`
`lever` `crystal` `gate` `plate` `block` `cracked` `lock` `boss-door` `sigil`
`beacon` `fissure`; floor things `heart` `gem` `bomb` `container`; in the air
`bolt` (iron, light, bright, dim) `wisp` `mark` (the wick-spark over a locked
target).

Tiles (procedural now, one 16 by 16 each per room when painted): floor in five
stones (the Stair grey-blue, the Lamp Hall oak planks, the Gallery blue slate,
the Vault black basalt, the Warden's ring of dark flags), wall, bars, pit,
doorway down. The Warden's floor carries a great star ring.

## Import

Each slot has an inactive map, `assets/art-slots/lanternward.<id>/lanternward.<id>.map.json`
(`runtime: false`, one piece per frame with its ticks, the slot's anchor): the
ordinary import (`docs/SLOT-ART-IMPORT.md`) fills `v1/<state>-<index>.png`,
approves the pieces and turns the slot on; the stage reads a frame by
`super::slot_art::bytes(id, "<state>-<index>")` and falls back to its block.
A bank is painted whole or not at all: if any frame of any state of a slot is
missing, every state of it is the block, and the code's own art (a flame, a
chest's mouth, a gate's bars) is drawn only over a block. The stage plays what
the rules can drive: the gate's `closed` and `open` (its `opening` frames wait
for a rules clock), the chest's `closed` and `open` (its `hidden` ghost is not
drawn while a chest is not yet there).

## Timings

Frame durations are in `slots.json` (ticks of the 30 Hz sim). The ones that
carry meaning and may not be stretched: the snuffer's tell is 24 ticks, the
Warden's and the turret's tell at least 40 (the picture holds the footprint
for exactly the tell), a parry's flash and ring 12, the item-get pose 60, the
death puff 14.

## Sound

The stage uses the Delve's existing cues (`card_pickup` for the lock's tick,
`holy_water` for the parry chime, `wish_grant` for a puzzle solved, `wheel_land`
for an item held up, `mana_empty` for the low-heart chirp). A dedicated lock
tick, parry chime and fanfare are a request for the sound pass.
