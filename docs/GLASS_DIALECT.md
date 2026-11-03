# glass's own dialect, version 3

This is the reference for programs that want more than any picture
dialect can say. glass takes in pictures through every common dialect;
this one lets a program speak to glass directly and reach what the
window itself can draw: layers of every kind, groups replaced whole
per frame, particles, lit primitives, a placed 3D scene, glTF models
whose clips glass plays itself, tweens that glass eases every frame
while the program says nothing, and the showcase (a picture, a video,
a chart, a model or a page). It is one more dialect glass takes in,
alongside the others.

Other terminals ignore unknown APC strings, so a program can send these
blind. `hello` lets it ask first and know.

## Framing

```
ESC _ glass ; <op> [; <key>=<value>]... [; <payload>] ESC \
```

- `ESC _` is APC (`0x1b 0x5f`); `ESC \` (ST, `0x1b 0x5c`) or BEL
  (`0x07`) terminates, as for other APC strings.
- The body starts `glass;` and is pure ASCII until the payload.
- Fields sit behind `;`. Each key field is one `key=value`; a value may
  hold commas (`at=2,1,4,2`), so never split a value on commas.
- The payload, when the op takes one (`picture`, `group`, `scene`,
  `frame`, `chart`, `model`, `page`, `asset`), is the last field: base64
  (standard alphabet, padded or not). The payload ops always carry keys
  before it, so a last field that parses as a `key=value` is a key, and
  a payload is recognised by shape: it holds no `=`, or its `=` is only
  base64 padding at the very end.
- Every key, the reply token `r=` included, goes before the payload:
  `picture;id=1;f=32;s=2;v=2;r=7;<payload>`, never `...;<payload>;r=7`.
- Ids are chosen by the program. Each program's ids live in this
  session's own space. A program id is any `u32` below 2^30.

### printf, in bash

```bash
# hello, with a reply token
printf '\033_glass;hello;r=1\033\\'

# one red glow, two cells wide, under the text
printf '\033_glass;layer;id=7;k=glow;c=ff0000;at=2,1,4,2\033\\'

# a raw RGB picture, 2x1 pixels
printf '\033_glass;picture;id=1;f=24;s=2;v=1;%s\033\\' "$(printf '\xff\x00\x00\x00\xff\x00' | base64)"

# replace a whole group in one go: here two layers
printf '\033_glass;group;g=1;n=2;r=2;%s\033\\' "$(printf 'layer;id=1;k=solid;c=ff0000;at=0,0,1,1\nlayer;id=2;k=solid;c=00ff00;at=1,1,1,1' | base64 -w0)"
```

## Discovery

```
ESC _ glass ; hello ESC \
```

is answered

```
ESC _ glass ; r=<token> ; OK ; v=3,ops=hello,picture,layer,group,remove,showcase,scene,frame,chart,model,page,progress,asset,tween,play ESC \
```

A program sends `hello` (with `r=`) then DA1 (`CSI c`). If the DA1
answer comes first with no hello answer before it, the terminal is not
glass: stop and draw plain. Replies go out in stream order with every
other reply, through `pump`.

## Id ranges

| who | ids |
| --- | --- |
| program ids (this dialect) | `0`..`2^30-1` |
| this dialect's window picture ids | `2^30`..`2^31-1` (program id + 2^30) |
| this dialect's window layer ids | `2^30`..`2^31-1` (program id + 2^30) |
| an asset's texture pictures | `2^31`..`0xBFFFFFFF`, minted |
| inline and asset meshes | from `0xC0000000`, minted |
| scene node ids a program names | below `2^63` |
| an asset's nodes in a scene | `2^63` and up, the window's own |
| the store's minted ids, the intake's client ids and everything else | below `2^30` |

Assets have an id space of their own (`asset id=`, `remove asset=`, a
scene's `"asset":N`), program ids like the others.

A program id is mapped by `+ 2^30` into a window id that cannot meet
the intake's picture group ids or anything else glass draws. `remove
id=a-b` spans program ids.

## picture

Upload a picture under a program id. The payload is base64 of the
picture bytes.

| key | type | range | default | meaning |
| --- | --- | --- | --- | --- |
| `id` | u32 | 0..2^30 | — | the program id to upload under |
| `f` | 100\|32\|24 | 100, 32, 24 | 32 | PNG (100), RGBA (32) or RGB (24) |
| `s` | u32 | 0..4096 | 0 | width in pixels, for `f=32`/`24` |
| `v` | u32 | 0..4096 | 0 | height in pixels, for `f=32`/`24` |
| `o` | `z` | `z` | — | zlib-compressed payload (as the APC graphics dialect) |
| `m` | 0\|1 | 0, 1 | 0 | `1` on every piece but the last of a chunked transmission |
| `r` | token | ascii | — | reply token; without it the op is silent |

Chunking follows the APC graphics dialect's own rule: `m=1` on every
piece but the last, the first piece's keys govern, and only the last
piece answers. A payload-less continuation writes its last field empty
(`picture;id=9;m=0;`). Decoding runs through the APC graphics
adapter's own loader, under its caps (4096 px per side, 64 MiB) and the
store's budget.

Reuploading under an id replaces that picture.

 Replies: `OK`, or `EINVAL` (bad keys, id out of range, format the
loader refuses), `EBADF` (payload not base64), `EFBIG` (past the
budget), `ENOMEM` (budget full).

## layer

Create or replace one layer by `id`.

| key | type | range | default | meaning |
| --- | --- | --- | --- | --- |
| `id` | u32 | 0..2^30 | — | the program id; reusing it replaces the layer |
| `k` | solid\|picture\|part\|glow\|particles\|prim\|ring\|line | | solid | the kind |
| `z` | i32 | −2^15..2^15 | 0 | depth; negative sits under the text |
| `o` | f32 | 0..1 | 1 | opacity; outside the range is an error, not a clamp |
| `a` | f32 | radians | 0 | angle |
| `h` | f32 | 0..8192 cells | 0 | lift height (how far the layer rides above its stand) |
| `b` | f32 | 0..8192 cells | 0 | breathe amplitude |
| `add` | 0\|1 | | 0 | additive blend |
| `fl` | f32 | 0..1 | 0 | flicker amount |
| `stand` | floor\|up\|screen | | floor | how it stands: floor, upright at `up=`, or screen |
| `up` | x,y | cells | 0,0 | the upright foot, for `stand=up` |
| `at` | x,y,w,h | cells, fractions allowed | — | placement in cells, anchored to the text (default) |
| `px` | x,y,w,h | pixels | — | placement in window pixels, always pinned to the window |
| `anchor` | window | | text | `anchor=window` pins an `at=` placement to the window |
| `c` | rrggbb or rrggbbaa | hex | ffffff | colour, straight RGBA in 0..1 |
| `p` | u32 | a picture id | — | picture program id, for `k=picture`/`part` |
| `src` | x,y,w,h | pixels | — | source rect, for `k=part` |
| `n` | u16 | 1..4096 | 64 | mote count, for `k=particles` |
| `size` | f32 | 0.5..64 px | 3 | mote size |
| `life` | f32 | 0.05..600 s | 2 | mote life |
| `rise` | f32 | −8192..8192 px/s | 20 | rise speed |
| `sway` | f32 | −8192..8192 px | 8 | sway width |
| `seed` | u32 | | 0 | mote seed |
| `burst` | f32 | | — | burst, once, when given |
| `shape` | 0..6 | | 0 | lit primitive shape, for `k=prim` |
| `speed` | f32 | | 1 | primitive speed |
| `phase` | f32 | radians | 0 | primitive phase |
| `lw` | f32 | 0.25..256 px | 1.5 | stroke width, for `k=ring`/`line` |
| `glow` | f32 | 0..256 px | 0 | how far a stroke's soft light reaches, for `k=ring`/`line` |
| `r` | token | | — | reply token |

Every kind: `k=solid` (a colour block), `k=picture` (a whole uploaded
picture, `p=`), `k=part` (a source rect of one, `p=` + `src=`),
`k=glow` (a soft light), `k=particles` (an emitter), `k=prim` (a lit
primitive), `k=ring` and `k=line` (strokes).

A stroke is drawn by its distance, so it stays sharp at any size and
turn, with a gaussian glow round it. A ring is the circle round the
placement's middle, its diameter the placement's shorter side. A line
runs along the placement's middle from its left edge to its right,
turned by `a` about the placement's middle: a line from (x0, y0) to
(x1, y1) is `px=` centred on its midpoint, as wide as its length, at
`a=atan2(y1-y0, x1-x0)`. The window draws past the placement by half
the width and three glows, so neither is cut. With `add=1` strokes add
light where they cross, which is how they glow into each other. Hosts
that draw only quads show a line as a turned bar and a ring as a loop of
48 bars, without the glow.

`at=` is anchored to the text: it scrolls with the text exactly as
intake pictures do, using the same anchor maths in the store, on the
screen it was placed on. The anchor is the cursor's absolute line at
placement time; `at=`'s row counts from there. `anchor=window` pins
it. `px=` places it in window pixels, always pinned.

Bash:

```bash
# a glow under some text, additive
printf '\033_glass;layer;id=1;k=glow;c=8844ccff;add=1;at=0,0,80,3\033\\'

