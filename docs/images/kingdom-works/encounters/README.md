# Encounter review frames

These PNGs come from the shared native game renderer at 768 × 448. The
`write_claim_boss_review` and `write_ore_threat_review` ignored tests regenerate
their originals in `/tmp/claim-boss-review` and `/tmp/ore-threat-review`.

The warning frames use isolated encounter rooms and advance normal simulation
ticks until an actual attack warning occurs. They keep the review knight alive
to capture the pose; they verify art, anchors and telegraph visibility rather
than difficulty. Obstacles are removed in this test arena to keep the views
readable. Native wall, door and projectile collision remains active.

`ironroot-victory.png` is different: it comes from a starter-bow playthrough
using ordinary movement and fire inputs, with no health, cooldown or enemy-HP
resets. The knight defeats the two iron sentries and Ironroot. The corresponding
integration test then returns to the vein, secures it, constructs and assigns a
mine, and observes the first ore carrier deliver its cargo.

The automated starter playthroughs currently finish the iron, coal and crystal
encounters in 37.8, 28.8 and 45.2 seconds respectively. Total incoming damage is
48, 0 and 52; the crystal run uses the ordinary starter healing potion. These
are deterministic bot measurements in a cleared arena, not human playtest or
procedural-dungeon difficulty claims.
