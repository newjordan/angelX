# Running a game's rules as a Bend module

Pilot: the Delve's Bone Dominoes nook (`crates/delve/src/rules/nooks/ossuary.rs`,
112 native lines). Its per-tick rules now also run as the Bend policy leg
`cockpit/live/delve-ossuary`, byte-for-byte equal to the native code under test.
The native code stays as the oracle and the fallback. This is the pattern the
~116k lines of `crates/delve/src/rules/` should follow, and its measured limits.

## Verdict

- Shape: a `policy` leg called once per tick with the rule's state passed in and
  out. Not a `system` module. Rust keeps the authoritative `Run`.
- Correct: 24,000 randomized lockstep ticks plus scripted edge cases agree with
  the native rules on every tick; three deliberate bugs in the Bend were caught.
- Fast enough for one live state machine (about 0.13% of a 33 ms tick), not for
  every rule on every tick, and not for per-entity loops. See "Frame budget".

## Shape, and why not a system module

| | policy leg, state in/out (chosen) | `system` module (api 2) |
|---|---|---|
| per tick, 1 seat, p50 | 38-40 us | 380-435 us |
| per tick, 4 seats, p50 | 43-44 us | 475-518 us |
| who owns the state | the `Run` (saves, rollback, host edits just work) | the Kernel checkpoint; the `Run` must be resynced after every host edit |
| hot reload | next tick uses the new generation; no migration, state is Rust's | needs `migrate`, schema, handoff |
| fallback | native tick runs, nothing half-applied | stage/commit is two-phase, but a fallback tick or host edit leaves the checkpoint stale (the test brain restates the line when it differs) |

The system shape costs about 10x because `Bundle::system_script`
(`crates/live/src/bundle.rs`) carries no `CALL_MARKER`, so the evaluator rebuilds
a fresh context and re-runs the module bootstrap on every call, while policy
calls reuse one warm context per generation. Giving `system_script` the same
cached path would likely close most of the gap (not measured). Even so, a game
rule's state is already the game's save state: two owners buy nothing.

## Layout

- `cockpit/live/delve-ossuary/`: `module.json` (api 1, kind `policy`, `probe` = one
  frame), `ossuary.bend` (the rules), `LAWS.bend` + `PROOF.bend`, `index.js`
  (adapter only: JSON to Bend values and back), `ossuary.compiled.js` and
  `bend-build.json` (from `node scripts/live/bend.mjs build <dir>`; commit both).
  Shipped in the binary like the other legs; add the 7 paths to
  `scripts/release/release-evidence.mjs`.
- `crates/delve/src/rules/nooks/ossuary_port.rs`: the seam in the rules crate.
  `nooks.rs` gained one dispatch: `if !self.tick_ossuary_brain(..) { native }`.
- `cockpit/src/drive/together_rules.rs`: the cockpit half, picks the generation.
  `ANGEL_BEND_RULES=1` arms the newest live generation for every sim thread
  (read in `delve_settings_from_env`). Unset, nothing changes. The separate
  rules-worker process (`ANGEL_DELVE_SYSTEM`) has its own rules and is not reached.
- Tests: `tests/cockpit/drive/together_rules__tests.rs` (parity, measurements),
  `crates/delve/tests/rules/ossuary_port__tests.rs` (seam, no V8 needed).

## What crosses, per tick

