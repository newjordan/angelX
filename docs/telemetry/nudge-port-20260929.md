# The 0.1.6 advisories, ported into the book (2026-09-29)

Branch `bench/port-nudges`, from `bench/legend-fixes` at 3a64125. Reference:
0.1.6 is source commit `9d73e92`.

## Why

On the DeepSeek polyglot run (thinking off, temperature 0) the 0.1.6 build
solved 26 of 26 of the micro set where the current build solves 12–13 of 18 of
the hard tasks, losing mostly to mechanical probe loops. The 0.1.6 harness spoke
to the model in second-person English at the moments that decide a run. The
book refactor (77dc070, b673787) replaced that with braille routes; several
advisories were compressed to a terse signal, lost their concrete advice, or
were deleted (`nudges.rs`).

## Rules this port applies

1. **Position.** Advice meant to change the next move is its own harness turn,
   never the tail of a tool result (a cue inside a result was read as output:
   0/26 loops broken; the same words as their own turn: 12–18/26). A warning
   leads with `⛔` (the loop-turn sign); guidance is bare cells. Both are
   introduced by the legend, because a stamp that opens a line is recognised.
2. **Novelty.** A repeated cue is absorbed. A route that speaks in full is
   introduced with every page once; a repeat is a page stamp (a new sentence to
   the legend), or, for a lasting loop, the next page of the `⠇⠓` ladder.
3. **Voice.** The pages are the 0.1.6 sentences, unchanged. A test
   (`every_ported_advisory_rebuilds_its_0_1_6_text_from_its_pages`) rebuilds
   each text from its pages, word for word; the clauses left out are named in
   the table below and beside the test's entries.
4. **No stops.** Only the advice is ported. Every clause that said the harness
   would deny, stop or refuse is gone (`FINAL_VERIFY`'s denial threat,
   `TASK_ACCEPT_RED`'s "the harness will stop", the storm/poll "not started"
   results). Nothing here suppresses a call or ends a turn.

## Mechanism

- `book::VOICED` lists the routes that speak in full (with a flag for warning).
  `introduction::entry` gives such a route all its pages (not signal → action),
  noting only the route as known, so a page met later is introduced as a new
  sentence.
- `Raise` can name a page (`Raise::page`) and carry evidence inline
  (`Raise::inline`); `book::advice_turn` builds a turn: warning sign, stamps,
  then inline facts.
- `turn/mod.rs` delivery: loop routes (`⠇`) as before (latch + `⠇⠓` ladder);
  voiced routes as their own turn (`⛔cells` for a warning, `cells` for
  guidance); the rest ride the last tool result's tail.
- The stop checkpoint's turn (`q_stop::stop_turn`) now carries, beside the
  stamps, what 0.1.6's completion notes quoted: the budget left, the failing
  run's tail, the changed files, the receipt.
- New chapters: `⠼` (`d3456_advisories.rs`; every 6-dot cell but `⠼` was in use
  and no chapter had two free cells) and the competition's shelf `⡅`
  (`k_competition.rs`, dot 7 on `⠅`: `⠅` had one cell free and five were
  needed). The table of contents is 45 primaries.
- Escalation names of 0.1.6 (`post_edit_logic_advisory`, `error_advisory`, …) are
  still recorded (`book::legacy_kind`) beside the route names, so the analysis
  scripts keep counting.

## Inventory

Fired counts are from the Sep 23 136-task run unless noted. "Before" is the
book at 3a64125; "now" is this branch.

### `turn/nudges.rs` (0.1.6)

