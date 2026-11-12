// Shared pixelfx fitting for the Delve's strips. `spellfx.mjs` and `wishfx.mjs`
// both snap every pixel to the game's inks and pack a trimmed sheet. The fitting
// itself is the Mage script's: colour under half its alpha is dropped, light
// adds onto the colour it falls on, and a glow with no colour under it stays
// only where it is bright.
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { join } from "node:path";

export const FPS = 15;

let api;
export async function init(pfx) {
  if (api?.pfx === pfx) return api;
  const { createFx, createFrame, clearFrame } = await import(pathToFileURL(join(pfx, "src/core/index.js")));
  const { encodePNG } = await import(pathToFileURL(join(pfx, "tools/lib/png.mjs")));
  const { PAL } = await import(pathToFileURL(join(pfx, "src/core/color.js")));
  api = { createFx, createFrame, clearFrame, encodePNG, PAL, pfx };
  return api;
}

export function inks(inkRs) {
  const s = readFileSync(inkRs, "utf8");
  const i = s.indexOf("pub fn ink(ch: char) -> Option<Rgb> {");
  const body = s.slice(i, s.indexOf("\n}\n", i));
  const out = [];
  for (const [, , h] of body.matchAll(/((?:'.'\s*\|\s*)*'.')\s*=>\s*0x([0-9a-fA-F]{6})/g))
    out.push([parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)]);
  return out;
}

export function makePixel(inkList) {
  const near = new Map();
  function ink(r, g, b) {
    const key = (r << 16) | (g << 8) | b;
    if (!near.has(key)) {
      let best = inkList[0], bd = Infinity;
      for (const c of inkList) {
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
  return function pixel(c, l, glow, halo) {
    const ch = (v, s) => (v >>> s) & 255;
    const lit = [ch(l, 0), ch(l, 8), ch(l, 16)];
    if (ch(c, 24) >= 128) return ink(...[0, 8, 16].map((s, i) => Math.min(255, ch(c, s) + lit[i] * glow)));
    if (Math.max(...lit) >= halo) return ink(...lit);
    return null;
  };
}

// One direction of an effect: its frames as ink RGBA on a canvas of w x h with
// the origin at (ox, oy). A numeric `spec.aim` is the Mage's aimed canvas
// (side 2 * aim, origin in the middle). A boolean aim is only a direction flag
// and does not resize the canvas.
export async function play(pfx, spec, angle) {
  const { createFx, createFrame, clearFrame } = await init(pfx);
  const mod = await import(pathToFileURL(join(pfx, "src/fx", `${spec.fx}.js`)));
  const [W, H] = mod.meta.size, [OX, OY] = mod.meta.origin;
  const aimed = typeof spec.aim === "number";
  const [w, h, ox, oy] = aimed ? [spec.aim * 2, spec.aim * 2, spec.aim, spec.aim] : [W, H, OX, OY];
  const frame = createFrame(w, h);
  const opts = resolveOpts(spec.opts);
  const fx = createFx(mod, { x: ox, y: oy, angle, ...opts });
  const n = Math.round((spec.to - spec.from) * FPS);
  if (spec.from > 0) fx.update(spec.from);
  const pixel = spec.pixel;
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
  return { w, h, ox, oy, frames, duration: mod.meta.duration, loop: !!mod.meta.loop };
}

function resolveOpts(opts) {
  const out = { ...(opts ?? {}) };
  if (typeof out.palette === "string") out.palette = api.PAL[out.palette];
  return out;
}

// The box every frame of every direction shares, so a strip is no wider than
// what it draws.
export function trim(dirs) {
  const { w, h } = dirs[0];
  let x0 = w, y0 = h, x1 = -1, y1 = -1;
  for (const d of dirs)
    for (const px of d.frames)
      for (let y = 0; y < h; y++)
        for (let x = 0; x < w; x++)
          if (px[(y * w + x) * 4 + 3]) (x0 = Math.min(x0, x)), (x1 = Math.max(x1, x)), (y0 = Math.min(y0, y)), (y1 = Math.max(y1, y));
  if (x1 < x0) return [0, 0, 1, 1];
  return [x0, y0, x1 - x0 + 1, y1 - y0 + 1];
}

export function pack(dirs, box) {
  const [x0, y0, fw, fh] = box;
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
  return { sheet, fw, fh, n, dirs: dirs.length, sw, sh, x0, y0 };
}
