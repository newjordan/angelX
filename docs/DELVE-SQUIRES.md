# The company's squires

Walk into the **Training Yard**, north of the Undercroft. Recruits stand along
its north side; company services are the five plates below them. **Hold E**
confirms, exactly as at Ector's lecterns. No new keys or wire commands.

- **Recruit:** read the figure's name, type, level, HP, damage and treasury wage;
  stand at that recruit and hold E to hire. A taken recruit cannot be bought twice.
- **Pick:** cycle through the shared company, then **rest** (no active squire).
  Each named knight has at most one active squire. A recruit cannot serve two
  knights. Shared ownership matches the existing shared realm treasury and
  retainers; this is not another player character, summon slot, or loot purse.
- **Train:** train the selected squire. Training costs 35 gold × current level,
  consumes practice, and raises health and power. Each selected squire gains one
  practice on departure for a delve. The yard reduces required practice.
- **Board / Hall / Yard:** three independent permanent buildings, each with
  levels 0–3. Upgrades cost 120 / 240 / 360 treasury gold. Board gives 3–6 offers,
  unlocks shieldbearers/alchemists at level 1 and level-2 recruits at level 2.
  Hall increases roster cap 3 / 5 / 7 / 9 and discounts wages by 10 gold per level.
  Yard raises training cap 2 / 4 / 6 / 8 and reduces required practice.

Offers refresh on departure for each delve, not wall-clock time. Names and types
are deterministic. Base wage is 60 + 20 × level, less hall discount. An active
squire returns after being downed: half health after combat or when the party
changes rooms/floors. No permadeath, wages per tick, inventory, or separate loot.
All damage and projectiles credit the owner; normal party loot remains normal.
Squires stay out of deathmatch and do not replace card minions or Beaumains.

| Type | Help | Training unlocks |
| --- | --- | --- |
| Ninja | Fast close fighter, triple backstab against a foe engaged elsewhere; thrown stars at range | L2 smoke/vanish for one second, six-second rest; L3 piercing stars |
| Healer | Keeps distance; mends lowest health ratio nearby | L2 cleanses hex/chill; L3 approaches a fallen knight's uncarried skull and raises at quarter health |
| Ranger | Keeps range; owner-credited arrows | L2 marks a foe, prioritised by other squires; L3 piercing arrows |
| Shieldbearer | Hardy front line, intercepts shots and attracts nearby ordinary foes | L2 shield bash briefly stuns |
| Alchemist | Area bombs with the existing explosion/sound | L2 nearby potion splash mends living knights |

## Implementation

`rules/squires.rs` owns definitions, economy, input plates, AI and validation.
The company lives in `Home.countryside.squires`, so the existing realm's atomic
country settlement/save/rollback path owns payment and persistence. Holds emit
private `country:squire:<revision>:<hex-knight-key>:<action>:<index>` intents;
per-knight revisions make retries harmless and simultaneous seats independent.
`country:squire-day:<day>` is the idempotent departure refresh/practice event.
Both are admitted by the existing checkpoint whitelist. Realm-name keys reuse
`Home::knight_key`, not seat numbers. Public live companions are `Run.squires`;
new save fields have serde defaults and empty fields are omitted.

Movement, hit grace, downing and recovery reuse `hireling::Hireling`/`AllyKind`;
shield attraction plugs into existing monster sight, not a second AI. Healing
uses the party mend limits. No new dependency or randomness source.

`render/overworld/arena/squires.rs` reuses the kit's little squire, with native
pixel type gear, six owner colours, health bars, smoke and target marks. Yard
boards are dark paper with grey pixel borders and one warm accent. No glow.
The five kinds are exhaustive in `Kind::ALL`; add a kind there, its action and
its icon. Focused rules/render tests live in `crates/delve/tests/`.

PNG review: run the ignored `write_squire_shots` test with `ANGEL_ARENA_SHOTS`
pointing at an output directory. It exports yard hiring/upgrades and six-knight
squire scenes at integer scaling; inspect at native size as well as enlarged.
