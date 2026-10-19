# Forest Road review

`main.rs` plays a real four-class party from entry through Old Oak's release.
It exports the start, war-wagon, Feller and freeing states, native PNGs and
actual 96×28 buffers from the cockpit's shared cell painter and text panels.
It also verifies the freed Ent on the kingdom map and measures four native views and one terminal view, retaining a final
state hash. `render.py OUTPUT` puts the native and cell views on one sheet.

Run builds with the task's TMPDIR, ANGEL_CARTRIDGES and assigned target cache,
inside `systemd-run --user --scope -q -p MemoryMax=40G nice`:

```
cargo run --manifest-path scripts/forest-review/Cargo.toml --release -- OUTPUT
python3 scripts/forest-review/render.py OUTPUT
```

The capture tool uses the explicit arcade evidence feature. Production checks
use normal features. The smoke's new scene is index 37: run the native smoke
with `--entry 37 0 --party --warm > native-smoke.json`, then compare the normal
WASM smoke with `node scripts/forest-review/wasm.mjs WASM native-smoke.json OUT`.
No previous scene or recorded replay baseline is re-recorded.
