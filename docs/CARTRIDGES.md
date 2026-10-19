# Cartridges

angelX ships the competition engine: the loop, the submission watcher, the
fleet line on the loop HUD, truthful `--model`/`--harness` attribution on board
submissions, and the book of behaviors. A **cartridge** plugs one competition
into that engine: its board CLI, its loop worker's brief, its status API, its
book pages. Cartridges are yours. They live outside the angelX tree, so a
private setup never ships with the harness.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="images/cartridges/slot-dark.png">
  <img alt="The engine's plug points, and which ones the yukon and lean-kernel cartridges fill, in Rust or in cartridge.toml" src="images/cartridges/slot-light.png">
</picture>

## Plugging one in

Put the cartridge's folder in `~/.angelX/cartridges/`, or point
`ANGEL_CARTRIDGES` at it (a path list; each entry is a cartridge or a folder of
them). The first cartridge found is active; `ANGEL_CARTRIDGE=<id>` picks
another. `angel --build-info --json` lists the cartridges a binary carries.

With no cartridge plugged in, the engine runs on its own: competition loops,
the watcher and the HUD work, and the book names only the platform CLI.

## How Yukon uses it

Yukon is the operator's own cartridge, and the reason the slot exists. It is a
source cartridge: a `cartridge.toml` plus Rust that the build compiles in. It
lives in `~/.angelX/cartridges/yukon`, a private repository of its own, and
none of it is in this tree.

- **The toml** names the board CLI (`yukon`, installed as `hilbert` too), the
  local measurements (`yukon run`, `yukon validate`), the worker's page, and how
  a result is read (`yukon_status or the platform CLI`).
- **The Rust** adds the `yukon_status` tool, the authenticated status API behind
  a live submission watch, the fleet sweep across every open benchmark, the
  seat's strategy brief and its experiment ledger, `angel --comp-status`, and
  the book pages that name all of it, in the words they had before cartridges.
  A golden test holds those pages to the old text.
- **Angel Y**, the pinned competition launcher, ships inside the cartridge and
  refuses a binary built without it.

Every turn of a Yukon loop passes through the cartridge at seven points:

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="images/cartridges/turn-dark.png">
  <img alt="A competition turn in seven steps, with the hook behind each and what the yukon and lean-kernel cartridges do there" src="images/cartridges/turn-light.png">
</picture>

What the split moved, measured in lines (data in
[`cartridges.json`](images/cartridges/cartridges.json)):

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="images/cartridges/lines-dark.png">
  <img alt="Lines of code that ship with angelX's engine, and lines that stay in the private yukon cartridge" src="images/cartridges/lines-light.png">
</picture>

## Another competition: the Lean Kernel Challenge

[The Lean Kernel Challenge](https://competition.sair.foundation/competitions/lean-kernel-challenge)
(Lean FRO and the SAIR Foundation; submissions close 2026-11-20) asks for
verified computation: for each problem, an algorithm `impl` and a proof
`impl_correct` that it matches the specification for every input. The rank is
the sum of per-case median kernel instruction counts, lowest first.

[`examples/cartridges/lean-kernel`](examples/cartridges/lean-kernel/) is that
competition as a config-only cartridge, written from its public rules and
starter repository. It has not been run on the live challenge.

- **No board to stamp.** Submissions go through the SAIR web platform, so
  `boards` is empty.
- **The worker's pages are data.** `[[pages]]` in the toml give the loop worker
  its brief at `⠻⠊`, the same address Yukon's Rust pages use.
- **Measuring.** `bench.sh` proves a candidate with `lake build`, then replays
  it in the kernel with the challenge's `evaluation/run.py`. angelX already
  counts a `bench*` script as a measurement.
- **The fleet line.** `fleet.sh` reads each problem's latest local evaluation.
- **When to add Rust.** To read the official leaderboard, watch a submission, or
  keep an instruction-count ledger, the cartridge would grow a `src/` with
  those hooks, as Yukon's did.

## A tool cartridge: REA

A cartridge can also carry slash commands instead of a competition. Each
`[[commands]]` entry names a word and a stack of skills from the cartridge's
`skills/` folder. `/<word> <task>` sends one turn with those skills and the
task, as `/skills` would. Until then the cartridge costs nothing: no process
starts, no tool joins the registry, and its skills stay out of the catalog. A
cartridge with commands and no board or worker is never the active competition.
In a skill, `${ANGEL_CARTRIDGE_DIR}` becomes the cartridge's folder.

It can carry tools too. Each `[[tools]]` entry names a program in the
cartridge's folder, a description and a JSON schema for its arguments. The tool
joins every session deferred, so its schema costs nothing until `tool_search`
brings it in. A call runs the program in the session's workspace with the
call's arguments as JSON on stdin, `ANGEL_CARTRIDGE_DIR` and `ANGEL_WORKSPACE`
set; its stdout is the result, and a failing exit is an error with the end of
its stderr. A name another tool already has keeps that tool. `core = true`
advertises the tool with the core coding tools instead, beside `grep`, at the
cost of its schema on every request.

[`examples/cartridges/rea`](examples/cartridges/rea/) plugs in
[REA](https://github.com/morluto/rea) (Reverse Engineer Anything, MIT), which
reverse engineers native binaries, JavaScript/Electron apps, .NET assemblies,
APKs, firmware and websites. REA's MCP server has 207 tools with 3.3 MB of
schema. Attached through `mcp.json`, every turn would carry them, so the
cartridge drives REA's CLI instead.

```sh
docs/examples/cartridges/rea/install.sh   # into ~/.angelX/cartridges/rea
```

The install pins `rea-agents` (6.5.0; `REA_VERSION` picks another release) and
takes REA's skill from that package, so the skill matches the CLI. Then:

```text
/rea /usr/lib/obsidian/app.asar how does search work? build a version here
```

The turn carries the cartridge's `rea` skill (where the CLI is, and how REA's
tool names map to its commands), then REA's own `reverse-engineer-anything`.
Remove `~/.angelX/cartridges/rea` to unplug it. Native decompilation needs
Ghidra, IDA or Hopper; the other targets need none.

### DigUp

[`examples/cartridges/dig`](examples/cartridges/dig/) plugs in
[DigUp](https://github.com/ARahim3/DigUp) (MIT), which finds files by what
they show or say: pictures, PDFs, documents, recordings and video, and code in
the folders added for it. It runs Google DeepMind's EmbeddingGemma 2 on
llama.cpp, on this machine, and keeps one SQLite index of vectors and exact
words. A hit names the page, the second or the line.

```sh
DIG_HOME=/work/digup docs/examples/cartridges/dig/install.sh   # into ~/.angelX/cartridges/dig
~/.angelX/cartridges/dig/bin/digup estimate ~/Pictures         # the first pass's time
~/.angelX/cartridges/dig/bin/digup index ~/Pictures            # adds the folder; again to catch up
```

The install pins DigUp's Linux x86_64 release (0.7.0-beta.2, checked by its
SHA-256), takes its command-line helper from the package and fetches the model
(865 MB, once). `DIG_HOME`, optional, keeps the helper, model and index
outside the cartridge. Then:

```text
/dig the knight sheet with the lance first
```

The turn carries the cartridge's `dig` skill (where the CLI is, what a hit
says, how to add a folder) and searches the index built ahead of time.

