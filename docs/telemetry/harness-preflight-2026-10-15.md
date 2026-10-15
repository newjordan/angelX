# Angel harness preflight — 2026-10-15 UTC

## Result

**OFFLINE PASS; LIVE EXPERIMENTS NOT YET QUALIFIED.** This is an offline runner/configuration preflight, not a performance benchmark, full Rust gate, or proof of leadership.

Machine receipt: `artifacts/research-preflight/20261015T015123Z/receipt.json`.
Identity/limitations: `artifacts/research-preflight/20261015T015123Z/summary.json`.
Exact config/commands/hashes: `artifacts/research-preflight/20261015T015123Z/cell-config/manifest.json`.
Logs: `artifacts/research-preflight/20261015T015123Z/` (locally retained, git-ignored; not available in a fresh clone).

## Checks actually run

| Check | Result |
|---|---|
| identity | PASS (exit 0) |
| source | PASS (exit 0) |
| build-info | PASS (exit 0) |
| terminal-boundary | PASS (exit 0) |
| active-connections | PASS (exit 0) |
| runner-tests | PASS (exit 0) |
| config-only | PASS (exit 0) |

The runner suite ran **36 tests: OK**. The config-only cell validated 12 selected tasks using the existing Luna-specific runner, without `--execute`. This does not validate Grok/TB2 live execution.

## Frozen identities at preflight

| Item | Value |
|---|---|
| head | `665569ee46208ca82a2d28e10b93f5054594e283` |
| source_digest_scheme | `angel-source-v3` |
| source_sha256 | `5c384d7c1c456c2b00687dc570e19547fcebc8183041c401b8709ef5a27cb9bb` |
| executable_sha256 | `b41f58c70a21cec95412b9bbc2a926cca46f53de54e811d1e4fcf0eca23bf9d4` |
| helper_sha256 | `1bebf47d10e03b4144788f5e78b058914d73222afbf47b4bb9031cad5b0d9996` |
| resource_files_sha256 | `8448e2a7e9c608135adc09b9a99843bd8dbccc723f5a46b0e06aedae6282033f` |
| selected_rows_sha256 | `cbdf9e81bc3898927987255961423df9f58b870668efbd8dffb2027a02c56578` |
| receipt_sha256 | `56f4600630a633da46fba2b38fb4989c04012060da139b3fa47aa1b459c00013` |
| config_manifest_sha256 | `203cdf84705b4a5bef82679cce9c2a783bb38e0def813d382c31357084c257f1` |

`--build-info --json` reported a release build with video enabled. Its source pin matched the clean HEAD build inputs. Build-info resources were `unbound`; the successful config-only cell separately snapshotted resources and recorded the resource hash above. Keep build features matched in future A/B runs.

## Environment and remaining gates

- Workspace: the main checkout, branch `dev` at the identity above.
- Before this documentation work there were no tracked edits; four unrelated untracked entries were preserved: `.video_agent/`, `bin/delve-host`, `docs/handoff-muse-scope-2026-10-13.md`, `output/`.
- Initial host snapshot: 117 GiB disk free, 57 GiB available RAM; load 5.05/3.96/3.47. Active compilation and other users/process workloads mean timings require a fresh contention check. These are observations, not reserved capacity.
- Rust/Cargo 1.95.0; Python 3.14.7; Node v26.8.1.
- Existing Polyglot dataset and local TB2 adapter were present. Presence is not execution qualification.
- Successful preflight set `ANGEL_YOLO=0`, `ANGEL_YOLO_SMART=0`; inherited launch had reported YOLO FULL. Check effective controls before each live experiment.
- Old standing goal remains the game/browser objective. It was inspected, not replaced. Check `/loop status` before starting a new loop; active loop state was not verified through a control interface.
- No full Rust suite/build was launched; no live route/credential, remote rig, or current solve-rate claim was established.
- Live/remote trials need recorded model/route, finite attempt/spend ceilings, evaluator/adapter/dependency qualification, and hardware availability.
- No campaign, goal mutation, paid model benchmark, publication, credential change, or competition submission was performed. **ZERO SUBMISSIONS.**

## Research launch

Use `docs/telemetry/harness-research-protocol-2026-10-15.md` for the charter, hypotheses, controls, ablations, record contract, and paste-ready `/goal` and bounded `/loop` prompts.
