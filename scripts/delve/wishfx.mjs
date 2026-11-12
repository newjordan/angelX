#!/usr/bin/env node
// Bake the wished casts (7 shapes x 7 elements) as pixelfx strips sized to the
// zone's level-2 reach. Level 2 is the bake; the game scales levels 1 and 3 by
// nearest-neighbour when it stamps.
//
//     node scripts/delve/wishfx.mjs [/path/to/pixelfx]
import { mkdirSync, readFileSync, unlinkSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { FPS, inks, init, makePixel, pack, trim } from "./fxfit.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const PFX = process.argv[2] ?? process.env.PIXELFX;
if (!PFX) {
  console.error("wishfx: name a pixelfx checkout, as the argument or $PIXELFX");
  process.exit(2);
}
const OUT = join(ROOT, "crates/delve/assets/wishfx");
const SHEET = join(ROOT, ".we-out");
const INK_RS = join(ROOT, "crates/delve/src/render/overworld/ink.rs");
const TABLE = JSON.parse(readFileSync(join(ROOT, "scripts/delve/wishfx.json"), "utf8"));

// Level-2 reach in pixels (8 px per world unit). rules::zones::reach, size 2.
const REACH = { wave: 96, burst: 32, cone: 68, line: 104, field: 38, pillar: 23, nova: 72 };
const SHAPES = ["wave", "burst", "cone", "line", "field", "pillar", "nova"];
const ELEMENTS = ["fire", "frost", "storm", "poison", "holy", "arcane", "earth"];
const MAX_FRAMES = 12;
const FONT = {
  a: "010101111101101", b: "110101110101110", c: "011100100100011", d: "110101101101110",
  e: "111100110100111", f: "111100110100100", g: "011100101101011", h: "101101111101101",
  i: "111010010010111", l: "100100100100111", m: "101111111101101", n: "110101101101101",
  o: "010101101101010", p: "110101110100100", r: "110101110101101", s: "011100010001110",
  t: "111010010010010", u: "101101101101011", v: "101101101101010", w: "101101111111101",
  y: "101101010010010",
};

const { createFx, createFrame, clearFrame, encodePNG } = await init(PFX);
const pixel = makePixel(inks(INK_RS));

function shapeOf(name) {
  return name.slice(0, name.indexOf("-"));
}

async function loadMod(fx) {
  return import(pathToFileURL(join(PFX, "src/fx", `${fx}.js`)));
}

function resolved(spec) {
  const opts = { ...(spec.opts ?? {}) };
  return opts;
}

// Draw `n` frames. Looping fields step across one full effect loop so the
// strip joins. One-shots play [from, to) at 15 fps, capped at MAX_FRAMES.
async function sample(spec, angle, mod) {
  const [W, H] = mod.meta.size, [OX, OY] = mod.meta.origin;
  let w = W, h = H, ox = OX, oy = OY;
  if (spec.aim) {
    const rad = Math.ceil(Math.max(OX, W - OX, OY, H - OY, 8) + 4);
    w = h = rad * 2;
    ox = oy = rad;
  }
  const frame = createFrame(w, h);
  const opts = { ...(spec.opts ?? {}) };
  if (typeof opts.palette === "string") {
    const { PAL } = await init(PFX);
    opts.palette = PAL[opts.palette];
  }
  const fx = createFx(mod, { x: ox, y: oy, angle, ...opts });
  const loop = spec.loop && mod.meta.loop && mod.meta.duration > 0;
  let n, times;
  if (loop) {
    n = Math.min(MAX_FRAMES, Math.max(8, Math.round(Math.min(mod.meta.duration, 2) * FPS)));
    const step = mod.meta.duration / n;
    times = Array.from({ length: n }, (_, i) => i * step);
  } else {
    n = Math.min(spec.aim ? 8 : MAX_FRAMES, Math.max(1, Math.round((spec.to - spec.from) * FPS)));
    const span = spec.to - spec.from;
    times = Array.from({ length: n }, (_, i) => spec.from + (i * span) / n);
  }
  const frames = [];
  let t = 0;
  for (const target of times) {
    const dt = target - t;
    if (dt > 0) fx.update(dt);
    t = target;
    clearFrame(frame);
    fx.draw(frame);
    const px = new Uint8ClampedArray(w * h * 4);
    for (let y = spec.top ?? 0; y < h; y++)
      for (let x = 0; x < w; x++) {
        const i = y * w + x;
        const c = pixel(frame.color[i], frame.light[i], spec.glow ?? 1, spec.halo ?? 160);
        if (c) (px[i * 4] = c[0]), (px[i * 4 + 1] = c[1]), (px[i * 4 + 2] = c[2]), (px[i * 4 + 3] = 255);
      }
    frames.push(px);
  }
  return { w, h, ox, oy, frames, loop };
}

function measure(d) {
  let n = 0, sx = 0, sy = 0, maxF = 0, maxH = 0, maxR = 0;
  const { w, h, ox, oy } = d;
  for (const px of d.frames)
    for (let y = 0; y < h; y++)
      for (let x = 0; x < w; x++)
        if (px[(y * w + x) * 4 + 3]) {
          const dx = x - ox, dy = y - oy;
          n++;
          sx += dx;
          sy += dy;
          maxF = Math.max(maxF, dx);
          maxH = Math.max(maxH, Math.abs(dx));
          maxR = Math.max(maxR, Math.hypot(dx, dy));
        }
  return { n, mean: n ? sx / n : 0, maxF, maxH, maxR };
}

function scaleDir(d, scale) {
  if (Math.abs(scale - 1) < 0.004) return d;
  const w = Math.max(1, Math.round(d.w * scale));
  const h = Math.max(1, Math.round(d.h * scale));
  const frames = d.frames.map((px) => {
    const out = new Uint8ClampedArray(w * h * 4);
    for (let y = 0; y < h; y++) {
      const sy = Math.min(d.h - 1, Math.floor((y * d.h) / h));
      for (let x = 0; x < w; x++) {
        const sx = Math.min(d.w - 1, Math.floor((x * d.w) / w));
        const s = (sy * d.w + sx) * 4, t = (y * w + x) * 4;
        out[t] = px[s];
        out[t + 1] = px[s + 1];
        out[t + 2] = px[s + 2];
        out[t + 3] = px[s + 3];
      }
    }
    return out;
  });
  return { w, h, ox: d.ox * scale, oy: d.oy * scale, frames };
}

function peakOf(d) {
  let best = null;
  for (const px of d.frames) {
    let n = 0, minx = d.w, miny = d.h, maxx = -1, maxy = -1, maxF = 0, up = 0;
    for (let y = 0; y < d.h; y++)
      for (let x = 0; x < d.w; x++)
        if (px[(y * d.w + x) * 4 + 3]) {
          n++;
          if (x < minx) minx = x;
          if (y < miny) miny = y;
          if (x > maxx) maxx = x;
          if (y > maxy) maxy = y;
          maxF = Math.max(maxF, x - d.ox);
          up = Math.max(up, d.oy - y);
        }
    if (!n) continue;
    const bw = maxx - minx + 1, bh = maxy - miny + 1;
    const stat = { n, bw, bh, maxF, up, fill: n / (bw * bh) };
    if (!best || n > best.n) best = stat;
  }
  return best ?? { n: 0, bw: 0, bh: 0, maxF: 0, up: 0, fill: 0 };
}

function blankRow() {
  const rgba = new Uint8ClampedArray(8 * 8 * 4);
  return { w: 8, h: 8, frames: 1, sheetW: 8, rgba, none: true };
}

const notes = [];
const extents = [];
const failures = [];
mkdirSync(OUT, { recursive: true });
mkdirSync(SHEET, { recursive: true });
const manifest = {};
const east = {};
let pillarTall = 96;

for (const shape of SHAPES) {
  for (const element of ELEMENTS) {
    const name = `${shape}-${element}`;
    const spec = TABLE[name];
    if (!spec) {
      east[name] = blankRow();
      notes.push(`${name}: no strip, code body only`);
      continue;
    }
    const reach = REACH[shape];
    const mod = await loadMod(spec.fx);
    let spin = 0;
    if (spec.aim) {
      const probe = await sample(spec, 0, mod);
      const m = measure(probe);
      if (m.n > 0 && m.mean < -4) {
        spin = Math.PI;
        notes.push(`${name}: spun ${spec.fx} by half a turn so the art runs east (it was drawn behind its origin)`);
      }
    }
    const angles = spec.aim ? [0, 1, 2, 3, 4, 5, 6, 7].map((k) => (k * Math.PI) / 4 + spin) : [spin];
    let dirs = [];
    for (const angle of angles) dirs.push(await sample(spec, angle, mod));
    const peak = peakOf(dirs[0]);
    let scale = 1;
    let measured = 0;
    let need = 0;
    if (shape === "nova" || shape === "burst" || shape === "field") {
      measured = Math.min(peak.bw, peak.bh);
      need = 0.8 * 2 * reach;
      if (measured > 0 && measured < need + 1) scale = (need + 2) / measured;
    } else if (shape === "cone" || shape === "line") {
      measured = peak.maxF;
      need = 0.85 * reach;
      if (measured > 0 && measured < need + 1) scale = (need + 2) / measured;
    } else if (shape === "pillar") {
      measured = peak.up;
      need = pillarTall;
      if (measured > 0 && measured < need) scale = need / measured;
    }
    if (scale > 4) {
      failures.push(`${name}: scale ${scale.toFixed(2)} would dither the strip (measured ${measured.toFixed(1)} need ${need.toFixed(1)})`);
      scale = 1;
    }
    // Never resample: a strip is baked at the effect's own size, hard pixels only.
    // The code-drawn body under it carries the reach; the strip is the hero piece.
    scale = 1;
    dirs = dirs.map((d) => scaleDir(d, scale));
    const after = peakOf(dirs[0]);
    if (shape === "pillar" && element === "fire" && after.up > pillarTall) pillarTall = after.up;
    const side = Math.min(after.bw, after.bh);
    const row = {
      name,
      bbox: side,
      bboxNeed: shape === "nova" || shape === "burst" || shape === "field" ? 0.8 * 2 * reach : null,
      length: after.maxF,
      lengthNeed: shape === "cone" || shape === "line" ? 0.85 * reach : null,
      up: after.up,
      fill: after.fill,
      scale,
    };
    extents.push(row);
    if (after.n === 0) failures.push(`${name}: peak frame has no opaque pixels`);
    else if (after.fill < 0.1) failures.push(`${name}: peak fill ${(after.fill * 100).toFixed(1)}% of bbox ${after.bw}x${after.bh} (need 10%)`);
    if (row.bboxNeed != null && side + 0.5 < row.bboxNeed)
      failures.push(`${name}: peak bbox ${side.toFixed(1)}px < 80% of reach diameter ${row.bboxNeed.toFixed(1)}px`);
    if (row.lengthNeed != null && after.maxF + 0.5 < row.lengthNeed)
      failures.push(`${name}: peak length ${after.maxF.toFixed(1)}px < 85% of reach ${row.lengthNeed.toFixed(1)}px`);
    const box = trim(dirs);
    const packed = pack(dirs, box);
    if (packed.fw < 2 || packed.fh < 2) notes.push(`${name}: trimmed to ${packed.fw}x${packed.fh}`);
    writeFileSync(join(OUT, `${name}.png`), encodePNG(packed.sw, packed.sh, packed.sheet));
    const [x0, y0] = box;
    manifest[name] = {
      fx: spec.fx,
      w: packed.fw,
      h: packed.fh,
      ox: Math.round(dirs[0].ox - x0),
      oy: Math.round(dirs[0].oy - y0 - (spec.sink ?? 0)),
      frames: packed.n,
      dirs: packed.dirs,
      reach,
      loop: !!spec.loop,
    };
    east[name] = { w: packed.fw, h: packed.fh, frames: packed.n, sheetW: packed.sw, rgba: packed.sheet };
    console.log(`${name.padEnd(16)} ${spec.fx.padEnd(28)} ${packed.n}f x ${packed.dirs}  ${packed.fw}x${packed.fh}  scale ${scale.toFixed(2)}  bbox ${side.toFixed(0)}  len ${after.maxF.toFixed(0)}  up ${after.up.toFixed(0)}  fill ${(after.fill * 100).toFixed(0)}%${spin ? " spin" : ""}`);
  }
  writeSheet(shape);
}

writeFileSync(join(OUT, "wishfx.json"), JSON.stringify(manifest, null, 1) + "\n");
const table = ["name bbox need length lenNeed up fill scale", ...extents.map((r) =>
  `${r.name} bbox=${r.bbox.toFixed(1)} need=${r.bboxNeed == null ? "-" : r.bboxNeed.toFixed(1)} length=${r.length.toFixed(1)} lenNeed=${r.lengthNeed == null ? "-" : r.lengthNeed.toFixed(1)} up=${r.up.toFixed(1)} fill=${(r.fill * 100).toFixed(1)}% scale=${r.scale.toFixed(2)}`)];
console.log("\n" + table.join("\n"));
writeFileSync(join(OUT, "NOTES.txt"), [...notes, "", ...table, "", ...failures].join("\n") + "\n");
writeFileSync(join(SHEET, "EXTENTS.txt"), table.join("\n") + "\n");
for (const name of ["wave-fire", "wave-frost", "wave-storm", "wave-poison", "wave-holy", "wave-arcane"]) {
  try { unlinkSync(join(OUT, `${name}.png`)); } catch {}
}

const arms = Object.keys(manifest)
  .map((name) => `        "${name}" => include_bytes!("../../../../assets/wishfx/${name}.png"),`)
  .join("\n");
writeFileSync(
  join(ROOT, "crates/delve/src/render/overworld/arena/wishfx_png.rs"),
  `// Generated by scripts/delve/wishfx.mjs. Do not edit by hand.\npub(super) fn png(name: &str) -> &'static [u8] {\n    match name {\n${arms}\n        _ => &[],\n    }\n}\n`,
);
if (failures.length) {
  console.error(failures.join("\n"));
  console.warn(`${failures.length} strip(s) are smaller than their zone; the code body carries the reach`);
}

function writeSheet(shape) {
  const rows = ELEMENTS.map((element) => east[`${shape}-${element}`]);
  const cellW = Math.max(...rows.map((r) => r.w)) * 2;
  const label = 78;
  const gap = 4;
  const heads = 16;
  let y = heads;
  const heights = rows.map((r) => Math.max(14, r.h * 2));
  const sheetH = heads + heights.reduce((a, b) => a + b + gap, 0);
  const sheetW = label + 8 * (cellW + gap) + gap;
  const img = new Uint8ClampedArray(sheetW * sheetH * 4);
  const bg = [0x26, 0x28, 0x2f, 255];
  for (let i = 0; i < img.length; i += 4) (img[i] = bg[0]), (img[i + 1] = bg[1]), (img[i + 2] = bg[2]), (img[i + 3] = 255);
  text(img, sheetW, 4, 2, shape, [0xee, 0xf0, 0xf2]);
  rows.forEach((row, ri) => {
    const element = ELEMENTS[ri];
    text(img, sheetW, 4, y + 4, element, [0xc2, 0xc5, 0xcc]);
    if (!row.none) {
      const n = row.frames;
      for (let f = 0; f < 8; f++) {
        const src = Math.round((f * (n - 1)) / 7);
        blit2(img, sheetW, label + f * (cellW + gap), y, row, src);
      }
    }
    y += heights[ri] + gap;
  });
  writeFileSync(join(SHEET, `wishfx-${shape}.png`), encodePNG(sheetW, sheetH, img));
}

function blit2(dst, dw, dx, dy, row, frame) {
  const fw = row.w, fh = row.h;
  for (let y = 0; y < fh; y++)
    for (let x = 0; x < fw; x++) {
      const s = ((y * row.sheetW) + frame * fw + x) * 4;
      if (row.rgba[s + 3] === 0) continue;
      for (let oy = 0; oy < 2; oy++)
        for (let ox = 0; ox < 2; ox++) {
          const t = ((dy + y * 2 + oy) * dw + dx + x * 2 + ox) * 4;
          dst[t] = row.rgba[s];
          dst[t + 1] = row.rgba[s + 1];
          dst[t + 2] = row.rgba[s + 2];
          dst[t + 3] = 255;
        }
    }
}

function text(img, w, x, y, s, rgb) {
  let cx = x;
  for (const ch of s) {
    const g = FONT[ch];
    if (!g) { cx += 4; continue; }
    for (let col = 0; col < 3; col++)
      for (let row = 0; row < 5; row++)
        if (g[row * 3 + col] === "1") {
          const t = ((y + row) * w + cx + col) * 4;
          img[t] = rgb[0]; img[t + 1] = rgb[1]; img[t + 2] = rgb[2]; img[t + 3] = 255;
        }
    cx += 4;
  }
}

void resolved;
