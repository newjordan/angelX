'use strict';
// The shell: asks for a name while the game's wasm streams in and
// compiles, joins over a WebSocket, and runs the frame loop:
//
//   gamepad poll -> the module's frame (commands, prediction, drawing)
//   -> present -> send what the module queued -> act on its lines
//
// The link is <door>/#<join key>: the key never leaves the page except in
// the hello. The seat token from the welcome is kept per join key, so a
// reload, a dropped wifi or a locked screen takes the same seat back.
// See docs/ENGINE.md, "The browser runtime".

/* global makePresenter, makeInput, makeSound, makeLoopback, makePanel */
(() => {
  const params = new URLSearchParams(location.search);
  // <key>, or <key>&dev for a link that offers the dev panel (F2).
  const [join, ...flags] = decodeURIComponent(location.hash.slice(1)).split('&');
  const devLink = flags.includes('dev');
  const offline = params.has('offline');
  const $ = id => document.getElementById(id);
  const canvas = $('screen'), form = $('join'), nameInput = $('name'), playButton = $('play');
  const statusLine = $('status'), toastLine = $('toast'), fullButton = $('full'), statsBox = $('stats');
  const decoder = new TextDecoder(), encoder = new TextEncoder();

  // Fixed numbers (docs/ENGINE.md).
  const BACKOFF_MS = [500, 1000, 2000, 4000, 5000];
  // angel-serve's close code when another connection took this seat
  // (`ws::code::REPLACED`).
  const CLOSE_REPLACED = 4000;
  // Snapshots come many times a second and pongs every second: this long
  // with nothing heard, the link is dead even if the socket has not noticed
  // (a phone that changed networks, a laptop that slept).
  const SILENCE_MS = 3000;
  const SAMPLES = 240;
  const NAME_CHARS = 24;
  // angel_net::proto::MAX_NAME: the server refuses a longer hello.
  const NAME_BYTES = 48;
  // Why the server turned the page away, by angel_net::proto::Reject code.
  const REJECTED = {
    1: 'this page is out of date: reload it',
    2: 'this link is not for a game that is running: ask the host for the link again',
    3: 'every place in this game is taken',
    4: 'the host has closed this game to newcomers',
    5: 'the host removed you from this game',
    6: 'the host has ended this game',
    7: 'this link is for a different game',
    8: 'the server closed the link',
  };

  // What probes read (and `?stats` shows). Timings are ms; the samples are
  // kept in fixed rings and only sorted when read, so the loop allocates
  // nothing for them.
  const wasmRing = new Float64Array(SAMPLES), presentRing = new Float64Array(SAMPLES);
  const scratch = new Float64Array(SAMPLES);
  let ringAt = 0, ringFull = false;
  function percentile(ring, p) {
    const n = ringFull ? SAMPLES : ringAt;
    if (!n) return 0;
    const s = scratch.subarray(0, n);
    s.set(ring.subarray(0, n));
    s.sort();
    return s[Math.min(n - 1, Math.floor(p * n))];
  }
  const A = window.__angel = {
    status: 'loading', seat: -1, presenter: '', controls: null,
    loadedMs: 0, frames: 0, dropped: 0, fps: 0, rtt: -1,
    bytesIn: 0, bytesOut: 0, reconnects: 0, sounds: 0,
    wasmP50: 0, wasmP95: 0, presentP50: 0, presentP95: 0,
    // The frame count when the last key or button went down: a probe
    // compares it with the frame its character first moved on.
    inputFrame: -1, inputAt: 0,
    // Probes may set this: (heap, ptr, w, h, frameIndex) after each frame.
    onframe: null,
    // Where the last frame drew the own character, in frame pixels, as
    // the game says (null when it does not say): for probes.
    own() {
      if (!x || !x.angel_own_x) return null;
      const at = [x.angel_own_x(), x.angel_own_y()];
      return Number.isNaN(at[0]) ? null : at;
    },
    stats() {
      A.wasmP50 = percentile(wasmRing, 0.5);
      A.wasmP95 = percentile(wasmRing, 0.95);
      A.presentP50 = percentile(presentRing, 0.5);
      A.presentP95 = percentile(presentRing, 0.95);
      return {
        status: A.status, seat: A.seat, presenter: A.presenter, loadedMs: A.loadedMs,
        frames: A.frames, dropped: A.dropped, fps: A.fps, rtt: A.rtt,
        bytesIn: A.bytesIn, bytesOut: A.bytesOut, reconnects: A.reconnects, sounds: A.sounds,
        memory: x ? x.memory.buffer.byteLength : 0,
        heardAt: lastHeard,
        wasmP50: A.wasmP50, wasmP95: A.wasmP95, presentP50: A.presentP50, presentP95: A.presentP95,
      };
    },
    reset() {
      A.frames = A.dropped = 0;
      ringAt = 0;
      ringFull = false;
    },
  };

  function say(text) {
    statusLine.textContent = text;
  }
  let toastTimer = 0;
  function toast(text) {
    toastLine.textContent = text;
    toastLine.classList.remove('gone');
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => toastLine.classList.add('gone'), 4000);
  }

  // ---- the module ----

  let x = null; // the exports
  let heap = null; // a Uint8Array over its memory, remade when memory grows
  let lastPanic = '';
  function mem() {
    if (heap.buffer !== x.memory.buffer) heap = new Uint8Array(x.memory.buffer);
    return heap;
  }
  function text(len) {
    if (!len) return '';
    const p = x.angel_text_ptr() >>> 0;
    return decoder.decode(mem().subarray(p, p + len));
  }
  // A name the server takes: at most NAME_CHARS characters and NAME_BYTES
  // bytes of UTF-8, cut between characters.
  function clip(name) {
    const chars = Array.from(name).slice(0, NAME_CHARS);
    while (encoder.encode(chars.join('')).length > NAME_BYTES) chars.pop();
    return chars.join('');
  }

  // Strings into the module, for the hello only.
  function withString(s, f) {
    const bytes = encoder.encode(s);
    const p = x.angel_alloc(bytes.length);
    mem().set(bytes, p >>> 0);
    try {
      return f(p, bytes.length);
    } finally {
      x.angel_free(p, bytes.length);
    }
  }

  const imports = {
    angel: {
      log(level, p, len) {
        const msg = decoder.decode(mem().subarray(p >>> 0, (p >>> 0) + len));
        if (level === 0) {
          lastPanic = msg;
          console.error('game:', msg);
        } else {
          console.info('game:', msg);
        }
      },
    },
  };
  const wasmUrl = document.body.dataset.wasm;
  const slow = () => fetch(wasmUrl).then(r => r.arrayBuffer()).then(b => WebAssembly.instantiate(b, imports));
  // Compiles while it downloads; a server with the wrong MIME type gets the
  // plain way.
  const loading = (WebAssembly.instantiateStreaming
    ? WebAssembly.instantiateStreaming(fetch(wasmUrl), imports).catch(slow)
    : slow()
  ).then(r => {
    x = r.instance.exports;
    heap = new Uint8Array(x.memory.buffer);
    return x;
  });

  // ---- the page's parts ----

  const presenter = makePresenter(canvas, params.get('present'));
  A.presenter = presenter.kind;
  let sound = null;
  let input = null;
  let panel = null;
  let running = false, crashed = false;
  function gameHasKeys() {
    return running && !crashed && form.hidden && !(panel && panel.focused());
  }

  // The page's own knobs, for the dev panel: whole-number scaling and a
  // frame-rate cap (0: the display's own rate).
  let fpsCap = 0, lastDrawn = 0;
  const pageKnobs = [
    { name: 'scale', group: 'render', label: 'Scale: 0 auto 1 whole 2 fit', constant: 'MIN_FILL (crates/web/shell/present.js)',
      min: 0, max: 2, step: 1, default: 0, value: 0, side: 'client' },
    { name: 'fps_cap', group: 'render', label: 'Frame cap (0 none)', constant: 'the display\'s refresh (crates/web/shell/boot.js)',
      min: 0, max: 240, step: 5, default: 0, value: 0, side: 'client' },
  ];
  function setPage(name, value) {
    const k = pageKnobs.find(p => p.name === name);
    if (!k) return NaN;
    let v = Math.min(k.max, Math.max(k.min, Math.round(Number(value) / k.step) * k.step));
    if (Number.isNaN(v)) return NaN;
    if (name === 'scale') v = presenter.setScale(v);
    if (name === 'fps_cap') fpsCap = v;
    k.value = v;
    return v;
  }
  function touched(kind) {
    if (sound) sound.unlock();
    if (kind === 'key' || kind === 'pad') {
      A.inputFrame = A.frames;
      A.inputAt = performance.now();
    }
  }
  // Audio may only start inside a gesture.
  window.addEventListener('pointerdown', () => touched('click'), true);
  window.addEventListener('keydown', () => touched('click'), true);

  let manifest = {};
  try {
    manifest = JSON.parse($('sounds').textContent || '{}');
  } catch {
    manifest = {};
  }
  sound = makeSound(manifest);

  // ---- the link ----

  const seatKey = 'angel.seat.' + join;
  let name = '';
  let sock = null;
  let resume = '';
  let attempt = 0, retryTimer = 0;
  let lastHeard = 0;
  let rejected = false;
  let lastStatus = -1;
  let inPtr = 0, inCap = 0; // the module buffer incoming messages are copied into

  function connect() {
    clearTimeout(retryTimer);
    retryTimer = 0;
    if (crashed || rejected) return;
    A.status = attempt ? 'reconnecting' : 'connecting';
    say(attempt ? 'reconnecting' : 'connecting');
    resume = localStorage.getItem(seatKey) || '';
    const s = offline
      ? makeLoopback(x, Number(params.get('offline')) || 0)
      : new WebSocket((location.protocol === 'https:' ? 'wss://' : 'ws://') + location.host + '/ws');
    s.binaryType = 'arraybuffer';
    sock = s;
    s.onopen = () => {
      if (s !== sock) return;
      lastHeard = performance.now();
      lastStatus = -1;
      const caps = presenter.kind === 'canvas2d' ? 0 : 2; // angel_net::proto::caps::WEBGL
      withString(join, (jp, jl) => withString(resume, (rp, rl) => withString(name, (np, nl) =>
        x.angel_connected(jp, jl, rp, rl, np, nl, caps))));
      flush();
    };
    s.onmessage = ev => {
      if (s !== sock || crashed) return;
      const data = new Uint8Array(ev.data);
      lastHeard = performance.now();
      A.bytesIn += data.length;
      if (data.length > inCap) {
        if (inCap) x.angel_free(inPtr, inCap);
        inCap = Math.max(4096, 1 << Math.ceil(Math.log2(data.length)));
        inPtr = x.angel_alloc(inCap) >>> 0;
      }
      mem().set(data, inPtr);
      try {
        if (!x.angel_recv(inPtr, data.length, lastHeard)) console.warn('a message did not read');
      } catch (e) {
        crash(e);
        return;
      }
      watch();
    };
    s.onclose = ev => {
      if (s !== sock) return;
      sock = null;
      if (ev.code === CLOSE_REPLACED) {
        // Another page took this seat (a second tab, or this link opened
        // elsewhere). Retrying would take it back, and the two would pass
        // the seat between them forever: stop, and say where it went.
        rejected = true;
        A.status = 'replaced';
        say('this seat is open in another tab');
        return;
      }
      retry();
    };
  }

  // Let go of a socket, quietly: its handlers ignore anything it still says.
  function drop() {
    const s = sock;
    sock = null;
    if (s && s.readyState < 2) s.close();
  }

  function retry() {
    if (crashed || rejected || retryTimer) return;
    const wait = BACKOFF_MS[Math.min(attempt, BACKOFF_MS.length - 1)];
    attempt++;
    A.status = 'reconnecting';
    A.reconnects++;
    say('reconnecting in ' + (wait / 1000) + ' s');
    retryTimer = setTimeout(connect, wait);
  }

  // A dead link the socket has not noticed: nothing heard for too long.
  setInterval(() => {
    if (sock && sock.readyState === 1 && !document.hidden && performance.now() - lastHeard > SILENCE_MS) {
      drop();
      retry();
    }
  }, 500);
  // The browser knows the network is back: don't wait out the backoff.
  window.addEventListener('online', () => {
    if (!sock && retryTimer) connect();
  });

  // The module's status moved: seated, or turned away.
  function watch() {
    const s = x.angel_status();
    if (s === lastStatus) return;
    lastStatus = s;
    if (s === 1) {
      attempt = 0;
      A.seat = x.angel_seat();
      A.status = A.seat >= 0 ? 'playing' : 'watching';
      const token = text(x.angel_seat_token());
      if (token) localStorage.setItem(seatKey, token);
      try {
        // The game's own keyboard, pad and stick maps, where it gives
        // them; the button list is kept for the touch layout (a later step).
        A.controls = JSON.parse(text(x.angel_controls()) || '{}');
      } catch {
        A.controls = null;
      }
      input.setControls(A.controls);
      if (panel) panel.seated();
      say(A.seat >= 0 ? '' : 'watching');
      wakeLock();
    } else if (s === 2) {
      const code = x.angel_reject();
      if (code === 2 && resume) {
        // The seat token is stale (a new session on the same link): join
        // fresh instead.
        localStorage.removeItem(seatKey);
        drop();
        connect();
        return;
      }
      rejected = true;
      A.status = 'rejected';
      say(REJECTED[code] || text(x.angel_reject_text()) || 'turned away');
      drop();
    }
  }

  // Send what the module queued; with no open socket it is dropped (the
  // next hello starts the commands over).
  function flush() {
    const n = x.angel_out();
    if (!n || !sock || sock.readyState !== 1) return;
    const h = mem(), p = x.angel_out_ptr() >>> 0;
    for (let i = 0; i < n;) {
      const len = h[p + i] | h[p + i + 1] << 8 | h[p + i + 2] << 16 | h[p + i + 3] << 24;
      i += 4;
      sock.send(h.subarray(p + i, p + i + len));
      A.bytesOut += len;
      i += len;
    }
  }

  function lines() {
    const len = x.angel_lines();
    if (!len) return;
    for (const line of text(len).split('\n')) {
      const space = line.indexOf(' ');
      const verb = space < 0 ? line : line.slice(0, space);
      const rest = space < 0 ? '' : line.slice(space + 1);
      if (verb === 'sfx') {
        sound.sfx(rest);
        A.sounds++;
      } else if (verb === 'music') {
        sound.music(rest);
      } else if (verb === 'say' || verb === 'notice') {
        toast(rest);
      }
    }
  }

  function crash(e) {
    if (crashed) return;
    crashed = true;
    A.status = 'crashed';
    console.error(e);
    say('the game stopped' + (lastPanic ? ': ' + lastPanic : '') + '. Reload to rejoin.');
    drop();
  }

  // ---- the frame loop ----

  let raf = 0;
  let lastT = 0, second = 0, secondFrames = 0;
  // Missed display frames: a gap well past the display's own interval. The
  // interval (60, 120, 144 Hz alike) is learned from the gaps themselves:
  // once a second, the 20th percentile of the last SAMPLES gaps, leaving out
  // gaps under 4 ms (two callbacks in one vsync, which some starts give).
  const gaps = new Float64Array(SAMPLES);
  let gapAt = 0, gapCount = 0, interval = 0;
  function learnInterval() {
    const s = scratch.subarray(0, gapCount);
    s.set(gaps.subarray(0, gapCount));
    s.sort();
    const from = s.findIndex(g => g >= 4);
    if (from >= 0) interval = s[from + Math.floor((gapCount - from) * 0.2)];
  }
  function frame(t) {
    raf = requestAnimationFrame(frame);
    if (crashed) return;
    // The dev panel's frame-rate cap: a frame due sooner is skipped whole
    // (the next one sends whatever commands came due meanwhile), and the
    // display's interval is learned from the frames drawn.
    if (fpsCap > 0 && t - lastDrawn < 1000 / fpsCap - 1) return;
    lastDrawn = t;
    if (lastT) {
      const dt = t - lastT;
      gaps[gapAt] = dt;
      gapAt = (gapAt + 1) % SAMPLES;
      gapCount = Math.min(gapCount + 1, SAMPLES);
      if (gapAt % 60 === 0) learnInterval();
      if (interval && dt > interval * 1.5) A.dropped += Math.round(dt / interval) - 1;
    }
    lastT = t;
    A.frames++;
    secondFrames++;
    if (t - second >= 1000) {
      A.fps = secondFrames * 1000 / (t - second);
      second = t;
      secondFrames = 0;
    }
    try {
      if (sock && sock.pump) sock.pump(t);
      input.poll();
      const t0 = performance.now();
      const ptr = (presenter.rgba ? x.angel_frame_rgba(t) : x.angel_frame(t)) >>> 0;
      const t1 = performance.now();
      const w = x.angel_frame_w(), h = x.angel_frame_h();
      if (w && h) presenter.draw(mem(), ptr, w, h);
      const t2 = performance.now();
      wasmRing[ringAt] = t1 - t0;
      presentRing[ringAt] = t2 - t1;
      if (++ringAt === SAMPLES) {
        ringAt = 0;
        ringFull = true;
      }
      flush();
      lines();
      watch();
      if (A.onframe && w && h) A.onframe(mem(), ptr, w, h, A.frames);
    } catch (e) {
      crash(e);
    }
  }

  // ---- life: hidden tabs, wake lock, full screen, stats ----

  // Hidden: let go of every key, stop drawing and stop the commands. A
  // slow timer keeps the pings going, so the link stays; the server counts
  // a seat sending no commands as away and keeps its knight out of harm's
  // way until the page is looked at again. Coming back draws from the
  // newest snapshot, which kept arriving meanwhile.
  let hiddenTimer = 0;
  document.addEventListener('visibilitychange', () => {
    if (!running || crashed) return;
    if (document.hidden) {
      // Said now: the loop that would send it is about to stop.
      input.releaseAll();
      flush();
      cancelAnimationFrame(raf);
      raf = 0;
      lastT = 0;
      hiddenTimer = setInterval(() => {
        try {
          x.angel_idle(performance.now());
          flush();
        } catch (e) {
          crash(e);
        }
      }, 1000);
      sound.suspend();
    } else {
      clearInterval(hiddenTimer);
      hiddenTimer = 0;
      lastHeard = Math.max(lastHeard, performance.now() - SILENCE_MS / 2);
      if (!raf) raf = requestAnimationFrame(frame);
      sound.resume();
      wakeLock();
    }
  });

  let lock = null;
  function wakeLock() {
    if (lock || document.hidden || !navigator.wakeLock) return;
    navigator.wakeLock.request('screen').then(l => {
      lock = l;
      l.addEventListener('release', () => { lock = null; });
    }).catch(() => {});
  }

  const root = document.documentElement;
  if (document.fullscreenEnabled || document.webkitFullscreenEnabled) {
    fullButton.hidden = false;
    fullButton.addEventListener('click', () => {
      if (document.fullscreenElement || document.webkitFullscreenElement) {
        (document.exitFullscreen || document.webkitExitFullscreen).call(document);
      } else {
        (root.requestFullscreen || root.webkitRequestFullscreen).call(root);
      }
      // Space and Enter belong to the game, not to this button.
      fullButton.blur();
    });
  }

  if (params.has('stats')) {
    statsBox.hidden = false;
    let inWas = 0, outWas = 0;
    setInterval(() => {
      const s = A.stats();
      const kIn = (s.bytesIn - inWas) / 1024, kOut = (s.bytesOut - outWas) / 1024;
      inWas = s.bytesIn;
      outWas = s.bytesOut;
      statsBox.textContent =
        `${s.fps.toFixed(0)} fps, ${s.dropped} dropped\n` +
        `wasm ${s.wasmP50.toFixed(2)} ms, p95 ${s.wasmP95.toFixed(2)}\n` +
        `present ${s.presentP50.toFixed(2)} ms (${s.presenter})\n` +
        `ping ${s.rtt < 0 ? '-' : s.rtt.toFixed(0) + ' ms'}\n` +
        `in ${kIn.toFixed(1)} KB/s, out ${kOut.toFixed(1)} KB/s`;
    }, 1000);
  }
  setInterval(() => {
    if (x && running && !crashed) A.rtt = x.angel_rtt();
  }, 1000);

  // ---- start ----

  function start() {
    if (running) return;
    running = true;
    input = makeInput(x, touched, gameHasKeys);
    if (typeof makePanel === 'function' && x.angel_knobs) {
      panel = makePanel({
        x, text, withString, flush, toast, game: document.title.toLowerCase(), dev: devLink,
        stats: () => A.stats(), page: pageKnobs, setPage, releaseAll: () => input.releaseAll(),
      });
    }
    presenter.clear();
    connect();
    raf = requestAnimationFrame(frame);
  }

  if (!join && !offline) {
    say('this link has no game key: ask the host for the link again');
    form.hidden = true;
    return;
  }

  loading.then(() => {
    if (offline && !x.loopback_open) {
      say('?offline needs a test build (crates/web/testgame); this game has no loopback');
      form.hidden = true;
      return;
    }
    A.loadedMs = performance.now();
    if (A.status === 'loading') A.status = 'ready';
    playButton.disabled = false;
    playButton.textContent = 'Play';
  }).catch(e => {
    console.error(e);
    say('the game did not load: ' + e);
  });

  nameInput.value = localStorage.getItem('angel.name') || '';
  form.addEventListener('submit', e => {
    e.preventDefault();
    if (playButton.disabled) return;
    name = clip(nameInput.value.trim()) || 'knight';
    localStorage.setItem('angel.name', name);
    nameInput.blur();
    form.hidden = true;
    sound.unlock();
    start();
  });

  // Back on a link this page has a seat on (a reload, a crash of the tab):
  // no prompt, take the seat back as soon as the module is ready.
  if (nameInput.value && localStorage.getItem(seatKey)) {
    name = nameInput.value;
    form.hidden = true;
    say('loading');
    loading.then(start, () => {});
  } else {
    nameInput.focus();
  }
})();
