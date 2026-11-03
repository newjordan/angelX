// SPDX-License-Identifier: MIT
// Vocabulary, palette data and validation; all rasterization lives in Bend.
import { createHash } from 'node:crypto'

export const TYPES = [
  'burst',
  'ring',
  'flame',
  'beam',
  'slash',
  'orb',
  'comet',
  'vortex',
  'pillar',
  'rays',
  'glyph',
  'puffs',
  'particles',
  'bolt',
  'cracks',
  'spikes',
  'star',
  'streaks',
  'bubble',
  'flash',
  'text',
  'tornado',
]
export const RAMPS = {
  fire: ['43172b', 'a13235', 'e86a2d', 'ffc45c', 'fff1b0'],
  ice: ['182e61', '286c9a', '52bfd0', 'a7eaf4', 'f0ffff'],
  lightning: ['32204d', '7455b8', 'b69aff', 'e9d9ff', 'ffffff'],
  poison: ['1b3031', '346346', '70a735', 'b9df55', 'f2ffc5'],
  holy: ['584026', 'b38337', 'e9bd62', 'ffe8a4', 'ffffe8'],
  shadow: ['17142b', '39234f', '654383', '9b70ba', 'd8b3ed'],
  arcane: ['232a59', '4250a8', '737bea', 'b5b9ff', 'f1ebff'],
  water: ['102f51', '185d85', '309fbb', '81d3e4', 'd1fbff'],
  wind: ['244147', '47756c', '83b29c', 'c3dfc0', 'f2ffe7'],
  earth: ['36293a', '635044', 'a57b50', 'd1af75', 'f0ddac'],
  blood: ['30172c', '6b203a', 'b23748', 'ea6b6a', 'ffc1aa'],
  acid: ['24352c', '547b2b', '9cba27', 'deec55', 'faffbf'],
  nature: ['153c37', '267351', '53a46b', '9ed58c', 'e2f5be'],
  crystal: ['3b284f', '835494', 'b788bd', 'e2bde5', 'fff1fc'],
  metal: ['232d3b', '4d5a6d', '8894a7', 'c0ceda', 'f1f8ff'],
  rainbow: ['5839a5', 'a64c9e', 'e76b75', 'f1bb64', 'd9edba'],
  smoke: ['202333', '424456', '6e7180', 'a3a6b0', 'dce0e4'],
  frost: ['253c60', '47798f', '7fbbca', 'c8e7eb', 'f7ffff'],
}
export const PALETTES = {
  'game-boy': ['142c24', '385c3a', '87a450', 'd5e6a2'],
  '1-bit': ['172027', 'f5f1d8'],
  'pico-8': [
    '000000',
    '1d2b53',
    '7e2553',
    '008751',
    'ab5236',
    '5f574f',
    'c2c3c7',
    'fff1e8',
    'ff004d',
    'ffa300',
    'ffec27',
    '00e436',
    '29adff',
    '83769c',
    'ff77a8',
    'ffccaa',
  ],
}
const FORMS = {
  explosion: ['burst', 'ring', 'particles', 'puffs', 'flash'],
  slash: ['slash', 'particles'],
  hit: ['star', 'particles', 'flash'],
  beam: ['beam', 'orb'],
  projectile: ['comet', 'particles'],
  aura: ['flame', 'particles'],
  portal: ['vortex', 'glyph'],
  strike: ['bolt', 'ring', 'flash'],
  slam: ['ring', 'cracks', 'puffs'],
  heal: ['ring', 'pillar', 'particles'],
  'level-up': ['rays', 'ring', 'particles'],
  rain: ['particles'],
  snow: ['particles'],
  smoke: ['puffs'],
  shield: ['bubble', 'glyph'],
  tornado: ['tornado', 'puffs'],
  spikes: ['spikes', 'particles'],
  damage: ['text'],
  sparkles: ['star', 'particles'],
  orb: ['orb'],
  circle: ['glyph'],
  speed: ['streaks'],
}
const ALIASES = [
  ['level-up', /\blevel[ -]?up\b/],
  ['strike', /\blightning strike\b|\bbolt\b|\bstrike\b/],
  ['portal', /\bblack hole\b|\bportal\b|\bvortex\b/],
  ['slam', /\bground slam\b|\bslam\b|\bcracks\b/],
  ['damage', /\bdamage\b|\bcritical\b/],
  ['explosion', /\bexplosion\b|\bexplode\b|\bnova\b|\bburst\b/],
  ['slash', /\bslash\b|\bsword\b/],
  ['hit', /\bhit\b|\bimpact\b/],
  ['beam', /\bbeam\b|\blaser\b/],
  ['projectile', /\bprojectile\b|\bcomet\b|\bmissile\b|\bfireball\b/],
  ['aura', /\baura\b|\bflame\b/],
  ['heal', /\bheal(?:ing)?\b/],
  ['rain', /\brain\b|\bweather\b/],
  ['snow', /\bsnow\b/],
  ['smoke', /\bsmoke\b|\bpuffs\b/],
  ['shield', /\bshield\b|\bbarrier\b|\bbubble\b/],
  ['tornado', /\btornado\b/],
  ['spikes', /\bspikes\b|\bshards\b/],
  ['sparkles', /\bsparkles\b|\bloot\b|\bstar\b/],
  ['orb', /\borb\b/],
  ['circle', /\bcircle\b|\bglyph\b/],
  ['speed', /\bspeed\b|\bstreaks\b/],
]
const MODES = ['burst', 'rise', 'fall', 'ambient', 'implode', 'orbit', 'trail', 'swirl']
const FONT = {
  0: ['111', '101', '101', '101', '111'],
  1: ['010', '110', '010', '010', '111'],
  2: ['111', '001', '111', '100', '111'],
  3: ['111', '001', '111', '001', '111'],
  4: ['101', '101', '111', '001', '001'],
  5: ['111', '100', '111', '001', '111'],
  6: ['111', '100', '111', '101', '111'],
  7: ['111', '001', '010', '010', '010'],
  8: ['111', '101', '111', '101', '111'],
  9: ['111', '101', '111', '001', '111'],
  '+': ['000', '010', '111', '010', '000'],
  '-': ['000', '000', '111', '000', '000'],
  '!': ['010', '010', '010', '000', '010'],
  ' ': ['000', '000', '000', '000', '000'],
}
const own = (o, k) => Object.hasOwn(o, k)
function object(v, name) {
  if (!v || typeof v !== 'object' || Array.isArray(v)) throw new Error(`${name} must be an object`)
  return v
}
function keys(v, allowed, name) {
  for (const k of Object.keys(v))
    if (!allowed.includes(k)) throw new Error(`Unsupported ${name} field: ${k}`)
}
function num(v, fallback, min, max, name, integer = false) {
  const n = v === undefined ? fallback : v
  if (
    typeof n !== 'number' ||
    !Number.isFinite(n) ||
    n < min ||
    n > max ||
    (integer && !Number.isInteger(n))
  )
    throw new Error(`${name} must be ${integer ? 'an integer' : 'a number'} in ${min}..${max}`)
  return n
}
function bool(v, fallback, name) {
  if (v === undefined) return fallback
  if (typeof v !== 'boolean') throw new Error(`${name} must be boolean`)
  return v
}
function hex(v) {
  if (typeof v !== 'string' || !/^#?[0-9a-f]{6}$/i.test(v))
    throw new Error(`Invalid RGB color: ${v}`)
  return v.replace('#', '').toLowerCase()
}
function ramp(value, ramps) {
  if (typeof value === 'string' && value.startsWith('$')) value = ramps[value.slice(1)] ?? value
  if (typeof value === 'string') value = RAMPS[value]
  if (!Array.isArray(value) || value.length < 2 || value.length > 16)
    throw new Error('ramp must name a palette or contain 2..16 RGB hex colors')
  return value.map(hex)
}
function rotate(colors, angle) {
  if (!angle) return colors
  const a = (angle * Math.PI) / 180,
    c = Math.cos(a),
    s = Math.sin(a)
  return colors.map((v) => {
    const r = parseInt(v.slice(0, 2), 16),
      g = parseInt(v.slice(2, 4), 16),
      b = parseInt(v.slice(4, 6), 16)
    const y = 0.299 * r + 0.587 * g + 0.114 * b,
      i = 0.596 * r - 0.274 * g - 0.322 * b,
      q = 0.211 * r - 0.523 * g + 0.312 * b
    const ii = i * c - q * s,
      qq = i * s + q * c
    return [y + 0.956 * ii + 0.621 * qq, y - 0.272 * ii - 0.647 * qq, y - 1.106 * ii + 1.703 * qq]
      .map((x) =>
        Math.round(Math.max(0, Math.min(255, x)))
          .toString(16)
          .padStart(2, '0'),
      )
      .join('')
  })
}
function lock(colors, palette) {
  if (!palette) return colors
  const rgb = (v) => [0, 2, 4].map((i) => parseInt(v.slice(i, i + 2), 16))
  return colors.map((c) => {
    const a = rgb(c)
    return palette.reduce((best, b) => {
      const dist = (v) => rgb(v).reduce((n, x, i) => n + (x - a[i]) ** 2, 0)
      return dist(b) < dist(best) ? b : best
    })
  })
}
export function fromPrompt(prompt, seed) {
  if (typeof prompt !== 'string' || !prompt.trim() || prompt.length > 512)
    throw new Error('prompt must contain 1..512 characters')
  const lower = prompt.toLowerCase()
  const matches = ALIASES.filter(([, re]) => re.test(lower)).map(([form]) => form)
  const element = Object.keys(RAMPS).find((e) => new RegExp(`\\b${e}\\b`).test(lower))
  if (!matches.length && !element)
    throw new Error(`No recognized effect or element. Forms: ${Object.keys(FORMS).join(', ')}`)
  const form = matches[0] ?? 'explosion',
    formTypes = FORMS[form]
  const looping =
    [
      'beam',
      'projectile',
      'aura',
      'portal',
      'rain',
      'snow',
      'smoke',
      'shield',
      'tornado',
      'orb',
      'circle',
      'speed',
    ].includes(form) || /\bloop\b/.test(lower)
  const main =
    element ??
    { heal: 'nature', slash: 'metal', shield: 'ice', rain: 'water', snow: 'frost', smoke: 'smoke' }[
      form
    ] ??
    'fire'
  const size = /\bhuge\b|\bbig\b/.test(lower) ? 96 : /\btiny\b|\bsmall\b/.test(lower) ? 32 : 64
  const layers = formTypes.map((type) => ({ type, ramp: type === 'puffs' ? 'smoke' : main }))
  for (const layer of layers) {
    if (layer.type === 'particles')
      Object.assign(layer, {
        count: form === 'snow' ? 24 : 16,
        size: form === 'snow' ? 0.035 : 0.025,
        speed: 0.8,
        mode: ['heal', 'level-up', 'aura'].includes(form)
          ? 'rise'
          : ['snow', 'rain'].includes(form)
            ? 'fall'
            : form === 'projectile'
              ? 'trail'
              : 'burst',
      })
    if (layer.type === 'puffs')
      Object.assign(layer, { count: 7, size: 0.18, speed: 0.65, gain: 0.6, t0: looping ? 0 : 0.2 })
    if (layer.type === 'ring')
      Object.assign(layer, {
        r0: 0.08,
        r1: 0.8,
        th: 0.065,
        squash: ['heal', 'slam', 'level-up'].includes(form) ? 0.35 : 1,
      })
    if (layer.type === 'beam') layer.th = 0.15
    if (layer.type === 'orb') layer.r1 = form === 'beam' ? 0.18 : 0.5
    if (layer.type === 'glyph') layer.r1 = 0.67
    if (layer.type === 'text') layer.text = lower.match(/\d{1,8}/)?.[0] ?? '+100'
    if (layer.type === 'star') layer.count = 7
  }
  const hue = /\bpurple\b/.test(lower) ? 55 : /\bblue\b/.test(lower) && main === 'fire' ? 180 : 0
  return {
    name: prompt.trim().slice(0, 160),
    w: size,
    h: size,
    frames: looping ? 24 : 16,
    fps: /\bfast\b/.test(lower) ? 24 : 15,
    loop: looping,
    seed: seed ?? createHash('sha256').update(prompt).digest().readUInt32LE(0),
    hue,
    layers,
  }
}

export function normalize(input) {
  const recipe = object(input, 'recipe')
  keys(
    recipe,
    [
      'name',
      'w',
      'h',
      'frames',
      'fps',
      'loop',
      'seed',
      'outline',
      'dither',
      'clean',
      'palette',
      'hue',
      'pivot',
      'ramps',
      'layers',
    ],
    'recipe',
  )
  const r = {
    name: recipe.name ?? 'effect',
    w: num(recipe.w, 64, 8, 256, 'w', true),
    h: num(recipe.h, 64, 8, 256, 'h', true),
    frames: num(recipe.frames, 16, 2, 64, 'frames', true),
    fps: num(recipe.fps, 15, 1, 60, 'fps', true),
    loop: bool(recipe.loop, false, 'loop'),
    seed: num(recipe.seed, 1, 0, 4294967295, 'seed', true),
    outline: recipe.outline ?? 'none',
    dither: num(recipe.dither, 0, 0, 1, 'dither'),
    clean: bool(recipe.clean, false, 'clean'),
    palette: recipe.palette ?? null,
    hue: num(recipe.hue, 0, -360, 360, 'hue'),
    pivot: recipe.pivot ?? [0.5, 0.5],
    layers: [],
  }
  if (typeof r.name !== 'string' || r.name.length > 160)
    throw new Error('name must be a string up to 160 characters')
  if (r.w * r.h * r.frames > 1048576)
    throw new Error('A render may contain at most 1,048,576 pixels across all frames')
  if (!['none', 'outer', 'inner'].includes(r.outline))
    throw new Error('outline must be none, outer or inner')
  if (r.palette !== null && !own(PALETTES, r.palette))
    throw new Error(`palette must be ${Object.keys(PALETTES).join(', ')}`)
  if (!Array.isArray(r.pivot) || r.pivot.length !== 2) throw new Error('pivot must be [x,y]')
  r.pivot = r.pivot.map((x, i) => num(x, 0.5, 0, 1, `pivot[${i}]`))
  if (!Array.isArray(recipe.layers) || !recipe.layers.length || recipe.layers.length > 24)
    throw new Error('layers must contain 1..24 layers')
  const ramps = recipe.ramps === undefined ? {} : object(recipe.ramps, 'ramps')
  const fields = [
    'type',
    'ramp',
    't0',
    't1',
    'x',
    'y',
    'r0',
    'r1',
    'th',
    'angle',
    'squash',
    'gain',
    'speed',
    'spin',
    'count',
    'size',
    'life',
    'mode',
    'gravity',
    'text',
  ]
  for (const [i, raw] of recipe.layers.entries()) {
    object(raw, `layers[${i}]`)
    keys(raw, fields, 'layer')
    if (!TYPES.includes(raw.type)) throw new Error(`Unknown layer type: ${raw.type}`)
    const l = { type: raw.type, ramp: ramp(raw.ramp ?? 'fire', ramps) }
    const defaults = {
      t0: 0,
      t1: 1,
      x: 0,
      y: 0,
      r0: 0.08,
      r1: 0.75,
      th: 0.12,
      angle: 0,
      squash: 1,
      gain: 1,
      speed: 1,
      spin: 1,
      count: 12,
      size: 0.045,
      life: 1,
      gravity: 0.5,
    }
    if (raw.type === 'puffs') Object.assign(defaults, { size: 0.18, count: 7, gain: 0.65 })
    if (raw.type === 'text') defaults.size = 0.06
    const bounds = {
      t0: [0, 0.99],
      t1: [0.01, 1],
      x: [-2, 2],
      y: [-2, 2],
      r0: [0, 2],
      r1: [0.01, 2],
      th: [0.008, 1],
      angle: [-360, 360],
      squash: [0.05, 4],
      gain: [0, 2],
      speed: [0, 8],
      spin: [-8, 8],
      count: [1, 64],
      size: [0.008, 0.5],
      life: [0.05, 1],
      gravity: [-4, 4],
    }
    for (const [k, d] of Object.entries(defaults))
      l[k] = num(raw[k], d, ...bounds[k], k, k === 'count')
    if (l.t1 <= l.t0) throw new Error('Layer t1 must be greater than t0')
    if (r.loop && (!Number.isInteger(l.speed) || !Number.isInteger(l.spin))) {
      // Particle travel is faded at its wrap; analytic flame/vortex periods need whole cycles.
      if (['flame', 'vortex', 'glyph', 'rays', 'tornado'].includes(l.type))
        throw new Error('Looping analytic layers need integer speed and spin')
    }
    l.mode = raw.mode ?? 'burst'
    if (!MODES.includes(l.mode)) throw new Error(`mode must be ${MODES.join(', ')}`)
    l.text = raw.text ?? '+100'
    if (
      typeof l.text !== 'string' ||
      !l.text.length ||
      l.text.length > 8 ||
      [...l.text].some((c) => !own(FONT, c))
    )
      throw new Error('text supports 1..8 digits and + - ! space')
    if (l.type === 'text') l.r0 = l.text.length * l.size * 2
    r.layers.push(l)
  }
  return r
}

export function wire(r) {
  const lines = [
    r.w,
    r.h,
    r.frames,
    r.seed,
    Number(r.loop),
    ['none', 'outer', 'inner'].indexOf(r.outline),
    r.dither,
    Number(r.clean),
    r.layers.length,
  ]
  for (const l of r.layers) {
    const colors = lock(rotate(l.ramp, r.hue), r.palette ? PALETTES[r.palette] : null)
    lines.push(
      TYPES.indexOf(l.type),
      colors
        .map((c) => {
          // Bend packs R in the low byte, then G, then B, then alpha.
          return parseInt(c.slice(4, 6) + c.slice(2, 4) + c.slice(0, 2), 16)
        })
        .join(','),
      [
        l.t0,
        l.t1,
        l.x,
        l.y,
        l.r0,
        l.r1,
        l.th,
        l.angle,
        l.squash,
        l.gain,
        l.speed,
        l.spin,
        l.count,
        l.size,
        l.life,
        MODES.indexOf(l.mode),
        l.gravity,
      ].join(','),
      [...l.text]
        .map((c) =>
          FONT[c]
            .join('')
            .split('')
            .reduce((n, b, i) => n + (b === '1' ? 2 ** i : 0), 0),
        )
        .join(','),
    )
  }
  return lines.join('\n')
}

export function catalog() {
  return {
    engine: 'angelX VFX / Bend 2',
    version: '1.0.0',
    layers: TYPES,
    forms: Object.keys(FORMS),
    ramps: Object.keys(RAMPS),
    palettes: Object.keys(PALETTES),
    modes: MODES,
    limits: { width: 256, height: 256, frames: 64, layers: 24, totalPixels: 1048576 },
    exports: ['png', 'gif', 'atlas.json', 'recipe.json', 'godot.tres', 'zip'],
    example: fromPrompt('holy sword slash', 7),
  }
}

export const RENDER_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    prompt: {
      type: 'string',
      minLength: 1,
      maxLength: 512,
      description: 'Effect words; use vfx_catalog for the supported vocabulary.',
    },
    recipe: {
      type: 'object',
      description: 'Layered recipe; see the cartridge README for the independent v1 schema.',
    },
    out: {
      type: 'string',
      description:
        'New output folder relative to the workspace; existing folders are never overwritten.',
    },
    seed: { type: 'integer', minimum: 0, maximum: 4294967295 },
    scale: {
      type: 'integer',
      minimum: 1,
      maximum: 8,
      description: 'Integer GIF preview scale; PNGs stay at native resolution.',
    },
    frames: { type: 'boolean', description: 'Include individual frame PNGs.' },
  },
  oneOf: [
    { required: ['prompt'], not: { required: ['recipe'] } },
    { required: ['recipe'], not: { required: ['prompt'] } },
  ],
}

export function request(value) {
  const v = object(value, 'request')
  keys(v, Object.keys(RENDER_SCHEMA.properties), 'request')
  if (own(v, 'prompt') === own(v, 'recipe'))
    throw new Error('Supply exactly one of prompt or recipe')
  if (v.out !== undefined && (typeof v.out !== 'string' || !v.out.trim()))
    throw new Error('out must be a nonempty path')
  if (v.seed !== undefined) num(v.seed, 0, 0, 4294967295, 'seed', true)
  const recipe = own(v, 'prompt') ? fromPrompt(v.prompt, v.seed) : { ...object(v.recipe, 'recipe') }
  if (v.seed !== undefined) recipe.seed = v.seed
  const normalized = normalize(recipe),
    scale = num(v.scale, 4, 1, 8, 'scale', true)
  if (normalized.w * normalized.h * normalized.frames * scale * scale > 16777216)
    throw new Error('GIF preview exceeds 16,777,216 pixels; lower scale or render size')
  return { recipe: normalized, out: v.out, scale, frames: bool(v.frames, false, 'frames') }
}
