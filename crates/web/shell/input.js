'use strict';

const BUTTON = {
  TEAM: (1 << 15) | (1 << 13),
  UP: 1 << 0, DOWN: 1 << 1, LEFT: 1 << 2, RIGHT: 1 << 3,
  FIRE: 1 << 4, USE: 1 << 5,
  SLOT1: 1 << 6, SLOT2: 1 << 7, SLOT3: 1 << 8, SLOT4: 1 << 9,
  JUMP: 1 << 10, MENU: 1 << 11, ALT: 1 << 12, SHIFT: 1 << 13, MAP: 1 << 14, OK: 1 << 15,
  AIM_UP: 1 << 16, AIM_DOWN: 1 << 17, AIM_LEFT: 1 << 18, AIM_RIGHT: 1 << 19,
  X1: 1 << 20, X2: 1 << 21, X3: 1 << 22, X4: 1 << 23, X5: 1 << 24, X6: 1 << 25,
  X7: 1 << 26, X8: 1 << 27, X9: 1 << 28, X10: 1 << 29, X11: 1 << 30, X12: (1 << 31) >>> 0,
};

const KEYS = {
  KeyW: BUTTON.UP, ArrowUp: BUTTON.UP,
  KeyS: BUTTON.DOWN, ArrowDown: BUTTON.DOWN,
  KeyA: BUTTON.LEFT, ArrowLeft: BUTTON.LEFT,
  KeyD: BUTTON.RIGHT, ArrowRight: BUTTON.RIGHT,
  KeyE: BUTTON.USE, KeyF: BUTTON.FIRE,
  Digit1: BUTTON.SLOT1, Digit2: BUTTON.SLOT2, Digit3: BUTTON.SLOT3, Digit4: BUTTON.SLOT4,
  Numpad1: BUTTON.SLOT1, Numpad2: BUTTON.SLOT2, Numpad3: BUTTON.SLOT3, Numpad4: BUTTON.SLOT4,
  Space: BUTTON.JUMP, Escape: BUTTON.MENU, KeyQ: BUTTON.ALT,
  ShiftLeft: BUTTON.SHIFT, ShiftRight: BUTTON.SHIFT,
  Tab: BUTTON.MAP, Enter: BUTTON.OK, NumpadEnter: BUTTON.OK,
};

const PAD = [
  BUTTON.USE, BUTTON.JUMP, BUTTON.SLOT1, BUTTON.SLOT2,
  BUTTON.ALT, BUTTON.SHIFT, BUTTON.SLOT3, BUTTON.FIRE,
  BUTTON.MAP, BUTTON.MENU, BUTTON.OK, BUTTON.SLOT4,
  BUTTON.UP, BUTTON.DOWN, BUTTON.LEFT, BUTTON.RIGHT,
];
const STICKS = { move: 'left', aim: '', aimHolds: 0 };
const AXES = { left: 0, right: 2 };
const DEAD = 0.2;
const LEAN = 0.5;

function bitOf(name) {
  return typeof name === 'string' && Object.prototype.hasOwnProperty.call(BUTTON, name) ? BUTTON[name] : 0;
}

function keysFrom(keys) {
  const out = {};
  for (const [code, name] of Object.entries(keys)) {
    const bit = bitOf(name);
    if (bit) out[code] = bit;
  }
  return out;
}

function padFrom(pad) {
  const plain = [];
  const chords = [];
  for (const [at, name] of Object.entries(pad)) {
    const bit = bitOf(name);
    if (!bit) continue;
    const parts = at.split('+').map(Number);
    if (!parts.every(n => Number.isInteger(n) && n >= 0 && n < 32)) continue;
    if (parts.length === 1) plain[parts[0]] = bit;
    else if (parts.length === 2) chords.push([parts[0], parts[1], bit]);
  }
  return { plain, chords };
}

function padBitsOf(map, down, count) {
  let bits = 0;
  const taken = new Set();
  for (const [lead, button, bit] of map.chords) {
    taken.add(lead);
    if (down(lead) && down(button)) {
      bits |= bit;
      taken.add(button);
    }
  }
  const n = Math.min(count, map.plain.length);
  for (let i = 0; i < n; i++) {
    if (map.plain[i] && !taken.has(i) && down(i)) bits |= map.plain[i];
  }
  return bits >>> 0;
}

