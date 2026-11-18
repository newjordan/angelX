<p align="center">
  <img alt="angelX 0.2.7 starting in its own window on Muse Spark 1.3: Excalibur in the stone, the knight above the hill" src="docs/images/intro.png" width="100%">
</p>

<h1 align="center">angelX</h1>

<p align="center">
  A terminal coding agent that runs on the models you already have,<br>
  asks before it acts, and checks its work before it says done.
</p>

<p align="center">
  <a href="https://omni-space.tech">Website</a> ·
  <a href="#install">Install</a> ·
  <a href="#how-it-works">How it works</a> ·
  <a href="docs/MODELS.md">Models</a> ·
  <a href="docs/COMMANDS.md">Commands</a> ·
  <a href="https://github.com/newjordan/angelX/releases">Releases</a>
</p>

---

https://github.com/user-attachments/assets/53be0360-3b4d-4176-ac7b-c924c101c927

**[Download the full-quality film](https://github.com/newjordan/angelX/releases/download/v0.2.7/angelX-how-to-use.mp4)** (2:47, MP4): install, connect a model, set a goal and run a loop.

angelX works inside your repository with any model you choose: a local server
such as Ollama, LM Studio, llama.cpp or vLLM, a ChatGPT or Grok plan, or an API
key from OpenAI, Anthropic, DeepSeek, xAI, Z.ai or OpenRouter. Every command
and edit arrives for your approval before it runs. Before it reports done, it
runs your checks and shows the receipts. What it learns about your project
stays for the next session.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/newjordan/angelX/main/install.sh | sh
```

Open a new terminal window, then:

```sh
cd your-project && angelX
```

Prebuilt for Linux x86_64 and arm64 (glibc 2.31 or newer) and macOS on Apple
silicon. The first launch opens a setup menu that finds the models already on
this machine, checks a pasted API key, or signs in to a ChatGPT or Grok plan
through your browser. `/setup` changes it later, and `angelX update` installs
the latest release.

Every install also ships **glass**, angelX's own GPU terminal, linked as
`glass` beside `angelX`. Run angelX inside glass and pictures, video and charts
show in the terminal too.

<details>
<summary><b>Build from source</b></summary>

You need Rust/Cargo, C/C++ tools, Bash, Node.js and Python 3. On macOS run
`xcode-select --install`, then install Rust from https://rustup.rs.

```sh
git clone https://github.com/newjordan/angelX.git
cd angelX
./bin/angelX
```

See the [development workflow](CONTRIBUTING.md#setup).

</details>

## Why angelX

**Your models, your choice.** Connect a local server, a plan login or an API
key from one menu. `/model` switches routes without a restart, `/think` sets
the reasoning level, and formations put several models on one task.

**Approval before action.** Commands and edits arrive as action capsules.
Approve one, approve the rest of the turn, or deny. Each edit is anchored to
the file's current content and shows its byte change before it lands.

**Checked before done.** The agent runs your checks and reports with receipts:
every tool call, its time and the model route.

**Less of the model's time.** The prompt cache keeps hitting, context is
compacted as the session grows, and the harness steers the model with compact
stamps instead of paragraphs. The same work takes fewer tokens.

**Work that outlasts the session.** `/goal` sets a durable objective with
acceptance criteria and a check. `/loop` runs it within time, iteration and
token limits. Sessions resume where they stopped.

**A window of its own.** On a desktop, angelX opens in a GPU window. Whatever
the agent shows you opens full size above the work: pictures, video, pages,
charts and 3D models. `--term` keeps it in the terminal.

## How it works

![GLM-5.3 reads a small Rust crate to find a unit bug while the knight rides to the Scriptorium on the overworld map](docs/images/task.png)

**1 · Choose a model.** `/model` lists every connected route with its thinking level; `/think` changes the level.

![Model picker with GLM-5.3 selected](docs/images/ui/model-picker.png)

**2 · Approve what it does.** Commands and edits arrive as action capsules: approve one (`y`), approve the rest of the turn (`a`), or deny (`n`).

![Approval capsule for a shell command](docs/images/ui/approval.png)

**3 · Content-checked edits.** Each edit is anchored to the file's current content and shows its byte change before it lands. `/diff` shows the result.

<p>
  <img alt="Edit approval: replace once in src/lib.rs, 32 B to 30 B" src="docs/images/ui/edit-approval.png" width="49%">
  <img alt="The one-line fix in /diff" src="docs/images/ui/diff.png" width="49%">
</p>

**4 · Checked before done.** The agent runs the checks and reports with receipts: every tool call, its time and the model route.

![The turn summary and the final answer with the tests passing](docs/images/ui/receipts.png)

**5 · Longer work.** `/goal` sets a durable objective with acceptance criteria and a check. `/loop` runs it within time, iteration and token limits.

<p>
  <img alt="A goal with acceptance criteria and a verify command" src="docs/images/ui/goal.png" width="49%">
  <img alt="Loop workshop: time, iterations and token cap" src="docs/images/ui/loop-workshop.png" width="49%">
</p>

## In the cockpit

<table>
  <tr>
    <td width="50%"><img alt="Formation roster" src="docs/images/ui/formation.png"><br><b>Formations</b> · model teams, from a lone coordinator to a full roster.</td>
    <td width="50%"><img alt="Context budget" src="docs/images/ui/context.png"><br><b>Context budget</b> · where the window goes; compaction runs automatically.</td>
  </tr>
  <tr>
    <td width="50%"><img alt="Command help and completion" src="docs/images/ui/command-picker.png"><br><b>Commands</b> · <code>/help</code> and Tab completion for every command.</td>
    <td width="50%"><img alt="The overworld map beside the code" src="docs/images/world.png"><br><b>World view</b> · the agent's work as it happens, drawn as a journey beside the code.</td>
  </tr>
  <tr>
    <td width="50%"><img alt="GLM-5.3 plants a bar chart one graph call at a time while a sprite carries the next point to the field" src="docs/images/graph-garden-live.png"><br><b>Graph garden</b> · every <code>graph</code> call grows a crop; a sprite carries each point.</td>
    <td width="50%"><img alt="The showcase over the cockpit: Knight Shift playing in a clear glass panel, a squire seated on its roof" src="docs/images/release-0.2.5.png"><br><b>The showcase</b> · whatever the agent presents opens full size above the work.</td>
  </tr>
</table>

## Knights Wish

angelX has a game in it. `/dungeon` walks your knight to the Delve, a co-op
knights-versus-monsters dungeon in the realm's own pixel art, with a kingdom
above it, a castle for every model family, and wishes that a model writes into
the game while you play. A friend joins from a link. None of it is needed to
use angelX; it is there for when the work is done. [The Delve](docs/DELVE.md).

![The first floor of the Mines in the window: the party band above, the skills bar and map below, the floor lit only by its braziers](docs/images/delve/black-dungeon.png)

## More

[Model setup](docs/MODELS.md) · [Commands](docs/COMMANDS.md) ·
[glass](docs/GLASS_TERM.md) · [Hands](docs/HANDS.md) ·
[Cartridges](docs/CARTRIDGES.md) · [Plugins](docs/PLUGINS.md) ·
[Release notes](docs/RELEASE_0.2.86.md) ·
[Attributions](THIRD_PARTY_NOTICES.md) · [Contributing](CONTRIBUTING.md) ·
[Security](SECURITY.md) · [MIT](LICENSE)

Fork the repository and open a pull request against `main`. Jordan
([@newjordan](https://github.com/newjordan)) reviews and merges. Report
security issues privately; see [SECURITY.md](SECURITY.md).
