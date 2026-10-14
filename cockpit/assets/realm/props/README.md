# Realm props

Sculpted props for the true-3D world (`stage/world_viz/world3d/props.rs`).
Each was generated as a textured mesh with Tencent HY 3D Global
(hy3d.tencent.ai, model 3DGeneration-V3.1, text-to-3D, 50k faces, four
candidates per prompt) on 2026-10-10, then reduced and recoloured for the
realm by a prop script kept out of the release: decimated to a few hundred faces and
every face snapped to one of the shared semantic materials, so a prop takes
the night palette, the fog and the firelight like the hand-built geometry.

Files in this folder are compiled in and placed in a room. Files in
`library/` are checked by the tests but not compiled in; a scene that wants
one adds it to `PROPS`. Every prompt ended with ", as a fantasy game prop.
Chunky, readable silhouette, hand-crafted look."

| Prop | Where | Prompt | Candidate | Converted with |
|---|---|---|---|---|
| `dragon_skull` | keep hall, on the flags left of the hearth | A huge ancient dragon skull with long backward-curving horns, jagged teeth and empty eye sockets, resting half-sunk on a pile of rocks | 3rd | `--height 1.5 --faces 420 --mats parchment,stone_dark,rock --light parchment --light-share 0.35` |
| `knight_statue` | gatehouse, behind the left brazier | A weathered stone statue of a knight in plate armor standing on a square plinth, both hands resting on a sword held point-down, a shield at his feet | 2nd | `--height 2.6 --faces 450 --mats armor,stone,stone_dark,brass` |
| `siege_perilous` | Round Table hall, across the board from the eye | The Siege Perilous: a tall carved stone throne with a high pointed back, worn red cushion, carved knotwork, an empty crown motif at the top | 3rd | `--height 2.3 --faces 180 --mats stone,stone_dark,banner,brass,parchment` |
| `library/castle` | | A small medieval castle keep as a fantasy game miniature: a square stone keep with two round corner towers, conical slate roofs, a gatehouse with a wooden door, arrow slits, and a long pennant banner on the tallest tower | 1st | `--height 4.5 --faces 2400 --mats stone,stone_dark,roof,door,banner,cloth_blue,grass,path,brass` |
| `library/fortune_wheel` | | Dame Fortune's wheel: a large upright wooden wheel of fortune with gilded spokes, painted segments and a pointer, standing on a carved oak stand with a small step | 1st | `--height 2.3 --faces 1400 --mats wood,door,brass,banner,cloth_blue,cloth_gold,parchment,foliage,stone_dark` |
| `library/great_forge` | | A dwarven great forge: a massive hearth of dark carved stone blocks with a glowing coal mouth, a tall square chimney, a heavy iron anvil on an oak stump in front, leather bellows at one side, hammers hanging on the stone | 2nd | `--height 3.0 --faces 900 --mats stone_dark,stone,wood,ember,armor,leather,brass` |
| `library/carved_throne` | | (the Siege Perilous prompt) | 2nd | `--height 2.4 --faces 900 --mats stone,stone_dark,banner,brass` |
| `library/stone_knight` | | (the knight statue prompt) | 3rd | `--height 2.6 --faces 600 --mats stone,stone_dark --light stone --light-share 0.3` |
| `library/brazier` | | A wrought iron brazier on three curled legs, a wide bowl heaped with glowing coals and small flames | 4th | `--height 1.6 --faces 600 --mats stone_dark,ember,fire --emissive-min 0.32` |
| `library/signpost` | | A wooden signpost with three arrow-shaped boards pointing in different directions, on a short post set in a small cairn of stones, a lantern hanging from it | 2nd | `--height 2.6 --faces 600 --mats wood,door,trunk,stone,stone_dark,fire,foliage,brass` |

The great forge was tried as the smithy's hero piece and taken out: at the
mini pane's 96 × 72 dots the hand-built forge (a big lit arch, a clear hood,
the anvil in front) reads better than any decimated one.

Rooms keep to their 700-triangle budget, and the castle court has no room
left under its 5,000, so library props wait for a scene with space for them.
Render any prop on a patch of ground with
`ANGEL_PROP_SHOTS=<dir> cargo test --bin angel write_prop_shots -- --ignored`
(`ANGEL_PROP_DIR=<dir>` renders loose `.obj` files instead), and every room
with `ANGEL_ROOM_SHOTS=<dir> cargo test --bin angel write_room_shots -- --ignored`.
