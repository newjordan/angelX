# Handoff — tree clean, pushed pending this docs commit (2026-09-21)

**State after the docs commit lands and is pushed:** `main` ahead of the old `d724dd4` by the commits below. Working tree should be clean. Do not rebuild the live polyglot pin.

## Landed this pass

| commit | what |
|---|---|
| `a12a51c` | fix(harness): hang ceilings stay armed under yolo; cancel token outranks the sleep guard. Launcher still exports hard cap `600` when unset; library default is `900`. |
| `8821d84` | feat(club): Grok HTTP default is `grok-4.7`. Explicit `4.6` still resolves to `grok-4.6`. |
| `8964f29` | feat(website): angelX series emphasized on the bench plots. Published 136-task numbers unchanged. |
| `20768c0` | test(ui): intro band width clamp assertion. Formatting-only wraps in turn timing and the intro band call. |

BUG-0002 verification (before these commits, same tree): `cargo test --bin angel -- --exact <full path>`, 1 passed / 0 failed, both `ANGEL_YOLO=1` and unset, for the ceiling test, both 30s diagnostics, the activity test, the delegate heartbeat test, `shell_tool_honors_registry_cancel_authority`, and `interactive_shell_rejects_excessive_sleep`. Full suite not run.

## Version

Still `0.1.3`. No version bump in this pass. Tree is ready for a version test or a `0.1.4` cut when asked.

`docs/images/provenance.json` stays at `0.1.2` until the screenshots are re-captured.

## Do not disturb

Live campaign: `/home/frosty40/angel_tests/angelX-bench/polyglot-20260921`, SHA-pinned `pin/angel`.

## Verification rules

- Never run bare `cargo test` in the foreground. BUG-0001 kills the turn's process group in the (120s, 240s] window.
- Long gates: `setsid nohup bash -c '…; echo CARGO_EXIT=$?' > /tmp/x.log 2>&1 &`
- `pgrep -f 'cargo test'` self-matches. Use `ps -eo pid,cmd | rg '[c]argo test'`.
- `cargo test --bin angel -- --exact full::path`. One `--exact` per invocation. A filter after `--` can match 0 tests and still exit 0.
- `ANGEL_YOLO=1` is set in this environment. Pair containment measurements. Non-fixed caller deadlines are still stripped under yolo; do not remove that strip.

## Historic

The 2026-09-20 refactor handoff is at `docs/handoff-refactor-src-layers.md`.
