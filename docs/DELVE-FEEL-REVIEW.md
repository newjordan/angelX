# Delve pressure and arms review — 2026-10-12

Measured against `snap/glm-night-20261012` (`f1099eb8`). The baseline uses only the extended party-size harness from this branch; its game code is unchanged. Both runs use identical seeds, loadouts and bot controls. No monster health or damage value was reduced.

The three arms now deal 72 / 71.7 / 84 nominal damage per second for bow / crossbow / handgonne before cards, misses and piercing. Their speeds are 40 / 70 / 34 units per second. Co-op pressure comes from target assignment, extra bodies, thicker volleys, early-floor head-count damage, shorter repeated controls, local bomb clearing and the revive/economy rules. The fixed tables are in [DELVE.md](DELVE.md).

## Same-seed party-size comparison

Clear time is the mean of successful clears only. Damage is total health lost during all fights, as a percentage of the starting party health pool; healing allows values above 100%. Clear rate includes wipes and timeouts. Room samples use seeds 0–11 with a 150-second limit; boss samples use seeds 0–7 with a 240-second limit. The room selector takes the fight hall with least boss health; some small floors have a chief in every candidate hall.

| Loadout / floor / fight | Knights | Clear time before → after | Damage before → after | Clear rate before → after | Bodies before → after |
|---|---:|---:|---:|---:|---:|
| fresh / 1 / room | 1 | 31.7s → 18.8s | 50% → 31% | 83% → 92% | 6.5 → 6.5 |
| fresh / 1 / room | 2 | 29.9s → 27.1s | 48% → 78% | 92% → 83% | 6.5 → 8.5 |
| fresh / 1 / room | 3 | 35.3s → 32.7s | 42% → 85% | 92% → 67% | 6.5 → 10.5 |
| fresh / 1 / room | 4 | 25.8s → 23.4s | 30% → 86% | 100% → 50% | 6.5 → 12.5 |
| fresh / 2 / room | 1 | 28.4s → 25.5s | 73% → 39% | 75% → 100% | 7.4 → 7.4 |
| fresh / 2 / room | 2 | 30.9s → 44.4s | 54% → 85% | 83% → 58% | 7.4 → 9.4 |
| fresh / 2 / room | 3 | 26.9s → 37.1s | 33% → 87% | 92% → 50% | 7.4 → 11.4 |
| fresh / 2 / room | 4 | 20.5s → 31.7s | 27% → 89% | 92% → 42% | 7.4 → 13.4 |
| mid / 3 / room | 1 | 44.6s → 39.2s | 77% → 72% | 75% → 67% | 8.2 → 8.2 |
| mid / 3 / room | 2 | 38.1s → 34.5s | 87% → 117% | 92% → 75% | 8.2 → 10.2 |
| mid / 3 / room | 3 | 23.2s → 42.7s | 72% → 140% | 83% → 67% | 8.2 → 12.2 |
| mid / 3 / room | 4 | 32.6s → 68.2s | 70% → 150% | 100% → 50% | 8.2 → 14.2 |
| won / 4 / room | 1 | 45.4s → 35.5s | 125% → 127% | 42% → 58% | 12.8 → 12.8 |
| won / 4 / room | 2 | 26.0s → 31.7s | 44% → 105% | 92% → 75% | 12.8 → 14.8 |
| won / 4 / room | 3 | 21.6s → 29.0s | 56% → 96% | 100% → 75% | 12.8 → 16.8 |
| won / 4 / room | 4 | 21.9s → 41.4s | 45% → 107% | 100% → 83% | 12.8 → 18.8 |
| fresh / 2 / boss | 1 | 41.3s → 40.3s | 90% → 90% | 75% → 62% | 3.9 → 3.9 |
| fresh / 2 / boss | 2 | 57.1s → 53.7s | 104% → 127% | 75% → 38% | 3.9 → 5.9 |
| fresh / 2 / boss | 3 | 52.9s → — (no clears) | 91% → 123% | 88% → 0% | 3.9 → 7.9 |
| fresh / 2 / boss | 4 | 50.3s → 112.2s | 82% → 127% | 88% → 12% | 3.9 → 9.9 |
| won / 4 / boss | 1 | 29.3s → 26.3s | 122% → 110% | 25% → 25% | 9.1 → 9.1 |
| won / 4 / boss | 2 | 33.0s → 28.7s | 98% → 148% | 50% → 25% | 9.1 → 11.1 |
| won / 4 / boss | 3 | 45.8s → 30.6s | 101% → 139% | 62% → 25% | 9.1 → 13.1 |
| won / 4 / boss | 4 | 39.6s → 44.4s | 92% → 134% | 62% → 25% | 9.1 → 15.1 |

