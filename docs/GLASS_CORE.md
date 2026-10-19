# Glass core: WebGPU everywhere, 3D underneath

The operator's order (2026-10-16):
1. Everything WebGPU.
2. 3D as the foundation.
3. Video as a property of a material inside that scene.

The point of glass is to break every media wall and control it to the core. A picture, a video, a model, another seat's layer and the cockpit's own text are all things a scene holds, and any of them can sit on any surface.

This continues [RENDER_FOUNDATION.md](RENDER_FOUNDATION.md), whose first principle was already "one renderer, two hosts". It replaces that doc's M2 ("media") with the order below. Research behind it: two read-only surveys of the tree on 2026-10-16, one of the renderers and wgpu on wasm, one of the scene types, producers, CPU 3D and media paths.

## Where it stands (dev + cleanup/integrate, 2026-10-16)

- **The window is WebGPU already.**
  - glass draws through wgpu 30.0.1 with WGSL, on Vulkan and Metal (`crates/glass/Cargo.toml:18`).
  - It has one pipeline and one shader (`render.rs:520-598`, `glass.wgsl`): instanced quads that are pictures, solid colours or round shadows.
  - There is no depth buffer: every pipeline has `depth_stencil: None`.
- **The browser is not.**
  - The page draws in JavaScript with WebGL2, then WebGL1 (no stage), then a 2D canvas (`crates/web/shell/present.js:10-14`).
  - `stage.js` is a second copy of the tilt, stand and slide maths, in GLSL ES 3.00.
  - The wasm (`crates/web/src/lib.rs`, no wasm-bindgen) only produces frames and STG2 bytes.
- **The scene is 2D with a tilt.**
  - `angel-stage` has flat layers (`Solid`, `Picture`, `PicturePart`, `Glow`, `Particles`) placed in pixels or cells.
  - A layer can turn, lift and breathe, and stands on a tilted floor through `Stand` / `Tilt`.
  - Nothing has a transform, a camera, a mesh or depth.
- **3D runs on the CPU.**
  - `crates/delve/src/render/world3d` (mesh, raster) and `cockpit/src/stage/world_viz/world3d` (ride, region, interiors) rasterize triangles in software.
  - The triangles carry positions, colours and normals, with no UVs and no indices.
  - The result is uploaded as a picture.
- **Text is per cell.** The grid is one quad per cell sampling a glyph atlas that swash fills on the CPU. It is not a texture of the whole grid.
- **Video reaches the screen only as pictures.**
  - Scryglass decodes through ffmpeg-next. That is the default feature, and it is off in the prebuilts.
  - glass gets each frame as a new picture upload.
  - The browser has no video path.

## Decisions

1. **One renderer crate, `angel-gpu`.**
   - It takes a wgpu `Device`, `Queue` and target texture, and a scene; it draws.
   - It knows nothing of windows, fonts on disk, the clipboard or threads.
   - glass becomes the window, input, font discovery and the clipboard around it.
   - The browser seat links the same crate into its wasm.
   - One WGSL source serves both; `stage.js` and `present.js` retire.
2. **The browser runs WebGPU, with wgpu's own WebGL2 backend as the only fallback, in two builds the page picks between.**
   - wgpu 30 has both for `wasm32-unknown-unknown` (`webgpu`, `webgl`).
   - Measured 2026-10-16 with a stand-in renderer (`/work/tmp/gcl-wasm-probe`): one pipeline from `glass.wgsl`, one texture, one instanced quad, after wasm-bindgen and `wasm-opt -Oz`.
     - **WebGPU only: 32 KB brotli** (97 KB raw). The browser compiles the WGSL, so naga is not linked.
     - **WebGPU and WebGL2: 724 KB brotli** (2.9 MB raw). It pulls in wgpu-core and naga.
     - For scale, the Delve's game wasm is 2.08 MB brotli (6.6 MB raw).
   - So there are two renderer builds, and the page loads the first if `navigator.gpu` is there and the second otherwise. Only a browser without WebGPU pays for the fallback. The operator approved this on 2026-10-16.
   - WebGL1 and the 2D canvas go. Every current browser has WebGL2; Safari was last, with version 15 in 2021.
