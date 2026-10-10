The eleven textures began as eleven separate built-in imagegen calls. Every generated original was visually inspected. `../sources/<name>.png` preserves the original for each texture. `prompts.json` preserves the exact prompts: concatenate `common`, a newline, and the corresponding `textures` value. No API key or image generation is needed to rebuild the existing textures.

From the worktree root, regenerate the inks, PNGs, sheet, and validation report with:

```sh
UV_OFFLINE=1 uv run --quiet --with pillow python cockpit/assets/crawl/tools/build.py
```

Alternatively, run `python cockpit/assets/crawl/tools/build.py` in any environment with Pillow installed.

The pipeline downsamples with Pillow BOX to 32×32, snaps each pixel to its nearest eligible brief ink without dithering, removes isolated majority-surrounded flecks, applies the hand-authored patches in `retouch.json`, and matches the required boundary pixels. Eligible inks are a restrained subset per surface; signal inks are reserved for the Keep embers, fungal bioluminescence, and unknown eye. Tie-breaking is deterministic. `source-manifest.json` records source hashes and generation file IDs.

Retouch patches contain an `(x, y)` origin and rows of ink characters; `.` leaves the underlying pixel unchanged. Change a patch and rerun to preserve the full processing history. You can also edit an `.ink` directly and use:

```sh
UV_OFFLINE=1 uv run --quiet --with pillow python cockpit/assets/crawl/tools/build.py --from-ink
```

`--from-ink` preserves the edited pixels and validates the required seams. Transfer direct edits to `retouch.json` to keep them when rebuilding from originals.

`sheet.png` shows every final texture enlarged exactly 8× on black, with three contiguous horizontal copies for wall surfaces, doors, and stairs, and two-by-two copies for floor and ceiling. The small swatches show the native 32×32 size. Add `--qa` to write four sheet crops under `tools/qa/` for inspection at a readable scale.

`validation.json` is produced during each build. Validation checks ink dimensions and characters, PNG dimensions and exact agreement with ink data, horizontally matching boundaries, vertically matching floor/ceiling boundaries, and restriction of signal inks to actual light-bearing textures. The textures and final sheet were visually inspected after cleanup; these pixel checks complement that inspection.