Fresh uses the stock Percival, Lynette, Gareth and Galahad loadouts. Mid and won use the recorded three-knight fixture; seat four repeats the first saved build. Boss health is unchanged in every paired boss sample (for example, floor 2 x4 remains 6,580); the extra room health comes from additional bodies.

## Limits and follow-up

- This is an aggressive pressure pass. Fresh floor-2 bosses cleared 0/8 times for three bots and 1/8 for four; that is the main human playtest concern.
- These bots never use skills and do not collect tempering upgrades. They use eight-direction keyboard aim, have finite healing, and cannot represent the new Valour/synergy play. Faster successful boss clears can accompany a lower clear rate because the surviving sample changes.
- Fresh four-player room damage rose from 30% to 86% on floor 1 and from 27% to 89% on floor 2. Solo arms feel is faster without changing solo monster stats.
- Manual co-op testing should check fresh bosses, rail readability and handgonne range/recoil before further pressure tuning. The monster pattern library and key bindings belong to their separate branches.

## Validation

Commands run from `cockpit/`, always with `CARGO_TARGET_DIR=/work/build-cache/angelx-feel TMPDIR=/work/tmp`:

```sh
cargo test -p angelX-cockpit --bin angel together_shooter -- --nocapture
cargo test -p angelX-cockpit --bin angel app::control::dungeon::tests -- --nocapture
cargo test -p angelX-cockpit --bin angel stage::world_viz::overworld::arena::tests -- --nocapture
cargo test -p angelX-cockpit --bin angel party_size_sim -- --ignored --nocapture
ANGEL_DELVE_FEEL=1 cargo test -p angelX-cockpit --bin angel together_shooter::feel::tests -- --nocapture
ANGEL_ARENA_SHOTS=/work/tmp/review-shots/feel cargo test -p angelX-cockpit --bin angel write_feel_shots -- --ignored --nocapture
```

The filename-shaped `together_balance` filter matches zero test names; `together_shooter` includes both `balance_tests` and `balance_sim_tests`. The shooter suite passed 591 tests (49 ignored), including both balance modules and all 30 new feel tests. The targeted control suite passed 51 tests (5 ignored); the arena suite passed 12 (40 ignored). With the ledger enabled, all 30 feel tests passed (1 render writer ignored). The render writer passed separately and produced all 13 PNGs. Existing unrelated unused-code warnings remain.

## Rendered PNGs

The ignored writer captures actual simulated shots in an open fight room and asserts that streams, impact sparks and room-end coins exist. Frames are 768×448, inspected visually.

- `/work/tmp/review-shots/feel/stream.png`
- `/work/tmp/review-shots/feel/rail_03.png`, `rail_10.png`, `rail_26.png`
- `/work/tmp/review-shots/feel/fan_00.png`, `fan_02.png`, `fan_03.png`
- `/work/tmp/review-shots/feel/sparks.png`
- `/work/tmp/review-shots/feel/death_1.png`, `death_5.png`
- `/work/tmp/review-shots/feel/coins_00.png`, `coins_01.png`, `coins_10.png`

Raw local outputs: `/work/tmp/feel-balance-before.log`, `/work/tmp/feel-balance-after.log`, `/work/tmp/feel-tests-final.log`, `/work/tmp/feel-controls.log`, `/work/tmp/feel-viz-tests.log`, `/work/tmp/feel-ledger-tests.log`, `/work/tmp/feel-renders.log`.
