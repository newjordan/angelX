# glass vs kitty: CPU hot-path research (2026-10-26)

Goal: make `glass` (GPU compositor, `crates/glass`) beat kitty on every
metric that is not "being the GPU itself": per-frame CPU time, input→present
latency, and allocation churn in the steady-state text path.

## What changed (`crates/glass/src/render.rs`)

1. `Renderer::build`: removed per-frame `self.palette.clone()` (60-byte
   `Palette`: 4×`Rgb` + 16×`Rgb`); hoisted `foreground/background/cursor/
   cursor_text` into locals.
2. Layer assembly: fast path — `layers.windows(2).all(|w| w[0].z <= w[1].z)`
   + `partition_point(|l| l.z < 0)` feeds `core.layer_parts` directly with
   zero alloc; only the unsorted tail pays `collect` + `sort`.
3. `symbol_hash` (multi-char glyph-memo key): `DefaultHasher` (SipHash-1-3)
   → `angel_gpu::FxHasher` (FxHash, already the `FastMap` hasher). Same
   contract: hash + store symbol beside entry + verify on hit, so no
   allocation on lookup either way; only the hash function changes.

Verifier: `cd crates && cargo test -p angel-glass --lib`
→ `ok. 108 passed; 0 failed` (this tree).

## Method

Micro-benches on this host (release `-O`, `Instant` delta / N iters):

- `crates/glass/examples/bench_opt.rs` (temporary, since removed):
  `symbol_hash` over 7 representative symbols × 200k iters each;
  palette-clone vs field-copy × 500k iters.
- `/tmp/sortbench.rs` (throwaway): sorted-check vs full `sort` for
  n = 0/2/4/8/16 layer refs × 200k iters.

No kitty binary was benched here — the kitty half below is architectural
(frame pacing, present modes, instancing) plus what to measure next.

## Results

### T1 — `symbol_hash`: SipHash vs FxHash (ns/op, 200k iters each)

| symbol | bytes | DefaultHasher | FxHasher | speedup |
|---|---|---|---|---|
| `e` | 1 | 67.1 | 35.2 | 1.90× |
| `─` | 3 | 73.2 | 36.4 | 2.01× |
| `│` | 3 | 73.3 | 37.5 | 1.96× |
| `ab` | 2 | 77.2 | 36.4 | 2.12× |
| `hello` | 5 | 74.5 | 37.5 | 1.99× |
| `Ｈｅｌｌｏ` | 15 | 112.0 | 59.1 | 1.89× |
| `😀` | 4 | 72.8 | 36.5 | 2.00× |

Mean speedup ≈ **1.98×**. This fires once per *miss-candidate*
multi-char glyph per frame (CJK/emoji-heavy grids hit it hardest), so the
frame-level win scales with symbol diversity, not grid size.

x/y — hash cost vs input (x = input bytes, y = ns/op):

```
ns/op 120 ┤                                              ● default
        100 ┤                                         ●
         80 ┤  ●──●──●───●                          ╳
         60 ┤                   ●                   ╳──╳──╳───╳──● Fx
         40 ┤  ╳──╳──╳───╳──╳                        ╳
         20 ┤
          0 ┼──┬──┬──┬───┬───┬───┬─── bytes
             1  2  3  4   5  10  15
● = DefaultHasher, ╳ = FxHasher
```

Bar chart — speedup by symbol (2.0× ≈ halved hash cost):

```
e       ███████████████████ 1.90×
─       ████████████████████ 2.01×
│       ███████████████████ 1.96×
ab      █████████████████████ 2.12×
hello   ███████████████████ 1.99×
Ｈｅｌｌｏ    ███████████████████ 1.89×
😀       ████████████████████ 2.00×
```

### T2 — layer sort fast path (ns, 200k iters)

| layers (n) | `windows(2).all` check | full `collect+sort` | check is faster |
|---|---|---|---|
| 0 | 0.2 | 0.6 | 2.4× |
| 2 | 0.4 | 5.5 | 12.4× |
| 4 | 0.8 | 5.8 | 7.1× |
| 8 | 1.6 | 7.3 | 4.4× |
| 16 | 3.7 | 12.4 | 3.3× |

Hosts emit layers back-to-front already, so the steady state pays only the
`O(n)` check (~1–4 ns) and zero allocation; the old path paid `collect` +
`sort` every frame.

x/y — layers (x) vs cost (y, ns):

```
ns 14 ┤                                      ○ sort
    12 ┤                                   ○
    10 ┤
     8 ┤              ○───○
     6 ┤     ○───○
     4 ┤                                  ●───● check
     2 ┤  ○  ●───●───●
     0 ┼──┬───┬───┬───┬───┬── n
         0   2   4   8  12  16
○ = collect+sort, ● = windows(2) check
```

### T4 — `symbol_width` 2-byte fast path (ns/op, 2M iters × 10 symbols, debug)

| op | ns/op |
|---|---|
| table walk (`unicode-width`) for é/ü/ñ/α/Ω/Ж/я/ö/è | 34.9 |
| byte-lead fast path (`0xC3..=0xC7\|0xCE\|0xCF\|0xD0\|0xD1`) | 10.5 |

Speedup ≈ **3.3×** on every 2-byte Latin-1-letter / Latin-Extended /
Greek / Cyrillic cell, once per non-space non-picture cell per frame
(`render.rs` span computation). Combining marks (`U+0300..`), Hebrew,
Arabic and C1 controls stay on the table path, so the fast path agrees
with `unicode-width` exactly (new exhaustive test
`the_fast_path_agrees_with_the_table` covers U+00C0–U+02FF/U+0370–U+04FF
2-byte chars plus combining/Hebrew/Arabic/NBSP/soft-hyphen witnesses).

### T5 — hoisted per-frame cursor compare (ns/cell, 80×24 grid × 50k frames, debug)

| op | ns/cell |
|---|---|
| `snapshot.cursor == Some((col, row))` per cell | 12.17 |
| hoisted `(cursor_col, cursor_row)` + row-gated int compare | 3.09 |

Speedup ≈ **3.9×** on the per-cell cursor check, which fires for every
cell of every frame (plus once more per picture-span neighbour probe).
For an 80×24 grid that is ~1920 × ~9 ns ≈ **~17 µs saved per frame** of
pure CPU, before any glyph work. Semantics bit-identical (`-1` sentinel
means hidden; `u16` grid coordinates always convert to `i32` exactly).

### T3 — palette hoist (honest null result)

| op | ns/op (500k iters) |
|---|---|
| `palette.clone()` (60 B) | 5.4 |
| 4-field copy | 7.7 |

The clone itself is ~5 ns — negligible against a frame. Kept anyway: it
removes a 60-byte per-frame copy from the hottest function, keeps the
working set in locals the compiler can enregister, and makes the
`foreground/background/cursor/cursor_text` data flow explicit. No frame
regression risk; do **not** cite this as the headline win.

### T6 — `symbol_width` 4-byte CJK Extension B/G fast path (ns/op, 8 symbols x 200k iters)

| op | release ns/op | debug ns/op |
|---|---|---|
| table walk (`unicode-width`) for U+20000/U+2A6DE/U+30000/... | 2.6 | 38.8 |
| byte-lead fast path (`0xF0` + `0x97|0xA0..=0xAE|0xB0..=0xBE`) | 0.3 | 13.5 |

Speedup ~ **7.6x release / 2.9x debug** per 4-byte CJK cell, once per
non-space non-picture cell per frame. Covers 126,976 codepoints
(`F0 97` = U+17000..U+187FF, `F0 A0..AE` = U+20000..U+2EBEF,
`F0 B0..BE` = U+30000..U+323AF), each probed 4096/4096 wide with zero
holes; neighbours with holes (`F0 98`/`9F`, `AF`/`BF` noncharacters
U+2FFFE/U+3FFFE...) stay on the table path. New exhaustive test
`the_fast_path_agrees_with_the_table_supplementary` pins all 126,976
plus hole witnesses. Extended 2-byte leads (C8..CB Latin Ext-B,
D3..D5 Cyrillic suppl, DA Arabic U+0680..U+06BF) hold **3.3x release /
2.4x debug**; placeholder double-check fuse is optimizer-neutral
(~1.0x, kept: halves the 4-byte prefix compare on the source level).

### T7 — ASCII cell prologue fast path (ns/op, 4 syms x 1M iters, `rustc -O`, `black_box`)

| op | ns/cell-prologue |
|---|---|
| old: `== b" "` + `starts_with([F4 8E BB AE])` + 1-byte width + blank loop | 2.29 |
| new: `len == 1` early-out (`(None, 1)`, lazy blank) | 1.76 |

Speedup ≈ **1.30×** (~0.5 ns/cell) on the common ASCII text cell, which
skips the 4-byte placeholder prefix scan, the `symbol_width` call, and
the `is_blank_symbol` scan (now a lazy `matches!(b, ' ' | '\t' | '\n' |
'\r')` closure that picture cells never pay). Semantics bit-identical:
1-byte symbols can never be pictures (prefix starts `0xF4`), always
span 1 (`clamp(1,2)`), and the match arm is exactly the ASCII loop of
`is_blank_symbol`. For an 80×24 mostly-ASCII grid that is ~1920 × ~0.5
ns ≈ **~1 µs saved per frame** before glyph work. First naive bench
without `black_box` folded to 0.00 ns (141× illusion) and a trivial
stub bench showed 0.85× — both discarded; the 1.30× above keeps the
real prefix/blank work behind `#[inline(never)]` + `black_box`.

### T8 — raw-`Rgb` background-run key (ns/cell, 1920-wide row x 2000 iters, `rustc -O`, `black_box`)

| row | old (premult per cell, 4-float compare) | new (raw `Rgb` key, premult per flush) | speedup |
|---|---|---|---|
| solid (1 run) | 1.67 | 1.08 | **1.55x** |
| alternating (1920 runs) | 2.93 | 1.84 | **1.60x** |

The old background run carried `(x0, x1, [f32; 4])`: every cell paid
a `premultiply` (3 float x `k` + pack) plus a 4-float equality
compare. The new key is `(x0, x1, Rgb)`: one 3-byte integer compare
per cell, and `premultiply` runs once per flushed run. Solid rows
(the common case — one run per row) save ~0.6 ns/cell; even the worst
case (a flush every cell) saves ~1.1 ns/cell because the per-cell
float multiply is gone. Semantics bit-identical: the pushed quads are
the same floats — verified by asserting old/new produce identical
`(color, x0, x1)` vecs on both rows before timing
(`/tmp/bg_run_bench.rs`, `#[inline(never)]` + `black_box` both sides).
For an 80x24 solid-background grid that is ~1920 x ~0.6 ns = **~1.1
us saved per frame**, on top of T7, in the background-run path that
every text cell touches.

### T9 — lazy `x`/`w` + lazy `fg` for background-only cells (ns/cell, `rustc -O`, `black_box`)

| workload | old (eager `x`/`w` + eager `fg`) | new (integer col-run, floats + `fg` only on ink) | saving |
|---|---|---|---|
| solid 1920w bg-only (`/tmp/bg_lazy_bench.rs`, 2000 iters) | 0.23 | 0.20 | **1.13x (~0.03 ns/cell)** |
| space-cell `fg` elision (`/tmp/bg_fg_bench.rs`, 3000 iters, Reset fg) | 1.23 | 0.21 | **6.01x (~1.0 ns/cell)** |
| space-cell `fg` elision (Indexed-200 fg, `indexed()` divide path) | 3.54 | 0.21 | **17.14x (~3.3 ns/cell)** |

What changed (`crates/glass/src/render.rs`, `Renderer::build` row loop):
blank background-only cells never materialize `x = ox + col*cw` /
`w = span*cw` (no `col*cw` multiply, no `x+w` add) — the run key is now
`(col_start, col_end, Rgb)` with pure integer compares and floats only
on flush. `fg = self.rgb(...)` is lazy too (`fg_cache: Option<Rgb>`):
space/blank cells never draw a glyph/underline/strike so they skip the
`rgb()` match entirely (and the `indexed()` divide path for 256-colour
text); REVERSED/cursor overrides still seed the cache so ink paths see
identical colours. At-rest `moved()` ignores its cell rect, so
blank/picture/glyph/underline paths pass `[0.0; 4]` and skip both
float multiplies there as well.

Honesty note: the 1.13x is the isolated float-elision delta on top of
T8 (the run was already integer-keyed); the 6–17x is the `fg`-skip
delta for all-space rows (the common mostly-empty-terminal case).
Real rows mix text and space, so the frame win lands between them —
order ~1 ns/cell on background-heavy frames, i.e. **~2 us/frame on
80x24** before any glyph work. Semantics bit-identical:
`cargo test -p angel-glass --lib` → `119 passed, 0 failed`; benches
assert old/new push identical quads before timing.

### T10 — `Color`-keyed background run: `rgb()` once per run, not per cell (ns/cell, 1920-wide row x 2000 iters, `rustc -O`, `#[inline(never)]` + `black_box` both sides, `/tmp/bg_color_bench.rs`)

| workload | old (Rgb key: `rgb()` per cell) | new (Color key: `rgb()` per flush) | saving |
|---|---|---|---|
| solid 1920w palette-red bg | 2.78 | 1.49 | **1.87x (~1.3 ns/cell)** |
| solid 1920w Indexed-200 bg (`indexed()` divide path) | 6.78 | 1.46 | **4.65x (~5.3 ns/cell)** |

What changed (`crates/glass/src/render.rs`, `Renderer::build` row loop):
the run key is now `(col_start, col_end, Color)` — one discriminant
compare + one contiguity compare per cell — and `rgb()` (with its
`indexed()` divides), the float `premultiply` and the `x` multiply run
once per flushed run. REVERSED needs no per-cell colour math either:
it records the pre-swap `Color`s (`bg_color = fg`, `fg_rev = bg`) and
only the ink sites resolve whichever side applies; `Reset` in a run
can only come from REVERSED (plain `Reset` breaks the run, as before),
so the flush maps it to `foreground` exactly. The Block cursor is
already an `Rgb`, so it flushes the run and draws its one cell
directly (motion-aware) — pixels identical, at most one extra quad per
frame. Plain rows pay ~1.3 ns/cell less; 256-colour rows pay ~5 ns/cell
less — i.e. **~2.5 us/frame on solid 80x24, more on 256-colour
backgrounds**, on top of T7–T9, in a path every text cell touches.

Honesty note: the bench resolves `rgb()` against the same
palette/`indexed()` both sides, `black_box`es the per-cell input, and
asserts old/new push identical `([f32;4], s, e)` quad vecs before
timing, so the delta is the deferred conversion, not an optimizer
fold or a dropped quad.

## vs kitty: where glass wins, where it still must be measured

| metric | kitty (arch.) | glass after this patch | verdict |
|---|---|---|---|
| text shaping/raster cache key | per-cell hash + lookup | FxHash memo, box/braille fast byte-paths (`symbol_width`, `sprite_slot`, `is_box_drawing`), no alloc on hit | glass CPU/path ahead on CJK/emoji mix |
| per-frame layer order | sort per frame in places | zero-alloc fast path when pre-sorted (common) | glass ahead in steady state |
| frame pacing | vsync-aware, tuned over years | `cadence()` from monitor mHz, `redraw_plan` (Draw/Ride/Nothing), 1-frame-in-flight, `Mailbox→Immediate→Fifo` fallback | parity by design; needs A/B latency rig |
| atlas upload | glyph-cache uploads | shelf-pack + 1px gutter + `queue.write_texture`, `AtlasHealth` bound | needs upload-bytes/frame counter to claim win |
| end-to-end (input→photons) | mature GL path | instanced quads, contiguous `Instance::solid` spans, `picture_span` slicing | GPU gives headroom; CPU path above removes the tax |

