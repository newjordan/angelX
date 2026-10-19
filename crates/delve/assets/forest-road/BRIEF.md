# Forest Road — first siege chapter

Run right, jump over fallen logs, shoot the army, free Old Oak.
Enter at kingdom tile (8,1), the existing northern forest path, or the
clearing's northwest edge (x<16 units, y<2.7). The old central return stays open. All knights
share the road, opponents and checkpoints. Each camera follows only its seat.
The target is 3–5 minutes for four seats. The first ten seconds let the player
run and fire before the first scouts enter range. UP jumps (release for a low
arc), FIRE uses the equipped arm, DOWN ducks, DASH rolls. Hold Esc leaves at
any time through the existing personal retreat system. E returns after thanks.

`stage.json` owns the defaults: 7,200 px, ground 184, 2.125 px/tick running,
6 px/tick jump impulse, 0.4375 px/tick² gravity, four-tick coyote time,
five-tick jump buffer, nine-tick roll with 36-tick rearm, 45-tick recovery at
the latest shared checkpoint. Six log hurdles are 36×18. Eight encounter
positions: 720, 1500, 2280, 3100 (war-wagon), 3980, 4860, 5740, 6900 (Feller).
Each ordinary wave has one scout, archer and woodcutter. At four seats each
has 720 HP; war-wagon 7,200; Feller 14,400. Checkpoints refill stage health;
falling restores it after 1.5 seconds. Stage health does not spend dungeon HP.

Class arms retain their damage, cooldown, pellet count, projectile lifetime,
piercing and class colour. World speeds convert at eight native pixels/unit.
Melee uses a 48×36 forward contact area. Projectiles do not hurt companions.
The road has no time limit. The host owns the stage; only integer fixed-point
positions and tick timers advance it. New seats enter at the shared checkpoint and scale the current encounter
up while retaining its remaining health share.

Enemy tells last 60/53/46 ticks by health thirds (at least 1.53 seconds).
They lock their danger rectangle at tell start. Scouts close and strike;
archers loose one ballistic arrow per cycle; woodcutters chop a 128×44 area.
The wagon marks a 64×64 mortar blast. Feller starts with low saw sweeps,
adds an 80×72 mortar at two-thirds health, then alternates a 208×34 wheel ram.
Attacks last 12 ticks, recovery 45, rest 60/50/40. Red borders are active;
ochre borders warn. No bullet fan or near-white core is introduced.

After Feller falls, two 240-HP chain links remain at different heights.
Shoot standing/ducking or aim to break both (melee also reaches both).
The next 24 ticks snap the chains; 66 ticks raise the Ent. Thanks then unlocks
`Home::siege_campaign_begun`, with an idempotent realm settlement mark.
Old Oak appears beside the Dark Forest and speaks on E. This is the hook
for a later chapter; no further campaign stage is implemented.

All new designs remain clearly temporary labeled blocks. `slots.json` records
canvas, bottom-centre anchors, collision geometry and animation timing. Each
slot has an inactive `assets/art-slots/<id>/<id>.map.json`; the ordinary import
pipeline can supply approved `<state>-<index>.png` frames in `v1/`, enable
runtime mapping, and rebuild. Native PNG dimensions must match the slot.
The existing delivered Pennon knight bank supplies player anatomy and colours.
No source-library asset is altered or new design drawn here.