| # | 0.1.6 injection | 0.1.6 text (quote) | Trigger (0.1.6) | Delivery (0.1.6) | Fired | Before | Now |
|---|---|---|---|---|---|---|---|
| 1 | `SPIN_NUDGE` | "You've repeated the same tool call several times with no new result. Change your approach…" | the anti-spin redirect when `ANGEL_SPIN_PERTURB=0` | harness turn | – | – | **not ported:** the ablation form of #2; duplicates it |
| 2 | `SPIN_PERTURBATION` | "You're stuck in a loop, not converging. Break the pattern deliberately: (1)… OPPOSITE hypothesis; (2)… analogy…; (3)… a different tool" | task turn: identical batch, at `spin % 2 == 0` (spin 2) | harness turn | `spin_advisory` 4 (all solved) | `⠇⠁` signal + action only | `⠇⠁` pages = the four sentences, voiced; `⠇⠁` as `⛔⠇⠁` its own turn (already); latch paces repeats |
| 3 | `MANDATORY REDIRECTION` (inline, turn/mod.rs) | "you MUST NOT repeat this call… use `write_file` to rewrite the implementing file cleanly… or `str_replace`… State your new hypothesis and edit the code now." | spin reached the stop (4) — twice, then the stop | harness turn | inside the 4 | absent (ladder had paraphrases) | `⠇⠓⠁`, the first page of the `⠇⠓` ladder: a lasting loop's second showing (spin 4). The four measured ladder pages follow it, shifted one (`⠇⠓⠃`–`⠇⠓⠑`). The stop is not ported |
| 4 | `ERROR_NUDGE` | "Every tool call in your last several turns failed. Stop and read the actual error messages… `list_dir`/`find_files` before reading…" | all calls of a hop errored, `err_streak % 3 == 0` in a task turn | harness turn | `error_advisory` 1 (Sep 21: 16) | `⠭⠁` signal + action | `⠭⠁` pages = the four sentences, voiced; `⛔⠭⠁` its own turn at the third hop |
| 5 | `ERROR CASCADE REDIRECTION` (inline) | "Every tool call in the last 6 hops failed. Stop repeating failing commands. Read the compiler diagnostics above and rewrite the file cleanly using `write_file`…" | `err_streak >= 6` (`ANGEL_ERROR_LIMIT`), twice, then the stop | harness turn | – | absent | `⠼⠊`, `⛔⠼⠊` at the sixth hop; the count starts again as at 0.1.6. **Stop not ported** |
| 6 | `NOPROGRESS_NUDGE` | "You've spent several turns re-reading files…" | – | – | never (`#[cfg(test)]` constant; the churn stop was removed in 0.1.6) | – | **not ported:** never emitted at 0.1.6 |
| 7 | `FIRST_WRITE_NUDGE` | "ACTIONABLE CANDIDATE PROGRESS. Inspection has not yet produced candidate progress. The next tool must mutate… Do not wrap another source read in a build/check." | `ANGEL_FIRST_WRITE_CALLS` (default 0, operator opt-in) free-form inspection calls with no edit; once per turn | harness turn | 0 (off) | absent | `⠼⠙`, opt-in with the same env, once, `⠼⠙` own turn |
| 8 | `RAPID_COMPETITION_FIRST_WRITE_NUDGE` | "RAPID COMPETITION CANDIDATE PROGRESS… a board receipt remains legal…" | #7 in a rapid competition turn | harness turn | 0 | absent | `⡅⠉` (competition shelf), same gate |
| 9 | `FIRST_WRITE_REJECT_RESULT` | "first-write inspection not started — the bounded reconnaissance budget is exhausted…" | first-write rejections (opt-in) | tool result replacing the call | 0 | – | **not ported:** it refuses calls (a cap) |
| 10 | `PASSIVE_POLL_NUDGE` | "PASSIVE WAIT BLOCKED. Passive status/sleep calls in this batch were not started… Status snapshots and shell sleeps are observations, not candidate progress…" | `ANGEL_POLL_GUARD` (default off), passive-only batch | harness turn + suppressed calls | 0 (off) | `⠇⠉` signal + action | the advice sentences (three) are pages of `⠇⠉`, voiced, given when the poll treadmill fires. The "not started" clause is gone with the suppression |
| 11 | `DEEP_PASSIVE_POLL_NUDGE` | "DEEP-SOLVE PASSIVE WAIT BLOCKED… Advance the evidence chain… This poll guard is not a request to submit." | #10 at deep pace | harness turn | 0 | – | **not ported:** the pace variant of #10 (duplicates the ported route; its sentences depend on the suppression) |
| 12 | `PASSIVE_POLL_RESULT` | "passive status/sleep call not started…" | poll guard suppressing | tool result | 0 | – | **not ported:** suppresses calls |
| 13 | `FINAL_VERIFY_NUDGE` | "You edited the workspace but have not run a verifier since the latest edit. Before claiming completion, run the smallest relevant `check`…" | an answer with a verifier owed (`ANGEL_VERIFY_BEFORE_DONE`, default off; the checkpoint now shows `⠧⠋` for any changed code) | harness turn after the retracted answer | – (off) | `⠧⠋` signal + action | `⠧⠋` pages = the five sentences, voiced; at the stop checkpoint (`⠧⠋⠟⠁`), the answer stands afterward. The denial threat ("may be denied up to the configured bounded limit") is dropped with the denial |
| 14 | `TASK_ACCEPT_RED_NUDGE` | "The task's operator-pinned acceptance command is still RED. This is a hard completion contract… Do not redefine, bypass, mask, or replace the command…" | answer with the pinned acceptance red | harness turn + "Latest receipt: …" | – | `⠧⠃` signal + action | `⠧⠃` pages, voiced, at the checkpoint with the receipt inline. "…the harness will stop rather than publish a false-green answer" is dropped with the stop |
| 15 | `FINAL_MILE_NUDGE` | "FINAL-MILE BUDGET ACTIVE. The workspace has changed and the bounded turn is near its horizon. Stop broad inspection…" | a bounded turn (`max_hops`), edited, within `ANGEL_FINAL_MILE_HOPS` (task default 4; 6 for rapid-bounded) of the cap; repeated on inspection-only hops while a verifier is owed | harness turn, once, then each such hop | `final_mile_advisory` 1 | absent | `⠼⠉`, `ANGEL_FINAL_MILE_HOPS` (task default 4, or 6 rapid-bounded); once at activation; on inspection-only hops the repeat is the page `⠼⠉⠑` ("Do not spend the remaining calls re-reading known context.") |
| 16 | `FINAL_MILE_ANSWER_NUDGE` | "FINAL RESPONSE WINDOW. Tool calls are now disabled…" | the answer-only window before the cap | harness turn + tools off | – | absent | **not ported:** it disables tool calls |
| 17 | `POST_EDIT_LOGIC_NUDGE` | "Review the edit logically before testing: trace the state transitions, invariants, cleanup/empty cases, and error paths… run one smallest relevant verifier from a *pre-existing* project test entry point and finish." | the turn's first successful (product) edit, once; `ANGEL_POST_EDIT_LOGIC_REVIEW` default on | harness turn | `post_edit_logic_advisory` **135** | **absent** | `⠼⠁`, the six sentences; bare-cell own turn right after the edit's result |
| 18 | `MUTATION_THRASH_NUDGE` | "MUTATION THRASH. You re-issued the same edit signature multiple times…" | none (`#[cfg(test)]`; the env `ANGEL_MUTATION_THRASH_NUDGE=3` was set by task defaults and never read) | – | never | absent | `⠼⠛`, **wired on the documented setting**: the same edit call issued three times, once per edit, `⛔⠼⠛`. See "Decisions" |
| 19 | `SELF_AUTHORED_VERIFY_NUDGE` | "WEAK VERIFICATION. The green check only ran tests or files you created…" | `ANGEL_SELF_AUTHORED_VERIFY_GUARD` (default off), a green that names only the turn's own test files, once | harness turn | 0 (off) | `⠧⠋` fact only | `⠼⠋`, same gate and once |
| 20 | `PERIPHERAL_FANOUT_NUDGE` | "PERIPHERAL FAN-OUT. Several edits landed under docs/, testdata/, fixtures…" | none (`#[cfg(test)]`; `ANGEL_PERIPHERAL_MUTATION_NUDGE=4` set, never read) | – | never | absent | `⠼⠓`, **wired on the documented setting**: four distinct peripheral paths, none in the code, once. See "Decisions" |
| 21 | `GREEN_VERIFY_DONE_NUDGE` | "GREEN VERIFIER. A full project verifier just passed on this workspace. Use this result as evidence. Complete every remaining requested deliverable…" | `green_verify_achieved` (a completion-sufficient verifier passed after an edit) and not yet told; re-armed by a red run or an edit | harness turn | `green_verify_advisory` 13 | absent | `⠼⠃`, same condition and re-arming |
| 22 | `COMPETITION_WINNER_BANK_NUDGE` | "COMPETITION CANDIDATE VERIFIED. A local check passed on this candidate. Preserve the passing candidate…" | #21 in a competition turn | harness turn | (in the 13, competition only) | absent | `⡅⠁` |
| 23 | `NO_EDIT_ANSWER_NUDGE` | "NO WORKSPACE MUTATION. You claimed progress or completion but no source file was changed this turn…" | `ANGEL_NO_EDIT_ANSWER_GUARD` (default off) with a first-write limit, an answer with no edit, no green; the turn then ended (the advice never reached the model) | pushed as the answer stood | 0 (off) | absent | `⠼⠑`, same gate; now a stop fact at the checkpoint (`⠼⠑⠟⠁`) so the model can hear it and answer again; the answer still stands after it |
| 24 | `COMPETITION_ACTION_POSTURE` | "COMPETITION CHALLENGE PACE — RAPID. ALWAYS BE IMPROVING. Revolving door…" | once at the start of a competition turn | harness turn | – | `⠅⠁` signal + action + ideas | `⠅⠁` pages = the posture, nine sentences, voiced (the engage warpath is already its own turn) |
| 25 | `COMPETITION_WORLD_CARD` (rapid) | "COMPETITION WORLD CARD — RAPID (fill from tools…)" tip / score / slot / status template | once at the start of a competition turn | harness turn | – | removed deliberately (test comment: "standing status is never a reply template") | **not ported:** an approved design; see "Decisions" |
| 26 | `DEEP_COMPETITION_POSTURE` | "COMPETITION CHALLENGE PACE — DEEP. This is a slow-burn solve…" | once, deep pace | harness turn | – | `⠅⠃` signal + action + ideas | `⠅⠃` pages = the four sentences, voiced |
| 27 | `DEEP_COMPETITION_WORLD_CARD` | as #25, deep | as #25 | harness turn | – | removed | **not ported:** as #25 |

