# angelX 0.1.9 release line

**0.1.92** ships live Sloptomizer relationships, signal legend continuity, the
School of Magic, and native mini-world adventures. These belong to the 0.1.9
line. The existing v0.1.9 release remains the record of the earlier
graph-garden release.

## Research that carries its evidence

Ordinary typed checks and measured experiments now feed Sloptomizer's shared
relationship memory. Compact `⚠⡫` cards connect optional caveman hypotheses and
expectations with observed results and their receipts. Passing, failing, and
inconclusive evidence stay distinct; formal experiments retain ownership of
fitness rewards. Repeated failed verdicts can prompt sparse advice even when
their diagnostic text changes.

This worker runs locally and asynchronously. Advice introduces no research stop,
forced pivot, model change, tool restriction, or research budget. Used braille
legends persist across sessions and compaction and are reintroduced when a model
takes over. Ordinary hops reuse the context already taught.

See [research behavior and controls](../cockpit/research/sloptomizer/README.md).

## The world follows actual work

The [School of Magic](world-school.md) adds two elves, two wizards, a study,
and an underground archive displaying actual receipt counts. The
[3D adventure](world-adventure.md) follows loop iterations into a mine with
torchlight, crystal workings, and visible lower passages. Camera travel follows
iterations; books and tools follow current activity. A solo run shows one knight.
Explicit visits, camera controls, and motion preferences remain available.

Two visual review rounds used the production renderer. Their event fixtures
demonstrate the presentation; they do not stand in for live benchmark results.

## Verification and performance status

The corrected harness passed 8 Python relationship tests, 7 Rust live-worker
tests, and 91 book/continuity tests (2 ignored measurement fixtures). The world
suite passed 366 tests (17 ignored), and Scryglass passed 47; those suites
overlap. The default-feature application builds, and the repository's Clippy
gate passes with its existing allowances. All 94 source-release tool tests pass.
The 69 script-hygiene tests and duplicate-content check also pass. The archive
explicitly includes the new relationship module and world guides.

A scripted local stress test delivered the sparse warnings with no additional
model calls. Across two samples per arm, median task time increased 48.5 ms
(2.9%) and transmitted request bytes increased 4.9%. This measures overhead,
not model task quality or billed token cost.

A separately supervised real GLM Flash pilot completed all 24 attempts: six
tasks, with memory off/on and then on/off. It used a frozen development binary,
the same external grader, fresh task homes, and identical model settings. All
154 harness, loop-controller, and Sloptomizer files match the shipping
implementation byte for byte. Every attempt passed, with complete usage and
grader-integrity receipts and no task timeout or error exit.

| Across both rounds | Memory off | Memory on |
| --- | ---: | ---: |
| Passed attempts | 12 / 12 | 12 / 12 |
| Model calls | 140 | 117 |
| Input tokens, including cached input | 1,287,692 | 987,457 |
| Derived uncached input tokens | 155,084 | 134,145 |
| Output tokens, including reasoning | 40,542 | 27,908 |
| Recorded task seconds, total | 1,906.1 | 1,091.5 |

The enabled arm used 16.4% fewer model calls and 23.3% fewer input tokens in
this pilot. Twelve distinct advice cards reached seven enabled attempts; none
reached a disabled attempt. Per-task regressions remain in the totals: React
used 30 calls enabled versus 25 disabled, and transpose used 29 versus 21.

These are descriptive observations from two repeats of six tasks. Five enabled
attempts received no advice card, and a disabled Forth attempt spent 340 seconds
in three tool timeouts before passing. The host was shared, and provider latency
and cache state were uncontrolled. The pilot establishes delivery and observed
correctness on this cohort; it does not establish a causal efficiency gain,
cross-session or model-transfer efficacy, or production-build speed.
