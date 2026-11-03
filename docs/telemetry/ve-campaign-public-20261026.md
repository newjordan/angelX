# Virtual Embryo campaign — public-safe snapshot, 2026-10-26

Scores and dates only, for the Meta hackathon evidence pack. The
challenge data is unpublished and is not reproduced here in any form;
the Heartglass demo mode (`ANGEL_GLASS_HEART=demo`) shows a synthetic
campaign for the same reason.

Board as the `vec` cartridge last wrote it, 2026-10-26T18:37:33Z
(phase P2; collaboration ends 2026-10-27T16:00Z, final due Dec 2):

| Board | Ours (best) | VE leader | VE 10th | Above 10th by |
|---|---:|---:|---:|---:|
| T1 single-cell E10.5 | 77.84 | 80.51 | 73.96 | +3.88 |
| Heart interp E8.5 | 75.53 | 76.36 | 72.32 | +3.20 |
| Heart extrap E10.5 | 72.03 | 74.68 | 67.14 | +4.89 |
| Embryo interp E7.5 | 81.04 | 79.91 | 76.09 | +4.94 |
| Gata4 KO E8.75 | 82.14 | 82.28 | 80.34 | +1.81 |

All five boards above the 10th-place line; all three heart boards in
the top ten (`vec_heart_top10 = 3`); on embryo interpolation our best
stands above the published leader mark. Campaign totals so far:
129 heart proxy runs, 63 submissions, against daily per-board
submission budgets (8) that were spent to their last slot on four
boards today.

Method, in one paragraph: an agent loop driven from Angel proposes
candidates; each must pass paired, frozen diagnostics and an
independent replay before it may spend scarce submission quota — the
default verdict is *do not submit*. The 2026-10-26 iteration is the
discipline in miniature: a defect found by review, retained and
repaired, replayed to the last digit, and **not** submitted because
the paired evidence did not justify the quota. Frontier output per
submission — and per unfunded solo builder — is the axis this
campaign optimizes.
