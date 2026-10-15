# /setup art

The `/setup` menu (cockpit/src/app/setup_menu.rs, drawn by
cockpit/src/ui/views/setup_view.rs) reads the two images in this folder at run
time. Either may be missing: without `guide.png` the scroll stands alone, and
without `scroll.png` the scroll is plain parchment cells.

## guide.png: the recruit who explains the menu

The cream-gi recruit with the brown headband, in eight side-view action poses
facing right, toward the scroll. It is one RGBA sheet of 4 × 2 cells, 480 × 512
each (1920 × 1024, under the 2,000,000-pixel sheet limit), repacked from the
art pass's irregular action sheet so every pose stands on the same ground
point: x 216, y 482 within its cell.

| cell | pose             | the menu uses it for                               |
|------|------------------|----------------------------------------------------|
| 1    | idle             | typing a server address                            |
| 2    | wave             | the first two seconds of the menu                  |
| 3    | point            | every list: the main choices, providers, models    |
| 4    | lift-key         | pasting an API key or a server key                 |
| 5    | pull             | anything running (checking a key, signing in,      |
| 6    | strain           | installing), alternating with pull every 0.7 s     |
| 7    | stumble-recovery | a problem: a refused key, a failed sign-in         |
| 8    | thumbs-up        | good news, and "You're all set!"                   |

The cockpit crops all eight poses by the union of their outlines (alpha ≥ 32)
and scales them once, so the feet stay put while arms and props reach out
(`helm::anchor_registered_frames`). Kitty draws it as a picture; other
terminals get half-blocks.

## scroll.png: the backdrop

The parchment tooltip from the torch-lit UI atlas: a wide scroll with a roller
at each end, transparent around it. It is drawn in half-block cells and
stretched nine-slice, so these fractions of the visible plate (cropped to
alpha ≥ 128) must hold for a new plate, or the constants at the top of
setup_view.rs must be re-measured:

- the left roller ends at 12.5% of the width, the right one starts at 88.4%;
- the knobs and torn top edge end at 25% of the height, the bottom edge starts
  at 76.5%.

The parchment between those lines is the writing surface and is the only part
that stretches.
