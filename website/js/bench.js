/* angelX trials — benchmark plots in the cockpit's own "measured" dot-matrix style
 * (one dot per Bresenham step, as in the app's draw_dot_line and js/mountain.js),
 * animated at the app's 12 fps so each plot replays the bench as it ran.
 * Data: window.BENCH (js/bench-data.js), generated from the eval traces.
 */
(() => {
'use strict';
const B = window.BENCH;
if (!B) return;

// site tokens (css/style.css :root); omp's mark gray is a TXT×BG mix (marks only, 4.0:1)
const TH = Object.assign({ INK: '#f4f4f7', DIM: '#a2a2ac', D3: '#686875', FAINT: '#5d5d68', RULE: '#3a3a44', BG: '#050506', HERO: '#ffffff' },
  window.BENCH_THEME || {});      // optional palette override (README renders); the site uses these defaults
const { INK, DIM, D3, FAINT, RULE, BG, HERO } = TH;
const P = 3, S = 2, FPS = 12;             // dot pitch, dot size (viewBox units), frame rate
const REDUCED = !!(window.matchMedia && matchMedia('(prefers-reduced-motion: reduce)').matches);
const NS = 'http://www.w3.org/2000/svg';
const el = (p, t, a) => { const n = document.createElementNS(NS, t); for (const k in a) n.setAttribute(k, a[k]); p.appendChild(n); return n; };
const txt = (p, a, s) => { const n = el(p, 'text', a); n.textContent = s; return n; };
const tip = (n, s) => { const t = document.createElementNS(NS, 'title'); t.textContent = s; n.appendChild(t); };
const sq = (x, y, s = S) => `M${x} ${y}h${s}v${s}h-${s}z`;

// identity = label + gray + dot rhythm (never colour alone)
const SER = {
  angelx: { label: 'angelX', color: HERO, on: i => true },
  opencode: { label: 'OpenCode', color: DIM, on: i => i % 2 === 0 },
  omp: { label: 'omp', color: D3, on: i => i % 4 < 2 },
};
const LEGEND_ORDER = ['angelx', 'opencode', 'omp'];
const DRAW_ORDER = ['omp', 'opencode', 'angelx'];      // angelX drawn last, on top
// model list is derived: add a model to js/bench-data.js and it becomes a tab
const MODELS = [...new Set(B.cells.map(c => c.model))];
const modelLabel = m => `${B.models[m].name} \u00b7 thinking ${B.models[m].thinking}`;
const cell = (m, h) => B.cells.find(c => c.model === m && c.harness === h);
const N = Math.max(...B.cells.map(c => c.attempts.length));
const fmtS = s => s >= 600 ? `${Math.floor(s / 60)}:${String(Math.round(s % 60)).padStart(2, '0')}` : `${Math.round(s)}s`;
const fmtClock = s => `T+${String(Math.floor(s / 60)).padStart(2, '0')}:${String(Math.floor(s % 60)).padStart(2, '0')}`;
const fmtTok = v => {
  if (v >= 1e6) {
    const m = v / 1e6;
    return (m >= 100 || m % 1 === 0) ? `${Math.round(m)}M` : `${m.toFixed(1)}M`;
  }
  return `${Math.round(v / 1e3)}k`;
};
const niceCeil = v => { const p = Math.pow(10, Math.floor(Math.log10(v))); for (const f of [1, 1.2, 1.5, 2, 2.5, 3, 4, 5, 6, 8, 10]) if (f * p >= v) return f * p; return 10 * p; };

// Bresenham over cells, one callback per dot — the app's draw_dot_line
function line(c0, r0, c1, r1, cb) {
  let x = c0, y = r0;
  const dx = Math.abs(c1 - c0), sx = c0 < c1 ? 1 : -1, dy = -Math.abs(r1 - r0), sy = r0 < r1 ? 1 : -1;
  let err = dx + dy;
  for (;;) {
    cb(x, y);
    if (x === c1 && y === r1) break;
    const e2 = 2 * err;
    if (e2 >= dy) { err += dy; x += sx; }
    if (e2 <= dx) { err += dx; y += sy; }
  }
}

// dots along a polyline, each stamped with the data time it represents
function trail(points, toCell, stairs) {
  const out = [];
  let last = '', i = 0;
  const seg = (a, b, t0, t1) => {
    const pts = [];
    line(a.c, a.r, b.c, b.r, (x, y) => pts.push([x, y]));
    pts.forEach(([x, y], j) => {
      const key = `${x},${y}`;
      if (key === last) return;
      last = key;
      out.push({ x, y, t: t0 + (t1 - t0) * (pts.length > 1 ? j / (pts.length - 1) : 1), i: i++ });
    });
  };
  for (let k = 1; k < points.length; k++) {
    const a = toCell(points[k - 1]), b = toCell(points[k]);
    if (stairs) {
      const corner = { c: b.c, r: a.r };
      seg(a, corner, points[k - 1].t, points[k].t);
      seg(corner, b, points[k].t, points[k].t);
    } else {
      seg(a, b, points[k - 1].t, points[k].t);
    }
  }
  return out;
}

// plot frame: faint dot lattice, dotted axes; returns cell→unit mappers
function frame(svg, o = {}) {
  const f = { ox: 50, oy: 34, GW: 124, GH: 58, ...o };
  f.X = c => f.ox + c * P;
  f.Y = r => f.oy + r * P;
  let d = '';
  for (let r = 0; r < f.GH; r += 6) for (let c = 0; c < f.GW; c += 6) d += sq(f.X(c), f.Y(r));
  el(svg, 'path', { d, fill: FAINT, opacity: .45 });
  let a = '';
  for (let c = 0; c < f.GW; c += 2) a += sq(f.X(c), f.Y(f.GH) + 1);
  for (let r = 0; r <= f.GH; r += 2) a += sq(f.X(0) - 2 * P, f.Y(r));
  el(svg, 'path', { d: a, fill: RULE });
  return f;
}
const yLabel = (svg, f, r, s) => txt(svg, { x: f.ox - 10, y: f.Y(r) + 5, 'text-anchor': 'end', class: 'vt', 'font-size': 12.5, fill: DIM }, s);
const xLabel = (svg, f, c, s, anchor = 'middle') => txt(svg, { x: f.X(c), y: f.Y(f.GH) + 18, 'text-anchor': anchor, class: 'vt', 'font-size': 12.5, fill: DIM }, s);

// 12 fps ticker with cancellation (the app's cadence)
function play(state, ticks, onTick, animate = true) {
  (state.timers || []).forEach(clearInterval);
  state.timers = [];
  if (REDUCED || window.BENCH_STATIC || ticks <= 1 || !animate) { onTick(1); return; }
  let k = 0;
  onTick(0);
  const t = setInterval(() => {
    k++;
    onTick(Math.min(1, k / ticks));
    if (k >= ticks) clearInterval(t);
  }, 1000 / FPS);
  state.timers.push(t);
}

// ════ THE RACE — solved attempts against the agent clock ════
function race(svg, m, state, anim = true) {
  svg.innerHTML = '';
  // Header holds the clock, angelX's score on its own line and the peers on the next.
  const f = frame(svg, { ox: 50, oy: 62, GW: 124, GH: 49 });
  const series = DRAW_ORDER.map(h => {
    const c = cell(m, h);
    let t = 0, v = 0;
    const pts = [{ t: 0, v: 0 }];
    c.attempts.forEach(a => { t += a.wall_s; if (a.solved) v++; pts.push({ t, v }); });
    return { h, c, pts, total: t, solved: v };
  });
  const last = Math.max(...series.map(s => s.total));
  const T = last * 1.05;
  const toCell = p => ({ c: Math.round(p.t / T * (f.GW - 1)), r: Math.round((1 - p.v / N) * (f.GH - 1)) });
  [Math.round(N / 3), Math.round(2 * N / 3), N].forEach(v => {
    const r = toCell({ t: 0, v }).r;
    let d = '';
    for (let c = 0; c < f.GW; c += 3) d += sq(f.X(c), f.Y(r), 1.4);
    el(svg, 'path', { d, fill: RULE });
    yLabel(svg, f, r, String(v));
  });
  yLabel(svg, f, f.GH - 1, '0');
  const mins = T / 60, stepM = [1, 2, 5, 10, 15, 20, 30].find(k => mins / k <= 5) || 60;
  for (let q = 0; q <= mins + 1e-9; q += stepM) {
    const c = Math.round(q * 60 / T * (f.GW - 1));
    xLabel(svg, f, c, `${q}m`, q === 0 ? 'start' : 'middle');
  }
  txt(svg, { x: f.X(f.GW - 1), y: f.Y(f.GH) + 34, 'text-anchor': 'end', class: 'vt', 'font-size': 13, fill: FAINT }, 'agent time (min)');
  const clock = txt(svg, { x: f.ox - 2 * P, y: 14, class: 'vt num', 'font-size': 15, fill: DIM }, '');
  const counters = {};
  LEGEND_ORDER.forEach((h, k) => {
    const at = [[0, 33, 19], [0, 52, 16], [196, 52, 16]][k];
    counters[h] = txt(svg, { x: f.ox - 2 * P + at[0], y: at[1], class: `vt num ${h === 'angelx' ? 'ax-txt-glow' : ''}`, 'font-size': at[2], fill: h === 'angelx' ? HERO : (SER[h].color === D3 ? DIM : SER[h].color) }, '');
  });
  const paths = {}, dots = {};
  series.forEach(s => {
    dots[s.h] = trail(s.pts, toCell, true).filter(d => SER[s.h].on(d.i));
    paths[s.h] = el(svg, 'path', { d: '', fill: s.h === 'angelx' ? HERO : SER[s.h].color, class: s.h === 'angelx' ? 'ax-glow' : '' });
    tip(paths[s.h], `${SER[s.h].label} — ${s.solved}/${s.pts.length - 1} solved in ${Math.round(s.total)} s of agent time`);
  });
  const finish = el(svg, 'g', {});
  play(state, Math.round(FPS * 6.5), p => {
    const now = p * last;
    clock.textContent = fmtClock(now);
    series.forEach(s => {
      const dotSize = s.h === 'angelx' ? 2.5 : S;
      paths[s.h].setAttribute('d', dots[s.h].filter(d => d.t <= now).map(d => sq(f.X(d.x), f.Y(d.y), dotSize)).join(''));
      /* solved-so-far: the cumulative value of the last elapsed point, not the
         number of points elapsed (those differ — 136 attempts, 133 solved) */
      const cur = s.pts.filter(q => q.t <= now).pop();
      const done = cur ? cur.v : 0;
      counters[s.h].textContent = `${SER[s.h].label} ${String(done).padStart(2, '0')}/${s.pts.length - 1}`;
    });
    if (p >= 1) {
      finish.innerHTML = '';
      series.forEach(s => {
        const e = toCell(s.pts[s.pts.length - 1]);
        const star = txt(finish, { x: f.X(e.c) + 1, y: f.Y(e.r) - 4, 'text-anchor': 'middle', class: `vt ${s.h === 'angelx' ? 'ax-glow' : ''}`, 'font-size': 16, fill: s.h === 'angelx' ? HERO : SER[s.h].color }, '◆');
        tip(star, `${SER[s.h].label} finished ${s.solved}/${s.pts.length - 1} at ${Math.round(s.total)} s`);
      });
    }
  }, anim);
}

// ════ SECONDS PER ATTEMPT — every attempt in run order, medians dotted ════
const MARK = {  // cell offsets: angelX block, OpenCode plus, omp cross
  angelx: [[0, 0], [1, 0], [0, 1], [1, 1]],
  opencode: [[0, 0], [-1, 0], [1, 0], [0, -1], [0, 1]],
  omp: [[0, 0], [-1, -1], [1, 1], [-1, 1], [1, -1]],
};
function trace(svg, m, state, anim = true) {
  svg.innerHTML = '';
  const f = frame(svg, { ox: 50, GW: 124 });
  const cs = DRAW_ORDER.map(h => cell(m, h));
  const walls = cs.flatMap(c => c.attempts.map(a => a.wall_s)).sort((a, b) => a - b);
  const top = niceCeil(walls[Math.floor(walls.length * .96)] * 1.1);
  const toCell = (i, s) => ({ c: Math.round(i / (N - 1) * (f.GW - 3)) + 1, r: Math.round((1 - Math.min(s, top) / top) * (f.GH - 2)) + 1 });
  [0, .5, 1].forEach(q => yLabel(svg, f, Math.round((1 - q) * (f.GH - 2)) + 1, `${Math.round(q * top)}s`));
  [1, Math.round(N / 3), Math.round(2 * N / 3), N].forEach((a, k) => xLabel(svg, f, toCell(a - 1, 0).c, k ? `#${a}` : '#1', k === 0 ? 'start' : k === 3 ? 'end' : 'middle'));
  txt(svg, { x: f.X(f.GW - 1), y: f.Y(f.GH) + 34, 'text-anchor': 'end', class: 'vt', 'font-size': 13, fill: FAINT }, 'attempt (run order)');
  const readout = txt(svg, { x: f.ox - 2 * P, y: 16, class: 'vt num', 'font-size': 15, fill: INK }, '');
  const paths = {}, meds = {};
  cs.forEach(c => { paths[c.harness] = el(svg, 'path', { d: '', fill: c.harness === 'angelx' ? HERO : SER[c.harness].color, class: c.harness === 'angelx' ? 'ax-glow' : '' }); });
  cs.forEach(c => { meds[c.harness] = el(svg, 'path', { d: '', fill: c.harness === 'angelx' ? HERO : SER[c.harness].color, opacity: .95, class: c.harness === 'angelx' ? 'ax-glow-soft' : '' }); });
  const hits = el(svg, 'g', {});
  cs.forEach(c => c.attempts.forEach((a, i) => {
    const p = toCell(i, a.wall_s);
    const r = el(hits, 'rect', { x: f.X(p.c) - 5, y: f.Y(p.r) - 5, width: 10, height: 10, fill: 'transparent' });
    tip(r, `${SER[c.harness].label} · ${a.task} · run ${a.run} — ${a.wall_s.toFixed(1)} s${a.wall_s > top ? ' (off the top)' : ''}`);
  }));
  play(state, N + 8, p => {
    const upto = Math.min(N, Math.ceil(p * (N + 8)));
    cs.forEach(c => {
      let d = '';
      const isAx = c.harness === 'angelx';
      c.attempts.slice(0, upto).forEach((a, i) => {
        const q = toCell(i, a.wall_s);
        MARK[c.harness].forEach(([dx, dy]) => { d += sq(f.X(q.c + dx), f.Y(q.r + dy), isAx ? 2.4 : S); });
        if (a.wall_s > top) d += sq(f.X(q.c), Math.max(f.oy + 1, f.Y(q.r) - 2 * P), 1.6);
      });
      paths[c.harness].setAttribute('d', d);
    });
    if (p >= 1) {
      cs.forEach(c => {
        const r = toCell(0, c.summary.wall.median).r;
        let d = '';
        const isAx = c.harness === 'angelx';
        for (let x = 0; x < f.GW; x++) if (SER[c.harness].on(x)) d += sq(f.X(x), f.Y(r) + .5, isAx ? 2.0 : 1.4);
        meds[c.harness].setAttribute('d', d);
      });
      readout.textContent = 'MEDIAN  ' + LEGEND_ORDER.map(h => `${SER[h].label} ${cell(m, h).summary.wall.median.toFixed(1)}s`).join('  ·  ');
    } else {
      readout.textContent = `ATTEMPT ${String(upto).padStart(2, '0')}/${N}`;
    }
  }, anim);
}

// ════ CONTEXT BURNED — cumulative input tokens over the run ════
function burn(svg, m, state, anim = true) {
  svg.innerHTML = '';
  const f = frame(svg, { ox: 50, GW: 124 });
  const series = DRAW_ORDER.map(h => {
    const c = cell(m, h);
    let v = 0, u = 0;
    const pts = [{ t: 0, v: 0 }];
    c.attempts.forEach((a, i) => { v += a.uncached_in + a.cached_in; u += a.uncached_in; pts.push({ t: i + 1, v }); });
    return { h, pts, total: v, uncached: u };
  });
  const top = niceCeil(Math.max(...series.map(s => s.total)) * 1.04);
  const toCell = p => ({ c: Math.round(p.t / N * (f.GW - 1)), r: Math.round((1 - p.v / top) * (f.GH - 1)) });
  [0, .5, 1].forEach(q => yLabel(svg, f, Math.round((1 - q) * (f.GH - 1)), q ? fmtTok(q * top) : '0'));
  [0, Math.round(N / 3), Math.round(2 * N / 3), N].forEach((a, k) => xLabel(svg, f, toCell({ t: a, v: 0 }).c, `#${a}`, k === 0 ? 'start' : k === 3 ? 'end' : 'middle'));
  txt(svg, { x: f.X(f.GW - 1), y: f.Y(f.GH) + 34, 'text-anchor': 'end', class: 'vt', 'font-size': 13, fill: FAINT }, 'attempts');
  const readout = txt(svg, { x: f.ox - 2 * P, y: 16, class: 'vt num', 'font-size': 14, fill: INK }, '');
  const paths = {}, dots = {}, ends = el(svg, 'g', {});
  series.forEach(s => {
    dots[s.h] = trail(s.pts, toCell, false).filter(d => SER[s.h].on(d.i));
    paths[s.h] = el(svg, 'path', { d: '', fill: s.h === 'angelx' ? HERO : SER[s.h].color, class: s.h === 'angelx' ? 'ax-glow' : '' });
    tip(paths[s.h], `${SER[s.h].label} — ${Math.round(s.total).toLocaleString('en-US')} input tokens over ${N} attempts, ${Math.round(s.uncached).toLocaleString('en-US')} uncached`);
  });
  play(state, Math.round(FPS * 5), p => {
    const now = p * N;
    series.forEach(s => {
      const dotSize = s.h === 'angelx' ? 2.5 : S;
      paths[s.h].setAttribute('d', dots[s.h].filter(d => d.t <= now).map(d => sq(f.X(d.x), f.Y(d.y), dotSize)).join(''));
    });
    if (p >= 1) {
      ends.innerHTML = '';
      svg.appendChild(ends);   /* paint the labels last: re-append moves the group above the curves */
      const placed = [];
      series.slice().sort((a, b) => b.total - a.total).forEach(s => {
        let y = f.Y(toCell({ t: N, v: s.total }).r) - 6;
        while (placed.some(q => Math.abs(q - y) < 15)) y += 15;
        placed.push(y);
        /* a solid BG plate under each endpoint label, so the three converging
           curves never cross the text */
        const lab = `${SER[s.h].label} ${fmtTok(s.total)}`;
        const w = lab.length * 8.4 + 8;
        el(ends, 'rect', { x: f.X(f.GW - 1) + 2 - w, y: y - 12, width: w, height: 16, fill: BG });
        txt(ends, { x: f.X(f.GW - 1) - 4, y, 'text-anchor': 'end', class: `vt num ${s.h === 'angelx' ? 'ax-txt-glow' : ''}`, 'font-size': 14.5, fill: s.h === 'angelx' ? HERO : (SER[s.h].color === D3 ? DIM : SER[s.h].color) },
          lab);
      });
      readout.textContent = 'UNCACHED  ' + LEGEND_ORDER.map(h => `${SER[h].label} ${fmtTok(series.find(s => s.h === h).uncached)}`).join('  ·  ');
    } else {
      readout.textContent = `ATTEMPT ${String(Math.round(now)).padStart(2, '0')}/${N}`;
    }
  }, anim);
}

// ════ CACHE — running cache hit rate over the run ════
function cache(svg, m, state, anim = true) {
  svg.innerHTML = '';
  const f = frame(svg, { ox: 50, oy: 44, GW: 124, GH: 55 });
  const series = DRAW_ORDER.map(h => {
    const c = cell(m, h);
    let tot = 0, hit = 0, fresh = 0;
    const pts = [];
    c.attempts.forEach((a, i) => {
      tot += a.uncached_in + a.cached_in; hit += a.cached_in; fresh += a.uncached_in;
      pts.push({ t: i + 1, v: 100 * hit / tot });
    });
    return { h, c, pts, rate: 100 * hit / tot, fresh: fresh / c.attempts.length };
  });
  const lo = Math.min(50, Math.floor(Math.min(...series.flatMap(s => s.pts.map(q => q.v))) / 10) * 10);
  const toCell = p => ({ c: Math.round((p.t - 1) / (N - 1) * (f.GW - 1)), r: Math.round((100 - p.v) / (100 - lo) * (f.GH - 1)) });
  [lo, (lo + 100) / 2, 100].forEach(v => {
    const r = toCell({ t: 1, v }).r;
    let d = '';
    for (let c = 0; c < f.GW; c += 3) d += sq(f.X(c), f.Y(r), 1.4);
    el(svg, 'path', { d, fill: RULE });
    yLabel(svg, f, r, `${Math.round(v)}%`);
  });
  [1, Math.round(N / 3), Math.round(2 * N / 3), N].forEach((a, k) => xLabel(svg, f, toCell({ t: a, v: 100 }).c, `#${a}`, k === 0 ? 'start' : k === 3 ? 'end' : 'middle'));
  txt(svg, { x: f.X(f.GW - 1), y: f.Y(f.GH) + 34, 'text-anchor': 'end', class: 'vt', 'font-size': 13, fill: FAINT }, 'attempts');
  const l1 = txt(svg, { x: f.ox - 2 * P, y: 14, class: 'vt', 'font-size': 14, fill: DIM }, '');
  const l2 = txt(svg, { x: f.ox - 2 * P, y: 30, class: 'vt num', 'font-size': 15, fill: INK }, '');
  const paths = {}, dots = {};
  series.forEach(s => {
    dots[s.h] = trail(s.pts, toCell, false).filter(d => SER[s.h].on(d.i));
    paths[s.h] = el(svg, 'path', { d: '', fill: s.h === 'angelx' ? HERO : SER[s.h].color, class: s.h === 'angelx' ? 'ax-glow' : '' });
    tip(paths[s.h], `${SER[s.h].label} — ${s.rate.toFixed(1)}% of input served from cache; ${Math.round(s.fresh).toLocaleString('en-US')} fresh tokens per task`);
  });
  play(state, Math.round(FPS * 5), p => {
    const now = 1 + p * (N - 1);
    series.forEach(s => {
      const dotSize = s.h === 'angelx' ? 2.5 : S;
      paths[s.h].setAttribute('d', dots[s.h].filter(d => d.t <= now).map(d => sq(f.X(d.x), f.Y(d.y), dotSize)).join(''));
    });
    const at = Math.max(1, Math.floor(now));
    l1.textContent = 'HIT  ' + LEGEND_ORDER.map(h => { const s = series.find(x => x.h === h); return `${SER[h].label} ${s.pts[Math.min(at, s.pts.length) - 1].v.toFixed(1)}%`; }).join('  ·  ');
    l2.textContent = p >= 1
      ? 'FRESH/TASK  ' + LEGEND_ORDER.map(h => `${SER[h].label} ${(series.find(x => x.h === h).fresh / 1000).toFixed(1)}k`).join('  ·  ')
      : `ATTEMPT ${String(at).padStart(2, '0')}/${N}`;
  }, anim);
}

// ════ SCOREBOARD — one lit dot per graded attempt ════
function scoreboard(svg, m, state, anim = true) {
  svg.innerHTML = '';
  /* One model per tab. Real time, not attempt index: every attempt sits at the
     moment it finished, in cumulative agent time since the run began. One
     shared scale across the rows, so a row that stops a fifth of the way
     across really did finish in a fifth of the time. */
  const X0 = 76, W = 314, DOT = 1.6, ROW = 44;
  const rows = [];
  let y = 74;
  LEGEND_ORDER.forEach(h => {
    const c = cell(m, h);
    txt(svg, { x: X0 - 8, y: y + 6, 'text-anchor': 'end', class: `vt ${h === 'angelx' ? 'ax-txt-glow' : ''}`, 'font-size': 13, fill: h === 'angelx' ? HERO : (SER[h].color === D3 ? DIM : SER[h].color) }, SER[h].label);
    let t = 0;
    const times = c.attempts.map(a => (t += a.wall_s));
    rows.push({ c, h, y, times });
    y += ROW;
  });
  const T = Math.max(...rows.map(r => r.times[r.times.length - 1] || 0));
  const xAt = t => Math.round(X0 + (t / T) * W);
  const axisY = y - ROW + 52;

  const mins = T / 60;
  const stepM = [5, 10, 15, 20, 30, 45, 60].find(k => mins / k <= 6) || 60;
  for (let q = 0; q <= mins + 1e-9; q += stepM) {
    const c = xAt(q * 60);
    let d = '';
    for (let yy = 56; yy < axisY - 8; yy += 4) d += sq(c, yy, 1.3);
    el(svg, 'path', { d, fill: RULE, opacity: .55 });
    txt(svg, { x: c, y: axisY + 14, 'text-anchor': q === 0 ? 'start' : 'middle', class: 'vt', 'font-size': 12, fill: DIM }, `${q}m`);
  }
  txt(svg, { x: X0 + W, y: axisY + 30, 'text-anchor': 'end', class: 'vt', 'font-size': 13, fill: FAINT }, 'cumulative agent time to completion (min)');

  const total = rows.reduce((n, r) => n + r.c.attempts.length, 0);
  const readout = txt(svg, { x: 2, y: 18, class: 'vt num', 'font-size': 15, fill: INK }, '');
  const lit = rows.map(r => el(svg, 'path', { d: '', fill: r.h === 'angelx' ? HERO : INK, class: r.h === 'angelx' ? 'ax-glow' : '' }));
  const miss = rows.map(() => el(svg, 'path', { d: '', fill: 'none', stroke: DIM, 'stroke-width': 1.2 }));
  const counts = rows.map(r => txt(svg, { x: 450, y: r.y + 7, 'text-anchor': 'end', class: `vt num ${r.h === 'angelx' ? 'ax-txt-glow' : ''}`, 'font-size': 15, fill: r.h === 'angelx' ? HERO : DIM }, ''));
  const spans = rows.map(r => txt(svg, { x: 0, y: r.y + 24, class: 'vt', 'font-size': 11.5, fill: FAINT }, ''));

  const fmtMin = sec => sec >= 3600 ? `${(sec / 3600).toFixed(1)}h` : `${Math.round(sec / 60)}m`;
  rows.forEach(r => r.c.attempts.forEach((a, k) => {
    const lang = (a.task.split('-')[0] || '').toUpperCase();
    const hit = el(svg, 'rect', { x: xAt(r.times[k]) - 2, y: r.y - 5, width: 8, height: DOT + 10, fill: 'transparent' });
    tip(hit, `${r.c.harness_label} \u00b7 ${B.models[m].name} \u00b7 ${a.task} (${lang}) \u00b7 run ${a.run} \u2014 ${a.solved ? 'passed' : 'failed'} in ${a.wall_s.toFixed(1)} s, ${fmtMin(r.times[k])} into the run`);
  }));
  play(state, Math.round(FPS * 8), p => {
    const now = p * T;
    let passed = 0;
    rows.forEach((r, ri) => {
      let d = '', dm = '', ok = 0, last = 0;
      const isAx = r.h === 'angelx';
      r.c.attempts.forEach((a, k) => {
        const t = r.times[k];
        if (t > now) return;
        last = t;
        if (a.solved) { d += sq(xAt(t), r.y - 2, isAx ? 2.4 : DOT); ok++; }
        else { dm += `M${xAt(t)} ${r.y - 2}h2v2h-2z`; }
      });
      lit[ri].setAttribute('d', d);
      miss[ri].setAttribute('d', dm);
      counts[ri].textContent = `${ok}/${r.c.attempts.length}`;
      if (last) { spans[ri].setAttribute('x', xAt(last) + 8); spans[ri].textContent = fmtMin(last); }
      else spans[ri].textContent = '';
      passed += ok;
    });
    readout.textContent = `passed ${passed}/${total}  \u00b7  T+${fmtMin(now)}`;
  }, anim);
}

// ════ OUTPUT PER TASK — tokens generated and calls made, with thick stacking bars at 2.5k cap ════
function bars(svg, m, state, anim = true) {
  svg.innerHTML = '';
  const BASE = 166, CAP_Y = 44, CAP_TOKENS = 2500;
  const H = BASE - CAP_Y; // 122px
  const cols = [];

  txt(svg, { x: 8, y: 18, class: 'vt', 'font-size': 13, fill: DIM }, B.models[m].name.toUpperCase());
  txt(svg, { x: 8, y: 30, class: 'vt', 'font-size': 11, fill: FAINT },
      'tokens generated (thick pillars) \u00b7 model calls per task \u00b7 cap at 2.5k tokens');

  /* three balanced slots across the 460px panel */
  const SLOT = [86, 230, 374];
  LEGEND_ORDER.forEach((h, k) => {
    cols.push({ c: cell(m, h), h, cx: SLOT[k] });
  });

  // Baseline rule
  let rule = '';
  for (let x = 8; x < 452; x += 3) rule += sq(x, BASE + 2, 1.4);
  el(svg, 'path', { d: rule, fill: RULE });

  // 2.5k cap line
  let capD = '';
  for (let x = 8; x < 452; x += 4) capD += sq(x, CAP_Y, 1.4);
  el(svg, 'path', { d: capD, fill: RULE, opacity: .8 });
  txt(svg, { x: 452, y: CAP_Y - 5, 'text-anchor': 'end', class: 'vt', 'font-size': 11.5, fill: FAINT }, '2.5k cap');

  // Paths for bars and cap roofs
  const paths = cols.map(c => el(svg, 'path', {
    d: '',
    fill: c.h === 'angelx' ? HERO : SER[c.h].color,
    class: c.h === 'angelx' ? 'ax-glow' : '',
  }));
  const capRoof = el(svg, 'path', { d: '', fill: '#f4f4f7', opacity: 0.95 });

  // Labels under the baseline
  cols.forEach(c => {
    const s = c.c.summary;
    const isAx = c.h === 'angelx';
    const mid = { 'text-anchor': 'middle' };
    const v = txt(svg, { x: c.cx, y: BASE + 18, class: `vt num ${isAx ? 'ax-txt-glow' : ''}`, 'font-size': 14, fill: isAx ? HERO : INK, ...mid },
      Math.round(s.out_per_task).toLocaleString('en-US') + ' tok');
    tip(v, `${c.c.harness_label} \u00b7 ${B.models[m].name} \u2014 ${Math.round(s.out_per_task)} output tokens and ${s.calls_mean.toFixed(1)} model calls per task`);
    txt(svg, { x: c.cx, y: BASE + 33, class: `vt ${isAx ? 'ax-txt-glow' : ''}`, 'font-size': 13, fill: isAx ? HERO : (SER[c.h].color === D3 ? DIM : SER[c.h].color), ...mid }, SER[c.h].label);
    txt(svg, { x: c.cx, y: BASE + 48, class: 'vt num', 'font-size': 12.5, fill: DIM, ...mid }, `${s.calls_mean.toFixed(1)} calls/task`);

    const stacks = Math.max(1, Math.ceil(s.out_per_task / CAP_TOKENS));
    if (stacks > 1) {
      txt(svg, { x: c.cx, y: BASE + 62, class: 'vt', 'font-size': 11, fill: FAINT, ...mid }, `${stacks}\u00d7 stacked`);
    }
  });

  play(state, Math.round(FPS * 5), p => {
    let allCapRoofs = '';
    cols.forEach((c, i) => {
      const s = c.c.summary;
      const currentTotal = p * s.out_per_task;
      const totalStacks = Math.max(1, Math.ceil(s.out_per_task / CAP_TOKENS));
      const isAx = c.h === 'angelx';

      // Determine bar width and spacing based on number of stacks so they are thick and centered
      let barW = 28, gap = 4;
      if (totalStacks === 2) { barW = 18; gap = 6; }
      else if (totalStacks === 3) { barW = 15; gap = 4; }
      else if (totalStacks >= 4) { barW = 13; gap = 4; }

      const totalGroupW = totalStacks * barW + (totalStacks - 1) * gap;
      const startX = Math.round(c.cx - totalGroupW / 2);

      let d = '';
      for (let sIdx = 0; sIdx < totalStacks; sIdx++) {
        const stackX = startX + sIdx * (barW + gap);
        const stackTokens = Math.min(CAP_TOKENS, Math.max(0, currentTotal - sIdx * CAP_TOKENS));
        if (stackTokens <= 0) continue;

        const stackH = (stackTokens / CAP_TOKENS) * H;
        const targetY = BASE - stackH;
        const dotPitchY = 3.4;
        const dotSize = isAx ? 2.5 : 2.2;

        // Render thick scanline/dot pattern across the full bar width
        for (let y = BASE - 2; y >= targetY; y -= dotPitchY) {
          for (let bx = 0; bx <= barW - 2; bx += 3) {
            d += sq(stackX + bx, Math.round(y), dotSize);
          }
        }

        // When reaching the 2.5k cap: solid glowing cap roof bar
        if (stackTokens >= CAP_TOKENS) {
          allCapRoofs += `M${stackX} ${CAP_Y - 2}h${barW}v2h-${barW}z`;
          allCapRoofs += `M${stackX + Math.round(barW / 2) - 2} ${CAP_Y - 5}h4v2h-4z`;
        }
      }
      paths[i].setAttribute('d', d);
    });
    capRoof.setAttribute('d', allCapRoofs);
  }, anim);
}

// ── model tabs: one tab per model in the data, holding every chart.
// Add a model to js/bench-data.js and it gets a tab here with no other edit.
const CHARTS = [
  { fig: 'tv-race', key: 'race', draw: race },
  { fig: 'tv-score', key: 'score', draw: scoreboard },
  { fig: 'tv-bars', key: 'bars', draw: bars },
  { fig: 'tv-trace', key: 'trace', draw: trace },
  { fig: 'tv-burn', key: 'burn', draw: burn },
  { fig: 'tv-cache', key: 'cache', draw: cache },
];

/* build one SVG per model inside each figure's panel grid, then wire the tabs */
const panels = [];   // { el, model }
CHARTS.forEach(({ fig, key }) => {
  const host = document.getElementById(fig)?.querySelector('[data-model-panels]');
  if (!host) return;
  MODELS.forEach(m => {
    const wrap = document.createElement('div');
    wrap.className = 'm-panel';
    wrap.dataset.model = m;
    wrap.hidden = true;
    const svg = el(wrap, 'svg', {
      id: `${key}-${m}`, viewBox: '0 0 460 262', role: 'img',
      'aria-label': `${B.models[m].name}: ${key} chart`,
    });
    host.appendChild(wrap);
    panels.push({ el: wrap, model: m });
  });
});

const tabHost = document.getElementById('model-tabs');
let activeModel = MODELS[0];
function applyTabs() {
  panels.forEach(p => { p.el.hidden = p.model !== activeModel; });
  if (tabHost) {
    tabHost.querySelectorAll('[role="tab"]').forEach(b => {
      const on = b.dataset.model === activeModel;
      b.setAttribute('aria-selected', on ? 'true' : 'false');
      b.tabIndex = on ? 0 : -1;
    });
  }
}
if (tabHost) {
  MODELS.forEach((m, i) => {
    const b = document.createElement('button');
    b.type = 'button';
    b.className = 'tab';
    b.dataset.model = m;
    b.setAttribute('role', 'tab');
    b.textContent = modelLabel(m);
    b.addEventListener('click', () => { activeModel = m; applyTabs(); });
    b.addEventListener('keydown', e => {
      const step = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
      if (!step) return;
      e.preventDefault();
      const next = MODELS[(i + step + MODELS.length) % MODELS.length];
      activeModel = next;
      applyTabs();
      tabHost.querySelector(`[data-model="${next}"]`).focus();
    });
    tabHost.appendChild(b);
  });
}
applyTabs();

// ── center-camera detector: returns true when element sits comfortably in central viewport
function isCenterCamera(el) {
  if (!el) return false;
  const r = el.getBoundingClientRect();
  const vh = window.innerHeight || document.documentElement.clientHeight;
  const mid = r.top + r.height / 2;
  return (mid >= vh * 0.25 && mid <= vh * 0.75) || (r.top <= vh * 0.5 && r.bottom >= vh * 0.5);
}

// ── wiring: draw when a figure scrolls into center camera; [ replay ] reruns
const FIGS = CHARTS.map(({ fig, key, draw }) => [
  fig,
  (st, anim) => MODELS.forEach(m => {
    const svg = document.getElementById(`${key}-${m}`);
    if (svg) draw(svg, m, st[m] = st[m] || {}, anim);
  }),
]);

FIGS.forEach(([id, draw]) => {
  const fig = document.getElementById(id);
  if (!fig) return;
  const state = { played: false };

  // 1. Immediately render the complete final benchmark state: every graph is fully drawn on load!
  draw(state, false);

  const replay = () => {
    draw(state, true);
  };
  const btn = fig.querySelector('.replay');
  if (btn) btn.addEventListener('click', replay);

  // Replay once when scrolled into center camera
  if (!REDUCED && 'IntersectionObserver' in window) {
    const io = new IntersectionObserver((entries, observer) => {
      entries.forEach(entry => {
        if (entry.isIntersecting && !state.played) {
          state.played = true;
          replay();
          observer.disconnect();
        }
      });
    }, { rootMargin: '-20% 0px -20% 0px', threshold: 0.15 });
    io.observe(fig);
  }
});

// ── table view: every number the plots draw
const tbody = document.getElementById('b-table');
if (tbody) {
  B.cells.forEach(c => {
    const s = c.summary, w = s.wall, tr = document.createElement('tr');
    [B.models[c.model].name, c.harness_label, `${s.solved}/${s.attempts}`, w.median.toFixed(1), w.mean.toFixed(1),
      s.calls_mean.toFixed(1), Math.round(s.in_per_task).toLocaleString('en-US'),
      Math.round(s.uncached_in_per_task).toLocaleString('en-US'), `${(100 * s.cache_hit).toFixed(1)}%`,
      Math.round(s.out_per_task).toLocaleString('en-US'), Math.round(s.reasoning_per_task).toLocaleString('en-US')]
      .forEach(v => { const td = document.createElement('td'); td.textContent = v; tr.appendChild(td); });
    tbody.appendChild(tr);
  });
}
})();
