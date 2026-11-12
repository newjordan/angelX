# Gauss / Glass

An interactive **native Glass** Gaussian-splat experiment, now with a GPU-resident
renderer for real trained assets. No browser, video playback, image generator,
network service, or API key at runtime.

Two modes live side by side:

- **The Resonator** — the original interactive 16,606-Gaussian procedural instrument.
- **Import study** — detailed `.splat` / Gaussian `.ply` models, including a verified
  **1,060,619-Gaussian degree-3 SH capture**, drawn directly on Glass's Vulkan device.

## Run

From this directory inside the angelX checkout:

```sh
./run.sh                                         # Original Resonator
./run.sh --import assets/nike.splat               # 270,491-Gaussian shoe
./run.sh --import assets/nike.splat --quality ultra  # FP16 + 2x per-axis sampling
./run.sh --import assets/train.splat              # 1,026,508-Gaussian scene
./run.sh --import assets/capture-sh3.ply          # 1,060,619 Gaussians, full SH3
./run.sh --inspect assets/capture-sh3.ply         # Validate/report, no window needed
```

The launcher uses Cargo offline; dependencies must already be cached. To reuse
the development cache:

```sh
CARGO_TARGET_DIR=~/build-cache/angelX/cockpit-target ./run.sh --import assets/nike.splat
```

A separate native Glass window opens. It does not replace the cockpit, change
its configuration, or install a permanent service. Third-party evaluation models
are local/ignored files; see [asset provenance, restore commands and license caveat](assets/README.md).

## Import controls

| Action | Control |
| --- | --- |
| Orbit | Drag; arrow keys |
| Cycle image quality | Click QUALITY; F: Balanced → Ultra → Reference |
| Toggle drag between orbit and pan | Click MODE; P |
| Pan | WASD; drag in pan mode |
| Zoom | Scroll; + / - |
| Auto-orbit | Click ORBIT; Space |
| Exposure | [ / ] in quarter-stop increments |
| Quarter-turn around X / Y / Z | X / Y / Z respectively |
| Reset camera and exposure | Click RESET; R |
| Close | Q / Escape; window close |

The pointer control strip is hidden on very small windows; keyboard shortcuts remain available. A button activates only when the press is released over the same control without dragging. Reset preserves the selected quality and viewport, and stops auto-orbit and pan mode.

Framing uses the dense 5th–95th percentile positional bounds to avoid distant
capture floaters. This is only a camera transform: **no Gaussian records are
removed or downsampled**. Source coordinates, scales, opacity and SH are retained.
Pan/zoom allow inspection beyond the initial frame. Text remains crisp native UI,
not splatted glyphs.

## Imported renderer

`src/import.rs` parses schemas instead of guessing fixed PLY layouts:

- Trained Gaussian PLY: ASCII, binary little-endian and binary big-endian.
- Arbitrary property order; log-scales, sigmoid opacity, normalized w/x/y/z quaternions.
- Graphdeco DC plus complete SH degrees 0–3, with coefficient-major RGB storage.
- Antimatter15 32-byte `.splat`: position, anisotropic scale, RGBA and packed rotation.
  This format has **no higher-order SH**; the importer cannot recover absent data.
- Rejects plain point clouds, malformed/truncated records, nonfinite values,
  incomplete SH schemas, unsupported formats and assets over 2,000,000 Gaussians.

`src/gpu.rs` and `src/gaussian.wgsl` implement:

1. One-time geometry/covariance-factor and full-float SH upload. Scalar SH is split across
   two storage banks, keeping the tested million-splat SH3 asset within device
   binding limits. Oversized assets fail explicitly rather than silently truncating.
2. GPU perspective-Jacobian projection of the three scale/rotation factors
   `A = R diag(s)`, preserving full anisotropy without squaring away thin axes.
   For projected rows `u`, `v`, area is `|cross(u,v)|²`, not the unstable
   subtraction `a*c - b*b`. The minor eigenvalue is `det/lambda_max`, avoiding
   cancellation in `mid-delta`. A 0.3-pixel-squared floor integrates pixel area.
   Balanced/Ultra compensate opacity by `sqrt(det(original)/det(filtered))`:
   the filter broadens tiny splats without inventing extra energy.
3. View-dependent real spherical harmonics through degree 3. This reproduces
   learned appearance; it is **not physically correct dynamic relighting**.
4. Stable, exact float-depth ordering using two CPU radix passes only when the
   camera rotation changes. No 16-bit depth quantization, no per-frame allocation.
5. An instanced quad pass, Gaussian alpha, and premultiplied back-to-front blending.
   Balanced/Ultra accumulate in **RGBA16Float**, then quantize once at resolve;
   bright SH values are not clipped after each translucent layer. This increases
   compositing precision; it does not reinterpret the source's color space.
