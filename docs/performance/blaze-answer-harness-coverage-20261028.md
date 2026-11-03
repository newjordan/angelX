# Blaze PR candidate: reproducible compiler-answer checks and honest coverage

## Scope and decision

Upstream pin: `b383b0bcf5da33816809b00a37e1c36a0e761efb`.

**Retain an unapplied harness-only candidate; do not install or claim a startup speedup.**

Final patch: `reports/performance/blaze-answer-coverage-iteration24/candidate-r5-upstream.patch`.

This removes a working-directory dependency from existing library questions and exposes previously silent exclusions. It does **not** repair the six installed-engine semantic differences discovered by the newly visible exclusions. It is not an all-reference fail-closed gate.

## Confirmed failure in the existing entry point

`harness/answers.py::check` invokes `LIBRARY` questions naming `src/lib.rs` without assigning a cwd. From an unrelated empty directory it prints 5/5 comparisons and exits zero for the first seven questions: two library references failed and were silently discarded. From a directory with an invalid `src/lib.rs`, all seven compare: rustc's `--print cfg` checks file existence but does not parse that source. Invalid-source contents are not a valid negative control here.

The same empty-directory effect changes the finite base grid from 329 to 327 comparisons without changing exit status. The baseline also leaks its work directory within the runner's owned temporary root; the outer experiment cleans that root. The candidate leaves no such directories.

## Harness changes

- Optional `cwd=None` in `ask`, forwarded to `subprocess.run`; other callers and `--write` stay otherwise unchanged.
- Owned temporary `src/lib.rs` fixture and the same private cwd for reference and engine comparisons; no process-global `chdir` and no ambient source writes.
- Context-managed work/home and answer-data file cleanup, including exception paths.
- Freeze the engine executable before changing its child cwd: explicit relative paths become absolute; bare names resolve against the caller's PATH, including relative/empty components. An entirely empty PATH is passed as `.` because Python's `shutil.which` otherwise treats it as no search path.
- When a child cwd is supplied, anchor relative/empty PATH entries to the caller's cwd so script interpreters and helpers keep their original lookup. Preserve default behavior when cwd is absent and do not mutate the caller environment.
- Count every requested question exactly once as compared, unknown spec, reference-stderr exclusion or nonzero-reference exclusion. Both reference exclusions and each unknown-spec request carry executable, argv, environment additions, stdin, return code and both output streams as JSON. The legacy target-keyed unknown-spec summary remains for convenience.
- Fail a zero-comparison check. Retain legacy reference exclusions when actual comparisons exist, with an explicit warning that exclusions remain unvalidated. Successful engine stderr remains an inherited unvalidated surface, stated in the module documentation.

## Measured selected results

Final real runner: `reports/performance/check-blaze-answer-coverage-real-r6-iteration24.py`; receipt: `reports/performance/blaze-answer-coverage-iteration24/real-r6-receipt.json`.

| Existing questions selected | Baseline | Harness candidate |
|---|---:|---:|
| First seven, empty unrelated cwd | 5/7 requested compared | 7/7 compared |
| First seven, invalid ambient `src/lib.rs` | 7/7 compared | 7/7 compared |
| First 345 (base host/target grid), empty cwd | 327 compared | 329 compared |

Final grid accounting: **345 requested = 329 compared + 10 reference-stderr exclusions + 6 nonzero-reference exclusions + 0 unknown specs**. All 329 compared exit codes/stdout match. All six selected real checks exit zero under the explicitly retained exclusion policy. This is NOT 345 passing comparisons.

Thirty-four same-author stdlib controls pass against the final candidate. The same controls yield 19 failures and 9 errors against the original module (6 pass); some errors are expected API/lookup incompatibilities, not independent product failures. Controls include cleanup, stdin/env forwarding, empty coverage, unknown counts, preserved mismatch detection, explicit executable paths, and real subprocesses with competing executables under relative/empty PATH, plus a real `/usr/bin/env python3` engine script invoking a PATH-resolved helper. They are not an independently authored evaluator.

## Separate, concrete engine PR opportunity

Installed ra-jit, in `RA_JIT_ANSWERS=own` mode, returns success on all six excluded Windows queries for which pinned rustc returns error:

- targets: `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc`, `x86_64-pc-windows-gnu`;
- two query forms per target: `--print cfg` and Cargo's file-name/cfg probe;
- argument: `-C force-unwind-tables=no` in the existing harness's allegedly answer-neutral flag group;
- rustc: exit **1**, stdout containing the requested answer, stderr explaining that the target requires unwind tables;
- installed ra-jit: exit **0**, identical stdout, empty stderr.

Raw paired argv, stdin, environment additions, output streams and statuses are in `reports/performance/blaze-answer-coverage-iteration24/excluded-engine-probe.json`. Do not hide this result by removing the target/flag cases.

Source path explaining the observation: `answers/own.rs::IGNORED_CODEGEN` includes `force-unwind-tables`; `Given::of` ignores it; `answers.rs::answer` synthesizes a successful `Output` with empty stderr for `own_answer`. This is a source-supported explanation, not a compiled candidate-engine fix.

**Next engine design:** stop answering unsupported/invalid option-target combinations as unconditional success. Prefer a precise conservative fallback over inventing cross-target diagnostic strings. In `own`-only validation, a refusal is not rustc-equivalent error reproduction, so define separate fallback and own-only acceptance cases before coding. Compare exit status, stdout AND stderr on both successful and intentional-error cases. A separate engine build and full acceptance are required; none was performed here.

## Retained failures and review

Initial strict reference-error policy failed the real grid because it promoted six intentional invalid-input references to harness failures. The first policy note miscounted those as warnings; raw calibration was correct. Both the erroneous note and correction are preserved. First real driver miscounted the version query and had the wrong expectation for invalid ambient source; its six results survive its failed assertion. A second driver has a retained syntax error. R3 executable resolution passed relative PATH controls but failed entirely empty PATH; R4 corrects it. A subsequent review identified script-interpreter/helper lookup under the changed cwd and duplicate unknown-spec record loss; R5 anchors child PATH and retains per-request records. The first R5 new unknown-spec test accidentally kept the known-spec fixture and failed; the frozen R6 test file clears that fixture and passes without further candidate change. Do not use the earlier candidate/runner names as the final result.

Three separate-context read-only reviews identified the nonzero-reference policy conflict, relative engine path bug, relative PATH bug and incomplete exclusion records; their relevant findings were addressed or explicitly scoped out. The last review predates R5: R5 resolves its two findings and is covered by 34 same-author controls and the final selected real run. No post-R5 independent dynamic review is claimed. Review/audit artifacts accompany the report. They are advisory reviews, not independent dynamic acceptance.

## Remaining upstream acceptance gates

1. Full dynamic CPU/feature enumeration and copied-toolchain questions through unmodified entry-point selection.
2. Linux AArch64 and macOS hosts; check Windows Python path/PATHEXT behavior if that host is supported.
3. Integrate an independently authored regression suite in upstream CI; this patch only changes `harness/answers.py`.
4. Separate engine correction for invalid flag/target pairs and a policy for reference warnings/unknown specs and engine stderr.
5. Operator resolution of the existing angelX lock-seal mismatch before host build/deployment qualification. No protected bytes, seals, installed tools, launcher, Cargo inputs or upstream checkout were changed by this iteration.

No record-time startup, changed-file compile latency, full Blaze qualification or completed long-horizon goal is claimed.
