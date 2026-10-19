# Pennon Rush native art

The siege captain's copper fan crest, swallowtail pennant and open ladder-hook
are the three identity props. The sentry has a forked vane, bow and seed weight.
The knight has twelve articulated bodies, seat-coloured plume and tabard, and a
one-native-pixel seat contour through overlaps. No sprite is scaled against another.

The retained concepts and exact requests are in
`/work/artifacts/boss-art-20261015/game-6/source/`: captain.png, knight.png,
sentry.png, effects.png and prompts.md. Concepts use the built-in image tool.
Direct nearest reduction was rejected for fragmented armour. Broad-tone cleanup
alone erased faces, plumes and banner fills and was also rejected. The selected
recipes reconstruct the concepts with deliberate native material planes and long
ribs, hems, bow limbs and rims. These details retain their LINEAR paths and original
underlays; untagged tiny patches still fail the shared boss gate.

Every material has exactly four hue-shifted tones. A figure or effect uses at most
ten inks, including a protected curved 3x2 amber slit with a one-pixel falloff.
The global ink palette is unchanged. The captain is 77px tall in an 80px cell;
the knight is 45px tall in a 48px cell. Both use the same grain as the stone and cloth.
The captain's helmet/body mean separation is 22.3 L*. Its outer contour is over
15 L* above the dark backdrop. Damage opens a broad dark-backed plate split around a warm core,
then a second plate split and crest dent. Both attacks have four anticipation poses
and exact fixed danger edges, followed by distinct four-frame solid effects.

Reproduce with:
`python scripts/pennon-art.py SOURCE_DIRECTORY REVIEW_DIRECTORY`
`python scripts/pennon-art.py --check`
`python scripts/pennon-review.py /work/artifacts/boss-art-20261015/game-6`
The shared boss gates check palette, four-tone masses, explicit narrow features,
signatures and singleton provenance. Native sheets show every asset at 1x and 3x;
the cockpit writer supplies actual half-block/braille buffers and cell-difference gates.

All sprites, recolours, contours, large glyphs and three seeded battlement layouts
bake into OnceLock caches. Per-frame drawing uses integer pixel sampling. Rafterwake's
image, mirror, seat recolour, contour and glyph helpers are shared without changing its
art, contacts, camera behavior or gameplay numbers. Terminal attack impacts and hit bursts stay solid in front of their targets;
only narrow identity strokes use the existing braille detail layer.