6. Alpha-aware support extends to the 1/255 contribution threshold (up to about
   3.33 sigma) rather than cutting every splat at 3 sigma. Reference retains the
   old cutoff. Very low alpha is discarded and huge projected axes remain capped
   at 2,048 render pixels as rendering safeguards.

### Quality modes

- **Balanced (default):** energy-compensated pixel filter + FP16 composition at
  native picture resolution. Better subpixel stability with one sample per pixel.
- **Ultra:** the same renderer at **2× width and 2× height**, with exact 2×2 box
  integration to the native picture. Four samples per output pixel, no sharpening
  or tone-curve trick. Text is still drawn natively, not supersampled.
- **Reference:** direct 8-bit composition, uncorrected opacity filter and fixed
  3-sigma support, retained for same-camera A/B comparisons. All modes now share
  the numerically stable factor projection; Reference is not a byte-for-byte
  replay of the pre-fix covariance shader.

Choose `--quality balanced|ultra|reference` at launch or press **F**. Switching
reuses geometry, SH, index buffers and both pipelines; resize only replaces the
intermediate target. Reset preserves the selected mode. Ultra falls back to 1×
if 2× would exceed the device texture dimension or **128 MiB intermediate-target
budget**. The status line reports the **actual** sample scale, dimensions and
separate target memory; the requested mode may still read Ultra after fallback.
At 1000×760, the FP16 intermediate costs 5.8 MiB Balanced / 23.2 MiB Ultra.
These costs exclude source buffers and Glass's own target/compositor textures.

Glass gained a small `PictureProducer` hook in `../crates/gpu` and
`../crates/glass`: a producer renders into a compositor-owned picture target on
its existing device/queue. Native picture layers place the result; replacement
and removal release the producer's resources. Targets resize with the viewport,
clamped to 4,096 pixels per dimension. The application does not maintain a second
GPU device or copy a rendered image through CPU memory every frame.

## Original Resonator

A dark ceramic plinth, metallic rings, illuminated core and engraved ticks,
sampled deterministically from 16,606 anisotropic Gaussians. Its frequency is a
visual parameter, **not an audio synthesizer or a cockpit setting**.

| Action | Mouse | Keyboard |
| --- | --- | --- |
| Orbit | Drag the object | Up/Down tilt; O toggles auto-orbit |
| Zoom | Scroll | + / - |
| Power / standby | Core or Power | Space |
| Relight | Warm/Cool Light | L |
| Resonance / core shape | Tuning track | Left / Right |
| Reset | Reset View | R |
| Close | Window close | Q / Escape |

This mode still projects and sorts its procedural splats on the CPU, then submits
ellipses as rotated picture layers. One immutable Gaussian atlas packs both light
palettes, four materials and 64 lighting levels; changing light selects UVs rather
than replacing a live texture. Its normal-based diffuse/specular relighting is
separate from the imported model's SH appearance.

## Verification and capture

```sh
cargo test --offline --all-targets
GAUSS_CAPTURE_DIR="$PWD/captures/resonator" ./run.sh --smoke
GAUSS_CAPTURE_DIR="$PWD/captures/sh3" ./run.sh --import assets/capture-sh3.ply --smoke
```

The original smoke sequence exercises five control states and prints
`GLASS_SMOKE_OK`. The import smoke saves three native GPU captures (initial,
orbit, zoom/pan/exposure), self-closes after about 8.5 seconds, and prints
`GAUSS_GPU_SMOKE`. Captures and logs are ignored by Git.

Verified on RTX 3090 / Vulkan:

- **9 model/import/control tests and 7 real-GPU regressions pass** on RTX 3090.
  GPU checks cover tiny-splat energy, bright translucent layers, exact 2×2 resolve,
  mode/resize without reuploads, thin anisotropic projection, fully 3D perspective
  projection, and subpixel motion energy. Two asset-dependent export tests are
  ignored by default and were run explicitly. GPU tests fail without an adapter
  rather than silently reporting a pass.
- Native GPU runs imported the 270,491-splat shoe and the 1,060,619-splat SH3 PLY.
- SH3 resident geometry + SH + index + uniform buffers: **275,761,052 bytes
  (263.0 MiB)**. This excludes picture targets, compositor textures, GPU allocator
  overhead and host-side source data; it is **not total process VRAM**.
- Exact CPU sorting for the million-splat PLY took about **45 ms on rotation
  changes** in the development build. Static views, pan, zoom and exposure reuse
  the order. Reported submission timings are CPU-side and may include sorting.
  Producer submissions are **not** presented frames or measured GPU frame time.

### Measured quality evidence

`tests/gpu_quality.rs` first reproduced the old renderer's defects, then verified
these corrections on the actual WGSL/production pipelines:

