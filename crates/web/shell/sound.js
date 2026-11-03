'use strict';
// Sound: Web Audio. Browsers start a page's audio suspended until the
// player does something (Safari and iOS strictly), so the first key or
// click resumes it. `sfx <name>` plays a short sound from the bundle's
// manifest, fetched and decoded the first time it is asked for; a few
// voices at most, the oldest cut when a new one needs room. `music <name>`
// loops one track under the effects; `music` alone stops it. A track asked
// for before the page may make sound starts on the first key or click.

/* exported makeSound */
const VOICES = 6;
// A sound that took longer than this to load is skipped: late is worse
// than silent. Use wall time: the audio clock freezes while suspended.
const LATE_S = 0.25;
// Music sits well under the effects: 15% (the operator, 2026-10-28; the
// Delve's host plays it at 0.15 too).
const MUSIC_LEVEL = 0.15;
const MUSIC_RETRY_MS = 1000;
const MUSIC_LOAD_MS = 15000;
// The player's own volume (0 to 1) and mute, kept in this browser.
const VOLUME_KEY = 'angel-volume';
const MUTE_KEY = 'angel-muted';
function remembered(key, fallback) {
  try {
    const v = localStorage.getItem(key);
    return v === null ? fallback : JSON.parse(v);
  } catch (e) {
    return fallback;
  }
}
function remember(key, value) {
  try { localStorage.setItem(key, JSON.stringify(value)); } catch (e) { /* private window */ }
}