### Inline in `turn/mod.rs` and other 0.1.6 files

| # | 0.1.6 injection | Text (quote) | Trigger (0.1.6) | Delivery | Fired | Before | Now |
|---|---|---|---|---|---|---|---|
| 28 | `VERIFICATION RECOVERY` | "3 consecutive verification failures detected. Pause speculative edits… stop micro-patching with str_replace and use write_file to rewrite the module cleanly. Do not re-run tests without changing code…" | three failed verifier runs in a row, once (re-armed by a pass) | harness turn | `verification_recovery` **16** | `⠧⠙` signal + action, on the result's tail | `⠧⠙` pages = the five sentences, voiced; `⛔⠧⠙` **its own turn** |
| 29 | `VERIFIED CANDIDATE CHANGED` | "The workspace changed after a passing check. Preserve the prior candidate…" | a competition edit after a green | harness turn | 0 | absent | `⡅⠃` |
| 30 | unproductive-streak notice | "unproductive streak: N consecutive actions changed nothing verifiable; the verifier's last outcome was X; produce a verified candidate… Scratch files outside the repository (e.g. in /tmp) do not count as progress; edit the target source file directly." (research: "…added no new sources; deliver the answer with supporting citations…NO citations") | streak ≥ 8, then each multiple of 8 (`ANGEL_UNPRODUCTIVE_STREAK_ESCALATE`) | harness turn | `unproductive_streak` 7 | `⠇⠛⠁ ⠇⠛⠃` pages (terse), delivered **to the tray only** (once per turn) | `⠇⠛⠁`/`⠇⠛⠃` pages = the full sentences; the model now hears `⛔⠇⠛⠃ streak=N last=X` at 8 and each multiple. See "Decisions" |
| 31 | `MANDATORY PROGRESS REDIRECTION` | "…You must stop inspecting and stop running unchanged commands. You MUST edit the target source code using `write_file` or `str_replace` before executing any more tools." | streak ≥ 16 (`ANGEL_UNPRODUCTIVE_STREAK_STOP`, task turn, not competition), twice, then the stop | harness turn | inside the 7 | absent | `⠇⠛⠉`, `⛔⠇⠛⠉ streak=16 last=X`, then the count starts again. **Stop not ported.** The trajectory's progress-json/digest diagnosis stays in the trajectory |
| 32 | `duplicate_storm_result` | "[duplicate call suppressed: N×] You have issued this exact `x` call N times… it was not executed again… Do not repeat this call. You must edit the code using write_file or str_replace to fix the issue, or run a different command." | opt-in storm window, 3rd identical call | **replaced the call's result** | Sep 21: 10 | `⠇⠃` signal + action | `⠇⠃` pages: the identical-call sentence (count read as "several"), "Do not repeat this call.", "You must edit the code…". Dropped: "it was not executed again" and "No fresh result was read…" (the call now runs) |
| 33 | `RED_COMPLETION_NUDGE` | "Your last test run on this exact code failed, so the task is not finished, and there is budget left to fix it. Read the failure below…" + "{left}" + last failing run | task-mode answer on code the last red run saw, ≤2 times | harness turn after the retracted answer | – | `⠧⠁` signal + action; evidence in the ledger | `⠧⠁` pages, voiced; at the checkpoint (once); the turn carries the budget line and the failing run's tail inline |
| 34 | `TEST_EDIT_NUDGE` | "You changed test files that came with the task. Those tests are the task's contract: leave them as they were. Restore them (for example `git checkout -- <file>`)…" + "Changed: paths" | task-mode answer with a task test file changed | harness turn | – | `⠧⠛` signal + action | `⠧⠛` pages, voiced; paths inline at the checkpoint |
| 35 | `CONFIRM_GREEN_NUDGE` | "Your last passing test run did not hold: angelX re-ran it on the same code and it failed. The solution passes by luck…" + failing run | the confirming re-run failed | harness turn | – | `⠧⠑` signal + action | `⠧⠑` pages, voiced; the run's tail inline (checkpoint), or as its own turn when the acceptance re-check finds it flaky |
| 36 | flaky acceptance message | "{summary}\nFailing run output:\n{tail}" | the post-hop acceptance check failed after passing | harness turn | – | `⠧⠑` cells only | `⠧⠑` voiced own turn, summary and output inline |
| 37 | `OUTPUT_CAP_NUDGE` | "Your last reply was cut off at the output token limit before it finished, so nothing in it ran… Split the work…" + "(error)" | cut-off reply, ≤3 per hop | harness turn | – | `⠭⠉` signal + action | `⠭⠉` pages, voiced; the error line beside it |
| 38 | `REASONING_CAP_NUDGE` | "…spent its whole output limit on private reasoning… Do not work the problem out in your head: take the next concrete step now with one tool call…" | reasoning-only cut-off | harness turn | – | `⠭⠙` | `⠭⠙` pages, voiced |
| 39 | empty-reply note | "Your previous reply arrived empty — no text and no tool calls were received. Respond now with either structured tool calls or answer text." | an empty reply, once | harness turn | – | `⠭⠑` | `⠭⠑` pages, voiced |
| 40 | raw-markup correction | "Your previous message printed raw tool markup as plain text — no tool was executed, and any results it described were invented. Re-issue the action through the structured tool-call interface now…" | raw tool markup in an answer | harness turn (the answer denied) | – | `⠭⠃` signal + action at the checkpoint | `⠭⠃` pages, voiced, at the checkpoint |
| 41 | "Background work finished while your answer was being generated…" | "Inspect its proc_status outcome and incorporate it before finishing." | a job completed during the answer | harness turn, answer denied | – | `⠏⠉` signal + action | `⠏⠉` pages 3–4, voiced |
| 42 | "Your final answer was deferred because this task still owned running background work…" | "Inspect proc_status and finish from its actual outcome… explicitly stop it with proc_stop… task exit stops remaining jobs." | an answer with owned jobs still live | harness turn, answer denied | – | `⠏⠁` signal + action | `⠏⠁` pages, voiced, at the checkpoint |
| 43 | task job-completion message | "background process [id] name state. Inspect proc_status id=… Process exit is not benchmark acceptance or a verified solve." | a job finished | harness turn per job | – | `⠏⠃`/`⠏⠉` cells only, the receipt in the ledger | `⠏⠃`/`⠏⠉` pages carry the two sentences; `[id] name state` rides beside the stamp (headless turns and the interactive turn) |
| 44 | `research::COMPOSE` | "Research compose step: answer now from the fetched evidence…do not search, fetch, or call tools." | the research turn's final-mile window | harness turn + tools off | – | absent (research ends on its own answer) | **not ported:** a forced tool-free window (a stop/cap) |
| 45 | research completion guidance | "Research completion: deliver the answer with supporting citations…" | research turn preamble | harness turn | – | `⠎⠁` (b673787) | unchanged; already a page set |
| 46 | hop advisor note | model-generated NOTE/BLOCK | `ANGEL_ADVISOR=hops` | harness turn | – | removed earlier | **not ported:** opt-in review by another model; no fixed text |
| 47 | teacher-watch notes | "teacher-watch: context overflow — rolled the tail into a ledger…" | a faulted local seat | harness turn | – | `⠟⠓ ⠟⠊` pages (b673787) | unchanged |
| 48 | post-write check at the answer | the diagnostics text | a failing project check at the answer | harness turn | – | `⠧⠉` fact, evidence in the ledger only | `⠧⠉` at the checkpoint with its diagnostics inline |
| 49 | `WATCHER NOTIFY` | "WATCHER NOTIFY — submission id status=… score=…" | a slot reached a terminal state | harness turn | – | `⠅⠉⠅⠙` cells only, the receipt in the ledger | `⠅⠉⠅⠙`, the receipt line (id, status, score/reason) beside the stamps |
| 50 | relentless directive | "[relentless execution active] Relentless execution to the details: keep taking concrete tool-backed actions…" | armed by the operator, or task default | harness turn | – | `⠗⠁` signal + action | `⠗⠁` pages, voiced |
| 51 | compaction notes, task recon, steer, backplane, deli/tutor/loop frames, swarm stage frames, workspace context header | frames and data | various | harness messages | – | book pages/routes since 77dc070 and b673787 | unchanged: not advisories |

