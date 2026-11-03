// SPDX-License-Identifier: MIT
import { deflateSync } from 'node:zlib'

const CRC = Uint32Array.from({ length: 256 }, (_, i) => {
  let c = i
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
  return c >>> 0
})
export function crc32(bytes) {
  let c = 0xffffffff
  for (const b of bytes) c = CRC[(c ^ b) & 255] ^ (c >>> 8)
  return (c ^ 0xffffffff) >>> 0
}
function chunk(type, data) {
  const t = Buffer.from(type),
    out = Buffer.alloc(12 + data.length)
  out.writeUInt32BE(data.length)
  t.copy(out, 4)
  data.copy(out, 8)
  out.writeUInt32BE(crc32(out.subarray(4, 8 + data.length)), 8 + data.length)
  return out
}
export function png(w, h, rgba) {
  if (rgba.length !== w * h * 4) throw new Error('RGBA buffer length does not match PNG dimensions')
  const header = Buffer.alloc(13)
  header.writeUInt32BE(w)
  header.writeUInt32BE(h, 4)
  header[8] = 8
  header[9] = 6
  const scan = Buffer.alloc((w * 4 + 1) * h)
  for (let y = 0; y < h; y++) rgba.copy(scan, y * (w * 4 + 1) + 1, y * w * 4, (y + 1) * w * 4)
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk('IHDR', header),
    chunk('IDAT', deflateSync(scan)),
    chunk('IEND', Buffer.alloc(0)),
  ])
}
function gifPalette(frames) {
  const counts = new Map()
  for (const frame of frames)
    for (let i = 0; i < frame.length; i += 4)
      if (frame[i + 3]) {
        const key = frame[i] | (frame[i + 1] << 8) | (frame[i + 2] << 16)
        counts.set(key, (counts.get(key) || 0) + 1)
      }
  const colors = [
    0,
    ...[...counts]
      .sort((a, b) => b[1] - a[1])
      .slice(0, 255)
      .map(([c]) => c),
  ]
  const table = Buffer.alloc(768),
    lookup = new Map()
  colors.forEach((c, i) => {
    table[i * 3] = c & 255
    table[i * 3 + 1] = (c >>> 8) & 255
    table[i * 3 + 2] = (c >>> 16) & 255
    if (i) lookup.set(c, i)
  })
  for (const c of counts.keys())
    if (!lookup.has(c)) {
      let best = 1,
        distance = Infinity
      for (let i = 1; i < colors.length; i++) {
        const b = colors[i],
          d =
            ((c & 255) - (b & 255)) ** 2 +
            (((c >>> 8) & 255) - ((b >>> 8) & 255)) ** 2 +
            ((c >>> 16) - (b >>> 16)) ** 2
        if (d < distance) {
          distance = d
          best = i
        }
      }
      lookup.set(c, best)
    }
  return { table, lookup }
}
// Reset the LZW dictionary every 200 literals. Codes stay 9 bits, including
// the decoder's dictionary growth. Larger than adaptive LZW, simple and valid.
function literalLzw(indices) {
  const bytes = [],
    codes = []
  for (let i = 0; i < indices.length; i += 200) {
    codes.push(256)
    for (let j = i; j < Math.min(i + 200, indices.length); j++) codes.push(indices[j])
  }
  codes.push(257)
  let bits = 0,
    n = 0
  for (const code of codes) {
    bits |= code << n
    n += 9
    while (n >= 8) {
      bytes.push(bits & 255)
      bits >>>= 8
      n -= 8
    }
  }
  if (n) bytes.push(bits & 255)
  const blocks = [Buffer.from([8])]
  for (let i = 0; i < bytes.length; i += 255) {
    const b = bytes.slice(i, i + 255)
    blocks.push(Buffer.from([b.length, ...b]))
  }
  blocks.push(Buffer.from([0]))
  return Buffer.concat(blocks)
}
export function gif(w, h, frames, fps, loop, scale = 1) {
  const { table, lookup } = gifPalette(frames),
    W = w * scale,
    H = h * scale
  const header = Buffer.alloc(13)
  header.write('GIF89a')
  header.writeUInt16LE(W, 6)
  header.writeUInt16LE(H, 8)
  header[10] = 0xf7
  const chunks = [header, table]
  if (loop) chunks.push(Buffer.from([0x21, 0xff, 11, ...Buffer.from('NETSCAPE2.0'), 3, 1, 0, 0, 0]))
  for (const f of frames) {
    const control = Buffer.from([0x21, 0xf9, 4, 9, 0, 0, 0, 0])
    control.writeUInt16LE(Math.max(1, Math.round(100 / fps)), 4)
    const image = Buffer.alloc(10)
    image[0] = 0x2c
    image.writeUInt16LE(W, 5)
    image.writeUInt16LE(H, 7)
    const indices = new Uint8Array(W * H)
    for (let y = 0; y < H; y++)
      for (let x = 0; x < W; x++) {
        const i = (Math.floor(y / scale) * w + Math.floor(x / scale)) * 4
        if (f[i + 3]) indices[y * W + x] = lookup.get(f[i] | (f[i + 1] << 8) | (f[i + 2] << 16))
      }
    chunks.push(control, image, literalLzw(indices))
  }
  chunks.push(Buffer.from([0x3b]))
  return Buffer.concat(chunks)
}
export function zip(files) {
  const parts = [],
    directory = []
  let offset = 0
  for (const [name, data] of files) {
    const n = Buffer.from(name),
      crc = crc32(data),
      local = Buffer.alloc(30),
      central = Buffer.alloc(46)
    local.writeUInt32LE(0x04034b50)
    local.writeUInt16LE(20, 4)
    local.writeUInt16LE(0x800, 6)
    local.writeUInt16LE(33, 12)
    local.writeUInt32LE(crc, 14)
    local.writeUInt32LE(data.length, 18)
    local.writeUInt32LE(data.length, 22)
    local.writeUInt16LE(n.length, 26)
    central.writeUInt32LE(0x02014b50)
    central.writeUInt16LE(20, 4)
    central.writeUInt16LE(20, 6)
    central.writeUInt16LE(0x800, 8)
    central.writeUInt16LE(33, 14)
    central.writeUInt32LE(crc, 16)
    central.writeUInt32LE(data.length, 20)
    central.writeUInt32LE(data.length, 24)
    central.writeUInt16LE(n.length, 28)
    central.writeUInt32LE(offset, 42)
    parts.push(local, n, data)
    directory.push(central, n)
    offset += local.length + n.length + data.length
  }
  const central = Buffer.concat(directory),
    end = Buffer.alloc(22)
  end.writeUInt32LE(0x06054b50)
  end.writeUInt16LE(files.size, 8)
  end.writeUInt16LE(files.size, 10)
  end.writeUInt32LE(central.length, 12)
  end.writeUInt32LE(offset, 16)
  return Buffer.concat([...parts, central, end])
}
export function sheet(w, h, frames) {
  const cols = Math.min(frames.length, Math.floor(4096 / w)),
    rows = Math.ceil(frames.length / cols),
    W = w * cols,
    H = h * rows,
    out = Buffer.alloc(W * H * 4)
  frames.forEach((f, i) => {
    for (let y = 0; y < h; y++)
      f.copy(
        out,
        ((Math.floor(i / cols) * h + y) * W + (i % cols) * w) * 4,
        y * w * 4,
        (y + 1) * w * 4,
      )
  })
  return { cols, w: W, h: H, rgba: out }
}
export function godot(r, stem, cols) {
  let out = `[gd_resource type="SpriteFrames" load_steps=${r.frames + 2} format=3]\n\n[ext_resource type="Texture2D" path="${stem}.png" id="1"]\n`
  for (let i = 0; i < r.frames; i++)
    out += `\n[sub_resource type="AtlasTexture" id="f${i}"]\natlas = ExtResource("1")\nregion = Rect2(${(i % cols) * r.w}, ${Math.floor(i / cols) * r.h}, ${r.w}, ${r.h})\n`
  return (
    out +
    `\n[resource]\nanimations = [{"frames": [${Array.from({ length: r.frames }, (_, i) => `{"duration": 1.0, "texture": SubResource("f${i}")}`).join(', ')}], "loop": ${r.loop}, "name": &"default", "speed": ${Number(r.fps).toFixed(1)}}]\n`
  )
}