function makeSound(manifest) {
  let ctx = null, master = null, musicGain = null, track = null, trackName = '';
  const buffers = new Map(); // SFX only: name -> Promise<AudioBuffer | null>
  let playingName = '', generation = 0, loading = null, retryTimer = null, retries = 0;
  let effectsGeneration = 0;
  const voices = []; // playing sources, oldest first
  let played = 0;
  let volume = Math.min(1, Math.max(0, Number(remembered(VOLUME_KEY, 0.8)) || 0));
  let muted = remembered(MUTE_KEY, false) === true;
  function level() {
    if (master) master.gain.value = muted ? 0 : volume;
  }

  // Inside a key or click handler.
  function unlock() {
    if (!ctx) {
      const Ctx = window.AudioContext || window.webkitAudioContext;
      if (!Ctx) return;
      ctx = new Ctx();
      master = ctx.createGain();
      master.connect(ctx.destination);
      level();
      musicGain = ctx.createGain();
      musicGain.gain.value = MUSIC_LEVEL;
      musicGain.connect(master);
    }
    resume();
  }

  function loadEffect(name) {
    let p = buffers.get(name);
    if (!p) {
      const url = manifest[name] || (/^live-[a-f0-9]{48}\.wav$/.test(name) ? `/a/${name}` : null);
      p = !url ? Promise.resolve(null) : fetch(url)
        .then(r => (r.ok ? r.arrayBuffer() : Promise.reject(r.status)))
        .then(bytes => ctx.decodeAudioData(bytes))
        .catch(() => {
          if (buffers.get(name) === p) buffers.delete(name);
          return null;
        });
      // Playing sources retain their own AudioBuffers when cached generations
      // retire. Bound the reusable decoded bank across arbitrarily many edits:
      // room for every shipped effect (the Bard's riff alone is fifteen, the
      // blades, the magic's families and the wishes thirty-four more, the
      // knights' clips ninety-six), so only live edits ever turn it over.
      while (buffers.size >= 256) buffers.delete(buffers.keys().next().value);
      if (/^live-[a-f0-9]{48}\.wav$/.test(name)) {
        const liveNames = [...buffers.keys()].filter(key => /^live-[a-f0-9]{48}\.wav$/.test(key));
        while (liveNames.length >= 32) buffers.delete(liveNames.shift());
      }
      buffers.set(name, p);
    }
    return p;
  }

  function sfx(name) {
    if (!ctx || ctx.state !== 'running') return;
    let gain = 1;
    const live = /^(live-[a-f0-9]{48}\.wav)@(\d{1,3})$/.exec(name);
    if (live) {
      gain = Number(live[2]) / 100;
      if (gain > 1 || gain === 0) return;
      name = live[1];
    }
    const now = () => typeof performance === 'undefined' ? Date.now() : performance.now();
    const asked = now(), epoch = effectsGeneration;
    loadEffect(name).then(buffer => {
      if (!buffer || epoch !== effectsGeneration || ctx.state !== 'running'
          || now() - asked > LATE_S * 1000) return;
      while (voices.length >= VOICES) voices.shift().stop();
      const src = ctx.createBufferSource();
      src.buffer = buffer;
      let channel = null;
      if (live) { channel = ctx.createGain(); channel.gain.value = gain; src.connect(channel); channel.connect(master); }
      else src.connect(master);
      src.onended = () => {
        src.disconnect();
        if (channel) channel.disconnect();
        const i = voices.indexOf(src);
        if (i >= 0) voices.splice(i, 1);
        src.disconnect();
        src.buffer = null;
      };
      try {
        src.start();
        voices.push(src);
        played++;
      } catch (e) { src.onended(); }
    });
  }

  function stopTrack() {
    if (!track) return;
    track.stop();
    track.disconnect();
    track.buffer = null;
    track = null;
    playingName = '';
  }

  function music(name) {
    if (name === trackName && (playingName === name || loading?.generation === generation || retryTimer !== null)) return;
    trackName = name;
    generation++;
    retries = 1;
    if (retryTimer !== null) clearTimeout(retryTimer);
    retryTimer = null;
    // Fetch can be cancelled; decodeAudioData cannot. Keep that one decode
    // in flight until it settles, then prepare only the latest request.
    if (loading) loading.controller.abort();
    if (!name) stopTrack();
    play();
  }

  function retryMusic(epoch) {
    if (epoch !== generation || retries <= 0) return;
    retries--;
    retryTimer = setTimeout(() => {
      retryTimer = null;
      if (epoch === generation) play();
    }, MUSIC_RETRY_MS);
  }

  // No decoded music cache: only the playing source and one preparing track
  // own buffers. Serial preparation also bounds rapid A/B/C requests while
  // a decode (which AbortController cannot cancel) is still working.
  function play() {
    const name = trackName, epoch = generation;
    if (!name || !ctx || playingName === name || loading || retryTimer !== null || !manifest[name]) return;
    const controller = new AbortController();
    loading = { controller, generation: epoch };
    const timeout = setTimeout(() => controller.abort(), MUSIC_LOAD_MS);
    fetch(manifest[name], { signal: controller.signal })
      .then(r => (r.ok ? r.arrayBuffer() : Promise.reject(r.status)))
      .then(bytes => {
        clearTimeout(timeout);
        if (epoch !== generation) return null;
        return ctx.decodeAudioData(bytes);
      })
      .catch(() => null)
      .then(buffer => {
        clearTimeout(timeout);
        loading = null;
        if (epoch !== generation) { play(); return; }
        if (!buffer) { retryMusic(epoch); return; }
        let next;
        try {
          next = ctx.createBufferSource();
          next.buffer = buffer;
          next.loop = true;
          next.connect(musicGain);
          // Start successfully before retiring the old source: network and
          // decode delays (or a failure) must not turn a switch into silence.
          next.start();
        } catch (e) {
          if (next) { next.disconnect(); next.buffer = null; }
          retryMusic(epoch);
          return;
        }
        stopTrack();
        track = next;
        playingName = name;
      });
  }

  function resume() {
    if (!ctx) return;
    if (ctx.state !== 'running') ctx.resume().then(play).catch(() => {});
    play();
  }

  return {
    unlock,
    sfx,
    music,
    suspend() {
      effectsGeneration++; // A frozen audio clock must not preserve old SFX.
      while (voices.length) {
        const src = voices.shift();
        src.stop();
        src.disconnect();
        src.buffer = null;
      }
      if (ctx) ctx.suspend().catch(() => {});
    },
    resume,
    get state() { return ctx ? ctx.state : 'locked'; },
    get volume() { return volume; },
    get muted() { return muted; },
    setVolume(v) {
      volume = Math.min(1, Math.max(0, v));
      if (volume > 0) muted = false;
      remember(VOLUME_KEY, volume);
      remember(MUTE_KEY, muted);
      level();
    },
    toggleMute() {
      muted = !muted;
      remember(MUTE_KEY, muted);
      level();
    },
    get played() { return played; },
  };
}
