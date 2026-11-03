#!/usr/bin/env node
// Render the Mage's weave spells with pixelfx and fit them to the Delve's inks.
//
//     node scripts/delve/spellfx.mjs [/path/to/pixelfx]
//
// pixelfx (https://github.com/patch-gg/pixelfx) is the argument, else $PIXELFX
// (a checkout of it; required when no argument is given). Each spell is played at the game's rate (15 frames a
// second, two ticks a frame), sized to the spell's reach in the game (8 pixels
// a world unit), and written to `crates/delve/assets/spellfx/<name>.png`: one
// row of frames, or eight rows for a spell cast along the aim (right, then
// clockwise). Every pixel is one of the game's inks and opaque: colour under
// half its alpha is dropped, light adds onto the colour it falls on, and a glow
// with no colour under it stays only where it is bright, so a spell reads as
// hard pixels on any floor with no dithered haze. `spellfx.json` holds each strip's frame size, origin (the floor point
// the spell stands on) and frame count.
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const PFX = process.argv[2] ?? process.env.PIXELFX;
if (!PFX) {
  console.error("spellfx: name a pixelfx checkout, as the argument or $PIXELFX");
  process.exit(2);
}
const OUT = join(ROOT, "crates/delve/assets/spellfx");
const INK_RS = join(ROOT, "crates/delve/src/render/overworld/ink.rs");

const { createFx, createFrame, clearFrame } = await import(pathToFileURL(join(PFX, "src/core/index.js")));
const { encodePNG } = await import(pathToFileURL(join(PFX, "tools/lib/png.mjs")));

const FPS = 15;
const STEAM = ["#1c1c24", "#2e2e38", "#45464f", "#5f616b", "#7d808a", "#9ea1aa", "#c2c5cc", "#eef0f2"];

// name: the pixelfx effect, its options, the stretch of it played (seconds),
// `aim` for a spell that is cast along the Mage's aim, `top` rows cut off the
// top, `sink` pixels the picture stands lower than its origin, `glow` the
// light's weight on colour and `halo` the brightness a light needs to show
// with no colour under it. Reaches are the game's: Hoarfrost 7 units, Static
// Ward 6, Wildfire 10, Scald 9, a burning patch 3.
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

function inks() {
  const s = readFileSync(INK_RS, "utf8");
  const i = s.indexOf("pub fn ink(ch: char) -> Option<Rgb> {");
  const body = s.slice(i, s.indexOf("\n}\n", i));
  const out = [];
  for (const [, chars, h] of body.matchAll(/((?:'.'\s*\|\s*)*'.')\s*=>\s*0x([0-9a-fA-F]{6})/g))
    out.push([parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)]);
  return out;
}
const INKS = inks();
const near = new Map();
function ink(r, g, b) {
  const key = (r << 16) | (g << 8) | b;
  if (!near.has(key)) {
    let best = INKS[0], bd = Infinity;
    for (const c of INKS) {
      const rm = (r + c[0]) / 2, dr = r - c[0], dg = g - c[1], db = b - c[2];
      const d = (2 + rm / 256) * dr * dr + 4 * dg * dg + (2 + (255 - rm) / 256) * db * db;
      if (d < bd) (bd = d), (best = c);
    }
    near.set(key, best);
  }
  return near.get(key);
}
// A pixel of colour `c` and light `l` (pixelfx's 0xAABBGGRR words) as an ink,
// the light weighed `glow` where it falls on colour and shown alone where it
// is `halo` bright, or null where it shows nothing.
function pixel(c, l, glow, halo) {
  const ch = (v, s) => (v >>> s) & 255;
  const lit = [ch(l, 0), ch(l, 8), ch(l, 16)];
  if (ch(c, 24) >= 128) return ink(...[0, 8, 16].map((s, i) => Math.min(255, ch(c, s) + lit[i] * glow)));
  if (Math.max(...lit) >= halo) return ink(...lit);
  return null;
}

// One direction of a spell: its frames as ink RGBA on a canvas of w x h with
// the origin at (ox, oy).
async function play(spec, angle) {
  const mod = await import(pathToFileURL(join(PFX, "src/fx", `${spec.fx}.js`)));
  const [W, H] = mod.meta.size, [OX, OY] = mod.meta.origin;
  const [w, h, ox, oy] = spec.aim ? [spec.aim * 2, spec.aim * 2, spec.aim, spec.aim] : [W, H, OX, OY];
  const frame = createFrame(w, h);
  const fx = createFx(mod, { x: ox, y: oy, angle, ...(spec.opts ?? {}) });
  const n = Math.round((spec.to - spec.from) * FPS);
  if (spec.from > 0) fx.update(spec.from);
  const frames = [];
  for (let i = 0; i < n; i++) {
    if (i > 0) fx.update(1 / FPS);
    clearFrame(frame);
    fx.draw(frame);
    const px = new Uint8ClampedArray(w * h * 4);
    for (let y = spec.top ?? 0; y < h; y++)
      for (let x = 0; x < w; x++) {
        const i = y * w + x, c = pixel(frame.color[i], frame.light[i], spec.glow ?? 1, spec.halo ?? 160);
        if (c) (px[i * 4] = c[0]), (px[i * 4 + 1] = c[1]), (px[i * 4 + 2] = c[2]), (px[i * 4 + 3] = 255);
      }
    frames.push(px);
  }
  return { w, h, ox, oy, frames };
}

// The box every frame of every direction shares, so a strip is no wider than
// what it draws.
function trim(dirs) {
  const { w, h } = dirs[0];
  let x0 = w, y0 = h, x1 = -1, y1 = -1;
  for (const d of dirs)
    for (const px of d.frames)
      for (let y = 0; y < h; y++)
        for (let x = 0; x < w; x++)
          if (px[(y * w + x) * 4 + 3]) (x0 = Math.min(x0, x)), (x1 = Math.max(x1, x)), (y0 = Math.min(y0, y)), (y1 = Math.max(y1, y));
  return [x0, y0, x1 - x0 + 1, y1 - y0 + 1];
}

mkdirSync(OUT, { recursive: true });
const manifest = {};
for (const [name, spec] of Object.entries(SPELLS)) {
  const dirs = [];
  for (const k of spec.aim ? [0, 1, 2, 3, 4, 5, 6, 7] : [0]) dirs.push(await play(spec, (k * Math.PI) / 4));
  const [x0, y0, fw, fh] = trim(dirs);
  const n = dirs[0].frames.length, sw = fw * n, sh = fh * dirs.length;
  const sheet = new Uint8ClampedArray(sw * sh * 4);
  dirs.forEach((d, row) =>
    d.frames.forEach((px, f) => {
      for (let y = 0; y < fh; y++)
        for (let x = 0; x < fw; x++) {
          const s = ((y + y0) * d.w + x + x0) * 4, t = ((row * fh + y) * sw + f * fw + x) * 4;
          for (let c = 0; c < 4; c++) sheet[t + c] = px[s + c];
        }
    }),
  );
  writeFileSync(join(OUT, `${name}.png`), encodePNG(sw, sh, sheet));
  manifest[name] = { fx: spec.fx, w: fw, h: fh, ox: dirs[0].ox - x0, oy: dirs[0].oy - y0 - (spec.sink ?? 0), frames: n, dirs: dirs.length };
  console.log(`${name.padEnd(12)} ${spec.fx.padEnd(28)} ${n} frames x ${dirs.length}  ${fw}x${fh}`);
}
writeFileSync(join(OUT, "spellfx.json"), JSON.stringify(manifest, null, 1) + "\n");
