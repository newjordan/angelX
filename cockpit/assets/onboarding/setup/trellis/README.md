# Local TRELLIS scroll roller — S02

This is a real image-to-3D export, not a procedural cylinder labeled as model output.
Only the rigid rollers/endcaps are model-derived. The writing surface remains the
live deforming paper and native glyph atlas so text stays readable and interactive.

## Identity

- Source art: `../scroll.png`, SHA-256 `1009e90450dcf147c03cafcb386f1cd21fcf3ccf2e4c7b33106f837de4e736c1`.
- Selected reference: `roller-only-reference.png`; authored roller crop, explicit transparent silhouette mask, rotation and scaling. No image synthesis.
- Local route: a local GPU worker / installed `trellis.cpp` v0.4.3, implementation commit `d24b02d90b327936f1d08517a35cf2c4e3b1a964`.
- Model: `microsoft/TRELLIS.2-4B`, GGUF repository `ilintar/trellis2-gguf`, revision `9fd0030e18ce6340f685519f42e12b6bd3e42248`.
- Selected job: `5c67c41d322745729a9673d5f7abc48b`, seed `27017`, 512 resolution, BiRefNet background removal, xatlas UVs, 1024 texture.
- Raw GLB SHA-256: `d36244a84dec603b6a2e35e809a4b003135151518bea12ada46a7255d3285d49`.
- Source mesh: 7,493 vertices / 9,186 triangles. Runtime: 6,487 vertices / 7,815 triangles.
- `roller.axsm`: SHA-256 `c06cf3a93565d4258a98db26c416f81bc0beb6cb2082089aacb51be816f03d32`.
- `roller-basecolor.png`: RGBA 1024×1024, SHA-256 `70bcf93a1569b93de397b8dcfa49d3fda73a23b25fcd5d5b4efd7c5862fa4a68`.

`roller-export.json` records normalization, reduction and output hashes.
The reproducible exporter is `scripts/scroll/export_trellis_roller.py` at the
repository root. It requires the existing worker's numpy/trimesh/Pillow; none
of these are runtime UI dependencies. It aligns the generated axle with X,
normalizes length/radius separately for UI fitting, clusters vertices within
spatial and UV cells to meet the budget, and recomputes normals. It is not
lossless or a full material-preserving PBR renderer: runtime uses base color
plus normal-based lighting. The raw GLB retains the original PBR export.

## Runtime and fallback

The mesh and texture are embedded in the Glass build and decoded only once.
The generative model is not needed at launch. Positions and normals rotate
around the axle with the released-paper/radius motion; UVs stay on the mesh.
Triangles use local painter ordering rather than a scene depth buffer.
`ANGEL_SCROLL_MESH=0` selects the prior procedural roller control. Terminal
and small-height fallbacks are unchanged; reduced/off motion snaps.

## Evidence and review boundary

All three generation attempts, original GLBs, derived PLYs, worker receipts,
reference operations and verification logs are retained under
`artifacts/scroll-menu/trellis-S02/`. Attempt 1 was rejected as flat; attempt 2
was not selected; attempt 3 uses the tighter roller-only silhouette.
A stale service path failed before generation and was stopped; the
container started for this job was stopped after export. No packages
or model weights were installed, and no unrelated running model was stopped.

This is a **local engineering candidate**, not a human-approved visual or
measured performance/usability improvement. The worker reports the source
model as MIT and its own implementation-code license as unresolved/local-eval;
these receipts are provenance, not redistribution/legal approval.