# particles
printf '\033_glass;layer;id=2;k=particles;c=ffd080;n=120;size=2;life=3;rise=30;sway=12;at=0,20,80,4\033\\'

# a lit primitive
printf '\033_glass;layer;id=3;k=prim;shape=3;c=4060ff;at=30,8,20,10\033\\'

# a glowing ring, and a line across it
printf '\033_glass;layer;id=5;k=ring;c=40b0ff;lw=2;glow=7;add=1;px=400,200,240,240\033\\'
printf '\033_glass;layer;id=6;k=line;c=ff60c0;lw=1.5;glow=4;add=1;a=0.785;px=370,319,300,2\033\\'

# window-pinned, in pixels
printf '\033_glass;layer;id=4;k=solid;c=000000;o=0.5;px=0,0,1920,32\033\\'
```

## group

Replace a whole set of layers in one go, for per-frame animation. The
payload is `n` layer records, newline-separated, in the same key syntax
as `layer`, base64-encoded.

| key | type | range | default | meaning |
| --- | --- | --- | --- | --- |
| `g` | u32 | | — | the group id |
| `n` | u32 | 0..65536 | — | how many records the payload holds |
| `r` | token | | — | reply token |

A group of up to the stage's layer limit (65536) must not stall the
reader; the measured time for 10 000 layers is printed by the test
(`a_group_replaces_whole_and_ten_thousand_layers_settle_fast`): parse
about 4 ms, with the frame about 6 ms, on the CI machine. Replacing a
group id does not add: it replaces whole.

Bash, animating a glow:

```bash
g=1
while true; do
  for t in $(seq 0 49); do
    y=$(( t / 5 ))
    printf '\033_glass;group;g=1;n=1;%s\033\\' \
      "$(printf 'layer;id=1;k=glow;c=ff8800;at=10,%d,8,2' "$y" | base64 -w0)"
    sleep 0.033
  done
