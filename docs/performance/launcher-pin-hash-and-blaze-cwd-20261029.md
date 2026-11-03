# Pinned launch scans and Blaze child-directory identity (2026-10-29)

## Working-tree launcher candidate; limited measurement

An explicitly pinned no-build image (`ANGEL_LAUNCH_BINARY` and its expected
`ANGEL_LAUNCH_BINARY_SHA256`) previously incurred two full SHA-256 scans even when
its private launch image was a hardlink to the selected file. The launcher now
keeps nonblocking regular/executable admission, skips the redundant source scan
on successful hardlinking, and hashes the private image before executing it.

A copy fallback still verifies the source before copying and the private image
after copying. This preserves early rejection of a wrong pin before a potentially
large cross-filesystem copy. Ordinary Cargo freshness, profiles, no-build rules,
and Blaze's isolated copies remain unchanged. This is not an immutable-executable
or general TOCTOU guarantee: hardlinks share mutable inodes, and a hash is a
snapshot, not a lock against subsequent mutation.

One preregistered, alternating 24-pair warm pilot used the actual shell launcher
and a private dependency-free native executable padded to **256 MiB**, comparable
to this workstation's 251 MiB fast executable. It measured command completion,
not actual cockpit or interactive readiness. Padding used sparse zero extension;
this is a hash/launcher pilot, not representative application contents or disk I/O:

| Metric | Observed |
| --- | ---: |
| Baseline median | 305.69 ms |
| Candidate median | 172.90 ms |
| Median paired reduction | 132.56 ms |
| Reduction / baseline median | 43.37% |
| Candidate faster | 24/24 pairs |

The pilot met all preregistered thresholds (15 ms, 10%, 18/24 wins). Cold disks,
macOS/Bash 3.2, actual application execution, and rebuilds were not measured.
There is **no new ordinary source-startup or compile-time speed claim** here.
The final two full launcher test files passed **72/72**; a scan-count regression
fails on the frozen baseline (two scans versus one) and passes the candidate.
Separate-context source review first found the costly-copy regression, then
found no remaining blocking source issue after its correction. Reviewers did not
rerun tests. The pre-existing protected-file seal mismatch still blocks full-host
acceptance; no seal, installed tool or running host was replaced.

## New Blaze PR lead: resolve the compiler in the child's working directory

Pinned upstream `answers.rs::toolchain_rustc` resolves relative program names and
relative/empty PATH entries against the **parent** working directory. Its
`own_answer` and `key` callers do not supply `Command::get_current_dir()`.

A byte-for-byte extraction of the unchanged resolver and `in_sysroot` functions
was compared with real Rust `Command` child execution on Linux. Three cases
(relative executable, relative PATH, empty PATH entry) identified the parent's
compiler while the command actually ran the child's compiler. An absolute
executable control correctly ran and identified the parent compiler despite the
child cwd. All **4/4 reproduction assertions** passed; three affirm the mismatch,
not successful implementation behavior.

These are shell identity fixtures in realistic sysroot directory layouts, not
real rustc answer/cache or full Blaze integration tests. No upstream patch is
installed or submitted. Next: carry cwd through resolver/answer/key APIs or
conservatively decline ambiguous context, then add failing real-module tests,
cache/synthetic-answer controls, and explicit/relative rustup-home coverage.
Do not simply add cwd to a key while leaving compiler selection wrong.

Cargo remains automatic; Blaze is explicit and experimental. The broader
record-time startup/rebuild objective remains unqualified.

Full findings, commands, receipts and resource accounting (`reports/research-82abbb708cda5ae57589ee8a17aeaaaa-38.md`, not published).
