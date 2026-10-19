// The Delve's frames in wasm under node, a tick stepped before each, the
// frame alone timed by node's clock:
//   cargo build -p angel-delve-smoke --target wasm32-unknown-unknown --release
//   node crates/delve/smoke/smoke.mjs <path to angel_delve_smoke.wasm>
import { readFileSync } from "node:fs";

const bytes = readFileSync(process.argv[2]);
const { instance } = await WebAssembly.instantiate(bytes, {});
const { scene: setScene, force_rage, force_ravage, step, frame, frame_live, wire_scene, wire_host, wire_client } = instance.exports;
const rage = process.argv.includes("--rage");
const ravage = process.argv.includes("--ravage");
const party = process.argv.includes("--party");
const scene = (kind, art) => {
  setScene(kind, art);
  if (party) instance.exports.party();
  if (rage) force_rage();
  if (ravage) force_ravage();
  // Match client boot plus its first snapshot; raw admission remains measurable.
  if (!process.argv.includes("--unprepared")) instance.exports.warm_entry(kind);
};
const scenes = ["first-room", "kingdom", "busy-fight", "cinder-stack", "latch-duel", "mossbell", "brasswake", "rafterwake", "pennon-rush", "cinderjaw", "the-index", "mother-of-spores", "waxen-warden", "the-bone-choir", "crypt-tithe-marshal", "crypt-tax-keeper", "crypt-relic-liege", "iron-tusk-chief", "red-pick-chief", "the-foreman", "late-fee-leviathan", "side-hall", "book-hop"];
// The Round Table adventures' stubs, scene kinds 39 to 43 (the holes between
// are the boss and forest scenes this script does not time).
["lanternward", "moonfall", "bannerpeak", "griffinwing", "lightgallop"].forEach((name, i) => { scenes[39 + i] = name; });
const hex = (n) => n.toString(16).padStart(8, "0");
for (const [artV2, w, h] of [[0, 384, 224], [1, 768, 448]]) {
  scenes.forEach((name, kind) => {
    if (rage && (kind < 9 || kind > 20)) return;
    if (ravage && name !== "late-fee-leviathan") return;
    scene(kind, artV2);
    let t = performance.now();
    const sum = frame(w, h) >>> 0;
    const cold = performance.now() - t;
    const n = 60;
    let warm = 0;
    let last = sum;
    for (let i = 0; i < n; i++) {
      step(1);
      t = performance.now();
      last = frame(w, h) >>> 0;
      warm += performance.now() - t;
    }
    warm /= n;
    console.log(
      `wasm ${w}x${h} v${artV2 + 1} ${name}: first ${cold.toFixed(1)} ms, then ${warm.toFixed(2)} ms a frame, sums ${hex(sum)} ${hex(last)}`,
    );
  });
}
// Frames as a client draws them (live), at 60 a second with a tick every
// other one: the slowest twentieth, and the slowest, against the exact
// frames' (where every sixth tick redraws the room's lit scenery at once).
for (const [artV2, w, h] of [[0, 384, 224], [1, 768, 448]]) {
  scenes.forEach((name, kind) => {
    if (rage && (kind < 9 || kind > 20)) return;
    if (ravage && name !== "late-fee-leviathan") return;
    const spread = draw => {
      scene(kind, artV2);
      for (let i = 0; i < 120; i++) { step(1); draw(w, h); }
      const ts = [];
      for (let i = 0; i < 360; i++) {
        if (i % 2 === 0) step(1);
        const t = performance.now();
        draw(w, h);
        ts.push(performance.now() - t);
      }
      ts.sort((a, b) => a - b);
      return `p95 ${ts[Math.floor(ts.length * 0.95)].toFixed(2)} max ${ts[ts.length - 1].toFixed(2)} ms`;
    };
    console.log(`wasm ${w}x${h} v${artV2 + 1} ${name} at 60 fps: exact ${spread(frame)}, live ${spread(frame_live)}`);
  });
}

// The client's half of the wire a tick: the patch read and applied, the
// tree read back into a run. Each scene is taken in for a while first, so
// the engine has compiled the reading code fully (as a page that has
// played a few seconds has), then timed.
scenes.forEach((name, kind) => {
  if (rage || ravage || party) return;
  const whole = wire_scene(kind);
  for (let i = 0; i < 240; i++) {
    wire_host();
    wire_client();
  }
  const n = 120;
  let took = 0, bytes = 0;
  for (let i = 0; i < n; i++) {
    bytes += wire_host();
    const t = performance.now();
    wire_client();
    took += performance.now() - t;
  }
  console.log(`wasm wire ${name}: whole ${whole} bytes, then ${Math.round(bytes / n)} bytes and ${(took / n).toFixed(3)} ms a tick to take in`);
});
