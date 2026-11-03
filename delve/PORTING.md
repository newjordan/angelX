# Porting Delve's Rust rules to Bend — the contract

Read `docs/architecture/delve-in-bend.md` first. This file is how every port
agent writes, builds and proves its share. The job is a faithful translation of
whole Rust modules into one Bend program — not decision legs, not a JS adapter,
not new Rust glue. Product Rust gets deleted when the whole Run replays green.

## Layout

```
delve/lib/          imported libraries (alias in brackets) — do not edit others' libs
  j.bend    [J]     the state tree: J.Val, J.get/set/put/at/nth/items/fields/u32/f32/...
  bj.bend   [B]     BJ1 read/show
  delta.bend [D]    delta ops
  w64.bend  [W]     u64 arithmetic (W.W{hi, lo})            — core-numerics agent
  rs.bend   [RS]    Rust-exact scalar ops: i32, casts, f32  — core-numerics agent
delve/rules/<group>/*.bend   FRAGMENTS: your translated code (no imports)
delve/stubs/<group>.bend     placeholder defs for names other groups own
delve/build.py               concatenates lib imports + fragments (+ needed stubs)
delve/build/                 generated, never edit, never commit
delve/tests/                 case runner
```

## Fragments

A fragment is a `.bend` file with **no `import` lines**: only `type` and `def`.
`delve/build.py` concatenates every fragment into `delve/build/rules.bend`
under one header that imports Base and the libs above (use the aliases `J`,
`B`, `D`, `W`, `RS`). In one file, order does not matter because the build marks
every def `@unsafe` (it appends `?` to each def name); write defs plainly or
with `?`, either works. Recursion (and mutual recursion) is free.

**Names.** Every def is named after where it lives in Rust:

- a free function `fn f` in `crates/delve/src/rules/foo/bar.rs` → `foo.bar.f`
- a method `fn m` in an `impl Type` block in that file → `foo.bar.Type.m`
- `rules.rs` is module `rules` (so `Run::step` is `rules.Run.step`); `world/x.rs`
  is `world.x`; `realm/x.rs` is `realm.x`; `rules/x.rs` is `x` (no `rules.` prefix)
- helpers you invent: `foo.bar.f.go`, `foo.bar.f.if`, ... (suffixes on the owner)
- Bend types you declare: `Foo.Bar.TypeName` style is NOT allowed for types —
  types are plain identifiers: prefix them with your module, e.g. `BarShot`.

When you call a Rust function another group owns, call it by this name. If it
is not ported yet, add a stub with the exact signature to
`delve/stubs/<your group>.bend`; the build includes a stub only while no
fragment defines that name. Stubs answer a harmless default; never rely on one
in a case you claim passes.

## Values and signatures

The Run is the J tree of `runtime_state::capture_for_rules` — the serde shape:
the same keys, enum spellings (`#[serde(rename_all)]`, tags), omitted fields
(`skip_serializing_if`), `Option::None` as `null` or absent exactly as serde
writes it, unit enum variants as strings, newtypes as their inner value, tuples
as arrays, `BTreeMap<u32, _>` as objects keyed by the decimal id. Read missing
fields as their Rust `Default` (J.u32 → 0, J.bool → False, J.items → []), but
WRITE exactly what serde would write — the host restores your output into the
Rust `Run`, and replay compares key by key.

| Rust | Bend |
| --- | --- |
| `&self` on Run | `+run: J.Val` (the `run` object, not the `{schema, run}` checkpoint) |
| `&mut self` on Run | `+run: J.Val` in, the new run out (`-> J.Val`, or `-> J.Val & R` when it also returns `R`) |
| `&self`/`&mut self` on another struct | that struct's J value, same rule |
| `u32`, `usize`, `u8`, `u16` | `U32` (wrapping as Rust release builds wrap; `saturating_*` via RS) |
| `i32`, `i8`, `i16`, `isize`, `i64` (small) | `U32` two's complement, ops via `RS.i_*` |
| `u64` | `W.W` (`W.W{hi, lo}`), ops via `W.*` |
| `f32` | `F32`; Rust-exact methods via `RS.f_*` (see below) |
| `bool` | `Bool` |
| `String`, `&str` | `String` |
| `Option<T>` | `Maybe<&2, T>` |
| `Vec<T>`, slices | `List<&2, T>` (or the J array when it stays in the state) |
| structs/enums held in state | `J.Val` in serde shape |
| `BTreeMap<u32, V>` iteration | **numeric key order**: use `J.int_fields` (sorted by numeric key) — Bend Maps iterate in string order ("10" < "2") |