3. **A 3D scene is the root of every frame.**
   - It is nodes in a tree, each with a transform and an optional mesh and material, seen by one or more cameras into targets, with a depth buffer always present.
   - Today's layers become a compiled special case: a quad node under a screen-space camera. `Stand` and `Tilt` become a perspective camera looking at a floor, with upright billboards.
   - Producers keep calling `set_layer` and `set_stage` until each is moved.
4. **Every medium is a texture source a material samples.** `TextureSource`:
   - `Picture(id)`, an upload, as today;
   - `Video(stream)`, decoded frames;
   - `Grid`, the cockpit's text, drawn into a target;
   - `Target(scene, camera)`, another scene's render;
   - `Pixels`, a buffer a producer writes.

   A material picks a source and how to sample it: straight, premultiplied, stacked alpha (alpha video), nearest or linear, with or without lighting. The cube with three videos is three face materials on one cube mesh; the cockpit on a turning panel is `Grid` on a quad.
5. **Video decodes without linking codecs.**
   - Natively, a decoder is an `ffmpeg` child process piping raw RGBA at the size the material needs. This works in the prebuilts, needs no new dependency, and the agent tools already run ffmpeg this way.
   - ffmpeg-next stays where it is built.
   - In the browser, an HTMLVideoElement is copied into a texture (`copyExternalImageToTexture`, or `importExternalTexture` on WebGPU).
   - Hardware decode straight to the GPU comes later.
6. **The wire carries scenes, not pixels, between processes.**
   - In the browser the renderer is its own small wasm module, one of the two builds above, beside the game's.
     - The page hands it the STG2 frames the game already makes, so the game's wasm and its build stay as they are (no wasm-bindgen there).
     - Joining the two modules, so a frame needs no encode, waits until the copy shows up in frame time.
   - STG2 stays the boundary until a scene encoding (STG3) replaces it with 3D (step 2).
   - Seats still exchange state and asset references, never pictures ([ENGINE.md](ENGINE.md)).
7. **The terminal stays as it is.**
   - It keeps its CPU paths (the braille rasterizer, half blocks) and loses its pixel pictures in cleanup item J.
   - Nothing in this doc changes what a terminal shows.

## Order

