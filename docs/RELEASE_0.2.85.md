# angelX 0.2.85: the speed update

On DeepSeek V4.1 Flash, angelX solved all 136 polyglot tasks on each
of three seeds in 19 to 23 minutes. pi 1.5.0, on the same model the same
day, solved 127 in 103 minutes.

## Upgrade

```sh
angelX update
```

Or install fresh with the one line in the [README](../README.md). From a source
checkout, `git pull` and run `./bin/angelX` again.

## Results

**All 136 polyglot-v1 tasks**, side by side with pi on the same model and seed.

| DeepSeek V4.1 Flash | angelX 0.2.85 | pi 1.5.0 |
|---|---:|---:|
| solved | **136 / 136** | 127 / 136 |
| agent time | **18.7 min** | 102.9 min |
| median task | **5.6 s** | 8.3 s |
| model calls / task | **2.3** | 17.9 |
| input tokens | **2.42 M** | 106.16 M |
| cost | **$0.19** | $0.91 |

Two more seeds: 136 / 136 in 20.1 min and 136 / 136 in 23.4 min.

**Tests the agent never saw.** The same tasks with every other test held
back from the workspace and graded on all of them (133 tasks; three whose
tests cannot be split are left out).

| DeepSeek V4.1 Flash | angelX 0.2.85 | pi 1.5.0 |
|---|---:|---:|
| solved | **113 / 133** | 101 / 133 |
| agent time | **21.4 min** | 137.6 min |
| cost | **$0.17** | $1.45 |

**A language we never tuned on.** Go, 36 polyglot exercises.

| DeepSeek V4.1 Flash | angelX 0.2.85 | pi 1.5.0 |
|---|---:|---:|
| solved | **36 / 36** | 32 / 36 |
| agent time | **9.1 min** | 44.3 min |
| median task | **3.6 s** | 7.3 s |
| cost | **$0.06** | $0.35 |

Two more seeds: angelX 35 / 36 and 36 / 36, pi 34 / 36 and 32 / 36.

<sub>Prime Intellect Verifiers v0.3.1 · DeepSeek V4.1 Flash, thinking off · temperature 0 · 8,192-token output cap · one attempt per task, 600 s, fresh environment · 2026-11-11</sub>

## What makes it fast

These are on for a headless task (`angel --task`) in a workspace of 40 files
or fewer. Set the variable to `0` to turn one off.

- **The files arrive with the task.** A small workspace is read before the
  first request and sent as context, each file whole
  (`ANGEL_TASK_PREREAD_AS_CONTEXT`). Package manifests and tool configuration
  stay out of it; the map still names them
  (`ANGEL_TASK_PREREAD_SKIP_MANIFESTS`).
- **Tests run with the write.** A response that only writes files gets the
  test run added to it, so the model sees the write and its tests in one turn
  (`ANGEL_TASK_TEST_AFTER_WRITE`).
- **It stops at green.** When that test run passes in full, the task ends
  there with a short notice from angelX instead of one more model call
  (`ANGEL_TASK_END_AT_GREEN`).
- **More of the first request is cached.** The brevity block now comes ahead
  of the workspace map (`ANGEL_TASK_STABLE_FIRST`).
- **A locked loop is sent once.** When the model repeats the same failing
  batch, the request carries one copy of it and the closing turns rotate
  (`ANGEL_LOOP_COLLAPSE`).
- **Rust builds once per run.** The Rust verifier keeps one private Cargo home
  for the whole run, so later test runs compile only what changed
  (`ANGEL_CARGO_HOME_REUSE`).

## Also in this release

**Knight Racers.** A pod race through Emberfall's foundry canyon, the
Glassway's crystal dark and Moatwalk's night, with its own title, modes,
classes, pods, garage, records, podium and ghost.

**Knights Wish.** A new upper world, the Creator as the rite you meet on
loading in, the mini-games back as rooms the party stumbles on, and the
Deep's villages and the Mines drawn in full.

**Plugins.** Anyone can publish an angelX plugin and anyone can install it
(`/plugins`).

**glass.** The world map reuses its ground and sprites and updates one texture
in place: idle CPU halves and GPU memory stays flat.
</content>
</invoke>
