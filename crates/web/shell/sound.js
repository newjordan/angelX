'use strict';

const VOICES = 6;
const LATE_S = 0.25;
const MUSIC_LEVEL = 0.15;
const BARD = /^bard_[a-h]_(\d{3}|lull)$/;
const BARD_REST = 'bard_rest';
const BARD_VOICES = 3;
const BARD_BANK = 240;
const BARD_AHEAD = 8;
const BARD_BEAT_S = 0.4;
const BARD_BEATS = 96;
const BARD_LOOP_FROM = 32;
const BARD_LEAD_S = 0.3;
const BARD_PUMP_MS = 25;
const BARD_KEEP_MS = 1500;
const BARD_FADE_S = 0.03;
const BARD_OPENING = 6;
const BARD_SONGS = 'abcdefgh';
const MUSIC_RETRY_MS = 1000;
const MUSIC_LOAD_MS = 15000;
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
  try { localStorage.setItem(key, JSON.stringify(value)); } catch (e) { }
}

function makeSound(manifest) {
  let ctx = null, master = null, musicGain = null, track = null, trackName = '';
  const buffers = new Map();
  let playingName = '', generation = 0, loading = null, retryTimer = null, retries = 0;
  let effectsGeneration = 0;
  const voices = [];
  const songBuffers = new Map();
  const songPinned = new Set();
  const songVoices = [];
  const songDecoded = new Map();
  let lane = null, laneTimer = null;
  let played = 0;
  let volume = Math.min(1, Math.max(0, Number(remembered(VOLUME_KEY, 0.8)) || 0));
  let muted = remembered(MUTE_KEY, false) === true;
  function level() {
    if (master) master.gain.value = muted ? 0 : volume;
  }

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
      if (manifest[sliceName(BARD_SONGS[0], 0)]) setTimeout(warmSongs, 1500);
    }
    resume();
  }

  function sliceName(song, beat) {
    return `bard_${song}_${String(beat).padStart(3, '0')}`;
  }

  function loadSlice(name) {
    let p = songBuffers.get(name);
    if (!p) {
      const url = manifest[name];
      p = !url ? Promise.resolve(null) : fetch(url)
        .then(r => (r.ok ? r.arrayBuffer() : Promise.reject(r.status)))
        .then(bytes => ctx.decodeAudioData(bytes))
        .then(buffer => {
          if (songBuffers.get(name) === p) songDecoded.set(name, buffer);
          return buffer;
        })
        .catch(() => {
          if (songBuffers.get(name) === p) songBuffers.delete(name);
          return null;
        });
      songBuffers.set(name, p);
      if (songBuffers.size > BARD_BANK) {
        for (const key of songBuffers.keys()) {
          if (!songPinned.has(key)) { songBuffers.delete(key); songDecoded.delete(key); break; }
        }
      }
    }
    return p;
  }

  function warmSongs() {
    if (!ctx) return;
    for (const song of BARD_SONGS) {
      for (let beat = 0; beat < BARD_OPENING; beat++) {
        const name = sliceName(song, beat);
        if (manifest[name]) { songPinned.add(name); loadSlice(name); }
      }
      const lull = `bard_${song}_lull`;
      if (manifest[lull]) { songPinned.add(lull); loadSlice(lull); }
    }
  }

  function oneShot(name) {
    const now = () => typeof performance === 'undefined' ? Date.now() : performance.now();
    const asked = now(), epoch = effectsGeneration;
    loadSlice(name).then(buffer => {
      if (!buffer || epoch !== effectsGeneration || ctx.state !== 'running'
          || now() - asked > LATE_S * 1000) return;
      while (songVoices.length >= BARD_VOICES) songVoices.shift().stop();
      const src = ctx.createBufferSource();
      src.buffer = buffer;
      src.connect(master);
      src.onended = () => {
        src.disconnect();
        const i = songVoices.indexOf(src);
        if (i >= 0) songVoices.splice(i, 1);
        src.buffer = null;
      };
      try {
        src.start();
        songVoices.push(src);
        played++;
      } catch (e) { src.onended(); }
    });
  }

  function beatAt(beat) {
    return beat < BARD_BEATS ? beat : BARD_LOOP_FROM + (beat - BARD_LOOP_FROM) % (BARD_BEATS - BARD_LOOP_FROM);
  }

  function wall() {
    return typeof performance === 'undefined' ? Date.now() : performance.now();
  }

  function stopLane() {
    if (laneTimer !== null) { clearInterval(laneTimer); laneTimer = null; }
    if (!lane) return;
    const held = lane;
    lane = null;
    const t = ctx ? ctx.currentTime : 0;
    try {
      held.gain.gain.setValueAtTime(held.gain.gain.value, t);
      held.gain.gain.linearRampToValueAtTime(0, t + BARD_FADE_S);
    } catch (e) { }
    for (const src of held.sources) {
      try { src.stop(t + BARD_FADE_S + 0.01); } catch (e) { }
    }
    setTimeout(() => held.gain.disconnect(), 250);
  }

  function pumpLane() {
    if (!lane || !ctx) return;
    if (ctx.state !== 'running' || wall() - lane.heard > BARD_KEEP_MS) { stopLane(); return; }
    for (let i = 0; i < BARD_AHEAD; i++) {
      const next = sliceName(lane.song, beatAt(lane.beat + i));
      if (manifest[next]) loadSlice(next);
    }
    const now = ctx.currentTime;
    if (lane.at === null) {
      if (!songDecoded.has(sliceName(lane.song, beatAt(lane.beat)))) return;
      lane.at = now + 0.02;
    }
    if (lane.at < now) {
      const missed = Math.ceil((now - lane.at) / BARD_BEAT_S);
      lane.beat += missed;
      lane.at += missed * BARD_BEAT_S;
    }
    while (lane.at < now + BARD_LEAD_S) {
      const buffer = songDecoded.get(sliceName(lane.song, beatAt(lane.beat)));
      if (buffer) {
        const src = ctx.createBufferSource();
        src.buffer = buffer;
        src.connect(lane.gain);
        const owner = lane;
        src.onended = () => {
          src.disconnect();
          const i = owner.sources.indexOf(src);
          if (i >= 0) owner.sources.splice(i, 1);
          src.buffer = null;
        };
        try {
          src.start(lane.at);
          lane.sources.push(src);
          played++;
        } catch (e) { src.onended(); }
      }
      lane.beat++;
      lane.at += BARD_BEAT_S;
    }
  }

  function song(name, beat) {
    if (!ctx || ctx.state !== 'running') return;
    if (beat < 0) { oneShot(name); return; }
    const letter = name[5];
    if (lane && lane.song === letter) {
      lane.heard = wall();
      return;
    }
    stopLane();
    const gain = ctx.createGain();
    gain.connect(master);
    lane = { song: letter, beat, at: null, heard: wall(), gain, sources: [] };
    loadSlice(name);
    pumpLane();
    laneTimer = setInterval(pumpLane, BARD_PUMP_MS);
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
    if (name === BARD_REST) { stopLane(); return; }
    const bard = BARD.exec(name);
    if (bard) return song(name, bard[1] === 'lull' ? -1 : Number(bard[1]));
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
      effectsGeneration++;
      stopLane();
      for (const bank of [voices, songVoices]) {
        while (bank.length) {
          const src = bank.shift();
          src.stop();
          src.disconnect();
          src.buffer = null;
        }
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
