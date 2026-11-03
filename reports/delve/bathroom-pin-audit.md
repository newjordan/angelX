# Bathroom branch: three `together_tune` pins

## Resolved cause

Compared **d59b1096a** with **a03a72c6a** (the current branch, including
creator/cosmetic-wish changes), using the same `played_with` helper from
`crates/delve/tests/rules/together_tune__tests.rs` in separate builds. The helper
was identical at those commits. No changed gameplay was found in these fixtures.

There are exactly two causes of the failed pins:

1. `Hero::mirror_wish`, introduced in `380ece3f3`, has `#[serde(default)]` but no
   skip-on-false, so **both players now write `"mirror_wish":false`**. This is
   absent in the baseline. `look`/`mirror` and wardrobe additions are omitted at
   their defaults and contribute no differences here.
2. `1c988ce79` extends the saved Alehouse from **36x20 to 44x20**. It is **room
   68**, cell **(16,-12)**, in the kingdom fixture. Only its `cols` and `tiles`
   differ; its other metadata is identical. The knights never enter that room.

These are not a tuning/hitstop/shot-size source implementation bug. Changing
creator/hero code is unnecessary to fix these three pins. There is deliberately
no production-code change in this fix.

## Exact comparison

Captured a full raw serialized `Run` initially and after every step: 901 fight
snapshots (900 steps) and 241 kingdom snapshots (240 steps), independently for
default hitstop, hitstop off, and legacy shot size. **3,426 snapshots total**.
For every snapshot the entire baseline/current JSON agrees after restoring
**only** the paths below. Array lengths also agree; comparisons do not zip away
extra snapshots or fields.

| Fixture | Differing JSON paths | Occurrences per mode |
|---|---|---|
| fight | `/players/1/mirror_wish`, `/players/2/mirror_wish`: absent -> false | each in all 901 snapshots |
| kingdom | same two hero fields | each in all 241 snapshots |
| kingdom | `/dungeon/rooms/68/cols`: 36 -> 44 | all 241 snapshots |
| kingdom | `/dungeon/rooms/68/tiles`: old hall -> annex layout | all 241 snapshots |

Expanded tile comparison, coordinates zero-based:

* Original 36x20 footprint: exactly **(35,5)** and **(35,6)** change from Wall to
  Floor (the two-cell doorway). **All other 718 original tiles agree.**
* Eight added columns (36..43), 160 tiles: **111 Wall, 46 Floor, 3 Block**.
  Blocks are basin cells (38,2), (39,2) and bucket (41,5).
* No enemy, projectile, player position/health/cooldown, RNG, book, room
  metadata, or other serialized state differs. This claim covers these
  deterministic fixtures, not arbitrary tavern/creator gameplay.

The final `written` strings below use all of `played_with`'s existing historical
projections, including expanded tile names and (for legacy) omitted shot beats.
They were captured **before adding the new projection**. Byte-for-byte
comparison after removing the two exact false fields and replacing only the
Alehouse `cols`/`tiles` with the baseline leaves succeeds for **all six strings**.
The old FNV-1a pins are recovered exactly, not regenerated:

| Mode | Scene | Baseline bytes | Current bytes | Baseline / restored FNV-1a | Raw current FNV-1a |
|---|---|---:|---:|---|---|
| default | fight | 103722 | 103762 | `e9202455d90edcc9` | `f7a5d35234fb7e4d` |
| default | kingdom | 599632 | 600843 | `0fc63daa37a1f094` | `1a92f80cc7345064` |
| hitstop off | fight | 103722 | 103762 | `e9202455d90edcc9` | `f7a5d35234fb7e4d` |
| hitstop off | kingdom | 599632 | 600843 | `0fc63daa37a1f094` | `1a92f80cc7345064` |
| legacy shot size | fight | 103720 | 103760 | `dfa1862d0475f928` | `607b6fdaeea7ccec` |
| legacy shot size | kingdom | 599612 | 600823 | `fcf0efddb5d5b296` | `eba83e487c006306` |

SHA-256 of the exact final strings (hitstop off equals default):

| Mode / scene | Baseline | Raw current |
|---|---|---|
| default / fight | `67d81a729e46d7ee4cc336935602eb55228a834b3cde5fe7bfde31ba48abfdb7` | `c9aa1fe6ea765a3a2d394ad98b0238e0c6733f2098337c1e5cd1fbaa225cf857` |
| default / kingdom | `cf9864afde4a851db6411803c89cebc73c0a2e96576ad5e1f4986be92dc97cc4` | `43779ad5a7cbc700948128fdf144556fa89f739cb24166dd7304b41482b47efd` |
| legacy / fight | `0de9a88c02d6300887e0e21d13ab77fd78662a193d2ac039d91fed05d5c7969d` | `97e1708c1613ecee251bbed857cc4daff9baa170032a758da475f69a3cdb95d4` |
| legacy / kingdom | `12b7ca67c5855991584b2e964499ef18faea9bd47246e491e2197012b67af252` | `2a688b6df8611116d0656e72e5fedac461948c5246a7e14f9159e23ff791e626` |