## Counts

- Ported into existing routes (full 0.1.6 English as pages, voiced, own-turn
  delivery where the advice changes the next move): 22 — `⠇⠁ ⠇⠃ ⠇⠉ ⠇⠓ ⠇⠛`
  (5), `⠭⠁ ⠭⠃ ⠭⠉ ⠭⠙ ⠭⠑` (5), `⠧⠁ ⠧⠃ ⠧⠙ ⠧⠑ ⠧⠋ ⠧⠛` (6), `⠏⠁ ⠏⠃ ⠏⠉` (3),
  `⠅⠁ ⠅⠃` (2), `⠗⠁` (1), plus the `⠅⠉` receipt delivery and the checkpoint's
  inline facts (delivery changes, no route).
- New routes: 12 — `⠼⠁ ⠼⠃ ⠼⠉ ⠼⠙ ⠼⠑ ⠼⠋ ⠼⠛ ⠼⠓ ⠼⠊` (9) and `⡅⠁ ⡅⠃ ⡅⠉` (3), in two new
  chapters.
- Not ported: 10 rows (1, 6, 9, 11, 12, 16, 25, 27, 44, 46; and clauses inside
  ported rows 10, 13, 14 and 32 that promised a denial, a stop or a suppressed call) — with the reasons in the table: stops/caps or call suppression
  (9, 12, 16, 44), duplicates (1, 11), never emitted at 0.1.6 (6), removed by an
  approved design (25, 27), not a fixed text (46).

