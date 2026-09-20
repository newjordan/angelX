# Angel DMD tourney assets

The two generated originals in `source/` were produced with the built-in
OpenAI image generation tool. Production files are deterministic derivatives:

- `arena-plate.png` is a terminal-sized, eight-color posterization of the arena
  source.
- `mounted-knight-pose-sheet.png` is the chroma-key-removed pose sheet.
- `poses/*.png` are calibrated crops, normalized and posterized by
  `scripts/process-tourney-assets.py`.

Generated-source prompts (normalized production specs):

1. **Arena plate** — stylized-concept, very wide orthographic side-view empty
   jousting lane; sparse grandstand silhouettes, rail, four pennants, open black
   center; crisp flat screenprint shapes; only black, phosphor cyan, HUD blue,
   steel, pale blue, and heraldic gold; no text, riders, particles, gradients,
   dense fantasy detail, photorealism, or watermark; must survive 72×16 colored
   braille reduction.
2. **Mounted-knight pose sheet** — stylized-concept, one consistent armored
   jouster and horse in six isolated orthographic poses (idle salute, approach,
   canter, impact recoil, victory, retreat), identical scale/baseline, on a
   perfectly flat `#FF00FF` chroma background; crisp posterized Angel palette,
   generous separation; no shadows, text, labels, extra riders, overlaps,
   cropping, costume drift, or watermark.

The imagegen chroma helper was run with border auto-keying, soft matte,
thresholds 12/220, and despill. The transparent sheet has transparent corners;
the production renderer still has a hand-corrected bundled rider dot mask for
missing or corrupt files.

The ordinary cockpit's lower-right rider rotates this approach/canter pair
with the installed Grok `knight_charge` and `joust_gallop` sprite loops. Frames
are held for nine 33 ms world ticks (about 3.4 fps), and each visual treatment
stays in place for about 12 seconds. A realm-seeded permutation makes the order
varied but redraw-stable; the approach/canter pair remains the decode fallback.
