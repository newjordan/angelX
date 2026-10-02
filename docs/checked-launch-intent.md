# Checked native launch intent

Native interactive entry accepts workspace, route, concrete model, effort, and one
initial input. Explicit selectors are mandates: unsupported intent fails instead
of selecting another route/model or discarding an option. Ordinary unpinned launch
behavior remains distinct. This source interface does not install an external
subscription adapter or qualify a public release.

## Entry syntax

```sh
# Composer text only; no initial user turn.
angel --workspace ./project --draft '  /quit is text  '

# Literal first user turn on an exact native subscription route.
ANGEL_OPENAI_CATALOG_SOURCE=pi-store:/absolute/path/models-store.json \
  angel --workspace ./project --driver openai --model MODEL_ID \
  --effort pi:high --prompt 'TASK'

# Directory-only source-launcher convenience remains supported.
bin/angelX ./project --draft 'Text to edit'

# Existing resume grammar; do not combine with initial input.
angel --resume
```

`openai` is the subscription route, not `openai-api`. `--model` requires an
explicit `--driver` and exact model ID. Interactive `--effort` accepts native
levels or Pi tags with a Pi catalog. Headless `--reasoning-effort` retains its
separate bare-level grammar. Catalog metadata does not prove account eligibility
or transport compatibility.

Argv is checked before terminal and route construction. Unknown/trailing tokens,
duplicate selectors, conflicting initial input and initial input with resume fail.
Values that look like options remain data. Initial text must be UTF-8, nonblank,
and at most 256 KiB; OS argv limits can be smaller. Selectors and resume IDs are
bounded at 1,024 bytes. Diagnostics do not echo initial text. Argv and shell history
are still visible through ordinary OS mechanisms: these are not secret-input flags.

Workspace precedence is CLI > invoking `ANGEL_WORKSPACE` > original cwd. Relative
roots resolve against the invoking cwd, must be existing directories and are
canonicalized. Tools, instructions, sessions and project state use that root;
sourced launcher configuration cannot redirect it afterward. Helper/headless modes
continue to own their grammar. Prompt contents never choose a helper mode.

## Binding and first-turn lifecycle

Captured CLI effort takes precedence over ambient effort, saved THINK and
`ultrathink` for the initial bound turn. Applicable provider pins also survive
ordinary preference restoration. A pin for another provider does not globally
block saved preferences.

For an explicit CLI subscription model with no explicit/ambient/configured effort,
a nonblank catalog default is used when present; otherwise the native fallback is
retained and checked against that model. No empty default is invented. Unsupported fallback still fails, rather
than silently clamping to a different level. Explicit effort is never replaced by
this defaulting rule.

- `--draft` copies exact composer text, with no initial enqueue or slash-command
  interpretation. Deliberate later Enter uses ordinary composer behavior. Startup
  warming/probes are unchanged; draft mode does not promise networkless startup.
- `--prompt` creates process-local pending text and performs one literal native
  enqueue after startup/workspace/route admission. Whitespace, Unicode, newlines
  and slash-looking text are preserved as a user message.
- Enqueued, cancelled and failed states do not automatically replay. This is one
  native enqueue per fresh invocation, not exactly-once remote execution across
  retries or crashes.
- Escape before dispatch restores the initial prompt as a draft; typed-ahead text
  is retained separately. Quit cancels without sending. Binding-changing
  interactions cancel pending input before changing its route/workspace/effort.
- Startup failure enqueues no initial turn. Saved sessions do not resurrect the
  process-local pending launch after a crash.

Ordinary approvals, verifier guards and tool capabilities are unchanged.

## Checked catalog ownership

The catalog source variable selects the Pi format explicitly:

```sh
ANGEL_OPENAI_CATALOG_SOURCE=pi-store:/absolute/path/models-store.json
```

The native loader checks a regular-file descriptor, size and
record bounds, revision coherence and supported identity/capability fields. It
projects only supported Codex records; imported endpoint/auth/instruction fields
cannot redirect execution. Explicit missing/malformed/empty/all-filtered sources
block selection without unchecked built-in fallback. Legacy missing/malformed cache
compatibility remains distinct; invalid declared capacity is not fallback permission.

One immutable startup load outcome is shared by the Bag, direct Codex alias,
registry and vision consumers. Bounded loader retry is not consumer rereading.
Replacing a store changes the next run, not the current snapshot. Vision shares
OAuth authority but retains its own effort preference. Explicit vision/Kimi
precedence and picker identity/deduplication remain intact.

Supported effort is derived from immutable capabilities and current intent, not a
sticky earlier error. Each dispatch captures effort once for serialization and
receipts. Native `ultra` means wire `xhigh`. Pi `pi:minimal` retains `minimal` unless
an explicit supported mapping changes it; no upward clamp is invented. Disabled
`off:null` and `off:"none"` metadata are accepted and omitted, not offered as
`pi:off` or native `none` requests. Invalid `off` mappings still fail.

Checked known context feeds both metadata interfaces and existing compaction,
schema sizing and result budgets, with overflow-safe arithmetic. Unknown legacy
context remains unknown. Explicit compaction overrides/disable settings remain
unchanged. There is no new request output cap or restriction of competition
research tools. Metadata projection is not publisher authentication.

## Regression checks

Run with synthetic credentials and isolated HOME/CODEX/XDG/session roots; never
supply real accounts for these fixtures. Follow CONTRIBUTING.md for the pinned
Rust toolchain and the native runner's environment prerequisites.

```sh
bash scripts/check/check-cockpit-fast.sh gate1_ --test-threads=1
bash scripts/check/check-cockpit-fast.sh launch --test-threads=1
bash scripts/check/check-cockpit-fast.sh catalog_r --test-threads=1
node --test tests/scripts/angel-club-policy.test.mjs tests/scripts/native-launch.test.mjs
```

`check-catalog-r1-r4.sh` is an additional focused group requiring explicitly
scrubbed fixture roots and offline cached Cargo. Local HTTP fixtures exercise
actual request/receipt agreement without live inference. Tests cover source and
selector rejection, shared snapshot/auth identity, supported effort correction,
context consumers, exact workspace, initial-turn cancellation and literal text.

No-video tests do not qualify video playback, optimized release behavior,
exhaustive model/tool/security behavior, entitlement or coding quality. External
adapters, catalog refresh services, account credential mutation, public catalog
feeds and release publication are outside this contribution.
