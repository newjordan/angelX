'use strict';
// The dev panel: the game's knobs and the engine's as sliders, moved while
// the game runs, beside the numbers that say what a move did. See
// docs/ENGINE.md, "The dev panel".
//
// It is offered only on a dev link (<door>/#<key>&dev) or in a session the
// host started in dev mode, and F2 opens and shuts it; a student's page
// never shows it. Three kinds of knob, one list:
//
// - the server's (the game's rules, the engine's snapshot rate): asked of
//   the server, which sets them between ticks for everyone and answers
//   with its table; only the host may, unless the session runs in dev mode;
// - this page's module's (how far behind everyone else is drawn, how long
//   a correction eases, the art kit): set at once, here only;
// - the page's own (whole-number scaling, a frame-rate cap).
//
// Client knobs are kept per game in localStorage, so a reload keeps them.
// "Copy as constants" gives every knob as `CONSTANT = value`, to paste back
// into the code once a number feels right: tune live, then bake it in.

/* exported makePanel */
function makePanel(env) {
  // env: { x, text(len), withString(s, f), stats(), game, dev, page: [knob],
  //        setPage(name, value), releaseAll() }
  const SEND_EVERY_MS = 80;
  const KEEP = 'angel.knobs.' + env.game;
  const box = document.createElement('aside');
  box.id = 'panel';
  box.hidden = true;
  box.tabIndex = -1;
  const head = document.createElement('header');
  const title = document.createElement('span');
  title.textContent = 'dev panel (F2)';
  const resetAll = button('reset all', 'Every knob back to the code\'s number');
  const copy = button('copy as constants', 'Every knob as CONSTANT = value, to paste into the code');
  head.append(title, resetAll, copy);
  const strip = document.createElement('pre');
  strip.className = 'strip';
  const note = document.createElement('div');
  note.className = 'note';
  const list = document.createElement('div');
  list.className = 'knobs';
  const out = document.createElement('textarea');
  out.className = 'constants';
  out.hidden = true;
  out.readOnly = true;
  box.append(head, strip, note, list, out);
  document.body.append(box);

  let offered = env.dev, open = false, said = -1, may = false;
  let server = [], client = [];
  const rows = new Map(); // name -> { knob, range, value, reset, side }
  const sent = new Map(); // name -> { at, timer }

  function button(label, title) {
    const b = document.createElement('button');
    b.type = 'button';
    b.textContent = label;
    b.title = title;
    return b;
  }

  function kept() {
    try {
      return JSON.parse(localStorage.getItem(KEEP) || '{}');
    } catch {
      return {};
    }
  }
  function keep(name, value, isDefault) {
    const k = kept();
    if (isDefault) delete k[name];
    else k[name] = value;
    localStorage.setItem(KEEP, JSON.stringify(k));
  }

  // The module's knob table: the server's (as it last said it) and its own.
  function read() {
    let t = null;
    try {
      t = JSON.parse(env.text(env.x.angel_knobs()));
    } catch {
      return;
    }
    may = !!(t.server && t.server.may);
    if (t.server && t.server.dev) offered = true;
    server = t.server ? t.server.knobs : [];
    client = t.client || [];
  }

  // Client knobs kept from an earlier visit, set once the game is seated.
  function restore() {
    const k = kept();
    for (const [name, value] of Object.entries(k)) {
      if (env.page.some(p => p.name === name)) env.setPage(name, value);
      else env.withString(name, (p, l) => env.x.angel_set_knob(p, l, value));
    }
  }

  function all() {
    return [...server, ...client, ...env.page];
  }

  function shown(k, v) {
    const digits = k.step >= 1 ? 0 : Math.min(4, Math.max(0, -Math.floor(Math.log10(k.step))));
    return Number(v).toFixed(digits);
  }

  function build() {
    list.textContent = '';
    rows.clear();
    const groups = new Map();
    for (const k of all()) {
      const g = k.group + (k.side === 'server' ? ' (everyone)' : ' (this page)');
      if (!groups.has(g)) groups.set(g, []);
      groups.get(g).push(k);
    }
    for (const [g, knobs] of groups) {
      const h = document.createElement('h2');
      h.textContent = g;
      list.append(h);
      for (const k of knobs) {
        const row = document.createElement('label');
        row.className = 'knob';
        row.title = k.constant + ' (' + k.name + ')';
        const name = document.createElement('span');
        name.className = 'name';
        name.textContent = k.label;
        const range = document.createElement('input');
        range.type = 'range';
        range.min = k.min;
        range.max = k.max;
        range.step = k.step;
        range.value = k.value;
        const value = document.createElement('span');
        value.className = 'value';
        const reset = button('↺', 'Back to ' + shown(k, k.default));
        reset.className = 'reset';
        const locked = k.side === 'server' && !may;
        range.disabled = reset.disabled = locked;
        row.classList.toggle('locked', locked);
        row.append(name, range, value, reset);
        list.append(row);
        const r = { knob: k, range, value, reset, dragging: false };
        rows.set(k.name, r);
        show(r, k.value);
        range.addEventListener('pointerdown', () => { r.dragging = true; });
        range.addEventListener('input', () => set(r, Number(range.value), false));
        range.addEventListener('change', () => {
          r.dragging = false;
          set(r, Number(range.value), true);
          // The keys go back to the game at once.
          range.blur();
        });
        reset.addEventListener('click', e => {
          e.preventDefault();
          set(r, k.default, true);
          reset.blur();
        });
      }
    }
    note.textContent = server.length && !may
      ? 'everyone\'s knobs: only the host sets them (start it in dev mode, ANGEL_DELVE_DEV=1 or --dev, to set them from any page)'
      : '';
  }

  function show(r, v) {
    r.value.textContent = shown(r.knob, v);
    r.row = r.row || r.value.parentElement;
    r.value.parentElement.classList.toggle('moved', Number(v) !== r.knob.default);
  }

  // Set a knob: the server's asked for (dragging, at most every
  // SEND_EVERY_MS, and the last one always), this page's set at once.
  function set(r, v, last) {
    const k = r.knob;
    r.range.value = v;
    show(r, v);
    if (k.side === 'server') {
      const s = sent.get(k.name) || { at: 0, timer: 0 };
      sent.set(k.name, s);
      const go = () => {
        s.at = performance.now();
        s.timer = 0;
        env.withString(k.name, (p, l) => env.x.angel_tune(p, l, Number(r.range.value)));
        env.flush();
      };
      clearTimeout(s.timer);
      if (last || performance.now() - s.at >= SEND_EVERY_MS) go();
      else s.timer = setTimeout(go, SEND_EVERY_MS);
      return;
    }
    let took = v;
    if (env.page.some(p => p.name === k.name)) {
      took = env.setPage(k.name, v);
    } else {
      took = env.withString(k.name, (p, l) => env.x.angel_set_knob(p, l, v));
    }
    if (!Number.isNaN(took)) {
      k.value = took;
      r.range.value = took;
      show(r, took);
      keep(k.name, took, took === k.default);
    }
  }

  // The server said a new table: its rows follow, unless one is held.
  function refresh() {
    const n = env.x.angel_knobs_said();
    if (n === said) return;
    const wasMay = may, names = server.map(k => k.name).join();
    said = n;
    read();
    if (!open) return;
    if (wasMay !== may || names !== server.map(k => k.name).join() || !rows.size) {
      build();
      return;
    }
    for (const k of server) {
      const r = rows.get(k.name);
      if (!r) continue;
      r.knob = k;
      if (!r.dragging) {
        r.range.value = k.value;
        show(r, k.value);
      }
    }
  }

  // ---- the numbers ----

  let last = null;
  function numbers() {
    const s = env.stats();
    let srv = {}, game = {};
    try {
      const t = JSON.parse(env.text(env.x.angel_stats()));
      srv = t.server || {};
      game = t.game || {};
    } catch {}
    const now = performance.now();
    const was = last;
    last = { at: now, bytesIn: s.bytesIn, bytesOut: s.bytesOut, dropped: s.dropped, corrections: game.corrections || 0 };
    if (!was) return;
    const secs = Math.max(0.001, (now - was.at) / 1000);
    const per = (a, b) => ((a - b) / secs).toFixed(1);
    const kb = (a, b) => ((a - b) / secs / 1024).toFixed(2);
    const n = v => (v === undefined ? '-' : v);
    strip.textContent =
      `fps ${s.fps.toFixed(0)}  dropped ${per(s.dropped, was.dropped)}/s\n` +
      `frame ${s.wasmP50.toFixed(2)} / ${s.wasmP95.toFixed(2)} ms  present ${s.presentP95.toFixed(2)} ms\n` +
      `rtt ${s.rtt < 0 ? '-' : s.rtt.toFixed(0) + ' ms'}  in ${kb(s.bytesIn, was.bytesIn)} KB/s  out ${kb(s.bytesOut, was.bytesOut)} KB/s\n` +
      `server tick ${n(srv.tick_ms)} / ${n(srv.tick_ms_max)} ms  snapshot every ${n(srv.snapshot_every)}\n` +
      `queue ${n(srv.queued)}  repeated ${n(srv.repeated)}/s  skipped ${n(srv.skipped)}/s\n` +
      `corrections ${per(last.corrections, was.corrections)}/s`;
  }

  // ---- open, shut ----

  function toggle() {
    if (!offered) return;
    open = !open;
    box.hidden = !open;
    document.body.classList.toggle('panel-open', open);
    env.x.angel_watch_stats(open ? 1 : 0);
    env.flush();
    if (open) {
      said = -1;
      refresh();
      build();
      numbers();
    } else if (box.contains(document.activeElement)) {
      document.activeElement.blur();
    }
  }

  window.addEventListener('keydown', e => {
    if (e.code === 'F2' && offered) {
      e.preventDefault();
      toggle();
    } else if (e.code === 'Escape' && box.contains(document.activeElement)) {
      document.activeElement.blur();
    }
  }, true);
  // Keys typed into the panel are the panel's: the game lets go at once.
  box.addEventListener('focusin', () => env.releaseAll());
  // A click on the game gives the keys back.
  document.getElementById('stage').addEventListener('pointerdown', () => {
    if (box.contains(document.activeElement)) document.activeElement.blur();
  });

  copy.addEventListener('click', () => {
    const text = all().map(k => `${k.constant} = ${shown(k, k.value)}  // ${k.name}${k.value === k.default ? '' : ' (moved)'}`).join('\n');
    out.hidden = false;
    out.value = text;
    out.select();
    if (navigator.clipboard && window.isSecureContext) navigator.clipboard.writeText(text).catch(() => {});
    console.info(text);
    copy.blur();
  });
  resetAll.addEventListener('click', () => {
    for (const r of rows.values()) {
      if (Number(r.range.value) !== r.knob.default && !r.range.disabled) set(r, r.knob.default, true);
    }
    resetAll.blur();
  });

  setInterval(() => { if (env.x) refresh(); }, 250);
  setInterval(() => { if (open) numbers(); }, 1000);

  return {
    // Seated: what was kept is set, and a dev link or a dev session says
    // F2 is there.
    seated() {
      read();
      restore();
      if (offered) env.toast('F2 opens the dev panel');
    },
    // Whether the keyboard is the panel's now.
    focused() {
      return open && box.contains(document.activeElement);
    },
    get offered() {
      return offered;
    },
  };
}
