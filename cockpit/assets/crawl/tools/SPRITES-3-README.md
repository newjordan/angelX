# Brief 3 sprite tools

`build_sprites_3.py` builds only `chest_open`, `sconce_a`, `sconce_b`, `banner`,
their transparent PNGs, and `../sprites/sheet-3.png`. Earlier textures and
sprites remain unchanged. All four source images were produced by separate
built-in `image_gen` calls with `transparent_background=true`, inspected,
and retained at `../sprites/sources/<name>.png`.

From the worktree root, with Pillow installed:

```sh
python cockpit/assets/crawl/tools/build_sprites_3.py --qa
```

Alternatively, in a writable cached uv environment:

```sh
UV_OFFLINE=1 uv run --quiet --with pillow python cockpit/assets/crawl/tools/build_sprites_3.py --qa
```

The exact prompt set is `sprite-3-prompts.json`: concatenate `common`, one
newline, and the named `sprites` value. The open chest uses the delivered
closed `sprites/png/chest.png` as its edit target. Sconce B uses the retained
sconce A source. `sprite-3-source-manifest.json` records input references,
dimensions, source hashes, prompt hashes, and visual inspection of originals.

`sprite-3-layout.json` retains exact source crops and target placement. The
open chest's raised lid and treasure cavity are fitted separately above the
original front. Sconce B uses A's source crop to avoid changing its frame
placement when the flame changes. RGBA is reduced with BOX, alpha is cut to
binary transparency, and opaque pixels snap to eligible inks from `build.py`.

`sprite-3-retouch.json` stores the final pixel cleanup. Each patch has `x`,
`y`, and `rows`; `_` preserves a pixel and `.` erases it. Cleanup makes the
coin highlights and single teal gem readable, gives the iron bracket broader
gray highlights, makes the two flame silhouettes lean in opposite directions,
and strengthens the banner's pale sword and crossguard. The cloth uses
nonemissive `p B I R X b` for its faded rust-red field; the palette's signal
red `7` is reserved for flame.

After cleanup, the chest's rows 8 through 15 are copied verbatim from
`chest.ink`. Its strap columns 4, 5, 14, 15 at rows 1 through 14 are also copied
verbatim, preserving the original band positions, widths, and shading. Only
the upper wooden lid, top edge, and contents change. Sconce B copies every
pixel of A at rows 8 through 19; only flame pixels in rows 1 through 7 differ.

To rebuild from directly edited inks without overwriting them with source
quantization and retouch patches:

```sh
python cockpit/assets/crawl/tools/build_sprites_3.py --from-ink --qa
```

Transfer permanent ink changes into the retouch file for reproducibility.
Both build modes validate the required animation invariants.

The 6x sheet shows closed and open chests side by side, then both sconces and
the banner on the unchanged `wall_stone` texture. A second wall vignette
places all fixtures together, with the chest pair below. Sprites and texture
pixels are enlarged exactly 6x with nearest-neighbor; labels are review-only.
`--qa` writes a plain-background sprite strip and a one-ink-per-2x4-dot
approximation. The dot approximation is not the crawler's Rust renderer.

`sprite-3-validation.json` records exact dimensions, valid ink characters,
pixel-for-pixel ink/PNG agreement, binary alpha, restricted signal usage,
treasure presence, fixed chest parts, flame-only flicker changes, and sheet
scale. `sprite-3-baseline.json` fingerprints the 87 earlier files present at
the start of this brief. All 86 other than the authorized sprite notes must
stay unchanged; deliberate later changes to those earlier assets require
updating that baseline before this preservation check will pass again.
