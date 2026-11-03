# Bend Pixel VFX cartridge

An independent, MIT-licensed pixel VFX implementation for angelX, inspired by
the capabilities of [Pixel VFX Forge](https://www.scenario.com/explorations/pixel-vfx-forge/).
No Scenario source, skills, artwork, model calls or other files are bundled.

`bend/engine.bend` implements the 22 procedural layers, seeded variation,
layer composition, timing, hard color bands, dithering, outlines and isolated
pixel cleanup. It compiles to a native CPU executable with **Bend 2.0.39**.
Balanced pixel chunks execute on four CPU threads. Node handles vocabulary,
recipe validation, palette preparation, file codecs
and transports. Every interface uses the same Bend executable. There is no
JavaScript rendering fallback or runtime subscription.

This is its own v1 recipe format and visual implementation. It does not
promise pixel parity or accept every parameter in a Scenario recipe. Unknown
fields are rejected. MP4 and character/background compositing are not included.
Preview GIFs use exact palette colors up to 255; larger palettes are reduced.

## Install and use in angelX

Requires Node.js 22+, Python 3.12+ and clang 14+. The installer supports Linux
x86_64/arm64 and macOS x86_64/arm64. It downloads Bend's checksum-pinned release
into the cartridge's `.toolchain/`, builds the native renderer, and edits no
shell configuration. The downloaded toolchain and linked Bend runtime are
Apache-2.0 licensed; see `THIRD_PARTY_NOTICES.md` and
`licenses/Bend-Apache-2.0.txt`. Only Node and the compiled
executable are needed at render time.

```sh
docs/examples/cartridges/vfx/install.sh
# Restart angelX so it discovers the tools and command:
# /vfx make an icy portal for the knight
```

The default destination is `~/.angelX/cartridges/vfx`; a first argument selects
another. Both tools (`vfx_catalog`, `vfx_render`) are deferred. `/vfx` carries
the cartridge's `pixel-vfx` skill. No core angelX code or competition cartridge
changes are required. Remove this cartridge folder to unplug it.

For a source checkout without installing:

```sh
docs/examples/cartridges/vfx/build.sh
ANGEL_CARTRIDGES="$PWD/docs/examples/cartridges/vfx" angelX
```

Set `BEND=/path/to/bend` to reuse a local **2.0.39** compiler. Re-run `build.sh`
after editing Bend source; `doctor` detects a stale binary. No compiler or
network is used during rendering.

## CLI and cartridge tool

```sh
~/.angelX/cartridges/vfx/bin/vfx catalog
~/.angelX/cartridges/vfx/bin/vfx render --prompt "holy sword slash" --seed 7 --frames
~/.angelX/cartridges/vfx/bin/vfx render examples/frost-nova.recipe.json --out vfx-out/frost
```

The tool entrypoint reads one JSON object from stdin and writes one result:

```sh
echo '{"prompt":"ice portal","out":"vfx-out/portal","seed":9}' | ~/.angelX/cartridges/vfx/bin/vfx-render
```

Supply exactly one of `prompt` and `recipe`; optional fields are `out`, `seed`,
`scale` (GIF only, 1..8, default 4), and `frames` (loose PNGs, default false).
`out` must name a **new folder inside the workspace**. Without it, a unique
folder is made under `vfx-out/`. The workspace is `ANGEL_WORKSPACE` or cwd.
Existing output folders are refused, including when calls race. Output parent
symlinks are refused; this path check is not an OS sandbox.

Outputs: native transparent PNG sheet, GIF, frame atlas JSON (fps, loop,
pivot and measured impact frame), editable recipe, Godot `.tres`, and ZIP.
The ZIP always includes individual PNG frames. Loose frames are optional.
Use nearest-neighbor filtering and disable texture compression in an engine.
The Godot resource refers to its adjacent PNG; native Godot import has not
been verified on a machine with Godot installed.

## MCP

Start the same adapter with `bin/vfx mcp`. It speaks newline-delimited
JSON-RPC over stdio, supports initialization, ping, `tools/list`, and
`tools/call`, and advertises MCP revision `2024-11-05` (also used by angelX).
Diagnostics stay on stderr. The installer writes `mcp.example.json` with the
absolute command path. Merge that entry into the client's MCP configuration
when using the server independently. In angelX, choose the cartridge or the
MCP connection to avoid advertising duplicate tool names. No MCP configuration
is edited by the installer.

## Local HTTP API

```sh
~/.angelX/cartridges/vfx/bin/vfx serve --port 8787 --workspace "$PWD"
curl http://127.0.0.1:8787/v1/catalog
curl -H 'Content-Type: application/json' \
  -d '{"prompt":"fire explosion","out":"vfx-out/explosion","seed":7}' \
  http://127.0.0.1:8787/v1/render
```

`GET /health` reports build readiness. `GET /v1/catalog` describes the engine.
`POST /v1/render` uses the same input as the cartridge tool and returns paths
plus download URLs. `GET /v1/files/<job-id>/<filename>` downloads a top-level
artifact; individual frames are in the ZIP. The last 128 jobs' URLs remain
available until the server stops. Files persist afterward in the workspace.
The server binds only to `127.0.0.1`, rejects browser Origin headers, accepts
JSON bodies up to 256 KiB, and allows two concurrent renders. It is a local
authoring service, without remote hosting or authentication features.

## Recipes

See `examples/frost-nova.recipe.json` for a complete example. Fields:

| Field | Values / default |
|---|---|
| `name` | Label, up to 160 characters |
| `w`, `h` | Native frame size, 8..256, default 64 |
| `frames`, `fps` | 2..64 frames, default 16; 1..60 fps, default 15 |
| `loop`, `seed` | Boolean, default false; 32-bit unsigned seed, default 1 |
| `outline`, `dither`, `clean` | `none` / `outer` / `inner`; 0..1; boolean |
| `palette`, `hue` | Optional `pico-8`, `game-boy`, `1-bit`; hue rotation -360..360 |
| `pivot` | Normalized [x,y], default [0.5,0.5] |
| `ramps` | Optional map of custom named ramps |
| `layers` | 1..24 layers, drawn in order |

Layer `type` is one of `burst`, `ring`, `flame`, `beam`, `slash`, `orb`,
`comet`, `vortex`, `pillar`, `rays`, `glyph`, `puffs`, `particles`, `bolt`,
`cracks`, `spikes`, `star`, `streaks`, `bubble`, `flash`, `text`, `tornado`.
`ramp` names an element in `catalog`, contains 2..16 RGB hex colors, or names
a recipe ramp with `$name`. Remaining parameters share the following vocabulary;
each layer uses the parameters appropriate to its shape:

| Parameter | Meaning / default |
|---|---|
| `t0`, `t1` | Layer's time window within the effect, 0..1 / default 0,1 |
| `x`, `y` | Center offset in half-short-canvas units, -2..2, default 0 |
| `r0`, `r1` | Start/end radius; default 0.08,0.75 |
| `th` | Edge, beam or ring thickness, 0.008..1, default 0.12 |
| `angle`, `squash` | Rotation in degrees; vertical scale, default 0,1 |
| `gain` | Heat multiplier, 0..2, default 1 |
| `speed`, `spin` | Travel speed / complete rotation cycles, default 1,1 |
| `count` | Particle, spoke or spike count, 1..64, default 12 |
| `size`, `life`, `gravity` | Particle radius, lifetime fraction and vertical acceleration |
| `mode` | `burst`, `rise`, `fall`, `ambient`, `implode`, `orbit`, `trail`, `swirl` |
| `text` | 1..8 digits or `+ - ! space`; default `+100` |

The shorter canvas dimension spans -1..1. Y grows downward. Radius and particle
sizes use those units. Text is a small 3x5 font. A one-shot fades to transparency
at the last frame; analytic loops use whole speed/spin periods, and particle
loops fade at their reset. Prompt vocabulary is fixed and rule-based, with one
primary form per prompt; it does not call a language model. Color words are
limited to the supported elements plus blue/purple adjustments. Seeds are
deterministic on this build; float math is not a cross-architecture pixel-parity
guarantee. Total render size is capped at 1,048,576 native pixels and 16,777,216
preview pixels, and native execution times out after 60 seconds.

## Validation

```sh
cd docs/examples/cartridges/vfx
./build.sh
npm test
```

The tests execute the native renderer, check all layer types, deterministic
recipes, byte lengths, binary alpha, one-shot completion, exports and errors,
and exercise the real stdio MCP, HTTP and cartridge entrypoints. Development
visual checks additionally decode PNG/GIF with Pillow and ZIP with Python's
standard library; neither is needed by the cartridge at runtime.
