# Scryglass teaching library

The Scriptorium is the world-owned teaching library. Its generated textures are
embedded into the authored raycast interior:

- `scriptorium-shelves.png` — five curriculum bays.
- `resident-tutors.png` — six original resident faculty portraits.
- `curriculum-books.png` — six domain book jackets.

These are Lanczos-downsampled runtime textures from full-resolution built-in
image-generation outputs, which are kept outside the repository. All three
prompts requested original, text-free pixel-art assets with no logos or
watermarks.

The curriculum catalog in `cockpit/src/library.rs` links rather than vendors
course content. Its upstream shelves currently include MIT 18.06 and 18.335,
GPU MODE, Tiny Renderer, Ray Tracing in One Weekend, `wgpu`, Bevy, Stanford
CS336, nanochat, MIT 18.S191, OSSU, and OpenStax Statistics, University
Physics, Chemistry, and Biology.
Every routed lesson prints the shelf's exact upstream GitHub URL after its
resident tutor prompt, so the catalog is learner-followable from the world.

The 16 linked repositories were checked on 2026-07-25: each resolved to a
public, non-archived GitHub repository, and the `wgpu` and Bevy `examples`
deep links resolved at their named refs. The MIT 18.S191 shelf uses the current
canonical `mitmath/computational-thinking` repository.

Catalog tests require every shelf to be reachable from representative lesson
context; secondary shelves are not decorative metadata.