In (`OssuaryFrame`, 162 B at 1 seat, 339 B at 4): the rule's own state (`running,
done, fallen, next, stalled, raised, score`) and one row per playable knight:
`{id, pressed, at_start, at_bone}`. Rust answers the geometry (distances against
the bone the line has reached) so Bend never sees a float position.
Out (`OssuaryStep`, about 110 B): the next state and an **ordered effect list**:
`sound`, `say` (banner), `raise` (monster at a tile point), `grant` (coin),
`crown`. Effects are enum indices; Rust owns every sound name, every word of text
and every monster body. Order is the native order, so `sounds`, the banner and
spawn order match exactly.
Rust checks the whole answer (`OssuaryStep::admit`) before writing anything:
state bounds, ids that exist, the raise count equals the `raised` delta, grant
and effect-count caps, positions inside the room. Any error, timeout, undecodable
or inadmissible answer returns false and the native tick runs for that tick.

## What stays in Rust

The `Run` and its save; the `1 << id` press edge detection (`tick_nook`); who is
playable; every distance test; sound names, banner words; spawning, gold purses,
laurels; the RNG (this rule uses none; a rule that rolls dice should take the
roll in the frame, or an index into rolls Rust already made, never a seed).

## Measured cost (1,260 ticks per cell, 7 cycles of stall and carry)

Shared 32-core box at load ~13, `--profile fast` (opt-level 1 for delve), so
absolute numbers are indicative; three runs agreed within the ranges shown.

| per tick | native | Bend policy leg | Bend system |
|---|---|---|---|
| 1 seat p50 / p95 / max | 31-40 ns / 50 ns / 1 us | 38-40 us / 54-63 us / 0.25-0.6 ms | 380-435 us / 0.7-0.8 ms / 1.6-2 ms |
| 4 seats p50 / p95 / max | 40-60 ns / 60-100 ns / 1.2 us | 43-44 us / 85-93 us / 0.2-0.65 ms | 475-518 us / 0.72-0.9 ms / 1.4-1.7 ms |

Policy leg split per call: JSON encode 1-2 us, V8 call 39-52 us, decode ~1 us.
An empty leg (swarm-route, no work) costs 21-25 us p50, 30-41 us p95: **more than
half of a call is the boundary, not the rule.** The module's own work is 0.1-0.4
us per call JIT-warm in Node; the embedded isolate spends 15-25 us more than the
empty leg on it (not profiled). Only 105 of 1,260 calls (1 seat) and 189 of 1,230
(4 seats) carried a key press, an effect or a state change; the rest were timers.

## Frame budget

`HZ = 30` (`crates/delve/src/rules.rs`), so 33.3 ms per tick. One live nook is 44
us p50, 93 us p95: 0.1-0.3%. Scaling it naively: 100 rules calling every tick is
4.4 ms at p50 (13%), 9 ms if each call ran at its p95, and as `system` modules about
50 ms, over budget. Per-entity calls (300 monsters x 40 us = 12 ms) are out.
Working rule: at most ~3 ms (9%) of module calls per tick, about 60 policy calls at
p50. Fixes, in order of value (none measured beyond the counts above):
1. Gate by activity, as this does: call only while the activity is on (`Activity::on`).
2. Wake-driven calls: have the step return `wake_in` ticks; call again on a key
   edge or at the wake. Here that is 8-15% of ticks, a 7-12x cut.
3. Batch: one call per tick carrying every live frame, paying the 21-25 us floor once.
4. Keep hot inner loops (collision, pathing, projectiles, per-entity AI, the
   distance tests) as Rust primitives; modules decide, Rust integrates.
5. Bend's C target is a later option for numeric kernels; it is not behind this
   loader. Cache the `system_script` context if system modules are wanted.

## Recipe: port another rules file

1. Read the native tick. List state it writes, world it reads (positions, hp), and
   world it writes (sounds, banner, spawns, purses, laurels).
2. Define the frame: state subset + one row per entity + Rust-computed features
   (booleans, counts, indices; never raw floats if a threshold decides). Define one
   effect variant per kind of world write. Copy `ossuary_port.rs`.
3. Write `admit()` from the invariants the native code relies on. Add the dispatch
   line to the caller; leave the native function untouched.
4. Copy `cockpit/live/delve-ossuary/`. Write the rule in Bend:
   - state is a record; thread fields as `+` params; no `if`, so fork with a helper
     that matches a Bool; no `match` on a computed value; a def must come after
     what it calls; no mutual recursion (fold with `Bool.pick` instead); `List.append(&2, T, a, b)`.
   - symbolic `U32` arithmetic is stuck in the checker: laws use concrete counts.
   - saturating math and `%` are explicit: `Bool.pick(U32, U32.is_gt(n, 0), (n - 1 : U32), 0)`.
5. `node scripts/live/bend.mjs build cockpit/live/delve-<x>` (0.2 s). Fix until it prints `ok`.
6. Copy `together_rules.rs` and the parity file. Parity = two `Run`s from one seed,
   same scripted acts, one native, one through the brain, compared every tick
   (nook, players, enemies, sounds, banner, cues, shake) and whole-`Run` JSON every
   20 ticks. Scripts: everyone idle, each branch by hand, simultaneous presses,
   knights out of play, a random sweep of 5 seeds x 1-4 seats x 1,200 ticks with
   coverage asserts. Heal knights for half the seeds so deaths are also covered.
7. Mutation check: flip one constant in the `.bend`, rebuild, expect red (tie-break,
   stall length and a bone coordinate all failed here), then restore.
8. Measure with the ignored bench (copy it): floor, native, policy, payload bytes.
9. Gates (`c.sh` is a local wrapper that runs `cargo` in the named workspace with a shared target): `c.sh crates test -p angel-delve --profile fast --lib nooks`,
   `c.sh cockpit test --profile fast --bin angel -- together_rules every_shipped_leg a_hot_edited_leg`.
   The ignored `an_edited_module_takes_over_a_running_line_and_keeps_its_state`
   (needs the compiler) rebuilds the rule mid-line: the generation bumps and the
   line keeps its bones.

## When the rules interleave with the body: Knights Wish

`cockpit/live/deathmatch-rules` moves a whole game (the Delve's first-to-ten
free-for-all) into Bend, and its Rust is no longer a rules file. That game cannot
be one call a tick: a tick walks the knights in seat order, and each hit changes
who is alive for the next, while the hit itself runs the Hero's kit (`hurt`,
`dress`, shields, cards, shots), which is shared with the whole Delve and stays
Rust. The shape that works:

- Rust keeps the bodies (the `Run`, positions, every distance and sweep, flight,
  the collision grid, sound names, the saves) and the order a tick walks them in.
  Wherever the oracle's code branched on match state it now asks the leg a small
  question and applies the answer: `begin`, `turn` (per knight: timers, respawn,
  burn, slam, rot), `spawn`, `hit` then `struck` (blow, then what it scored),
  `grab` (pickups), `arm` and `armed_end` (may they attack; what attacking breaks),
  `landed` (a shot's stun, chill, burn, burst, chain, leech, cloud), `close`
  (fields and the kill feed), `sync` and `wrap` (late joiners, sounds, the pot, the
  podium). About 11 calls a tick at four knights.
- A question is a JSON object whose keys are Rust's own field names, built with
  `pick!` and applied with `put!` (`deathmatch/leg.rs`), so the adapter has no
  mirror structs; enum values travel as their names; `index.js` only builds the
  Bend constructors from the keys and turns zero-field constructors back into names.
- The floats stay Rust: the leg compares two distances Rust measured (the spawn
  choice) and carries positions through untouched, but no sine, hypot or sweep runs
  in Bend (`f32::hypot` and `sin` are libm, not `Math.fround` of a double).
- Order-sensitive details are rules too: the champion's wind-up and rot run down
  *after* the walk (a slam roots the knight on the old count), which is why `turn`
  returns them apart.
- The native rules are the parity oracle in `tests/cockpit/drive/deathmatch_oracle.rs`
  (same bodies, methods renamed `native_*`), compared tick by tick with the leg.

## A whole game: Griffinwing

`cockpit/live/griffinwing-rules` runs the whole of Griffinwing's tick (the flight, the
fight, the three beats' scripts, the Wyrm, the seating, the tally) and a bot's hands, about
3,600 lines of Bend for the 3,000 lines of Rust it replaced, which are gone from
`crates/delve`. The Rust that stays is the session's types, its saves and wire, the
pictures' reads and a short adapter: `Session::step` and `Session::bot_with` hand the
session itself (`serde_json` on the types the Delve already saves) to
`scripted::step("griffinwing-rules", ..)` with the keys, who is at the table and the five
numbers, and take back the next session, the sounds (by index) and who left the tally.
`valid()` admits the answer; a missing engine leaves the session as it was.

- `index.js` is the only marshalling: it lays the session JSON into a frame of 4,332
  signed words, in the order of the `layout` section of the Bend, and reads the next one out.
- The frame is held as `Mem`, a persistent quad trie of 2^14 words (a read costs no
  threading, a write is a new `Mem`, all-zero stretches are shared). Entities are rows in
  tables with a count word; `retain` and row removal are templates and word copies.
  Every number is an integer on the 30 Hz sim, so nothing needed an f32; what is wider than a
  word (a squared length, a product) goes through `Nat`, exact to 2^48, and `isqrt` takes a
  float guess and corrects it with exact steps.
- Quirks of the Rust that the Bend keeps on purpose: `fly` works on a copy of the craft
  that is written back whole (so Starfall's kills are scored on the craft as it stood
  before the tick and then overwritten), `step_foes` runs with the foe list taken out (so
  nothing is "latched" while a kite chooses its victim) and `step_boss` with the Wyrm taken out.
- The native tick is the oracle (`tests/cockpit/drive/griffinwing_oracle.rs`, a trait on
  `Session`, answering the module's two ops). `crates/delve`'s own tests tick through it
  (`install_for_test`), and `tests/cockpit/drive/griffinwing_rules__tests.rs` ticks the
  shipped module beside it and compares the whole session every tick.
- Limits met: a module generation (every `.bend`, `.js` and `.json` in the directory) is
  512 KiB, so source plus compiled output must fit (every def is also exported, so a def
  costs about 150 bytes of JS beyond its body); a result is 64 KiB; the build adapter wants
  the compiled module to end in its exports, which a constant `match` table (emitted as
  `const TAB_n = [..]` in front of them) breaks, hence `word` in the script data.
- Bend 2 notes: a `let` may not precede a `match` on a parameter; scrutinees follow binder
  order; the shrinking argument of a recursion must come first (put the `Nat` fuel before
  the `Mem`); `A & B` is not `Data`, so a function hands a second result back in a register
  word; a template's function type spells its quantities (`@+m: Mem -> ..`).

## Open

- The seam is per-rule Rust (~250 lines beyond tests). Extract the brain slots,
  tally and lockstep `Pair` into generics before many files are ported.
- The Bend is 3x the Rust (314 vs 112 lines) from `+` threading; a `State`
  getter/setter idiom would shrink it.
- The embedded rules worker process is not wired; it needs the same brain.
