# Three original ranged heroes — part 1

Base: `acdd152d205416663ef101b78d2c13af5edaa0e6`. Integrated on
`delve/heroes-bar` with the home lamp, free arms stand and unrestricted weapon swaps.

## Survey and integration

- `knights::COMPANY` owns kits; `classes::CLASSES` is the narrower set of
  kingdom hall callings, not an exhaustive list of heroes. Append three kits
  without renumbering older ones or inventing new hall/class aliases.
- Private cards use the existing built-in card book, `feat`, four spell slots,
  spell cooldown/upgrade waits, and zero drop/chest weights. Twelve original
  card vignettes are compiled by the existing card asset pipeline.
- Follow the existing champion pattern: serializable per-hero stance and
  room-owned effects, dispatched after ordinary card/ult casting. New ultimate
  names are appended, not replacements for an older hero's ultimate.
- Basic fire is shared by authoritative simulation and browser `predict_fire`.
  Shared `own_keys` clocks/charge and `own_step` speed retain movement parity.
  Browser held slot input already carries charge hold/release; a test pins it.
  Host alone resolves damaging spell fields. Guest incremental patch dispatch
  accepts the new `heroes` field without full-decode fallback.
- Original ink-row sprites use the existing atlas/Img/native scene pipeline:
  copper scout/rifle, split green hood/reed bow, flame-crowned red robe/staff.
  Standing, stride, facing, attack/cast and charge poses; spell tells, linked
  roots, charge meter, wind trails, cast-stack pips and sniper target reticle.
  No worn-art fitting or `tools/sprite-rig` edits.

## Behavior (30 ticks/second; damage before ordinary hero scaling)

### Iona Farwatch — `farwatch`, 75 HP

- Horizon Rifle: 38 damage, 42-tick base cooldown, speed 48, 28-tick life.
  Every fourth shot deals double damage, deterministically; no random crit.
- **1 Sightline:** narrow, wall-checked shot to 30 units; first enemy takes 120.
- **2 Brass Rain:** radius-three shrapnel field five units ahead, five seconds;
  eight damage every ten ticks and locomotion slowed to 45%. Attacks continue.
- **3 Steady Sights:** three seconds of guaranteed double-damage rifle shots.
- **R Last Light:** selects one visible forward enemy up to 32 units away;
  one-second visible mark then 320 damage. Rechecks range/stone at impact;
  target death does not retarget. No eligible target refunds the charge.
- Original line: “One breath. One horizon.”

### Tavi Reedwake — `galeweaver`, 100 HP

- Reedwake Bow: 13 damage, 18-tick base cooldown.
- **1 Reedwake Draw:** hold 1 to charge (up to 45 ticks); release or maximum
  charge sends a wall-checked piercing line through all aligned foes, range
  24, damage 30–130. Charging moves at 45% speed and suppresses basic fire
  from the first draw tick. Card cooldown prevents repeated retriggering.
- **2 Twin Reed Bind:** one visible forward target within 15, plus its nearest
  living partner within five and an unobstructed link. Roots **both**, at
  their own feet, three seconds (one second if a boss participates). Not a
  stun: AI/attack clocks continue. No second enemy means no root; either
  death breaks the pair. One pair per owner, with a visible connecting reed.
- **3 Windrun:** three seconds, 1.65× movement, complete evasion of touch and
  projectile attacks including projectile chill/hex. Not immunity to fire,
  poison, floors, or unavoidable boss abilities.
- **R Needle Squall:** six seconds of +250% attack rate, locked onto a selected
  forward target within 18. Both host and browser aim at that same living
  target; its death cancels focus rather than silently retargeting.
- Original line: “The wind leaves no footprints.”

### Sera Kilnheart — `cinderheart`, 95 HP

- Kilnheart Staff: 16-damage ember projectile, 24-tick base cooldown.
- **1 Kiln Wave:** broad wall-checked piercing fire wave, range 14, 70 damage.
- **2 Kiln Pillar:** visible radius-2.4 floor tell five units ahead; after 24
  ticks erupts for 85 damage and two-second stun (boss stun divided by three).
- **3 Banked Coals:** radius-three flame field five units ahead, four seconds;
  12 damage every ten ticks.
- **Passive:** each successful spell cast (including learned fourth-slot
  spells) adds +25% attack rate, up to five stacks. Casts refresh six-second
  expiry; after that all stacks fall off. The fire ultimate also adds a stack.
- **R Furnace Verdict:** 420 damage to exactly one visible forward target
  within 18, with a fire beam; no eligible target refunds the charge.
- Original line: “Every spark remembers the kiln.”

## Safety and scope

- Old saves default absent stance/field state to empty. Kit switches forget
  owned fields and reset buffs; room transitions clear everything; owner
  death clears damaging fields/stance. Deathmatch spawn/respawn clears stance.
- Deterministic target ties follow enemy order; no new random state. Projectile
  caps and existing damage/stat scaling are retained. Equipping a forged arm
  or taking a transformed body does not get overridden by the kit's basic arm.
- All three have physical tavern patrons. Farwatch stands west of the bar,
  Cinderheart between the taps, and Galeweaver by the south-west bench.
- No new dependencies, worn-art fitting, or `tools/sprite-rig` changes.
- Balance is initial fixed tuning, not live multiplayer playtesting. Damage
  and fields are authoritative; charge, movement, buffs and fire cadence are
  predicted. Tests cover borrowed ultimates, a learned spell displacing Power
  from key 1, and Desertion suppressing predicted casts.

## Verification

Final integrated verification:

```
CARGO_TARGET_DIR=<build-cache>/heroes cargo test \
  --manifest-path crates/Cargo.toml -p angel-delve --profile fast
```

**2821 passed, 0 failed, 302 ignored**; doc tests: 0 failures.
The explicit ignored PNG exporters also passed (three tests) and wrote shots
under `<scratch>/heroes-bar/shots`. The sprites and home fixtures were visually
reviewed; revisions improved the lamp silhouette, rack bays and casting poses.

The twelve new private cards expand the published book and contact sheets.
Historical tune goldens are unchanged: their existing original-book projection
now also asserts and removes those inert zero-loot cards. Wire catalogue and
exhaustive art-sheet fingerprints were updated for the additional entries;
ordinary mechanics, wire guest equality and all legacy effect pages remain pinned.

No cockpit suite or live play-server deployment was run. See
`<scratch>/heroes-bar/REPORT.md` for the complete integrated delivery.