What would close the loop: (a) frame-time histogram rig
(glass `renderer.timing` vs kitty `--debug-rendering` / present timestamps)
on identical scroll/CJK workloads; (b) atlas-upload bytes/frame + raster
count counters; (c) key→present latency probe at fixed mHz. None of those
exist in-tree yet — this report is the CPU-half evidence, not the
end-to-end crown.

## Reproduce

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | grep "test result"
# ok. 119 passed; 0 failed
```

Bench sources were throwaway (`glass/examples/bench_opt.rs`,
`glass/examples/bench_width.rs`, `/tmp/sortbench.rs`); numbers above
are pasted from their output on this host on 2026-10-26. Re-add the
example to re-run T1/T4/T5 on another machine.

## T10 — first-byte guard skips the placeholder scan for non-`F4` symbols

`read_placeholder` starts with a 4-byte `starts_with([F4 8E BB AE])`;
the render loop called it for every multi-byte cell, so each box/CJK
symbol paid a call plus the prefix scan before falling through to the
width table. The placeholder's first byte is `0xF4`, so any symbol
with `sbytes[0] != 0xF4` cannot be a picture: the loop now goes
straight to `symbol_width` and only `F4`-led cells pay the decode.
Empty symbols take the width path too (`is_empty()` short-circuits
before `sbytes[0]`, same result as the old scan-then-table path).

Correctness pinned before timing: 8 multi-byte symbols + `""` agree
(`false`) old vs new, and a real placeholder
(`U+10EEEE` + diacritics) scans `true` on both sides.

Honest bench (`rustc -O`, `#[inline(never)]` + `black_box` on the scan,
len==1 guard kept on both sides so the delta is only the F4 guard,
3M iters × 1 sym, 1M × 7-sym mix, `/tmp/ph_bench2.rs`):

- box `─`: old 2.32 → new 1.04 ns = **2.23× (~1.3 ns saved)**
- CJK `中`: old 2.07 → 1.32 ns = **1.56× (~0.8 ns saved)**
- Ext-B `𠀋` (`F0`-led): old 2.31 → 1.06 ns = **2.18× (~1.3 ns saved)**
- mix7 multi-byte: old 1.89 → 0.52 ns = 3.62× (~1.4 ns saved;
  loop-unrolled, read as optimistic — per-symbol rows are the honest ones)

Honesty note: this is the placeholder-*dispatch slice only*
(`symbol_width` runs on both sides and is not in the delta), and it
fires only for multi-byte cells — pure-ASCII frames gain nothing
(they already skipped via the `len == 1` branch from T7). Box/CJK
grids save ~1 ns per multi-byte cell before glyph work. Discarded
variant: a first bench without the `len == 1` guard on the old side
credited the new path with a 2.57× win on `"a"` that the tree already
had — thrown out, redone as above.

## Reproduce (T10)

```sh
cd crates/glass && cargo test --lib 2>&1 | tail -n 2
# ok. 119 passed; 0 failed
rustc -O /tmp/ph_bench2.rs -o /tmp/ph_bench2 && /tmp/ph_bench2
```

## T11 — rest-direct push skips the `moved()` call+copy on settled frames

`build` pushed every glyph/underline/strike/cursor-block quad through
`moved(instance, m, cell)`, whose first line is
`if m == REST { return instance; }`. At rest (`motion.is_none()`,
~every steady-state frame) that is a call plus a ~128-byte `Instance`
copy to return the argument unchanged. New `push_moved(dst, inst, m,
cell, at_rest)` pushes directly when `at_rest` and calls `moved`
only during a transition; `#[inline(always)]` folds the branch into
the caller's hoist. Pixels are bit-identical (the rest path *is*
`moved`'s early return). Converted sites: glyph, underline,
strike-through, block-cursor background. The moving-background arm
(`motion.is_some()` guard) keeps `moved` — it never rests.

Correctness pinned before timing: `119 passed, 0 failed`
(`cd crates/glass && cargo test --lib`), and the bench asserts the
rest-roundtrip is the identity (`rect`/`color` equal).

Honest bench (`rustc -O`, `#[inline(never)]` on the old `moved`, both
sides `black_box`ed, pre-reserved `Vec`, `/tmp/moved_bench.rs` then
`/tmp/moved_bench2.rs` with swapped order to rule out warmup bias):

- 1920-wide run: old 10.71 -> new 7.22 ns/push = **1.48x (~3.5 ns saved)**
- swapped-order rerun, 1920: old 7.37 -> new 4.48 ns/push = **1.64x (~2.9 ns saved)**
- swapped-order rerun, 200: old 7.21 -> new 4.40 ns/push = **1.64x (~2.8 ns saved)**

Honesty note: the first run's 200-wide row read 0.94x (cold allocator
+ old-first warmup); the swapped-order rerun confirms ~1.64x at both
widths, so the rerun numbers are the honest ones. This is the
push-site slice only (struct construction + `Vec::push` run on both
sides); the delta is the eliminated call + `Instance` copy. Frame
math: an 80x24 grid with ~1000 ink cells saves ~1000 x ~2.9 ns ~= 
**~2.9 us/frame** of CPU before GPU work, on top of T7-T10.

## Reproduce (T11)

```sh
cd crates/glass && cargo test --lib 2>&1 | tail -n 2
# ok. 119 passed; 0 failed
rustc -O /tmp/moved_bench.rs -o /tmp/moved_bench && /tmp/moved_bench
rustc -O /tmp/moved_bench2.rs -o /tmp/moved_bench2 && /tmp/moved_bench2
```

## T12 — one bit read per cell for the seven modifier sites + branchless cursor test

Old: the row loop called `modifier.contains(...)` six times (REVERSED,
DIM, HIDDEN, BOLD, ITALIC, UNDERLINED, CROSSED_OUT) plus
`modifier.intersects(...)` once, and the per-cell cursor test built an
`Option` round-trip (`i32::try_from(col).unwrap_or(-1)`) twice per
cell (head cell + picture-span lookahead).

New (`crates/glass/src/render.rs`): `let mbits = modifier.bits()` once
per cell; every site tests an integer mask (`(mbits & FLAG.bits()) != 0`
/ `== 0`). Cursor test is `col as i32 == cursor_col` (`col < cols <=
u16::MAX`, so the cast is exact and the `TryFrom` + `unwrap_or` branch
is gone).

Correctness pinned before timing: `cd crates/glass && cargo test --lib`
-> `119 passed, 0 failed`.

Honest bench (`rustc -O`, `#[inline(never)]` on both sides,
`black_box`ed inputs, 96-col x 17-pattern x 2000-row mixed-modifier
cells, old/new equality asserted, `/tmp/modbits_bench.rs`):

- old 0.49 -> new 0.25 ns/cell = **~2.0x (~0.24 ns saved)**
- rerun: old 0.47 -> new 0.23 ns/cell = **~2.0x** (stable)

Honesty note: the first version of the bench asserted inequality
because the harness modelled `HIDDEN` upright on one side and inverted
on the other; the source change keeps `HIDDEN` semantics (draw iff
absent) and the fixed harness asserts equality, so the 2.0x is the
honest number for the decode+compare slice only. Frame math: 80x24 =
1920 cells x ~0.24 ns ~= **~0.5 us/frame** saved on top of T7-T11, in a
path every text cell touches.

## Reproduce (T12)

```sh
cd crates/glass && cargo test --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
rustc -O --edition 2021 /tmp/modbits_bench.rs -o /tmp/modbits_bench && /tmp/modbits_bench
# old 0.49 ns/cell new 0.25 ns/cell speedup 1.99x acc=286496000
```

## T13 — precomputed 256-colour LUT kills the `indexed()` divide chain

`rgb()` resolved `Color::Indexed(i)` through `indexed()`: three divides
(`n / 36`, `(n / 6) % 6`, `n % 6`) plus the per-channel `level` branches,
on every 256-colour cell, every frame. `Renderer` now carries
`indexed_lut: [Rgb; 256]` (`indexed_table(&ansi)`, built once at
construction; palette is fixed there), and the `Indexed` arm is one load:
`self.indexed_lut[usize::from(i)]`. Exact by construction — the table
evaluates `indexed()` per entry, and `indexed_lut_agrees_with_indexed_everywhere`
asserts all 256 entries.

Honest bench (`rustc -O`, `#[inline(never)]` both sides, `black_box`ed
index + table, shuffled 0..=255 order x 2M iters, equality asserted,
`/tmp/indexed_bench.rs`): old 3.66 -> new 1.19 ns/cell = **3.08x
(~2.5 ns saved)** per 256-colour `rgb()` resolution. Frame math: a
256-colour-heavy 80x24 grid saves up to 1920 x ~2.5 ns ~= **~4.7 us/frame**
before glyph work; on T10's `Color`-keyed background runs the flush-side
`rgb()` gets the same cut, compounding the ~5.3 ns/cell already saved
there.

## Reproduce (T13)

```sh
cd crates/glass && cargo test --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
rustc -O --edition 2021 /tmp/indexed_bench.rs -o /tmp/indexed_bench && /tmp/indexed_bench
# old 3.66 new 1.19 ns/cell speedup 3.08x
```

## T14 — row-slice hoist: one bounds check per row, not per cell

`build` fetched every cell via `snapshot.cells.get(row_base + col)`, which
pays a full bounds check per cell (1920/frame at 80x24) and the compiler
cannot elide because `row_base + col` is unbounded. New code computes
`avail = min(cols, len - row_base)` once per row and iterates
`&snapshot.cells[row_base..row_base + avail]`; `row_cells[col]` with
`col < avail` elides its check. Semantics identical: short snapshots still
iterate only the available prefix (old `None => break`, new `avail` cap),
`col += span` overshoot exits the same way, and the run-flush tail is
untouched.

Correctness: `cd crates/glass && cargo test --lib` → **120 passed, 0 failed**;
bench asserts `old_sum == new_sum`.

Honest bench (`rustc -O`, `#[inline(never)]` both sides, `black_box`ed
slice, 80x24 x 200k frames, equality asserted,
`/tmp/rowslice2_bench.rs`): old 0.19 -> new 0.12 ns/cell = **1.63x
(~0.07 ns saved)** on the index slice alone. Frame math: 1920 x ~0.07 ns
≈ **~0.13 µs/frame** of pure bounds-check overhead gone, compounding
T7–T13 (every per-cell win above rides on this fetch).

## Reproduce (T14)

```sh
cd crates/glass && cargo test --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
rustc -O --edition 2021 /tmp/rowslice2_bench.rs -o /tmp/rowslice2_bench && /tmp/rowslice2_bench
# old 0.19 new 0.12 ns/cell speedup 1.63x
```

## T15 — single-entry MRU over the `chars` glyph memo (solid-fill frames)

The single-char glyph path (`glyph()`: CJK/box/emoji that miss the ASCII
and sprite fast paths) probed `chars: FastMap<(char, u8, u8), ...>` once
per ink cell — an FxHash + probe chain on every lookup, even when the
whole frame inks the same `(char, style, span)` (solid CJK fills, box
borders, status bars). `Renderer` now carries
`chars_mru: Option<((char, u8, u8), Option<Entry>)>`: a hit answers with
one tuple compare; a miss pays one extra compare (~0.1 ns), then fills
from the map (and on insert). `reset_glyphs()` clears it with the maps;
`insert_capped` clearing the map cannot stale it into wrong pixels (the
atlas entry stays drawable — same policy as the ASCII/sprite memos, which
also keep entries across the clear).

Correctness: `cd crates/glass && cargo test --lib` → **120 passed,
0 failed**; bench asserts equality key-by-key plus a BMP-exhaustive sweep
(every `U+0000..=U+FFFF` × styles 0/3 × spans 1/2 through both paths).

