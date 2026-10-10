# Research labyrinth and lower tunnels

Angel uses the map contract from [Labyrinth Exploration](https://github.com/nasqret/labyrinth-exploration)
by Bartosz Naskręcki. The verified fork is
[newjordan/labyrinth-exploration](https://github.com/newjordan/labyrinth-exploration).
The bundled engine and schema are pinned in
[UPSTREAM.json](../cockpit/research/labyrinth/UPSTREAM.json); the Rust adapter is
[drive/labyrinth](../cockpit/src/drive/labyrinth/mod.rs).

## Start a map

In the workspace whose research you want to retain:

```text
/labyrinth init
/labyrinth status
/labyrinth frontier
/labyrinth plan
/labyrinth route q.my-question th.my-result
/labyrinth route th.start th.goal established
```

`init` creates `labyrinth/knowledge.json`, the Python engine, its dashboard
template, and the complete pinned upstream workflow in `labyrinth/workflow/`.
Existing files are kept. A project with its own upstream engine can
use the bundled engine adaptation after reviewing the changes; an older engine
does not read Angel's observation fragments automatically.

The `labyrinth` tool provides read-only `status`, `check`, `frontier`, `plan` and
`route` actions. It is available alongside research tools in autonomous loops
and in scoped research, code and read-only agent registries. Seats without a
workspace-read grant receive neither the tool nor automatic map context.
`plan` combines doors with
up to twelve recorded attempts, including exact measurement and board receipts.
Referee GAP and FALSE outcomes retain the report and independently executed
check identities. A GAP keeps the question open; neither outcome removes the
question or changes its prior authority. The latest failures also reach the
bounded turn context, and planning can select a changed route for future work.
Inspect failed attempts before repeating them; change the hypothesis, candidate
or test when the previous result already answers the same question.
Edit authored claims, tests, links and referee provenance in `knowledge.json`
with ordinary file tools. `check` on the native surface validates the graph;
`campaign check` validates the graph, event log, numerical frontier, SOTA and
integration journal together. The adapted Python check validates those local
artifacts and refuses pending or inconsistent transactions.

```sh
python3 labyrinth/lab.py check
python3 labyrinth/lab.py build
# Open labyrinth/dashboard/index.html locally.
```

The dashboard includes the knowledge graph, timeline, board and optional
numerical frontier/SOTA data. It uses upstream's browser assets from CDNs.
No dashboard, research text or observation is uploaded by this integration.

## Native research campaigns

The upstream repository supplies an agent workflow, map contract and dashboard;
it does not supply a separate trainable RL policy. Angel executes its literature,
attack, independent referee, writer and coordinator stages with its own agent
harness. The entire pinned upstream repository is bundled, including its skill,
role briefs, campaign and saturation references, examples and tests.

Labyrinth policies are registered in Angel's native book at `⡬`; isolated-role
tools use `⣬`. The same Legend reader, encoded routes and turn machinery used by
Deli resolve these chapters. Adapters send role addresses plus task, source,
budget and output-schema JSON. Task descriptions, source documents and raw
reports remain research data. English policy pages are resolved through the
existing Legend mechanism instead of appended as adapter instructions.

The headless campaign entrypoint shares its implementation with the
`labyrinth_campaign` agent tool and `loop_research action=campaign`:

```sh
angel --labyrinth --workspace /path/to/project campaign start campaign.json
angel --labyrinth --workspace /path/to/project campaign run my-campaign
angel --labyrinth --workspace /path/to/project campaign status my-campaign
angel --labyrinth --workspace /path/to/project campaign check
angel --labyrinth --workspace /path/to/project campaign check BUNDLE.json
angel --labyrinth --workspace /path/to/project campaign cancel my-campaign
angel --labyrinth --workspace /path/to/project campaign recover
angel --labyrinth --workspace /path/to/project campaign recheck my-campaign
```

`run` uses the configured live Angel model. Optional `--driver`, `--model` and
`--effort` selectors choose a concrete supported route; a model selector requires
a driver. Practice output cannot stand in for a model campaign. `start` and all
inspection commands need no model call.

A campaign spec names an ID, task, one to three doors, at least five concrete
attack perspectives per door, explicit `inputs`, fresh `referee_inputs`, and
`canonical_documents`. It also pins `checks` and independent coordinator
`spot_checks`, each as an exact `argv` and `timeout_secs`. Optional bounded fields
control concurrency, model hops, role and campaign deadlines, writer retries,
closed approaches and side projects. The tool schemas expose the full contract;
invalid paths, aliases, unbounded budgets and incomplete specs are rejected.
Omitted limits default to 64 model hops per role, 1,800 seconds per role and
7,200 seconds per campaign. Smaller explicit limits remain authoritative.

| Stage | Executed behavior |
|---|---|
| Literature | Reads primary material and streams source-grounded leads to active attacks |
| Attack | Works the five perspectives, retains executable evidence and records failed routes |
| Referee | Starts with a fresh identity and conversation, reads the exact author artifacts, and writes separate verification code |
| Writer | Receives immutable review material in its own copy, checks before editing, proposes exact OLD-to-NEW edits, and accounts for every correction |
| Coordinator | Materializes the proposed map/SOTA/frontier into isolated copies, runs post-edit and spot checks, and validates source hashes before canonical integration |

Referee own-code checks use a trusted isolated Python interpreter with
`python3 -I -S -B artifacts/independent.py`. Executed checks cannot read sibling
role workspaces or credentials. Source files are read-only during checks;
generated outputs belong in the granted artifact or build directories. Reports,
code, actual process output and source manifests are retained immutably under
`labyrinth/angel/campaigns/`. Roles share the caller's descendant budget and
physical worker capacity; starting a background RL campaign does not reset them.

Exact referee corrections select a claim's `statement`, `test` or `lesson`.
Their OLD text must occur exactly once in that field. Legacy reports without a
field selector require one unique occurrence across those three fields. The
coordinator independently reconstructs these changes and rejects writer drift;
corrections cannot change structural fields such as status or code identity.
Fresh executions retain their output artifacts under immutable check identities
and reject verification code that changes itself while running.
Writer and integrator share a 128 KiB limit for each edit fragment; the complete
raw role report also stays within 128 KiB, final documents within 2 MiB and
canonical transactions within 16 MiB.

Claim-bearing map nodes bind to independently reviewed claims through `claim_id`.
Explicit IDs remain authoritative. When omitted, a matching node ID wins;
otherwise the complete statement must match exactly one reviewed claim. Invalid
explicit IDs, changed text and ambiguous matches stop integration. The native
coordinator records the resolved ID in the checked canonical node and preserves
the writer's original report. This rule lives in the encoded Legend chapter.

`recheck` queues fresh coordinator verification of a complete retained referee
answer or a completed writer's blocked integration after an operational or
protocol failure. It requires an idle, unintegrated blocked campaign and unchanged
report, result, artifact and source identities. It preserves completed model roles,
failed states and prior receipts. Each door allows at most three referee and three
integration rechecks. Integration
replays use new checked identities and copy the writer's immutable source and
artifact snapshots, preserving earlier working copies and receipts. Source
digests order filenames by path components consistently at capture and
validation. Use `run` afterward to execute the fresh checks and canonical integration.
Before publication, `check BUNDLE.json` validates a complete retained bundle using the same source,
document and check-receipt validators as publication. It returns proposed file
hashes against the current canonical preimages with zero canonical writes and no
model call or recheck. Use `campaign check` to validate the workspace journal after
integration. The native tool
accepts `bundle`; `loop_research` accepts `campaign_bundle`. RL rechecks require
the same opaque workspace-write authority as the native campaign tool.

Each stage and escalation has durable state and live events. A stopped run can
resume from committed stages. Nine explicit upstream saturation rungs retain
failed methods and changed questions. Exhaustion, cancellation, failed checks
and incomplete review retain their actual status. A campaign is completed only
after a validated integration receipt exists.

Canonical writes use exact source guards, checked generated JSON and a private
transaction journal. Pending transactions block map readers. `recover` rolls
back only recorded before-or-after bytes and preserves independent user edits;
a conflict remains explicit. Replaying a completed bundle returns its original
receipt without applying old edits again. Authored T1 authority, proof tiers,
SOTA history and numerical frontier constraints remain separate from review.
Campaign completion never promotes an official benchmark submission.

## How the systems share evidence

| Producer | Map record | What later work sees |
|---|---|---|
| Deli's reasoning findings | T5 conjectures with a stated independent test | A door to check; a premise/derivation citation alone does not prove it |
| Deli's hypotheses and unverified loop claims | T6 hunches | A direction to explore |
| Tool-grounded `/loop` findings | T4 evidence, under review | A recorded observation to check independently |
| Acceptance-verifier receipts, Sloptomizer experiments and RL campaign rounds | T4 evidence retaining run ID, source/receipt/evidence digests, route and measured delta | The verdict for that specific attempt; failed promotion is not a refutation of an idea |
| Terminal competition watcher receipts | T4 evidence with exact submission/benchmark identity, official metrics, rejection and promotion fields | Verification and improvement remain separate; repeated polls of the same result deduplicate |
| Explicit local CLI benchmark runs | T4 evidence with source identities before/after, command identity, fresh score and process outcome | A reproducible local measurement; its meaning still depends on the invoked verifier |
| Authored proof/referee results | Upstream T1–T6 and independent review metadata | Established routing requires proof evidence and satisfactory review for T2/T3 |

After initialization, Deli and `/loop` retain new observations without extra
model calls. Agent turns and fresh prompts receive at most three scoped doors,
including curated project questions, without changing the stable system prefix
or adding duplicate context to a turn. Explicit RL experiment candidates receive
the original workspace's context before moving into isolation.
`loop_research context` and `suggest` expose the same
frontier next to the existing relationship/fitness advice. Measured results
continue through Sloptomizer and RL's existing evaluators and promotion gates.

Automatic records are immutable JSON fragments in
`labyrinth/angel/observations/`. Both the native reader and adapted Python engine
merge these with authored knowledge. Curated nodes take precedence; neither
producer edits a curated tier, review, claim or event. Repeated observations
deduplicate; distinct measurements retain distinct receipts. These are local
workspace files, not shared multiplayer state. Automatic observations are Git
ignored so they cannot count as candidate edits or enter RL source snapshots.
Keep unpublished curated research out of
public source commits. Archive/curate observations before reaching the explicit
2,048-fragment or 4,096-node budgets. Invalid maps report an error while the
original Deli/RL work continues.

The native research workspace's Library also shows a cached map overview and
up to 128 linked claims. These rows are always `RECORDED`: authored proof tiers
and review labels do not become native verifier verdicts. Switching workspaces
clears the previous map projection. The activity view recognizes map navigation
as study work.

Native reads never execute `tools/blueprint_data.py`. Projects using that optional
upstream facility must materialize its nodes in `knowledge.json` for native
routing; the Python engine retains upstream blueprint support.

## Run a benchmark through Angel

The headless entrypoint works without a model call:

```sh
angel --labyrinth --workspace /path/to/project plan
angel --labyrinth --workspace /path/to/project run \
  --task "benchmark objective" --idea "reproduce the baseline" \
  --score-file scripts/score.json --direction lower \
  -- python3 scripts/run.py
```

`run` invokes the caller's exact program and arguments in the selected workspace.
It requires a Git workspace, retains command and score digests, and records the
score JSON (a finite numeric `score` plus any verifier metrics). A completed local
measurement requires exit zero, a fresh score artifact, and unchanged candidate
source bytes. A stale score, a failed child process or a source change produces
a retained diagnostic and a failing CLI exit. Use the benchmark's normal ignored
score path so generated receipts do not change candidate source identity.
The command uses Angel's process ownership boundary and never submits to a board.

The benchmark's own checker stays responsible for its verdict. Labyrinth stores
the run's receipt alongside research ideas; it does not replace the checker or
reward a score merely because an agent wrote it.

## Routing and tunnel variants

Research routes distinguish charted corridors, testable doors, dark hunches,
results awaiting review, and refuted walls. `explore` admits unresolved steps at
higher cost and labels every step; `established` admits charted steps only.
An unverified prerequisite keeps a dependent result out of an established route.
`uses` dependencies must be acyclic. `cites` and `refutes` links never create a
walking shortcut. A refutation requires retained evidence to become a wall.

The same bounded shortest-path primitive walks the crawl's actual traversable
cells. Furniture, rock, pits and closed walls stay blocked. Starting at floor
three, fresh Delve floors rotate through three deterministic frontier-growth
grammars:

| Variant | Topology |
|---|---|
| Gallery | Long passages with spurs, favoring fewer early loops |
| Warrens | Branching connected expansion |
| Karst | Compact chambers with alternate paths |

The existing seeded pack/room system supplies monsters, obstacles, secrets,
stairs and visual style. Each new cell connects to existing geography, and
growth consumes a finite frontier, including when the requested map fills.
The first two floors keep their established approach passages and encounter
seed stream. Stored floors retain their existing geometry. These variants apply to the
current Delve; the Undernetwork's 100-stratum atlas remains a separate
world-development plan.

Paid `/loop` excavation also uses the shared research map. When a new iteration
is committed, Angel saves the selected door, bounded route, review outcome and
map digest with that iteration. The saved choice determines a connected room
target, its parent, passage direction, slice axis, rock cuts and room kind.
Replaying the settlement uses the saved plan and checks its geometry; later map
edits cannot change a room that was already paid for. Legacy iterations without
a saved plan keep their existing settlement behavior. Loading a map, saving a
checkpoint or drawing the world cannot create a paid excavation.
Retained failed routes affect later room choices while existing paid room
receipts and geometry stay fixed.

Research passages and physical rooms use distinct state. Research novelty,
visiting a node, generating coordinates and failed experiments grant no loot,
training reward or promoted policy. The current evaluators, shared settlement
transactions and protected encounter gates remain the authorities for those.
