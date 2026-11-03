// SPDX-License-Identifier: MIT
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { readFile, writeFile, mkdir, mkdtemp, rm, stat, realpath, lstat } from 'node:fs/promises'
import { resolve, dirname, join, relative, isAbsolute } from 'node:path'
import { fileURLToPath } from 'node:url'
import { tmpdir } from 'node:os'
import { randomUUID, createHash } from 'node:crypto'
import { request, wire } from './recipes.mjs'
import { png, gif, sheet, godot, zip } from './codecs.mjs'

const exec = promisify(execFile)
export const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..')
export const BINARY = join(ROOT, '.build', 'vfx-render')
export async function doctor() {
  const built = await stat(BINARY)
    .then((s) => s.isFile())
    .catch(() => false)
  const source = createHash('sha256')
    .update(await readFile(join(ROOT, 'bend', 'engine.bend')))
    .digest('hex')
  const stamp = await readFile(join(ROOT, '.build', 'source.sha256'), 'utf8').catch(() => '')
  const ok = built && stamp.trim() === source
  return {
    ready: ok,
    engine: 'Bend 2.0.39 native CPU',
    binary: BINARY,
    node: process.versions.node,
    ...(ok
      ? {}
      : {
          fix: `Run ${join(ROOT, 'build.sh')} (requires clang and downloads the pinned Bend toolchain if needed).`,
        }),
  }
}
export async function render(
  args,
  { workspace = process.env.ANGEL_WORKSPACE || process.cwd() } = {},
) {
  const q = request(args),
    r = q.recipe,
    stem =
      r.name
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, '-')
        .replace(/^-|-$/g, '')
        .slice(0, 80) || 'effect'
  const root = await realpath(workspace),
    out = resolve(root, q.out ?? join('vfx-out', `${stem}-${randomUUID().slice(0, 8)}`)),
    rel = relative(root, out)
  if (
    !rel ||
    rel === '..' ||
    rel.startsWith(`..${process.platform === 'win32' ? '\\' : '/'}`) ||
    isAbsolute(rel)
  )
    throw new Error('out must be inside the workspace, in a new folder')
  if (!(await doctor()).ready)
    throw new Error(`Bend renderer is not built. Run ${join(ROOT, 'build.sh')}`)
  const work = await mkdtemp(join(tmpdir(), 'angel-vfx-')),
    start = performance.now()
  let reserved = false
  try {
    const input = join(work, 'scene.wire'),
      raw = join(work, 'frames.rgba')
    await writeFile(input, wire(r))
    await exec(BINARY, [input, raw, '+RTS', '--threads', '4', '-RTS'], {
      timeout: 60000,
      maxBuffer: 65536,
    })
    const rgba = await readFile(raw),
      size = r.w * r.h * 4
    if (rgba.length !== size * r.frames)
      throw new Error(`Bend returned ${rgba.length} bytes; expected ${size * r.frames}`)
    const frames = Array.from({ length: r.frames }, (_, i) =>
        rgba.subarray(i * size, (i + 1) * size),
      ),
      s = sheet(r.w, r.h, frames)
    const coverage = frames.map((f) => {
      let n = 0
      for (let i = 3; i < f.length; i += 4) if (f[i]) n++
      return n
    })
    let impact = 0,
      best = -1
    for (let i = 0; i < Math.ceil(r.frames * 0.6); i++) {
      const delta = coverage[i] - (coverage[i - 1] ?? 0)
      if (delta > best) {
        best = delta
        impact = i
      }
    }
    const atlas = {
      frames: frames.map((_, i) => ({
        filename: `${stem}_${String(i).padStart(2, '0')}`,
        frame: { x: (i % s.cols) * r.w, y: Math.floor(i / s.cols) * r.h, w: r.w, h: r.h },
        duration: Math.round(1000 / r.fps),
      })),
      meta: {
        app: 'angelX VFX',
        engine: 'Bend 2',
        version: 1,
        image: `${stem}.png`,
        size: { w: s.w, h: s.h },
        fps: r.fps,
        loop: r.loop,
        impactFrame: impact,
        pivot: r.pivot,
      },
    }
    const files = new Map(),
      put = (n, data) => files.set(n, Buffer.isBuffer(data) ? data : Buffer.from(data))
    put(`${stem}.png`, png(s.w, s.h, s.rgba))
    put(`${stem}@${q.scale}x.gif`, gif(r.w, r.h, frames, r.fps, r.loop, q.scale))
    put(`${stem}.json`, JSON.stringify(atlas, null, 2) + '\n')
    put(`${stem}.recipe.json`, JSON.stringify(r, null, 2) + '\n')
    put(`${stem}.tres`, godot(r, stem, s.cols))
    // The pack always contains individual frames; loose frames are optional.
    const pack = new Map(files)
    for (let i = 0; i < frames.length; i++)
      pack.set(`frames/${stem}_${String(i).padStart(2, '0')}.png`, png(r.w, r.h, frames[i]))
    pack.set(
      'README.txt',
      Buffer.from(
        'angelX VFX — generated locally by Bend 2.\nPNG pixels have binary alpha. Use nearest-neighbor filtering and disable texture compression.\nImport the .tres beside its PNG in Godot; use the JSON atlas in other engines.\nThe recipe can be rendered again with bin/vfx render <recipe.json>.\n',
      ),
    )
    if (q.frames) for (const [n, data] of pack) if (n.startsWith('frames/')) put(n, data)
    put(`${stem}.zip`, zip(pack))
    let parent = root
    for (const part of relative(root, dirname(out)).split('/').filter(Boolean)) {
      parent = join(parent, part)
      await mkdir(parent).catch((e) => {
        if (e.code !== 'EEXIST') throw e
      })
      const info = await lstat(parent)
      if (info.isSymbolicLink() || !info.isDirectory())
        throw new Error('Output parents must be real workspace directories')
    }
    await mkdir(out)
    reserved = true
    for (const [n, data] of files) {
      await mkdir(dirname(join(out, n)), { recursive: true })
      await writeFile(join(out, n), data, { flag: 'wx' })
    }
    return {
      name: r.name,
      engine: 'Bend 2 native',
      out,
      w: r.w,
      h: r.h,
      frames: r.frames,
      fps: r.fps,
      loop: r.loop,
      seed: r.seed,
      impactFrame: impact,
      elapsedMs: Math.round(performance.now() - start),
      files: [...files.keys()].map((n) => join(out, n)),
      coverage,
    }
  } catch (e) {
    if (reserved) await rm(out, { recursive: true, force: true })
    if (e.code === 'EEXIST')
      throw new Error(`Output folder already exists: ${out}. Choose a new folder.`)
    if (e.killed)
      throw new Error(
        'Bend render exceeded the 60 second limit; reduce dimensions, layers or particle counts',
      )
    throw e
  } finally {
    await rm(work, { recursive: true, force: true })
  }
}