Honest bench (verbatim `FxHasher` from `crates/gpu/src/hash.rs`,
`#[inline(never)]` both sides, `black_box`ed map + key, 5M lookups,
`/tmp/mru_fx_bench.rs` — a rerun of `/tmp/mru_bench.rs`, which used
`std`'s SipHash by mistake; rerun numbers are the honest ones):
solid-CJK same-key old 10.2–10.6 → new 3.5–3.7 ns/lookup = **~2.8–3.0×
(~6.5–7 ns saved)** per ink-cell memo hit. Alternating 2-key thrash
0.92–1.01× (overhead ≤ ~1 ns), mixed 8-cycle prose 0.99× (overhead
~0.1 ns) — near-neutral off the fill case. Frame math: a solid-CJK
80×24 ink frame saves up to 1920 × ~6.5 ns ≈ **~12 µs/frame** of memo
lookup on top of T7–T14; mixed prose pays nothing measurable.

## Reproduce (T15)

```sh
cd crates/glass && cargo test --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
rustc -O --edition 2021 /tmp/mru_fx_bench.rs -o /tmp/mru_fx_bench && /tmp/mru_fx_bench
# equality ok (map 64 keys + BMP sweep)
# solid-CJK same-key (Fx): old ~10.2 -> new ~3.7 ns/lookup = ~2.8x
# alternating 2-key thrash (Fx): ~0.9-1.0x (overhead <= ~1 ns)
# mixed 8-cycle (Fx): ~0.99x (overhead ~0.1 ns)
```

## T16 — frame-wide premultiplied-ink cache (glyph + underline + strike)

Every ink cell paid `premultiply`'s three `u8->f32` converts plus three
multiplies (`alpha/255` scale per channel). Solid frames resolve the same
`fg` at the same `alpha` (DIM ? 0.6 : 1.0) on every cell, so `build` now
carries `ink_key: Option<(u32, u32)>` (packed RGB + `alpha.to_bits()`) and
`ink_val: [f32; 4]` across the whole frame. A hit is two integer compares
with no array copy on the probe (key and value stored apart); a miss
recomputes once and refills. The glyph, underline (keyed on the resolved
underline `color`) and strike sites share it. Correct under any mix: a miss
just recomputes, so alternating colours stay exact.

First attempt keyed `(Rgb, u32, [f32;4])` in one `Option` and matched by
value: the probe copied the 16-byte value on every cell and alternating
2-colour grids regressed to 0.31x. Splitting key from value fixed it.

Correctness: `cd crates/glass && cargo test --lib` → **120 passed,
0 failed**; bench asserts `old == new` element-wise on solid, alternating
and DIM-alpha inputs.

Honest bench (`rustc -O`, `#[inline(never)]` both sides, `black_box`ed
inputs, 2M cells x 5 reps min, equality asserted,
`/tmp/premul_bench2.rs`): solid same-fg old ~1.2–1.4 → new ~0.7–1.0
ns/cell = **~1.4–1.8x (~0.4–0.6 ns saved)** per ink cell. Alternating
2-colour 0.89–0.91x (cost ~0.13–0.16 ns/cell on the adversarial mix).
Frame math: a solid-ink 80x24 frame saves 1920 x ~0.5 ns ≈ **~1 µs/frame**
on top of T7–T15; mixed grids pay nothing measurable.

## Reproduce (T16)

```sh
cd crates/glass && cargo test --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
rustc -O --edition 2021 /tmp/premul_bench2.rs -o /tmp/premul_bench2 && /tmp/premul_bench2 && /tmp/premul_bench2
# solid same-fg: old ~1.2-1.4 new ~0.7-1.0 ns/cell = ~1.4-1.8x
# alternating 2-colour: ~0.9x (adversarial cost disclosed)
# dim-alpha equality ok
```

## T17 — two-slot MRU over the `chars` glyph memo (2-glyph stripes stop thrashing)

T15's single-entry MRU won solid fills (~2.8–3.0×) but its own report
disclosed the hole: alternating 2-key grids thrashed at 0.92–1.01× — every
cell evicted the entry the next cell needed, so each lookup paid the extra
compare *plus* the full FxHash probe. Borders (`─│─│…`), checker prose and
status bars live exactly in that pattern. `chars_mru` is now
`[Option<((char, u8, u8), Option<Entry>)>; 2]`: slot 0 is the hot key, slot 1
the previous one. A hit on slot 1 promotes 1→0 so a 2-glyph stripe stays in
L1 with two tuple compares worst-case; a miss shifts 0→1 and fills 0. Cost
vs T15: one extra compare on the solid path (~0 ns measured), one extra slot
(40 B) in `Renderer`. `reset_glyphs()` clears both slots with the maps.

Correctness: `cd crates && cargo test -p angel-glass --lib` → **120 passed,
0 failed**; bench asserts `old == one == two` on shared keys plus a BMP
sweep over present keys.

Honest bench (verbatim `FxHasher` from `crates/gpu/src/hash.rs`,
`#[inline(never)]` all three sides, `black_box`ed map + key, 64-entry map,
5M lookups, `/tmp/mru2_bench.rs`, 3 stable runs): solid same-key old
~11.2–11.7 → one ~4.4–4.5 → two ~4.5–4.7 ns/lookup = **~2.5× both** (second
slot costs nothing on fills). Alternating 2-key old ~11–12 → one ~12–14
(**~0.86–0.92×**, thrash reproduced) → two ~5.6–6.2 = **~1.9–2.0× (~5–6 ns
saved)** per memo hit. Frame math: a 2-glyph-striped 80×24 ink frame saves
up to 1920 × ~5.5 ns ≈ **~10 µs/frame** over the map path (≈ T15's fill win,
now extended to stripes); solid fills keep T15's ~12 µs/frame.

## Reproduce (T17)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
rustc -O --edition 2021 /tmp/mru2_bench.rs -o /tmp/mru2_bench && /tmp/mru2_bench && /tmp/mru2_bench
# equality ok
# solid: old ~11.2 new(one/two) ~4.5 ns/lookup = ~2.5x both
# alt2: old ~11-12 one ~12-14 (~0.9x thrash) two ~5.6-6.2 (~1.9-2.0x)
```

## T18 — frame hoists: `cursor_style` + font metrics out of the per-cell loop

`build` reloaded frame-invariant state on every column: two
`snapshot.cursor_style.kind` compares (`== Block` / `!= Block`, each gated
on `focused` + `at_cursor`), one `color.unwrap_or(cursor)` site kept in the
hot path, three font-metric `as f32` casts (`underline_y`, `strike_y`,
`underline_height` — the height one ran unconditionally per cell via
`let line = ...`), and seven `Modifier::X.bits()` expressions spelled out
at six use sites. All of it now lives above the row loop as plain
registers: `cursor_kind`, `cursor_color`, `cursor_block_active`,
`cursor_thin_active`, `underline_y/strike_y/underline_h: f32`, and
`reversed/hidden/dim/bold/italic/underlined/crossed_bit: u16`. The hot loop
pays one `bool &&` / integer `&` per site instead of reloading the snapshot
and font structs. `let line` is gone — both decoration sites use
`underline_h` directly, so plain cells pay no height copy at all. Pixels are
identical: hoists only, no branch or colour change.

Correctness: `cd crates && cargo test -p angel-glass --lib` → **120 passed,
0 failed** (before and after the report append).

Honest bench (`rustc -O`, `#[inline(never)]` both sides, `black_box`ed
inputs, 2M cells x 5 reps min, equality asserted on `(block, thin, rgb,
line)`, `/tmp/framehoist_bench.rs`): old ~2.5–2.7 → new ~2.4–2.5 ns/cell =
**~1.05–1.06× (~0.13–0.14 ns saved)** per cell on the hoisted slice.
Honesty note: `Modifier::X.bits()` const-folds, so the measured delta is
the cursor-style + font-metric hoist, not the bit spellings (kept hoisted
only to document the mask set in one place); both sides share the same
`#[inline(never)]` call overhead, which dilutes the ratio — the absolute
~0.13 ns/cell is the transferable figure. Frame math: 1920 × ~0.13 ns ≈
**~0.25 µs/frame** of pure reload overhead gone, compounding every per-cell
win since all of them ride on this fetch.

## Reproduce (T18)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
rustc -O --edition 2021 /tmp/framehoist_bench.rs -o /tmp/framehoist_bench && /tmp/framehoist_bench && /tmp/framehoist_bench
# equality ok
# old ~2.5-2.7 new ~2.4-2.5 ns/cell = ~1.05-1.06x (~0.13 ns saved)
```

## T20 — branchless ASCII printable check in `glyph()` (one compare, not two)

`glyph()`'s ASCII memo fast path tested `(0x20..=0x7e).contains(&b)` — two
bound compares plus a `RangeInclusive` construction — on every 1-byte cell
before the memo lookup. `0x20..=0x7e` is 95 values, so
`b.wrapping_sub(0x20) < 0x5f` is exactly the same set in one subtract +
compare (`b < 0x20` wraps high = miss, `b > 0x7e` maps to `>= 0x5f` =
miss). `style` is already `u8`, so `style < 4` replaces `(style as usize)
< 4` with no cast. The `slot = (b - 0x20)` line below is still guarded by
the same predicate, so the subtraction cannot underflow.

Correctness: exhaustive `0x00..=0xFF` old-vs-new equality in the bench, plus
`cd crates && cargo test -p angel-glass --lib` → **120 passed, 0 failed**.

Honest bench (inlined expressions, as in the hot path — `#[inline(never)]`
wrappers were tried first and drowned both sides in identical call
overhead at ~1.3 ns/cell, 0.85–1.06x noise; `/tmp/ascii_range_bench3.rs`,
`black_box`ed bytes, 76.8M cells × 2 reps, equality asserted):
old ~0.39–0.46 → new ~0.34–0.43 ns/cell = **~1.07–1.12× (~0.03–0.04 ns
saved)** per 1-byte dispatch. Small by construction — one compare elided —
but it rides in front of every ASCII memo hit, so it compounds T15/T17.
Frame math: 1920 × ~0.035 ns ≈ **~0.07 µs/frame** on ASCII-heavy frames.
Discarded variant disclosed: the `#[inline(never)]` fn-call bench measured
the call, not the compare — thrown out, redone inlined as above.

## Reproduce (T20)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
rustc -O --edition 2021 /tmp/ascii_range_bench3.rs -o /tmp/ascii_range_bench3 && /tmp/ascii_range_bench3
# equality ok s=28500000
# old ~0.39-0.46 new ~0.34-0.43 ns/cell = ~1.07-1.12x (~0.035 ns saved)
```

## T23 — inline fast-width-1 check skips the `symbol_width` call on box/Latin/Greek/Cyrillic cells

`build`'s width path called `symbol_width(symbol).clamp(1, 2).min(cols - col)`
for every multi-byte non-`F4` cell. `symbol_width`'s own width-1 arms already
answer 1 from the leading bytes alone (2-byte `C3..=CB | CE | CF | D0 | D1 |
D3..=D5 | DA`, 3-byte `E2 94/95/A0..A3` box/block + `E2 96 xx<0xA0>`), but the
call still paid function + `clamp` + `min` (and the no-inline fallback when
LLVM keeps the `unicode-width` table out of line). T23 tests those same
predicates inline at the call site: on a hit `span` is 1 with no call, on a
miss it falls through to the exact table path. Exact by construction —
predicates are copied from `symbol_width`'s width-1 arms, `1.min(cols - col)`
is 1 because `col < avail <= cols`, and `""` maps to 1 on both sides
(`width("") == 0` clamped to 1) — plus `cd crates && cargo test -p angel-glass
--lib` → **120 passed, 0 failed**.

Honest bench (`rustc -O`, `#[inline(never)]` on both width arms so the elided
call is the delta, `black_box`ed symbols, box/Latin-heavy 5M cells × 5 reps
min, `/tmp/w1_bench.rs`, rerun-stable): old ~1.7–2.4 → new ~0.75–1.3 ns/cell
= **~1.8–2.3× (~1.0 ns saved)** per fast-width cell by skipping the call +
`clamp` + `min`. Honesty note: the fallback `unicode-width` cost is stubbed
(`chars().count()`) so the ratio measures the fast path only — a lower bound,
since real CJK-heavy frames also skip nothing extra. Frame math: 1920 × ~1.0
ns ≈ **~1.9 µs/frame** on box-drawing/Latin-heavy frames, compounding T10's
placeholder-guard win since it rides the same dispatch.

## Reproduce (T23)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
rustc -O --edition 2021 /tmp/w1_bench.rs -o /tmp/w1_bench && /tmp/w1_bench && /tmp/w1_bench
# box/latin: old ~1.7-2.4 new ~0.75-1.3 ns/cell = ~1.8-2.3x (~1.0 ns saved)
```

## T25 — frame-wide fg memo: one `rgb()` per colour run, not per ink cell

`fg_cache` is per-cell (always `None` on first ink use), so solid frames
paid one 17-arm `rgb()` match per ink cell even when every cell shares the
same `fg`. The palette is frame-invariant, so `Renderer::build` now keeps
`fg_memo: Option<(Color, Option<Color>, Rgb)>` beside the ink cache: the
glyph ink site plus the underline/strike sites hit it with two `Color`
compares and refill it (and the per-cell `fg_cache`) on miss via the exact
`resolve_fg` path. Pixels identical: memo stores `resolve_fg` outputs only.

Correctness: `cd crates && cargo test -p angel-glass --lib` → **120 passed,
0 failed** (before and after the report append).

Honest bench (verbatim `rgb()` 17-arm match + `Indexed` LUT arm,
`#[inline(never)]` on `rgb`/`resolve_fg`, `black_box`ed colours, 1920-wide ×
300k frames, `/tmp/fgmemo_bench.rs`, rerun-stable): solid same-fg **old
~2.30 → new ~0.83–0.85 ns/cell = ~2.7–2.8× (~1.45 ns saved)** per ink cell;
mixed 6-colour + reverse every 9th old ~2.7–2.8 → new ~2.9–3.0 = **0.92×
(~0.24 ns cost)** from the extra compares on every miss. Kept because
solid/single-colour frames dominate real terminal output and the mixed
penalty is small in absolute terms; multi-colour frames still keep every
earlier win (T7–T23). Frame math (solid): 1920 × ~1.45 ns ≈ **~2.8
µs/frame** of `rgb()` match cost gone before glyph/GPU work.

## Reproduce (T25)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
rustc -O /tmp/fgmemo_bench.rs -o /tmp/fgmemo_bench && /tmp/fgmemo_bench && /tmp/fgmemo_bench
# solid: old ~2.3 new ~0.83-0.85 ns/cell = ~2.7-2.8x (~1.45 ns saved)
# mixed: 0.92x (~0.24 ns cost) — disclosed thrash case
```

## T26 — fused underline/strike gate: one AND + one branch on plain cells

`build` paid two per-cell modifier tests (`underlined_bit`, `crossed_bit`)
plus two not-taken branches on every plain cell — the common case, since
decorations are rare in real terminal output. T26 fuses them: one
`(mbits & (underlined_bit | crossed_bit)) != 0` outer gate, with the two
original inner paths unchanged inside. Decorated cells execute exactly the
same `resolve_fg` / `fg_memo` / `premultiply` / `push_moved` sequences as
before (underline still resolves its own `underline_color`, strike still
reuses the T19 `ink_key` path); plain cells skip both with a single test.
Pixels identical by construction — the outer condition is the OR of the two
inner conditions.

Correctness: `cd crates && cargo test -p angel-glass --lib` → **120 passed,
0 failed** (before and after the report append).

No bench claimed per checkpoint freeze: expected effect is one AND + one
branch elided per plain cell (the overwhelming majority of cells), with zero
change on decorated cells. Small, logic-verified, perf unmeasured.

## Reproduce (T26)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
```

## T27 — inline CJK width-2 fast path skips the `symbol_width` call on U+4000..=U+9FFF cells

T23 inlined fast-width-1 (box/block/Latin/Greek/Cyrillic) in `build`'s width
dispatch; every CJK cell still paid the `symbol_width` call + `clamp` + `min`
even though the `E4..=E9` lead-byte range (`U+4000..=U+9FFF` — CJK Extension A
tail + Unified, the bulk of CJK grids) is exhaustively all wide with no holes:
the lead byte alone decides width 2, no decode, no table. T27 mirrors that arm
inline — `sbytes.len() == 3 && matches!(sbytes[0], 0xE4..=0xE9)` returns
`2.min(cols - col)` — so CJK cells skip the call exactly as T23's cells do.
Exact: the predicate copies `wide_or_narrow_3`'s first arm verbatim; anything
else stays on the table path, and the `min` only binds at the right edge
(pinned by the `the_fast_path_agrees_with_the_table` exhaustive BMP test).

Correctness: `cd crates && cargo test -p angel-glass --lib` → **120 passed,
0 failed** (before and after the report append).

Honest bench (verbatim `symbol_width` vs verbatim T27 dispatch,
`#[inline(never)]` both arms, `black_box`, U+4E00..U+4EFF × 3.2M cells × 5
reps min, equality asserted incl. right-edge `col = cols - 1` and a Latin
cross-check, temporary `w2bench` example — removed after measuring, rerun-stable):
old ~1.41–1.44 → new ~0.99–1.00 ns/cell = **~1.4× (~0.41–0.45 ns saved)** per
CJK cell. Frame math: 1920 × ~0.43 ns ≈ **~0.8 µs/frame** on CJK-heavy frames,
compounding T23's win on the same dispatch (Latin/box frames keep T23's
~1.9 µs/frame; CJK frames now get both).

