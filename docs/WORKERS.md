# Local workers

The book's editable guidance and tool chapters live in `cockpit/book/`. After saving the
navigation TSV or a `.book` chapter, run
`python3 scripts/runtime/publish-book-catalog.py` from the resource bundle. It
validates the complete source set and atomically publishes a manifest of their
hashes. New conversations then pick up that generation; existing conversations
keep their original catalog pages across turns, compaction, and session restore.
Active cartridge overrides still take precedence over catalog pages. While
source files differ from the published manifest, a new generation cannot load
until publication succeeds. For an installed read-only bundle, copy the
resources to a writable directory and point `ANGEL_RESOURCE_DIR` there before
editing and publishing. An older installation with no book catalog assets uses
the embedded baseline; a partial catalog bundle is reported as an error. A
complete older manifest remains readable after upgrading: chapters added in
later versions use their embedded baseline until a complete new generation is published. The
locked advisory chapter is not part of this catalog.

Archived conversations retain the route and page set of their saved catalog;
new routes become available to new conversations. Before code emits a newly
added stamp directly, check how it behaves for an older conversation whose
catalog does not contain that route.

Node.js and Python 3 are required. These commands run one tick; installing Angel
does not schedule background jobs. `/dossier`, `/habits`, `/conductor`, `/cut`
and `/still` read their workers' artifacts.

Successful non-Cargo `run_tests` calls use Node.js with
`scripts/runtime/test-runner-report.mjs` and `lib/test-runner/report.mjs` to
interpret runner output. Edits take effect on the next report. If Node or the
module is unavailable, the result reports that the summary is unavailable and
leaves the reward unlabeled. A failed test-process exit remains authoritative.

`/moa` uses `scripts/runtime/moa-report.mjs` to summarize the current project's
ledger; the native application still owns filtering and writes. The report
requires Node.js and rejects ledgers over 64 MiB or projects over 100,000
records. A worker failure is shown as an unavailable report.

| Worker | Command from the source checkout | Output |
|---|---|---|
| Dossier | `node scripts/runtime/repo-dossier.mjs --refresh` | Repository facts for task context |
| Dossier probes | `node scripts/runtime/dossier-tick.mjs --force` | Refreshed facts and probe evidence |
| Habitsmith | `node scripts/runtime/habitsmith-tick.mjs --force` | Draft skills, approval feedback and usage status |

`--force` bypasses the idle/window check. Kill switches, leases and budgets still
apply. Graph writers share one process lease. Conductor hands its graph and
ledger paths to child workers and reloads their completed writes.

`/cut` runs its read-only report module on demand and requires Node.js. It reads the authored manifest but never prints the
authored bodies. The native host still owns capture and verification; the report
does not change evidence or the graph. Each report has a 10-second deadline,
accepts at most 64 MiB of shard data, skips individual shards over 32 MiB and
ignores lines over 512 KiB. Output is capped at 256 KiB. A missing Node runtime,
invalid module or exceeded limit is reported as unavailable instead of a
partial result. Edits to the installed report module take effect on the next
`/cut` invocation.

`/repos` and `repo_search` use a request-scoped Node module to rank GitHub
metadata and render the shortlist. The native host owns HTTPS, authentication,
metadata admission and the cache; GitHub tokens are neither sent in the request
nor inherited from the environment. The trusted local module runs with the
operator's OS permissions. Each report snapshots its entry point and rule file;
edits during snapshot preparation are rejected, and a running report uses the
copied files. Finish saving both files before making a request: without a shared
generation manifest, a stable pause between the two saves cannot be detected.
A fresh cached result is re-ranked with the current module, so edits take effect
without another network request. Entries created by an older build
may contain only its already-deduplicated results until the cache expires. A
missing or invalid module is reported rather than cached as an empty result.

The shared graph defaults to `~/.angelX/dossier/graph.json`; `ANGEL_CAUSAL_GRAPH`
or `--graph` overrides it. State lives under `~/.angelX/`. Overrides include
`ANGEL_DOSSIER_DIR`, `ANGEL_HABITS_DIR`, `ANGEL_HABIT_PROPOSED_DIR`,
`ANGEL_SKILLS_DIR`, `ANGEL_CONDUCTOR_DIR`, `ANGEL_REFLEX_DIR`, `ANGEL_CUT_DIR`,
`ANGEL_STILL_DIR`, `ANGEL_BARREL_DIR`, `ANGEL_TRAJECTORY_DIR` and
`ANGEL_EXPERIENCE_LOG`.

Habitsmith proposes recurring workflows; `/habits approve <name>` installs a
draft. The next tick folds the verdict and records skill usage for that
repository. `ANGEL_HABITS=0` disables the worker. Dossier runs presence probes by
default; command execution requires `ANGEL_DOSSIER_PROBE_RITUALS=1` and a clean
worktree. Its default allowance is eight probes per day.

