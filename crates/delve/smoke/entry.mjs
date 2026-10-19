// Fresh module per game/kit: first two seconds plus a warm two seconds.
// node entry.mjs <angel_delve_smoke.wasm> [--warm]
import { readFileSync } from "node:fs";
const bytes = readFileSync(process.argv[2]);
for (let kind = 3; kind <= 8; kind++) {
  for (let kit = 0; kit < 2; kit++) {
    const { instance } = await WebAssembly.instantiate(bytes, {});
    const e = instance.exports;
    e.scene(kind, kit);
    const state = e.state_hash(0);
    const t = performance.now();
    if (process.argv.includes("--warm")) e.warm_entry(kind);
    const preparation_ms = performance.now() - t;
    if (state !== e.state_hash(0)) throw Error("entry preparation changed the wire state");
    const ms = [], sums = [];
    for (let n = 0; n < 240; n++) {
      if (n > 0 && n % 2 === 0) e.step(1);
      const start = performance.now();
      sums.push(e.frame(kit ? 768 : 384, kit ? 448 : 224) >>> 0);
      ms.push(performance.now() - start);
    }
    console.log(JSON.stringify({kind, kit: kit + 1, preparation_ms, ms, sums}));
  }
}
