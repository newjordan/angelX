// Synchronous text persistence for local agent stores. The Python adapter owns
// nofollow file publication, permissions, and the canonical scanner rule.
import fs from 'node:fs'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
export * from 'node:fs'

const bridge = fileURLToPath(new URL('./private-store-bridge.py', import.meta.url))
function call(request) {
  const result = spawnSync('python3', [bridge], {
    input: JSON.stringify(request),
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  })
  if (result.error || result.status !== 0) {
    throw new Error('Private store write refused; unsafe path, invalid text, or sealed credentials')
  }
  return result.stdout
}
export function redactText(body) {
  return call({ operation: 'redact', text: body })
}
export function assertCleanText(body) {
  if (redactText(body) !== body) throw new Error('Exact-body evidence contains credentials')
  return body
}
function text(data, options) {
  const encoding = typeof options === 'string' ? options : options?.encoding || 'utf8'
  if (!['utf8', 'utf-8'].includes(encoding))
    throw new TypeError('Private text writes require UTF-8')
  if (typeof data === 'string') return data
  return new TextDecoder('utf-8', { fatal: true }).decode(data)
}
export function writeFileSync(path, data, options) {
  if (typeof data !== 'string' && typeof path !== 'number') {
    const flag = typeof options === 'object' ? options?.flag : undefined
    if (flag && flag !== 'w') throw new TypeError('Binary private writes require replacement')
    call({
      operation: 'replace-bytes',
      path: filePath(path),
      body: Buffer.from(data).toString('base64'),
      executable: Boolean(options?.mode & 0o100),
    })
    return
  }
  const body = text(data, options)
  if (typeof path === 'number') {
    const clean = call({ operation: 'redact', text: body })
    const info = fs.fstatSync(path)
    if (!info.isFile() || info.nlink !== 1 || info.uid !== process.getuid()) {
      throw new Error('Private file descriptor must identify a singly linked owner file')
    }
    fs.fchmodSync(path, 0o600)
    return fs.writeFileSync(path, clean, options)
  }
  const flag = typeof options === 'object' ? options?.flag : undefined
  if (flag && !['w', 'a'].includes(flag)) throw new TypeError('Unsupported private write flag')
  call({
    operation: flag === 'a' ? 'append' : 'replace',
    path: filePath(path),
    text: body,
    executable: Boolean(options?.mode & 0o100),
  })
}
function filePath(path) {
  return path instanceof URL ? fileURLToPath(path) : String(path)
}
export function appendFileSync(path, data, options) {
  if (typeof path === 'number') return writeFileSync(path, data, options)
  call({ operation: 'append', path: filePath(path), text: text(data, options) })
}
export function copyFileSync(source, target, flags = 0) {
  if (flags) throw new TypeError('Unsupported private text copy flags')
  call({
    operation: 'copy',
    source: filePath(source),
    path: filePath(target),
    executable: Boolean(fs.statSync(source).mode & 0o100),
  })
}
export function mkdirSync(path, options) {
  return fs.mkdirSync(path, { ...(typeof options === 'object' ? options : {}), mode: 0o700 })
}
export default { ...fs, writeFileSync, appendFileSync, copyFileSync, mkdirSync }
