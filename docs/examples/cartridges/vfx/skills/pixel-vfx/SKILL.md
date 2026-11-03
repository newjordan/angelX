---
name: pixel-vfx
description: Author and revise local pixel-art effects for angelX using the Bend VFX cartridge, including spell, hit, portal, aura and loot animation sheets.
---

Use `vfx_catalog` to discover effect words and layer types, then `vfx_render`
with either `prompt` or `recipe`. The tools are deferred; discover them with
`tool_search` if needed. Rendering runs on the CPU and needs no service account.

The equivalent CLI is `${ANGEL_CARTRIDGE_DIR}/bin/vfx`:

```sh
"${ANGEL_CARTRIDGE_DIR}/bin/vfx" render --prompt "holy sword slash" --seed 7 --frames
"${ANGEL_CARTRIDGE_DIR}/bin/vfx" render path/to/effect.recipe.json --out vfx-out/revised
```

Inspect the GIF and sampled PNG frames. Match the effect's native pixel density
to the character; GIF scale changes the preview only. Tune the exported recipe
and render into a new folder. Preserve the atlas's fps, pivot and impact frame
when importing it into the Rust renderer. A procedural effect does not alter
gameplay collision or timing by itself.

For layer parameters and interfaces, read
`${ANGEL_CARTRIDGE_DIR}/README.md`. This is an independent recipe format:
unsupported fields are errors. A blank or unknown prompt is an error, not an
invitation to silently substitute an effect. If the native renderer is missing
or stale, run `${ANGEL_CARTRIDGE_DIR}/build.sh`.

Deliver the native PNG sheet, JSON atlas and editable recipe; include the GIF
for review and the ZIP for transfer. Use a new output folder each iteration;
the tool refuses to overwrite existing folders.