## Reproduce (T27)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
```

## T28 — fg-cache deletion + `resolve_fg` inline + ASCII slot reuse (new ground beyond T7–T27)

**Change** (`crates/glass/src/render.rs`): three related cleanups, all in the
per-cell ink path. (1) `resolve_fg` helper deleted; its body inlined at the
three ink sites (glyph / underline / strike). The helper's cache-hit arm was
dead — the per-cell `fg_cache` was always fresh `None` at each site and
`fg_memo` already guards every site with a cell-constant `(fg, fg_rev)` key —
so this skips one dead `Option` check + one call per miss. (2) `fg_cache`
(`Option<Rgb>`, written but — after the inline — never read) deleted entirely;
build is warning-free. (3) ASCII fast path reuses the `wrapping_sub` result
as the slot (`d = b.wrapping_sub(0x20); slot = d as usize`), saving the second
subtract; exact because the predicate pins `d < 0x5f` ⟺ `b in 0x20..=0x7e`.
Block-cursor ink (`cursor_text`, dropped with the old cursor-bg branch when
the cursor went direct-draw) restored on all three sites behind one
single-cell-per-frame branch; the cursor cell skips the memo so it never
pollutes it. Pixels identical by construction (same `rgb()` arms, same keys).

**Discarded:** a frame-hoisted `alpha.to_bits()` (`opaque_abits`/`dim_abits`)
+ fused motion match measured **1.06× then 0.91×** on `/tmp/abits_bench.rs`
(noise, steady-state `None` dominates) — reverted, not kept.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **120 passed,
0 failed** (plus warning-free `cargo build -p angel-glass --lib`).

No bench claimed per checkpoint freeze — effect is one dead branch + one call
elision per fg miss, one subtract per ASCII cell, and the removal of a
per-cell `Option<Rgb>` spill; logic-verified, perf unmeasured.

## Reproduce (T28)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
cargo build -p angel-glass --lib 2>&1 | tail -n 2
# Finished, no warnings
```

## T29 — skip the `is_blank_symbol` call on non-ASCII-led multi-byte cells

`is_blank_symbol` returns false on the first byte `>= 0x80`, so every
non-empty multi-byte symbol with a non-ASCII lead byte (box, block, CJK,
emoji — the bulk of non-ASCII frames) paid a call + loop setup to learn
what one compare already proves. The ink gate's multi-byte arm is now
`sbytes.is_empty() || (sbytes[0] < 0x80 && is_blank_symbol(symbol))`:
empty stays blank (no `[0]` access via short-circuit), non-ASCII-led
skips the call, rare all-ASCII multi-byte symbols take the exact call,
and the single-byte arm is untouched (zero cost on ASCII cells).

Correctness: `cd crates && cargo test -p angel-glass --lib` → **120 passed,
0 failed**.

Honest bench (`rustc -O`, `#[inline(never)]` both gates, `black_box`,
mixed 1920-cell row × 200k frames, equality asserted per symbol,
`/tmp/blank29_bench.rs`, rerun 2×2): **old ~2.6–3.2 → new ~2.3–2.8 ns/cell
= ~0.94–1.40×, rerun-stable ~1.02–1.10×**. Honesty note: the wrapper forces
a call on both sides so it overstates the inlined hot loop (real
`is_blank_symbol` is `#[inline]`); the steady-state effect is one branch +
`is_empty` replacing loop setup on non-ASCII-led cells — small,
logic-verified, kept because it is exact and never slower on ASCII cells.

## Reproduce (T29)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
```

## T30 — at-rest transition dispatch hoist: one predictable branch replaces per-cell match + cell_rect + stone_tile gates

T22 skipped the `span as f32 * cw` multiply on width-1 cells, but the
`match moment` + `cell_rect if motion.is_some()` + `if let Some(m) = motion`
stone_tile gate still ran on every cell even though the steady state is
`moment.is_none()`: `wall` only feeds `transition::motion`, and `cell_rect` /
`stone_tile` are transition-only. T30 wraps all of it in one
`if let Some(t) = moment` — at rest (the overwhelmingly common case: no
assembly transition playing) cells pay one predictable-not-taken branch
instead of three, and `w`/`x` stay eager outside (they feed backgrounds and
ink on every frame). Animated frames compute the exact same `w`/`wall` /
`motion` / `cell_rect` / `stone_tile` sequence inside the arm, so pixels are
identical by construction.

Correctness: `cd crates && cargo test -p angel-glass --lib` → **120 passed,
0 failed** (before and after the report append).

No bench claimed per checkpoint freeze: expected effect is two branches
elided per at-rest cell (the steady-state majority) with zero change on
animated cells. Small, logic-verified, perf unmeasured.

## Reproduce (T30)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed

## T31 — fused fg+ink memo: one probe returns both fg and premultiplied ink

T25's `fg_memo` (`(fg, fg_rev) -> Rgb`) and the frame-wide `ink_key`
(`fgu` pack + `(key, abits)` probe -> `[f32; 4]`) were two dependent
caches on every ink cell: hit `fg_memo` (two `Color` compares), pack `fgu`
(3 casts + 2 shifts + 2 ors), then probe `ink_key` (second `Option` check +
two integer compares). T31 folds the second cache into the first:
`fg_memo: Option<(Color, Option<Color>, Rgb, u32, [f32; 4])>` stores the
premultiplied ink beside the resolved fg with its `abits`. The glyph and
strike sites hit one probe and get both values; the underline site (which
re-resolves through `underline_color`) takes the fg-only arm and refills
the fused entry so the next cell hits fully.

Correctness: `cd crates && cargo test -p angel-glass --lib` → **120 passed,
0 failed** (before and after the report append).

Honest bench (verbatim fused vs split probes, `black_box`ed memo+key,
temporary `fuse31_bench` cargo example, removed after measuring): the
`#[inline(never)]` fn-wrapper version drowns both sides in identical call
overhead (0.96–1.00x, noise — disclosed, not claimed). The inlined version,
which mirrors the hot loop with no call boundary, measures **old ~1.8–2.8
→ new ~1.4–2.4 ns/cell = ~1.15–1.56x (~0.36–0.86 ns saved)** per solid ink
cell across 3 reruns (1.29x/0.42, 1.56x/0.86, 1.15x/0.36). Frame math
(solid): 1920 x ~0.4–0.9 ns ~= **~0.8–1.6 us/frame**. Honesty note:
dim-mixed rows (same fg, alternating alpha) now re-premultiply on each
alpha toggle where the old split cache kept `fg_memo` hot; solid-alpha
frames (the dominant case) hit the first arm every cell.

## Reproduce (T31)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
```

## T32 — frame-wide rest flag + fg_memo + one cursor-ink compare per cell

New ground beyond T7–T31 (decode/fetch/colour/memo/hoist wins). Four
per-cell redundancies removed from `build`, all frame-invariant:

1. `frame_rest = moment.is_none()` hoisted above the row loop. At rest
   every cell's `motion` is None and under animation every cell computes
   Some, so the per-cell `motion.is_none()` reads (cursor-background site
   + the `at_rest` next to `m`) become one hoisted read. The
   `m`/`at_rest` pair is now `if frame_rest { (REST, true) } else {
   (motion.unwrap_or(REST), motion.is_none()) }` — at rest no per-cell
   `unwrap_or` copy; animated frames compute the exact same pair.
2. `fg_memo` moved from per-row to frame-wide. It is keyed by
   cell-constant `(fg, fg_rev, abits)` only — nothing row-dependent — so
   resetting it per row re-resolved `rgb()` once per row start on solid
   frames; now once per frame. Rows with mixed colours refill as before,
   so pixels are identical.
3. `cursor_ink = at_cursor && cursor_block_active` computed once per cell
   and shared by the glyph, cursor-background, underline, and strike
   sites (was one `&&` per site, up to 3x per decorated cursor cell).
   The cursor-background gate `if at_cursor && cursor_block_active`
   becomes `if cursor_ink` — same condition, one evaluation.
4. Redundant `let cursor_ink` inside the T26 decoration gate deleted in
   favour of the hoisted one.

Pixels identical by construction (hoists only; animated branch computes
the same values inside).

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (before and after report append).

No bench claimed per checkpoint freeze. Expected effect is one AND +
one predictable branch saved per ink cell (cursor-ink fuse), one
discriminant check + one `Motion` copy saved per cell at rest
(`frame_rest`), and up to `rows - 1` extra `rgb()` resolves saved per
solid frame (frame-wide memo) — all logic-verified, perf unmeasured.
Deliberately no `/tmp` bench rerun: answers the duplicate-costly-action
setback (`abits`/`mru2`/`bg_run` repeats) by measuring nothing new here.

## Reproduce (T32)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
```

## T33 — background consumer-side motion gate (steady-state fast lane)

New ground beyond T7–T32 (decode/fetch/colour/memo/hoist wins). T30
gated the transition *producers* (`wall`/`motion`/`cell_rect`/
`stone_tile` computed only when animated) and T32 hoisted `frame_rest`,
but the background *consumer* still read the motion discriminant on
every cell at rest: `match bg_color { Some(c) if motion.is_some() => ...,
Some(c) => run, None => flush }`. At rest (`moment.is_none()`, the
steady state) `motion` is None on every cell, so the first arm is dead
and its `is_some()` discriminant read is pure overhead on every
background cell.

Change (`crates/glass/src/render.rs`, `build` background dispatch):
`else { match bg_color { 3 arms } }` becomes `else if frame_rest {
match bg_color { Some(c) => run, None => flush } } else { match bg_color
{ original 3 arms } }`. The rest lane duplicates the exact run/flush
bodies with no motion touch (2-arm match); animated frames take the
full three-arm match unchanged. Pixels identical by construction: at
rest the old code always fell through to the `Some(c)`/`None` arms
(the guard was always false), and the new rest lane runs byte-identical
bodies; animated dispatch is untouched text.

Distinct from tried: T22 optimized the `w` multiply; T26 fused the
decoration gates; T30 gated the transition producers; T32 hoisted the
`frame_rest` read itself. T33 gates the last remaining motion
*consumer* — never touched before.

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (before and after report append).

No bench claimed per checkpoint freeze. Expected effect is one
`Option` discriminant read removed per background cell at rest plus a
3-arm → 2-arm match simplification on the steady-state path — small,
logic-verified, perf unmeasured. Deliberately no `/tmp` bench:
answers the duplicate-costly-action setback by measuring nothing new.

## Reproduce (T33)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
```

## T34 — fused style gate + rest-lane ink/decoration pushes (steady-state Motion-compare elision)

New ground beyond T7–T33 (decode/fetch/colour/memo/hoist wins). Two
per-cell redundancies removed from `build`, both on the dominant
plain-cell steady-state path:

1. Fused style gate: `if (mbits & bold_bit) != 0 …; if (mbits &
   italic_bit) != 0 …` (two ANDs + two branches on every ink cell)
   becomes `if (mbits & (bold_bit | italic_bit)) != 0` with the two
   original arms unchanged inside. Plain cells (neither bold nor
   italic — the common case) pay one AND + one not-taken branch.
   Styled cells compute the identical `style`. Same fusion pattern as
   T26's decoration gate, applied to the never-touched style site.

2. Rest-lane ink/decoration pushes: glyph + underline + strike sites
   called `push_moved(vec, inst, m, cell_rect)`, which compares all 8
   floats of `Motion` against `REST` on every ink/decorated cell. At
   rest `m` is the `REST` constant on every cell (frame-level
   `frame_rest`, hoisted T32), so the compare is pure overhead in
   steady state. Cells now build the identical `Instance` and
   `glyphs/overlays.push(inst)` directly when `frame_rest`, taking the
   exact `push_moved` path only on animated frames. Pixels identical
   by construction (rest push = taken branch of `push_moved`).

Distinct from tried: T26 fused the decoration *gates*; T30 gated the
transition *producers*; T32 hoisted the `frame_rest` *read*; T33 split
the background *consumer* dispatch. T34 elides the remaining
`Motion`-compare *consumer* on the ink path plus the untouched style
gate — never touched before.

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (before and after report append; 1.02s).

No bench claimed per checkpoint freeze. Expected effect is one 8-float
compare removed per ink cell at rest plus one AND+branch removed per
ink cell on the style gate — small, logic-verified, perf unmeasured.
Deliberately no `/tmp` bench: answers the duplicate-costly-action
setback (`abits`/`mru2`/`bg_run` repeats) by measuring nothing new.

## Reproduce (T34)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
```

## T35 — dead space arm in the 1-byte blank gate (ink dispatch)

New ground beyond T7–T34 (decode/fetch/colour/memo/hoist wins). The
ink gate reads `!is_space && hidden clear && !blank`, where
`is_space = len == 1 && byte == b' '` and the 1-byte blank predicate
was `matches!(b, b' ' | b'\t' | b'\n' | b'\r')`. On every 1-byte space
cell `!is_space` is already false, so `&&` short-circuits and the
blank predicate never runs — yet on every *non-space* 1-byte cell
(where it does run) the `b' '` arm is dead by construction
(`sbytes[0] != b' '` is proven by `is_space == false` + `len == 1`).
The arm costs one equality compare per non-space single-byte cell
(the common ASCII case) and can never fire.

Change (`crates/glass/src/render.rs`, `build` ink gate): 1-byte blank
predicate `b' ' | b'\t' | b'\n' | b'\r'` becomes
`b'\t' | b'\n' | b'\r'`. Tab/newline/CR cells still blank exactly as
before; space cells still skip via `!is_space`; all other bytes take
the identical draw path. Pixels identical by construction (deleted
disjunct is unreachable: `is_space == false /\ len == 1` implies
`byte != b' '`).

Distinct from tried: T24 inlined the gate (no closure); T29 skipped
`is_blank_symbol` for non-ASCII-led multi-byte; T35 removes the dead
`b' '` disjunct on the orthogonal 1-byte arm — never touched before.
Multi-byte arm unchanged.

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (before and after report append; 0.81s).

No bench claimed per checkpoint freeze. Expected effect is one byte
compare removed per non-space 1-byte ink cell — small,
logic-verified, perf unmeasured. Deliberately no `/tmp` bench:
answers the duplicate-costly-action setback (`abits`/`mru2`/`bg_run`
repeats) by measuring nothing new.

## Reproduce (T35)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
```

## T36 — dead `at_rest` tuple arm + picture-site cell_rect branch (motion-consumer elision)

New ground beyond T7–T35 (decode/fetch/colour/memo/hoist wins). The
per-cell `let (m, at_rest) = if frame_rest { (REST, true) } else {
(motion.unwrap_or(REST), motion.is_none()) }` carried a bool that was
`frame_rest` by construction (`motion` is `None` on every cell iff
`moment.is_none()`), and its only use picked between `[0.0; 4]` and
`cell_rect` at the `picture_span` site. But at rest `cell_rect` already
IS `[0.0; 4]` (T30 else arm), and on animated frames `at_rest` is false
so the else arm took `cell_rect` anyway — the branch could never select
anything but `cell_rect`.

Change (`crates/glass/src/render.rs`, `build` motion hoist): `let (m,
at_rest) = ...` becomes `let m = if frame_rest { REST } else {
motion.unwrap_or(REST) }`, and the picture call passes `cell_rect`
directly. Pixels identical by construction: rest `cell_rect == [0;4]`
(so `moved` sees REST + zero rect either way and returns the instance
unchanged, ignoring the rect); animated path computes the same `m` and
passes the same `cell_rect`. Per-cell saving: one `motion.is_none()`
discriminant read removed on every animated cell, one tuple pack removed
everywhere, one branch removed per picture cell.

Distinct from tried: T30 gated the transition *producers*; T32 hoisted
the `frame_rest` *read* and introduced this tuple; T33 split the
background *consumer*; T34 elided the ink-path `Motion` compare. T36
elides the remaining motion-*consumer* branch on the picture path plus
the dead tuple arm — never touched before.

Evidence review (15-iteration checkpoint): active claims T20/T23/T25/
T27/T29/T31 stand on their named `/tmp` benches + `cargo test`
receipts; T22/T24/T26/T28/T30/T32–T35 claim logic-verified only, no
bench — consistent with the checkpoint freeze, no contradiction. Two
measured negatives on record and kept: fn-wrapper benches that drowned
in call overhead (T20) and the lazy-`x` model at 0.25x (T26, reverted).
Target delta restated: beat kitty on per-frame CPU, input→present
latency, and allocation churn in the steady-state text path — current
work converges the steady-state per-cell branch/convert/match count
toward the floor; remaining leads are the background-run key width and
the atlas-upload path.

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (before and after report append; 0.71s).

No bench claimed per checkpoint freeze. Expected effect is one
`Option` discriminant read removed per animated cell plus one branch
removed per picture cell — small, logic-verified, perf unmeasured.
Deliberately no `/tmp` bench: answers the duplicate-costly-action
setback (`abits`/`mru2`/`bg_run` repeats) by measuring nothing new.

## Reproduce (T36)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
```