done
```

## remove

| key | type | range | default | meaning |
| --- | --- | --- | --- | --- |
| `id` | u32 or `a-b` | 0..2^30 | — | one layer or scene, or a span of program ids |
| `g` | u32 | | — | one group |
| `asset` | u32 or `a-b` | 0..2^30 | — | one asset, or a span; a scene showing it keeps it until the scene goes |
| `all` | 0\|1 | | — | `1` removes everything this dialect made, assets and tweens too |
| `r` | token | | — | reply token |

A removed layer or scene takes its tweens with it.

```bash
printf '\033_glass;remove;id=3\033\\'
printf '\033_glass;remove;id=10-19\033\\'
printf '\033_glass;remove;g=1\033\\'
printf '\033_glass;remove;asset=1\033\\'
printf '\033_glass;remove;all=1\033\\'
```

## showcase

Open the showcase on an uploaded picture, or open a video panel of a
pixel size. The panel opens and changes only when this request changes,
never once per frame. Frames and progress ride beside it.

| key | type | range | default | meaning |
| --- | --- | --- | --- | --- |
| `id` | u32 | an uploaded picture id | — | open on that picture |
| `s` | u32 | 1..=2048 | — | video panel width, with `v` |
| `v` | u32 | 1..=2048 | — | video panel height, with `s` |
| `fit` | contain\|width | | contain | how the panel fits |
| `events` | 0\|1 | | 0 | `1` opts into showcase events |
| `close` | 0\|1 | | — | `1` closes the showcase |
| `r` | token | | — | reply token |

`id` and `s`/`v` are alternatives. `events=1` may travel alone or with
either. Closing does not clear the opt-in; `events=0` or a reset does.
Sending the same video size and fit again does not reopen the panel.

```bash
printf '\033_glass;showcase;id=5\033\\'
printf '\033_glass;showcase;s=160;v=90;fit=width;events=1\033\\'
printf '\033_glass;showcase;close=1\033\\'
```

## scene

A 3D scene as data. The payload is base64 of a versioned JSON envelope.
The window draws it into a picture of `s` by `v` pixels and places that
picture the way a layer is placed. `remove id=` covers the scene as
well as a layer of the same program id. A text-anchored scene leaves
with its text, the same way a layer does.

| key | type | range | default | meaning |
| --- | --- | --- | --- | --- |
| `id` | u32 | 0..2^30 | required | program id; also the picture id |
| `at` | four numbers | finite, width and height > 0 | — | cells, anchored to text |
| `px` | four numbers | finite, width and height > 0 | — | window pixels, pinned |
| `anchor` | text\|window | | text | `window` pins an `at` placement |
| `s` | u32 | 1..=2048 | required | render width, pixels |
| `v` | u32 | 1..=2048 | required | render height, pixels |
| `z` | i32 | | 1 | layer order |
| `o` | number | 0..=1 | 1 | opacity |
| `r` | token | | — | reply token |

The envelope is pinned so the window's internal types can change behind
it. Unknown fields are errors. Every number must be finite.

```json
{"v":1,"scene":{"camera":{...},"nodes":[...]}}
```

| field | type | range | default |
| --- | --- | --- | --- |
| `v` | number | 1 | required |
| `scene` | object | | required |
| `scene.camera` | object | | the defaults below |
| `scene.camera.eye` | [number, number, number] | finite | [0, 0, 3] |
| `scene.camera.target` | [number, number, number] | finite, not the eye | [0, 0, 0] |
| `scene.camera.up` | [number, number, number] | finite, not parallel to the view | [0, 1, 0] |
| `scene.camera.fov` | number | (0, pi) radians | pi/4 |
| `scene.camera.near` | number | > 0 | 0.1 |
| `scene.camera.far` | number | > near | 100 |
| `scene.camera.clear` | [number x4] or null | each 0..=1; null draws over | [0, 0, 0, 0] |
| `scene.camera.ortho` | number | > 0; present selects orthographic | absent (perspective) |
| `scene.nodes` | array | at most 256 | [] |
| `scene.nodes[].id` | number | integer below 2^63, unique | required |
| `scene.nodes[].parent` | number or null | an earlier node id | null |
| `scene.nodes[].at` | [number x3] | finite | [0, 0, 0] |
| `scene.nodes[].rot` | [number x4] | finite quaternion, not all zero | [0, 0, 0, 1] |
| `scene.nodes[].scale` | [number x3] | finite | [1, 1, 1] |
| `scene.nodes[].mesh` | string or object | see below | none |
| `scene.nodes[].color` | [number x4] | each 0..=1 | [1, 1, 1, 1] |
| `scene.nodes[].visible` | bool | | true |
| `scene.nodes[].lit` | bool | | false |
| `scene.nodes[].emissive` | number | 0..=1 | 0 |
| `scene.nodes[].picture` | number | an uploaded picture id | none |
| `scene.nodes[].sampling` | `linear` or `nearest` | | `linear` |
| `scene.nodes[].chrome` | number | 0..=1 | 0 (off) |
| `scene.nodes[].liquid` | number | 0..=1 | 0 (still) |
| `scene.nodes[].faces` | array of up to 6, each a picture id or null | a `cube` or `roundcube` mesh | none |
| `scene.nodes[].edge` | object: `color`, `glow` | a `roundcube` mesh | none |
| `scene.nodes[].edge.color` | [number x4] | each 0..=1 | the node's `color` |
| `scene.nodes[].edge.glow` | number | 0..=1 | 0 |

A node's `picture` is shown on its mesh by the mesh's uvs, tinted by
its `color`: a picture the program uploaded with `picture`, named by its
id. A picture sent again under the same id shows on the next scene sent,
so a face can play video. A picture never uploaded is refused by name
(`EINVAL`, `no picture N was uploaded`).

A mesh string is one of `quad`, `cube`, `plane`, `gem`, `wedge`,
`gable`, `hip`, `pyramid`. A mesh object has `kind`:

| kind | fields | range | default |
| --- | --- | --- | --- |
| sphere, cone, cylinder, frustum, arch | `segments` | 3..=64 | 16 |
| stairs | `steps` | 1..=64 | 8 |
| rock, blob | `seed` | 0..=65535 | 1 |
| fence | `posts` | 2..=64 | 4 |
| roundcube | `radius`, `segments` | 0..=0.5, 1..=16 | 0.12, 4 |
| inline | `positions`, `indices`, optional `normals`, `uvs`, `colors` | see caps | — |
| gltf | `asset`, optional `scene` | an uploaded asset; one of its scenes | the file's own scene |

Inline `positions` are `[x,y,z]` triples, `indices` a triangle list,
`normals` triples, `uvs` pairs, `colors` `[r,g,b,a]`. Across one
envelope, at most 4096 vertices and 12288 indices. A missing parent or
a parent cycle is an error. The decoded payload is at most 1 MiB.

A `gltf` mesh hangs an asset (`asset` below) from the node: the asset's
node tree, its meshes and its materials go under it, and the node's
own transform places the whole model. The node's `color` multiplies
every material of the model (`[1,1,1,0.5]` makes it half transparent)
and its `emissive` is the least any of them shines; `lit` does not
apply, as a glTF material is always lit. `visible` hides the whole
model. The asset's meshes are not inline and do not count against the
4096 vertices: they go up to the window once, when a scene first shows
the asset. With its assets hung, one scene holds at most 16384 nodes.
An asset not uploaded is an error. Sending the scene again keeps its
clips (`play`) where they were.

```bash
printf '\033_glass;scene;id=1;px=40,40,160,120;s=160;v=120;%s\033\\' \
  "$(printf '%s' '{"v":1,"scene":{"nodes":[{"id":1,"mesh":"cube","color":[0.85,0.45,0.2,1]}]}}' | base64 -w0)"