Conductor dispatch requires `ANGEL_CONDUCTOR=1`. It dispatches at most one agenda
item per tick, subject to its evidence floor, cooldown and daily budget (two by
default, controlled by `ANGEL_CONDUCTOR_MAX_RUNS_PER_DAY`). Code work also needs
`ANGEL_CONDUCTOR_DRIVER`; it runs in a Git worktree, checks a passing baseline,
and parks passing changes for `/conductor review`. Its default limits are four
iterations, 900 seconds per model task and 3,600 seconds for the entire run,
including build checks. Failed or repeated results do not qualify as progress.

Reflex config experiments and optional post-approval performance measurement
require an external task/verifier runner. Set `ANGEL_BENCHMARK_COMMAND` to a JSON
argument array, such as `["/absolute/path/to/evaluator"]`. The executable reads
one JSON request on stdin:

```json
{"schema":"angel.benchmark_request/v1","request_id":"unique-id","spec":{"suite":"your-suite","seeds":2,"subjects":[{"name":"reflex-baseline","env":{}},{"name":"reflex-treatment","env":{"ANGEL_SPIN_LIMIT":"8"}}]}}
```

It must return a freshly measured report on stdout, echoing the request and
suite, with exactly the requested subjects and a SHA-256 identity of its task
set, verifier configuration and seed schedule:

```json
{"schema":"angel.benchmark_report/v1","request_id":"unique-id","control":{"suite":"your-suite","evaluation_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"metrics":[{"subject":"reflex-baseline","accuracy_pct":50,"accuracy_std":2,"n":8},{"subject":"reflex-treatment","accuracy_pct":75,"accuracy_std":3,"n":8}]}
```

The numbers above illustrate the schema. The evaluator owns task isolation and
independent verification; the [native runner contract](../cockpit/docs/COMPETITION_RUNNER.md)
describes the policy process. Missing measurements, null values, stale request
IDs and failed processes are rejected. Reports persist under
`ANGEL_BENCHMARK_REPORTS_DIR` (default `~/.angelX/benchmarks`). Reflex defaults to
four attempts per day; `ANGEL_REFLEX=0` disables it. It only applies an overlay
after a measured experiment concludes. Conductor compares only reports with
matching evaluation identities and subjects. Its confidence is a graph heuristic,
not a statistical confidence level or a causal claim.

Still replays captured records to an OpenAI-compatible student endpoint, then
asks the configured judge to score teacher quality and the student gap.
`ANGEL_STILL_STUDENT_URL` pins the destination; `ANGEL_STILL_STUDENT_MODEL` and
`ANGEL_STILL_STUDENT_KEY` configure its model/authentication. Judge overrides are
`ANGEL_STILL_JUDGE_URL`, `ANGEL_STILL_JUDGE_MODEL` and `ANGEL_STILL_JUDGE_KEY`.
Without a pinned endpoint, discovery checks the configured local serving URLs.
The default limits are 40 scoring attempts per tick and per day, including
failures; three consecutive failures stop the tick. `ANGEL_STILL_MAX_SCORES` and
`ANGEL_STILL_MAX_SCORES_PER_DAY` control those limits. `ANGEL_STILL=0` disables
scoring while the seven-day barrel expiration remains active. Weekly datasets
are written to the barrel's `distillate/` directory and the trajectories store.
They contain `messages`, `answer`, quality-derived `reward`, provenance and gap;
an external trainer consumes them. Still does not execute a weight-training job.

Runtime assets and helpers resolve from `ANGEL_RESOURCE_DIR`, a source-bound
installed bundle, or the executable's source checkout. The release installation
check installs the bundle with the binary and verifies its availability without
the producing checkout. Pxpipe additionally needs the dependencies from
`npm ci` in that resource directory.

`/science` and `science_search` invoke `scripts/runtime/science-brief.mjs` to
map provider fields, rank results and build the briefing. The cockpit retains
network authority, response limits, citation validation and cache ownership.
Each synthesis pins a private snapshot of the current entry and analysis module
through provider mapping, ranking, and briefing; later edits apply to the next
synthesis. The temporary snapshot is removed when its last user finishes.
Provider mapping runs in one bounded batch after the parallel HTTP requests;
the host validates the mapped records before ranking or rendering citations.
The entry point advertises protocol 2 and source identities for batch results
before use. The exact previously shipped
entry point retains its single-source protocol; other edited older entry points
must implement the capability response before use. Failed batch calls are not
retried or replayed as individual calls.
Both source files are read twice before snapshotting to detect concurrent saves;
this is not an atomic publication protocol for edits spanning multiple files.
Finish saving both before starting a synthesis. Node.js is required; module
failures surface as source or analysis errors and incomplete results are not
cached. New caches retain the pre-deduplication candidates for later reranking;
older caches retain their previous already-deduplicated records until expiry.
