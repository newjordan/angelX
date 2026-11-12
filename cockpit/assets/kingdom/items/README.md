# Kingdom equipment art

Six transparent PNG source masters generated with built-in `image_gen` for
Knight's Wish. Full prompts, image dimensions, checksums and original resource
associations are retained in `manifest.json`. The matching `.card` files define
actual combat behavior. The kingdom gameplay catalog owns production costs,
durations and research unlocks.

| Asset | Intended use |
| --- | --- |
| `iron-oathblade.png` | Broad, fast melee sweep: 28 damage |
| `ember-edge.png` | 28-damage melee; at range, 18-damage returning blade |
| `mooncrystal-staff.png` | Three 12-damage shards in a 24-degree fan |
| `hearth-wardstone.png` | Reusable spell: heal 18, ward 2 seconds, recharge 18 seconds |
| `prospectors-pick.png` | Narrow, heavy 44-damage melee strike |
| `runebound-aegis.png` | Held barrier: width 4, drain 25 mana/s, allies' shots +15% |

All images are 1254×1254 RGBA with transparent backgrounds. Keep the original
alpha when loading. Render with aspect ratio preserved, using a contained
square inventory slot. The build creates compact runtime paintings. The
equipment renderer turns the weapon portraits upright and attaches them to
the knight's existing hand and swing sockets. Each `.card` also includes a
portable ink icon and worn/projectile art for the ordinary card system.

Finished equipment must reach camp stock before a knight can issue it from
the kingdom armory. Issuing consumes one item and records ownership by player
profile. Owned items can be equipped again without spending or creating floor
copies. Selected equipment follows the knight into later expeditions. Repeated
equipping does not heal, refill mana, duplicate spell slots or reset cooldowns.
Bound relics cannot be consumed as cauldron ingredients. Changing the player
in a seat returns the previous player's bound equipment to their own loadout.

The ore encounters use existing combat rules rather than elemental resistances:

| Ore threat and tell | Equipment answer |
| --- | --- |
| Iron: Pavise front plate and drawn-back bash | Pick lands a heavy melee hit in the recovery window; Oathblade sweeps a pair. The plate blocks projectiles, not these melee strikes. |
| Coal: sapper places a blinking keg | Staff reaches the keg from safety; keep away from the blast. |
| Crystal: mirrorling flashes before copying a shot | Aegis stops the returning hostile projectile. Ember Edge's returning blades are excluded from mirrorling copying. |
| Ember: hound marks a charge line and leaves fire | Sidestep the line and attack at range with Ember Edge. Wardstone heals and briefly wards an escape; it grants no permanent fire resistance. |

These are tactical options, not equipment requirements. Starter knight kits
can establish the first claims; workshop equipment expands their choices.

The [equipped-art review](https://github.com/newjordan/angelX/blob/main/docs/images/kingdom-works/equipment.png)
shows the registered paintings and real idle/attack knight frames. Regenerate
it with the ignored `write_kingdom_gear_review` renderer test.

Visual review confirmed complete silhouettes, distinct material cues and no
labels or scenery. Metadata checks confirmed RGBA, both transparent and opaque
pixels, and an unclipped main silhouette in every image. Very faint alpha
fringes are retained from generation rather than destructively stripped.

These six PNGs and their manifest are source-master records. The Armory and
library screenshots are integrated game-renderer captures, while combat
captures exercise the equipped renderer and simulation. Their fixture limits
are recorded in [the screenshot index](https://github.com/newjordan/angelX/blob/main/docs/images/kingdom-works/README.md).