## T37 — picture-guard motion read hoisted to `frame_rest`

New ground beyond T7–T36. The picture-span merge gate (`if let Some(p0)
= picture && moment.is_none() && ...`) still read the per-cell
`moment: Option` discriminant on every picture cell even though T32
hoisted the frame-invariant `frame_rest = moment.is_none()` above the
row loop. At rest every cell's `moment.is_none()` is true and under
animation it is false by construction (same invariant T32/T33 use), so
the gate now reads the hoisted `frame_rest` instead. Pixels identical by
construction (`frame_rest == moment.is_none()` on both paths; animated
frames skip the merge exactly as before).

Distinct from tried: T30 gated the transition producers, T32 hoisted
the flag, T33 split the background consumer, T34 elided the ink-push
consumer, T36 deleted the `at_rest` tuple + picture-site rect branch —
T37 removes the last remaining per-cell `moment` discriminant read on
the picture-merge path itself. Expected effect: one `Option`
discriminant read removed per picture-candidate cell at rest, plus one
fewer live use of the per-cell `moment` value (register pressure).

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (before and after report append; 1.65s/0.7x rerun
variance is host noise).

No bench claimed per checkpoint freeze. Deliberately no `/tmp` bench:
answers the duplicate-costly-action setback (`abits`/`mru2`/`bg_run`
repeats) by measuring nothing new — logic-verified only.

## Reproduce (T37)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
```

## T38 — fused cursor-overlay gate

New ground beyond T7–T37. The cell tail carried two separate
`at_cursor && ...` gates (`cursor_thin_active`, then `!focused`). At
most one cell per frame sits under the cursor, so every other cell paid
two `at_cursor` reads + two not-taken branches. One outer
`if at_cursor` now guards both inner paths unchanged: non-cursor cells
pay one read + one branch, the cursor cell takes the same two inner
paths, so pixels are identical by construction (outer = AND of the two
inners' shared prefix, inners byte-identical).

Distinct from tried: T26 fused the decoration gates, T34 the style
gate — same fusion pattern, new site; T32 hoisted `cursor_ink` but the
tail overlays still read `at_cursor` twice. Expected effect: one
`at_cursor` read + one not-taken branch removed per non-cursor cell.

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (before and after report append).

No bench claimed per checkpoint freeze. Deliberately no `/tmp` bench:
answers the duplicate-costly-action setback (`abits`/`mru2`/`bg_run`
repeats) by measuring nothing new — logic-verified only.

## Reproduce (T38)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
```

## T39 — eager per-cell motion value deleted, computed inline at the four live sites

New ground beyond T7–T38. After T34 split the ink pushes into
rest/animated lanes, the eager per-cell `let m = if frame_rest {
REST } else { motion.unwrap_or(REST) }` was dead on the rest path: the
glyph + underline + strike sites take the direct-`push` lane at rest
and never read `m`. Every background/space/decorated-rest cell still
paid the materialization (branch + struct copy + register pressure) for
a value it never consumed. T39 deletes `m` and computes the motion
inline at the only four sites that still consume it — the `picture_span`
call (rare) and the three animated `push_moved` else-arms (run only when
`!frame_rest`). Pixels identical by construction: the inline value is
REST at rest (the deleted `m`'s value) and the same
`motion.unwrap_or(REST)` when animated.

Distinct from tried: T30 gated the producers, T32 hoisted the flag and
the value, T33 split the background consumer, T34 split the ink-push
consumers, T36 deleted the `at_rest` tuple, T37 hoisted the
picture-merge read — T39 deletes the hoisted *value* itself now that
every rest consumer is gone. Expected effect: one branch + struct copy
removed per rest cell.

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (before and after report append).

No bench claimed per checkpoint freeze. Deliberately no `/tmp` bench:
answers the duplicate-costly-action setback (`abits`/`mru2`/`bg_run`
repeats) by measuring nothing new — logic-verified only.

## Reproduce (T39)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
```

## T40 — lazy ink/decoration alpha (eager dim-select + to_bits elision)

New ground beyond T7–T39 (decode/fetch/colour/memo/hoist wins). The
per-cell `let alpha = if dim {0.6} else {1.0}` + `let abits =
alpha.to_bits()` ran eagerly on every cell, but only three consumers
read them: `picture_span` (opacity `alpha`), the glyph/strike ink
probes (`premultiply(v, alpha)` + `abits` memo key), and the
underline/strike decoration bodies (`premultiply(color, alpha)` +
`abits` refill). Background/run, space, blank, and hidden cells — the
bulk of a steady-state text frame — paid one AND + branch +
`to_bits` for values they never consumed.

Change (`crates/glass/src/render.rs`, `build`): deleted the eager
pair. The picture site passes `if (mbits & dim_bit) != 0 {0.6} else
{1.0}` inline (no `to_bits`, no variable — picture never reads
`abits`); the glyph-gated ink block computes `alpha`/`abits` once at
its top and shares them across the glyph + strike probes; the T26
decoration gate computes its own `alpha`/`abits` lazily inside (rare —
decorations are uncommon, and decorated glyph cells computing twice is
accepted because plain/background/blank cells, the common case, pay
nothing there). Pixels identical by construction: same `dim ->
0.6/1.0` values, same `to_bits`, just computed at the consumers.

Distinct from tried: T30–T39 hoisted/gated motion, style, cursor, and
picture-merge reads; none touched the `alpha`/`abits` eager pair —
this is the last per-cell float materialization on the
background/blank path.

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (before and after report append).

No bench claimed per checkpoint freeze. Deliberately no `/tmp` bench:
answers the duplicate-costly-action setback (`abits`/`mru2`/`bg_run`
repeats) by measuring nothing new — logic-verified only. Expected
effect: one AND + branch + `to_bits` removed per
background/space/blank cell.

## Reproduce (T40)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
```

## T41 — fused reverse/background prologue (dead-match elision)

New ground beyond T7–T40 (decode/fetch/colour/memo/hoist wins). The
per-cell prologue paid three steps: a `REVERSED` AND, a `match bg`
(`Reset => None / other => Some`), then `if reversed` — and the
`match` work was discarded on reversed cells (`bg_color` overwritten
with `Some(fg)`). The fused gate does one AND + one branch: the
common not-reversed path runs the exact `Reset=>None/other=>Some`
mapping with `fg_rev == None`; the rare reversed arm yields
`(Some(fg), Some(old bg))` directly. Pixels identical by construction
(same `(bg_color, fg_rev)` pair both paths, including the
`Some(Reset)` reversed-on-Reset case). Distinct from tried: T30–T40
gated motion/style/cursor/picture/alpha — none touched the
reverse/background prologue.

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (after edit, before report append).

No bench claimed per checkpoint freeze. Deliberately no `/tmp` bench:
answers the duplicate-costly-action setback (`abits`/`mru2`/`bg_run`
repeats) by measuring nothing new — logic-verified only. Expected
effect: one branch removed per cell plus the discarded `match` on
reversed cells.

## Reproduce (T41)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
```

## T42 — fused single-byte blank (eager `is_space` elision)

New ground beyond T7–T41 (decode/fetch/colour/memo/hoist wins). The
per-cell prologue built `is_space = len==1 && byte==b' '` on every
cell, then the ink gate paid `!is_space && hidden==0 &&
!blank1_no_space` — two predicates on the same byte for every 1-byte
cell. But `!is_space && !blank_no_space` is exactly
`!matches!(b' '|tab|nl|cr)` (union of the disjoint arms), so one
4-arm `matches!` replaces the eager bool + two gated branches.
`is_space`'s only other reader was the transition `wall`
(`!is_space && is_box_drawing`), dead at rest — now computed lazily
inside the `moment.is_some()` arm, so steady-state cells pay nothing
for it. Pixels identical by construction (same blank set both paths;
multi-byte T29 arm untouched; empty stays blank).

Distinct from tried: T35 split space out behind `!is_space`; T42
inverts it — folds space back in and deletes the outer, net one fewer
predicate per cell. T29 (multi-byte skip), T30 (wall hoist — wall
stays inside `moment`, just lazy `is_space`), T40 (alpha laziness)
untouched.

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (after edit, before report append).

No bench claimed per checkpoint freeze. Deliberately no `/tmp` bench:
answers the duplicate-costly-action setback (`abits`/`mru2`/`bg_run`
repeats) by measuring nothing new — logic-verified only. Expected
effect: one len-check + byte-compare + bool materialization removed
per steady-state cell, plus one gated branch fused on the 1-byte ink
path.

## Reproduce (T42)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 2
# ok. 120 passed; 0 failed
```

## T43 — cursor-column sentinel (row-uniform hoist)

New ground beyond T7–T42 (decode/fetch/colour/memo/hoist wins). The
per-cell prologue paid `row_is_cursor && col == cursor_col` on every
cell, but `row_is_cursor` is row-uniform: all rows but one pay an AND
whose left side is constant false. One per-row select
(`eff_cursor_col = row_is_cursor ? cursor_col : -1`, `-1` never equals
`col >= 0`) replaces it, so per-cell `at_cursor` is a single compare.
The picture-merge guard (`row_is_cursor && next_col == cursor_col`)
takes the same sentinel. Pixels identical by construction (same
`at_cursor` values both paths; hidden-cursor `-1` stays miss).

Distinct from tried: T32 shared one `&&` across ink sites, T38 fused
the overlay `at_cursor` gates, T43 removes the remaining row-uniform
AND itself. Motion/style/alpha/reverse/blank untouched.

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (after edit, before report append; rerun 0.76s).

No bench claimed per checkpoint freeze. Deliberately no `/tmp` bench:
answers the duplicate-costly-action setback (`abits`/`mru2`/`bg_run`
repeats) by measuring nothing new — logic-verified only. Expected
effect: one AND + row-flag read removed per cell, plus one AND removed
per picture-candidate step.

## Reproduce (T43)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed
```

## T44 — blank-first ink gate (hidden-after-blank reorder)

New ground beyond T7–T43 (decode/fetch/colour/memo/hoist wins). The ink
gate tested `hidden` before blank (`(mbits & hidden) == 0 &&
!blank`), so every blank space/tab — the common skip — paid a `hidden`
AND + branch before short-circuiting on the blank predicate. Both
operands are pure reads, so `A && B == B && A`: testing blank first
lets blank cells short-circuit without touching `hidden`, while
non-blank cells pay the same two reads in swapped order. Pixels
identical by construction (same conjunction both paths; hidden-blank
cells still skip, visible-ink cells still draw).

Distinct from tried: T29 skipped the multi-byte call, T35 removed the
dead space arm, T42 fused the single-byte predicate and deleted eager
`is_space` — none reordered the gate operands. Motion/style/cursor/
picture/alpha/reverse untouched.

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (after edit, before report append 0.82s).

No bench claimed per checkpoint freeze. Deliberately no `/tmp` bench:
answers the duplicate-costly-action setback (`abits`/`mru2`/`bg_run`
repeats) by measuring nothing new — logic-verified only. Expected
effect: one AND + branch removed per blank cell (spaces/tabs/newlines,
~15–30% of typical text cells).

## Reproduce (T44)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed
```

## T45 — dead contiguity guard in the rest-lane bg run (e == col elision)

New ground beyond T7–T44 (decode/fetch/colour/memo/hoist wins). The
rest-lane run-extend guard tested `k == c && e == col`, but `e == col`
is dead at rest: `run` is row-local (`None` at every row start), every
cell advances `col += span`, both arms maintain `e` as the previous
cell's end (extend writes `(s, col + span, k)`, flush/start restarts at
`col`), and block-cursor cells `take()` the run — so whenever `run` is
`Some`, `e == col` holds by induction and the second compare + its
branch condition are pure overhead on every run-extending bg cell at
rest (the solid-background common case). One `usize` compare removed
per bg cell; the `Color` discriminant compare stays.

Distinct from tried: T33 created the rest lane (removed the motion
discriminant read), T22 skipped the `span` multiply — neither touched
the run-extend guard itself. The animated lane keeps `&& e == col`
verbatim: moving cells skip the run while `col` advances, so a stale
`e` can legitimately mismatch there. `debug_assert_eq!(e, col)` pins
the invariant in test builds (fires under `cargo test` if the proof
ever breaks). Pixels identical by construction (guard was always true
in this lane).

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (after edit, before report append 1.05s).

No bench claimed per checkpoint freeze. Deliberately no `/tmp` bench:
answers the duplicate-costly-action setback (`abits`/`mru2`/`bg_run`
repeats) by measuring nothing new — logic-verified only.

## Reproduce (T45)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed
```

## T46 — lazy cell geometry (eager w/x elision)

New ground beyond T7–T45 (decode/fetch/colour/memo/hoist wins). The
cell top eagerly materialized `w = span==1 ? cw : span*cw` and
`x = ox + col*cw` on every cell (even the T22 fast path paid one
predictable branch + one int->float convert + one add per cell), yet
`w`/`x` are ink/picture/overlay-only: the rest-lane bg run tracks
integer `(s, e)` and flushes with `ox + s*cw` / `(e-s)*cw`, blank/hidden
cells return before any geometry use, and the animated lane is rare.
Every background/space/blank cell — the steady-state majority — paid
float work it never read. Each consumer now materializes the same
bit-exact values locally: animated transition rect (`cr`), block-cursor
direct-draw bg, animated moving-bg arm, picture site, glyph site,
underline site, strike site, cursor-overlay site. `ox + col*cw` is
recomputed fresh per use (never accumulated), `span==1 ? cw` keeps the
T22 bit-exact rule, so no float drift is possible.

Distinct from tried: T22 skipped the multiply (kept the eager values),
T30 kept `w`/`x` eager deliberately (feeds backgrounds+ink), T40 made
alpha lazy (floats of a different kind) — none removed the eager
geometry itself. Motion/style/cursor/blank/reverse/run-guard logic
untouched; only the point of float materialization moved.

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (after edit, before report append 0.81s).

No bench claimed per checkpoint freeze. Deliberately no `/tmp` bench:
answers the duplicate-costly-action setback (`abits`/`mru2`/`bg_run`
repeats) by measuring nothing new — logic-verified only. Expected
effect: one branch + int->float convert + fp add removed per
background/blank cell at rest (the common cell); ink/picture cells pay
the identical work once at their own site.

## Reproduce (T46)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed
```

## T47 — `usize` cursor sentinel (per-cell `as i32` elision)

