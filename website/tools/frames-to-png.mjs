#!/usr/bin/env node
// Frame text + palette cell map → PNG, via the system chromium.
//
// Input is what the capture test writes: `<name>.json` with {width, height,
// palette, cells} where cells is row-major [symbol, paletteIndex] pairs. Output
// is `<name>.png` at the same path, rendered with a real monospace font so
// braille, box-drawing and block glyphs land on the pixel grid.
//
//   NODE_PATH=/tmp/shot/node_modules node website/tools/frames-to-png.mjs \
//     /tmp/angelx-frames website/assets/frames [--scale 2]
//
// Only the plates `index.html` actually shows are rasterized by default: a
// capture the page does not reference is a byproduct of the capture run, not an
// asset, and promoting it ships bytes no visitor loads. `--all` converts every
// capture and `--only a,b` names them explicitly.

import { readFileSync, readdirSync, mkdirSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright-core");

const HERE = dirname(fileURLToPath(import.meta.url));

/** The plate names `index.html` references, read from the page itself. */
const pagePlates = () => {
  const html = readFileSync(join(HERE, "..", "index.html"), "utf8");
  return new Set(
    [
      ...html.matchAll(/<img[^>]*src=["']assets\/frames\/([^"']+)\.png["']/g),
    ].map((m) => m[1]),
  );
};

const [, , srcDir, outDir, ...rest] = process.argv;
if (!srcDir || !outDir) {
  console.error(
    "usage: frames-to-png.mjs <frames-dir> <out-dir> [--scale N] [--cell WxH] [--all | --only a,b]",
  );
  process.exit(2);
}
const scaleArg = rest.indexOf("--scale");
const scale = scaleArg >= 0 ? Number(rest[scaleArg + 1]) : 2;
const cellArg = rest.indexOf("--cell");
const [cellW, cellH] =
  cellArg >= 0 ? rest[cellArg + 1].split("x").map(Number) : [9, 19];

// ── colour tables ──────────────────────────────────────────────────────────
const ANSI16 = {
  Black: "#1c1c22",
  Red: "#c14a52",
  Green: "#5aa469",
  Yellow: "#c8a24a",
  Blue: "#5a7fc1",
  Magenta: "#a06bb8",
  Cyan: "#4fa3a3",
  Gray: "#c3c3c8",
  DarkGray: "#5b5b66",
  LightRed: "#e0736f",
  LightGreen: "#8ecf8e",
  LightYellow: "#e8dd8a",
  LightBlue: "#8fb3e8",
  LightMagenta: "#d4a0e0",
  LightCyan: "#8fd8d8",
  White: "#f4f4f7",
};
const DEFAULT_FG = "#e2e2e8";
const DEFAULT_BG = "#0a0a0c";

const xterm = (n) => {
  if (n < 16) return Object.values(ANSI16)[n];
  if (n < 232) {
    const i = n - 16;
    const steps = [0, 95, 135, 175, 215, 255];
    const r = steps[Math.floor(i / 36)],
      g = steps[Math.floor((i % 36) / 6)],
      b = steps[i % 6];
    return `rgb(${r},${g},${b})`;
  }
  const v = 8 + (n - 232) * 10;
  return `rgb(${v},${v},${v})`;
};

// ratatui Debug strings: "Rgb(12, 14, 18)" | "Indexed(238)" | "Reset" | "Green" | "LightGreen"
const colour = (spec, fallback) => {
  const s = spec.trim();
  if (s === "Reset" || s === "") return fallback;
  const rgb = s.match(/^Rgb\(\s*(\d+),\s*(\d+),\s*(\d+)\)$/);
  if (rgb) return `rgb(${rgb[1]},${rgb[2]},${rgb[3]})`;
  const idx = s.match(/^Indexed\((\d+)\)$/);
  if (idx) return xterm(Number(idx[1]));
  // modifier debug strings arrive as "BOLD | ITALIC" or "Modifier::BOLD"
  const bare = s
    .replace(/Modifier::/g, "")
    .replace(/\s*\|\s*/g, "+")
    .trim();
  if (!bare || bare === "NONE") return fallback;
  if (ANSI16[bare]) return ANSI16[bare];
  return fallback;
};

const weight = (spec) => (/BOLD/.test(spec) ? 700 : 400);
const italic = (spec) => (/ITALIC/.test(spec) ? "italic" : "normal");
const dimmed = (spec) => /DIM/.test(spec);

const escapeHtml = (s) =>
  s.replace(
    /[&<>"']/g,
    (c) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[
        c
      ],
  );

const page = (frame) => {
  const { width, height, palette, cells } = frame;
  let spans = "";
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const [symbol, index] = cells[y * width + x];
      if (symbol === " ") continue;
      const [fgSpec, bgSpec, modSpec] = palette[index];
      const fg = colour(fgSpec, DEFAULT_FG);
      const bg = colour(bgSpec, null);
      const style = [
        `left:${x * cellW}px`,
        `top:${y * cellH}px`,
        `color:${fg}`,
        `font-weight:${weight(modSpec)}`,
        `font-style:${italic(modSpec)}`,
        dimmed(modSpec) ? "opacity:.62" : "",
        bg ? `background:${bg}` : "",
      ]
        .filter(Boolean)
        .join(";");
      // Braille and block glyphs are wide-ish in most monos; nudge them onto
      // the cell grid so columns line up.
      spans += `<i style="${style}">${escapeHtml(symbol)}</i>`;
    }
  }
  const w = width * cellW;
  const h = height * cellH;
  return `<!doctype html><meta charset="utf-8"><style>
  html,body{margin:0;background:${DEFAULT_BG}}
  #f{position:relative;width:${w}px;height:${h}px;background:${DEFAULT_BG};
     font-family:"DejaVu Sans Mono","JetBrains Mono","Liberation Mono",monospace;
     font-size:${Math.round(cellH * 0.78)}px;line-height:${cellH}px;letter-spacing:0;
     -webkit-font-smoothing:antialiased;overflow:hidden}
  #f i{position:absolute;font-style:normal;white-space:pre}
  </style><div id="f">${spans}</div>`;
};