printf '\033_glass;remove;id=1\033\\'
```

### The rounded cube, its faces and its edges

`{"kind":"roundcube","radius":r,"segments":n}` is the unit cube
(-0.5..0.5 on each axis) with its edges and corners rounded to radius
`r` (0..=0.5, default 0.12; kept to the thousandth) in `n` steps round
each (1..=16, default 4). Its mesh has seven groups:

| group | what | normal |
| --- | --- | --- |
| 0 | the +X face | +X |
| 1 | the -X face | -X |
| 2 | the +Y face (top) | +Y |
| 3 | the -Y face (bottom) | -Y |
| 4 | the +Z face (front, toward the default camera) | +Z |
| 5 | the -Z face (back) | -Z |
| 6 | the twelve rounded edges and eight corners, together | the rounded box's own |

Groups 0 to 5 are in the order of the built-in `cube`'s faces, so face
`i` is the same side on both. Each flat face carries the whole picture
across its flat part, uvs 0..1, upright as the cube's are: seen from
outside, a side face's picture stands up (+Y), the top's top is toward
-Z, the bottom's toward +Z. The rounded edges meet the flat faces
smoothly; at radius 0 it is a cube, at 0.5 a ball.

`faces` dresses a `cube`'s or a `roundcube`'s six faces: an array of up
to six entries, entry `i` for face `i` (the order above), each a picture
uploaded with `picture` (its program id) or `null`. A face with a
picture shows it, read linear, tinted by the node's `color` (and lit as
the node is, never chrome); a `null` face, and every face past the end
of the array, is the node's own material. A picture never uploaded is
refused by name, as a node's `picture` is (`EINVAL`, `no picture N was
uploaded`). A picture sent again under its id shows on the next scene
sent, so a face can play video. `faces` on any other mesh is an error.

`edge` is the material of a `roundcube`'s group 6: `{"color":[r,g,b,a],
"glow":g}`. Without `edge` the edges are the node's own material. With
`glow` 0 they are `color`, lit as the node is. With `glow` above 0 they
shine by themselves at that strength (unlit, past their colour toward
white as `glow` rises) and a soft halo is drawn round them in the same
pass: light added beyond and over the edges, fading toward its outline
and where the edges meet the flat faces, covering nothing. A program
animates the glow by sending the scene again with new colours. `edge`
on any other mesh is an error; unknown fields in it are errors.

A colour `tween` on the node moves its faces with it (a picture face is
tinted by it); the edge keeps its own colour.

```bash
# six pictures, ids 1..6, uploaded first (here 1x1: red, green, blue,
# yellow, magenta, cyan; a PNG with f=100 works the same)
i=1
for rgb in '\xff\x30\x30' '\x30\xff\x30' '\x30\x30\xff' '\xff\xff\x30' '\xff\x30\xff' '\x30\xff\xff'; do
  printf '\033_glass;picture;id=%d;f=24;s=1;v=1;%s\033\\' "$i" "$(printf "$rgb" | base64)"
  i=$((i + 1))
done
# a rounded cube with a picture on each face and glowing cyan edges
printf '\033_glass;scene;id=2;px=40,40,320,320;s=320;v=320;%s\033\\' \
  "$(printf '%s' '{"v":1,"scene":{"camera":{"eye":[1.6,1.2,2.2]},"nodes":[{"id":1,"mesh":{"kind":"roundcube","radius":0.15,"segments":6},"lit":true,"faces":[1,2,3,4,5,6],"edge":{"color":[0.2,1,1,1],"glow":0.9}}]}}' | base64 -w0)"
# the built-in cube takes faces too; null keeps the node's colour
printf '\033_glass;scene;id=3;px=380,40,160,160;s=160;v=160;%s\033\\' \
  "$(printf '%s' '{"v":1,"scene":{"nodes":[{"id":1,"mesh":"cube","rot":[0.2,0.3,0,0.93],"color":[1,0.9,0.8,1],"faces":[1,null,3]}]}}' | base64 -w0)"
```

### Chrome and liquid

`chrome` (0..=1) makes the node's surface a mirror of a studio: a cool
sky darker overhead, a dark floor, a bright horizon band between them,
three soft rectangular softboxes (a broad key overhead and to the left,
a tall cool strip to the right, a warm strip behind on the left), the
light a little warmer on one side of the room than the other. It is
made in the shader, from the way the eye's ray leaves the surface; no
picture, no light of the scene's: an unlit node is chrome as a lit one
is. Head on the reflection is tinted by the node's `color`; toward the
rim it runs to white (Fresnel), with a hard highlight of the key light
and a cool rim. Between 0 and 1 the reflection is mixed over the
surface as it would otherwise be drawn.

`liquid` (0..=1) makes it molten: its normals ripple with travelling
waves over a slow warp, and its vertices wobble a little along their
normals (a cube's, whose faces split their normals at the edges,
wobble out from its middle instead, so its seams hold), all moving with
the window's own clock: glass redraws a scene with anything liquid in
it every display frame while the program says nothing. Ripples and
wobble are sized by the mesh, so they look alike on a cube and a
figure; at 1 it is strongly molten, the wobble about 4% of its size.
With `chrome` it is flowing mercury; alone it ripples the light on a
lit surface.

On a `gltf` node both apply to every mesh of the asset. A node with
`chrome` draws them all in its own material (its colour the chrome's
tint; the file's colours and textures are not shown), each keeping
only its own sides (`doubleSided`); a node with only `liquid` keeps the
file's materials and makes them liquid.

A glass from before these fields answers `EINVAL:unknown node field
chrome`, so a program that sends the scene with `r=` learns which it
has.

```bash
# a liquid-chrome sphere, unlit, on black
printf '\033_glass;scene;id=4;px=40,40,320,320;s=320;v=320;%s\033\\' \
  "$(printf '%s' '{"v":1,"scene":{"camera":{"clear":[0,0,0,1]},"nodes":[{"id":1,"mesh":{"kind":"sphere","segments":48},"scale":[1.6,1.6,1.6],"color":[0.92,0.94,1,1],"chrome":1,"liquid":0.6}]}}' | base64 -w0)"