Each step ends with screenshots looked at, and with frame time (`ANGEL_GLASS_STATS=1`) and key-to-frame (the glass test kit's `keyframe`) against the last numbers: glass p50 4.0 ms.

### 1. Everything WebGPU

1. **Split `angel-gpu` out of glass. Done 2026-10-16 (593e87ec).**
   - `crates/gpu` takes the WGSL, pipeline, `Instance`/`Draw`/`Frame`, atlas, pictures, stages and slides, the layer compiler, the wallpaper's GPU side, drawing into any view, and readback. That is about 2,040 lines; glass's `render.rs` went from 2,551 to 1,065.
   - The core makes no device and names no backend. It takes the time from its host as seconds, and it checks for wasm32 with `--features web`.
   - Proof:
     - the practice cockpit drawn by the old and new binaries differs in 1,303 of 1,386,880 pixels, all of them the live CPU readout and the animated dot wave's phase;
     - a dev-host fight draws through the moved stage code;
     - glass 52 + gpu 14 tests, the same 66 as before; clippy clean; the browser fuzz clean.
2. **Measure the wasm cost. Done 2026-10-16:** see decision 2. `crates/Cargo.lock` gains the web dependencies, re-pinned for 0.2.5.
3. **The browser draws with `angel-gpu`. Built and proved in headless Chromium 2026-10-16 (`glass/webgpu-3d`); `present.js` and `stage.js` stay until real browsers have run it.**
   - `crates/web/render` (`angel-web-render`) is a wasm-bindgen cdylib in two builds (features `webgpu`, `webgl`), made by `crates/web/build.sh` into the bundle beside the game's wasm when the wasm-bindgen CLI (the version the crate pins) is at hand, and skipped with one line when it is not: no build needs the CLI. The cockpit's `build.rs` makes the same two builds into the full game's page when it is at hand (`$WASM_BINDGEN`), and warns and ships none when it is not.
   - Its JS API is `Renderer.create(canvas, backend)`, `present_stage(bytes, now_ms)`, `present_frame(w, h, rgba)`, `resize`, `set_scale`, `clear`, `kind`, `adapter`, `lost`. `present_stage` decodes the game's STG2 frame (`angel_stage::wire::decode`, held to what `stage.js` refuses), turns its quads back into layers (`angel_stage::layers_of`) and draws them with `angel-gpu`'s `set_stage`, `render_stages` and `draw`; none of the tilt, stand, slide or blend maths is written twice.
   - `shell/boot.js` loads the WebGPU build where `navigator.gpu` is there and the WebGL2 build where only WebGL2 is, and keeps `present.js` where neither is in the bundle or either fails to start. `?present=gpu|webgl|legacy` forces a way. A lost device sends the page back to `present.js` for the rest of the session, said once in the console, with the pictures sent again.
   - Sizes (the renderer's wasm, raw / gzip -9 / brotli -q 11): WebGPU build 188 KB / 70 KB / 58 KB, WebGL2 build 3.2 MB / 1.03 MB / 741 KB; plus 11 KB and 17 KB of brotli for the generated glue. The Delve's wasm is 2.08 MB of brotli.
   - Proof (RTX 3090, Vulkan, headless):
     - the same 40 recorded stage frames drawn through `stage.js` and through each build, at a whole-number scale: 88% of pixels identical and nearly all the rest one level of 255 off (the glow's falloff: the window's atlas mask against the old shader's formula), with ten pixels of drifting motes more than eight levels off; a finished frame is identical to the pixel;
     - at a fitted scale (not a whole number) edges differ by up to a pixel, because the core rounds a stage's texture where the old shader floored its box;
     - the real page and game in each way of drawing, a lost device, no adapter, no `navigator.gpu` and a bundle with no renderer all end in a page that draws (`crates/web/probe/present-compare.mjs`, `present-fallback.mjs`).
   - Still to do before the deletes: Firefox, Safari (WebGPU and the WebGL2 fallback) and a phone; GPU time, which the probe's present number leaves out; and the copy of a frame into the renderer's memory, which joining the two modules would drop.
   - Then `present.js`'s WebGL1 and 2D paths and `stage.js` are deleted.
   - **Exit:** the Delve in Chrome and Firefox (WebGPU) and Safari (WebGPU or the WebGL2 fallback), and on a phone. Frame time is no worse than `stage.js`.

### 2. 3D as the foundation

1. **Scene types and the 3D pipeline. Built and proved 2026-10-16 (`glass/webgpu-3d`).**
   - `angel-stage::scene` is the description, plain data and its small maths (no new crate), shared by every host:
     - `Transform` (translation, unit quaternion, scale; `matrix()` column-major as WGSL wants it, `then`), `Node` (id, parent, transform, mesh, material, `face_materials`, visible), `Camera` (perspective or orthographic, a clear colour; the aspect comes from the target at draw time) and `Scene`.
     - A node whose parent is missing or whose chain loops is not drawn, and nothing panics; world matrices are the exact product of the chain, and a hidden node hides what hangs from it.
     - `MeshRef` is quad, cube, plane, sphere (segments clamped to 3..128) or `Uploaded(id)`. `MeshData` (positions, normals, UVs, colours, indices, and `groups`, index ranges) is checked before it nears a GPU. The cube has 24 vertices, four to a face, in the order +X, -X, +Y, -Y, +Z, -Z, so `face_materials[i]` is face `i`; the picture on each face is upright and unmirrored seen from outside.
     - `Material` is a `TextureSource`, a straight tint, nearest or linear, opaque / straight / premultiplied alpha, and double sided. The sources are `None` and `Picture(id)`; `Video`, `Grid`, `Target` and `Pixels` are named in a comment and come in their own steps.
   - `angel-gpu` gets `mesh.wgsl` (position, normal, UV, colour; one uniform per draw at a dynamic offset), a `Depth32Float` buffer per target size (four kept since the stage's props, the least recently used goes on a resize), and four pipelines made on the first scene (opaque or blended, one or two sided).
     - The shading is unlit, `texture x vertex colour x tint`, premultiplied out into the quads' own blend (a lit material came with the 3D shots, below).
     - Opaque draws go first, in node order, writing depth; blended draws follow, farthest node centre first, testing depth without writing it.
     - Built-in meshes are made once; `upload_mesh` takes a host's own under a budget (128 MiB, 4,096 meshes, 262,144 vertices and 1,572,864 indices each) and refuses what is malformed or oversize.
     - `draw_scene` draws into any colour view; `set_scene_target` and `render_scenes` draw a scene into a picture every frame, which any layer places, lifts and turns as it does a picture. That is how 3D reaches the window (`Host::set_scene`) and, once the browser has a way to send one, the browser.
     - A scene target is premultiplied, so the layer compiler reads it with a new `PICTURE_PREMULTIPLIED` kind: read as a straight picture, its soft edges would come out dark.
   - The target this step makes possible, a cube with a different picture on each face, is `ANGEL_GLASS_DEMO=cube`: a turning, bobbing cube with six numbered test cards, drawn into a picture twice its shown size and lifted over the cockpit's text.
   - Proof (RTX 3090, Vulkan):
     - `crates/glass/tests/scene_gpu.rs` renders scenes into a texture and reads them back (and skips cleanly with no adapter): a cube in front of a plane keeps its own colour with the plane listed after it (255, 0, 0) and the plane shows round it (0, 255, 0); a cube turned a half turn shows the opposite face (blue to yellow), and each of the six faces can be brought to the front and shows its own picture; a face's picture is upright and unmirrored on +Z and on -Z; a straight-blended quad over an opaque one gives (128, 0, 127), and the same cover as a premultiplied picture (127, 0, 128); two blended quads give (0, 64, 127) in either listing order; a scene shown by a layer over red composites to (128, 0, 127), as drawing it straight would. Turning the depth test or the sort off fails the tests that should notice.
     - 35 scene tests in `angel-stage` (maths against hand-computed values, parent chains and loops, generators, refusals), 5 in `angel-gpu`, 4 in glass, and the 14 GPU tests above; the lockfile does not change; clippy clean; `angel-gpu` checks for wasm32 with `--features web` and `angel-web-render` with `webgpu`; the cockpit builds with `--features glass`.
     - The demo drawn by the cockpit in the nested seat: two shots a second apart, no GPU errors, and a debug build's frame time p50 about 3 ms (`ANGEL_GLASS_STATS=1`).
   - Not here: no multisampling and no mip levels (the demo draws twice as large and lets the layer shrink it, so its edges are smooth, but a texture far below its size will shimmer); the browser has no way to send a whole scene yet (a stage's props travel as STG3, below; layers compiling into scenes is 2.2).

   **3D shots: a stage holds a scene, and the Delve's shots are in it. Built and proved 2026-10-16 (`glass/webgpu-3d`).** The operator asked for the projectiles to be 3D instead of 2D sprites. This is the first thing of the Delve's to be nodes, and it is built on the room's floor as it is, so the rest can follow.
   - **Props.** A stage (`angel_gpu::Stage`, `angel_stage::Frame`) gains `props`: `angel_stage::scene::Props`, the scene's nodes and where they lie (`at`, how many layers are under them) and how fine they are drawn (`grain`).
     - They are drawn with depth among themselves into a picture of their own, `grain` times the stage's size, premultiplied. A layer over the whole stage then shows it between `layers[..at]` and the rest, nearest. So the shots lie over the shots' light and the figures, and under the room's own light, the motes and the screen's marks.
     - Not into the stage's own texture, as first planned: a stage is drawn at the density of the window (1440 x 840 for a 384 x 224 room at a 3.75 scale), so a mesh drawn there would have the window's grain, smooth next to the room's. In a picture of its own, a grain of one is the room's own pixel (`props_have_the_stages_pixel_grain_at_a_grain_of_one`); the Delve draws shots at 2, half a pixel of the art.
   - **The camera comes from the floor.** `Tilt::camera` (`stage_camera(tilt, size)`; the flat floor is `Tilt::flat`) is a `Camera` with `Projection::Custom(matrix)`. The world is the stage's own, in its pixels: `x` across, `z` down the floor toward the viewer (the floor point (x, y) is the world's (x, 0, y)), `y` the height off it.
     - A prop at floor point `p` and height `h` lands where an upright layer standing at `p` and lifted `h` does: at `floor(p)` plus `h * scale_at(p)` straight up. A projection that does that for every point is this matrix and no other, so nothing was chosen.
     - Proved two ways. 702 floor and lifted points on each of five tilts and three stage sizes, through `Tilt` and through the camera, agree to 0.0002 px at worst (`angel-stage`). On the GPU, a card prop and an upright layer of the same size at the same place cover the very same pixels, tilted and flat (`a_prop_lands_on_the_pixels_an_upright_layer_lifted_to_it_does`).
     - It is a pinhole level with the horizon, its picture shifted, which is what keeps what stands up upright. It also makes the projection oblique (height and depth both run up the screen at nearly full length), so a sphere shows 1.36 times as tall as wide. The shots' round things are flattened in depth by 0.45 (`shot_meshes::DEPTH`) to show round; long things keep their length whichever way they fly.
     - Depth is the floor's: a nearer point is nearer, a pixel apart is far more than the buffer can tell, and what is level is level.
   - **Light.** `Material` gains `lit` and `emissive`.
     - A lit surface is a plain Lambert from one fixed light (`KEY_LIGHT`, from above, the left and the front, over `AMBIENT_LIGHT` 0.42) in `mesh.wgsl`, the same for every scene. Normals go through the model's cofactor matrix, so a stretched shape keeps them square to its faces (`a_stretched_shape_keeps_its_normals_square_to_its_faces`); a double-sided card is lit from the side it shows.
     - `emissive` is how much of a lit surface shines by itself, 0 to 1: at 1 it is its own colour whatever the light does. It is a field and not a tint above 1 because tints are clamped to 0..1, and a tint cannot make a surface brighter than its colour. Unlit stays the default.
     - Two built-in meshes were added for it, `Gem` (an octahedron, flat faces) and `Cone { segments }`, which an arrowhead and a shard need to be pointed. The draw's uniform grew by a vector (176 bytes), and the depth buffers kept went from three to four (a window, the props and the scene targets).
   - **The shots** are built once, in the Delve (`arena/shot_meshes.rs`), from the sprite's own inks: so the window and the browser draw identical shots.
     - Each of the 17 kinds, a forged bolt (7 colours by 6 shapes) and the Mage's and the Healer's own is a few shapes (2 to 9), lit or shining as it is: arrows and bolts a box shaft, a cone head and crossed fins; blades two crossed flat boxes spinning; bone a shaft with four knuckles tumbling end over end; ore and coal a low faceted lump tumbling; pages a flat card with a line of ink, fluttering; orbs, embers, flames, wisps, hexes and spores a glowing veil (blended) over a hot core, with a tail of beads or motes turning round; frost and stars three crossed gems; Pilgrim's Arrow a long shining shaft with a white head and gold fins; the missile a steel body with a red band, a nose and fins; the eye a pale ball with an iris and a pupil that look at the viewer however it flies.
     - A turn is a function of where the shot is (a turn for each 22 pixels it flies), so nothing is kept between frames.
     - The halo, the comet's tail, the speed line, the shadow and the crowd dimming stay layers of light, under and round the shape: the look the operator liked. The sprite and the white-hot core are what the shapes replace. A card's own drawing (`sprite shot`) stays a sprite; and a room the stage does not carry at all (the second art kit's, the kingdom, a plate or a pastime's own screen: `room_scene::layered`) stays one flat picture, shots and all, as before.
   - **The wire.** A stage frame with props is `STG3`: the STG2 frame, then a block the tag promises (`at` counted in quads, `grain`, the nodes, 92 bytes each). `wire::decode` reads both and refuses, whole, a block that lies (a count past the end or past the limit, a place past the quads, a grain out of range, a mesh that is none); a node with per-face materials is left out, an uploaded mesh is named and not sent, and nothing of the camera is on the wire, since the floor in the header gives it. The fuzz tests cover STG3; `probe/stage-fuzz.mjs` is untouched and passes.
     - An STG3 is sent only to a page that said it can draw one. `angel_stage_props(on)` (a new export of the game's wasm) is called by `boot.js` for the presenter in use: the renderer module says yes, `present.js` (`stage.js`) says no, and then the game sends STG2 with its shots as the sprites and cores it always did. A page that falls back to `present.js` after a lost device says no from that frame on. The legacy page is not changed and not broken.
   - **Both hosts.** The window: the cockpit's stage handoff carries `props` into `Host::set_stage` and nothing else of the cockpit changed. The browser: the renderer module decodes the frame and hands the stage, props and all, to the same `angel-gpu` code.
   - **Proof (RTX 3090, Vulkan):**
     - New tests: the camera against the layers' maths (angel-stage, above), the wire's props (round trips of every mesh kind and flag, every lie refused, 4,000 mutated frames, props keeping their place among the layers through quads), the shapes (every form builds, in palette inks, pointing the way it flies, a comet's tail behind it, a turn that is a function of place), and on the GPU the lit shading against hand-computed values, the stretched gem, gem and cone silhouettes, a prop on the very pixels of an upright layer, props between layers, a grain of one blocky and finer ones not, and depth among props on a tilted and a flat floor. The lockfile does not change.
     - The window, in the nested seat with the dev host's volley (`--scene volley`): before and after screenshots of the volley and of a gallery of shots standing in the same places, with 3x crops of each, are in `/work/artifacts/glass3d-shots/`. A debug build's frame time, render p50 5.5 ms before and 6.4 ms after in the volley (about 50 shots in flight), 4.5 ms and 5.5 ms for the stationary gallery.
     - The browser, in headless Chromium on the GPU: `?present=gpu` draws the shapes (`web-gpu/desktop.png`) and `?present=legacy` the sprites it always did (`web-legacy/desktop.png`). The same recorded STG2 frames through `stage.js` and through the renderer still differ as before (`present-compare.mjs`, 30 frames: 11% of the pixels between one and eight levels off, none by more), so nothing moved for the flat frames.
     - The renderer's WebGPU build grew from 188 KB to 269 KB raw (58 to 81 KB brotli: the mesh pipeline and the props block, which were not linked before as nothing used them); the WebGL2 build from 741 to 760 KB brotli; the Delve's wasm by 16 KB raw.
   - **Not here:** the shapes are not anti-aliased (the grain hides it), have no shadow cast on the room by the light (the 2D shadow on the floor stays), and the second art kit's rooms, a card's own shot drawing and the `stage.js` page keep sprites. Uploaded meshes are not sent on the wire. The figures, the room and its lights stay layers (2.2 and 2.3).

**The world's look. Built and proved 2026-10-16 (`glass/world-look`).**

The operator chose the look of the world's 3D: Jason Strachan's dithered night shoreline (x.com/jasonstrachan/status/2111047350609453291), with our knights. Measured from the video: real-time 3D at about 540 rows, scaled up 4x by nearest; every shade a 4x4 Bayer dither between two neighbouring colours of one indigo ramp (about 7 steps, `#1e2431` to `#8a93c7`), a 50% checkerboard and "+" crosses at the levels between, no smooth gradient anywhere; one warm yellow accent (moon glints, lit windows) that is not dithered into the blues; distance fog fading toward the ramp; a slow camera. It is a property a scene asks for, not a property of the shore: every scene brings its own palette.

- **The data**, `angel_stage::scene::Look` (`crates/stage/src/look.rs`), an optional `Scene::look`:
  - `ramp`, 2 to 16 colours dark to light; `accent`, 0 to 4 colours kept undithered; `dither`, `Bayer4`, `Bayer8` or `None`; `fog`, one ramp entry with a start and an end by distance from the eye; `pixel_rows`, the internal height (16 to 2160), the width following the target's aspect.
  - `Material::accent` marks a surface whose colour is kept.
  - `Look::validate` says why a look is unusable. The renderer uses `Look::normalized`: it cuts and clamps, and a ramp under 2 entries is no look at all, so the scene is drawn as it would be without one. A scene without a look is unchanged.
- **The pass**, in `angel-gpu` (`look.rs`, `look.wgsl`, `fs_look` in `mesh.wgsl`):
  - The scene is drawn at the internal size, through `fs_look`, into two targets at once: a colour texture and an R8 mask in which an accent surface is 1. Multiple render targets, because WebGPU and WebGL2 (through wgpu) both have them and it costs no extra pass. The fog is applied in that fragment shader, by distance from the eye. An accent lost in the fog (past its midpoint) is no longer an accent: it goes onto the ramp as the fog colour.
  - A fullscreen pass then reads both with `textureLoad` and writes the scene's output. For a scene's own picture (`set_scene_target`) that is the internal size 1:1, and the layer that places the picture enlarges it by nearest, as it does any picture shown bigger than it is, so one art pixel is several window pixels. Drawn straight into a bigger view (`draw_scene`), the pass enlarges by nearest itself, and the dither is still the internal pixel's.
  - **Brightness is the pixel's luminance** (Rec. 709 weights), placed between the two ramp entries whose own luminances hold it, in proportion; the Bayer threshold at the internal pixel picks the upper or the lower. So a surface coloured like a ramp entry comes out as that entry whatever its hue, and a mix has the brightness the eye reads from the dither. The other choice, the position along the ramp's own line in colour space, would put a hue the ramp does not have (a warm sand, a plume's red) wherever the line passes nearest, which is not what its brightness says.
  - The look is hard-edged: a surface is covered at half alpha or more, or it is not there (`discard`), so there are no soft edges to put on the ramp. A cleared pixel under half alpha stays transparent.
  - `Look::quantize` in `angel-stage` is the same maths on the CPU, and the tests hold the GPU's pixels to it. The core adds no threads, clock or file access: it still checks for wasm32.
- **The demo**, `ANGEL_GLASS_DEMO=shore` (`crates/glass/src/shore.rs`; `ANGEL_GLASS_SHORE_ROWS` changes the 540): a test of the look, built from built-in meshes only: bands of sea falling in brightness from the edge, foam strips drifting at the shore, sand, a seawall of boxes with dark pillars, rib arches of scaled boxes, a spine, a long mound for the beached shape, 72 accent strips on the water shimmering as the moon, and one knight from the Delve's committed sprites (`cockpit/assets/dungeon/art-v2/knight-run.png`) as an upright billboard walking the water's edge. The measured ramp, `Bayer4`, fog toward the dark end, 540 rows, a slow camera; it fills the window below the text.
- **Proof** (RTX 3090, Vulkan, `crates/glass/tests/look_gpu.rs`, which skips with no adapter): a black-to-white gradient at 64 x 64 uses all 5 ramp colours, no pixel off the ramp, and differs from `Look::quantize` in 0 of 4,096 pixels; the halfway level between each pair of ramp entries is an exact checkerboard, for `Bayer4` and `Bayer8` (0 of 4,096 off, all 8 cases); a quarter level is 4 upper pixels in every 4 x 4 tile, the pixels whose threshold is under a quarter; an accent quad is exactly its colour in all 1,984 of its pixels beside a dithered quad of the same colour; fog takes a receding floor's bands from 0.727 mean luminance near to 0.206 far, with the far rows exactly the fog colour and the nearest rows the same as with no fog; an accent in clear air is the accent and 30 units out is a ramp colour; 32 art rows shown in a 128 px view are 4 x 4 blocks (0 of 16,384 pixels unlike their block); a scene shown by a layer is a picture of its internal size (48 x 32 for a 96 x 64 view) and 2 x 2 blocks through the layer. The shore itself, drawn at 960 x 540: 8 colours (the 7 and the moon), none off the palette, 3,800 to 7,100 moon pixels over three moments.
- **Not here:** the browser has no way to send a scene yet, so the look's WebGL2 path is checked for wasm32 and by construction (MRT, R8, `textureLoad`, uniform arrays), not run in a browser; a ramp is plain sRGB-encoded bytes, as the rest of the core's colours are, with no gamma step; a scene shown at a size that is not a whole multiple of its rows has art pixels of two widths (the shore picks 540 rows; `ANGEL_GLASS_SHORE_ROWS` picks another).

**A look that lights its scene. Built 2026-10-18 (`world/monolith`).**

The operator's direction for the world (2026-10-18): dial the colour way back and the scale way up. Each place gets one clean, nearly monotone palette, with smooth passages to the next place's base colour. Castles and land are large, almost monolithic brutalist shapes in solid colours, and the light, the shadows and the dither do the work that textures did. The proof is one castle's outer wall and the approach to it over a river, before anything grows.

- **The data**, all optional on `Look`; a look with a `sun`, `lamps` or `zones` *lights* its scene (`Look::lights`):
  - `sun`: one far light that replaces the fixed key light for lit surfaces. It has a direction, a strength, an ambient, a shadow square (`Shadow`, a focus and a reach) and `rise`, which lets the sky's light grow with height, so a tall face pales toward its top.
  - `lamps`: up to 8 warm pools of light (light = activity), each moving what it lights toward the `warm` ramp.
  - `zones`: up to 4 places with their own ramp. Each is a rounded box on the ground (`at`, `half`, `radius`) with a `feather` over which its ramp passes into the one before, entry by entry.
  - `water`: the ramp water is always seen through, because a winding river fits no zone.
  - `reveal`: a point (the knight) kept in sight through anything between it and the eye, by a stippled opening that fades out by the dither. Only what is drawn is opened; shadows still fall.
  - `time`: the clock that moving surfaces read.
  - `Material::role` adds `Figure` (keeps its own colours, darkened by the light, never on a ramp: the knights), `Water` (bands running along its uv's u with the clock, and glints as the first accent) and `Sky` (unlit, on the ramp of the place the eye stands in).
- **The pass:** when a look lights its scene, each pixel is made final in `fs_look` (`lit_look` in `mesh.wgsl`), where its world point is known. It is lit by the sun (a 2048-texel depth map from the light, drawn first by `vs_shadow`, compared nine taps at a time) and the lamps. Its ramp is the look's own with each zone mixed in, then toward the warm ramp by the lamps' warmth. The brightness is then placed on that ramp by the Bayer threshold, as before. The mask then says 0.5 and `look.wgsl` only enlarges the pixel. Looks without lights take the old path unchanged.
- **The proof place**, `crates/glass/src/monolith.rs`:
  - One height field, not chunk cells: a meadow down to a meandering river, a battered quay holding up the plateau, a high slab bridge on two piers fed by a walled ramp, a dry moat with a causeway, and a curtain wall 58 m from the moat floor (ribbed with fins, banded, corniced) between square battered towers of 86 m.
  - The gate is a slot 26 m high with a lamp in it, and a keep stands behind.
  - Standing slabs line the road, and the trees are masses of dark cypress spires.
  - The palettes run meadow sage, river teal, castle indigo, with ember for lamplight.
  - `monolith_shots` shoots stills and the walk, and `monolith_walk` walks it in a window: WASD (Shift runs), Q/E turn the eye, Z/X pull it in and out.
- **Proof** (`crates/glass/tests/look_lit_gpu.rs`, real GPU):
  - A tower's shadow darkens the ground it falls on, with every pixel still a ramp colour.
  - A box zone puts its place on its own ramp.
  - A figure stays red.
  - The reveal shows the floor through a roof and nothing else.
  - The new `mesh.wgsl` and `look.wgsl` translate to WebGL2 GLSL (naga 30.0.1).
- **Not here yet:** the knight is one frame per facing with a bob (no walk cycle). The place is not in the Delve or on the wire, so it is a window and offscreen shots only, not the browser. Figures cast a blob, not a shadow.

2. **Layers compile to nodes.**
   - Every current producer draws through the scene: the Delve stage, thrown things, pictures, the council table, the map, the blast.
   - Golden tests compare the compiled scene's pixels with today's quad expansion at the same sizes.
3. **The Delve floor becomes a real camera and plane**, figures billboards standing on it, with the same look. `Tilt` retires.
4. **The text grid is a texture source.**
   - The cockpit draws into a target, which is shown on a screen quad; the window looks the same.
   - The same target on a turning panel is the proof that the grid is just a surface.
5. **The CPU world meshes reach the GPU.**
   - The ride, regions and interiors draw as meshes in the window.
   - The terminal keeps the software rasterizer.

### 3. Media as materials

1. **`TextureSource::Video`**: the ffmpeg child process natively, the video element in the browser. Frames reach the texture at the clip's own rate, apart from drawing.
2. **The cube.**
   - Three MP4s on the faces of a floating, turning cube in the chat window, opened by a command.
   - **Exit:** it holds frame time, on Linux and on the Mac mini.
3. **Then:** alpha video, glTF models with a light set (the old M2), and shared scene nodes between seats over `crates/net` (the old M4).

## Who does what

- **Design and core** (the crate boundary, the scene types, the shaders, the depth and camera work): one owner, reviewed merges.
- **Bounded jobs go to headless seats:** measurements, mechanical moves inside a decided boundary, test scaffolds, docs. Each brief carries its stop condition and the files to leave alone. A seat's diff is reviewed, and fixed or dropped, before it is committed.

## Risks

- **Bundle size on phones.** The Delve's wasm is "downloaded by every phone that joins" (`crates/Cargo.toml:17`). The wgpu web backends are measured before the browser moves (1.2).
- **Browsers without WebGPU** fall back to wgpu's WebGL2. A device without WebGL2 is not served.
- **A lost GPU context.** `present.js` rebuilds on context loss today. The wgpu surface must recover the same way, or the page reloads its seat.
- **Text in the browser.** `fc-match` cannot run there. The browser seat draws game frames, not the cockpit grid, so this waits until a grid crosses to the browser.
- **The release.** 0.2.5 ships glass and this work together.
