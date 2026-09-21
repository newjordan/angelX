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
const INK = '#f4f4f7', DIM = '#a2a2ac', D3 = '#6f6f7a', FAINT = '#5b5b66', RULE = '#3a3a44', BG = '#050506';
const P = 3, S = 2, FPS = 12;             // dot pitch, dot size (viewBox units), frame rate
const REDUCED = !!(window.matchMedia && matchMedia('(prefers-reduced-motion: reduce)').matches);
const NS = 'http://www.w3.org/2000/svg';
const el = (p, t, a) => { const n = document.createElementNS(NS, t); for (const k in a) n.setAttribute(k, a[k]); p.appendChild(n); return n; };
const txt = (p, a, s) => { const n = el(p, 'text', a); n.textContent = s; return n; };
const tip = (n, s) => { const t = document.createElementNS(NS, 'title'); t.textContent = s; n.appendChild(t); };
const sq = (x, y, s = S) => `M${x} ${y}h${s}v${s}h-${s}z`;

// identity = label + gray + dot rhythm (never colour alone)
const SER = {
  angelx: { label: 'angelX', color: INK, on: i => true },
  opencode: { label: 'OpenCode', color: DIM, on: i => i % 2 === 0 },
  omp: { label: 'omp', color: D3, on: i => i % 4 < 2 },
};
const LEGEND_ORDER = ['angelx', 'opencode', 'omp'];
const DRAW_ORDER = ['omp', 'opencode', 'angelx'];      // angelX drawn last, on top
const MODELS = ['deepseek', 'glm'];
const cell = (m, h) => B.cells.find(c => c.model === m && c.harness === h);
const N = Math.max(...B.cells.map(c => c.attempts.length));
const fmtS = s => s >= 600 ? `${Math.floor(s / 60)}:${String(Math.round(s % 60)).padStart(2, '0')}` : `${Math.round(s)}s`;
const fmtClock = s => `T+${String(Math.floor(s / 60)).padStart(2, '0')}:${String(Math.floor(s % 60)).padStart(2, '0')}`;
const fmtTok = v => v >= 1e6 ? `${(v / 1e6).toFixed(2)}M` : `${Math.round(v / 1e3)}k`;
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
  const f = { ox: 46, oy: 32, GW: 136, GH: 62, ...o };
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
const yLabel = (svg, f, r, s) => txt(svg, { x: f.ox - 12, y: f.Y(r) + 5, 'text-anchor': 'end', class: 'vt', 'font-size': 15, fill: DIM }, s);
const xLabel = (svg, f, c, s, anchor = 'middle') => txt(svg, { x: f.X(c), y: f.Y(f.GH) + 20, 'text-anchor': anchor, class: 'vt', 'font-size': 15, fill: DIM }, s);

