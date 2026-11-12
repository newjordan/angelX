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
// The Bard's songs (`rules::bard`): a track of Knight Shift Radio cut into one-beat slices,
// `bard_<song>_<beat>`, the held trigger naming each beat and `bard_rest` its letting go. The page
// lays the slices on its own audio clock a beat apart (`song`). Each slice carries 16 ms of the
// next beat's start, faded, so laid on time it crossfades into the one before it: they are never
// counted among the six effect voices or cut by them, and they sit in a bank of their own (the
// first beats of every song and the Lullabies stay in it, the rest turn over).
const BARD = /^bard_[a-h]_(\d{3}|lull)$/;
// Named once when the held trigger is let go: the song stops now.
const BARD_REST = 'bard_rest';
const BARD_VOICES = 3;
const BARD_BANK = 240;
const BARD_AHEAD = 8;
// The song's clock: a beat is 0.4 s; 96 beats, then round the last 64 again.
const BARD_BEAT_S = 0.4;
const BARD_BEATS = 96;
const BARD_LOOP_FROM = 32;
// Slices are laid this far ahead on the audio clock, from a pump this often.
const BARD_LEAD_S = 0.3;
const BARD_PUMP_MS = 25;
// No beat named for this long and no rest heard: the fire stopped (or the link did), so stop.
const BARD_KEEP_MS = 1500;
const BARD_FADE_S = 0.03;
const BARD_OPENING = 6;
const BARD_SONGS = 'abcdefgh';
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
  const songBuffers = new Map(); // the Bard's slices: name -> Promise<AudioBuffer | null>
  const songPinned = new Set();
  const songVoices = []; // the Bard's Lullabies sounding, oldest first
  const songDecoded = new Map(); // the Bard's slices decoded: name -> AudioBuffer
  let lane = null, laneTimer = null; // the held song (`song`)
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
      // Idle a moment, then have every song's opening beats and Lullaby decoded: the first slice
      // of a hold must not wait on a fetch (a late slice is skipped, not played).
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

  // Bard sounds that are not the held song: its Lullaby, played as it comes.
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

  // The held song. While the Bard holds its trigger the page plays the song on its own audio clock,
  // each slice laid exactly a beat after the last, so a slice named early or late (the network, a
  // busy tab) never moves it and nothing restarts mid-song. A press starts it on the beat the game
  // names (where the last press let go); the rest (`bard_rest`) stops it at once.
  function stopLane() {
    if (laneTimer !== null) { clearInterval(laneTimer); laneTimer = null; }
    if (!lane) return;
    const held = lane;
    lane = null;
    const t = ctx ? ctx.currentTime : 0;
    try {
      held.gain.gain.setValueAtTime(held.gain.gain.value, t);
      held.gain.gain.linearRampToValueAtTime(0, t + BARD_FADE_S);
    } catch (e) { /* closed context */ }
    for (const src of held.sources) {
      try { src.stop(t + BARD_FADE_S + 0.01); } catch (e) { /* already stopped */ }
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
      // Not started: wait for the beat it starts on to be decoded, then go.
      if (!songDecoded.has(sliceName(lane.song, beatAt(lane.beat)))) return;
      lane.at = now + 0.02;
    }
    if (lane.at < now) {
      // The clock ran on without us (a hidden tab): rejoin it on the next beat.
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
      // Held: the page's clock carries the song; the named beat only keeps it going.
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
      // Playing sources retain their own AudioBuffers when cached generations
      // retire. Bound the reusable decoded bank across arbitrarily many edits:
      // room for every shipped effect (the blades, the magic's families and
      // the wishes thirty-four more, the knights' clips ninety-six; the
      // Bard's songs have a bank of their own), so only live edits ever turn
      // it over.
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
