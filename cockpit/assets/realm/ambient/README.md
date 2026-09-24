# Location artwork

This directory contains room and location paintings, their generated sources,
prompts and `motion.json`. The terminal renderer uses these assets for explicit
location entry. Outdoor travel is rendered by Dotmax.

The admitted plates use the shared [Realm palette](../palette.json). Source art
is retained beside its processed plate. Runtime loading and animation are in
[world_viz/cinematics.rs](../../../src/stage/world_viz/cinematics.rs).
