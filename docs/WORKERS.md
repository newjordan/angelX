# Local workers

Node.js and Python 3 are required. These commands run one tick; installing Angel
does not schedule background jobs. `/dossier`, `/habits`, `/conductor`, `/cut`
and `/still` read their workers' artifacts.

| Worker | Command from the source checkout | Output |
|---|---|---|
| Dossier | `node scripts/runtime/repo-dossier.mjs --refresh` | Repository facts for task context |
| Dossier probes | `node scripts/runtime/dossier-tick.mjs --force` | Refreshed facts and probe evidence |
| Habitsmith | `node scripts/runtime/habitsmith-tick.mjs --force` | Draft skills, approval feedback and usage status |
| Conductor | `ANGEL_CONDUCTOR=measure node scripts/runtime/conductor-tick.mjs --force` | Ranked agenda and briefing |
| Still | `node scripts/runtime/still-tick.mjs --force` | Student scores, weekly datasets and gap history |

`--force` bypasses the idle/window check. Kill switches, leases and budgets still
apply. Graph writers share one process lease. Conductor hands its graph and
ledger paths to child workers and reloads their completed writes.

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
