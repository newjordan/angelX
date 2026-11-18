#!/usr/bin/env node
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { FPS, inks, init, makePixel, pack, play, trim } from "./fxfit.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const PFX = process.argv[2] ?? process.env.PIXELFX;
if (!PFX) {
  console.error("spellfx: name a pixelfx checkout, as the argument or $PIXELFX");
  process.exit(2);
}
const OUT = join(ROOT, "crates/delve/assets/spellfx");
const INK_RS = join(ROOT, "crates/delve/src/render/overworld/ink.rs");

const { encodePNG } = await init(PFX);

const STEAM = ["#1c1c24", "#2e2e38", "#45464f", "#5f616b", "#7d808a", "#9ea1aa", "#c2c5cc", "#eef0f2"];

const SPELLS = {
  sunfall: { fx: "fire/meteor", from: 0, to: 1.8 },
  hoarfrost: { fx: "ice/frost-nova", opts: { reach: 56 }, from: 0.2, to: 2.6 },
  thunderhead: { fx: "lightning/lightning-strike", from: 0.75, to: 2.4, top: 160, glow: 0.4, halo: 230 },
  scald: { fx: "ice/frost-breath", opts: { palette: STEAM, spread: 0.5, power: 0.6 }, glow: 0.25, from: 0, to: 0.8, aim: 84 },
  wildfire: { fx: "fire/flamethrower", opts: { reach: 80 }, from: 0, to: 0.5, aim: 104 },
  "frost-ring": { fx: "ice/snowflake-burst", from: 0.1, to: 2.2, sink: 44 },
  sleet: { fx: "ice/blizzard-vortex", opts: { height: 48 }, from: 0, to: 2 },
  "static-ward": { fx: "lightning/thunderclap", opts: { reach: 48 }, from: 0.1, to: 1.4 },
  cataclysm: { fx: "explosions/plasma-explosion", from: 0.05, to: 1.3, halo: 250 },
  burn: { fx: "fire/fire-ring", opts: { reach: 24 }, from: 0.45, to: 3.4 },
};

const pixel = makePixel(inks(INK_RS));
void FPS;

mkdirSync(OUT, { recursive: true });
const manifest = {};
for (const [name, spec] of Object.entries(SPELLS)) {
  const dirs = [];
  for (const k of spec.aim ? [0, 1, 2, 3, 4, 5, 6, 7] : [0]) dirs.push(await play(PFX, { ...spec, pixel }, (k * Math.PI) / 4));
  const [x0, y0, fw, fh] = trim(dirs);
  const packed = pack(dirs, [x0, y0, fw, fh]);
  writeFileSync(join(OUT, `${name}.png`), encodePNG(packed.sw, packed.sh, packed.sheet));
  manifest[name] = { fx: spec.fx, w: fw, h: fh, ox: dirs[0].ox - x0, oy: dirs[0].oy - y0 - (spec.sink ?? 0), frames: packed.n, dirs: dirs.length };
  console.log(`${name.padEnd(12)} ${spec.fx.padEnd(28)} ${packed.n} frames x ${dirs.length}  ${fw}x${fh}`);
}
writeFileSync(join(OUT, "spellfx.json"), JSON.stringify(manifest, null, 1) + "\n");