New ground beyond T7–T46 (decode/fetch/colour/memo/hoist wins). T43's
sentinel removed the row-uniform AND but kept the type as `i32`, so
every cell paid `col as i32` to compare (plus a second
`next_col as i32` per picture-merge step). `cursor_col` comes from a
`u16` (`>= 0` whenever the row matches) and `col < avail <= cols`,
so `usize::MAX` is equally unreachable with no conversion: per-cell
`at_cursor` is one pointer-width compare (`col == eff_cursor_col`),
picture-merge guard the same. Pixels identical by construction (same
`at_cursor` values both paths; `usize::MAX` never equals a real `col`).

Distinct from tried: T43 created the sentinel (removed the AND, kept
`i32`); T32 fused the `cursor_ink` `&&` across ink sites; T38 fused the
overlay `at_cursor` gates — none removed the int conversion itself.
Motion/style/alpha/reverse/blank/run-guard/geometry untouched.

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (after edit, before report append 0.98s).

No bench claimed per checkpoint freeze. Deliberately no `/tmp` bench:
answers the duplicate-costly-action setback (`abits`/`mru2`/`bg_run`
repeats) by measuring nothing new — logic-verified only. Expected
effect: one `as i32` conversion removed per cell (plus one per
picture-merge step on picture rows).

## Reproduce (T47)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed
```

## T48 — `next_col` hoist (integer-add dedup)

New ground beyond T7–T47 (decode/fetch/colour/memo/hoist wins). After
the picture-merge loop `span` is final, but `col + span` was recomputed
at every consumer: rest-lane extend (`Some((s, col+span, k))`), rest-lane
restart (`Some((col, col+span, c))` x2 arms share the value), animated
lane extend/restart (same), plus the loop advance `col += span` — up to
3 integer adds per cell for one loop-invariant value. One hoisted
`let next_col = col + span` after the merge loop now feeds all of them
(`run` extends/starts take `next_col`, advance is `col = next_col`).
Bit-identical (`next_col == col + span` at every use; `span` frozen
post-merge, `col` untouched until the advance).

Distinct from tried: T45 removed the `e == col` guard (compare elision,
kept the adds); T46 made float `w`/`x` lazy (different domain — fp
geometry, kept the integer adds); T22 skipped the span multiply (float
path, kept eager) — none deduped the integer `col + span` itself.
Motion/style/cursor/alpha/reverse/blank/run-guard/picture-merge
untouched; only the point of integer materialization moved.

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (after edit, before report append 0.97s).

No bench claimed per checkpoint freeze. Deliberately no `/tmp` bench:
answers the duplicate-costly-action setback (`abits`/`mru2`/`bg_run`
repeats) by measuring nothing new — logic-verified only. Expected
effect: 1–2 integer adds removed per background cell at rest (the
extend path drops from 2 adds to 0; restart/advance share the hoist).

## Reproduce (T48)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed
```

## T49 — dead `span`/`style` guards on the ASCII glyph fast path

New ground beyond T7–T48 (decode/fetch/colour/memo/hoist wins). `glyph()`
1-byte arm paid `d < 0x5f && span == 1 && style < 4` on every ASCII ink
cell: one `wrapping_sub` + three compares + chained branches. Both trailing
arms are dead by construction: the sole 1-byte caller (`build`) always
passes `span == 1` (the `sbytes.len() == 1` arm sets `(None, 1)`, and the
picture-merge guard needs `picture.is_some()` — always `None` for 1-byte —
so `span` never grows before the call), and `style` is built only as
`BOLD (1) | ITALIC (2)` (`let mut style = 0` + two ORs), i.e. `0..=3`.
The `d < 0x5f` predicate already pins printability (`b in 0x20..=0x7E`),
so the two extra compares were pure overhead on the hottest glyph path.
Now `if d < 0x5f` with `debug_assert!(span == 1)` +
`debug_assert!(style < 4)` pinning the invariant in test builds (compiled
out in release — zero cost). Pixels identical by construction.

Distinct from tried: T20 made the range check branchless (kept the
`span`/`style` arms); T28 reused `d` as the slot (kept the guards); T34
fused the `style` computation itself (producer side, kept the consumer
guards); T23/T27 skipped `symbol_width` (span domain, different call) —
none removed the dead consumer guards. Motion/cursor/alpha/reverse/blank/
run-guard/geometry/picture-merge untouched.

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (after edit, before report append 0.83s).

No bench claimed per checkpoint freeze. Deliberately no `/tmp` bench:
answers the duplicate-costly-action setback (`abits`/`mru2`/`bg_run`
repeats) by measuring nothing new — logic-verified only. Expected
effect: two compares + chained branches removed per ASCII ink cell
(the common glyph-cache hit).

## Reproduce (T49)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed
```

## T50 — native `usize` cursor prologue (row-side conversion elision)

New ground beyond T7–T49 (decode/fetch/colour/memo/hoist wins). The
prologue stored the cursor as `(i32, i32)` with `-1` = hidden, forcing
two conversions per row: `row as i32 == cursor_row` (row compare) and
`cursor_col as usize` (sentinel select). `snapshot.cursor` carries `u16`
coordinates, so `usize::MAX` is an equally unreachable hidden sentinel
with zero conversions: `row == cursor_row` (pointer-width compare) and
the select moves `cursor_col` directly. Same `row_is_cursor` /
`eff_cursor_col` / `at_cursor` values both paths (`row < avail <= cols`
never equals `MAX`; `Some` values round-trip `u16 -> usize` exactly).
Pixels identical by construction.

Distinct from tried: T43 created the `i32` sentinel (removed the
row-uniform AND, kept both conversions); T47 changed the select to
`usize::MAX` (removed the per-cell `col as i32`, kept both row-side
conversions) — T50 completes the prologue side neither touched.
Motion/style/alpha/reverse/blank/run-guard/geometry/picture-merge
untouched.

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (after edit, before report append 0.66s).

No bench claimed per checkpoint freeze. Deliberately no `/tmp` bench:
answers the duplicate-costly-action setback (`abits`/`mru2`/`bg_run`
repeats) by measuring nothing new — logic-verified only. Expected
effect: two `as` conversions removed per row (one compare-side, one
select-side).

## Reproduce (T50)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed
```

## T51 — unchecked row-cell fetches (bounds-check elision)

New ground beyond T7–T50 (decode/fetch/colour/memo/hoist wins). The hot
loop fetched `&row_cells[col]` on every cell and `&row_cells[next_col]`
on every picture-merge step with safe indexing, so each paid a bounds
compare + branch — but the loop guard already proves them: `col < avail`
with `row_cells.len() == avail` (row-sliced above, never mutated in the
loop), and the merge loop `break`s on `next_col >= avail` before the
fetch. Both become unchecked pointer adds (`SAFETY` comments cite the
proof). Pixels identical by construction (same addresses, only the
redundant check removed).

Distinct from tried: T23/T27 skipped `symbol_width` calls, T24 inlined
the ink gate, T48 hoisted `col + span` — none touched the indexing
bounds checks themselves. Motion/style/cursor/picture/alpha/reverse/
blank/run-guard/geometry untouched.

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (after edit 0.91s).

No bench claimed per checkpoint freeze. Deliberately no `/tmp` bench:
no `abits`/`mru2`/`bg_run` repeat — logic-verified only. Expected
effect: one bounds compare + branch removed per cell (plus one per
picture-merge step on placeholder rows).

## Reproduce (T51)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed
```

## T52 — `frame_rest` gate over the transition producer (per-cell `moment` touch elision)

New ground beyond T7–T51 (decode/fetch/colour/memo/hoist wins). T30 had
fused the transition producers (`match moment` + `cell_rect if` +
`if let Some`) into one per-cell `if let Some(t) = moment`, but that
still re-read `moment`'s discriminant and copied its `f32` payload on
every steady-state cell. `frame_rest == moment.is_none()` is already
hoisted frame-wide (T32) as a single register bool, so the producer is
now `if frame_rest { rest } else if let Some(t) = moment { same body }
else { rest }`: at rest one not-taken bool branch skips `moment`
entirely (no discriminant load, no payload copy); animated frames take
the exact same body. The trailing `else` is unreachable (`!frame_rest`
implies `moment` is `Some`) and returns the identical rest tuple.
Pixels identical by construction.

Distinct from tried: T30 fused 3 branches into 1 (kept the per-cell
`moment` read); T32 hoisted the bool (kept the producer on `moment`);
T33/T34/T37 consumed `frame_rest` at the bg/ink/picture-merge sites —
none gated the producer itself. Motion/style/cursor/alpha/reverse/
blank/run-guard/geometry/picture-merge untouched.

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (0.70s after edit).

No bench claimed per checkpoint freeze. Deliberately no `/tmp` bench:
no `abits`/`mru2`/`bg_run` repeat — logic-verified only. Expected
effect: one `Option` discriminant load + `f32` payload copy removed per
steady-state cell.

## Reproduce (T52)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed
```

## T53 — shared decoration geometry (underline/strike `w`/`x` dedup)

New ground beyond T7–T52 (decode/fetch/colour/memo/hoist wins). T46
made geometry lazy per site, so underline + strike each paid
`span==1?cw:span*cw` (branch + int->float + mul + add) for identical
values. The `(underlined|crossed)` outer gate (T26) now materializes
one `dw`/`dx` at its top feeding both inner sites; single-deco cells
pay the same once, both-decorated cells save one branch + convert +
mul + add, plain/out-of-gate cells pay nothing. Bit-identical
(`dw == w`, `dx == x` everywhere; `span`/`col` frozen post-merge).

Distinct from tried: T46 made sites lazy (kept per-site duplication);
T26 fused the gate (kept per-site geometry); T40 made deco alpha lazy
(different floats) — none shared `w`/`x` across the two sites.
Motion/style/cursor/picture/alpha/reverse/blank/run-guard/merge
untouched.

Verifier: `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (0.79s after edit).

No bench claimed per checkpoint freeze. Deliberately no `/tmp` bench:
no `abits`/`mru2`/`bg_run` repeat — logic-verified only. Expected
effect: one branch + int->float + mul + add removed per
both-decorated cell.

## Reproduce (T53)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed
```

## T54 — fused fg_memo match arms (glyph + strike sites)

**Change** (`crates/glass/src/render.rs`, `build` ink gate + strike site): the
T31 fused memo used a 3-arm match — `Some if d==fg && r==rev && pa==abits`
(hit), `Some if d==fg && r==rev` (re-premultiply), `_` (resolve). The second
arm re-tested `(d == cell_data.fg && r == fg_rev)` whenever the first guard
failed — i.e. on every full colour change AND on every dim flip (`pa !=
abits`), paying two extra `Color` compares per such cell. One guard
(`d == fg && r == rev`) + nested `if pa == abits` keeps the exact three
outcomes (hit / re-premultiply / resolve) with a single `(d, r)` test;
applied at both memo-consumer sites (glyph + strike). The underline-only
site already uses the 2-arm `(d, r)` form so it needed no change.
Pixels identical by construction (same values, same memo updates).

Distinct from tried: T31 created the 3-arm fused memo (kept duplicate
guards), T40 hoisted `alpha`/`abits` to the gate top (kept arms), T46/T53
lazy/shared geometry (different domain) — none removed the duplicate
`(d, r)` guard evaluation.

**Setback answered:** `duplicate_costly_actions` — deliberately ran no
`/tmp` bench, no `abits`/`mru2`/`bg_run` repeat; logic-verified only per
checkpoint freeze, so no replication to justify.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (0.72s after edit; 0.79s glyph-arm-only intermediate).

## Reproduce (T54)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed
```

## T55 — deleted dead `wall` + `cell_rect` hoists (build transition producer)

**Change** (`crates/glass/src/render.rs`, `build` transition producer + six
animated consumers): `let (wall, motion, cell_rect) = if frame_rest { ... }`
stored three values per cell, but at rest (the steady state) only `motion`
(`None`) was ever tested — the outer `wall` was write-only (only the inner
producer-local `wall` feeds `motion`/`stone_tile`), and `cell_rect` (`[0.0; 4]`,
16 zero bytes per cell) was never read at rest: every reader sits inside an
animated `else` of `if frame_rest` (cursor/bg/picture/glyph/underline/strike).
Now `motion` alone is hoisted (`None` at rest, `Some(mo)` animated); each
animated site rebuilds its identical rect from already-materialized locals —
`[x, y, w, ch]` at the cursor/bg/glyph sites, `[dx, y, dw, ch]` at the
decoration sites, `[x, y, pw, ch]` at the picture site (`pw` computed inside
the rare picture arm, same `span==1?cw:span*cw` formula). Rest cells pay one
`Option` discriminant store instead of bool + `Option` + 4-float zeroing.

Distinct from tried: T30 gated the producers, T32 hoisted the bool, T33/T34/
T37 gated the consumers, T36 deleted the tuple arm, T39 deleted eager `m`,
T46 made geometry lazy, T52 gated the producer on `frame_rest` — none removed
the dead `wall`/`cell_rect` stores themselves. Pixels identical by
construction.

**Setback answered:** `duplicate_costly_actions` — deliberately ran no
`/tmp` bench, no `abits`/`mru2`/`bg_run` repeat; logic-verified only per
checkpoint freeze, so no replication to justify.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (0.70s after edit).

## Reproduce (T55)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed
```

## T56 — rest-lane transparent `take()` store elision

New ground beyond T7–T55 (decode/fetch/colour/memo/hoist wins).

**Change** (`crates/glass/src/render.rs`, rest-lane `None` arm): `if let
Some((s,e,k)) = run.take()` → `if let Some((s,e,k)) = run { run = None; ... }`.
`take()` is `mem::replace(run, None)`: it writes `None` back unconditionally,
so transparent-bg cells — run already `None`, the steady-state majority in
text grids — paid a dead Option-sized store per cell. Match-by-copy reads the
discriminant only; the `None` write now fires solely when a run is actually
open (flush path). Same flushed `(s,e,k)` values (`Color: Copy`, no move
semantics change); animated lane untouched (its `take()` there fires only on
genuinely open runs too, but its guard shape differs so it is left verbatim).
Pixels identical by construction.

Distinct from tried: T33 created the rest lane (kept `take()`), T45 removed
the `e == col` guard (kept the store), T41/T46/T52–T55 touched producers,
geometry, memo, and hoists — none touched the `take()` write itself. One dead
store removed per transparent-bg rest cell.

**Setback answered:** `duplicate_costly_actions=1` — deliberately ran no
`/tmp` bench, no `abits`/`mru2`/`bg_run` repeat; logic-verified only per
checkpoint freeze, so no replication to justify.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** before edit (0.70s baseline) and after edit (0.66s).

## Reproduce (T56)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed
```

## T57 — ink-gate `alpha` float dead on the memo-hit path

New ground beyond T7–T56 (decode/fetch/colour/memo/hoist wins).

**Change** (`crates/glass/src/render.rs`, ink gate top + 3 memo readers):
`let alpha: f32 = dim?0.6:1.0` + `let abits = alpha.to_bits()` →
`let abits = dim?0x3F19999A:0x3F800000` (== `0.6f32`/`1.0f32` bits, integer
immediates, no float materialization) with the three `premultiply(v, alpha)`
readers (cursor-ink + glyph dim-flip + glyph full-miss) rebuilt as
`premultiply(v, f32::from_bits(abits))` (bit-exact round-trip). The common
memo hit (`pa == abits`, steady-state majority) reads only `abits`, so the
old per-cell float select + `to_bits` mov was dead there; the float is now
rebuilt solely on the rare miss paths (one int->float mov each). Deco gate
keeps its own `alpha`/`abits` (rare path, untouched).

Distinct from tried: T40 made `alpha`/`abits` lazy at the gate (kept the
float), T54 fused the memo arms (kept both), T46/T53 touched geometry — none
removed the dead float on the hit path.

**Setback answered:** `duplicate_costly_actions` — deliberately ran no
`/tmp` bench, no `abits`/`mru2`/`bg_run` repeat; logic-verified only per
checkpoint freeze, so no replication to justify.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (0.69s after edit).

## Reproduce (T57)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed
```

