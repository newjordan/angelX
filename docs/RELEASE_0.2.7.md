# angelX 0.2.7: the Muse Hackathon update

Much of this release was built in the Muse hackathon. angelX 0.2.7 gives the
agent hands: on Omarchy it can see and work the desktop and a real browser.
It does the same work as the model makers' own coding tools in less time and
with less than half their tokens. Its window and its terminal, glass, are
faster than kitty at every measure we took. Underneath, the development loop
went from a 24-second edit-and-test to half a second, the harness's judgement
moved into Bend modules you can edit while angelX runs, and Blaze arrived as
an experimental build backend. 0.2.7 also carries everything prepared for
0.2.6, the Heartglass build, which never shipped on its own.

## Upgrade

```sh
angelX update
```

Or install fresh with the one line in the [README](../README.md). From a source
checkout, `git pull` and run `./bin/angelX` again.

## Results

**Same model, the maker's own CLI.** Each pair ran side by side on all 136
polyglot-v1 tasks (one attempt, 600 s, fresh environment, graded however the
agent exits).

| Grok 4.7, low | angelX 0.2.7 | Grok CLI 1.0.70 |
|---|---:|---:|
| solved | **136 / 136** | 135 / 136 |
| agent time | **45.6 min** | 74.9 min |
| median task | **16.9 s** | 28.3 s |
| model calls / task | **3.98** | 4.97 |
| input tokens | **5.38 M** | 13.68 M |

| Muse Spark 1.3 Contributor | angelX 0.2.7 | Muse Code 1.4.14 |
|---|---:|---:|
| solved | **136 / 136** | 136 / 136 |
| agent time | **37.2 min** | 119.8 min |
| median task | **14.3 s** | 41.9 s |
| input tokens | **6.58 M** | 26.33 M |
| output tokens | **175 k** | 557 k |

angelX reached Grok through the bench's interception proxy; the Grok CLI ran
on xAI's own backend, which reports the model as `grok-4.7-build`. Muse Code
ran as it ships (`muse exec`, its default effort and delegation); its token counts are the `run.usage` records it writes itself. One seed, one attempt per task.

**Better than our own last release.** On the same Grok cell, 0.2.7 used 20%
fewer model calls (4.0 per task, from 5.0) and 20% fewer input tokens (5.4 M,
from 6.7 M) than 0.1.6, with the cache hit rising from 57% to 67%. Muse Spark
1.3 Contributor also solved 136 of 136.

