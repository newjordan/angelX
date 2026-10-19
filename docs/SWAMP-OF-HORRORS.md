# The Swamp of Horrors: save the white horse

A new mission area (play-test ask, 2026-10-17): the party goes through the
Swamp of Horrors and saves the white horse.

## Shape

The surface stays free of combat (countryside.rs retired the Risen for
that), so the horrors live in a delve: an entrance plate at the Wetland
leads into a new swamp delve, the horse is penned in its last room, and
once freed she joins the realm's herd.

## To do

Rules
1. Hook: a notice at the Wetland, or one of Wren's bounties (bounties.rs),
   says a white horse was taken into the swamp. The realm map already has
   `Place::Swamp` (world/kingdom.rs:38).
2. Entrance: a travel plate at the Wetland (world.rs `ENTRANCES`,
   `Dest::Delve(Pack::Swamp)`).
3. `Pack::Swamp` (layout.rs:203 and every site that matches on Pack):
   rosters from Leech, Slime, Spitter, Wraith, Hexer and Whirlgrave; the
   floor ramp and `BOSS_ALONE_TO` apply.
4. Terrain: `Tile::Bog` (slows feet, not shots), gas seeps (perils.rs), fog
   patches (the Lights Out mode's darkness, local), reed islands.
5. Horrors: two or three new swamp monsters (a bog hand, a drowned knight,
   a luring will-o'-wisp), fixed numbers, a readable tell each.
6. Boss: the Mire King (working name), a `.boss` file with `where swamp`,
   holding the horse in the last room.
7. The rescue: the horse's pen on the rescues.rs pattern (hold E), mark
   `rescue:white-horse`, a feat, a cue.
8. After: she joins `Stable.herd` (realm/chivalry.rs) as a named horse
   with a new `coat` field (white), grazes in the Pasture (ranch.rs),
   races and jousts; riding her is a later step.
9. Saves: new fields default; `valid()`; the marks join INTENT_LIMIT,
   `intent_key` and `intent_valid` (countryside.rs).

Art (Codex, the art team)
10. Swamp ground, bog water, reeds, dead trees, mist.
11. The horrors and the Mire King, Metal Slug era in our dark palette.
12. The white horse: pasture and joust coats; a frightened pen pose.
13. The pen, the map plaque icon, the establishing plate.

Sound
14. A swamp track from Openverse, license recorded (no per-area music yet).

Proof
15. Tests: the route is reachable, the rescue, the herd, old saves load.
16. A party sim of 2 to 4 knights on the swamp floors.
17. A play-test on the server.
