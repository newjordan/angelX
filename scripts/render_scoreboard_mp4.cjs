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
<link href="https://fonts.googleapis.com/css2?family=Cinzel:wght@600;700&family=VT323&display=swap" rel="stylesheet">
<style>
  :root {
    --bg: #050506;
    --card-bg: rgba(9, 9, 12, 0.96);
    --ink: #f4f4f7;
    --dim: #a2a2ac;
    --faint: #686875;
    --rule: #282833;
    --rule-card: #3a3a44;
    --hero: #ffffff;
    --mono: ui-monospace, 'Cascadia Mono', Menlo, Consolas, monospace;
    --dos: 'VT323', ui-monospace, monospace;
    --fantasy: 'Cinzel', serif;
  }
  * { box-sizing: border-box; margin: 0; padding: 0; }
  html, body {
    width: 1080px;
    height: 1080px;
    overflow: hidden;
    background: var(--bg);
    color: var(--ink);
    font-family: var(--mono);
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
  }
  .stage {
    width: 960px;
    display: flex;
    flex-direction: column;
  }
  /* Header */
  .sec-head {
    margin-bottom: 22px;
  }
  .sec-kicker {
    font-family: var(--dos);
    font-size: 20px;
    letter-spacing: 0.2em;
    color: var(--faint);
    text-transform: uppercase;
    margin-bottom: 2px;
  }
  .sec-title {
    font-family: var(--fantasy);
    font-size: 42px;
    font-weight: 700;
    letter-spacing: 0.03em;
    color: var(--hero);
    line-height: 1.15;
  }
  .sec-sub {
    font-family: var(--mono);
    font-size: 14px;
    color: var(--dim);
    margin-top: 4px;
    letter-spacing: 0.01em;
  }

  /* TV Frame (website .tv style) */
  .tv {
    position: relative;
    border: 1px solid var(--rule-card);
    background: var(--card-bg);
    padding: 28px 24px 18px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.75);
  }
  .tv-title {
    position: absolute;
    top: 0;
    left: 20px;
    transform: translateY(-55%);
    background: var(--bg);
    padding: 0 10px;
    font-family: var(--mono);
    font-size: 13.5px;
    letter-spacing: 0.01em;
    color: var(--ink);
    white-space: nowrap;
  }
  .tv-replay {
    position: absolute;
    top: 0;
    right: 20px;
    transform: translateY(-55%);
    background: var(--bg);
    border: 1px solid var(--rule-card);
    color: var(--dim);
    font-family: var(--dos);
    font-size: 16px;
    letter-spacing: 0.08em;
    padding: 0 10px;
    line-height: 1.4;
  }
  svg {
    width: 100%;
    height: 520px;
    display: block;
    overflow: visible;
  }
  .vt { font-family: var(--dos); }
  .num { font-weight: 700; }
  
  .ax-glow {
    filter: drop-shadow(0 0 2px #ffffff) drop-shadow(0 0 6px rgba(255,255,255,0.85));
  }
  .ax-txt-glow {
    filter: drop-shadow(0 0 6px rgba(255,255,255,0.85));
    fill: #ffffff !important;
  }

  /* Legend */
  .legend {
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 32px;
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px dashed var(--rule);
    font-family: var(--dos);
    font-size: 18px;
    letter-spacing: 0.06em;
    color: var(--dim);
  }
  .legend span {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  .legend .pass {
    width: 9px;
    height: 9px;
    background: #ffffff;
    box-shadow: 0 0 6px rgba(255,255,255,0.8);
    display: inline-block;
  }
  .legend .fail {
    width: 9px;
    height: 9px;
    border: 1.5px solid #a2a2ac;
    display: inline-block;
  }
  .legend .ax-hero {
    color: #ffffff;
    font-weight: 700;
    text-shadow: 0 0 8px rgba(255,255,255,0.6);
  }

  /* Footer */
  .provenance {
    display: flex;
    justify-content: space-between;
    margin-top: 20px;
    font-family: var(--dos);
    font-size: 17px;
    letter-spacing: 0.04em;
    color: var(--faint);
  }
  .provenance strong {
    color: var(--dim);
  }
</style>
</head>
<body>
<div class="stage">

  <div class="sec-head">
    <p class="sec-kicker">angelX · measured telemetry</p>
    <h1 class="sec-title">Scoreboard</h1>
    <p class="sec-sub">Every attempt placed at the moment it completed · DeepSeek V4.1 Flash</p>
  </div>

  <div class="tv">
    <div class="tv-title">Scoreboard · DeepSeek V4.1 Flash · thinking off</div>
    <div class="tv-replay">[ 136 TASKS ]</div>
    <svg id="scoreboard-svg" viewBox="0 0 912 480"></svg>
    <div class="legend">
      <span class="ax-hero"><i class="pass"></i>passed</span>
      <span><i class="fail"></i>failed</span>
    </div>
  </div>

  <footer class="provenance">
    <div>Graded on Prime Intellect evaluators (Verifiers v0.3.1)</div>
    <div><strong>angelx.dev</strong></div>
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

const X0 = 135, W = 625;
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
const axisY = 395;

// Grid vertical lines every 15 min up to 60m
for (let q = 0; q <= 60; q += 15) {
  const x = xAt(q * 60);
  let d = '';
  for (let y = 70; y < axisY - 8; y += 7) d += 'M' + x + ' ' + y + 'h2v2h-2z';
  el(svg, 'path', { d, fill: '#333342', opacity: 0.65 });
  txt(svg, { x, y: axisY + 22, 'text-anchor': q === 0 ? 'start' : 'middle', class: 'vt', 'font-size': 18, fill: '#a2a2ac' }, q + 'm');
}
// 1.3h marker at end of timeline (budget cap)
const xMax = xAt(T);
let dCap = '';
for (let y = 70; y < axisY - 8; y += 7) dCap += 'M' + xMax + ' ' + y + 'h2v2h-2z';
el(svg, 'path', { d: dCap, fill: '#4a4230', opacity: 0.75 });
txt(svg, { x: xMax, y: axisY + 22, 'text-anchor': 'middle', class: 'vt', 'font-size': 18, fill: '#e5c07b' }, '1.3h (cap)');
txt(svg, { x: X0 + W, y: axisY + 46, 'text-anchor': 'end', class: 'vt', 'font-size': 15, fill: '#686875' }, 'cumulative agent time to completion (min)');

// Row labels on left
rows.forEach(r => {
  const isAx = r.h === 'angelx';
  const label = isAx ? 'angelX' : (r.h === 'opencode' ? 'OpenCode' : 'oh-my-pi');
  txt(svg, {
    x: X0 - 16, y: r.y + 7, 'text-anchor': 'end',
    class: 'vt ' + (isAx ? 'ax-txt-glow' : ''),
    'font-size': isAx ? 24 : 21,
    fill: isAx ? '#ffffff' : (r.h === 'opencode' ? '#a2a2ac' : '#7d8590')
  }, label);
});

// Top telemetry readout: passed X/331 · T+XXm
const readout = txt(svg, { x: 20, y: 24, class: 'vt num', 'font-size': 21, fill: '#f4f4f7' }, '');

const litPaths = rows.map(r => el(svg, 'path', { d: '', fill: r.h === 'angelx' ? '#ffffff' : (r.h === 'opencode' ? '#a2a2ac' : '#7d8590'), class: r.h === 'angelx' ? 'ax-glow' : '' }));
const missPaths = rows.map(() => el(svg, 'path', { d: '', fill: 'none', stroke: '#a2a2ac', 'stroke-width': 1.4 }));
const countTexts = rows.map(r => txt(svg, {
  x: 888, y: r.y + 7, 'text-anchor': 'end',
  class: 'vt num ' + (r.h === 'angelx' ? 'ax-txt-glow' : ''),
  'font-size': r.h === 'angelx' ? 24 : 21,
  fill: r.h === 'angelx' ? '#ffffff' : '#a2a2ac'
}, ''));
const spanTexts = rows.map(r => txt(svg, { x: 0, y: r.y + 26, class: 'vt', 'font-size': 16, fill: '#888899' }, ''));

const fmtMin = sec => sec >= 3600 ? ((sec / 3600).toFixed(1) + 'h') : (Math.round(sec / 60) + 'm');
const total = rows.reduce((n, r) => n + r.c.attempts.length, 0);

window.renderFrame = function(p) {
  const now = p * T;
  let totalPassed = 0;

  rows.forEach((r, ri) => {
    let d = '', dm = '', ok = 0, last = 0;
    const isAx = r.h === 'angelx';
    const dotH = isAx ? 6.5 : 4.5;
    const dotW = isAx ? 5.5 : 3.5;
    
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

    if (last > 0) {
      const isFinished = now >= r.totalTime;
      const pxLast = xAt(last);
      
      if (r.h === 'angelx') {
        spanTexts[ri].setAttribute('x', pxLast + 8);
        spanTexts[ri].setAttribute('text-anchor', 'start');
        if (isFinished) {
          spanTexts[ri].setAttribute('fill', '#ffffff');
          spanTexts[ri].textContent = '35m ★ finished';
        } else {
          spanTexts[ri].setAttribute('fill', '#ffffff');
          spanTexts[ri].textContent = fmtMin(last);
        }
      } else if (r.h === 'opencode') {
        spanTexts[ri].setAttribute('x', pxLast + 8);
        spanTexts[ri].setAttribute('text-anchor', 'start');
        if (isFinished) {
          spanTexts[ri].setAttribute('fill', '#a2a2ac');
          spanTexts[ri].textContent = '59m';
        } else {
          spanTexts[ri].setAttribute('fill', '#888899');
          spanTexts[ri].textContent = fmtMin(last);
        }
      } else if (r.h === 'omp') {
        if (isFinished) {
          spanTexts[ri].setAttribute('x', pxLast - 8);
          spanTexts[ri].setAttribute('text-anchor', 'end');
          spanTexts[ri].setAttribute('fill', '#e5c07b');
          spanTexts[ri].textContent = '1.3h (cap)';
        } else {
          spanTexts[ri].setAttribute('x', pxLast + 8);
          spanTexts[ri].setAttribute('text-anchor', 'start');
          spanTexts[ri].setAttribute('fill', '#888899');
          spanTexts[ri].textContent = fmtMin(last);
        }
      }
    } else {
      spanTexts[ri].textContent = '';
    }
    
    totalPassed += ok;
  });

  readout.textContent = 'passed ' + totalPassed + ' / ' + total + '  \u00b7  T+' + fmtMin(now);
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
    '--window-size=1080,1080',
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
          width: 1080,
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

        console.log('Square viewport (1080x1080) set and fonts ready.');

        if (isTestFrame) {
          console.log('Capturing square test frames...');
          // Mid-race frame (p = 0.5)
          await callCdp('Runtime.evaluate', { expression: 'window.renderFrame(0.5);' });
          const snapMid = await callCdp('Page.captureScreenshot', {
            format: 'png',
            clip: { x: 0, y: 0, width: 1080, height: 1080, scale: 1 }
          });
          fs.writeFileSync('/tmp/scoreboard_square_mid.png', Buffer.from(snapMid.data, 'base64'));
          console.log('Saved /tmp/scoreboard_square_mid.png');

          // Finished frame (p = 1.0)
          await callCdp('Runtime.evaluate', { expression: 'window.renderFrame(1.0);' });
          const snapDone = await callCdp('Page.captureScreenshot', {
            format: 'png',
            clip: { x: 0, y: 0, width: 1080, height: 1080, scale: 1 }
          });
          fs.writeFileSync('/tmp/scoreboard_square_done.png', Buffer.from(snapDone.data, 'base64'));
          console.log('Saved /tmp/scoreboard_square_done.png');

          cleanup();
          process.exit(0);
          return;
        }

        console.log('Spawning ffmpeg for 1080x1080 square encode...');
        const ffmpeg = spawn('/usr/bin/ffmpeg', [
          '-y',
          '-f', 'image2pipe',
          '-vcodec', 'png',
          '-r', '30',
          '-i', '-',
          '-vf', 'scale=1080:1080,format=yuv420p',
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
        // Total 390 frames = 13 seconds
        // 0 to 14: intro holding at 0 (0.5s)
        // 15 to 255: animation running 0.0 to 1.0 (8.0s)
        // 256 to 389: freeze frame at 1.0 (4.5s)
        const TOTAL_FRAMES = 390;
        const RUN_START = 15;
        const RUN_END = 255;

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
              clip: { x: 0, y: 0, width: 1080, height: 1080, scale: 1 }
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

        console.log('All frames sent. Finalizing square video...');
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