// 12 fps ticker with cancellation (the app's cadence)
function play(state, ticks, onTick) {
  (state.timers || []).forEach(clearInterval);
  state.timers = [];
  if (REDUCED || ticks <= 1) { onTick(1); return; }
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
function race(svg, m, state) {
  svg.innerHTML = '';
  const f = frame(svg, { oy: 46, GH: 58 });
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
  [N / 3, 2 * N / 3, N].forEach(v => {
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
  txt(svg, { x: f.X(f.GW - 1), y: f.Y(f.GH) + 36, 'text-anchor': 'end', class: 'vt', 'font-size': 14, fill: FAINT }, 'AGENT TIME →');
  const clock = txt(svg, { x: f.ox - 2 * P, y: 13, class: 'vt', 'font-size': 18, fill: INK }, '');
  const counters = {};
  LEGEND_ORDER.forEach((h, k) => {
    counters[h] = txt(svg, { x: f.ox - 2 * P + [0, 150, 316][k], y: 31, class: 'vt', 'font-size': 14.5, fill: SER[h].color === D3 ? DIM : SER[h].color }, '');
  });
  const paths = {}, dots = {};
  series.forEach(s => {
    dots[s.h] = trail(s.pts, toCell, true).filter(d => SER[s.h].on(d.i));
    paths[s.h] = el(svg, 'path', { d: '', fill: SER[s.h].color });
    tip(paths[s.h], `${SER[s.h].label} — ${s.solved}/${s.pts.length - 1} solved in ${Math.round(s.total)} s of agent time`);
  });
  const finish = el(svg, 'g', {});
  play(state, Math.round(FPS * 6.5), p => {
    const now = p * last;
    clock.textContent = `${fmtClock(now)}  AGENT CLOCK`;
    series.forEach(s => {
      paths[s.h].setAttribute('d', dots[s.h].filter(d => d.t <= now).map(d => sq(f.X(d.x), f.Y(d.y))).join(''));
      const done = s.pts.filter(q => q.t <= now).length - 1;
      counters[s.h].textContent = `${SER[s.h].label} ${String(done).padStart(2, '0')}/${s.pts.length - 1}` + (now >= s.total ? ` ✓${fmtClock(s.total).slice(2)}` : '');
    });
    if (p >= 1) {
      finish.innerHTML = '';
      series.forEach(s => {
        const e = toCell(s.pts[s.pts.length - 1]);
        const star = txt(finish, { x: f.X(e.c) + 1, y: f.Y(e.r) - 4, 'text-anchor': 'middle', class: 'vt', 'font-size': 15, fill: SER[s.h].color }, '✦');
        tip(star, `${SER[s.h].label} finished ${s.solved}/${s.pts.length - 1} at ${Math.round(s.total)} s`);
      });
    }
  });
}

// ════ SECONDS PER ATTEMPT — every attempt in run order, medians dotted ════
const MARK = {  // cell offsets: angelX block, OpenCode plus, omp cross
  angelx: [[0, 0], [1, 0], [0, 1], [1, 1]],
  opencode: [[0, 0], [-1, 0], [1, 0], [0, -1], [0, 1]],
  omp: [[0, 0], [-1, -1], [1, 1], [-1, 1], [1, -1]],
};
function trace(svg, m, state) {
  svg.innerHTML = '';
  const f = frame(svg);
  const cs = DRAW_ORDER.map(h => cell(m, h));
  const walls = cs.flatMap(c => c.attempts.map(a => a.wall_s)).sort((a, b) => a - b);
  const top = niceCeil(walls[Math.floor(walls.length * .96)] * 1.1);
  const toCell = (i, s) => ({ c: Math.round(i / (N - 1) * (f.GW - 3)) + 1, r: Math.round((1 - Math.min(s, top) / top) * (f.GH - 2)) + 1 });
  [0, .5, 1].forEach(q => yLabel(svg, f, Math.round((1 - q) * (f.GH - 2)) + 1, `${Math.round(q * top)}s`));
  [1, N / 3, 2 * N / 3, N].forEach((a, k) => xLabel(svg, f, toCell(a - 1, 0).c, k ? `#${a}` : '#1', k === 0 ? 'start' : k === 3 ? 'end' : 'middle'));
  txt(svg, { x: f.X(f.GW - 1), y: f.Y(f.GH) + 36, 'text-anchor': 'end', class: 'vt', 'font-size': 14, fill: FAINT }, 'ATTEMPT, IN RUN ORDER →');
  const readout = txt(svg, { x: f.ox - 2 * P, y: 14, class: 'vt', 'font-size': 16, fill: DIM }, '');
  const paths = {}, meds = {};
  cs.forEach(c => { paths[c.harness] = el(svg, 'path', { d: '', fill: SER[c.harness].color }); });
  cs.forEach(c => { meds[c.harness] = el(svg, 'path', { d: '', fill: SER[c.harness].color, opacity: .9 }); });
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
      c.attempts.slice(0, upto).forEach((a, i) => {
        const q = toCell(i, a.wall_s);
        MARK[c.harness].forEach(([dx, dy]) => { d += sq(f.X(q.c + dx), f.Y(q.r + dy)); });
        if (a.wall_s > top) d += sq(f.X(q.c), f.Y(q.r) - 2 * P, 1.4);
      });
      paths[c.harness].setAttribute('d', d);
    });
    if (p >= 1) {
      cs.forEach(c => {
        const r = toCell(0, c.summary.wall.median).r;
        let d = '';
        for (let x = 0; x < f.GW; x++) if (SER[c.harness].on(x)) d += sq(f.X(x), f.Y(r) + .5, 1.4);
        meds[c.harness].setAttribute('d', d);
      });
      readout.textContent = 'MEDIAN  ' + LEGEND_ORDER.map(h => `${SER[h].label} ${cell(m, h).summary.wall.median.toFixed(1)}s`).join('  ·  ');
    } else {
      readout.textContent = `ATTEMPT ${String(upto).padStart(2, '0')}/${N}`;
    }
  });
}