# a dancing figure in gold chrome (asset 1 uploaded, as below)
printf '\033_glass;scene;id=7;px=40,40,640,480;s=640;v=480;%s\033\\' \
  "$(printf '%s' '{"v":1,"scene":{"camera":{"eye":[0,1.4,3.6],"target":[0,0.9,0]},"nodes":[{"id":1,"mesh":{"kind":"gltf","asset":1},"color":[1,0.8,0.45,1],"chrome":1,"liquid":0.3}]}}' | base64 -w0)"
printf '\033_glass;play;id=7\033\\'
```

## frame

One video frame for an open video showcase (`showcase` with `s` and
`v`). A picture upload or raw RGBA, chunked the way `picture` is
(`m=1` on every piece but the last). `id` names an already uploaded
picture instead of a payload. A frame does not reopen the showcase.

| key | type | range | default | meaning |
| --- | --- | --- | --- | --- |
| `id` | u32 | an uploaded picture id | — | use that picture's pixels |
| `f` | 100\|32\|24 | | 32 | payload format, as `picture` |
| `s` | u32 | 1..=2048 | 0 | raw width |
| `v` | u32 | 1..=2048 | 0 | raw height |
| `o` | z or empty | | empty | `z` is zlib |
| `m` | 0\|1 | | 0 | more pieces follow |
| `r` | token | | — | reply token; on a chunked frame, the first piece's |

```bash
printf '\033_glass;showcase;s=8;v=4\033\\'
printf '\033_glass;frame;f=32;s=2;v=1;%s\033\\' "$(printf '\xff\x00\x00\xff\x00\xff\x00\xff' | base64 -w0)"
printf '\033_glass;frame;id=5\033\\'
```

## progress

Playback position and pause. This does not reopen the showcase.

| key | type | range | default | meaning |
| --- | --- | --- | --- | --- |
| `t` | number | 0..=1 | — | position; omit to leave it unset |
| `pause` | 0\|1 | | 0 if `t` is set | paused |
| `r` | token | | — | reply token |

At least one of `t` and `pause` is required.

```bash
printf '\033_glass;progress;t=0.5;pause=0\033\\'
printf '\033_glass;progress;pause=1\033\\'
```

## chart

Marks over a picture uploaded with `picture`. `p` names that picture,
and its size is the chart's size. The marks lie over it. Without `p=`
it is an error. Sending a chart opens the showcase on that picture and
those marks; the same request does not reopen it.

| key | type | range | default | meaning |
| --- | --- | --- | --- | --- |
| `p` | u32 | under `0x40000000` | required | a picture uploaded with `picture`; its size is the chart's size |
| `n` | u32 | 0..=4096 | required | record count |
| `fit` | contain\|width | | contain | how the panel fits |
| `r` | token | | — | reply token |

A mark record:

| key | type | range | default | meaning |
| --- | --- | --- | --- | --- |
| `x`,`y`,`w`,`h` | number | finite | required | the mark's rectangle, content pixels |
| `ax`,`ay` | number | finite | the rectangle's centre | where the mark is anchored |
| `t` | text | | empty | tip lines; `\|` splits lines |

```bash
printf '\033_glass;picture;id=1;f=32;s=2;v=1;%s\033\\' \
  "$(printf '\xff\x00\x00\xff\x00\xff\x00\xff' | base64 -w0)"
printf '\033_glass;chart;p=1;n=1;%s\033\\' \
  "$(printf '%s' 'mark;x=0;y=0;w=1;h=1;t=glass' | base64 -w0)"
```

## model

Scene nodes for the showcase, through the scene wire form above. The
payload is the same envelope. The camera is validated and ignored; the
nodes are shown. Inline meshes are uploaded before the panel opens. A
`gltf` mesh shows the asset in its rest pose.
Sending a model opens the showcase; the same nodes do not reopen it.

| key | type | range | default | meaning |
| --- | --- | --- | --- | --- |
| `s` | u32 | 1..=2048 | required | panel width |
| `v` | u32 | 1..=2048 | required | panel height |
| `fit` | contain\|width | | contain | how the panel fits |
| `r` | token | | — | reply token |

```bash
printf '\033_glass;model;s=160;v=120;%s\033\\' \
  "$(printf '%s' '{"v":1,"scene":{"nodes":[{"id":1,"mesh":"cube"}]}}' | base64 -w0)"
```

## page

Text the window rasterises as a page and shows in the showcase. The
payload is base64 UTF-8; newlines separate lines. The window pads to
`rows` and shows the raster with `fit=width`, so a tall page scrolls.
A missing font leaves the request stored and the panel unchanged.

| key | type | range | default | meaning |
| --- | --- | --- | --- | --- |
| `cols` | u32 | 1..=256 | required | page columns |
| `rows` | u32 | 1..=256 | required | page rows |
| `r` | token | | — | reply token |

The payload is at most 1 MiB.

```bash
printf '\033_glass;page;cols=24;rows=4;%s\033\\' \
  "$(printf '%s' 'a page of text' | base64 -w0)"