## Decisions for the operator

1. **`⛔` for warnings only.** Loop and verification-recovery style advice
   (`⠇⠁ ⠇⠃ ⠇⠉ ⠭⠁ ⠧⠙ ⠼⠛ ⠼⠊ ⠇⠛`) leads with the sign; guidance
   (`⠼⠁ ⠼⠃ ⠼⠉ ⠼⠙ ⠼⠓ ⠼⠋ ⡅⠁ ⡅⠃ ⡅⠉`) is bare cells. `book::VOICED` is the one list
   to flip. Measured only for loops.
2. **The unproductive streak now reaches the model.** The current code said the
   notice was tray-only because pushing it "re-fires every 8 hops, bloats the
   prompt" (the polyglot slowdown). The operator's list counts `unproductive_streak`
   among the advisories to port, and 0.1.6 sent it, so it is sent at 8 and at
   each multiple of 8; the redirect (16, task turn, not competition) resets the
   count. The old test that asserted tray-only was rewritten
   (`r03b_unproductive_dialogues_escalate_stop_and_reset`). Revert = drop the
   push in `handle_unproductive_streak!`.
3. **World cards stay gone** (`⡅` has no card route). A test pins that no page
   contains one.
4. **`⠼⠛` mutation thrash and `⠼⠓` peripheral fan-out are wired although
   0.1.6 never fired them** (their constants were `#[cfg(test)]`; the task
   defaults set `ANGEL_MUTATION_THRASH_NUDGE=3` and
   `ANGEL_PERIPHERAL_MUTATION_NUDGE=4` and nothing read them). They fire at
   those two numbers. If "same conditions" means only what fired, delete the two
   observe calls in `turn/mod.rs`; the pages stay.
5. **Error advice at the third hop in every mode.** 0.1.6 was 3 in a task turn
   and 2 elsewhere (`error_stop / 2`, minimum 2); the current fixed number is 3.
6. **Two new chapters** (`⠼`, `⡅`) because every other chapter was full.
7. **Repeated advice.** After the sixth failed hop the count starts again
   (as 0.1.6 did), so `⛔⠭⠁` returns at the ninth; a lasting `⠼⠉` repeats as a
   page (`⠼⠉⠑`) rather than as a bare route.
8. **Checkpoint turns now hold data.** The stop checkpoint's turn is the
   warpath plus the failing run's tail, changed files, receipt and the budget
   left. `ledger::is_warpath_message` reads the first line, so summaries still
   drop it.