## Regression-preserving fix

`TODAY` and `BEFORE_SHOT_SIZE` remain **unchanged**. All steps still run against
the current room geometry and current rules. Only the final cloned snapshot is
projected to the old fixture:

* Production serde omits the idle `mirror_wish: false` default (the same old-save value); true remains serialized and pinned. No hero field is removed by the fixture.
* Crop only the Alehouse's added columns and restore the two doorway tiles to
  Wall. Preserve every other old tile **from the played snapshot**, plus all
  room metadata, items, roster, chest, and other gameplay state. Do not regenerate
  the whole room or omit it from the hash.
* Assert the fixture isn't playing in the tavern, check the current dimensions,
  and assert the two restored doorway cells really are Floor. This is explicitly
  a historical **non-tavern** fixture, not a test claiming the room didn't change.

A separate geometry test pins all 720 original hall cells against the baseline
layout, explicitly allowing only the two doorway cells, plus all 160 added
cells against the reviewed annex. This doesn't derive its expectations from the
current builder or its dimension/fixture constants. Existing bathroom tests
exercise actual reachability, collision, entry/exit, fixtures, plates and save
round-trip. A projection regression test checks that non-default mirror state,
a changed old hall tile, and room metadata survive normalization and that the
played `Run` itself is untouched.

Do **not** install a 36x20 tavern before stepping to make the hashes pass:
`creator_inputs` migrates old 36x20 saved taverns on every step; that approach
would change the input world and then silently re-upgrade it. No test-mode bypass
of this production migration is added.

## Local evidence and reproduction

Artifacts are under `<build-cache>/bathroom-pin-audit` (not committed):

* `baseline-src`: detached d59b1096a worktree, with only the temporary diagnostic
  instrumentation in `together_tune__tests.rs`.
* `instrument.py`: inserts an opt-in `DELVE_PIN_AUDIT` recorder into the unchanged
  helper and an ignored `rules::tune::tests::pin_trace_audit` test calling all
  three modes. `audited-together_tune__tests.rs` is the instrumented pre-fix
  current helper, retained after removing the recorder from the working tree.
* `baseline/` and `current/`, each with `hitstop-1-legacy-false`,
  `hitstop-0-legacy-false`, `hitstop-1-legacy-true`: `fight`/`kingdom` raw
  `*-ticks.jsonl`, raw `*-initial.json`, and final historical `*-written.json`.
* `compare.py`, `comparison.txt`: exhaustive JSON leaf comparison, then exact
  byte restoration/equality and FNV checks on all six final strings.
  `verify_ticks.py`, `tick-equality.txt`: exhaustive per-tick equality after the
  enumerated changes.
* `baseline-build.log`, `current-build.log`, `original-pins.log`, `fixed-pins.log`:
  separate diagnostic builds, reproduction of exactly the three failing tests,
  and targeted verification after the fix.

To re-run a diagnostic build, from the corresponding instrumented `crates/`:

```sh
DELVE_PIN_AUDIT=<build-cache>/bathroom-pin-audit/<baseline-or-current> \
CARGO_TARGET_DIR=<build-cache>/bathroom-pin-audit/<revision>-target \
cargo test -p angel-delve --lib rules::tune::tests::pin_trace_audit -- --ignored --nocapture
```

The JSONL recorder appends: remove an old output directory before re-recording.
To repeat the exact comparison without a build:

```sh
python3 <build-cache>/bathroom-pin-audit/compare.py
python3 <build-cache>/bathroom-pin-audit/verify_ticks.py
```

Targeted post-fix verification: **9 passed, 0 failed** (the three original
pins, two new regression tests, and the four existing bathroom rule tests):

```sh
cd crates
CARGO_TARGET_DIR=<build-cache>/bathroom-pin-audit/current-target \
cargo test -p angel-delve --lib -- \
  rules::tune::tests::hitstop_default_keeps_the_ordinary_two_knight_baseline \
  rules::tune::tests::hitstop_off_plays_the_old_rules_tick_for_tick \
  rules::tune::tests::shots_as_they_were_play_the_rules_before_the_size_tick_for_tick \
  rules::tune::tests::pin_fixture:: \
  rules::alehouse::bathroom_tests::
```

No full suite or cockpit check was run; the driver's independent fourth failure
is outside this change.
