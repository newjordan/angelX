# Hit-stop recordings

`off-{scene}.json` records the original simulation at 69d7e40a, before
hit-stop, with the same four-shooter harness as `on-{scene}.json`. Each runs
1,800 transport ticks and records save and full wire hashes every 60 ticks.
The new default intentionally changes the on recordings. The review binary's
`--check` flag compares the complete report with these files. Tune metadata
alone is normalized so the added off switch does not count as gameplay.

The six `{scene}-{native|wasm}-on.csv` files record the default with four
seats over 60 ticks: both art kits, normal/rage and painted/plain variants.
Columns are art, rage, plain, transport tick, whole wire, enemies, projectiles,
knights, projectile count, RGB frame. Physics hashes match between platforms.
Warden and Bone Choir have existing pixel differences; the crypt has four
one-ulp differences in display-only stuck-arrow directions in the wire,
including with hit-stop off. Retain each platform's exact hashes.

The original/off byte-stream checks, field-level diagnostic and reviewed
captures are in /work/artifacts/delve-int-20261015/hitstop-1/.

INT-3 re-recorded the three `on-*.json` reports and all six native/wasm ON
CSVs after merging the hit-stop and dungeon finish into
`integrate/delve-art-20261015` on 15 October. The save/wire reports retain
the same values; the CSVs use the merged dungeon/HUD/flight painters. OFF
fixtures remain the original recordings. The integration's OFF checks compare
51,000 old-schema wire/physics hashes against INT-2, all sixteen dungeon
snapshots, 5,639 mini-game trace frames and six final mini-game snapshots.
Only the six added tuning fields and verified default omission are normalized;
every pre-existing field and nondefault tune value remains checked. Evidence:
`/work/artifacts/delve-int-20261015/int-3/`.
