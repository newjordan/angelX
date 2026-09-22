const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');
const vm = require('vm');

const REPO_ROOT = path.resolve(__dirname, '..');
const OUTPUT_MP4 = path.join(REPO_ROOT, 'website', 'assets', 'scoreboard-timeline.mp4');

// Load benchmark data
const sandbox = { window: {} };
vm.runInNewContext(fs.readFileSync(path.join(REPO_ROOT, 'website', 'js', 'bench-data.js'), 'utf8'), sandbox);
const BENCH = sandbox.window.BENCH;

const HTML_CONTENT = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Scoreboard Timeline — angelX</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Cinzel:wght@600;700;900&family=VT323&display=swap" rel="stylesheet">
<style>
  :root {
    --bg: #050506;
    --card-bg: rgba(11, 11, 14, 0.96);
    --ink: #f4f4f7;
    --dim: #a2a2ac;
    --faint: #686875;
    --rule: #282833;
    --rule-light: #3e3e4d;
    --hero: #ffffff;
    --mono: ui-monospace, 'Cascadia Mono', Menlo, Consolas, monospace;
    --dos: 'VT323', ui-monospace, monospace;
    --fantasy: 'Cinzel', serif;
  }
  * { box-sizing: border-box; margin: 0; padding: 0; }
  html, body {
    width: 1920px;
    height: 1080px;
    overflow: hidden;
    background: var(--bg);
    color: var(--ink);
    font-family: var(--mono);
  }
  .stage {
    width: 1920px;
    height: 1080px;
    padding: 34px 60px 26px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
  }
  /* Masthead */
  .mast {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    border-bottom: 1px solid var(--rule-light);
    padding-bottom: 14px;
  }
  .kicker {
    font-family: var(--dos);
    font-size: 21px;
    letter-spacing: 0.22em;
    color: var(--faint);
    text-transform: uppercase;
    margin-bottom: 4px;
  }
  h1 {
    font-family: var(--fantasy);
    font-size: 42px;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: var(--hero);
    margin: 0;
  }
  .mast-sub {
    font-size: 14.5px;
    color: var(--dim);
    letter-spacing: 0.02em;
    text-align: right;
    line-height: 1.5;
  }

  /* Scoreboard Card */
  .tv-box {
    position: relative;
    border: 1px solid var(--rule-light);
    background: var(--card-bg);
    padding: 22px 30px 16px;
    height: 575px;
    display: flex;
    flex-direction: column;
    box-shadow: 0 12px 40px rgba(0,0,0,0.8);
  }
  .tv-tag {
    position: absolute;
    top: 0;
    left: 24px;
    transform: translateY(-50%);
    background: var(--bg);
    padding: 0 12px;
    font-family: var(--dos);
    font-size: 17px;
    color: var(--dim);
    letter-spacing: 0.08em;
  }
  .tv-badge {
    position: absolute;
    top: 0;
    right: 24px;
    transform: translateY(-50%);
    background: var(--bg);
    padding: 0 12px;
    font-family: var(--dos);
    font-size: 17px;
    color: var(--hero);
    letter-spacing: 0.1em;
    text-shadow: 0 0 8px rgba(255,255,255,0.8);
  }
  svg {
    width: 100%;
    height: 100%;
    display: block;
    overflow: visible;
  }
  .vt { font-family: var(--dos); }
  .mono { font-family: var(--mono); }
  
  .ax-glow {
    filter: drop-shadow(0 0 3px #ffffff) drop-shadow(0 0 8px rgba(255,255,255,0.85));
  }
  .ax-txt-glow {
    filter: drop-shadow(0 0 6px rgba(255,255,255,0.85));
    fill: #ffffff !important;
  }

  /* Bottom 3 Summary Cards */
  .cards {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 24px;
    height: 175px;
  }
  .card {
    border: 1px solid var(--rule);
    background: var(--card-bg);
    padding: 16px 20px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
  }
  .card.hero {
    border-color: rgba(255,255,255,0.45);
    box-shadow: 0 0 24px rgba(255,255,255,0.08);
  }
  .card-top {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .card-name {
    font-family: var(--dos);
    font-size: 26px;
    font-weight: 700;
    letter-spacing: 0.06em;
  }
  .card-name.hero { color: #fff; text-shadow: 0 0 8px rgba(255,255,255,0.7); }
  .card-name.dim { color: var(--dim); }
  .card-name.faint { color: var(--faint); }
  .card-rate {
    font-family: var(--dos);
    font-size: 28px;
    font-weight: 700;
  }
  .card-rate.hero { color: #fff; text-shadow: 0 0 10px rgba(255,255,255,0.8); }
  .card-rate.dim { color: var(--dim); }
  .card-rate.faint { color: var(--faint); }
  .card-metrics {
    font-size: 13.5px;
    color: var(--dim);
    line-height: 1.6;
  }
  .card-status {
    font-family: var(--dos);
    font-size: 17px;
    letter-spacing: 0.08em;
    padding-top: 6px;
    border-top: 1px dashed var(--rule);
  }
  .card-status.hero { color: #fff; font-weight: 700; }
  .card-status.dim { color: var(--dim); }
  .card-status.warn { color: #e5c07b; }

  /* Footer */
  .footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 12.5px;
    color: var(--faint);
    letter-spacing: 0.04em;
    padding-top: 6px;
  }
  .legend-items {
    display: flex;
    gap: 20px;
    font-family: var(--dos);
    font-size: 16px;
    color: var(--dim);
  }
  .legend-items span { display: flex; align-items: center; gap: 6px; }
  .legend-items .pass-box { width: 9px; height: 9px; background: #fff; box-shadow: 0 0 6px #fff; }
  .legend-items .fail-box { width: 9px; height: 9px; border: 1.5px solid #e06c75; }
</style>
</head>
<body>
<div class="stage">

  <header class="mast">
    <div>
      <p class="kicker">angelX · measured telemetry</p>
      <h1>Scoreboard Timeline</h1>
    </div>
    <div class="mast-sub">
      <p><strong>136 repository-repair tasks</strong> (JS, Python, Rust, C++)</p>
      <p>DeepSeek V4.1 Flash · thinking off · 600s wall cap per attempt</p>
    </div>
  </header>

  <div class="tv-box">
    <div class="tv-tag">◇ measured · scoreboard timeline · 2026-09-21 ◇</div>
    <div class="tv-badge" id="status-badge">● 136 TASKS EVALUATED</div>
    <svg id="scoreboard-svg" viewBox="0 0 1740 500"></svg>
  </div>

  <div class="cards">
    <div class="card hero">
      <div class="card-top">
        <span class="card-name hero">angelX</span>
        <span class="card-rate hero" id="card-rate-ax">0 / 136</span>
      </div>
      <div class="card-metrics">
        <div>Agent time: <strong>34.5 min</strong> (fastest by far)</div>
        <div>Model calls: <strong>7.5 / task</strong> · 1,891 tokens/task</div>
      </div>
      <div class="card-status hero" id="stat-angelx">RUNNING...</div>
    </div>

    <div class="card">
      <div class="card-top">
        <span class="card-name dim">OpenCode 1.18.31</span>
        <span class="card-rate dim" id="card-rate-oc">0 / 136</span>
      </div>
      <div class="card-metrics">
        <div>Agent time: <strong>58.7 min</strong> (+70% slower)</div>
        <div>Model calls: <strong>12.5 / task</strong> · 2,568 tokens/task</div>
      </div>
      <div class="card-status dim" id="stat-opencode">RUNNING...</div>
    </div>

    <div class="card">
      <div class="card-top">
        <span class="card-name faint">oh-my-pi 18.2.4</span>
        <span class="card-rate faint" id="card-rate-omp">0 / 59</span>
      </div>
      <div class="card-metrics">
        <div>Agent time: <strong>76.5 min (1.3h)</strong></div>
        <div>Model calls: <strong>38.3 / task</strong> · 8,683 tokens/task</div>
      </div>
      <div class="card-status warn" id="stat-omp">RUNNING...</div>
    </div>
  </div>

  <footer class="footer">
    <div class="legend-items">
      <span><i class="pass-box"></i> passed attempt (tests pass)</span>
      <span><i class="fail-box"></i> failed attempt</span>
      <span style="margin-left:14px">| Evaluator traces — zero self-reporting</span>
    </div>
    <div>Graded by task-native test suites on Prime Intellect evaluators (Verifiers v0.3.1) · <strong>angelx.dev</strong></div>
  </footer>

</div>

<script>
window.BENCH = ${JSON.stringify(BENCH)};

const svg = document.getElementById('scoreboard-svg');
const NS = 'http://www.w3.org/2000/svg';
const el = (p, t, a) => { const n = document.createElementNS(NS, t); for (const k in a) n.setAttribute(k, a[k]); p.appendChild(n); return n; };
const txt = (p, a, s) => { const n = el(p, 'text', a); n.textContent = s; return n; };

const B = window.BENCH;
const m = 'deepseek';
const cell = (h) => B.cells.find(c => c.model === m && c.harness === h);

const X0 = 240, W = 1240;
const rows = [];
const ROW_Y = [115, 215, 315];
const HARNESSES = ['angelx', 'opencode', 'omp'];

HARNESSES.forEach((h, i) => {
  const c = cell(h);
  let t = 0;
  const times = c.attempts.map(a => (t += a.wall_s));
  rows.push({ c, h, y: ROW_Y[i], times, totalTime: t, solvedTotal: c.attempts.filter(a => a.solved).length });
});

const T = Math.max(...rows.map(r => r.times[r.times.length - 1] || 0)); // 4588.3s
const xAt = t => Math.round(X0 + (t / T) * W);
const axisY = 385;

// Static Grid vertical lines every 15 min up to 60m
for (let q = 0; q <= 60; q += 15) {
  const x = xAt(q * 60);
  let d = '';
  for (let y = 70; y < axisY - 10; y += 8) d += 'M' + x + ' ' + y + 'h2.5v2.5h-2.5z';
  el(svg, 'path', { d, fill: '#333342', opacity: 0.75 });
  txt(svg, { x, y: axisY + 24, 'text-anchor': q === 0 ? 'start' : 'middle', class: 'vt', 'font-size': 21, fill: '#a2a2ac' }, q + 'm');
}
// 1.3h marker at end of timeline (budget cap)
const xMax = xAt(T);
let dCap = '';
for (let y = 70; y < axisY - 10; y += 8) dCap += 'M' + xMax + ' ' + y + 'h2.5v2.5h-2.5z';
el(svg, 'path', { d: dCap, fill: '#554830', opacity: 0.85 });
txt(svg, { x: xMax, y: axisY + 24, 'text-anchor': 'middle', class: 'vt', 'font-size': 21, fill: '#e5c07b' }, '1.3h (cap)');
txt(svg, { x: X0 + W, y: axisY + 48, 'text-anchor': 'end', class: 'vt', 'font-size': 18, fill: '#686875' }, 'cumulative agent time to completion (min)');

// Dotted track lines for lanes
rows.forEach(r => {
  let trackD = '';
  for (let x = X0; x < X0 + W; x += 6) trackD += 'M' + x + ' ' + (r.y - 1) + 'h2v2h-2z';
  el(svg, 'path', { d: trackD, fill: '#2e2e3d', opacity: 0.8 });
});

// Row labels on left
rows.forEach(r => {
  const isAx = r.h === 'angelx';
  const label = isAx ? 'angelX' : (r.h === 'opencode' ? 'OpenCode' : 'oh-my-pi');
  txt(svg, {
    x: X0 - 24, y: r.y + 7, 'text-anchor': 'end',
    class: 'vt ' + (isAx ? 'ax-txt-glow' : ''),
    'font-size': isAx ? 28 : 24,
    fill: isAx ? '#ffffff' : (r.h === 'opencode' ? '#a2a2ac' : '#7d8590')
  }, label);
});

// Dynamic Elements
const readout = txt(svg, { x: 20, y: 34, class: 'vt', 'font-size': 24, fill: '#ffffff' }, '');
const clock = txt(svg, { x: 1720, y: 34, 'text-anchor': 'end', class: 'vt', 'font-size': 24, fill: '#a2a2ac' }, '');

// Sweep line cursor
const sweepLine = el(svg, 'line', {
  x1: X0, y1: 65, x2: X0, y2: axisY - 6,
  stroke: '#ffffff', 'stroke-width': 1.5, 'stroke-dasharray': '3 3', opacity: 0.3
});
const sweepPointer = el(svg, 'polygon', { points: '', fill: '#ffffff', opacity: 0.65 });

const litPaths = rows.map(r => el(svg, 'path', { d: '', fill: r.h === 'angelx' ? '#ffffff' : (r.h === 'opencode' ? '#a2a2ac' : '#7d8590'), class: r.h === 'angelx' ? 'ax-glow' : '' }));
const missPaths = rows.map(() => el(svg, 'path', { d: '', fill: 'none', stroke: '#e06c75', 'stroke-width': 1.8 }));
const countTexts = rows.map(r => txt(svg, {
  x: 1720, y: r.y + 8, 'text-anchor': 'end',
  class: 'vt ' + (r.h === 'angelx' ? 'ax-txt-glow' : ''),
  'font-size': r.h === 'angelx' ? 28 : 24,
  fill: r.h === 'angelx' ? '#ffffff' : '#a2a2ac'
}, ''));
const spanTexts = rows.map(r => txt(svg, { x: 0, y: r.y + 36, class: 'vt', 'font-size': 18, fill: '#888899' }, ''));

const statCards = {
  angelx: document.getElementById('stat-angelx'),
  opencode: document.getElementById('stat-opencode'),
  omp: document.getElementById('stat-omp')
};
const rateCards = {
  angelx: document.getElementById('card-rate-ax'),
  opencode: document.getElementById('card-rate-oc'),
  omp: document.getElementById('card-rate-omp')
};

const fmtMin = sec => sec >= 3600 ? ((sec / 3600).toFixed(1) + 'h') : (Math.round(sec / 60) + 'm');
const fmtClock = s => 'T+' + String(Math.floor(s / 60)).padStart(2, '0') + ':' + String(Math.floor(s % 60)).padStart(2, '0');

window.renderFrame = function(p) {
  const now = p * T;
  let totalPassed = 0;
  
  // Sweep cursor position
  const curX = Math.min(xAt(now), X0 + W);
  sweepLine.setAttribute('x1', curX);
  sweepLine.setAttribute('x2', curX);
  sweepPointer.setAttribute('points', (curX - 4) + ',62 ' + (curX + 4) + ',62 ' + curX + ',69');

  rows.forEach((r, ri) => {
    let d = '', dm = '', ok = 0, last = 0;
    const isAx = r.h === 'angelx';
    const dotH = isAx ? 7 : 5;
    const dotW = isAx ? 6 : 4;
    
    r.c.attempts.forEach((a, k) => {
      const t = r.times[k];
      if (t > now) return;
      last = t;
      const px = xAt(t);
      if (a.solved) {
        d += 'M' + px + ' ' + (r.y - Math.round(dotH/2)) + 'h' + dotW + 'v' + dotH + 'h-' + dotW + 'z';
        ok++;
      } else {
        dm += 'M' + px + ' ' + (r.y - Math.round(dotH/2)) + 'h' + dotW + 'v' + dotH + 'h-' + dotW + 'z';
      }
    });

    litPaths[ri].setAttribute('d', d);
    missPaths[ri].setAttribute('d', dm);
    countTexts[ri].textContent = ok + ' / ' + r.c.attempts.length;
    
    const pct = ((ok / r.c.attempts.length) * 100).toFixed(1);
    rateCards[r.h].textContent = ok + ' / ' + r.c.attempts.length + ' (' + pct + '%)';

    if (last > 0) {
      const isFinished = now >= r.totalTime;
      const pxLast = xAt(last);
      
      if (r.h === 'angelx') {
        spanTexts[ri].setAttribute('x', pxLast + 12);
        spanTexts[ri].setAttribute('text-anchor', 'start');
        if (isFinished) {
          spanTexts[ri].setAttribute('fill', '#ffffff');
          spanTexts[ri].textContent = '34.5m ★ FINISHED (ALL 136 TASKS)';
          statCards.angelx.textContent = '★ FINISHED at 34.5m (ALL 136 TASKS)';
        } else {
          spanTexts[ri].setAttribute('fill', '#ffffff');
          spanTexts[ri].textContent = fmtMin(last) + ' · ' + ok + '/136';
          statCards.angelx.textContent = 'RUNNING: ' + ok + '/136 (' + fmtMin(last) + ')';
        }
      } else if (r.h === 'opencode') {
        spanTexts[ri].setAttribute('x', pxLast + 12);
        spanTexts[ri].setAttribute('text-anchor', 'start');
        if (isFinished) {
          spanTexts[ri].setAttribute('fill', '#a2a2ac');
          spanTexts[ri].textContent = '58.7m (FINISHED)';
          statCards.opencode.textContent = 'FINISHED at 58.7m (+70% slower)';
        } else {
          spanTexts[ri].setAttribute('fill', '#888899');
          spanTexts[ri].textContent = fmtMin(last) + ' · ' + ok + '/136';
          statCards.opencode.textContent = 'RUNNING: ' + ok + '/136 (' + fmtMin(last) + ')';
        }
      } else if (r.h === 'omp') {
        if (isFinished) {
          // Anchor to end if near edge to prevent collision with right count text
          spanTexts[ri].setAttribute('x', pxLast - 12);
          spanTexts[ri].setAttribute('text-anchor', 'end');
          spanTexts[ri].setAttribute('fill', '#e5c07b');
          spanTexts[ri].textContent = '1.3h (BUDGET CAP REACHED)';
          statCards.omp.textContent = '▲ STOPPED AT 59/136 (200M TOKEN BUDGET)';
        } else {
          spanTexts[ri].setAttribute('x', pxLast + 12);
          spanTexts[ri].setAttribute('text-anchor', 'start');
          spanTexts[ri].setAttribute('fill', '#888899');
          spanTexts[ri].textContent = fmtMin(last) + ' · ' + ok + '/59';
          statCards.omp.textContent = 'RUNNING: ' + ok + '/59 (' + fmtMin(last) + ')';
        }
      }
    } else {
      spanTexts[ri].textContent = '';
      statCards[r.h].textContent = 'STARTING...';
    }
    
    totalPassed += ok;
  });

  readout.textContent = 'PASSED ' + totalPassed + ' / 331 ATTEMPTS';
  clock.textContent = fmtClock(now);
};

// Render completed state by default
window.renderFrame(1.0);
</script>
</body>
</html>`;

const HTML_PATH = '/tmp/scoreboard_video.html';
fs.writeFileSync(HTML_PATH, HTML_CONTENT);
console.log('Written', HTML_PATH);

async function main() {
  const isTestFrame = process.argv.includes('--test-frame');
  const chrome = spawn('chromium', [
    '--headless=new',
    '--remote-debugging-port=9238',
    '--disable-gpu',
    '--no-sandbox',
    '--window-size=1920,1080',
    'file://' + HTML_PATH
  ]);

  let killed = false;
  const cleanup = () => {
    if (!killed) {
      killed = true;
      try { chrome.kill(); } catch (e) {}
    }
  };
  process.on('exit', cleanup);
  process.on('SIGINT', cleanup);

  setTimeout(async () => {
    try {
      const res = await fetch('http://127.0.0.1:9238/json');
      const tabs = await res.json();
      const pageTab = tabs.find(t => t.type === 'page');
      if (!pageTab) throw new Error('No page tab found');
      const ws = new WebSocket(pageTab.webSocketDebuggerUrl);

      let nextId = 1;
      const pending = new Map();

      ws.onmessage = (event) => {
        const msg = JSON.parse(event.data);
        if (msg.id && pending.has(msg.id)) {
          const { resolve, reject } = pending.get(msg.id);
          pending.delete(msg.id);
          if (msg.error) reject(new Error(msg.error.message || JSON.stringify(msg.error)));
          else resolve(msg.result);
        }
      };

      function callCdp(method, params = {}) {
        const id = nextId++;
        return new Promise((resolve, reject) => {
          pending.set(id, { resolve, reject });
          ws.send(JSON.stringify({ id, method, params }));
        });
      }

      ws.onopen = async () => {
        await callCdp('Runtime.enable');
        await callCdp('Page.enable');
        await callCdp('Emulation.setDeviceMetricsOverride', {
          width: 1920,
          height: 1080,
          deviceScaleFactor: 1,
          mobile: false
        });

        // Wait for webfonts
        await new Promise(r => setTimeout(r, 800));
        await callCdp('Runtime.evaluate', {
          expression: 'document.fonts.ready.then(() => true);',
          awaitPromise: true
        });

        console.log('Viewport set and fonts ready.');

        if (isTestFrame) {
          console.log('Capturing test frames...');
          // Mid-race frame (p = 0.5)
          await callCdp('Runtime.evaluate', { expression: 'window.renderFrame(0.5);' });
          const snapMid = await callCdp('Page.captureScreenshot', {
            format: 'png',
            clip: { x: 0, y: 0, width: 1920, height: 1080, scale: 1 }
          });
          fs.writeFileSync('/tmp/scoreboard_mid.png', Buffer.from(snapMid.data, 'base64'));
          console.log('Saved /tmp/scoreboard_mid.png');

          // Finished frame (p = 1.0)
          await callCdp('Runtime.evaluate', { expression: 'window.renderFrame(1.0);' });
          const snapDone = await callCdp('Page.captureScreenshot', {
            format: 'png',
            clip: { x: 0, y: 0, width: 1920, height: 1080, scale: 1 }
          });
          fs.writeFileSync('/tmp/scoreboard_done.png', Buffer.from(snapDone.data, 'base64'));
          console.log('Saved /tmp/scoreboard_done.png');

          cleanup();
          process.exit(0);
          return;
        }

        console.log('Spawning ffmpeg...');
        const ffmpeg = spawn('/usr/bin/ffmpeg', [
          '-y',
          '-f', 'image2pipe',
          '-vcodec', 'png',
          '-r', '30',
          '-i', '-',
          '-vf', 'scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2,format=yuv420p',
          '-c:v', 'libx264',
          '-preset', 'slow',
          '-crf', '17',
          '-movflags', '+faststart',
          OUTPUT_MP4
        ], { stdio: ['pipe', 'inherit', 'inherit'] });

        ffmpeg.on('error', err => console.error('ffmpeg error:', err));
        ffmpeg.on('close', code => {
          console.log('ffmpeg exited with code', code);
          cleanup();
          process.exit(code);
        });

        // Frame timing: 30 fps
        // Total 420 frames = 14 seconds
        // 0 to 14: intro holding at 0 (0.5s)
        // 15 to 285: animation running 0.0 to 1.0 (9.0s)
        // 286 to 419: freeze frame at 1.0 (4.5s)
        const TOTAL_FRAMES = 420;
        const RUN_START = 15;
        const RUN_END = 285;

        console.log('Rendering ' + TOTAL_FRAMES + ' frames...');
        const t0 = Date.now();
        let lastPngBuffer = null;

        for (let frame = 0; frame < TOTAL_FRAMES; frame++) {
          let p = 0;
          if (frame >= RUN_END) {
            p = 1.0;
          } else if (frame >= RUN_START) {
            p = (frame - RUN_START) / (RUN_END - RUN_START);
          }

          let pngBuffer;
          if (frame >= RUN_END && lastPngBuffer) {
            pngBuffer = lastPngBuffer;
          } else {
            await callCdp('Runtime.evaluate', {
              expression: 'window.renderFrame(' + p + ');'
            });

            const snap = await callCdp('Page.captureScreenshot', {
              format: 'png',
              clip: { x: 0, y: 0, width: 1920, height: 1080, scale: 1 }
            });
            pngBuffer = Buffer.from(snap.data, 'base64');
            if (p === 1.0) lastPngBuffer = pngBuffer;
          }

          const ok = ffmpeg.stdin.write(pngBuffer);
          if (!ok) {
            await new Promise(r => ffmpeg.stdin.once('drain', r));
          }

          if (frame % 30 === 0 || frame === TOTAL_FRAMES - 1) {
            const elapsed = ((Date.now() - t0) / 1000).toFixed(1);
            console.log('Frame ' + frame + '/' + TOTAL_FRAMES + ' (' + (frame/30).toFixed(1) + 's, ' + elapsed + 's elapsed)');
          }
        }

        console.log('All frames sent. Finalizing video...');
        ffmpeg.stdin.end();
      };
    } catch (e) {
      console.error(e);
      cleanup();
      process.exit(1);
    }
  }, 1000);
}

main();
