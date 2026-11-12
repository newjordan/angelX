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

/* global makePresenter, loadRenderer, makeGpuPresenter, makeInput, makeSound, makeLoopback, makePanel, el, keyHint, drawSheet */
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
  const sayForm = $('say'), sayInput = $('said'), sheetBox = $('sheet'), choiceBox = $('choices');
  const sayTitle = $('say-title'), sayKeys = $('say-keys'), sayPicks = $('say-picks'), sheetPage = $('sheet-page');
  const decoder = new TextDecoder(), encoder = new TextEncoder();

  // The page's own hand (knight.ttf, shell/art/font.py): loaded by name, as
  // the server's policy lets a font come only from this page's own files.
  const fontFile = document.body.dataset.font;
  if (fontFile && window.FontFace) {
    new FontFace('Knight Hand', `url(${fontFile})`).load()
      .then(f => { document.fonts.add(f); })
      .catch(() => { /* the fallback monospace stands in */ });
  }

  // The picture's place on the page, for everything laid over it: its
  // box (--pic-l, --pic-t, --pic-w, --pic-h), the room's view inside it
  // (--view-t, --view-b, from the game's `controls.view`: the native rows
  // its bands take above and below the view), and --px, the page pixel:
  // as many device pixels as one of the picture's, so the frames and the
  // hand stand at the picture's own scale. Fitted as present.js and the
  // renderer fit it (place.rs): the largest whole number while it fills
  // four fifths of the fitted size, else fitted.
  let placed = '';
  // The same, for the page's own hit tests: the picture's top-left in CSS
  // pixels and one of its pixels' size (`slotAt`).
  let pic = null;
  function placePicture(w, h) {
    const r = canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    const view = A.controls && A.controls.view;
    const key = `${w}x${h} ${r.left},${r.top},${r.width}x${r.height} ${dpr} ${view ? view.top + ',' + view.bottom : ''}`;
    if (key === placed || !w || !h || !r.width || !r.height) return;
    placed = key;
    const sw = Math.round(r.width * dpr), sh = Math.round(r.height * dpr);
    const k = Math.min(Math.floor(sw / w), Math.floor(sh / h));
    const s = Math.min(sw / w, sh / h);
    const whole = k >= 1 && k >= 0.8 * s;
    const dw = whole ? w * k : Math.max(1, Math.floor(w * s));
    const dh = whole ? h * k : Math.max(1, Math.floor(h * s));
    // A page pixel: the picture's, but never under two CSS pixels, where
    // the hand would be too small to read.
    let pxDev = whole ? k : Math.max(1, Math.floor(s));
    if (pxDev / dpr < 2) pxDev = Math.ceil(2 * dpr);
    const css = v => `${v}px`;
    const left = r.left + ((sw - dw) >> 1) / dpr, top = r.top + ((sh - dh) >> 1) / dpr;
    const ph = dh / dpr, unit = ph / h;
    pic = { left, top, unit };
    const st = document.documentElement.style;
    st.setProperty('--px', css(pxDev / dpr));
    st.setProperty('--pic-l', css(left));
    st.setProperty('--pic-t', css(top));
    st.setProperty('--pic-w', css(dw / dpr));
    st.setProperty('--pic-h', css(ph));
    st.setProperty('--view-t', css(top + (view ? view.top : 0) * unit));
    st.setProperty('--view-b', css(top + ph - (view ? view.bottom : 0) * unit));
  }
  window.addEventListener('resize', () => { placed = ''; });

  // Fixed numbers (docs/ENGINE.md).
  const BACKOFF_MS = [500, 1000, 2000, 4000, 5000];
  // angel-serve's close code when another connection took this seat
  // (`ws::code::REPLACED`).
  const CLOSE_REPLACED = 4000;
  // Snapshots come many times a second and pongs every second: this long
  // with nothing heard, the link is dead even if the socket has not noticed
  // (a phone that changed networks, a laptop that slept). Three seconds
  // dropped live players on a busy frame or a slow link mid-game; the
  // server gives a client ten (`limits::SILENT_CLOSE`), and so does this.
  const SILENCE_MS = 10000;
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

  // The game's Bend rules modules (`rules.js`, crates/web/rules_js.rs), run
  // here as a native host runs the same files in V8: the client's rules ask
  // one by id (`angel_delve::scripted`), and its answer waits for
  // `rules_take`.
  const rulesLoaded = Object.create(null);
  let rulesAnswer = null;
  function rulesModule(id) {
    if (rulesLoaded[id]) return rulesLoaded[id];
    const mod = globalThis.angelRules && globalThis.angelRules[id];
    if (!mod) throw Error('no rules module named ' + id);
    const cache = Object.create(null);
    const asset = name => { throw Error('asset not declared: ' + name); };
    const resource = (module, name) => { throw Error('resource is not imported: ' + module + '/' + name); };
    const load = name => {
      if (cache[name]) return cache[name].exports;
      if (!Object.hasOwn(mod.files, name)) throw Error('missing dependency: ' + name);
      const module = {exports: {}};
      cache[name] = module;
      const require = specifier => {
        if (typeof specifier !== 'string' || !specifier.startsWith('.')) throw Error('use local relative dependencies');
        const parts = name.split('/').slice(0, -1);
        for (const part of specifier.split('/')) {
          if (part === '' || part === '.') continue;
          if (part === '..') { if (!parts.length) throw Error('dependency leaves module'); parts.pop(); }
          else parts.push(part);
        }
        const path = parts.join('/');
        const resolved = [path, path + '.js', path + '/index.js'].find(p => Object.hasOwn(mod.files, p));
        if (!resolved) throw Error('missing dependency: ' + specifier);
        return load(resolved);
      };
      mod.files[name](module, module.exports, require, asset, resource);
      return module.exports;
    };
    const feature = load(mod.entry);
    if (feature.apiVersion !== 1 || typeof feature.render !== 'function') throw Error('expected apiVersion 1 and render(input)');
    return (rulesLoaded[id] = feature);
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
      rules(idP, idLen, inP, inLen) {
        const read = (p, n) => decoder.decode(mem().subarray(p >>> 0, (p >>> 0) + n));
        let answer;
        try {
          answer = {value: rulesModule(read(idP, idLen)).render(JSON.parse(read(inP, inLen)))};
        } catch (e) {
          answer = {error: String((e && e.message) || e)};
        }
        rulesAnswer = encoder.encode(JSON.stringify(answer));
        return rulesAnswer.length;
      },
      rules_take(p) {
        mem().set(rulesAnswer, p >>> 0);
        rulesAnswer = null;
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
  let sheetTimer = 0, sheetSaid = null;
  function showSheet(on) {
    clearInterval(sheetTimer);
    sheetBox.hidden = !on;
    sheetSaid = null;
    if (!on || !x || !x.angel_sheet) return;
    const fill = () => {
      const said = text(x.angel_sheet());
      if (said === sheetSaid) return;
      sheetSaid = said;
      drawSheet(sheetPage, said);
    };
    fill();
    sheetTimer = setInterval(fill, 400);
  }
  window.addEventListener('keydown', e => {
    const s = A.controls && A.controls.sheet;
    if (!s || e.code !== s.key || e.repeat || e.ctrlKey || e.metaKey || !gameHasKeys()) return;
    e.preventDefault();
    showSheet(sheetBox.hidden);
  }, true);

  // A game's choices to tap (`angel_choices`: one label a line, such as the
  // Delve's wish scroll): a button each, and one to let them go; a tap is
  // `angel_choose`, which the game sends with its next command.
  let choicesSaid = '';
  setInterval(() => {
    const said = running && !crashed && x && x.angel_choices ? text(x.angel_choices()) : '';
    if (said === choicesSaid) return;
    choicesSaid = said;
    choiceBox.replaceChildren();
    choiceBox.hidden = !said;
    if (!said) return;
    // A plate to tap for each, under the game's own scroll (which says
    // them in full): the key in its cap and the choice's head, what it
    // gives in its tip. `controls.choices.back`: the way out's words.
    const c = (A.controls && A.controls.choices) || {};
    const lines = said.split('\n');
    lines.concat([null]).forEach((line, i) => {
      const b = el('button', i === lines.length ? 'plate back' : 'plate');
      b.type = 'button';
      if (line === null) {
        b.append(el('kbd', '', 'Esc'));
        b.dataset.tip = c.back || 'Roll it up';
      } else {
        // "1 Head: what it gives", or any line as it is.
        const m = /^(\d+) ([^:]+): (.*)$/.exec(line);
        if (m) {
          b.append(el('kbd', '', m[1]), el('span', '', m[2]));
          b.dataset.tip = m[3];
        } else b.append(el('span', '', line));
      }
      b.addEventListener('pointerdown', e => e.preventDefault());
      b.addEventListener('click', () => { if (x && x.angel_choose) x.angel_choose(i === lines.length ? 99 : i); });
      choiceBox.append(b);
    });
  }, 250);

  // A game's text line (`controls.text`: the key that opens it, the action
  // kind it goes as, and the JSON field it fills), such as the Delve's wish.
  // While it is open the keys are the line's; Enter sends, Esc lets go.
  function closeSay() {
    sayForm.hidden = true;
    clearInterval(picksTimer);
    sayInput.blur();
  }
  // What the line says, sent as the action its words go as.
  function sendSay(said) {
    const t = A.controls && A.controls.text;
    closeSay();
    if (!t || !said || !x) return;
    withString(JSON.stringify({ [t.field || 'text']: said }), (p, l) => x.angel_action(t.kind >>> 0, p, l));
    flush();
  }
  // The game's picks over the line (`angel_picks`: a heading, then one a
  // line, `key<TAB>label<TAB>tag<TAB>tip<TAB>words`, and a line with no tab
  // a note), such as the Delve's "Upgrade what you hold": a tap, or its key
  // on an empty line, sends its words as Enter sends what is typed. One with
  // no words is shown faint and not taken. Read again while the line is up.
  let picksSaid = null, picksTimer = 0, pickWords = {};
  function fillPicks() {
    if (sayForm.hidden) {
      clearInterval(picksTimer);
      return;
    }
    const said = running && !crashed && x && x.angel_picks ? text(x.angel_picks()) : '';
    if (said === picksSaid) return;
    picksSaid = said;
    pickWords = {};
    sayPicks.replaceChildren();
    sayPicks.hidden = !said;
    if (!said) return;
    const [head, ...lines] = said.split('\n');
    sayPicks.append(el('h3', '', head));
    for (const line of lines) {
      const f = line.split('\t');
      if (f.length < 5) {
        sayPicks.append(el('p', 'note', line));
        continue;
      }
      const [key, label, tag, tip, words] = f;
      const b = el('button', words ? 'pick plate' : 'pick plate off');
      b.type = 'button';
      b.disabled = !words;
      b.append(el('kbd', '', key), el('span', 'label', label), el('span', 'tag', tag), el('span', 'tip', tip));
      if (words) {
        if (key) pickWords[key] = words;
        // The line keeps the keys: a press here must not blur it shut.
        b.addEventListener('pointerdown', e => { if (e.pointerType !== 'touch') e.preventDefault(); });
        b.addEventListener('click', () => sendSay(words));
      }
      sayPicks.append(b);
    }
  }
  window.addEventListener('keydown', e => {
    const t = A.controls && A.controls.text;
    if (!t || e.code !== t.key || e.repeat || e.ctrlKey || e.metaKey || !gameHasKeys()) return;
    e.preventDefault();
    openSay();
  }, true);
  // The line opened by its key, or by the game itself (an `ask` line: the
  // Delve's knight rubbed the wish altar's lamp).
  function openSay() {
    // A game's text purpose can change without reseating (the tavern mirror).
    try { A.controls = JSON.parse(text(x.angel_controls()) || '{}'); } catch { /* keep the last map */ }
    const t = A.controls && A.controls.text;
    if (!t || !sayForm.hidden) return;
    input.releaseAll();
    // `controls.text`: the line's title, its placeholder (else its hint),
    // and what Enter does (else the hint stands under the line alone).
    sayInput.placeholder = t.placeholder || t.hint || '';
    sayTitle.hidden = !t.title;
    sayTitle.textContent = t.title || '';
    sayKeys.replaceChildren();
    if (t.send) sayKeys.append(keyHint('Enter', t.send), keyHint('Esc', t.cancel || 'let it go'));
    else if (t.placeholder && t.hint) sayKeys.append(el('span', '', t.hint));
    sayInput.value = '';
    sayForm.hidden = false;
    picksSaid = null;
    fillPicks();
    clearInterval(picksTimer);
    picksTimer = setInterval(fillPicks, 400);
    sayInput.focus();
  }
  sayForm.addEventListener('keydown', e => {
    if (e.code === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      closeSay();
      return;
    }
    if (e.target !== sayInput) return;
    // A pick's number on an empty line takes it; any other is typed.
    const n = /^[1-9]$/.test(e.key) ? e.key : null;
    if (n && !sayInput.value && pickWords[n] && !e.ctrlKey && !e.metaKey && !e.altKey) {
      e.preventDefault();
      e.stopPropagation();
      sendSay(pickWords[n]);
    }
  });
  sayForm.addEventListener('focusout', e => {
    if (!sayForm.contains(e.relatedTarget)) {
      sayForm.hidden = true;
      clearInterval(picksTimer);
    }
  });
  sayForm.addEventListener('submit', e => {
    e.preventDefault();
    sendSay(sayInput.value.trim());
  });

  // A game's plates to drag one onto another (`controls.drag`), such as the
  // Delve's hand: where they are comes in the game's `slots` line, each
  // `[slot, x, y, w, h]` in the frame's pixels. A plate pressed and let go
  // on another sends `{from, to}` as the drag's action kind; let go
  // anywhere else, or on its own plate, it sends nothing. Mouse, pen and
  // touch alike: the stage takes no panning or zooming (shell.css).
  let slots = [];
  const stageBox = $('stage');
  let drag = null;
  let builderContext = false;
  // The marks a drag lays over the picture: the plate it left, the plate
  // it would land on, and its outline riding the pointer.
  const dragFrom = el('div', 'drag-mark from'), dragTo = el('div', 'drag-mark to'), dragGhost = el('div', 'drag-mark ghost');
  for (const mark of [dragFrom, dragTo, dragGhost]) {
    mark.hidden = true;
    document.body.append(mark);
  }
  // A plate under the pointer, a frame pixel of slack round it (half the
  // gap between two plates), or null.
  function slotAt(e) {
    if (!pic || !slots.length) return null;
    const fx = (e.clientX - pic.left) / pic.unit, fy = (e.clientY - pic.top) / pic.unit;
    return slots.find(([, sx, sy, sw, sh]) => fx >= sx - 1 && fx < sx + sw + 1 && fy >= sy - 1 && fy < sy + sh + 1) || null;
  }
  function cover(mark, plate) {
    mark.hidden = !plate;
    if (!plate) return;
    const [, sx, sy, sw, sh] = plate;
    mark.style.left = `${pic.left + sx * pic.unit}px`;
    mark.style.top = `${pic.top + sy * pic.unit}px`;
    mark.style.width = `${sw * pic.unit}px`;
    mark.style.height = `${sh * pic.unit}px`;
  }
  function follow(e) {
    const [, , , sw, sh] = drag.plate;
    dragGhost.hidden = false;
    dragGhost.style.left = `${e.clientX - sw * pic.unit / 2}px`;
    dragGhost.style.top = `${e.clientY - sh * pic.unit / 2}px`;
    dragGhost.style.width = `${sw * pic.unit}px`;
    dragGhost.style.height = `${sh * pic.unit}px`;
    const over = slotAt(e);
    cover(dragTo, over && over[0] !== drag.plate[0] ? over : null);
  }
  function letGo() {
    drag = null;
    for (const mark of [dragFrom, dragTo, dragGhost]) mark.hidden = true;
    stageBox.classList.remove('dragging');
  }
  stageBox.addEventListener('pointerdown', e => {
    builderContext = false;
    if (!drag && pic && x && typeof x.angel_pointer === 'function' && (e.button === 0 || e.button === 2)) {
      const fx = Math.floor((e.clientX - pic.left) / pic.unit);
      const fy = Math.floor((e.clientY - pic.top) / pic.unit);
      if (fx >= 0 && fy >= 0 && x.angel_pointer(fx, fy, e.button)) {
        builderContext = e.button === 2;
        e.preventDefault();
        flush();
        return;
      }
    }
    const d = A.controls && A.controls.drag;
    if (!d || !x || drag || e.button !== 0) return;
    const plate = slotAt(e);
    if (!plate) return;
    e.preventDefault();
    try { stageBox.setPointerCapture(e.pointerId); } catch { /* the drag still follows within the stage */ }
    drag = { id: e.pointerId, plate };
    cover(dragFrom, plate);
    stageBox.classList.add('dragging');
    follow(e);
  });
  stageBox.addEventListener('contextmenu', e => {
    if (builderContext) { e.preventDefault(); builderContext = false; }
  });
  stageBox.addEventListener('pointermove', e => {
    if (drag && e.pointerId === drag.id) follow(e);
    else if (!drag && e.pointerType === 'mouse') stageBox.classList.toggle('over-slot', !!slotAt(e));
  });
  stageBox.addEventListener('pointerup', e => {
    if (!drag || e.pointerId !== drag.id) return;
    const d = A.controls && A.controls.drag, from = drag.plate[0], over = slotAt(e);
    letGo();
    if (!d || !x || !over || over[0] === from) return;
    withString(JSON.stringify({ from, to: over[0] }), (p, l) => x.angel_action(d.kind >>> 0, p, l));
    flush();
  });
  stageBox.addEventListener('pointercancel', e => { if (drag && e.pointerId === drag.id) letGo(); });

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
  // A page that stalled (a long frame, a throttled tab) runs this late, with
  // messages still queued behind it: that is the page's silence, not the
  // link's, so the clock starts again.
  let lastCheck = performance.now();
  setInterval(() => {
    const now = performance.now();
    const late = now - lastCheck > 1500;
    lastCheck = now;
    if (late) {
      lastHeard = Math.max(lastHeard, now);
      return;
    }
    if (sock && sock.readyState === 1 && !document.hidden && now - lastHeard > SILENCE_MS) {
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
      } else if (verb === 'ask') {
        openSay();
      } else if (verb === 'slots') {
        try { slots = JSON.parse(rest) || []; } catch { slots = []; }
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
    // A tab hidden when the game trapped is still being kept idle; the
    // visibility handler will not clear that once the page is crashed.
    clearInterval(hiddenTimer);
    hiddenTimer = 0;
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
      placePicture(w, h);
      // The frame from its parts where the page and the game both can
      // (`?stage=0` keeps finished frames).
      let staged = false, sp = 0, sl = 0;
      if (useStage && x.angel_stage) {
        sp = x.angel_stage(t) >>> 0;
        sl = x.angel_stage_len() >>> 0;
        staged = sl > 0 && presenter.drawStage(mem(), sp, sl, t);
        if (staged) {
          takeHd();
          takePaintings();
        }
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

  // ---- paintings ----

  // A painting the stage would draw (a wish's picture, by its sha256):
  // fetched from the host beside the page, decoded off the frame with the
  // browser's own decoder, and its pixels handed to the game, which sends
  // it to the presenter with the stage frames that name it. Until then, and
  // if it never comes, the stage draws its inks.
  function takePaintings() {
    if (!x.angel_painting_wanted) return;
    for (let len = x.angel_painting_wanted() >>> 0; len; len = x.angel_painting_wanted() >>> 0) {
      const name = text(len);
      if (/^[0-9a-f]{64}$/.test(name)) fetchPainting(name, '/paint/' + name + '.png');
      else if (HD_NAME.test(name) && !name.includes('..')) fetchPainting(name, '/' + name);
      else withString(name, (p, l) => x.angel_painting_failed(p, l));
    }
  }

  // ---- HD art (docs/LADDER.md, "The browser") ----

  // `?hd=1`: the host's HD art pack (`/hd/pack.json`, served when the host
  // runs with ANGEL_DELVE_HD_WEB=1) handed to the game with the page's
  // whole-number scale of its pixels; the game then names the drawings its
  // knights' rungs want among the pictures above (`hd/<file>`), each fetched
  // from `/hd/<file>` the first time a figure on screen asks for it.
  // `?ladder=32|64|128` shows every knight at that rung (to look at one).
  const HD_NAME = /^hd\/[a-z0-9_@.\-\/]+\.png$/;
  let hdState = params.get('hd') === '1' ? 0 : 2; // 0 to ask, 1 asking, 2 done
  function takeHd() {
    if (hdState !== 0 || !x.angel_hd_manifest) return;
    hdState = 1;
    const scale = Math.max(1, Math.min(
      Math.floor(window.innerWidth * (window.devicePixelRatio || 1) / 384),
      Math.floor(window.innerHeight * (window.devicePixelRatio || 1) / 224)));
    fetch('/hd/pack.json').then(r => {
      if (!r.ok) throw new Error('HTTP ' + r.status);
      return r.text();
    }).then(json => {
      if (!x || crashed) return;
      // `?ladder=64`: every knight shown at that rung, to look at it.
      const rung = Number(params.get('ladder')) || 0;
      const took = withString(json, (p, l) => x.angel_hd_manifest(p, l, scale, rung)) >>> 0;
      // Not taken yet (no game): asked again on the next frame.
      hdState = took ? 2 : 0;
      if (took) console.info('angelX: HD art at up to ' + scale + 'x');
    }).catch(e => {
      hdState = 2;
      console.info('angelX: no HD art (' + (e && e.message || e) + ')');
    });
  }
  async function fetchPainting(name, url) {
    let rgba = null, w = 0, h = 0;
    try {
      const r = await fetch(url);
      if (!r.ok) throw new Error('HTTP ' + r.status);
      const bitmap = await createImageBitmap(await r.blob(), { premultiplyAlpha: 'none', colorSpaceConversion: 'none' });
      w = bitmap.width;
      h = bitmap.height;
      const pad = typeof OffscreenCanvas === 'function'
        ? new OffscreenCanvas(w, h)
        : Object.assign(document.createElement('canvas'), { width: w, height: h });
      const g = pad.getContext('2d', { willReadFrequently: true });
      g.drawImage(bitmap, 0, 0);
      bitmap.close();
      rgba = g.getImageData(0, 0, w, h).data;
    } catch (e) {
      console.warn('a painting did not load', e);
    }
    if (!x || crashed) return;
    try {
      if (!rgba) {
        withString(name, (p, l) => x.angel_painting_failed(p, l));
        return;
      }
      const p = x.angel_alloc(rgba.length) >>> 0;
      try {
        mem().set(rgba, p);
        withString(name, (n, l) => x.angel_put_painting(n, l, w, h, p, rgba.length));
      } finally {
        x.angel_free(p, rgba.length);
      }
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
      const idle = () => {
        try {
          x.angel_idle(performance.now());
          flush();
        } catch (e) {
          crash(e);
        }
      };
      // Mark the runtime idle immediately, not a second later: even a
      // short hidden visit must discard queued and incoming transient SFX.
      idle();
      hiddenTimer = setInterval(idle, 1000);
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
    if (A.status === 'loading') A.status = 'failed';
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
