/* THE MOUNTAIN — the app's second scene (cockpit/src/startup_intro.rs),
   ported to the web finale. The ascent chart draws in as the visitor climbs;
   when they reach the very top, the wizard walks in from the right along the
   summit shelf and settles — the same 37-point outline, the same smoothstep
   walk (1.24 → 0.84 over 24 ticks at 12 fps), the same stride cycle. */
(() => {
  'use strict';

  // dot grid: one canvas pixel per dot. 3× the app's 200×80 dot pane — the
  // wizard is the finest thing in the scene and needs the extra pitch to read
  // as a figure (hat, beard gap, cloak folds) instead of a blob.
  const DW = 600, DH = 240;
  const FPS = 12, WALK_TICKS = 24;
  const SETTLE_X = 0.84;              // where wizard_x_at() lands him on the shelf
  // A single rising contour: no mesh, hidden edges, axes or terrain underneath.
  const ASCENT = [
    [0.06, 0.89], [0.19, 0.86], [0.30, 0.76], [0.40, 0.74], [0.51, 0.63],
    [0.60, 0.60], [0.70, 0.48], [0.79, 0.43], [0.95, 0.43],
  ];
  // Left-facing profile: hooked hat, wide brim, brow/nose, long tapered beard,
  // a bent sleeve, and a wind-caught travelling cloak with an uneven hem.
  const OUTLINE = [
    [-19, -96], [-7, -100], [1, -96], [7, -84], [10, -77], [21, -72], [8, -70],
    [9, -63], [15, -56], [17, -42], [22, -24], [29, -8], [36, -3], [21, -5],
    [13, -2], [3, -4], [-9, -2], [-16, -5], [-11, -22], [-7, -39], [-15, -44],
    [-22, -42], [-28, -47], [-28, -52], [-22, -50], [-14, -55], [-10, -60],
    [-15, -54], [-18, -48], [-18, -62], [-21, -64], [-16, -68], [-16, -71],
    [-29, -72], [-24, -76], [-12, -79], [-9, -88], [-9, -94],
  ];
  // Negative space, carried over from the app: it separates the beard from the
  // shoulder, cuts a brim shadow, and opens a long robe fold.
  const BEARD_GAP = [[-9, -66], [-5, -62], [-7, -53], [-15, -44]];
  const BRIM_SHADOW = [[-14, -71], [6, -71], [1, -67], [-10, -68]];
  const CLOAK_SHADOW = [[11, -53], [13, -33], [24, -10], [15, -15], [8, -32]];
  const CLOAK_FOLD = [[4, -44], [3, -24], [10, -7], [6, -25]];
  // Open crook, clear of the face: the ancient wooden staff.
  const STAFF = [[-30, 0], [-29, -36], [-31, -65], [-35, -74], [-33, -80],
    [-27, -82], [-23, -78], [-25, -74]];
  const WIND = [0, 0.7, 1, 0.7, 0, -0.7, -1, -0.7];
  const BAYER2 = [[0, 2], [3, 1]];    // ordered stippling for the cloth

  const canvas = document.getElementById('mountain');
  const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  const state = { reveal: 0, wizard: null, tick: 0, raf: 0, last: 0, acc: 0 };

  const smoothstep = (v) => {
    v = Math.min(1, Math.max(0, v));
    return v * v * (3 - 2 * v);
  };
  const plot = (p) => [Math.round(p[0] * (DW - 1)), Math.round(p[1] * (DH - 1))];

  function boot() {
    if (!canvas) return;
    canvas.width = DW; canvas.height = DH;
    if (reduced) {
      // motion off: the climb is already finished — full chart, wizard settled
      state.reveal = 1;
      state.wizard = { x: SETTLE_X, pose: 0, settled: true };
      render();
    }
  }

  function setPx(ctx, x, y) {
    if (x >= 0 && y >= 0 && x < DW && y < DH) ctx.fillRect(x, y, 1, 1);
  }

  // Bresenham, one dot per step — the app's draw_dot_line
  function dotLine(ctx, from, to) {
    let [x, y] = from;
    const [tx, ty] = to;
    const dx = Math.abs(tx - x), sx = x < tx ? 1 : -1;
    const dy = -Math.abs(ty - y), sy = y < ty ? 1 : -1;
    let err = dx + dy;
    for (;;) {
      setPx(ctx, x, y);
      if (x === tx && y === ty) break;
      const twice = err * 2;
      if (twice >= dy) { err += dy; x += sx; }
      if (twice <= dx) { err += dx; y += sy; }
    }
  }

  function drawChart(ctx, reveal) {
    // Length-based reveal keeps the pen moving evenly over steep gains
    // and quieter connecting segments alike.
    const lengths = [];
    let total = 0;
    for (let i = 0; i < ASCENT.length - 1; i++) {
      const dx = (ASCENT[i + 1][0] - ASCENT[i][0]) * DW;
      const dy = (ASCENT[i + 1][1] - ASCENT[i][1]) * DH;
      const l = Math.hypot(dx, dy);
      lengths.push(l); total += l;
    }
    let remaining = total * reveal;
    for (let i = 0; i < ASCENT.length - 1 && remaining > 0; i++) {
      const portion = Math.min(1, remaining / lengths[i]);
      const end = [
        ASCENT[i][0] + (ASCENT[i + 1][0] - ASCENT[i][0]) * portion,
        ASCENT[i][1] + (ASCENT[i + 1][1] - ASCENT[i][1]) * portion,
      ];
      dotLine(ctx, plot(ASCENT[i]), plot(end));
      remaining -= lengths[i];
    }
  }

  // Even-odd containment, as the app's `inside()` walks the same polygons.
  function inside(x, y, poly) {
    let hit = false;
    let prev = poly[poly.length - 1];
    for (const next of poly) {
      if ((next[1] > y) !== (prev[1] > y)
        && x < (prev[0] - next[0]) * (y - next[1]) / (prev[1] - next[1]) + next[0]) hit = !hit;
      prev = next;
    }
    return hit;
  }

  function drawWizard(ctx, wizard) {
    const [cx, ground] = plot([wizard.x, ASCENT[8][1]]);
    const groundY = ground - 1;
    const height = Math.max(Math.min(DH * 0.23, DW * 0.28), 12) * 0.5;
    const scale = height / 100;
    const pt = (x, y) => [Math.round(cx + x * scale), Math.round(groundY + y * scale)];
    // He only catches the wind while he walks; settled, he is still.
    const wind = wizard.settled ? 0 : WIND[wizard.pose % 8];

    const [left, top] = pt(-31, -101);
    const [right, bottom] = pt(38, 1);
    for (let y = Math.max(0, top); y <= Math.min(DH - 1, bottom); y++) {
      for (let x = Math.max(0, left); x <= Math.min(DW - 1, right); x++) {
        const localY = (y - groundY) / scale;
        // Warp the silhouette and its cuts together: planted feet and staff
        // stay still while cloth and hat tip catch the wind.
        const cloth = Math.min(1, Math.max(0, (localY + 48) / 48));
        const hat = Math.min(1, Math.max(0, (-localY - 80) / 20));
        const localX = (x - cx) / scale - wind * (cloth * 5 + hat * 4);
        if (!inside(localX, localY, OUTLINE)) continue;
        if (inside(localX, localY, BEARD_GAP) || inside(localX, localY, CLOAK_FOLD)
          || inside(localX, localY, BRIM_SHADOW) || inside(localX, localY, CLOAK_SHADOW)) continue;
        // Ordered stippling models cloth turning from the light. Anchored to the
        // sprite, never the clock, so walking does not sparkle at random.
        const clothOrHat = !(localY >= -79 && localY <= -58);
        const shade = localX > 5 ? 2 : 3;
        const stipple = BAYER2[(((y - groundY) % 2) + 2) % 2][(((x - cx) % 2) + 2) % 2];
        // Keep the light-facing contour, profile and hat tip intact — a
        // sprite this small has very few dots to describe them.
        const litEdge = !inside(localX - 1 / scale, localY, OUTLINE);
        if (!clothOrHat || litEdge || localY < -92 || stipple < shade) setPx(ctx, x, y);
      }
    }

    for (let i = 0; i < STAFF.length - 1; i++) {
      dotLine(ctx, pt(...STAFF[i]), pt(...STAFF[i + 1]));
    }
    const stride = wizard.settled ? 0 : [-3, 0, 3, 0][wizard.pose % 4];
    for (const [ankle, toe] of [[-7, -13 - stride], [10, 15 + stride]]) {
      dotLine(ctx, pt(ankle, -5), pt(toe, 0));
    }
  }

  function render() {
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    ctx.clearRect(0, 0, DW, DH);
    ctx.fillStyle = '#fff';
    drawChart(ctx, state.reveal);
    if (state.wizard) drawWizard(ctx, state.wizard);
  }

  /* the climb: chart reveal follows the visitor's ascent progress */
  function progress(p) {
    if (reduced || state.wizard) return; // settled/walking scene no longer redraws
    state.reveal = smoothstep((p - 0.78) / 0.2);
    render();
  }

  /* the summit: the wizard walks in from the right and settles */
  function walk() {
    if (!canvas || state.wizard || reduced) return; // reduced: already settled in boot
    state.wizard = { x: 1.24, pose: 0, settled: false };   // walks in from the right
    state.tick = 0; state.last = performance.now(); state.acc = 0;
    const step = () => {
      const t = state.tick;
      const eased = smoothstep(t / WALK_TICKS);
      state.wizard.x = 1.24 + (SETTLE_X - 1.24) * eased;
      state.wizard.pose = Math.floor(t / 3);
      state.wizard.settled = t >= WALK_TICKS;
      render();
      state.tick++;
      if (state.wizard.settled) { state.raf = 0; return; } // he stays on the shelf
      if (!state.raf) return; // another walk() or reset() took over the loop
      state.raf = requestAnimationFrame(loop);
    };
    const loop = (now) => {
      state.acc += now - state.last; state.last = now;
      let n = 0;
      while (state.acc >= 1000 / FPS && n++ < 4) { state.acc -= 1000 / FPS; step(); }
      if (state.raf) state.raf = requestAnimationFrame(loop);
    };
    state.raf = requestAnimationFrame(loop);
  }

  /* back down the mountain: clear the scene for the next climb */
  function reset() {
    if (state.raf) cancelAnimationFrame(state.raf);
    state.raf = 0;
    state.reveal = 0;
    state.wizard = null;
    state.tick = 0;
    if (canvas) {
      const ctx = canvas.getContext('2d');
      ctx.clearRect(0, 0, DW, DH);
    }
    if (reduced) boot();
  }

  window.Mountain = { progress, walk, reset };

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', boot, { once: true });
  } else {
    boot();
  }
})();
