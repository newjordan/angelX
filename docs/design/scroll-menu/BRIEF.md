# Floating setup scroll — S01

## Goal
A delightful physical foreground menu above the cockpit; no game changes.
Use the existing Rust/WGPU compositor and font atlas; no added packages.

## Delivery and assets
Live `/setup` UI, resizable window, existing fonts. Procedural parchment,
wooden rollers with brass endcaps, restrained warm lighting, cast shadow.
Terminal: static readable parchment/plain-text fallback. No audio in S01.

## Layout
Centered scroll, safe window margins, flat reading area, curved ends. Underlying
workspace stays visible. Text is rendered by the native atlas, not a screenshot.
Mouse/keyboard behavior stays with existing setup actions. Approval prompts win.

## Proposed timeline (reference frames at 60 Hz, not measured performance)
0: scroll arrives rolled, top roller near its final anchor.
1–30: weighted lower roller drops, unwinding the paper; rotation = travel/radius.
31–60: damped settling; no perpetual idle wobble during reading.
Close: rewind then depart, retaining the last page only for the visual exit.
Reopen during exit: reverse continuously. Escape closes input immediately.
Reduced/off motion: stable instant surface; no flights or bouncing.

## Implementation and review
Fixed 1/120 s steps with capped elapsed time and bounded catch-up. Constrained
paper surface rather than unconstrained cloth. Projection and normal-based
shading, explicit frontmost compositor surface, native text mapped with paper.
Review rolled, half-open, fully-open, rewinding, and reverse-motion boundaries.
Tests cover workspace preservation, motion bounds/rest/reversal, coherent
geometry/hits, existing setup actions, tiny layouts, and alpha/shader validation.
Preregistration and source identities: artifacts/scroll-menu/preregistration.json.
No usability/performance improvement claim without a separate controlled trial.
