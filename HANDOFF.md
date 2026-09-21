# Handoff — `cockpit/src` layer reorganization

**Branch:** `refactor/src-layers` (branched from `main` @ `c20d455`)
**Status:** structure landed and compiling clean; test suite triage in progress.
**Scratch:** `.refactor-notes/` (untracked — the migration scripts, the bucket
map, and a partial test log). Delete it when this is done.

---

## 1. What was done

`cockpit/src` had 105 loose modules + 25 sibling directories declared as one
flat list of 134 `mod` lines in `main.rs`. It is now seven layered directories,
plus `main.rs` and `sandbox_main.rs`, and nothing else at the root.

| dir | files | what belongs there |
|---|---|---|
| `app/` | 21 | the shell: `App` state, command dispatch, startup wiring |
| `ui/` | 61 | rendering and input — layout, widgets, panes, visualizations |
| `stage/` | 45 | the miniworld and its ceremonies |
| `drive/` | 100 | unattended controllers (loop, RL, competition, campaign) |
| `agent/` | 183 | the model runtime: harness, tools, providers, sandbox, edits |
| `knowledge/` | 20 | ledgers, memory, curated sources |
| `platform/` | 13 | paths, workspace identity, operator-owned policy |

Placement rule: **a module goes in the lowest layer that can hold it.** Draws →
`ui/`. Only computes or persists → `knowledge/` or `platform/`. Runs the model →
`agent/`. Runs unattended across turns → `drive/`.

The exact module→bucket map is `.refactor-notes/map.txt` (one line per bucket,
first token is the bucket name). `.refactor-notes/migrate.py` and `stage_d.py`
are the scripts that performed the move — useful mainly as a record of *which*
module went where, and for the `target()` helper if you need to rewrite more
paths.

GLM's prefix directories from `28d5c17` nest inside the layers unchanged
(`views/`, `viz/`, `dots/`, `term/` → `ui/`; `git/` → `agent/`; `memory/` →
`knowledge/`). Its `agent/` pair was used only by UI code, so it became
`ui/agent_panel/`, freeing the `agent/` name for the runtime layer. `app.rs`
became `app/mod.rs`, so `crate::app::App` still resolves.

**Commits:**
- `db711c7` Group cockpit/src into seven dependency layers (735 files, 438 renames)
- `8954709` Document the cockpit src layers in the README

**Uncommitted:** 16 modified files under `tests/` — the fixes described in §3.
They are *not* yet verified. Do not commit them until the suite is re-run.

**Not mine — check with whoever owns it:**
`tests/cockpit/club/http__glm_image_recovery_tests.rs` is untracked and appeared
*after* commit `db711c7`. It already uses the new `crate::agent::club::types`
path, and nothing under `cockpit/src/` includes it via `#[path]`, so it is
currently an orphan that never compiles. I left it alone. Either wire it into
`src/agent/club/http.rs` or delete it — don't just commit it as-is.

---

## 2. Verified green

- `cargo check --all-targets` — **0 errors**, 25 warnings (baseline was 26; one
  dead re-export in `app/control` was removed). The three
  `unused import: self` warnings are pre-existing at `HEAD`, not from this work.
- `cargo fmt --check` — clean.
- `scripts/check/check-legacy-terminal-boundary.sh` — pass.
- `scripts/check/check-active-connections.py` — pass, 987 source files, no
  missing literal targets.

---

## 3. Test triage — the actual remaining work

A full `cargo test --bins` run was started but **did not finish** (4875 tests;
~2105 passed, 12 failed before the session ended). The partial log is
`.refactor-notes/run1-partial.log`. **The run needs redoing from scratch.**

### First, do this — one failure is an artifact of how I invoked cargo

`target/debug/angel` does not exist. `cargo check` never produces a binary, and
`cargo test --bins` builds the test harness, not the plain binary. Any test that
shells out to the real `angel` binary fails on a missing file. So:

```bash
cd cockpit
cargo build --bins          # MUST run before the test suite
cargo test --bins 2>&1 | tee ../.refactor-notes/run2.log
```

Do **not** run other cargo commands concurrently — the first attempt sat idle
for 20 minutes blocked on the build lock behind a concurrent `cargo check`.

### The 12 observed failures and their status

| test | status |
|---|---|
| `harness::agent_graph::…::reward_binding_cli_round_trips_a_real_episode_end_to_end` | **needs `cargo build --bins`** — spawns `target/debug/angel` |
| `harness::tests::scenarios::scenario_list_dir_and_grep_real_tree` | **fix applied**, unverified |
| `harness::tests::scenarios::scenario_outline_miss_suggests_paths` | **fix applied**, unverified |
| `harness::tests::scenarios::scenario_outline_real_source` | **fix applied**, unverified |
| `tools::file::interruption_tests::patch_exit_drain_completes_active_transaction` | **fix applied**, unverified |
| `tools::file::interruption_tests::patch_exit_drain_sigint_between_real_patch_writes` | **fix applied**, unverified |
| `sandbox::hardlinks::tests::landlock_only_hardlink_write_keeps_outside_unchanged_and_local_links_work` | **fix applied**, unverified |
| `harness::workspace_state::tests::evaluator_execution_pinned_git_ignores_hostile_path` | **fix applied** (its `--exact` child filter), unverified |
| `harness::tests::tool_aging::tool_aging_c03d_long_workload_scripted_smoke` | **unattributed** — in-process, scripted club, no obvious path dependency |
| `harness::tests::web::yolo_http_request_can_mutate_a_local_network_service_without_opt_ins` | **unattributed** — touches local network |
| `tools::embed::tests::semantic_read_ranks_relevant_chunks_and_sends_auth` | **unattributed** — touches network/auth |
| `tools::http_transport::tests::body_progress_resets_stall_without_total_cap` | **unattributed** — timing-sensitive |

