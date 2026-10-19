# Engine asks from realm/kit

What the lit realm (docs/REALM-MONOLITH.md, "The realm, lit") needs from
`crates/stage` and `crates/gpu`, found while proving it, and how realm/kit
works round each meanwhile.

## 1. The reveal on the stage camera: the eye is the stage's origin

**What happens.** `gpu/src/scene.rs` (about line 911) takes the look's eye as
`scene.camera.transform.translation`. The stage camera (`Tilt::camera`,
`stage_camera`) is `Transform::IDENTITY` with a `Projection::Custom` matrix,
so the eye is `(0, 0, 0)`, the stage's top-left corner on the floor.
`mesh.wgsl`'s `opened()` then measures the line of sight from there to
`Look::reveal`, so on a stage the reveal opens the wrong things (or nothing:
the occluders near the knight sit at `t` about 1 of that line, past the
0.92 taper). The same eye feeds fog and anything else measured from it.

**Ask.** For a camera whose projection is `Custom` (or always), take the eye
as the projection's camera centre: the point `C` with `M * C = 0` in the
clip x, y and w rows of the view-projection (its null vector; for the Tilt
camera a finite pinhole, `w = 1 - near * (z - cy)`, so at `z = cy + 1 / near`
on the stage's middle line). Or let `Look` carry the eye explicitly.

**Meanwhile.** Nothing in realm/kit can open what stands before a knight;
a knight behind a building or a wood is hidden past its helmet in the
window and on a page. As it stands the reveal opens a stray stippled hole
along the line from the stage's origin, so realm/kit leaves it off:
`realm_look::REVEAL_ON_STAGE` is `false`; turn it on when this lands.
Everything else is in place for when the eye is right: the reveal sits at the knight's feet plus `REVEAL_AT` (8 px) with a
30 px radius (`realm_look`), and the window draws the followed knight's
card once more inside the lit world as a `Role::Figure` node
(`stage_frame::kingdom_world`, ids from `0x7f00_0000_0000`), so the reveal
shows it (ask 2).

## 2. Figures behind a revealed mass: their depth is drawn without the reveal

**What happens.** `gpu/src/stage.rs` `draw_props` draws the lit world, then
the same nodes again with no look into a spare target for the depth the
figures (`World::figures`) are tested against. That depth pass knows no
reveal, so even with ask 1 fixed a figure behind what the reveal opens is
still hidden by the depth.

**Ask.** Apply the look's reveal in the depth pass too (discard what
`opened()` opens), or draw the figures with the lit world when the world has
a look that lights it.

**Meanwhile.** The window's copy of the followed knight in the lit pass
(ask 1, "Meanwhile") is the workaround; only the followed knight, since the
reveal opens round one point. A page cannot have it (ask 3).

## 3. The stage wire drops a node's accent and role

**What happens.** `stage/src/wire.rs` `encode_node` carries `lit`, alpha,
sampling, double-sided and the picture, but not `Material::accent` or
`Material::role`. Nodes that cross as nodes (`World::extra`, `Props::nodes`)
arrive as plain: a lit window or a fire loses its accent, a figure card its
`Role::Figure`, water its `Role::Water`.

**Ask.** Two more flag bits for `accent`, and two bits for the role (plain,
water, figure, sky), in `encode_node` / `decode_node`.

**Meanwhile.** What a run stands in the kingdom (`world3d::lots`) goes to a
page as a set of its own a chunk (`Share::text`, `set lots`) which the page
fills lit, so its windows and fires keep their accents; the followed
knight's copy (ask 1) is the window's only.

## 4. The reveal's margins are metres

**What happens.** `opened()` keeps what is under `r.y - 0.7`, and faces
turned up under `r.y + 1.3`, and its doc takes the point as a chest 1.2 over
the feet: metres, as the monolith proof is. On a stage `Look::metre` is
about 11 (the kingdom's pixels), so those margins are a pixel or so.

**Ask.** Scale those three by `Look::metre`.

**Meanwhile.** `realm_look` puts the reveal point 8 px over the feet, low
enough that the plinths before a knight are opened and the ground is not.

## 5. A page fills a lit set a node a shape

**What happens.** `Sets::fill` makes a lit set's nodes one a shape
(`Set::lit_world_nodes`). A kingdom view's sixteen chunks are a few
thousand nodes, each drawn three times (shadows, lit, the figures' depth).
In headless Chromium (WebGL2 over ANGLE GL, RTX 3090) the page still holds
60 fps (presenting 6.6 to 9.0 ms p95), but the window, which bakes, draws the
same view in a tenth of the draws.

**Ask.** Bake a lit set in `Sets::fill` as `delve::world3d::kingdom::bake`
does for the window: the nodes of one material (its tint aside) made one
uploaded mesh, each vertex coloured with its node's tint, its id from the
set's key; a node with face materials kept as it is. The function is
generic over a `Set` and could move to `angel_stage::set` as it stands.
