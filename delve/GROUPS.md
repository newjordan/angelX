# Port groups

Each group is one agent's share of `crates/delve/src` (rules, world, realm).
Fragments go in `delve/rules/<group>/`, stubs in `delve/stubs/<group>.bend`.
`host_commands` and `runtime_state` stay host-side Rust for now (the host's journal and codec).
`numerics` owns `delve/lib/w64.bend` and `delve/lib/rs.bend`.

## core — 20792 Rust lines, 51 modules

Modules: `rules`, `hero`, `skills`, `phrasebook`, `feel`, `looks`, `arms`, `ults`, `shoulders`, `knights`, `hud`, `activity`, `stakes`, `fortune`, `mirror`, `gore`, `tune`, `identity`, `floor_stats`, `tray`, `swamp`, `abilities`, `artistry`, `command`, `music`, `classes`, `gear`, `skulls`, `mend`, `wishcraft`, `spells`, `desperate`, `fidelity`, `hitstop`, `aftermath`, `trophies`, `hexer`, `fx`, `lich`, `campaign`, `script`, `wounds`, `aoe`, `again`, `items`, `peasant`, `spin`, `bard`, `scratch`, `risen`, `map`

<details><summary>files</summary>

- `crates/delve/src/rules.rs`
- `crates/delve/src/rules/abilities.rs`
- `crates/delve/src/rules/activity.rs`
- `crates/delve/src/rules/aftermath.rs`
- `crates/delve/src/rules/again.rs`
- `crates/delve/src/rules/aoe.rs`
- `crates/delve/src/rules/arms.rs`
- `crates/delve/src/rules/artistry.rs`
- `crates/delve/src/rules/bard.rs`
- `crates/delve/src/rules/campaign.rs`
- `crates/delve/src/rules/classes.rs`
- `crates/delve/src/rules/command.rs`
- `crates/delve/src/rules/desperate.rs`
- `crates/delve/src/rules/feel.rs`
- `crates/delve/src/rules/fidelity.rs`
- `crates/delve/src/rules/floor_stats.rs`
- `crates/delve/src/rules/fortune.rs`
- `crates/delve/src/rules/fx.rs`
- `crates/delve/src/rules/gear.rs`
- `crates/delve/src/rules/gore.rs`
- `crates/delve/src/rules/hero.rs`
- `crates/delve/src/rules/hexer.rs`
- `crates/delve/src/rules/hitstop.rs`
- `crates/delve/src/rules/hud.rs`
- `crates/delve/src/rules/identity.rs`
- `crates/delve/src/rules/items.rs`
- `crates/delve/src/rules/knights.rs`
- `crates/delve/src/rules/lich.rs`
- `crates/delve/src/rules/looks.rs`
- `crates/delve/src/rules/looks/fit.rs`
- `crates/delve/src/rules/map.rs`
- `crates/delve/src/rules/mend.rs`
- `crates/delve/src/rules/mirror.rs`
- `crates/delve/src/rules/music.rs`
- `crates/delve/src/rules/peasant.rs`
- `crates/delve/src/rules/phrasebook.rs`
- `crates/delve/src/rules/risen.rs`
- `crates/delve/src/rules/scratch.rs`
- `crates/delve/src/rules/script.rs`
- `crates/delve/src/rules/shoulders.rs`
- `crates/delve/src/rules/skills.rs`
- `crates/delve/src/rules/skulls.rs`
- `crates/delve/src/rules/spells.rs`
- `crates/delve/src/rules/spin.rs`
- `crates/delve/src/rules/stakes.rs`
- `crates/delve/src/rules/swamp.rs`
- `crates/delve/src/rules/tray.rs`
- `crates/delve/src/rules/trophies.rs`
- `crates/delve/src/rules/tune.rs`
- `crates/delve/src/rules/ults.rs`
- `crates/delve/src/rules/wishcraft.rs`
- `crates/delve/src/rules/wounds.rs`

</details>

## combat — 18057 Rust lines, 34 modules

Modules: `patterns`, `gauntlets`, `bosses`, `critters`, `signatures`, `depths`, `foes`, `champions`, `minions`, `monsters`, `cinder`, `spoils`, `outlaw`, `boss_gates`, `hazards`, `boss_ecology`, `secrets`, `feats`, `thief`, `keyring`, `boss_population`, `bodies`, `horrors`, `loot`, `encounter_catalog`, `hunt`, `hollow`, `pit`, `tide`, `press`, `works`, `hell`, `cover`, `pacing`

<details><summary>files</summary>

- `crates/delve/src/rules/bodies.rs`
- `crates/delve/src/rules/boss_ecology.rs`
- `crates/delve/src/rules/boss_gates.rs`
- `crates/delve/src/rules/boss_population.rs`
- `crates/delve/src/rules/bosses.rs`
- `crates/delve/src/rules/champions.rs`
- `crates/delve/src/rules/cinder.rs`
- `crates/delve/src/rules/cover.rs`
- `crates/delve/src/rules/critters.rs`
- `crates/delve/src/rules/depths.rs`
- `crates/delve/src/rules/encounter_catalog.rs`
- `crates/delve/src/rules/feats.rs`
- `crates/delve/src/rules/foes.rs`
- `crates/delve/src/rules/gauntlets.rs`
- `crates/delve/src/rules/hazards.rs`
- `crates/delve/src/rules/hell.rs`
- `crates/delve/src/rules/hollow.rs`
- `crates/delve/src/rules/horrors.rs`
- `crates/delve/src/rules/hunt.rs`
- `crates/delve/src/rules/keyring.rs`
- `crates/delve/src/rules/loot.rs`
- `crates/delve/src/rules/minions.rs`
- `crates/delve/src/rules/monsters.rs`
- `crates/delve/src/rules/outlaw.rs`
- `crates/delve/src/rules/pacing.rs`
- `crates/delve/src/rules/patterns.rs`
- `crates/delve/src/rules/pit.rs`
- `crates/delve/src/rules/press.rs`
- `crates/delve/src/rules/secrets.rs`
- `crates/delve/src/rules/signatures.rs`
- `crates/delve/src/rules/spoils.rs`
- `crates/delve/src/rules/thief.rs`
- `crates/delve/src/rules/tide.rs`
- `crates/delve/src/rules/works.rs`

</details>

## world — 19484 Rust lines, 26 modules

Modules: `world`, `layout`, `realm`, `countryside`, `forest_road`, `floors`, `exits`, `trinkets`, `kingdom`, `floor_map`, `roundtable`, `runes`, `rooms`, `woodland`, `gantry`, `foreman`, `powder`, `windcountry`, `missions`, `coastal`, `bowcountry`, `hop`, `wetland`, `guide`, `relics`, `mummers`

<details><summary>files</summary>

- `crates/delve/src/realm.rs`
- `crates/delve/src/realm/appearance.rs`
- `crates/delve/src/realm/chivalry.rs`
- `crates/delve/src/realm/effects.rs`
- `crates/delve/src/realm/forge.rs`
- `crates/delve/src/rules/bowcountry.rs`
- `crates/delve/src/rules/coastal.rs`
- `crates/delve/src/rules/countryside.rs`
- `crates/delve/src/rules/exits.rs`
- `crates/delve/src/rules/floor_map.rs`
- `crates/delve/src/rules/floors.rs`
- `crates/delve/src/rules/foreman.rs`
- `crates/delve/src/rules/forest_road.rs`
- `crates/delve/src/rules/gantry.rs`
- `crates/delve/src/rules/guide.rs`
- `crates/delve/src/rules/hop.rs`
- `crates/delve/src/rules/kingdom.rs`
- `crates/delve/src/rules/layout.rs`
- `crates/delve/src/rules/layout/open.rs`
- `crates/delve/src/rules/missions.rs`
- `crates/delve/src/rules/mummers.rs`
- `crates/delve/src/rules/powder.rs`
- `crates/delve/src/rules/relics.rs`
- `crates/delve/src/rules/rooms.rs`
- `crates/delve/src/rules/roundtable.rs`
- `crates/delve/src/rules/runes.rs`
- `crates/delve/src/rules/trinkets.rs`
- `crates/delve/src/rules/wetland.rs`
- `crates/delve/src/rules/windcountry.rs`
- `crates/delve/src/rules/woodland.rs`
- `crates/delve/src/rules/world.rs`
- `crates/delve/src/world.rs`
- `crates/delve/src/world/biome.rs`
- `crates/delve/src/world/castles.rs`
- `crates/delve/src/world/college.rs`
- `crates/delve/src/world/doors.rs`
- `crates/delve/src/world/dressing.rs`
- `crates/delve/src/world/finds.rs`
- `crates/delve/src/world/houses.rs`
- `crates/delve/src/world/kingdom.rs`
- `crates/delve/src/world/map.rs`
- `crates/delve/src/world/roads.rs`
- `crates/delve/src/world/routing.rs`
- `crates/delve/src/world/signs.rs`
- `crates/delve/src/world/wild.rs`
- `crates/delve/src/world/wishes.rs`

</details>

## home — 22707 Rust lines, 42 modules

Modules: `cards`, `holdings`, `home_settlement`, `cauldron`, `hireling`, `questline`, `home`, `college`, `barony`, `scrolls`, `alehouse`, `genie`, `curios`, `tavern`, `bounties`, `frogs`, `town`, `overclass`, `dead_wishes`, `ranch`, `merchant`, `imps`, `spell_upgrades`, `wish_jobs`, `brew_jobs`, `raids`, `shield`, `crown`, `rescues`, `bestiary`, `talents`, `reforge_jobs`, `bazaar`, `yard`, `combos`, `card_paint`, `fellowship`, `room_end`, `presence`, `pots`, `merlin`, `exhibits`

<details><summary>files</summary>

- `crates/delve/src/rules/alehouse.rs`
- `crates/delve/src/rules/barony.rs`
- `crates/delve/src/rules/bazaar.rs`
- `crates/delve/src/rules/bestiary.rs`
- `crates/delve/src/rules/bounties.rs`
- `crates/delve/src/rules/brew_jobs.rs`
- `crates/delve/src/rules/card_paint.rs`
- `crates/delve/src/rules/cards.rs`
- `crates/delve/src/rules/cauldron.rs`
- `crates/delve/src/rules/college.rs`
- `crates/delve/src/rules/combos.rs`
- `crates/delve/src/rules/crown.rs`
- `crates/delve/src/rules/curios.rs`
- `crates/delve/src/rules/dead_wishes.rs`
- `crates/delve/src/rules/exhibits.rs`
- `crates/delve/src/rules/fellowship.rs`
- `crates/delve/src/rules/frogs.rs`
- `crates/delve/src/rules/genie.rs`
- `crates/delve/src/rules/hireling.rs`
- `crates/delve/src/rules/holdings.rs`
- `crates/delve/src/rules/home.rs`
- `crates/delve/src/rules/home_settlement.rs`
- `crates/delve/src/rules/imps.rs`
- `crates/delve/src/rules/merchant.rs`
- `crates/delve/src/rules/merlin.rs`
- `crates/delve/src/rules/overclass.rs`
- `crates/delve/src/rules/pots.rs`
- `crates/delve/src/rules/presence.rs`
- `crates/delve/src/rules/questline.rs`
- `crates/delve/src/rules/raids.rs`
- `crates/delve/src/rules/ranch.rs`
- `crates/delve/src/rules/reforge_jobs.rs`
- `crates/delve/src/rules/rescues.rs`
- `crates/delve/src/rules/room_end.rs`
- `crates/delve/src/rules/scrolls.rs`
- `crates/delve/src/rules/scrolls/policy.rs`
- `crates/delve/src/rules/shield.rs`
- `crates/delve/src/rules/spell_upgrades.rs`
- `crates/delve/src/rules/talents.rs`
- `crates/delve/src/rules/tavern.rs`
- `crates/delve/src/rules/town.rs`
- `crates/delve/src/rules/wish_jobs.rs`
- `crates/delve/src/rules/yard.rs`

</details>

## games_a — 18188 Rust lines, 7 modules

Modules: `lanternward`, `bannerpeak`, `griffinwing`, `altar`, `aerie`, `hunters`, `ledge`

<details><summary>files</summary>

- `crates/delve/src/rules/aerie.rs`
- `crates/delve/src/rules/altar.rs`
- `crates/delve/src/rules/bannerpeak.rs`
- `crates/delve/src/rules/bannerpeak/body.rs`
- `crates/delve/src/rules/bannerpeak/boss.rs`
- `crates/delve/src/rules/bannerpeak/bot.rs`
- `crates/delve/src/rules/bannerpeak/foes.rs`
- `crates/delve/src/rules/bannerpeak/knight.rs`
- `crates/delve/src/rules/bannerpeak/layout.rs`
- `crates/delve/src/rules/bannerpeak/oracle.rs`
- `crates/delve/src/rules/bannerpeak/session.rs`
- `crates/delve/src/rules/bannerpeak/world.rs`
- `crates/delve/src/rules/griffinwing.rs`
- `crates/delve/src/rules/griffinwing/boss.rs`
- `crates/delve/src/rules/griffinwing/bot.rs`
- `crates/delve/src/rules/griffinwing/combat.rs`
- `crates/delve/src/rules/griffinwing/flight.rs`
- `crates/delve/src/rules/griffinwing/rail.rs`
- `crates/delve/src/rules/griffinwing/script.rs`
- `crates/delve/src/rules/griffinwing/things.rs`
- `crates/delve/src/rules/griffinwing/trig.rs`
- `crates/delve/src/rules/griffinwing/types.rs`
- `crates/delve/src/rules/hunters.rs`
- `crates/delve/src/rules/lanternward.rs`
- `crates/delve/src/rules/lanternward/blow.rs`
- `crates/delve/src/rules/lanternward/boss.rs`
- `crates/delve/src/rules/lanternward/bot.rs`
- `crates/delve/src/rules/lanternward/ctx.rs`
- `crates/delve/src/rules/lanternward/foes.rs`
- `crates/delve/src/rules/lanternward/hint.rs`
- `crates/delve/src/rules/lanternward/kit.rs`
- `crates/delve/src/rules/lanternward/props.rs`
- `crates/delve/src/rules/lanternward/review.rs`
- `crates/delve/src/rules/lanternward/stage.rs`
- `crates/delve/src/rules/lanternward/world.rs`
- `crates/delve/src/rules/ledge.rs`

</details>

## games_b — 15597 Rust lines, 9 modules

Modules: `pastimes`, `moonfall`, `lightgallop`, `folk`, `dares`, `cat`, `forest`, `citizens`, `floor_plates`

<details><summary>files</summary>

- `crates/delve/src/rules/cat.rs`
- `crates/delve/src/rules/citizens.rs`
- `crates/delve/src/rules/dares.rs`
- `crates/delve/src/rules/floor_plates.rs`
- `crates/delve/src/rules/folk.rs`
- `crates/delve/src/rules/forest.rs`
- `crates/delve/src/rules/lightgallop.rs`
- `crates/delve/src/rules/lightgallop/bot.rs`
- `crates/delve/src/rules/lightgallop/level.rs`
- `crates/delve/src/rules/lightgallop/math.rs`
- `crates/delve/src/rules/lightgallop/rider.rs`
- `crates/delve/src/rules/lightgallop/session.rs`
- `crates/delve/src/rules/lightgallop/terrain.rs`
- `crates/delve/src/rules/lightgallop/world.rs`
- `crates/delve/src/rules/moonfall.rs`
- `crates/delve/src/rules/moonfall/clock.rs`
- `crates/delve/src/rules/moonfall/folk.rs`
- `crates/delve/src/rules/moonfall/ledger.rs`
- `crates/delve/src/rules/moonfall/plan.rs`
- `crates/delve/src/rules/moonfall/says.rs`
- `crates/delve/src/rules/moonfall/state.rs`
- `crates/delve/src/rules/moonfall/story.rs`
- `crates/delve/src/rules/moonfall/town.rs`
- `crates/delve/src/rules/moonfall/walk.rs`
- `crates/delve/src/rules/moonfall/wisps.rs`
- `crates/delve/src/rules/pastimes.rs`
- `crates/delve/src/rules/pastimes/cart.rs`
- `crates/delve/src/rules/pastimes/chariot.rs`
- `crates/delve/src/rules/pastimes/duckshoot.rs`
- `crates/delve/src/rules/pastimes/gander.rs`
- `crates/delve/src/rules/pastimes/halfpipe.rs`
- `crates/delve/src/rules/pastimes/longbow.rs`
- `crates/delve/src/rules/pastimes/powder.rs`
- `crates/delve/src/rules/pastimes/put.rs`
- `crates/delve/src/rules/pastimes/siege.rs`
- `crates/delve/src/rules/pastimes/surf.rs`
- `crates/delve/src/rules/pastimes/trencher.rs`
- `crates/delve/src/rules/pastimes/volley.rs`

</details>

## games_c — 17061 Rust lines, 22 modules

Modules: `race3d`, `joust`, `deathmatch`, `nooks`, `folly`, `pennon`, `brasswake`, `rafterwake`, `flight`, `chariot`, `pitstop`, `spectacle`, `mossbell`, `latch`, `perils`, `votes`, `crossroads`, `colosseum`, `knight_racers`, `chatter`, `race_wagers`, `hippodrome`

<details><summary>files</summary>

- `crates/delve/src/rules/brasswake.rs`
- `crates/delve/src/rules/chariot.rs`
- `crates/delve/src/rules/chatter.rs`
- `crates/delve/src/rules/colosseum.rs`
- `crates/delve/src/rules/crossroads.rs`
- `crates/delve/src/rules/deathmatch.rs`
- `crates/delve/src/rules/deathmatch/combat.rs`
- `crates/delve/src/rules/flight.rs`
- `crates/delve/src/rules/folly.rs`
- `crates/delve/src/rules/hippodrome.rs`
- `crates/delve/src/rules/joust.rs`
- `crates/delve/src/rules/knight_racers.rs`
- `crates/delve/src/rules/latch.rs`
- `crates/delve/src/rules/mossbell.rs`
- `crates/delve/src/rules/nooks.rs`
- `crates/delve/src/rules/nooks/bellows.rs`
- `crates/delve/src/rules/nooks/kitchen.rs`
- `crates/delve/src/rules/nooks/loft.rs`
- `crates/delve/src/rules/nooks/ossuary.rs`
- `crates/delve/src/rules/nooks/ossuary_port.rs`
- `crates/delve/src/rules/nooks/reading.rs`
- `crates/delve/src/rules/nooks/still.rs`
- `crates/delve/src/rules/pennon.rs`
- `crates/delve/src/rules/perils.rs`
- `crates/delve/src/rules/pitstop.rs`
- `crates/delve/src/rules/race3d.rs`
- `crates/delve/src/rules/race3d/drift.rs`
- `crates/delve/src/rules/race_wagers.rs`
- `crates/delve/src/rules/rafterwake.rs`
- `crates/delve/src/rules/spectacle.rs`
- `crates/delve/src/rules/votes.rs`

</details>
