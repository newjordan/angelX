# Doppelgangers

Fifteen figures, one per model family, kept for the realm and not yet placed.

They come from the first self-portrait poll (2026-09-23). Each model was asked
through its own angelX route (`angel --ask`) how it wanted to be drawn, then
saw the whole cast in a council round and changed its look to stand apart. That
question said menace was welcome, and every model drew a darker twin of
itself: a broken-halo judicator, a ram-horned reaver, an executioner with a
key-bladed axe, a headless mourner under a slipped halo. The portraits come
from the neutral poll that followed; these twins stay here until the world
has a place for them.

- `sprites/`: the 96px sprite each twin would wear, drawn at 2× in a 192px
  frame (transparent PNG).
- `qwen/`: Qwen-Image 2.1 renders, transparent, 512px.
- `luna/`: Luna (gpt-5.6-luna) renders on black, 512px.
- `manifest.json`: each twin's model, its own words (figure, title, look, why),
  how it got there, and file hashes.

Nothing in the cockpit reads this directory yet. The full-resolution renders,
including the AGY and Z-Image versions, every prompt and the poll transcripts,
are in the gitignored `artifacts/doppelgangers/`.

Qwen-Image 2.1 ran locally in ComfyUI. Qwen is licensed under the Qwen RESEARCH
LICENSE AGREEMENT, Copyright (c) 2026 Hangzhou Tongyi Laboratory Technology
Co., Ltd. All Rights Reserved.
