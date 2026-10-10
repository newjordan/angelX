#!/usr/bin/env node
// Checkout-only publisher. Linux descriptor-relative opens prevent symlink
// swaps between validation and use. Never scans, admits, rewrites reports, or
// imports a worker into the pure manifest library.
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { randomBytes } from 'node:crypto'
import { assertCleanText } from './private-store-fs.mjs'
import { encodeManifest, manifestPath, MANIFEST_LIMIT, SOURCE_LIMIT } from '../../lib/settlement/exhibits.mjs'

const C = fs.constants
const flags = C.O_RDONLY | C.O_NOFOLLOW | C.O_NONBLOCK
const at = (fd, name) => `/proc/self/fd/${fd}/${name}`
const text = (body) => new TextDecoder('utf-8', { fatal: true }).decode(body)

function readBounded(fd, cap) {
  const body = Buffer.alloc(cap + 1)
  let used = 0
  while (used < body.length) {
    const n = fs.readSync(fd, body, used, body.length - used, null)
    if (!n) break
    used += n
  }
  if (used > cap) throw new Error('Input exceeds its byte limit')
  return body.subarray(0, used)
}

function directory(parent, name, create = false) {
  const target = at(parent, name)
  if (create) {
    try { fs.mkdirSync(target, { mode: 0o700 }) }
    catch (error) { if (error.code !== 'EEXIST') throw error }
  }
  const fd = fs.openSync(target, flags | C.O_DIRECTORY)
  const info = fs.fstatSync(fd)
  if (create && (info.uid !== process.geteuid() || (info.mode & 0o022))) {
    fs.closeSync(fd)
    throw new Error('Manifest directory must be owner-controlled and not group/other writable')
  }
  return fd
}

function workspace(root) {
  if (process.platform !== 'linux' || !path.isAbsolute(root) || root.split('/').includes('..'))
    throw new Error('Publisher requires Linux and an absolute workspace without traversal')
  let fd = fs.openSync('/', flags | C.O_DIRECTORY)
  try {
    for (const part of root.split('/').filter((p) => p && p !== '.')) {
      const child = directory(fd, part)
      fs.closeSync(fd)
      fd = child
    }
    if (fs.fstatSync(fd).uid !== process.geteuid()) throw new Error('Workspace belongs to another owner')
    return fd
  } catch (error) { fs.closeSync(fd); throw error }
}

function readFile(root, relative, cap) {
  const parts = relative.split('/')
  const dirs = []
  let parent = root
  let fd
  try {
    for (const part of parts.slice(0, -1)) {
      parent = directory(parent, part)
      dirs.push(parent)
    }
    fd = fs.openSync(at(parent, parts.at(-1)), flags)
    const info = fs.fstatSync(fd)
    if (!info.isFile() || info.uid !== process.geteuid() || info.nlink !== 1 || info.size > cap)
      throw new Error('Expected a bounded, singly linked owner file')
    return readBounded(fd, cap)
  } finally {
    if (fd !== undefined) fs.closeSync(fd)
    for (const dir of dirs.reverse()) fs.closeSync(dir)
  }
}

function publish(directoryFd, name, body) {
  const destination = `${name}.json`
  // Stage complete bytes, then hard-link as an atomic no-replace operation.
  // Remove the staging name before returning: native admission rejects nlink>1.
  // A crash between link/unlink fails closed (operator must remove the temp link).
  const temporary = `.${name}.${randomBytes(12).toString('hex')}.tmp`
  const fd = fs.openSync(at(directoryFd, temporary), C.O_WRONLY | C.O_CREAT | C.O_EXCL | C.O_NOFOLLOW, 0o600)
  let staged = true
  try {
    fs.writeFileSync(fd, body, 'utf8')
    fs.fsyncSync(fd)
    try { fs.linkSync(at(directoryFd, temporary), at(directoryFd, destination)) }
    catch (error) {
      if (error.code !== 'EEXIST') throw error
      if (!readFile(directoryFd, destination, MANIFEST_LIMIT).equals(Buffer.from(body)))
        throw new Error('Named manifest already exists with different bytes; choose a new name')
      return false
    }
    fs.unlinkSync(at(directoryFd, temporary))
    staged = false
    fs.fsyncSync(directoryFd)
    return true
  } finally {
    try { if (staged) fs.unlinkSync(at(directoryFd, temporary)) }
    finally { fs.closeSync(fd) }
  }
}

export function publishManifest({ root, name, manifest }) {
  const relative = manifestPath(name)
  assertCleanText(name)
  const body = encodeManifest(manifest)
  assertCleanText(body) // refuse credentials rather than change integrity-bound metadata
  const rootFd = workspace(root)
  let store, queue
  try {
    for (const artifact of manifest.artifacts)
      assertCleanText(text(readFile(rootFd, artifact.source, SOURCE_LIMIT)))
    // Validate sources before creating anything. Existing directory modes stay unchanged.
    store = directory(rootFd, '.angelX', true)
    queue = directory(store, 'settlement-exhibits', true)
    fs.fsyncSync(store)
    fs.fsyncSync(rootFd)
    const created = publish(queue, name, body)
    return { created, manifest: relative, command: `/dungeon deposit ${name}`,
      authority: 'Published only; host admission after a saved loop receipt is still required.' }
  } finally {
    if (queue !== undefined) fs.closeSync(queue)
    if (store !== undefined) fs.closeSync(store)
    fs.closeSync(rootFd)
  }
}

function main() {
  const [verb, root, name, ...rest] = process.argv.slice(2)
  if (verb === '--help' && !root) {
    console.log('Usage: node scripts/runtime/settlement-exhibits.mjs publish <absolute-workspace> <name> < manifest.json\nLinux checkout helper; immutable explicit publication, not admission or verification.')
    return
  }
  if (verb !== 'publish' || !root || !name || rest.length)
    throw new Error('Use --help for publisher usage')
  manifestPath(name)
  const manifest = JSON.parse(text(readBounded(0, MANIFEST_LIMIT)))
  console.log(JSON.stringify(publishManifest({ root, name, manifest })))
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { main() }
  catch {
    // Do not echo user data, source paths/content, or credential-bearing parse errors.
    console.error('Research publication refused: check name, manifest, source safety, credentials and existing destination. No admission performed.')
    process.exitCode = 1
  }
}