```

## asset

Upload a glTF 2.0 model under a program id: a `.glb`, or a `.gltf`
whose buffers ride inside it as base64 `data:` URIs. A scene shows it
through a `gltf` mesh (`scene` above), and `play` plays its clips.
Reusing an id replaces the asset; a scene already showing the old one
keeps it until that scene is sent again or removed.

| key | type | range | default | meaning |
| --- | --- | --- | --- | --- |
| `id` | u32 | 0..2^30 | — | the asset's program id |
| `t` | d\|f | | d | `d`: the payload is the file; `f`: the payload is a path glass reads |
| `m` | 0\|1 | | 0 | `1` on every piece but the last of a chunked transmission |
| `r` | token | | — | reply token; on a chunked asset, the first piece's |

Chunking is `picture`'s: `m=1` on every piece but the last, the first
piece's keys govern, a continuation is `asset;m=1;<piece>`, and only
the last piece answers, with the first piece's token. The pieces count
against the store's budget while they gather.

With `t=f` the payload is the base64 of a path, and glass reads the
file under the APC file medium's own rules: the path is made canonical
first; nothing under `/proc` or `/sys`, nothing under `/dev` but
`/dev/shm`, and nothing but a regular file is opened; it is opened
with `O_NOFOLLOW` and checked again through the open handle; at most
64 MiB is read.

What glass reads from the file:

- **The container.** A GLB's header and chunk lengths are checked
  against the bytes there; a JSON chunk first, a BIN chunk after it,
  other chunks passed over. A `.gltf`'s buffers must be base64 `data:`
  URIs; glass reads no other file.
- **Accessors.** Every component type, normalized or not (so
  `KHR_mesh_quantization` is taken), interleaved views with a stride,
  sparse accessors, and accessors with no view (zeros). Every index,
  offset, stride and count is checked against what is there before
  anything is read.
- **Meshes.** `POSITION`; `NORMAL`, or flat normals made when a
  primitive has none (as glTF says); the `TEXCOORD_n` set its base
  colour texture names; `COLOR_0` (three or four components); indices
  of 8, 16 or 32 bits, or none. `TRIANGLES`, `TRIANGLE_STRIP` and
  `TRIANGLE_FAN` (strips and fans become lists in glTF's own order);
  points and lines are skipped and counted.
- **The node tree.** Translation, rotation and scale, or a `matrix`
  (taken apart into the three); children; the file's scenes and its
  default scene (a file without scenes is one scene of every node
  without a parent). A node with two parents, a cycle, or a tree
  deeper than 128 is an error.
- **Materials.** `baseColorFactor` and `baseColorTexture` (decoded
  through the same loader `picture` uses and kept as a picture in the
  store; a `NEAREST` magnification filter keeps texels crisp, anything
  else is linear), `alphaMode` (`OPAQUE`; `BLEND`; `MASK` is drawn as
  `BLEND`), `doubleSided`, and `emissiveFactor`: its strongest channel
  is how much the surface shines by itself, and the emissive colour is
  mixed into the surface's colour by as much. Every glTF material is
  lit. The lit path has one light and no reflections, so
  `metallicFactor` and `roughnessFactor` shade the colour (a smooth
  metal is darkened by up to half); the factors, the alpha cutoff and
  every other texture (metallic-roughness, normal, occlusion, emissive)
  are kept with the asset for a physically based path later.
- **Animations.** Translation, rotation and scale channels with
  `LINEAR` (rotations by spherical interpolation), `STEP` and
  `CUBICSPLINE` samplers. Times must not go back. Morph-target weight
  channels are skipped and counted.
- **Skins.** Each skin's joints (nodes of the file, at most 256) and
  its inverse bind matrices (4x4 floats, the identity for each when the
  file gives none; at least as many as the joints), and each skinned
  primitive's `JOINTS_0` (unsigned bytes or shorts) and `WEIGHTS_0`
  (floats, or normalized unsigned bytes or shorts), both or neither.
  A node with a skin and a mesh is bent by its joints in the mesh
  shader every frame: each vertex goes through its four joints' world
  matrices times their inverse bind matrices, blended by its weights
  (scaled to sum to 1), and the skinned node's own transform is
  ignored, as glTF says. The joints are the asset's nodes as the scene
  hangs them, so a clip that moves them (`play`) bends the mesh, and
  with no clip playing the mesh rests in the pose its nodes give it
  (the bind pose, for a file whose nodes stand where it was bound).
  A joint index past its skin's joints is an error; the `skeleton`
  field is not needed and not read; `JOINTS_1` and `WEIGHTS_1` (more
  than four joints a vertex) are not read.
- **Not read.** Morph targets, cameras, lights and extensions. A file
  that requires an extension other than `KHR_mesh_quantization` is
  refused; one it only uses (`extensionsUsed`, such as
  `KHR_materials_specular`) is read without it. An image named by an
  external URI is skipped and counted, and its material keeps its
  colour. Pictures decode through the same loader `picture` uses, PNG
  and JPEG among them.

The asset's meshes go up to the window (its mesh budget, 128 MiB)
when a scene first shows the asset, never once a frame.

A skinned vertex costs 24 bytes more of the mesh budget (its joints
and weights).

Replies: `OK;nodes=N,meshes=M,primitives=P,vertices=V,triangles=T,materials=K,textures=X,clips=C`,
then `,seconds=a/b/...` (each clip's length) when it has clips, then
`,skipped=p/i/c` (primitives, images, channels) when anything was
skipped. `EINVAL` for bad keys and for anything the file names that is
not there (an index, an accessor, a view, a buffer, a node, a mesh, a
material, a texture, a sampler, an image, a skin, a joint), a matrix
accessor where a vector belongs (or anything but 4x4 floats for
inverse bind matrices), joints without weights or weights without
joints, a negative weight, a skin with no joints, values
that do not match their times, times that go back, a node tree that
cycles, has a node with two parents or is too deep, and a required
extension. `EBADF` when the payload is not base64, the file is not a
glb or glTF JSON, the JSON does not parse, a length lies (a header or
chunk past the file, a buffer shorter than it says, a view past its
buffer, an accessor past its view), a texture does not decode, or the
path is refused. `EFBIG` past a cap (Limits). `ENOMEM` when 64 assets
are kept, or their meshes would pass 128 MiB together.

```bash
# a .glb in one go
printf '\033_glass;asset;id=1;r=1;%s\033\\' "$(base64 -w0 model.glb)"

