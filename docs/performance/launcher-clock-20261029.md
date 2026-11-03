# Launcher diagnostic clock — 2026-10-29

## Applied change

`bin/angelX` uses Bash's builtin `printf -v` clock on Bash >=4.2, avoiding both an external `date` and a command-substitution subshell at each diagnostic timestamp site. Successful source CLI inspection previously invoked `date` six times; ordinary terminal interaction adds a seventh stderr-header timestamp. There are nine sites across all branches, not nine calls on every invocation.

Bash <4.2 retains external `date`; both branches tolerate clock failure without aborting a launch or replacing its intended exit status. Each call clears inherited export state before assigning the internal timestamp. Time is read per call, including after trusted configuration changes TZ. No new configuration knob is needed.

This changes diagnostics, not compilation: Cargo remains automatic and still checks every ordinary source launch. Profiles, features, explicit build/background/native reload commands, Blaze selection, image copying/hardlinking, hash validation and failure handling remain unchanged. Never use `ANGEL_NO_BUILD=1` as evidence of current-source startup speed.

## Retained evidence

Iteration 31 report (`reports/research-82abbb708cda5ae57589ee8a17aeaaaa-31.md`) and raw experiment directory (`reports/performance/launcher-clock-iteration31`). These files are kept with the measurement and are not published.

- Existing test entrypoint: `node --test tests/scripts/native-launch.test.mjs` — **44/44 passed**, exit 0. Uses fake Cargo/compiler fixtures. Includes new zero-date, forced fallback, failed-clock, exit/cleanup and child-environment checks. The baseline failed the new external-date behavior assertion.
- Separate-context code review caught the initial fallback `set -e` regression; fixed before the pilot. Final separate-context skeptic independently recomputed timing and compared raw streams, but did not certify the full product.
- Real pinned Cargo/Rust 1.95.0, dependency-free offline crate, one shared warmed target, unchanged feature/profile arguments, Bash 5.3.20. **24 alternating AB/BA pairs / 48 warm launches**.

| Pilot measure | Baseline | Candidate |
|---|---:|---:|
| Median launch | 25.453 ms | 23.215 ms |
| Observed range | 23.358–29.288 ms | 18.981–27.835 ms |

Median **paired** reduction: **1.778 ms**, **6.986% of baseline median**; candidate faster 19/24 pairs. This is not subtraction of the two arm medians. Both order subgroups have positive median reductions, but magnitudes/win counts differ (AB 1.679 ms, 8/12; BA 3.021 ms, 11/12). Preregistered descriptive pilot threshold was met. All warm stdout/stderr streams match; no timeouts/errors; fixture source/image/tool hashes unchanged. Initial primes had unequal cache state and are excluded, not called cold-start measurements.

## Limits / operating guidance

This is **launcher overhead in a tiny fixture**, not angelX composer readiness, first install, full-host compile speed, game experience, or a record-time comparison. Do not add its percentage to earlier compiler-probe or compile-boundary measurements. Actual Bash 3.2/macOS was unavailable: forced fallback on current Bash tests branch behavior only. TTY/Glass-specific clock branches, a locale matrix, and failed builtin injection remain unqualified.

The existing `cockpit/src/drive/loop_ctl.rs` protection mismatch still produces `check-locks` exit 1. No seal/protected source was changed. No full-host build, installed Blaze change or upstream publication occurred. Full-product qualification remains blocked until operator approval of the exact retained protected diff and a successful protection check; no bypass or re-seal is proposed.

For long-term dev, continue ordinary `angelX` / `angelX build`, explicit background preparation, and focused native/V8 boundaries where applicable. Blaze remains experimental because its prior full-host restored-image failure is unresolved. This local clock change is not a Blaze fix.

## Upstream Blaze disposition

Do not copy the shell fix into Blaze speculatively: inspection of pinned `crates/ra-jit/src/cargo/units.rs:341` shows `stamp_now()` already uses in-process `SystemTime::now()`, not `date`. Its source hash/lines are retained in the new report's evidence. Existing query/environment/target candidates still require callback integration and real-caller acceptance; no new upstream performance claim follows from this pilot.

A read-only startup review also raised public-artifact selection and post-Cargo image-snapshot concurrency leads. Neither was reproduced this iteration; retain as unverified future correctness work, not confirmed defects or reasons to change Cargo locking now.