// ════ CONTEXT BURNED — cumulative input tokens over the run ════
function burn(svg, m, state) {
  svg.innerHTML = '';
  const f = frame(svg);
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
  [0, N / 3, 2 * N / 3, N].forEach((a, k) => xLabel(svg, f, toCell({ t: a, v: 0 }).c, `#${a}`, k === 0 ? 'start' : k === 3 ? 'end' : 'middle'));
  txt(svg, { x: f.X(f.GW - 1), y: f.Y(f.GH) + 36, 'text-anchor': 'end', class: 'vt', 'font-size': 14, fill: FAINT }, 'ATTEMPTS →');
  const readout = txt(svg, { x: f.ox - 2 * P, y: 14, class: 'vt', 'font-size': 16, fill: DIM }, '');
  const paths = {}, dots = {}, ends = el(svg, 'g', {});
  series.forEach(s => {
    dots[s.h] = trail(s.pts, toCell, false).filter(d => SER[s.h].on(d.i));
    paths[s.h] = el(svg, 'path', { d: '', fill: SER[s.h].color });
    tip(paths[s.h], `${SER[s.h].label} — ${Math.round(s.total).toLocaleString('en-US')} input tokens over ${N} attempts, ${Math.round(s.uncached).toLocaleString('en-US')} uncached`);
  });
  play(state, Math.round(FPS * 5), p => {
    const now = p * N;
    series.forEach(s => paths[s.h].setAttribute('d', dots[s.h].filter(d => d.t <= now).map(d => sq(f.X(d.x), f.Y(d.y))).join('')));
    if (p >= 1) {
      ends.innerHTML = '';
      const placed = [];
      series.slice().sort((a, b) => b.total - a.total).forEach(s => {
        let y = f.Y(toCell({ t: N, v: s.total }).r) - 6;
        while (placed.some(q => Math.abs(q - y) < 15)) y += 15;
        placed.push(y);
        txt(ends, { x: f.X(f.GW - 1) - 4, y, 'text-anchor': 'end', class: 'vt', 'font-size': 16, fill: SER[s.h].color === D3 ? DIM : SER[s.h].color,
          style: `paint-order:stroke;stroke:${BG};stroke-width:5px;stroke-linejoin:round` },
          `${SER[s.h].label} ${fmtTok(s.total)}`);
      });
      readout.textContent = 'UNCACHED  ' + LEGEND_ORDER.map(h => `${SER[h].label} ${fmtTok(series.find(s => s.h === h).uncached)}`).join('  ·  ');
    } else {
      readout.textContent = `ATTEMPT ${String(Math.round(now)).padStart(2, '0')}/${N}`;
    }
  });
}

