/* EXCALIBUR — the real angelX startup intro, ported to the web 1:1.
   Source: cockpit/assets/excalibur/rise.png — a 10×6 atlas of 60 frames
   (680×384 each, white ink on transparent, alpha = luminance).
   Rendered exactly like cockpit/src/app/startup_intro.rs `prepare()`:
   · fixed side crop CROP_X=140 (whole blade + hand + waterline, no wings)
   · fit into a columns×2 × rows×4 pixel canvas, centered (Lanczos-quality)
   · fixed level curve: (v-10)/220 clamp → pow 0.68 → ×255
   · 4×4 Bayer ordered dithering → one dot per canvas pixel
   Timing matches the app: 12 fps · frames 0–49 rise once · hold 49 ·
   water band (lower third + smoothstep feather) flows frames 50–59 with
   a 4-tick melt seam, gated by openness so steel never jitters. */
(() => {
  'use strict';

  const ATLAS_URL = 'assets/excalibur/rise.png';
  const COLS = 10, ROWS = 6;              // atlas grid
  const FRAME_W = 680, FRAME_H = 384;     // atlas frame size
  const CROP_X = 140;                     // fixed side crop, as in the app
  const CROP_W = FRAME_W - CROP_X * 2;    // 400 px of source width
  const FPS = 12, HOLD = 49;
  const RIPPLE = [50, 51, 52, 53, 54, 55, 56, 57, 58, 59];
  const SEAM = 4;
  const CYCLE = RIPPLE.length + SEAM;
  // The app renders into columns×2 × rows×4 braille dot pixels. A generous
  // pane (~129×36 cells) gives ~258×144; we render at 2× that dot pitch so
  // the blade, hand and water resolve like the terminal's fine-dot transport.
  const COLS_PX = 1032, ROWS_PX = 576;

  // 8×8 Bayer matrix — the exact matrix dotmax's bayer() dither uses
  // (values 0..63 over 64, comparison = level > matrix/64).
  const BAYER8 = [
    0, 32, 8, 40, 2, 34, 10, 42,
    48, 16, 56, 24, 50, 18, 58, 26,
    12, 44, 4, 36, 14, 46, 6, 38,
    60, 28, 52, 20, 62, 30, 54, 22,
    3, 35, 11, 43, 1, 33, 9, 41,
    51, 19, 59, 27, 49, 17, 57, 25,
    15, 47, 7, 39, 13, 45, 5, 37,
    63, 31, 55, 23, 61, 29, 53, 21,
  ];

  const canvas = document.getElementById('excalibur');
  const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  const state = { ok: false, raf: 0, last: 0, acc: 0, tick: 0 };

  const fail = () => {
    // atlas unusable → no dots to paint; the mark stands alone
    state.ok = false;
    if (canvas && canvas.parentNode) canvas.remove();
    if (window.Excalibur) window.Excalibur.visible = () => false;
  };

  // deterministic per-dot hash for the seam crossfade
  const hash = (x, y) => (((x * 2654435761) ^ (y * 40503)) >>> 0 & 255) / 255;

  /* one atlas frame → the app's `prepare()`:
     crop → fit → center → level curve, leaving a Float32 level canvas */
  function prepare(img, frame) {
    const sc = document.createElement('canvas');
    sc.width = CROP_W; sc.height = FRAME_H;
    const sg = sc.getContext('2d', { willReadFrequently: true });
    sg.imageSmoothingEnabled = false;
    sg.drawImage(img, (frame % COLS) * FRAME_W + CROP_X,
      Math.floor(frame / COLS) * FRAME_H, CROP_W, FRAME_H, 0, 0, CROP_W, FRAME_H);

    const scale = Math.min(COLS_PX / CROP_W, ROWS_PX / FRAME_H);
    const fw = Math.max(1, Math.round(CROP_W * scale));
    const fh = Math.max(1, Math.round(FRAME_H * scale));
    const fc = document.createElement('canvas');
    fc.width = fw; fc.height = fh;
    const fg = fc.getContext('2d', { willReadFrequently: true });
    fg.imageSmoothingQuality = 'high';          // browser Lanczos-class resample
    fg.drawImage(sc, 0, 0, CROP_W, FRAME_H, 0, 0, fw, fh);

    const out = new Float32Array(COLS_PX * ROWS_PX);
    let px;
    try {
      px = fg.getImageData(0, 0, fw, fh).data;
    } catch (e) { fail(); return out; }
    // center the fitted frame on the dot canvas (app overlay)
    const ox = (COLS_PX - fw) >> 1, oy = (ROWS_PX - fh) >> 1;
    for (let y = 0; y < fh; y++) {
      for (let x = 0; x < fw; x++) {
        const src = (y * fw + x) * 4;
        const level = px[src + 3] / 255;        // alpha = luminance (white ink)
        // fixed toe + steel lift, exactly the app curve — never per-frame
        const lin = Math.min(1, Math.max(0, (level * 255 - 10) / 220));
        out[(y + oy) * COLS_PX + (x + ox)] = Math.pow(lin, 0.68);
      }
    }
    return out;
  }

  /* the water band the flow may touch, feathered in from the settled steel */
  function waterBand(h) { return [Math.floor(h * 2 / 3), Math.max(1, Math.floor(h / 12))]; }
  function bandWeight(y, top, feather) {
    const t = Math.min(1, (y - top) / feather);
    return t * t * (3 - 2 * t);                 // smoothstep, as in the app
  }
  /* 1 where settled level is free water, 0 where it is steel */
  function openness(level) {
    const WATER = 64 / 255, STEEL = 128 / 255;
    if (level >= STEEL) return 0;
    if (level <= WATER) return 1;
    return (STEEL - level) / (STEEL - WATER);
  }

  /* compose a tick exactly like `compose()`/`flow_water()`: settled frame,
     water band crossfaded toward the ripple pair by band weight × openness */
  function composeFrame(buf, settled, ripA, ripB, melt01) {
    const [top, feather] = waterBand(ROWS_PX);
    for (let y = top; y < ROWS_PX; y++) {
      const weight = bandWeight(y, top, feather);
      for (let x = 0; x < COLS_PX; x++) {
        const i = y * COLS_PX + x;
        const base = settled[i];
        const open = openness(base);
        if (open <= 0) continue;                    // steel: never touched
        const blend = melt01 != null ? weight * open : 0;
        if (ripA && blend > 0) {
          let shimmer;
          if (ripB) {
            shimmer = ripA[i] * (1 - melt01) + ripB[i] * melt01; // crossfade pair
          } else {
            shimmer = ripA[i];
          }
          buf[i] = base * (1 - blend) + shimmer * blend;
        }
      }
    }
  }

  function paint(ctx, buf) {
    const img = ctx.createImageData(COLS_PX, ROWS_PX);
    const d = img.data;
    for (let y = 0; y < ROWS_PX; y++) {
      for (let x = 0; x < COLS_PX; x++) {
        const i4 = (y * COLS_PX + x) * 4;
        const on = buf[y * COLS_PX + x] > BAYER8[(x & 7) + (y & 7) * 8] / 64;
        if (on) { d[i4] = d[i4 + 1] = d[i4 + 2] = 255; d[i4 + 3] = 255; }
        // off → transparent: nothing is painted behind the ink
      }
    }
    ctx.putImageData(img, 0, 0);
  }

  if (canvas) {
    canvas.width = COLS_PX; canvas.height = ROWS_PX;
    const ctx = canvas.getContext('2d');
    const img = new Image();
    img.src = ATLAS_URL;
    const go = () => {
      state.ok = true;
      // prepared once, re-used: the rise frame under the cursor + ripples
      const cache = { settled: null, rip: {} };
      const getPrep = (f) => {
        if (f === HOLD) { cache.settled ??= prepare(img, HOLD); return cache.settled; }
        return cache.rip[f] ??= prepare(img, f);
      };
      const buf = () => new Float32Array(COLS_PX * ROWS_PX);

      if (reduced) {
        // reduced motion: hold the raised blade, water frozen
        const b = getPrep(HOLD); paint(ctx, b); return;
      }

      const step = () => {
        const t = state.tick;
        let b;
        if (t <= HOLD) {
          b = getPrep(t);                              // the rise, once
        } else {
          b = getPrep(HOLD).slice();                   // settled frame
          const s = (t - HOLD - 1) % CYCLE;            // ambient water
          if (s < RIPPLE.length) {
            composeFrame(b, getPrep(HOLD), getPrep(RIPPLE[s]), null, 0);
          } else {
            const melt = (s - RIPPLE.length + 1) / SEAM;
            composeFrame(b, getPrep(HOLD), getPrep(RIPPLE[RIPPLE.length - 1]),
              getPrep(RIPPLE[0]), melt);               // dissolve 59 → 50
          }
        }
        paint(ctx, b);
        state.tick++;
      };

      const loop = (now) => {
        if (!state.ok) return;
        state.acc += Math.min(100, now - state.last); state.last = now;
        let n = 0;
        while (state.acc >= 1000 / FPS && n++ < 4) { state.acc -= 1000 / FPS; step(); }
        state.raf = requestAnimationFrame(loop);
      };
      state.last = performance.now();
      state.raf = requestAnimationFrame(loop);
    };
    if (img.decode) {
      img.decode().then(go).catch(() => { img.onload = go; img.onerror = fail; img.src = ATLAS_URL; });
    } else {
      img.onload = go; img.onerror = fail;
    }
  }

  window.Excalibur = {
    // the blade tip in page coordinates — the guiding star's launch point
    tip(el) {
      const r = el.getBoundingClientRect();
      return { x: r.left + r.width * 0.5, y: r.top + r.height * 0.04 };
    },
    visible: () => state.ok,
  };
})();