- Tiny test splat: summed red contribution above background **248 → 12**;
  the enlarged low-pass footprint no longer invents brightness.
- Bright layer behind a dark translucent foreground: centre pixel **128 → 255**;
  FP16 preserves the bright value until all layers have blended.
- Ultra versus explicitly averaged 2×-resolution output: maximum per-channel
  error **1/255**, consistent with one final quantization vs four quantizations.

Same-camera, 1000×760 shoe captures and 1:1 detail crops are in
`captures/quality/nike/`. A separately rendered 4000×3040 output, box-downsampled to
1000×760, serves as a **sampling-consistency reference**, not independent ground
truth. Current factor-projection captures reproduce these foreground RMS errors
in 8-bit levels; the preceding pass's files are preserved separately under
`captures/quality/preceding-pass/`. Each current asset directory contains source
metadata, image hashes and its own `metrics.json`.

| Shoe view | Reference | Balanced | Ultra |
| --- | ---: | ---: | ---: |
| Overview | 14.69 | 3.69 | 1.00 |
| Close-up | 5.16 | 1.19 | 0.44 |

The mask selects reference luminance >10. These measurements quantify closeness
to higher sampling of this same renderer; they do **not** establish source-image
fidelity, trained anti-aliasing correctness, or a subjective visual-quality score. No timing/FPS benefit is claimed; Ultra adds GPU work.

### Thin-detail and motion-stability correction

Projecting a 10,000:1 thin Gaussian at 45 degrees exposed a **48.4% brightness
error** in the previous covariance-subtraction path: 3,104 summed red levels
versus 2,092 in an independent double-precision oracle. Factored projection gives
2,074; the residual pixel quantization is at most **1/255**. All **35** tested
orientation/subpixel combinations, including near-axis angles, stay within
1/255 per channel. Three off-centre perspective cases with arbitrary 3D Gaussian
and camera rotations also agree with an independent `J C J^T` f64 oracle within
1/255. The oracle does not reuse production factor/covariance/eigenvector code.

Across 17 translations spanning one output pixel, a small isotropic splat's
summed red energy varies peak-to-peak **6.07% Balanced vs 3.05% Ultra**. Reference
varies 1.33%, but produces approximately **4.6× too much energy**; lower variation
alone is not better fidelity. This is one controlled fixture, not a guarantee
against all motion shimmer, depth-order popping or reconstruction floaters.

Preserving the three factors uses 64 rather than 48 geometry bytes per Gaussian:
+16 bytes each, about **16.2 MiB extra** for the million-splat SH3 model. No source
records, SH coefficients or small axes are removed to obtain the improvement.

A real-GPU-frame motion preview is in
`captures/stability/gauss-motion-quality.mp4`: **1488×680, 144 frames, six seconds
at 24 fps**, Balanced versus Ultra at identical cameras, with quiet initial/final
holds. It is an **offline render**, not a real-time recording or performance
benchmark. `REFERENCE.md` and `BRIEF.md` in that directory document its camera
sequence, sources, layout and review limits. Encoding/frame count and full decode
were verified; independent aesthetic review remains quota-blocked.

To reproduce the manual asset-dependent comparison (ignored in normal test runs):

```sh
cargo test --offline --test gpu_quality export_same_camera_quality_comparison -- --ignored --nocapture
# Optional alternate local trained asset (writes its own directory):
GAUSS_QUALITY_ASSET=assets/capture-sh3.ply cargo test --offline --test gpu_quality export_same_camera_quality_comparison -- --ignored --nocapture
# Deterministic motion frames and comparison sheets:
cargo test --offline --test gpu_quality export_quality_motion_preview -- --ignored --nocapture
python3 tools/quality_evidence.py --motion
ffmpeg -framerate 24 -i captures/stability/frames/%04d.png -c:v h264_nvenc -preset p7 -cq 19 -pix_fmt yuv420p captures/stability/gauss-motion-quality.mp4
```

The optional evidence assembler needs Pillow and fontconfig; the app does not.
Rendered thumbnails/crops are labelled; raw GPU PNGs are retained unretouched.

Remaining boundaries: no SPZ/compressed PLY, animated `.splatv`, GPU sorting,
LOD/streaming, physical shadows or imported-object deformation yet. SH correctness
is implemented by convention, not established against a reference image corpus.
Full GPU timestamp/frame-pacing profiling and subjective visual approval are still
pending. The vision service is quota-blocked; native captures, image statistics
and OCR are evidence of rendering/labels, not an independent aesthetic review.
Real keyboard orbit was verified in the preceding viewer pass. During this quality
pass, private-display key delivery did not reliably reach the test windows, so live
F-key switching is **not claimed verified**. Quality state transitions and resize
are covered by the real-GPU lifecycle regression. Neither scripted smoke captures
nor these tests prove end-to-end mouse input.
