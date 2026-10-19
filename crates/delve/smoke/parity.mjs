// Read-only parity evidence using the timer's same 60-tick horizon.
// node parity.mjs angel_delve_smoke.wasm BOSS_ID
import { readFileSync } from "node:fs";
const {instance} = await WebAssembly.instantiate(readFileSync(process.argv[2]), {});
const {scene,plain_art,force_rage,step,state_hash,frame,hitstop_off,party} = instance.exports;
const names = ["first-room", "kingdom", "busy-fight", "cinder-stack", "latch-duel", "mossbell", "brasswake", "rafterwake", "pennon-rush", "cinderjaw", "the-index", "mother-of-spores", "waxen-warden", "the-bone-choir", "crypt-tithe-marshal", "crypt-tax-keeper", "crypt-relic-liege", "iron-tusk-chief", "red-pick-chief", "the-foreman", "late-fee-leviathan", "side-hall", "book-hop"];
// The Round Table adventures' stubs are scene kinds 39 to 43.
["lanternward", "moonfall", "bannerpeak", "griffinwing", "lightgallop"].forEach((name, i) => { names[39 + i] = name; });
const kind = names.indexOf(process.argv[3]);
if (kind < 0) throw new Error("parity needs a smoke scene ID");
const off = process.argv.includes("--off"), four = process.argv.includes("--party");
const hex = n => (n >>> 0).toString(16).padStart(8,"0");
for (const art of [0,1]) for (const rage of [false,true]) for (const plain of [false,true]) {
  scene(kind,art);
  if (off) hitstop_off();
  if (four) party();
  if (plain) plain_art();
  if (rage) force_rage();
  for (let tick=0; tick<=60; tick++) {
    if (tick) step(1);
    const [w,h] = art ? [768,448] : [384,224];
    console.log(`${art},${rage},${plain},${tick},${hex(state_hash(off ? 6 : 0))},${hex(state_hash(1))},${hex(state_hash(2))},${hex(state_hash(3))},${state_hash(4)},${hex(frame(w,h))}`);
  }
}