Floats must be bit-exact with Rust. Use the libm-backed primitives (`F32.sin`,
`cos`, `atan2`, `sqrt`, `floor`, `ceil`, `trunc` match glibc as Rust's do), and
for Rust methods whose semantics differ from Bend's Base use RS:
`RS.f_hypot` (glibc hypotf; NOT `F32.hypot`), `RS.f_round` (half away from
zero; NOT `F32.round`), `RS.f_powi`, `RS.f_signum`, `RS.f_rem_euclid`,
`RS.f_min`/`RS.f_max` (Rust NaN rules), `RS.f_clamp`, `RS.f_as_u32`,
`RS.f_as_i32`, `RS.f_as_usize` (saturating casts, NaN → 0), `RS.u_as_f32`,
`RS.i_as_f32`, `RS.f_abs`. Keep Rust's operation order: `a * b + c` is two
roundings, never reassociate. The Run's generator is `loot::Rng` (xorshift64*):
port it as `loot.Rng.next` etc. on `W.W`.

## Rust globals

`angel_delve::settings::get()` and the installed phrasebook are thread-local
globals in Rust, set by the worker from the event before each step. In Bend
they ride on the run for the tick: read `J.get(run, "$settings")` and
`J.get(run, "$phrasebook")`; `delve/step.bend` puts them there from the event
and strips them from the reply. Other `thread_local!` caches in the rules are
caches: recompute instead (say so in a comment if a cache carried state).

## Proving a port: cases

Every ported function with a meaningful body gets cases recorded from the Rust
original:

1. A test-only Rust recorder for the module it tests, in
   `crates/delve/tests/rules/<module>_bend_cases__tests.rs` (a `__tests` name
   keeps it out of the product line count), mounted from the module's file as
   the crate already mounts its tests:
   `#[cfg(all(test, feature = "bend-cases-<group>"))] #[path = "<relative>/tests/rules/<module>_bend_cases__tests.rs"] mod <module>_bend_cases_tests;`
   (group names with `_` take `-` in the feature: `bend-cases-games-a`). It builds realistic inputs —
   prefer Runs restored from the replay corpus (`artifacts/delve-replay/`,
   gzip'd JSONL; `runtime_state::restore_value`) and the scenario setups in
   `crates/delve/tests/rules/` — calls the Rust function, and appends
   `{"fn": "<bend name>", "in": <json>, "out": <json>}` lines to
   `$DELVE_CASES/<bend name>.jsonl` (`DELVE_CASES` must be set).
   Run it with `cargo test --manifest-path crates/Cargo.toml -p angel-delve --profile fast
   --features bend-cases-<group> --lib <filter> -- --ignored` (mark recorders
   `#[ignore]`). Your recorders compile only under your group's feature, so
   another group's broken recorder never stops yours — and yours must never
   break the default build. A struct that is not
   `Serialize` may gain `#[cfg_attr(test, derive(serde::Serialize))]`.
2. A Bend case wrapper in your fragment: `def case.<bend name>(+input: J.Val) -> J.Val`
   that unpacks `in`, calls your def, and packs the result in the same JSON
   shape as `out`.
3. `python3 delve/tests/cases.py <group> [<bend name>...]` builds the program,
   runs every case file of those names through the wrappers and prints
   pass/fail with the first differing path. A function is done when all its
   cases pass. Aim for ≥ 50 cases per function from varied real states.

## Building

- `python3 delve/build.py [--only <group> ...]` → `delve/build/rules.bend`, then
  `bend delve/build/rules.bend` checks it. `--only` includes just your group's
  fragments (+ stubs for everything else), so another group's half-written
  file never blocks you. The full build (`python3 delve/build.py`) must check
  before you finish.
- Bend: `~/.cache/angel/bend2/221a350c2f7f7134884740018e6e90001fb6d230/bin/bend`
  (read its `guide/GUIDE.md`). Wrap every bend/node run in `timeout 300`.
- Rust: the pinned toolchain (rustc 1.95), one build at a time, with a
  memory cap; point `CARGO_TARGET_DIR` at a shared build cache.

## Bend 2 pitfalls (each cost us a compile)

- `match` scrutinizes only a parameter or a pattern-bound variable, never a
  computed value: `match f(x):` is refused — pass `f(x)` to a helper whose
  parameter you match. No `let` before a `match` on a parameter.
- No `if`: match `True{}`/`False{}`, or `Bool.pick(T, c, a, b)` (both arms are
  evaluated — keep them cheap and total).
- Affine by default: a value is used once unless the parameter is `+x`
  (Data only). `J.Val`, `U32`, `F32`, `String`, `List<&2, _>` are Data.
- Base generics need explicit type args: `List.length(&2, J.Val, xs)`,
  `List.reverse(&2, T, xs)`, `Map.get(V, default, m, key)` (it hands the map
  back: `(m, v)`).
- Operators need a type: `(a + b : U32)`, `(x * y : F32)`. U32 literals are
  plain (`42`), F32 have a dot (`1.0`), Nat end in `n`.
- Char patterns work (`case SCon{'a', t}:`); string literal patterns do not.
- A pair type in a list is `Sigma<&2, &2, String, _ => J.Val>` (see J.fields).

## Scope and done

Your group's Rust modules are listed in `delve/GROUPS.md`. Port every function
reachable from `Run::step` (and every compute contract) in them; skip
`#[cfg(test)]` code, Debug/Display impls, and host-only paths the worker never
runs (say which in your report). Commit your own paths only (`git add
delve/rules/<group> delve/stubs/<group>.bend crates/.../*_bend_cases_tests.rs`),
often, as Jordan Newman (author and committer), never
with any AI attribution. Do not delete Rust: deletion happens at the flip, when
the whole Run replays green.