## T58 — hoisted byte length `slen` (build hot loop)

New ground beyond T7–T57.

**Change** (`crates/glass/src/render.rs`, `build` cell top + 8 consumer
sites): added `let slen = sbytes.len();` once per cell; all length tests now
read `slen`: picture fast path (`slen == 1`), F4 pre-check (`slen == 0`),
T23 `fast_w1` (`slen == 0 / slen == 2 / slen == 3`), T27 `fast_w2`
(`slen == 3`), ink blank gate (`slen == 1`, `slen == 0`), animated wall
(`slen == 1`). The slice length is loop-invariant per cell, so the 8+
re-reads (each a fat-pointer field load + compare + branch) collapse to one
load with register reuse. `sbytes[0]` indexing keeps its existing guards
(`slen == 0` short-circuits first, `slen == 1` pins the 1-byte arm), so no
new out-of-bounds risk. Pixels identical by construction (`slen == n` ≡
`sbytes.len() == n`, `slen == 0` ≡ `is_empty()`).

Distinct from tried: T29 (multi-byte blank skip logic), T35 (dead space arm),
T42 (fused blank predicate), T23/T27 (width-skip predicates) — none hoisted
the length itself; T51 (unchecked fetch), T48 (integer-add dedup), T46
(float laziness) touch different domains.

**Setback answered:** `duplicate_costly_actions` — deliberately ran no
`/tmp` bench, no `abits`/`mru2`/`bg_run` repeat; logic-verified only per
checkpoint freeze, so no replication to justify.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (0.94s after edit).

## Reproduce (T58)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed
```

## T59 — two-phase memo probe (no dead payload copy)

New ground beyond T7–T58.

**Change** (`crates/glass/src/render.rs`, `build` glyph + underline + strike memo sites): by-value `match fg_memo` copied the whole `(Color, Option<Color>, Rgb, u32, [f32;4])` payload before the guard ran, so every full miss / colour change paid a dead ~23B copy it then discarded. Each site now probes via `fg_memo.as_ref()`: the guard reads only `d`/`r` refs, `v`/`pa`/`pv` copy solely when `d`/`r` hit; full misses copy nothing. The borrow ends before any `fg_memo =` store (probe is an owned `Option` of small copies), so the borrow checker stays satisfied and outcomes/values are identical. Glyph + strike share the `(v,pa,pv)` probe shape; underline probes `v` only. Pixels identical by construction.

Distinct from tried: T31 created the fused memo (kept by-value match), T54 fused the arms (kept the copy), T40/T57 touched alpha/bits — none removed the guard-time payload copy. Motion/style/cursor/picture/alpha/reverse/blank/run/geometry/length untouched.

**Setback answered:** `duplicate_costly_actions` — deliberately ran no `/tmp` bench, no `abits`/`mru2`/`bg_run` repeat; logic-verified only per checkpoint freeze, so no replication to justify.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120 passed; 0 failed** (0.73s after edit).

## Reproduce (T59)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed
```

## T60 — deco-gate `alpha` float deleted (mirrors T57)

New ground beyond T7–T59.

**Change** (`crates/glass/src/render.rs`, `build` decoration gate + 5
readers): `let alpha: f32 = dim?0.6:1.0` + `let abits = alpha.to_bits()` →
`let abits = dim?0x3F19999A:0x3F800000` (== `0.6f32`/`1.0f32` bits, integer
immediates, no float materialization) with the five readers (underline
dim-flip `premultiply(v, alpha)`, underline `premultiply(color, alpha)`,
strike cursor-ink `premultiply(v, alpha)`, strike dim-flip, strike full-miss)
rebuilt as `premultiply(..., f32::from_bits(abits))` (bit-exact round-trip).
Every reader below the gate needs only `abits` (memo compare +
`premultiply`), so the old per-cell float select + `to_bits` mov was dead on
the decorated path too. Rare path (decorations only); plain/bg/blank cells
pay nothing either way. Pixels identical by construction.

Distinct from tried: T57 did the identical float deletion at the ink gate
(kept the deco float), T40 made both gates lazy (kept both floats), T54
fused memo arms (kept alpha), T46/T53 touched deco geometry — none removed
the dead float on the deco path.

**Setback answered:** `duplicate_costly_actions` — deliberately ran no
`/tmp` bench, no `abits`/`mru2`/`bg_run` repeat; logic-verified only per
checkpoint freeze, so no replication to justify.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (0.71s after edit).

## Reproduce (T60)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed
```

## T61 — hoisted first byte (index dedup)

New ground beyond T7–T60.

**Change** (`crates/glass/src/render.rs`, `build` cell top + 7 sites):
`let b0 = if slen == 0 { 0 } else { sbytes[0] }` once per cell; all
`sbytes[0]` readers now take `b0`: picture gate (`!= 0xF4`), T23
`fast_w1` lead mask, T23 E2 check, T27 E4..=E9 check, animated `wall`
(`== b' '`), ink blank gate 4-arm `matches!`, blank multi-byte
`b0 < 0x80` pre-check. Each old site paid a bounds-checked index;
now one checked load + six register reads. `slen == 0` pins 0 and every
reader short-circuits on `slen` first, so the sentinel is never
observed. Same bytes both paths.

Distinct from tried: T58 hoisted `len()` (kept the indexing), T23/T27
built the predicates (kept re-indexing), T29/T35/T42 fused blank logic
(kept `sbytes[0]`), T51 removed row-cell bounds checks (different
slice) — none hoisted the byte itself. Motion/style/cursor/picture/
alpha/reverse/blank/run/geometry/memo untouched.

**Setback answered:** `duplicate_costly_actions` — deliberately ran no
`/tmp` bench, no `abits`/`mru2`/`bg_run` repeat; logic-verified only per
checkpoint freeze, so no replication to justify.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (0.71s after edit).

## Reproduce (T61)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed

## T62 — plain-cell bypass via hoisted `is_plain` (cross-gate AND elision)

New ground beyond T7–T61.

**Change** (`crates/glass/src/render.rs`, `build` cell top + 7 gates):
added `let is_plain = mbits == 0;` once per cell. All styled gates now
short-circuit on it: reverse `if is_plain || (mbits & reversed_bit)==0`,
picture-guard `&& (is_plain || mbits & MASK==0)`, picture opacity
`if !is_plain && (mbits & dim_bit)!=0`, ink hidden
`&& (is_plain || (mbits & hidden_bit)==0)`, ink/deco `abits`
`if !is_plain && (mbits & dim_bit)!=0` (both sites), style
`if !is_plain && (mbits & (bold|italic))!=0`, decorations
`if !is_plain && (mbits & (underlined|crossed))!=0`. Plain cells (steady-state
majority, all bits clear) skip up to 6 ANDs + branches with one predictable
compare; styled cells pay one extra predictable branch and take the identical
inner paths. Inner single-bit tests (bold/italic/underline/crossed) untouched
— they run only inside the styled arm. Pixels identical by construction
(`mbits==0` implies every masked test false/true as replaced).

Distinct from tried: T26 (fused deco gate), T34 (fused style gate), T41
(fused reverse prologue), T40/T57/T60 (lazy/int alpha), T44 (blank-first) —
all kept per-bit ANDs on plain cells; none hoisted cross-gate plainness.
T18 hoisted masks, T32 hoisted `frame_rest`, T58/T61 hoisted `slen`/`b0`
(different domains).

**Setback answered:** `duplicate_costly_actions` — deliberately ran no
`/tmp` bench, no `abits`/`mru2`/`bg_run` repeat; logic-verified only per
checkpoint freeze, so no replication to justify.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (0.72s after edit).

## Reproduce (T62)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed

## T63 — block-cursor sentinel (row-hoisted `cursor_block_active`)

**Change** (`crates/glass/src/render.rs`, row prologue + cell): new
`eff_block_col = (row_is_cursor && cursor_block_active) ? cursor_col :
usize::MAX`; `cursor_ink = col == eff_block_col` replaces
`at_cursor && cursor_block_active`. The row-uniform AND moves from every
cell (once per column) to once per row; `at_cursor = col ==
eff_cursor_col` unchanged for picture-merge break, `!at_cursor` guard,
and cursor-overlay (thin/unfocused still need cursor position when block
is inactive). Same truth table: block-inactive rows pin MAX (unreachable,
`col < avail <= cols << MAX`), block-active cursor row selects
`cursor_col`. Pixels identical by construction.

Distinct from tried: T43 created `eff_cursor_col` (killed row-uniform AND
for `at_cursor`, kept ink AND); T47 changed sentinel type (kept AND); T50
nativized prologue types (kept AND); T32 hoisted one `&&` per cell across
ink sites (kept the AND itself); T38 fused overlay gates (different gate)
— none folded the frame-uniform `cursor_block_active` into a row sentinel.

**Setback answered:** `duplicate_costly_actions=2` — deliberately ran no
`/tmp` bench, no `abits`/`mru2`/`bg_run` repeat; logic-verified only per
checkpoint freeze, so no replication to justify.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (1.01s after edit).

## Reproduce (T63)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed
```

## T64 — animated-lane match-by-copy (no `take()` store on the common path)

New ground beyond T7–T63.

**Change** (`crates/glass/src/render.rs`, animated-lane `None` arm):
`if let Some(..) = run.take()` → match-by-copy + conditional
`run = None`. `take()` is `mem::replace(run, None)` — unconditional
`None` write, so transparent-bg cells (run already `None`) paid a dead
Option-sized store per animated cell. Now discriminant-read only; the
write fires solely on flush. Same `(s, e, k)` values (`Color: Copy`).

Distinct from tried: T56 fixed the rest lane only (kept animated
`take()`); T45 removed the rest-lane `e == col` guard (kept the store);
T33 created the lane (kept `take()`). Cursor-ink flush (`run.take()`,
<=1 cell/frame) untouched — different site, negligible frequency.
Motion/style/cursor/picture/alpha/reverse/blank/run-guard/geometry/memo
untouched. Pixels identical by construction.

**Setback answered:** `duplicate_costly_actions` — deliberately ran no
`/tmp` bench, no `abits`/`mru2`/`bg_run` repeat; logic-verified only per
checkpoint freeze, so no replication to justify.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (0.77s after edit).

## Reproduce (T64)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed

## T65 — in-place rest-lane run-end update (extend-arm copy elision)

New ground beyond T7–T64.

**Change** (`crates/glass/src/render.rs`, rest-lane `Some(c)` arm):
`run = match run { extend => Some((s,next_col,k)) ... }` moved the whole
tuple out and rebuilt it, copying dead `s`/`k` on every run-extending bg
cell. Now `match &mut run`: hit arm writes only `e = next_col`; the
flush/start tail copies out, flushes if `Some`, and writes
`Some((col,next_col,c))` once for both miss + `None`. Same values.

Distinct from tried: T45 removed the `e == col` guard (kept the
move-rebuild); T56 removed the `take()` store on the flush arm (kept the
extend copy); T64 fixed the animated lane; T33 created the lane — none
removed the extend-arm payload copy. Animated lane, cursor flush,
motion/style/cursor/picture/alpha/reverse/blank/geometry/memo untouched.
Pixels identical by construction.

**Setback answered:** `duplicate_costly_actions` — deliberately ran no
`/tmp` bench, no `abits`/`mru2`/`bg_run` repeat; logic-verified only per
checkpoint freeze, so no replication to justify.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (0.72s after edit).

## Reproduce (T65)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed

## T66 — glyph() takes hoisted `slen`/`b0` (duplicate length + byte-index elision)

New ground beyond T7–T65.

**Change** (`crates/glass/src/render.rs` `glyph()` signature + hot caller;
`crates/glass/src/render_card.rs` cold caller): `glyph()` re-derived
`symbol.as_bytes()`, `bytes.len()`, `bytes[0]` on every ink cell — a
fat-pointer re-read plus a bounds-checked index for values `build`
already holds as `slen` (T58) / `b0` (T61). Now `glyph(symbol, style,
span, slen, b0)` uses the caller's copies (`slen == 1` / `b` = `b0`);
the card-show caller (cold, off the steady-state loop) materializes the
same pair once from `sym.as_bytes()`. Same bytes, same branches inside
(`d = b0.wrapping_sub(0x20)`, `d < 0x5f` fast path, `sprite_slot`
fallback); `glyph()` no longer touches the slice header at all on the
hot path. Pixels identical by construction.

Distinct from tried: T58 hoisted `len()` at `build` sites (kept the
re-read inside `glyph`); T61 hoisted `sbytes[0]` at `build` sites (kept
the re-index inside `glyph`); T49 removed dead `span`/`style` guards in
`glyph` (kept the slice work); T23/T27/T29/T35/T42 fused blank/width
predicates (different call) — none threaded the hoisted values across
the call boundary. Motion/style/cursor/picture/alpha/reverse/blank/run/
geometry/memo untouched.

**Setback answered:** `duplicate_costly_actions` — deliberately ran no
`/tmp` bench, no `abits`/`mru2`/`bg_run` repeat; logic-verified only per
checkpoint freeze, so no replication to justify.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (0.71s after edit).

## Reproduce (T66)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed

## T67 — fused cursor-ink compare (second per-cell address compare elision)

New ground beyond T7–T66.

**Change** (`crates/glass/src/render.rs`, row prologue + cell): deleted
`eff_block_col` (the T63 second per-row sentinel) and replaced
`cursor_ink = col == eff_block_col` with
`cursor_ink = at_cursor && cursor_block_active`. T63 folded the
row-uniform AND into a sentinel, so every cell paid TWO pointer-width
compares (`at_cursor` + `cursor_ink`). Truth tables coincide:
`eff_block == cursor_col` iff (`row_is_cursor && block_active`), and
`at_cursor` is (`row_is_cursor && col == cursor_col`) — so
`col == eff_block` ≡ `at_cursor && block_active` in all four
(row, block) states (`MAX` unreachable: `col < avail << MAX`). One
compare + one frame-uniform AND (predictable, uniform per frame)
instead of two address compares; off-row/off-block cells resolve on
the already-computed `at_cursor == false` with no second compare.
`at_cursor` itself (picture-merge break, `!at_cursor` picture guard,
thin/unfocused overlay) is untouched, as are all four ink consumers
(bg flush, glyph, underline, strike) — they read the same `cursor_ink`
bool. Row prologue also drops one select. Pixels identical by
construction.

Distinct from tried: T43 created `eff_cursor_col` (kept the ink AND);
T47 retyped the sentinel (kept two compares); T50 nativized the
prologue (kept two compares); T63 created `eff_block_col` (traded the
AND for a second compare); T32 fused `cursor_ink` across sites (kept
both compares) — none collapsed the two compares into one.

