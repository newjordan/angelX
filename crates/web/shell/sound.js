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
// than silent.
const LATE_S = 0.25;
// Music sits under the effects (the Delve's host plays it at 0.35 against
// effects at 0.5).
const MUSIC_LEVEL = 0.7;
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
  const buffers = new Map(); // name -> Promise<AudioBuffer | null>
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
    if (ctx.state !== 'running') ctx.resume().catch(() => {});
    play();
  }

  function load(name) {
    let p = buffers.get(name);
    if (!p) {
      const url = manifest[name];
      p = !url ? Promise.resolve(null) : fetch(url)
        .then(r => (r.ok ? r.arrayBuffer() : Promise.reject(r.status)))
        .then(bytes => ctx.decodeAudioData(bytes))
        .catch(() => null);
      buffers.set(name, p);
    }
    return p;
  }

  function sfx(name) {
    if (!ctx || ctx.state !== 'running') return;
    const asked = ctx.currentTime;
    load(name).then(buffer => {
      if (!buffer || ctx.currentTime - asked > LATE_S) return;
      while (voices.length >= VOICES) voices.shift().stop();
      const src = ctx.createBufferSource();
      src.buffer = buffer;
      src.connect(master);
      src.onended = () => {
        const i = voices.indexOf(src);
        if (i >= 0) voices.splice(i, 1);
      };
      src.start();
      voices.push(src);
      played++;
    });
  }

  function music(name) {
    if (name === trackName) return;
    trackName = name;
    if (track) {
      track.stop();
      track = null;
    }
    play();
  }

  // Start the track wanted, once there is audio to play it on.
  let starting = '';
  function play() {
    const name = trackName;
    if (!name || !ctx || track || starting === name) return;
    starting = name;
    load(name).then(buffer => {
      if (starting === name) starting = '';
      if (!buffer || trackName !== name || track) return;
      track = ctx.createBufferSource();
      track.buffer = buffer;
      track.loop = true;
      track.connect(musicGain);
      track.start();
    });
  }

  return {
    unlock,
    sfx,
    music,
    suspend() { if (ctx) ctx.suspend().catch(() => {}); },
    resume() { if (ctx) ctx.resume().catch(() => {}); },
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
