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

/* global makePresenter, loadRenderer, makeGpuPresenter, makeInput, makeSound, makeLoopback, makePanel */
(() => {
  const params = new URLSearchParams(location.search);
  let postWanted = 1;
  try {
    const kept = localStorage.getItem('angel.post');
    if (kept === '0' || kept === '1') postWanted = Number(kept);
  } catch { /* no storage */ }
  if (params.has('post')) postWanted = params.get('post') === '0' ? 0 : 1;
  // <key>, or <key>&dev for a link that offers the dev panel (F2).
  const [join, ...flags] = decodeURIComponent(location.hash.slice(1)).split('&');
  const devLink = flags.includes('dev');
  const offline = params.has('offline');
  const $ = id => document.getElementById(id);
  // The canvas keeps the first kind of context it is given (see `freshCanvas`).
  let canvas = $('screen');
  const form = $('join'), nameInput = $('name'), playButton = $('play');
  const statusLine = $('status'), toastLine = $('toast'), fullButton = $('full'), statsBox = $('stats');
  const sayForm = $('say'), sayInput = $('said'), sheetBox = $('sheet');
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
    // Probes may set this: (heap, ptr, w, h, frameIndex, staged, stagePtr,
    // stageLen) after each frame: the finished frame (BGRA words, or RGBA
    // bytes for a presenter with `rgba`), and the stage's bytes when it drew one.
    onframe: null,
    // Where the last frame drew the own character, in frame pixels, as
    // the game says (null when it does not say): for probes.
    own() {
      if (!x || !x.angel_own_x) return null;
      const at = [x.angel_own_x(), x.angel_own_y()];
      return Number.isNaN(at[0]) ? null : at;
    },
    // The game's own counts as its dev panel reads them (the Delve's
    // corrections, its own shots on the last frame), or null before play.
    game() {
      if (!x || !x.angel_stats) return null;
      try { return JSON.parse(text(x.angel_stats())).game; } catch { return null; }
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
        post: presenter && presenter.setPost ? postWanted : 0,
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
  // One short line at the picture's top edge; a repeat only keeps it up.
  const TOAST_MS = 2500;
  function toast(text) {
    if (toastLine.textContent !== text) toastLine.textContent = text;
    toastLine.title = text;
    toastLine.classList.remove('gone');
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => toastLine.classList.add('gone'), TOAST_MS);
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
  const instantiating = (WebAssembly.instantiateStreaming
    ? WebAssembly.instantiateStreaming(fetch(wasmUrl), imports).catch(slow)
    : slow()
  ).then(r => {
    x = r.instance.exports;
    heap = new Uint8Array(x.memory.buffer);
    return x;
  });

  // ---- the page's parts ----

  // How the game reaches the screen. The bundle may carry the renderer
  // module (render-webgpu.* and render-webgl.*, named in the page's
  // `renderers` manifest; gpu.js wraps it): `angel-gpu`, the core the native
  // window draws with, behind this canvas. It is used where the browser can
  // run it, WebGPU where `navigator.gpu` is there and its WebGL2 backend
  // where only WebGL2 is. Where the bundle has none, or loading or starting
  // it throws, present.js draws as it always has (WebGL2, then WebGL1, then
  // a 2D canvas). `?present=` forces a way, for testing:
  //   gpu     the WebGPU build         webgl    the WebGL2 build
  //   legacy  present.js, best first   webgl1 / 2d   present.js, that fallback
  const want = params.get('present');
  let renderers = {};
  try {
    renderers = JSON.parse($('renderers').textContent || '{}');
  } catch {
    renderers = {};
  }

  // A canvas keeps the first kind of context it was given: one the renderer
  // touched cannot go back to present.js's. A new canvas takes its place.
  function freshCanvas() {
    const fresh = canvas.cloneNode(false);
    canvas.replaceWith(fresh);
    canvas = fresh;
    return fresh;
  }

  const legacyPresenter = () => makePresenter(canvas, want === 'webgl1' || want === '2d' ? want : null);

  // The presenter to start with. Never rejects: every way of not getting the
  // renderer ends in present.js's.
  async function choosePresenter() {
    let build = null, backend = '';
    if (want !== 'legacy' && want !== 'webgl1' && want !== '2d') {
      if (want === 'webgl') {
        backend = 'webgl';
      } else if (want === 'gpu' || navigator.gpu) {
        backend = 'webgpu';
      } else if (typeof WebGL2RenderingContext === 'function') {
        backend = 'webgl';
      }
      build = backend ? renderers[backend] : null;
      if (backend && !build && want) console.warn('angelX: this bundle has no ' + backend + ' renderer: drawing with the page\'s own presenter');
    }
    if (build) {
      try {
        const p = await makeGpuPresenter(canvas, await loadRenderer(build), backend);
        console.info('angelX: drawing with the ' + p.kind.slice(4) + ' renderer' + (p.adapter ? ' (' + p.adapter + ')' : ''));
        gpuActive = true;
        return p;
      } catch (e) {
        console.warn('angelX: the renderer did not start (' + (e && e.message || e) + '): drawing with the page\'s own presenter');
        freshCanvas();
      }
    }
    return legacyPresenter();
  }

  let presenter = null, useStage = false, gpuActive = false;
  // The next stage frame carries every picture again (a presenter that lost its pictures).
  const resetStage = () => {
    if (x && x.angel_stage_reset) x.angel_stage_reset();
  };
  // Whether the game may send a stage's 3D props (`STG3` frames): only to a
  // presenter that draws them (the renderer module); present.js's stage.js
  // does not, and the game then keeps its shots as the flat sprites it knew.
  const declareProps = () => {
    if (x && x.angel_stage_props) x.angel_stage_props(presenter && presenter.props ? 1 : 0);
  };
  function adopt(p) {
    presenter = p;
    if (p.setPost) postWanted = p.setPost(postWanted);
    declareProps();
    useStage = !!p.drawStage && params.get('stage') !== '0';
    p.onreset = resetStage;
    if (gpuActive) {
      p.onlost = why => {
        // The module stopped (a lost device, a trap): once, and for the rest
        // of the session present.js takes over, with the pictures sent again.
        gpuActive = false;
        console.warn('angelX: the GPU renderer stopped (' + (why && why.message || why) + '): drawing with the page\'s own presenter from here on');
        p.dispose();
        freshCanvas();
        adopt(legacyPresenter());
        resetStage();
      };
    }
    A.presenter = p.kind;
  }
  const presenting = choosePresenter().then(adopt);
  // Ready to start: the module compiled, and a way to draw chosen.
  const loading = Promise.all([instantiating, presenting]).then(([ex]) => {
    declareProps();
    return ex;
  });
  let sound = null;
  let input = null;
  let panel = null;
  let running = false, crashed = false;
  function gameHasKeys() {
    return running && !crashed && form.hidden && sayForm.hidden && !(panel && panel.focused());
  }

  // A game's sheet (`controls.sheet`: the key that shows it), its own text
  // over the picture, such as the Delve's cards and wishes. The keys stay
  // the game's while it is up.
  let sheetTimer = 0;
  function showSheet(on) {
    clearInterval(sheetTimer);
    sheetBox.hidden = !on;
    if (!on || !x || !x.angel_sheet) return;
    const fill = () => { sheetBox.textContent = text(x.angel_sheet()) || 'Nothing to show yet.'; };
    fill();
    sheetTimer = setInterval(fill, 400);
  }
  window.addEventListener('keydown', e => {
    const s = A.controls && A.controls.sheet;
    if (!s || e.code !== s.key || e.repeat || e.ctrlKey || e.metaKey || !gameHasKeys()) return;
    e.preventDefault();
    showSheet(sheetBox.hidden);
  }, true);

  // A game's text line (`controls.text`: the key that opens it, the action
  // kind it goes as, and the JSON field it fills), such as the Delve's wish.
  // While it is open the keys are the line's; Enter sends, Esc lets go.
  function closeSay() {
    sayForm.hidden = true;
    sayInput.blur();
  }
  window.addEventListener('keydown', e => {
    const t = A.controls && A.controls.text;
    if (!t || e.code !== t.key || e.repeat || e.ctrlKey || e.metaKey || !gameHasKeys()) return;
    e.preventDefault();
    input.releaseAll();
    sayInput.placeholder = t.hint || '';
    sayInput.value = '';
    sayForm.hidden = false;
    sayInput.focus();
  }, true);
  sayInput.addEventListener('keydown', e => {
    if (e.code === 'Escape') {
      e.preventDefault();
      closeSay();
    }
  });
  sayInput.addEventListener('blur', () => { sayForm.hidden = true; });
  sayForm.addEventListener('submit', e => {
    e.preventDefault();
    const t = A.controls && A.controls.text;
    const said = sayInput.value.trim();
    closeSay();
    if (!t || !said || !x) return;
    withString(JSON.stringify({ [t.field || 'text']: said }), (p, l) => x.angel_action(t.kind >>> 0, p, l));
    flush();
  });

  // The page's own knobs, for the dev panel: whole-number scaling, a
  // frame-rate cap (0: the display's own rate) and the GPU renderer's post
  // pass (bloom, vignette, grain: crates/gpu/src/post.rs), on unless
  // `?post=0` or this device turned it off (remembered: a weak device).
  let fpsCap = 0, lastDrawn = 0;
  const pageKnobs = [
    { name: 'post', group: 'render', label: 'Post pass: 0 off 1 on', constant: 'crates/gpu/src/post.rs',
      min: 0, max: 1, step: 1, default: 1, value: postWanted, side: 'client' },
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
    if (name === 'post') {
      postWanted = v;
      if (presenter.setPost) v = presenter.setPost(v);
      try { localStorage.setItem('angel.post', String(postWanted)); } catch { /* no storage: this page only */ }
    }
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
  // The volume: the note mutes, the slider sets it, - and = step it. The
  // slider gives the keys back to the game once it is let go.
  const volBox = $('vol'), muteButton = $('mute'), volumeRange = $('volume');
  function showVolume() {
    volumeRange.value = String(Math.round(sound.volume * 100));
    muteButton.classList.toggle('off', sound.muted);
  }
  volBox.hidden = false;
  showVolume();
  volumeRange.addEventListener('input', () => { sound.setVolume(Number(volumeRange.value) / 100); showVolume(); });
  volumeRange.addEventListener('change', () => volumeRange.blur());
  volumeRange.addEventListener('pointerup', () => volumeRange.blur());
  muteButton.addEventListener('click', () => { sound.unlock(); sound.toggleMute(); showVolume(); muteButton.blur(); });
  window.addEventListener('keydown', e => {
    if ((e.code !== 'Minus' && e.code !== 'Equal') || e.ctrlKey || e.metaKey || !sayForm.hidden) return;
    if (document.activeElement && document.activeElement.tagName === 'INPUT') return;
    e.preventDefault();
    sound.setVolume(sound.volume + (e.code === 'Equal' ? 0.1 : -0.1));
    showVolume();
    toast(sound.muted || sound.volume === 0 ? 'Sound off' : `Volume ${Math.round(sound.volume * 100)}%`);
  }, true);

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
      // A presenter that takes RGBA bytes and draws the stage takes the
      // frame's words first and turns them into bytes only when the stage
      // did not draw (a probe watching the finished frame gets bytes, as
      // ever): turning every frame was a tenth of the page's time.
      const late = presenter.rgba && useStage && !!x.angel_stage && !!x.angel_frame_rgba_last && !A.onframe;
      let ptr = (presenter.rgba && !late ? x.angel_frame_rgba(t) : x.angel_frame(t)) >>> 0;
      const t1 = performance.now();
      const w = x.angel_frame_w(), h = x.angel_frame_h();
      // The frame from its parts where the page and the game both can
      // (`?stage=0` keeps finished frames).
      let staged = false, sp = 0, sl = 0;
      if (useStage && x.angel_stage) {
        sp = x.angel_stage(t) >>> 0;
        sl = x.angel_stage_len() >>> 0;
        staged = sl > 0 && presenter.drawStage(mem(), sp, sl, t);
      }
      if (!staged && late) ptr = x.angel_frame_rgba_last() >>> 0;
      if (!staged && w && h) presenter.draw(mem(), ptr, w, h);
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
      if (A.onframe && w && h) A.onframe(mem(), ptr, w, h, A.frames, staged, sp, sl);
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
