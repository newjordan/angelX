# scripts/

Host-side belt around the cockpit. Not the TUI.

| Directory | What |
| --- | --- |
| `release/` | Public archive + verify gates |
| `check/` | Quality / boundary / connection gates |
| `runtime/` | Workers the shipped binary actually runs: dossier, Habitsmith, pxpipe, and the store helpers they import |

Conductor / Still / Cut / Reflex experiment CLIs, machine-queue, and research cohorts are operator-private. They are not this tree.