const main = async () => {
  mkdirSync(outDir, { recursive: true });
  const captures = readdirSync(srcDir).filter((f) => f.endsWith(".json"));
  if (!captures.length) {
    console.error(`no frame json in ${srcDir}`);
    process.exit(1);
  }
  const onlyArg = rest.indexOf("--only");
  const explicit =
    onlyArg >= 0
      ? rest[onlyArg + 1]
          .split(",")
          .map((s) => s.trim())
          .filter(Boolean)
      : null;
  const wanted = explicit
    ? new Set(explicit)
    : rest.includes("--all")
      ? null
      : pagePlates();
  const files = wanted
    ? captures.filter((f) => wanted.has(basename(f, ".json")))
    : captures;
  if (!files.length) {
    console.error(
      `no capture in ${srcDir} matches ${explicit ? `--only ${explicit.join(",")}` : "any plate index.html shows"}`,
    );
    process.exit(1);
  }
  if (explicit) {
    const missing = explicit.filter(
      (n) => !captures.some((f) => basename(f, ".json") === n),
    );
    if (missing.length) {
      console.error(
        `note: no capture json for ${missing.join(", ")} — run the capture test first`,
      );
    }
  }
  const skipped = captures.filter((f) => !files.includes(f));
  if (skipped.length) {
    console.log(
      `skipping ${skipped.length} capture(s) index.html does not show: ${skipped
        .map((f) => basename(f, ".json"))
        .join(", ")}`,
    );
    console.log(
      "  (--all rasterizes every capture; --only a,b names them explicitly)",
    );
  }
  const browser = await chromium.launch({
    executablePath: "/usr/bin/chromium",
    args: ["--no-sandbox", "--disable-gpu"],
  });
  for (const file of files) {
    const frame = JSON.parse(readFileSync(join(srcDir, file), "utf8"));
    const name = basename(file, ".json");
    const p = await browser.newPage({
      viewport: { width: frame.width * cellW, height: frame.height * cellH },
      deviceScaleFactor: scale,
    });
    await p.setContent(page(frame), { waitUntil: "load" });
    await p.evaluate(() => document.fonts.ready);
    const out = join(outDir, `${name}.png`);
    await p.screenshot({ path: out });
    console.log(`${name}: ${frame.width}x${frame.height} cells → ${out}`);
    await p.close();
  }
  await browser.close();
};

await main();