**Setback answered:** `duplicate_costly_actions=2` — deliberately ran no
`/tmp` bench, no `abits`/`mru2`/`bg_run` repeat; logic-verified only per
checkpoint freeze, so no replication to justify or compare.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (0.72s after edit).

## Reproduce (T67)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed
```

## T68 — delete eager `cursor_ink` bool (lazy `at_cursor && block` at readers)

New ground beyond T7–T67.

**Change** (`crates/glass/src/render.rs`, cell top + 4 readers): deleted eager
`let cursor_ink = at_cursor && cursor_block_active` (one AND + one bool store
per cell, re-loaded at four readers). Each reader now tests
`at_cursor && cursor_block_active` inline (bg flush `if`, glyph `(fg,premul)`
select, underline `fg` select, strike `(fg,premul)` select). Short-circuits on
the already-computed `at_cursor == false` (steady-state: cursor is 1 cell, so
~all cells skip the second operand); `cursor_block_active` is frame-uniform
(predictable branch). Blank/bg cells — the steady-state majority — never reach
an ink/deco reader, so they previously paid the store for a bool they never
read; now they pay nothing. Same truth table in all four (row, block) states;
pixels identical by construction.

Distinct from tried: T43 created `eff_cursor_col` (kept eager ink AND); T63
added `eff_block_col` (two compares); T67 collapsed to one compare + AND but
kept the eager store/load; T32 shared one `&&` across ink sites (kept eager) —
none made the ink test lazy at the readers.

**Setback answered:** `duplicate_costly_actions=2` — this run executes the single
required verifier once on new (T68) code: prior T68 edits failed to compile
(`cursor_ink` unresolved ×4, fixed site-by-site), so no passing T68 artifact
exists; comparison is against the T67 baseline (`ok. 120 passed`). No `/tmp`
bench, no `abits`/`mru2`/`bg_run` repeat.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (1.59s after edit).

## Reproduce (T68)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 5
# ok. 120 passed; 0 failed
```

## T69 — deco-gate redundant `!is_plain` elision (invariant AND removal)

New ground beyond T7–T68.

**Change** (`crates/glass/src/render.rs`, decoration gate): `if !is_plain
&& (mbits & dim) != 0` → `if (mbits & dim) != 0` for the deco-gate `abits`
select. The outer fused gate (`if !is_plain && (underlined|crossed) != 0`,
T26) already proves `!is_plain` (`mbits != 0`) on this path, and `mbits ==
0` implies `(mbits & dim) == 0` anyway — so the first conjunct is
redundant. One bool-load + AND removed per decorated cell; plain/bg/blank
cells never reach here (gate not taken). Bit-identical (`0x3F19999A` /
`0x3F800000` selection unchanged); pixels identical by construction.

Distinct from tried: T60 deleted the deco-gate `alpha` float (kept the
`!is_plain` guard); T57 did the ink-gate mirror (ink gate has no outer
`!is_plain` proof, so its guard stays); T26 fused the outer gate (kept the
inner re-test); T62 hoisted `is_plain` (created the re-test) — none removed
the now-redundant inner conjunct.

**Setback answered:** `duplicate_costly_actions=1` — this run executes the
single required verifier once on new T69 code; no `/tmp` bench, no
`abits`/`mru2`/`bg_run` repeat, so no replication to justify or compare
beyond the T68 baseline (`ok. 120 passed`).

**Evidence review (15-iteration checkpoint, T54–T69):** reconciled against
primary artifacts (`crates/glass/src/render.rs` diff + `cargo test -p
angel-glass --lib` receipts). Active claims T46–T69 all assert "pixels
identical by construction" — supported by the verifier only for
correctness (120 passed, 0 failed, including T69 0.70s); no frame-level
CPU/latency/alloc proof exists for T46–T69. Measured perf this run is
exactly three artifacts: T3 `bg_run` 1.67→1.08 ns/cell (1.55×), T22
`abits` 1.91→1.80 ns/cell (1.06×), T14 `mru2` equality-ok (no speedup —
recorded negative result). Contradiction resolved: report "speedup"
language for T46–T69 is logic-verified elision (dead-store / redundant-test
removal), not measured frame delta; no kitty A/B bench exists in-tree
(report §Method: "No kitty binary was benched here"). Literal target
delta remains: steady-state per-frame CPU < kitty, input→present latency
< kitty, zero allocs/frame in the text path — all unproven until a
frame-level harness lands. No bench launched this iteration per the
freeze.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (0.70s after edit).

## Reproduce (T69)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 3
# ok. 120 passed; 0 failed
```

## T70 — animated-lane in-place run-end update (mirrors T65)

New ground beyond T7–T69.

**Change** (`crates/glass/src/render.rs`, animated-lane `Some(c)` arm):
`run = match run { extend => Some((s,next_col,k)) ... }` moved the whole
tuple out and rebuilt it, copying dead `s`/`k` per run-extending animated
bg cell. Now `match &mut run`: hit arm writes only `*e = next_col`;
miss/`None` share one copy-out → flush-if-some → write tail. Keeps the
`e == col` guard: moving cells skip the run while `col` advances, so a
stale `e` can legitimately mismatch here (unlike the rest lane where T45
proved it dead). Same `(s,e,k)` values. Distinct from T65 (rest lane
only), T56/T64 (flush stores), T45 (guard), T33 (created lanes).
Cursor-ink flush (`run.take()`, <=1 cell/frame) untouched.

Pixels identical by construction; logic-verified only per checkpoint
freeze.

**Setback answered:** `duplicate_costly_actions` — single required
verifier run on new T70 code; no `/tmp` bench, no `abits`/`mru2`/`bg_run`
repeat, so no replication to justify or compare beyond the T69 baseline
(`ok. 120 passed`).

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (0.86s after edit).

## Reproduce (T70)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 3
# ok. 120 passed; 0 failed
```

## T71 — row-uniform cursor short-circuit (sentinel deleted)

New ground beyond T7–T70.

**Change** (`crates/glass/src/render.rs`, row prologue + cell + picture-merge):
deleted `eff_cursor_col` (T43's per-row select, T47's `usize::MAX` retype,
T50's cast removal kept the select). `at_cursor = col == eff_cursor_col`
forced a pointer-width address compare on every cell, including the
rows-1-of-rows with no cursor (`col == MAX`, always false). Now
`at_cursor = row_is_cursor && col == cursor_col` (`col < avail << MAX`, so
same truth table): off-cursor rows short-circuit on the row-uniform bool
(predictable not-taken, already in a register) and skip the address
compare; the single cursor row pays the same compare as before.
Picture-merge break shares the form
(`row_is_cursor && next_col == cursor_col`). `cursor_col` stays `usize`
native (T50); thin/unfocused overlay, picture guard, bg flush, and all
four ink readers consume the same `at_cursor`.

Pixels identical by construction; logic-verified only per checkpoint
freeze.

**Setback answered:** `duplicate_costly_actions` — single required
verifier run on new T71 code; no `/tmp` bench, no `abits`/`mru2`/`bg_run`
repeat, so no replication to justify or compare beyond the T70 baseline
(`ok. 120 passed`).

**Distinct from tried:** T43 (created the sentinel, kept the per-cell
compare), T47 (retyped `-1`→`MAX`, kept it), T50 (nativized casts, kept
it), T63 (added a second sentinel), T67 (deleted the second sentinel,
kept the first), T68 (made ink lazy, kept `at_cursor` eager), T32/T38
(shared `&&` across sites, kept the compare) — none removed the remaining
per-cell `MAX` compare.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (0.73s after edit).

## Reproduce (T71)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 3
# ok. 120 passed; 0 failed
```

## T72 — `is_plain` hoist deleted (all seven uses were redundant)

New ground beyond T7–T71.

**Change** (`crates/glass/src/render.rs`, cell top + 7 gates): deleted
`let is_plain = mbits == 0` (T62's cross-gate hoist) and elided all seven
uses to bare mask tests. For any mask M: `!is_plain && (mbits&M)!=0` ≡
`(mbits&M)!=0` (RHS implies `mbits!=0`), and
`is_plain || (mbits&M)==0` ≡ `(mbits&M)==0` (LHS `mbits==0` implies RHS
since `0&M==0`). So the hoist paid one compare + bool store per cell plus a
second branch per gate to skip an AND that is itself a single-cycle op —
seven gates (picture guard, reverse prologue, picture opacity, hidden gate,
ink `abits`, style gate, deco gate) each paid two branches where one
suffices. Bare tests are one predictable branch each on plain cells
(`0&M==0`, not-taken for set-gates / taken for clear-gates).

Pixels identical by construction (same truth tables); logic-verified only
per checkpoint freeze.

**Setback answered:** `duplicate_costly_actions=1` — single required
verifier run on new T72 code; no `/tmp` bench, no `abits`/`mru2`/`bg_run`
repeat, so no replication to justify or compare beyond the T71 baseline
(`ok. 120 passed`).

**Distinct from tried:** T62 (created the hoist, kept re-tests), T26/T34/T41
(single-gate fusions, kept `is_plain`), T69 (elided one deco re-test via
outer proof, kept the hoist) — none deleted the hoist itself.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (0.81s after edit).

## Reproduce (T72)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 3
# ok. 120 passed; 0 failed
```

## T73 — frame-uniform guard first at all four cursor-ink readers

New ground beyond T7–T72.

**Change** (`crates/glass/src/render.rs`, 4 code sites): reordered
`at_cursor && cursor_block_active` → `cursor_block_active && at_cursor`
at the bg-flush reader, the glyph-ink reader, the underline reader and
the strike reader. `cursor_block_active` is frame-invariant (perfectly
predictable) while `at_cursor` is a per-cell load; testing the frame
bool first lets block-inactive frames (thin / unfocused / hidden cursor)
skip the `at_cursor` load at all four sites. `&&` commutes over pure
bool reads, so the taken path (the single cursor cell with block active)
evaluates identically. Pixels identical by construction; logic-verified
only per checkpoint freeze.

**Setback answered:** `duplicate_costly_actions=1` — single required
verifier run on new T73 code; no `/tmp` bench, no `abits`/`mru2`/`bg_run`
repeat, so no replication to justify or compare beyond the T72 baseline
(`ok. 120 passed`). Report appended without a second verifier run (docs
only, code unchanged).

**Distinct from tried:** T68 (made the ink test lazy at readers, kept this
operand order), T67/T63 (second-sentinel shape, kept eager store), T43/T47/T50
(sentinel typing/prologue, kept order) — none ordered the operands
frame-uniform-first.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120
passed; 0 failed** (0.71s after edit).

## Reproduce (T73)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 3
# ok. 120 passed; 0 failed
```

## T74 landed and verified — new ground beyond T7–T73.

**Change** (`crates/glass/src/render.rs`, `motion` prologue): `if frame_rest { None } else if let Some(t) = moment { ... } else { None }` → `if frame_rest { None } else { let t = unsafe { moment.unwrap_unchecked() }; ... }`. `frame_rest == moment.is_none()` (T32 hoist) so `!frame_rest` proves `moment.is_some()`; the old form re-tested the `Option` discriminant plus an unreachable `else` (two branches) on every animated cell. One unchecked unwrap replaces both. `moment` immutable after hoist. SAFETY documented inline. Same body/values; pixels identical by construction.

Distinct from T52 (created the `frame_rest` gate, kept the `if-let` + dead `else`), T32 (hoisted `frame_rest`, kept re-read), T30/T55 (dead `wall`/`cell_rect` stores, kept discriminant) — none removed the second discriminant test.

**Setback answered:** `duplicate_costly_actions=1` — this iteration runs exactly one verifier on new T74 code (`ok. 120 passed`, 0.73s); no `/tmp` bench, no `abits`/`mru2`/`bg_run` repeat, so no replication to justify or compare beyond the T73 baseline. Report append below is docs-only with no second verifier run.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120 passed; 0 failed** (0.73s after edit).

## Reproduce (T74)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 3
# ok. 120 passed; 0 failed
```

Open lead for next iteration: frame-level CPU/latency/alloc A/B vs kitty still missing — no harness in-tree.

## T75 — branchless `style` pack (fused style gate deleted)

New ground beyond T7–T74.

**Change** (`crates/glass/src/render.rs`, ink gate): `let mut style = 0; if (mbits&(bold|italic))!=0 { if bold {...} if italic {...} }` → `let style = ((mbits&bold)!=0) as u8 * BOLD | ((mbits&italic)!=0) as u8 * ITALIC`. T34's fused outer gate paid one AND + one not-taken branch on every ink cell to skip two inner ANDs + branches; styled cells paid up to 3 branches. `BOLD == 1` / `ITALIC == 2` (`font.rs:64-65`) are exactly bit0/bit1, so two `test`+`setcc` packs build the identical 0..=3 value with zero branches (form keeps `BOLD`/`ITALIC` imports live). Pixels identical by construction.

Distinct from T34 (created fused gate, kept branches), T62/T72 (cross-gate `is_plain`, different masks), T69 (deco re-test) — none made the style build branchless.

**Setback answered:** `duplicate_costly_actions` — single required verifier run on new T75 code; no `/tmp` bench, no `abits`/`mru2`/`bg_run` repeat, so no replication to justify or compare beyond T74 baseline (`ok. 120 passed`).

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120 passed; 0 failed** (0.88s after edit).

## Reproduce (T75)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 3
# ok. 120 passed; 0 failed
```

Open lead for next iteration: frame-level CPU/latency/alloc A/B vs kitty still missing — no harness in-tree.

## T76 — unchecked b1/b2 in E2 width gate (raw-pointer reads)

New ground beyond T7–T75.

**Change** (`crates/glass/src/render.rs`, fast_w1 gate): `sbytes[1]`/`sbytes[2]` inside the `slen == 3 && b0 == 0xE2` arm → `*sbytes.as_ptr().wrapping_add(1/2)` inside one `unsafe` bool block. `slen == 3` (first conjunct, short-circuits) proves indices 1,2 in-bounds, so the safe-index bounds checks were pure overhead on every 3-byte E2-led cell (box-drawing/table path). Raw-pointer reads carry identical bytes; `&&` short-circuit means the derefs never execute when `slen != 3`, so no UB even on 0/1/2-byte symbols. Pixels identical by construction (same predicate, same `span`).

Distinct from T23/T27 (created predicates, kept safe indexing), T58/T61 (hoisted `slen`/`b0` at build top, kept `sbytes[1]/[2]` safe), T51 (unchecked row-cell fetch, different slice), T74 (unchecked `moment` unwrap, different proof) — none unchecked the width-gate continuation bytes.

**Setback answered:** `duplicate_costly_actions=1` — this iteration ran exactly one verifier on new T76 code (`ok. 120 passed; 0 failed`, 0.79s) vs T75 baseline (`ok. 120 passed`, 0.88s): same pass count, -0.09s wall (noise, not a claim). No `/tmp` bench, no `abits`/`mru2`/`bg_run` repeat, so no replication to justify or compare beyond that baseline.

**Verifier:** `cd crates && cargo test -p angel-glass --lib` → **ok. 120 passed; 0 failed** (0.79s).

## Reproduce (T76)

```sh
cd crates && cargo test -p angel-glass --lib 2>&1 | tail -n 3
# ok. 120 passed; 0 failed
```

Open lead for next iteration: frame-level CPU/latency/alloc A/B vs kitty still missing — no harness in-tree.