While coding, the agent has `dig_search` (`bin/dig-search`), a core tool beside
`grep`. It
searches the workspace's code by what it does or a name in it and answers
`path:line`; a worktree of an indexed checkout is served from that checkout's
index, its hits mapped onto the worktree's files. With `kinds`, it searches
those kinds of file in every indexed folder instead. Index a repository's code
with `digup index --code <repo>`. Nothing watches the folders; re-running
`digup index` on a schedule (a systemd timer, say) keeps the index current, and
an unchanged repository costs seconds. Remove `~/.angelX/cartridges/dig` to
unplug it.

## A config-only cartridge

A folder holding one `cartridge.toml` is a whole cartridge. It is read at
startup; nothing is rebuilt.

```toml
id = "kernelbench"
label = "KernelBench"
# The board CLI's program names. `<board> submit` gets angelX's own --model and
# --harness; `<board> submit|submissions|status|list` count as outcomes.
boards = ["kb"]
# `<board> <sub>` commands that are local measurements.
measures = ["run", "validate"]
# The loop worker's brief: a page address, defined below.
worker = "⠻⠊"
# How the prompts name reading a submission's result.
status_check = "`kb status` or the platform CLI"
# The fleet line: a command printing one snapshot. It runs in the session's
# directory; ANGEL_CARTRIDGE_DIR names this folder.
fleet_command = "\"$ANGEL_CARTRIDGE_DIR/fleet.sh\""
fleet_interval_secs = 45

[[pages]]
route = "⠻⠊"
name = "kernelbench"
signal = "the KernelBench loop worker"
pages = ["You are a competition loop worker on KernelBench."]

# A slash command: `/kb <task>` sends one turn with skills/kb-triage.
[[commands]]
name = "kb"
usage = "<kernel> [what to try]"
skills = ["kb-triage"]

# A deferred tool: bin/kb-best reads the call's JSON on stdin and prints the
# result. `params` is the arguments' JSON schema.
[[tools]]
name = "kb_best"
description = "The best local score for a kernel so far"
run = "bin/kb-best"
params = { type = "object", required = ["kernel"], properties = { kernel = { type = "string" } } }
```

`fleet_command` prints:

```json
{"benchmarks": 2, "failed": 0,
 "submissions": [{"benchmark": "kb/softmax", "id": "a1b2", "status": "validating", "score": null}]}
```

## A source cartridge

Add `src/mod.rs` beside the toml and the cockpit's build compiles the folder
in as a module. It exports `pub(crate) static HOOKS`, an implementation of
`Hooks` (`cockpit/src/agent/harness/cartridges/mod.rs`). Every hook is
optional:

| Hook | What it gives the competition |
|---|---|
| `tools` | Tools added to every session (a status reader, say) |
| `research_tools` | Which of those long research always sees |
| `pages` | Book pages: new routes, or the competition's own wording of a route |
| `configured_watch` | A live submission watch the operator configures |
| `fleet_sweep` | The fleet line, in Rust |
| `seat_entry_focus` | Evidence carried inline when a competition turn engages |
| `record_dispatch`, `record_terminal`, `terminal_results` | A durable experiment ledger the loop reconciles against |
| `status_cli` | `angel --comp-status ARGS…` |

A source cartridge can use anything in the cockpit crate. Rebuild after
changing it; the build watches its files. A source cartridge that a binary was
not built with is ignored at startup rather than run without its hooks.