The suite had not finished, so **there may be more failures past the point it
reached.** Treat the list above as a floor, not a total.

### For the four unattributed ones — do not guess

They touch network, landlock and git, so they may well be environmental and
failing on `main` too. Establish a baseline rather than assuming:

```bash
git worktree add /tmp/baseline 28d5c17
cd /tmp/baseline/cockpit && cargo build --bins && \
  cargo test --bins tool_aging_c03d yolo_http_request semantic_read body_progress
```

If they fail there too, they are pre-existing and out of scope — say so
explicitly rather than silently fixing them. Remove the worktree afterwards
(`git worktree remove /tmp/baseline`).

---

## 4. Three failure patterns this refactor creates — check for more of each

These are the classes that bit us. If new failures appear, they are most likely
one of these:

**(a) Hardcoded fully-qualified test names in `--exact` filters.** Several tests
re-exec the test binary to run a sibling test in a forked process, spelling out
the name: `"tools::file::interruption_tests::patch_exit_drain_…"`. Every such
name gained a bucket prefix. 17 sites across 15 files were rewritten. To find
any remaining:

```bash
grep -rn '"--exact"' tests/ --include='*.rs' -A2
```

**(b) The three `sandbox__*` test files compile into *both* binaries** — as
`agent::sandbox::…` in `angel` and `sandbox::…` in `angel-sandbox` — so no
literal name is correct in both. They now derive the filter from
`module_path!()` via a `self_test_filter!` macro defined at the top of each
file. Same applies to any `crate::` path inside `src/agent/sandbox/**`: use
relative `super::` paths there, never `crate::agent::sandbox::…`.

**(c) Tests asserting on the real source tree.** `list_dir src` expecting
`harness/`, grep expecting `harness/turn/mod.rs:`, outline reading
`src/lsp.rs`. Fixed in `tests/cockpit/harness/tests__scenarios.rs`. To find more:

```bash
grep -rn '"src/\|cockpit/src/' tests/ --include='*.rs'
```

Note many `cockpit/src/...` strings in tests are arbitrary *fixture data*, not
real paths — those were rewritten for consistency but don't need to resolve.

---

## 5. Mechanical notes (already handled — context if something looks odd)

Every moved file is one level deeper, so anything escaping `src/` needed one
more `../`:
- `#[path = "../../tests/…"]` → 278 files bumped
- `include_str!` / `include_bytes!` → 68 sites
- `include!` → 5 sites (a separate macro; missed on the first pass)

`main.rs` keeps a block of `use <bucket>::{…};` short names under
`#[allow(unused_imports)]`. This is deliberate: the `use crate::*` consumers
(`app/mod.rs`, `ui/term.rs`, `ui/draw.rs`) and the `cfg(test)` tree reach
modules by bare name, and the non-test build sees some as unused.

`tests/cockpit/integration/process_ownership.rs` has a `#[path]` pointing *into*
`cockpit/src` — repointed to `agent/sandbox/process_owner.rs`.

`sandbox_main.rs` now nests its shim modules as `mod agent { mod harness }` and
`mod platform { mod yolo }` so the shared `sandbox.rs` resolves the same crate
paths in both binaries.

---

## 6. Known remaining work beyond the test fixes

- **`tests/cockpit/` still mirrors the old flat layout.** Its subdirectories are
  `app/`, `club/`, `competition/`, `harness/`, `loop_ctl/`, `reinforce/`,
  `tools/`, `world_viz/`, `integration/`. Because src moved one level deeper,
  every `#[path]` prefix got *longer* rather than simpler. Mirroring the seven
  buckets here is the natural follow-up and is independent of everything above.
- **`agent/` is 183 files / ~119k lines**, nearly half the crate. It wants a
  second level: `agent/providers/` (`club`, `openai_codex`, `backplane`),
  `agent/exec/` (`sandbox`, `code_mode`, `mcp`, `lsp`), `agent/edit/`
  (`hashline`, `staged_edit`, `conflict`, `git`). Same for `drive/`, where
  `competition/` alone is 61 files.
- **Inner module style is still mixed** — some directories use `mod.rs`, others
  are sidecars next to a sibling `foo.rs`. The seven bucket dirs use `mod.rs`;
  GLM's newer leaf dirs use sidecars. Not worth churning, but worth a decision.

---

## 7. Final dependency check (for the commit message / PR, once green)

Rows depend on columns. `app` takes 16 inbound edges; `platform` takes 327
inbound against 32 outbound. The `cut → knowledge` placement removed a 105-edge
`harness → cut` backward dependency.

```
             app     ui  stage  drive  agent   know   plat
     app      57    275     23    100    238     86     36
      ui       7    382     37     38     94     47     14
   stage       0     35     76      3      2     22      5
   drive       2     27      2     62    142    163     62
   agent       7     24      0     37    934    245    143
    know       0      1      0     16     60     36     67
    plat       0     11      0      0     18      3      6
```
