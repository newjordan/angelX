# Chariot sprites

Two-horse team and chariot in the realm ink palette, two gallop frames a
heading: side view east (`chariot_e_a/b.ink`, 40×22; west is the mirror),
front view south (`chariot_s_a/b.ink`, 24×30) down the east end, and rear
view north (`chariot_n_a/b.ink`, 24×30) up the west end — so corners never
ride sideways. The far/left horse wears the fixed bay coat `B`; `R` is the
house colour (near horse, cab, driver) and `I` its shadow, both repainted
per house banner by the arena; `O` is the gold harness, `x` the cab's iron,
`b` hooves. The driver wears the hero's steel helm, and every coat is lit
from above by the generator's `shade` pass. Authored as pixel sprites underneath;
the braille/dotmax surface renders them, and the mini-game view shrinks its
text size so the sprites read at SNES density, not Atari blocks.
Regenerate: `python3 cockpit/assets/dungeon/chariot/make_chariot_sprites.py`
(in this folder); palette chars must exist in
`crates/delve/src/render/overworld/ink.rs`.
