# Mossbell composed pieces

These pieces use the delivered `mossbell.*.geometry-1` art, not new drawings.
`composition.json` records the source-sheet SHA-256, crop rectangles, copied crop
hashes and output hashes. The three source crops are retained here.

`scripts/mossbell-compose.py` downsamples each crop once and snaps it to the
existing shared ink palette (maximum channel 224). Fixed native caps and pivots
are retained; only the straight middle is repeated or stretched. Flippers use
integer column shears between the existing projected physics pivots and tips.
The root's leading bud stays at the existing progressive reach, before the
plunger wall. Rightward art is mirrored in the existing renderer's cache.

The crown source contains two arcs, so only its left leaf bar is imported.
The existing four jaw silhouettes register the material, preserving every
outside pixel and safe gap. Transparent source gutters are projected from the
nearest opaque sample in the same source column. This avoids perforated tells.
The broad corner posts follow the old mask; leaf bars use the six-row material.
The dark leaf material merges with the Keeper in the real terminal cells;
that path retains the old four crown frames. RGB uses the composed leaf frames.

Legacy x5/4 and current x8/5 outputs document both contracts: 42/53x5 paddles,
114/144x26 pairs, 179/229x11 root bands, and 51/65x24 crown gates. The current
renderer uses the current set. It decodes these tiny PNGs into its existing
OnceLock cache and falls back to old art when a composition is disabled or has
the wrong dimensions. Rules, hit shapes and timing are unchanged.

Reproduce with `python scripts/mossbell-compose.py`; verify with `--check` and
`python scripts/test-mossbell-compose.py`. `mossbell-fit-review` in
`scripts/slot-art-review` exports four-seat native/cell fixtures and a complete
1800-tick four-bot rule trace. `scripts/mossbell-fit-evidence.py <artifact-dir>`
builds bounded before/after contacts from those exports.