// ════ CACHE — running cache hit rate over the run ════
function cache(svg, m, state) {
  svg.innerHTML = '';
  const f = frame(svg, { oy: 46, GH: 58 });
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
    yLabel(svg, f, r, `${v}%`);
  });
  [1, N / 3, 2 * N / 3, N].forEach((a, k) => xLabel(svg, f, toCell({ t: a, v: 100 }).c, `#${a}`, k === 0 ? 'start' : k === 3 ? 'end' : 'middle'));
  txt(svg, { x: f.X(f.GW - 1), y: f.Y(f.GH) + 36, 'text-anchor': 'end', class: 'vt', 'font-size': 14, fill: FAINT }, 'ATTEMPTS →');
  const l1 = txt(svg, { x: f.ox - 2 * P, y: 13, class: 'vt', 'font-size': 16, fill: INK }, '');
  const l2 = txt(svg, { x: f.ox - 2 * P, y: 31, class: 'vt', 'font-size': 16, fill: DIM }, '');
  const paths = {}, dots = {};
  series.forEach(s => {
    dots[s.h] = trail(s.pts, toCell, false).filter(d => SER[s.h].on(d.i));
    paths[s.h] = el(svg, 'path', { d: '', fill: SER[s.h].color });
    tip(paths[s.h], `${SER[s.h].label} — ${s.rate.toFixed(1)}% of input served from cache; ${Math.round(s.fresh).toLocaleString('en-US')} fresh tokens per task`);
  });
  play(state, Math.round(FPS * 5), p => {
    const now = 1 + p * (N - 1);
    series.forEach(s => paths[s.h].setAttribute('d', dots[s.h].filter(d => d.t <= now).map(d => sq(f.X(d.x), f.Y(d.y))).join('')));
    const at = Math.max(1, Math.floor(now));
    l1.textContent = 'HIT  ' + LEGEND_ORDER.map(h => { const s = series.find(x => x.h === h); return `${SER[h].label} ${s.pts[Math.min(at, N) - 1].v.toFixed(1)}%`; }).join('  ·  ');
    l2.textContent = p >= 1
      ? 'FRESH/TASK  ' + LEGEND_ORDER.map(h => `${SER[h].label} ${(series.find(x => x.h === h).fresh / 1000).toFixed(1)}k`).join('  ·  ')
      : `ATTEMPT ${String(at).padStart(2, '0')}/${N}`;
  });
}

// ════ SCOREBOARD — one lit dot per graded attempt ════
function scoreboard(svg, state) {
  svg.innerHTML = '';
  const X0 = 80, PITCH = 5.7, DOT = 4, GAP = 7, rows = [];
  let y = 40;
  MODELS.forEach(m => {
    txt(svg, { x: 0, y: y, class: 'vt', 'font-size': 15, fill: DIM }, m === 'deepseek' ? 'DEEPSEEK · THINKING OFF' : 'GLM · THINKING LOW');
    y += 10;
    LEGEND_ORDER.forEach(h => {
      const c = cell(m, h);
      txt(svg, { x: X0 - 10, y: y + 9, 'text-anchor': 'end', class: 'vt', 'font-size': 17, fill: SER[h].color === D3 ? DIM : SER[h].color }, SER[h].label);
      rows.push({ c, y });
      y += 22;
    });
    y += 16;
  });
  const total = B.cells.reduce((n, c) => n + c.attempts.length, 0);
  const readout = txt(svg, { x: 0, y: 14, class: 'vt', 'font-size': 18, fill: INK }, '');
  const lit = rows.map(() => el(svg, 'path', { d: '', fill: INK }));
  const miss = rows.map(() => el(svg, 'path', { d: '', fill: 'none', stroke: DIM, 'stroke-width': 1 }));
  const counts = rows.map(r => txt(svg, { x: 460, y: r.y + 9, 'text-anchor': 'end', class: 'vt', 'font-size': 17, fill: INK }, ''));
  const xAt = k => X0 + k * PITCH + Math.floor(k / (N / 3)) * GAP;
  rows.forEach((r, ri) => r.c.attempts.forEach((a, k) => {
    const hit = el(svg, 'rect', { x: xAt(k) - 1, y: r.y - 1, width: PITCH, height: DOT + 8, fill: 'transparent' });
    tip(hit, `${r.c.harness_label} · ${B.models[r.c.model].name} · ${a.task} · run ${a.run} — ${a.solved ? 'passed' : 'failed'} (${a.wall_s.toFixed(1)} s)`);
  }));
  play(state, N + 6, p => {
    const upto = Math.min(N, Math.ceil(p * (N + 6)));
    let passed = 0;
    rows.forEach((r, ri) => {
      let d = '', dm = '', ok = 0;
      r.c.attempts.slice(0, upto).forEach((a, k) => {
        if (a.solved) { d += sq(xAt(k), r.y + 2, DOT); ok++; } else { dm += `M${xAt(k) + .5} ${r.y + 2.5}h${DOT - 1}v${DOT - 1}h-${DOT - 1}z`; }
      });
      lit[ri].setAttribute('d', d);
      miss[ri].setAttribute('d', dm);
      counts[ri].textContent = `${ok}/${r.c.attempts.length}`;
      passed += ok;
    });
    readout.textContent = `PASSED ${String(passed).padStart(3, '0')}/${total}`;
  });
}

