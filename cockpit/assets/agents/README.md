# Agent portrait assets

The cockpit selects these portraits through `src/agent_profile.rs`.

## Cyberknight effort pairs

The active portrait set was regenerated for this project on 2026-07-20 with the
built-in OpenAI image-generation workflow:

- `turbo-cyberknight-neutral.png`
- `atlas-cyberknight-neutral.png`
- `sparky-cyberknight-neutral.png`
- `apollo-cyberknight-neutral.png`
- `codex-cyberknight-neutral.png`

Each supported cockpit identity also has one generated high-effort companion:

- `turbo-cyberknight-high.png`
- `atlas-cyberknight-high.png`
- `sparky-cyberknight-high.png`
- `apollo-cyberknight-high.png`
- `codex-cyberknight-high.png`

The masters are genuine 128x128 square head-and-shoulders rasters built from a
coarse, regular halftone-dot lattice on pure black. Cream and copper describe
faces, charcoal and steel describe armor, and restrained blue/violet accents
differentiate the agents while matching the cockpit HUD. Dots remain discrete
at native size; there are no continuous painted fills or smooth gradients.
Each pair preserves its agent's silhouette, clothing, composition, and identity.
The neutral master is the low/medium (including none, shallow, and unconfigured)
state. The high master adds restrained eye, temple, and armor signal accents for
high/ultra and equivalent top-tier efforts such as xhigh, max, and deep. Thinking
activity remains a separate caption/status signal and never changes the selected
portrait tier by itself.

The older neutral and active files remain unreferenced for visual comparison and
rollback; they are not part of the active portrait lane.