**Virtual Embryo.** On the challenge's five public boards on
[yukon.org](https://www.yukon.org/) angelX's runs are in the top five on every
board: 2nd on heart interpolation, 3rd on the Gata4 knockout, 4th on T1
single-cell and on embryo interpolation, 5th on heart extrapolation. They set
the board's best 29 times. Standings as of 2026-11-01, with no submissions
since 2026-10-27.

## Hands: computer use and web control

- **The desktop.** In a Hyprland session the agent has a `desktop` tool: see monitors and windows, take screenshots, focus, press keys, type, click and launch, through Omarchy's own `hyprctl`, `grim`, `wtype` and `ydotool` or `wlrctl`. Looking needs no approval. Acting asks once per batch in an action capsule, as shell commands and edits do, and `/yolo` skips it.
- **The browser.** A `browser` tool keeps one Chrome (Chromium as fallback) open over CDP: open a page, read its interactive elements and text, click, type, press keys, take screenshots. Only `http(s)` and `about:blank` open. `/browser` picks the browser.
- **A screen of its own.** `desktop agent_start` adds a headless display on workspace 90, where launches, screenshots and the browser can work away from your monitors.
- **Mage grounding.** `desktop ground {target}` asks Microsoft's Mage-VL-Cowork (served at `ANGEL_GROUND_URL`) where to click, zooming in twice, and returns a point for an approved click; garbled or off-screen answers are refused. On a 618-target bench it hits 79% of web and 51% of desktop targets, and live it passed 23 of 24 tasks.
- Screenshots open on the showcase so you can watch the work land. `ANGEL_HANDS=0` hides both tools. See [Hands](HANDS.md).

## glass: the window and the terminal

- **The window, sooner.** The GPU, fonts and wallpaper open in parallel with the window, and on Wayland the CPU draws the first frame while the GPU is still opening. Over 16 runs the window was on screen in 31 ms (kitty 0.49.2: 73 ms) and the program was running in 39.5 ms (kitty: 193 ms) ([GLASS_TERM.md](GLASS_TERM.md)). An idle window with a full-HD background now holds about 45 MiB of VRAM instead of 216 ([GLASS_CORE.md](GLASS_CORE.md)).
- **glass, the terminal.** `crates/term` builds `glass`, a terminal on the `alacritty_terminal` engine drawn by the window's renderer. Against kitty 0.49.2 on one machine: first output on screen in 46.1 ms (kitty 185), a key press reaching the program while it floods output in 3.0 ms (kitty 38.6), full-screen TUI frames at 3,457 fps (kitty 917), truecolor output 2.8× faster, and half the CPU over six workloads. Only kitty has been measured. It ships in every prebuilt: the installer links `glass` next to `angelX` and never replaces a `glass` you already have.
- **Open intake.** glass takes in the APC graphics protocol (chunked, shared memory, placeholders, animation), sixel and OSC 1337 pictures, tmux passthrough, CSI u keys, cursor shapes, OSC 52 copy, synchronized output and links on Ctrl+click. Its own dialect, version 3, adds layers, 3D scenes, glTF 2.0 models with skins and clips, tweens and showcase items ([GLASS_DIALECT.md](GLASS_DIALECT.md)). The 3D scene in the release film is `crates/term/examples/glass_showoff.rs`, and you can run it yourself ([GLASS_TERM.md](GLASS_TERM.md#filming-the-window)).
- **It shows you.** Evidence a finished reply links to, and local files clicked in the transcript, open full size in the showcase. `ANGEL_GLASS_RECORD=film.mp4` films the window through the GPU's encoder.
- **Remote glass.** `angelX <host>[:<session>]` runs the cockpit on another machine and draws it here over SSH. The session outlives the window: Ctrl+\ detaches. See [Remote glass](REMOTE_GLASS.md).

## Engineering: faster to build, faster to start

- **Edit to tested in half a second.** A focused edit-and-test of the cockpit went from 24.4 s to 0.55 s, by moving V8's compilation behind its own boundary with the same version still linked ([v8-compile-boundary-20261028](performance/v8-compile-boundary-20261028.md)).
- **Native builds 2.6× faster.** Native C compiles run in parallel within Cargo's job budget (905 to 320 ms) and keep their own fingerprint, so unrelated build-script reruns skip them ([native-reload-parallel-20261028](performance/native-reload-parallel-20261028.md), [native-fingerprint-20261028](performance/native-fingerprint-20261028.md)). A measured edit loop runs 2.57× faster ([dev-loop-20261028](performance/dev-loop-20261028.md)).
- **Startup.** First terminal output went from 36.9 to 28.5 ms by waiting on a pidfd instead of polling ([startup-readiness-20261028](performance/startup-readiness-20261028.md)). The Delve's build script emits 87.7% fewer watch directives ([delve-watch-startup-20261028](performance/delve-watch-startup-20261028.md)), and video playback no longer loads the capture stack at startup ([video-playback-startup-20261028](performance/video-playback-startup-20261028.md)).
- **Blaze, experimental.** A fast build backend you can opt into: `python3 scripts/runtime/install-blaze.py`, then `ANGEL_BUILD_BACKEND=blaze`. It went through 45 qualification iterations, and each correctness problem we found is written up as an upstream fix in 26 studies. Cargo stays the default until Blaze passes a full edit-and-restore freshness check ([startup-evidence-checkpoint-iteration45](performance/startup-evidence-checkpoint-iteration45.md)).
- **Bend.** 414 Bend files ship in this release: the Delve's game rules, each checked against the native rules by an oracle suite, and the harness's live modules.
- Source launches use the incremental `fast` profile. `angelX build [--background | --release | --modules]` builds without opening a cockpit.

## Live modules

- **Edit it while it runs.** The running cockpit picks up changes in `cockpit/live/` between calls: pane layout and composer text, how tools are shortlisted and ranked, Delve tuning, and character and ability packs. If a new version fails, the working one stays. `/reload status` shows what is loaded.
- **The harness's judgement, in Bend.** 44 modules now decide reasoning effort, how a failed provider request is classed and how long to wait before a retry, the compaction window, swarm routing and waves, lane scheduling, verification policy, rewards, promotion, worker health and cost. Each checks its own laws when it builds, is pinned for the whole turn and is recorded in the run's identity. Without a live version, the copy built into the binary answers. See [Live application modules](../cockpit/README.md#live-application-modules).

## The Heartglass (from 0.2.6)

- `/heart` shows the Virtual Embryo campaign in the window: boards, cell cloud, stage cycle, heartbeat and countdown. The modules float and can be dragged and resized; `/heart plate` gives the leaned plate and `/heart reset` restores the layout. `/heart demo` shows a made-up campaign. See [The Heartglass](HEARTGLASS.md).

## The harness

- **The prompt cache keeps hitting.** Every route that re-renders history is tested so each request starts with the exact bytes of the one before it.
- **Cheaper hops.** While a session is under budget, a hop boundary skips compaction and tool-result fitting when they would change nothing.
- **Post-edit review restored (from 0.2.6).** The first successful edit in a turn brings back the logic review once, as harness advice that never tells the model to finish.
- **Loops.** A corrupt loop checkpoint is moved aside and reported. A podrace run's submission clock starts again after every submission.
- **Commands cannot hang a task.** In task mode a single shell command is stopped after 120 s (`ANGEL_TASK_CALL_TIMEOUT_SECS`), so a stuck test or a `find /` no longer eats the whole attempt. Each task run gets a private cache directory, so sandbox launches stop paying for the host's cache.
- **Loops are caught.** When the model repeats the same tool batch three times, or goes eight hops without a word, the harness says so (at most three notes a turn), and after a failing test reasoning is lifted for one hop. An answer that is only tool-call markup is no longer taken as a finished reply.
- **The post-edit review asks for a trace.** The review now asks the model to write the state transitions it checked in a line or two before its next call; models that used to answer it with silence now answer it.
- A `SessionStart` hook hands the session id and workspace to outside tools and never blocks the session. Continual-harness context is ranked by relevance to the turn (BM25).

## Backup and peer watch

- `angelX --backup run|restore|status|prune` takes tiered snapshots of `~/.angelX` to a local path or over rsync, with a SHA-256 per file. A restore checks every hash first and will not overwrite a differing file without `--force`. State files are written atomically. `/peers` lists other angelX instances. See [Backup and peer watch](BACKUP.md).

## Cartridges

- `dig_search` is a core tool next to `grep`. [`examples/cartridges/vfx`](examples/cartridges/vfx/) is a pixel-effects renderer written in Bend, used through `/vfx`.

## The Delve

- Rules run in a separate realm host and as Bend programs, and can be rebuilt and swapped mid-run. Squires to hire, the Kingdom Loop's buildings, the Deep below floor 6, 42 new heroes and 203 new cards, Change Ringing in the Bashful Belfry, and a reworked first ten minutes.
- Guardians fall faster and fight louder: 70% of their old health, and more bullets in the air at every stage.

## Contributing

- Outside contributors fork the repo and open a pull request against `main`. See [CONTRIBUTING](../CONTRIBUTING.md).