// ════ OUTPUT PER TASK — dot columns, the cockpit's bar style ════
function bars(svg, state) {
  svg.innerHTML = '';
  const UNIT = 25, PITCH = 4.4, DOT = 3, BASE = 196, cols = [];
  MODELS.forEach((m, g) => {
    const gx = g ? 262 : 14;
    txt(svg, { x: gx, y: 18, class: 'vt', 'font-size': 15, fill: DIM }, m === 'deepseek' ? 'DEEPSEEK' : 'GLM');
    LEGEND_ORDER.forEach((h, k) => cols.push({ c: cell(m, h), h, x: gx + 8 + k * 64 }));
  });
  let rule = '';
  for (let x = 6; x < 460; x += 3) rule += sq(x, BASE + 4, 1.4);
  el(svg, 'path', { d: rule, fill: RULE });
  const paths = cols.map(c => el(svg, 'path', { d: '', fill: SER[c.h].color }));
  cols.forEach(c => {
    const s = c.c.summary;
    const v = txt(svg, { x: c.x, y: BASE + 24, class: 'vt', 'font-size': 20, fill: INK }, Math.round(s.out_per_task).toLocaleString('en-US'));
    tip(v, `${c.c.harness_label} · ${B.models[c.c.model].name} — ${Math.round(s.out_per_task)} output tokens and ${s.calls_mean.toFixed(1)} model calls per task`);
    txt(svg, { x: c.x, y: BASE + 40, class: 'vt', 'font-size': 16, fill: SER[c.h].color === D3 ? DIM : SER[c.h].color }, SER[c.h].label);
    txt(svg, { x: c.x, y: BASE + 55, class: 'vt', 'font-size': 14, fill: DIM }, `${s.calls_mean.toFixed(1)} calls`);
  });
  const maxRows = Math.max(...cols.map(c => Math.round(c.c.summary.out_per_task / UNIT)));
  play(state, maxRows + 4, p => {
    const rowsNow = Math.ceil(p * (maxRows + 4));
    cols.forEach((c, i) => {
      const n = Math.min(rowsNow, Math.round(c.c.summary.out_per_task / UNIT));
      let d = '';
      for (let r = 0; r < n; r++) for (let k = 0; k < 3; k++) d += sq(c.x + k * PITCH, BASE - r * PITCH - DOT, DOT);
      paths[i].setAttribute('d', d);
    });
  });
}

// ── wiring: draw when a figure scrolls into view (or at once on the report page); [ replay ] reruns
const FIGS = [
  ['tv-race', st => MODELS.forEach(m => race(document.getElementById(`race-${m}`), m, st[m] = st[m] || {}))],
  ['tv-score', st => scoreboard(document.getElementById('score'), st)],
  ['tv-bars', st => bars(document.getElementById('bars'), st)],
  ['tv-trace', st => MODELS.forEach(m => trace(document.getElementById(`trace-${m}`), m, st[m] = st[m] || {}))],
  ['tv-burn', st => MODELS.forEach(m => burn(document.getElementById(`burn-${m}`), m, st[m] = st[m] || {}))],
  ['tv-cache', st => MODELS.forEach(m => cache(document.getElementById(`cache-${m}`), m, st[m] = st[m] || {}))],
];
FIGS.forEach(([id, draw]) => {
  const fig = document.getElementById(id);
  if (!fig) return;
  const state = {};
  const go = () => draw(state);
  const btn = fig.querySelector('.replay');
  if (btn) btn.addEventListener('click', go);
  if (window.BENCH_EAGER || !('IntersectionObserver' in window)) { go(); return; }
  const io = new IntersectionObserver(es => { if (es[0].isIntersecting) { go(); io.disconnect(); } }, { threshold: .25 });
  io.observe(fig);
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