# a big one in pieces of 64 KiB of base64
b64=$(base64 -w0 model.glb); i=0
while [ $i -lt ${#b64} ]; do
  piece=${b64:$i:65536}; i=$((i + 65536))
  m=$([ $i -lt ${#b64} ] && echo 1 || echo 0)
  if [ $i -eq 65536 ]; then printf '\033_glass;asset;id=2;m=%s;r=2;%s\033\\' "$m" "$piece"
  else printf '\033_glass;asset;m=%s;%s\033\\' "$m" "$piece"; fi
done

# glass reads the file itself
printf '\033_glass;asset;id=3;t=f;r=3;%s\033\\' "$(printf '%s' "$PWD/model.glb" | base64 -w0)"

# shown in a scene, hung from node 1, then played
printf '\033_glass;scene;id=7;px=40,40,640,480;s=640;v=480;%s\033\\' \
  "$(printf '%s' '{"v":1,"scene":{"camera":{"eye":[0,1.5,5],"target":[0,1,0]},"nodes":[{"id":1,"mesh":{"kind":"gltf","asset":1}}]}}' | base64 -w0)"
printf '\033_glass;play;id=7;clip=0\033\\'
```

## play

Play an asset's clips on a scene's glTF nodes: every asset the scene
shows, or the one hanging from `node=`. glass samples the clip every
display frame itself; the program says nothing more.

| key | type | range | default | meaning |
| --- | --- | --- | --- | --- |
| `id` | u32 | a scene's id | required | the scene |
| `node` | u64 | a node with a `gltf` mesh | every one | which asset of the scene |
| `clip` | index or `all` | the asset's clips | 0 | the clip; `all` plays every clip at once |
| `speed` | number | −16..=16 | 1 | how fast; negative plays backwards |
| `loop` | 0\|1 | | 1 | `0` plays once and holds the last pose |
| `t` | number | 0..=3600 seconds | — | seek |
| `pause` | 0\|1 | | 0 | pause where it is, or go on |
| `stop` | 0\|1 | | — | `1` stops and puts the model back in the file's own pose |
| `r` | token | | — | reply token |

The first `play` on an asset starts its clip at 0, or at its end when
it plays backwards. A later one changes only what it names, and the
clip carries on from where it was: a new speed, a pause or a resume
never jumps. A new `clip` starts from 0 from the rest pose. A clip
that runs out (`loop=0`) holds its last pose. A paused clip, or one
that ran out, wakes nothing.

Replies: `OK;seconds=<the clip's length>`; `EINVAL` when the scene, the
node, or the clip is not there, or the asset has no animations.

```bash
printf '\033_glass;play;id=7\033\\'                  # clip 0, looping
printf '\033_glass;play;id=7;clip=1;speed=0.5\033\\' # another clip, half speed
printf '\033_glass;play;id=7;pause=1\033\\'
printf '\033_glass;play;id=7;t=1.25;pause=0\033\\'   # seek and go on
printf '\033_glass;play;id=7;loop=0\033\\'           # finish this time round
printf '\033_glass;play;id=7;stop=1\033\\'
```

## tween

Move a property from where it is now to where the program says, eased,
over a time: glass works out the value every display frame itself, so
the motion is smooth while the program says nothing.

A tween names one target:

| target | keys |
| --- | --- |
| a layer, or a scene's placement | `layer=<id>` (a `layer`'s id; a scene's when no layer has it) |
| a scene node | `id=<scene>;node=<node id>` (a node the program named) |
| a scene's camera | `id=<scene>;camera=1` |

and moves one or more of its properties, each to the value given:

| target | key | value | how it moves |
| --- | --- | --- | --- |
| layer | `at` | x,y,w,h cells | each number; a text-anchored layer's y counts from the line its `at=` was placed on, and it still rides the text |
| layer | `px` | x,y,w,h pixels | each number |
| layer | `o` | 0..1 | |
| layer | `a` | radians | |
| layer | `c` | rrggbb or rrggbbaa | each channel; solid, glow, prim and particles |
| layer | `size` | 0.5..=64 px | particles' mote size |
| layer | `b`, `h` | 0..=8192 | breathe, lift |
| node | `at`, `scale` | x,y,z | each number |
| node | `rot` | x,y,z,w quaternion | along the shorter arc (slerp) |
| node | `color` | r,g,b,a, each 0..1 | each channel; a glTF node's colour multiplies its model |
| node | `emissive` | 0..1 | |
| node | `visible` | 0\|1 | at the end |
| camera | `eye`, `target` | x,y,z | each number; the camera keeps looking at `target` |
| camera | `fov` | (0, pi) radians | perspective only |

and its timing:

| key | type | range | default | meaning |
| --- | --- | --- | --- | --- |
| `dur` | number | 0..=3600 seconds | 0.3 | one run; a spring sets its own |
| `delay` | number | 0..=3600 seconds | 0 | before the run begins |
| `ease` | name | below | linear | how time is shaped |
| `repeat` | count or `forever` | 0..=1000000 | 0 | runs after the first |
| `yoyo` | 0\|1 | | 0 | every second run goes back |
| `tid` | u32 | | — | names the tween; a new tween with the same `tid` replaces it |
| `after` | u32 | a `tid` | — | begin when that tween ends |
| `stop` | 0\|1 | | — | `1` stops every tween on the target, or the one `tid` names |
| `r` | token | | — | reply token |

Easings: `linear`; `quad`, `cubic`, `quart`, `quint`, `sine`, `expo`,
`circ`, `back`, `elastic` and `bounce`, each `-in`, `-out` or `-in-out`
(`quad-in-out`), the usual curves; `cubic-bezier(x1,y1,x2,y2)` as CSS
has it (x1 and x2 in 0..1); and `spring(stiffness,damping[,mass])`, a
damped spring from rest (stiffness up to 10000, damping up to 1000,
mass up to 1000, default 1), which runs until it has settled within
0.1% of its target (at most 60 seconds) and ignores `dur`. Back,
elastic, a bezier with a y outside 0..1 and a spring overshoot; an
opacity or a colour that overshoots is held to 0..1.

How tweens behave:

- **From where it is.** A tween starts from the property's value at the
  moment its run begins, so nothing jumps when one tween takes over
  from another, after a delay, or at the end of a chain.
- **Cancel and replace.** A new tween on a property replaces whatever
  was moving that property of that target, at once; its other
  properties carry on. A tween with `after=` takes over only when its
  run begins. A `layer` or `scene` op that sets a target outright stops
  its tweens.
- **Chains.** `after=<tid>` sequences tweens by name: the tween begins
  when the named one ends, at the very moment it ended (plus its own
  delay), however late the frame that notices. A tween after one that
  is not there (never was, already ended, stopped) begins at once.
  glass chose named waits over a timeline op: a chain is any number of
  tweens, each its own op, and a branch is two tweens after one.
- **Timing.** A tween's clock starts at the first frame after it
  arrives, then runs on the window's own clock. `repeat=forever` never
  ends, so nothing after it begins.
- **Wake-ups.** The window wakes at once for a new tween, at the end of
  a delay, one display frame at a time while a tween runs (or at its
  end, when that comes sooner), and not at all when nothing moves. A
  tween that only changes `visible` wakes the window once, at its end.
- **Stop.** `stop=1` leaves what the tweens moved where it is.

Replies: `OK`; `EINVAL` when the target is not there, a key is not a
property of it (`rot` on a layer, `at` on a layer placed with `px=`),
a value is out of range, the easing is not one of the above, or a
tween waits on itself; `ENOMEM` when 4096 tweens are running.

```bash
# slide a layer across and fade it, eased
printf '\033_glass;tween;layer=1;at=40,2,8,2;o=0.4;dur=0.8;ease=cubic-out\033\\'

# breathe for ever
printf '\033_glass;tween;layer=2;c=ff8800;dur=1.5;ease=sine-in-out;repeat=forever;yoyo=1\033\\'

# turn a node a quarter round y with a spring, then hide it
printf '\033_glass;tween;id=7;node=1;rot=0,0.7071,0,0.7071;ease=spring(170,12)\033\\'
printf '\033_glass;tween;id=7;node=1;visible=0;dur=2\033\\'

# a camera fly-through: three moves, one after another
printf '\033_glass;tween;id=7;camera=1;eye=4,2,3;target=0,1,0;dur=2;ease=sine-in-out;tid=1\033\\'
printf '\033_glass;tween;id=7;camera=1;eye=-3,1,3;fov=0.5;dur=2;ease=cubic-in-out;tid=2;after=1\033\\'
printf '\033_glass;tween;id=7;camera=1;eye=0,1.5,5;fov=0.8;dur=2;ease=back-out;after=2\033\\'

# a CSS curve, and stopping
printf '\033_glass;tween;layer=1;px=0,0,320,40;dur=1;ease=cubic-bezier(0.68,-0.6,0.32,1.6);tid=9\033\\'
printf '\033_glass;tween;tid=9;stop=1\033\\'
printf '\033_glass;tween;layer=2;stop=1\033\\'
```

## Showcase events

What the operator does in the showcase reaches the program only when it
opted in with `showcase;events=1`. As the counters change, glass writes,
in this order, through the session's writer:

```
ESC _ glass ; event ; showcase ; <key>=<value> ESC \
```

| key | when |
| --- | --- |
| `seek` | the operator seeked; the value is the position |
| `steps` | the step counter changed |
| `copies` | the copy counter changed |
| `dismissals` | the dismissal counter changed |
| `clicks` | the click counter changed |
| `close` | the operator closed the showcase; the value is `1` |

The program sees none of these unless it opted in. A producer's own
close does not count as `close`.

## Replies

Every op answers only when the program gave `r=<token>`:

```
ESC _ glass ; r=<token> ; OK [; extra] ESC \        success
ESC _ glass ; r=<token> ; <CODE>:<message> ESC \    failure
```

Codes: `EINVAL` (unknown op, unknown key, malformed or out-of-range
value, a reference to something not there), `EBADF` (payload not
base64, not layer records, not a glb or glTF, or a length in it that
lies; a path the file medium refuses), `EFBIG` (payload or a count in
it past its cap), `ENOMEM` (the store's picture budget is full, the
asset count or mesh budget is full, or 4096 tweens are running).
Unknown ops, unknown keys and malformed
values are errors and never panic. Every count, size and allocation is
capped before anything is allocated.

## Lifecycle

- A layer anchored to text leaves with its text: on ED 3 (history
  cleared), when it is pruned out of history, and when its screen is
  cleared (ED 2). On the alternate screen it goes when that screen is
  left.
- Window-pinned layers and scenes stay until removed, a RIS, or the program exits.
- A text-anchored scene leaves with its text, the same way a layer does.
- RIS removes everything this dialect made: layers, scenes, assets,
  tweens and clips.
- When the child process exits, everything this dialect made is
  removed.
- A tween stops when its target goes (a layer or scene removed, or a
  text-anchored one leaving with its text) and when the program sets
  the target outright again (`layer` or `scene` with the same id).
- An asset's texture pictures leave the store with `remove asset=` or
  `remove all=1` once no scene shows it; after RIS or an exit they age
  out of the store's budget like any picture.

These ride the store's existing clear and reset operations; no new
engine events were added.

## Limits

| limit | value |
| --- | --- |
| program ids | 0..2^30−1 |
| this dialect's window layer ids | 2^30..2^31−1 |
| one `group` | at most 65536 records |
| one `group` payload | at most 1 MiB decoded |
| picture sides | at most 4096 px |
| picture budget | the store's own, shared with the intake's pictures |
| `h=`, `b=`, `rise=`, `sway=` | at most 8192 |
| `n=` (particles) | at most 4096 motes |
| scene nodes | at most 256 |
| scene vertices | at most 4096 |
| scene indices | at most 12288 |
| scene, chart, model, page payload | at most 1 MiB decoded |
| scene and showcase pixel sides | 1..=2048 |
| chart marks | at most 4096 |
| page columns and rows | 1..=256 |
| inline mesh ids | from `0xC0000000` |
| one asset file | at most 64 MiB |
| assets kept | at most 64, their meshes at most 128 MiB together (48 bytes a vertex, 4 an index) |
| an asset's JSON | at most 262144 objects and arrays |
| an asset's nodes, tree depth | 4096 nodes, 128 deep |
| an asset's meshes, primitives | 1024 meshes, 4096 primitives |
| an asset's accessors, buffer views, textures, samplers, images | 16384 each |
| an asset's buffers, materials, clips | 64 buffers, 1024 materials, 64 clips |
| an asset's decoded textures | 32, each at most 4096 px a side, 64 MiB of RGBA together |
| an asset's vertices, indices | 1048576 vertices, 3145728 indices; a primitive 262144 and 1572864 |
| an asset's channels, keys | 4096 channels, 1048576 keyframe values |
| an asset's skins, a skin's joints | 64 skins, 256 joints |
| a `roundcube` | radius 0..=0.5, 1..=16 steps; at most 64 kinds kept on the GPU at once |
| a node's `faces` | at most 6 |
| one scene with its assets hung | 16384 nodes |
| tweens running at once | 4096 |
| `dur=`, `delay=`, `t=` | 0..=3600 seconds |
| `repeat=` | up to 1000000, or `forever` |
| `speed=` | −16..=16 |
