# Handoff — 0.1.3 close-down (2026-09-21)

**State:** `main` @ `47f6720`, pushed (`origin/main` in sync). The tree is dirty with
ONE in-flight package from another seat — that work is deliberately uncommitted so
its author can finish it. This handoff exists so that can happen without collision.

**Next work:** the two fixes in BUG-0002. Read `docs/BUGS.md` first.

---

## 1. Landed in this pass (all pushed)

| commit | what | evidence |
|---|---|---|
| `7373b11` | Intro band: width clamped to the dots that can carry the art (`rows*2*CROP_W/FRAME_H`, cap `INTRO_MAX_COLUMNS=96`), centred, rows untouched so the waterline stays on the pane floor. Applied in `ui/draw/transcript_view.rs` **before** `dot_geometry()` so canvas, fine-dot raster and declared cell rect share one width. | `cargo test --bin angel startup_intro` → 17 passed, 0 failed, 2 ignored |
| `08fefd5` | Version 0.1.3: `cockpit/Cargo.toml`, `cockpit/Cargo.lock`, `package.json`, `package-lock.json`. | crate compiles as 0.1.3; lock re-validated by cargo |
| `36adc72` | Restored the hero square preview that `19bea5d` had swapped for an 18 s 4:1 sliver — back byte-for-byte at sha256 `581f19fe…` (1600×1600, 50.8 s, 981 kbps). `teaser.mp4` and the 136-task plots were kept (both legitimate). | `ffprobe` on both versions |
| `984b48f` | `docs/yukon-wins-audit-20260921.md` (Taildrop from toymaker, verbatim) + BUG-0001 | — |
| `47f6720` | BUG-0002, below | paired A/B runs |

`docs/images/provenance.json` intentionally stays at 0.1.2: it is the hashed capture
record for the 0.1.2 screenshots and names the binary that produced them. Re-capture,
then move it.

## 2. In flight — another seat's package (do not commit, do not push as-is)

`bin/angelX`, `cockpit/docs/ENV.md`, `cockpit/src/agent/club/{failover,grok,http}.rs`,
`cockpit/src/agent/harness/{exec.rs,turn/mod.rs}`, `cockpit/src/agent/tools/shell.rs`,
`cockpit/src/app/model_setup.rs`, `cockpit/src/ui/draw/transcript_view.rs` (fmt only),
+ 5 test files. It adds a 900 s hard ceiling, a 120 s idle floor, a synchronous-sleep
guard, and grok 4.7. It is **red — 5 failures**, and BUG-0002 explains why: four are
the `yolo::enabled()` bypass, one is a real cancel-authority regression.

## 3. Immediate work on restart

1. Wait for the seat to commit its package, then `git fetch && git pull --rebase`.
2. `cockpit/src/agent/harness/exec.rs` — `tool_hard_timeout()` and `tool_idle_floor()`
   both open with `if crate::platform::yolo::enabled() { return None; }`. This box runs
   `ANGEL_YOLO=1`, so the new guards are inert in the operator's own posture — the
   posture where the hang happened. Yolo governs approvals; a runaway process group is
   containment, not consent. Honour the ceilings regardless of yolo, or scope the bypass
   to approvals only.
3. `cockpit/src/agent/tools/shell.rs` — the new `contains_excessive_sleep(cmd, 10)`
   guard pre-empts a cancel that is already queued, so
   `shell_tool_honors_registry_cancel_authority` fails (3.61 s, red with yolo on *and*
   off). A pending registry cancel must outrank the sleep guard.
4. Verify **paired** — the same test with and without `ANGEL_YOLO`:

   ```
   cd cockpit
   env -u ANGEL_YOLO cargo test --bin angel -- --exact <full::test::path>   # must pass
   cargo test --bin angel -- --exact <full::test::path>                     # must also pass
   ```

   This pair is the whole point: with `ANGEL_YOLO=1` the first four were red and are
   green without it, so a fix that only makes the ambient run pass has fixed nothing.

## 4. Verification rules for this cockpit (learned the hard way)

- **Never run bare `cargo test` in the foreground.** BUG-0001: the agent shell kills
  the turn's process group in the (120 s, 240 s] window (`sleep 240` → Terminated,
  `sleep 120` → fine), so the suite can never print a verdict. Not memory: 94 GB box,
  ~70 GB free at kill time.
- Long gates go detached, or they die with the tool's process group:
  `setsid nohup bash -c '…; echo CARGO_EXIT=$?' > /tmp/x.log 2>&1 &`
- `pgrep -f 'cargo test'` self-matches the probe's own command line and lies
  `RUNNING`. Use `ps -eo pid,cmd | rg '[c]argo test'`.
- Check contention first: a live campaign at
  `/home/frosty40/angel_tests/angelX-bench/polyglot-20260921` (SHA-pinned `pin/angel`)
  saturates this box and starves test workers. Do not disturb it; do not rebuild it.
- `--exact` needs `--` before it. `cargo test --exact X` silently selects 0 tests.
- `ANGEL_YOLO=1` is set in this environment. Pair every measurement accordingly.

## 5. Historic

The 2026-09-20 refactor handoff (the `src/` layer map, module→bucket) is preserved at
`docs/handoff-refactor-src-layers.md`.
