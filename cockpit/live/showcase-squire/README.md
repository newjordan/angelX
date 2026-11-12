# Squire art and authored card tracks

`glass-squire-clip/2` keeps `/1` palette, nine indexed frames and idle/wave
clips, and requires `card.states`. The shipped module explicitly imports
`glass-card-motion/1` from `showcase-card-motion` and merges its value. The
canonical reactions live in that independent module; art bytes remain unchanged.
Glass prepares bounded tracks at activation and samples them against the
existing state-start clock without running JavaScript or copying art per frame.
Showcase entrance/exit and card layout/page transitions remain Glass machinery.

The seven states are `sit`, `wave`, `present`, `busy`, `oops`, `cheer`, and
`guard`. A track has `steps`, `repeat`, and optional `then`. Steps have `frame`
(0–8), `millis` (1–10000), `offset` (two coordinates within ±128 logical
pixels), `interpolate`, optional `anchor` (`stand` by default, or `seat`),
optional `left` (default false), and optional `clip: "idle"|"wave"`. Frames
and direction hold until the next step. Interpolation changes offsets between
steps with the same anchor. A one-shot holds its last step or continues into a
repeating `then` state; tails cannot chain. Limits are 2048 steps/60 seconds per
track and 8192 steps overall. Prepared compatibility motion is at most 48 KiB,
reserving 16 KiB for the maximum canonical legacy art/clips. The complete uncompressed
wire packet, including identity and the five-byte framing header, is checked
against 64 KiB too. Unknown top-level art/editor fields
and whitespace are excluded from canonical wire size.

Keyframes approximate the previous renderer formulas. Midpoint sampling and
exact-boundary/neighbor tests compare the frozen old consumer with the new
one. Integer-millisecond authored transitions can differ from old floating
arithmetic at boundaries; the tests quantify the differences rather than
claiming bit-for-bit motion parity.

Legacy `/1` art keeps its reactions through a validated compatibility resource.
Declared imported motion takes precedence. An explicitly declared optional
provider that is absent or withdrawn uses art-only idle/wave fallback, even if
an unrelated ambient provider exists. The agent keeps that api-version-2
manifest intent in a private host binding; the shared Glass format is unchanged.
Only art with no declared motion import uses the unique registered
`glass-card-motion/1` default, without a fixed provider ID. Ambiguous/absent
implicit defaults use the art's idle/wave seated fallback. A missing
whole art resource hides the actor. No choreography fallback lives in Glass.

| Change | Prepared identity and ownership |
| --- | --- |
| `/1` art-only edit | Composite SHA-256 includes new exact art identity and current exact motion identity. Clock stays unchanged. |
| Motion-only edit | Composite changes; art remains the current validated art. Clock stays unchanged. |
| Invalid edit to a present provider | Registry retains that provider's last-good generation; composition stays on validated generations. |
| Healthy published snapshot without compatibility motion, or ambiguous providers | `/1` uses current art's idle/wave. This requires actual resource withdrawal, not a missing source directory. |
| Missing provider source | Registry retains its admitted generation and reports the source error. Dependents use that admitted provider; source deletion does not unregister it. |
| Missing art source with a valid pending provider update | Accepted art bytes/source identity remain old and the missing-source error stays visible, but the valid provider can rebind that accepted art and advance its generation/composite identity. Previously issued compositions keep their old motion. |
| Invalid captured dependent candidate | Its affected provider/dependent transaction rolls back together; unrelated valid modules can still activate. |
| Required provider withdrawal | Reject unless its required dependents are included in the exact-generation withdrawal. Optional consumers remain admitted and rebind to absent-provider fallback. |
| Healthy published snapshot without art | Host and renderer release the actor lease/sheet, even if motion remains. Source-directory deletion alone does not produce this snapshot. |
| Invalid provider candidate | Registry retains the existing admitted generation; this does not constitute withdrawal or re-admission. |
| Defensive composite overflow | Refuse motion and use current validated art only; never retain an unrelated old composite. |

This isolated V10 composition adds explicit system-module withdrawal and enable
controls; source-directory deletion remains a failed candidate and never means
withdrawal. Disable intent is process-local, and already-issued jobs can keep a
withdrawal draining until completion, cancellation, retry or abort. Re-enable
validates its selected owners atomically and installs fresh generations; existing
files and newly enabled dependents cannot silently resurrect disabled providers.

Actual V8 Registry-to-App-to-Host regressions cover required-provider refusal,
joint withdrawal, remote observation of removal, failed and valid re-enable,
optional-provider absence/withdrawal with an unrelated ambient provider, and
unchanged previously issued compositions/imports. Weak references verify retired
composites release after outstanding remote publication and issued references
drop. A separate real GPU fixture verifies renderer replacement, clock retention
and sheet/resource removal. These are candidate-only correctness checks, not a
packaged application launch, watcher-to-GPU/photon latency, or qualification of
the separate native withdrawal prototype. Source and test receipts are recorded
in the candidate's modularity map.

Bootstrap reads the same module-owned assets and checks selected bytes plus the
complete directory/manifest discovery witness twice. It is an asset preview,
not arbitrary JavaScript evaluation. Limits: 128 entries, 16 KiB per manifest,
1 MiB per selected pack, 8 MiB total reads, and a 50 ms cooperative deadline.
A blocking filesystem call cannot be preempted; over-budget work returns no
preview once it returns. The live Registry later installs the actual validated
resource. Missing required `/2` motion gives no cold preview; legacy art remains
usable with its neutral fallback when no compatibility provider is available.

Remote Glass keeps envelope version 1 and its existing bincode shape. `card` is
additive inside the JSON string. Old viewers ignore it and retain art/idle/wave
and their prior built-in reactions; new viewers execute the authored tracks.
Release inventories include both modules and the new pack; an archive fixture
executes their actual JavaScript from extracted runtime paths. A full packaged
GPU launch is a separate qualification from this source/archive check.
