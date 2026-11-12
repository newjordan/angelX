# Claim guardians

Six original pixel-art monster portraits generated with the built-in ImageGen
tool for Knight's Wish. The source PNGs retain their generated RGBA transparency.
Use them for deposit details, expedition planning and the kingdom's bestiary.
Ironroot, Cindermaw and the Amethyst-Crowned Wyrm also have real optional claim
encounters in the Delve. The other three remain expansion portraits.

| Asset | Claim resource | Identity |
|---|---|---|
| `ironroot-colossus.png` | `iron-ore` | Stone and iron bulwark with hammer fists |
| `cindermaw.png` | `coal`, `ember` | Coal-armored salamander with ember seams |
| `verdigris-mantis.png` | `copper-ore` | Copper and turquoise hook-limbed guardian |
| `amethyst-crowned-wyrm.png` | `crystal` | Coiled mineral dragon with a crystal crown |
| `mossback-briarhorn.png` | `timber` | Root-and-oak boar with a mossy back |
| `chalkwing-gargoyle.png` | `stone` | Chipped limestone winged quarry sentinel |

`manifest.json` includes dimensions, hashes, resource IDs, intended use, visual
review notes, the complete generation prompts and the Cindermaw cleanup prompt.
`prompts.json` retains the initial prompt set. Copper is an expansion asset and
need not be enabled by a content pack. Cindermaw can stand for either coal or
ember deposits.

Preserve aspect ratio when displaying the masters (`object-fit: contain` in a
browser). Five files are 1254 × 1254; Mossback is 1312 × 1199. The build creates
48-pixel compact portraits, 128-pixel gallery portraits and 64-pixel combat
sprites. These are derived runtime images, not additional source masters.
Combat sources are cropped
to their visible alpha bounds and share a scale across their resting and
wind-up paintings, so their feet stay planted as poses change.

Validation: all six decode as RGBA with transparent background pixels, complete
visible silhouettes and distinct resource materials. Source alpha includes soft
edge pixels; `visible_alpha_bbox` in the manifest measures alpha >= 128, and
`corner_alpha` preserves the exact corner values. No source pixels were altered
by scripts. The coal sprite's halo was removed with an ImageGen edit.


## Implemented encounters

| Guardian | Claim | Signature and counterplay |
|---|---|---|
| Ironroot | Iron ore | Three ground rings with a clear lane; a shown charge lane joins the fight below 70% health. Sidestep the charge and attack during recovery. |
| Cindermaw | Coal or ember | A marked leap, then an inhale below 70% health. Leave the landing circle, walk away from the pull, and strike after it lands. |
| Amethyst | Crystal | A warned sweeping beam and crystal volleys; ground rings join its rage. Cross the beam before it fires and use the gap between volleys. |

All three escalate at low health. Their existing boss-engine signatures have
real floor telegraphs, damage, one-hit-per-signature protection and recovery
windows. The generated `combat/*-wind.png` paintings replace their resting
poses during warnings; the renderer animates breath, crouch, leap height,
mineral charge and recovery stars from actual simulation state. Full edit
prompts and provenance live in `combat/prompts.json` and `combat/manifest.json`.

Clear the room, then press E at its discovered vein to challenge the guardian.
A settled guardian death unlocks the claim; return to the vein and press E to
secure it. The boss drops normal expedition spoils once. The kingdom receives
no ore until a staffed, connected claim outpost extracts and delivers it.
Missing bodies, canceled encounters and saved revisits cannot count as kills.

Implementation: `crates/delve/src/rules/claim_bosses.rs` and
`crates/delve/src/render/overworld/arena/claim_bosses.rs`. Run the `claim_boss`
test filter for gating, signatures, saves, render silhouettes and a normal-input
starter-bow-to-mine integration test. The ignored `write_claim_boss_review`
test saves actual arrival, warning and recovery frames under
`/tmp/claim-boss-review`. Arrival, warning and recovery captures refill the
review knight's health so the desired pose can be observed. The victory capture
comes from the separate normal-input starter-knight fight without health resets
or an assigned corpse. `measure_claim_boss_starter_duels` reports full fights
without resetting health or cooldowns. Checked-in captures and these distinctions
are listed in [the screenshot index](https://github.com/newjordan/angelX/blob/main/docs/images/kingdom-works/README.md).


## Ore sentries and advance warnings

Each newly discovered seam has two finite ore sentries before its optional
boss challenge. Four new ore-associated appearances are retained in
`ore-enemies-atlas.png`, with complementary attack anticipation paintings in
`ore-enemies-windup-atlas.png`. Their manifests retain exact crop rectangles,
prompts, alpha bounds and checksums. Source artwork is compiled into 36-pixel resting sprites and 36–44-pixel
attack anticipation sprites, with a consistent floor anchor. The planning
guide shows the same recognizable silhouettes.

| Resource | Enemy | Real threat | Response |
|---|---|---|---|
| Iron | Ironplate sentinel | Frontal projectile shield and a warned heavy bash | Flank, force a guard break with shots, or attack during its shield-down pause |
| Coal | Blast crawler | Blinking planted kegs and bombs aimed at marked landing circles | Leave the circle; shoot a planted keg from a safe distance |
| Crystal | Prism stalker | Copies and returns a player's projectile after a flash, even under continuous fire | Move sideways after firing or shield the reflected shot |
| Ember | Cinderhound | Locks a shown charge lane and rushes, leaving burning embers | Sidestep the lane; attack after the rush and avoid the trail |

These use the existing Pavise, Sapper, Mirrorling and Hound state machines.
The ore adapter adds bounded, warned lobs to coal and ensures crystal finishes
its reflection tell under continuous fire. Shields, explosions, reflected
shots and fire trails are actual simulation objects and damage sources.
Sentries retain their IDs and wounds through checkpoints; settled deaths pay
once, missing bodies cannot unlock a claim, and a cleared roster does not
respawn on repeat discovery. The shared `OreKind` descriptions drive the ore
guide, discovery warnings and the upcoming-resource forecast.