function makeInput(x, on, active) {
  const held = new Map();
  let keys = KEYS;
  let pad = { plain: PAD, chords: [] };
  let sticks = STICKS;
  let keyBits = 0;
  let padBits = 0, padX = 0, padY = 0, padStick = false, pads = 0;
  let aimX = 0, aimY = 0, padAim = false;

  function setKeys(bits) {
    bits >>>= 0;
    const down = (bits & ~keyBits) >>> 0, up = (keyBits & ~bits) >>> 0;
    keyBits = bits;
    if (down) x.angel_down(down);
    if (up) x.angel_up(up);
  }

  function recount() {
    let bits = 0;
    for (const bit of held.values()) bits |= bit;
    setKeys(bits);
  }

  window.addEventListener('keydown', e => {
    if (!active() || e.ctrlKey || e.metaKey) return;
    const bit = keys[e.code];
    if (!bit) return;
    e.preventDefault();
    if (e.repeat || held.has(e.code)) return;
    held.set(e.code, bit);
    recount();
    on('key');
  });

  window.addEventListener('keyup', e => {
    if (e.code === 'MetaLeft' || e.code === 'MetaRight') {
      releaseAll();
      return;
    }
    if (!held.delete(e.code)) return;
    e.preventDefault();
    recount();
  });

  window.addEventListener('gamepadconnected', () => { pads++; });
  window.addEventListener('gamepaddisconnected', () => { pads = Math.max(0, pads - 1); });

  function lean(sx, sy) {
    const len = Math.hypot(sx, sy);
    if (len <= DEAD) return null;
    const s = Math.min(1, (len - DEAD) / (1 - DEAD)) / len;
    return [Math.round(sx * s * 127), Math.round(sy * s * 127), len];
  }

  function poll() {
    if (!pads || !navigator.getGamepads) return;
    let bits = 0, move = null, aim = null;
    for (const g of navigator.getGamepads()) {
      if (!g || !g.connected) continue;
      const down = i => !!(g.buttons[i] && (g.buttons[i].pressed || g.buttons[i].value > 0.5));
      bits |= padBitsOf(pad, down, g.buttons.length);
      const stick = side => {
        const at = AXES[side];
        if (at === undefined || g.axes.length < at + 2 || (at > 0 && g.mapping !== 'standard')) return null;
        return lean(g.axes[at], g.axes[at + 1]);
      };
      const m = sticks.move ? stick(sticks.move) : null;
      if (m && (!move || m[2] > move[2])) move = m;
      const a = sticks.aim ? stick(sticks.aim) : null;
      if (a && (!aim || a[2] > aim[2])) aim = a;
    }
    const past = LEAN * 127;
    if (move) {
      if (move[0] < -past) bits |= BUTTON.LEFT;
      if (move[0] > past) bits |= BUTTON.RIGHT;
      if (move[1] < -past) bits |= BUTTON.UP;
      if (move[1] > past) bits |= BUTTON.DOWN;
    }
    if (aim) {
      if (aim[0] < -past) bits |= BUTTON.AIM_LEFT;
      if (aim[0] > past) bits |= BUTTON.AIM_RIGHT;
      if (aim[1] < -past) bits |= BUTTON.AIM_UP;
      if (aim[1] > past) bits |= BUTTON.AIM_DOWN;
      if (aim[2] > LEAN) bits |= sticks.aimHolds;
    }
    if (!active()) {
      bits = 0;
      move = aim = null;
    }
    bits >>>= 0;
    if (bits !== padBits) {
      if ((bits & ~padBits) >>> 0) on('pad');
      padBits = bits;
      x.angel_set_held(bits);
    }
    const [mx, my] = move || [0, 0];
    if (!!move !== padStick || mx !== padX || my !== padY) {
      padStick = !!move;
      padX = mx;
      padY = my;
      x.angel_stick(move ? 1 : 0, mx, my);
    }
    const [ax, ay] = aim || [0, 0];
    if (!!aim !== padAim || ax !== aimX || ay !== aimY) {
      padAim = !!aim;
      aimX = ax;
      aimY = ay;
      x.angel_aim(aim ? 1 : 0, ax, ay);
    }
  }

  function releaseAll() {
    held.clear();
    keyBits = padBits = padX = padY = aimX = aimY = 0;
    padStick = padAim = false;
    x.angel_release_all(performance.now());
  }
  window.addEventListener('blur', releaseAll);

  function setControls(controls) {
    const c = controls && typeof controls === 'object' ? controls : {};
    keys = c.keys && typeof c.keys === 'object' ? keysFrom(c.keys) : KEYS;
    pad = c.pad && typeof c.pad === 'object' ? padFrom(c.pad) : { plain: PAD, chords: [] };
    sticks = STICKS;
    if (c.sticks && typeof c.sticks === 'object') {
      const side = s => (s === 'left' || s === 'right' ? s : '');
      sticks = { move: side(c.sticks.move), aim: side(c.sticks.aim), aimHolds: bitOf(c.sticks.aim_holds) };
    }
    if (held.size || padBits || padStick || padAim) releaseAll();
  }

  return { poll, releaseAll, setControls };
}
