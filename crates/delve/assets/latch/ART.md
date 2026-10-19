# Clasp and Latch Duel

Clasp is an original copper practice automaton: hinged hood, warm rivet eyes,
leather bellows, hook and floor rake. No borrowed character or franchise.
Every 80x80 state shares a baseline and one conversion scale: idle, high tell,
high attack, low tell, low attack, flinch, one crease, two creases, defeat.
Four 32x32 frames each: parry crescent, contact clash, riposte slash.

Rebuild: `python3 scripts/latch-art.py <source-dir> --review <review-dir>`.
Source directory has `clasp.png` and `effects.png`; retained at
`/work/artifacts/boss-art-20261015/game-2/source/`.
The converter uses the existing Cinder converter's palette helpers and
nearest sampling, 16 inks, connected-cluster cleanup, a coloured 1px contour,
and hand placed rivet lights after cleanup. It retains dark neutral metal;
only the midgray backdrop is removed. Effects use changing connected sizes.
Room, decoded art, action damage overlays, terminal exposure and 2x pops are
OnceLock caches. Contact is solid half-block ink, with a restrained braille
arc around a clean hit. Terminal-only sockets keep both rivet lights visible.

Rejected: removing every low-saturation pixel hollowed out the bellows and
feet; fixed by separating midgray backdrop from dark metal. Source highlights
were too bright, so the palette maps them to muted parchment/teal. A neighbour's
hook tip crossed a source panel; only the main connected figure is retained.
First room spacing felt ranged; the knight grows to 3x nearest size, the rival
moves 30px closer, and the three contact effects meet in the same gap.

State prompt:
Finished pixel art, a 3x3 sheet of 80x80 sprites at common size and baseline,
front three-quarter facing left. Same Clasp: rounded copper hood with two
amber rivet eyes, leather bellows torso, iron feet, raised hook and blunt low
rake. Idle; high wind-up; high strike; low wind-up; low strike; hit flinch;
one dark plate crease; two dark creases around amber core; slumped defeat.
4-tone hue-shifted ramps, 1px coloured outline, 12-16 inks, flat neutral gray,
no text, dither, speckles, bloom, white, smoothing or commercial characters.

Effect prompt:
3x4 sheet of 32x32 pixel sprites, flat neutral gray, four consistent frames
per row. A blue-teal parry crescent opens and recedes; a copper/amber diamond
clash expands into blunt connected petals and recedes; a muted teal diagonal
riposte slash grows into a curved wedge and fades. 3-5 colour clusters,
coloured contour, clean connected shapes; no character, text, white, bloom,
dither, particles, isolated pixels or blurry edges.
