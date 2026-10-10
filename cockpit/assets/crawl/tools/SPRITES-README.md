# Sprite tools

The ten sprites were generated with ten separate built-in `image_gen` calls, with genuine transparent backgrounds. `../sprites/sources/` retains each generated original. No network access, API key, or model call is needed to rebuild the delivered assets.

Run from the worktree root in an environment with Pillow:

```sh
python cockpit/assets/crawl/tools/build_sprites.py --qa
```

With the project's cached uv environment:

```sh
UV_OFFLINE=1 uv run --quiet --with pillow python cockpit/assets/crawl/tools/build_sprites.py --qa
```

`sprite-prompts.json` preserves the exact generation prompts: concatenate `common`, one newline, and the named `sprites` value. Raise and swing use the idle source as a reference; torch B uses torch A; campfire B uses campfire A. Each source was visually inspected before processing. Source and prompt hashes, input references, and source dimensions are recorded in `sprite-source-manifest.json`.

`sprite-layout.json` defines the exact output size, fitted silhouette box, and eligible nonemissive inks. The script imports the existing palette from `build.py`. It crops the high-alpha source bounds, downsamples RGBA with BOX (preserving transparent pixels), applies a binary alpha cutoff, and snaps opaque pixels to the nearest eligible ink without dithering. Fire tongues have their own restricted ink set. No white or glow is added to armor or stone.

All sword poses share the four-row mail cuff at `x=25..39`, `y=44..47`. Raise has an extended wrist so its hand sits higher while the bottom attachment stays fixed. Flicker frames copy their A-frame non-flame pixels, including the torch head/hand and the campfire logs/stones. `sprite-retouch.json` then applies retained pixel cleanup: a patch has `x`, `y`, and `rows`; `_` keeps the original pixel, `.` clears it, and any other character paints the corresponding palette ink. Patches restore book marks, candle, chest hardware, and crossed-log edges.

To preserve manual edits made directly to the `.ink` files:

```sh
python cockpit/assets/crawl/tools/build_sprites.py --from-ink --qa
```

Transfer lasting changes into `sprite-retouch.json` so a rebuild from originals retains them. Literal transparency is supported in patches.

The build writes all ten `.ink` files, matching RGBA PNGs, and `../sprites/sheet.png`. Sprites are enlarged exactly 6x with nearest-neighbor sampling. The upper row shows the three sword poses side by side, with cuff-anchor guides. The next row shows every other sprite. The bottom contains a 288x160 corridor composited at 6x with hands at the bottom corners and props for context. The corridor is only a review background.

`--qa` adds readable sheet crops, each of the three sword poses with the torch in corridor context, and a one-color-per-2x4-dot approximation under `sprite-qa/`. The dot preview uses a simple brightness threshold and average ink, not the engine's actual dithering implementation.

`sprite-validation.json` is regenerated on every build. Checks cover exact ink/PNG dimensions, allowed characters and colors, exact ink-to-PNG agreement, binary alpha, bottom hand attachment, matching sword cuff pixels, stable flicker substrates, visible flicker differences, and signal inks confined to actual flame regions.
