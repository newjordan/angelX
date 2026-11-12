#!/usr/bin/env node
// Build and atomically publish executable systems, independently of the cockpit.
import { spawn, spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { chmodSync, copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, renameSync, statSync, unlinkSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const args = process.argv.slice(2)
const watching = args.includes('--watch')
if (args.some(arg => !['delve', '--watch'].includes(arg))) {
  console.error('usage: node scripts/dev/systems.mjs [delve] [--watch]')
  process.exit(2)
}
const channel = readFileSync(join(root, 'rust-toolchain.toml'), 'utf8').match(/^channel\s*=\s*"([^"]+)"/m)[1]
const env = { ...process.env, RUSTUP_TOOLCHAIN: channel, RUSTUP_AUTO_INSTALL: '0' }
let cargo = 'cargo'
for (const [tool, variable] of [['cargo', null], ['rustc', 'RUSTC'], ['rustdoc', 'RUSTDOC']]) {
  const found = spawnSync('rustup', ['which', '--toolchain', channel, tool], { encoding: 'utf8', env })
  if (found.status !== 0) throw new Error(`The repository requires the installed Rust ${channel} toolchain: ${found.stderr}`)
  if (variable) env[variable] = found.stdout.trim()
  else cargo = found.stdout.trim()
}
env.CARGO_TARGET_DIR = resolve(env.CARGO_TARGET_DIR || join(root, 'crates/target/systems'))
const output = resolve(env.ANGEL_DELVE_SYSTEM || join(root, 'cockpit/systems/delve/engine'))
mkdirSync(dirname(output), { recursive: true })
let child
let stopping = false
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => { stopping = true; child?.kill(signal) })

function fingerprint() {
  const hash = createHash('sha256')
  const walk = path => {
    if (!existsSync(path)) return
    for (const entry of readdirSync(path, { withFileTypes: true }).sort((a,b) => a.name.localeCompare(b.name))) {
      if (entry.name.startsWith('target') || entry.name === 'node_modules') continue
      const file = join(path, entry.name)
      if (entry.isDirectory()) walk(file)
      else if (entry.isFile() && /\.(rs|toml|json|txt|card|art)$/.test(entry.name)) {
        const stat = statSync(file)
        hash.update(`${file}\0${stat.size}\0${stat.mtimeMs}\0`)
      }
    }
  }
  for (const dir of ['crates/delve', 'crates/kingdom', 'delve/kingdom', 'crates/delve-system', 'crates/live', 'cockpit/assets/dungeon', 'cockpit/assets/kingdom']) walk(join(root, dir))
  for (const file of ['crates/Cargo.toml', 'crates/Cargo.lock', 'rust-toolchain.toml']) hash.update(readFileSync(join(root,file)))
  return hash.digest('hex')
}
async function build() {
  const started = performance.now()
  const status = await new Promise((resolve, reject) => {
    child = spawn(cargo, ['build', '--locked', '--manifest-path', join(root,'crates/Cargo.toml'), '-p', 'angel-delve-system', '--profile', 'fast'], { cwd: root, env, stdio: 'inherit' })
    child.once('error', reject)
    child.once('exit', (code, signal) => resolve(signal ? 1 : code))
  })
  child = null
  if (status !== 0 || stopping) return false
  const built = join(env.CARGO_TARGET_DIR, 'fast', `angel-delve-system${process.platform === 'win32' ? '.exe' : ''}`)
  const pending = `${output}.${process.pid}.pending`
  try {
    copyFileSync(built, pending)
    chmodSync(pending, 0o700)
    renameSync(pending, output)
  } finally {
    if (existsSync(pending)) unlinkSync(pending)
  }
  console.error(`Delve implementation published in ${((performance.now()-started)/1000).toFixed(2)}s: ${output}`)
  return true
}
let seen
do {
  const current = fingerprint()
  if (current !== seen) {
    seen = current
    const ok = await build()
    if (!watching) process.exitCode = ok ? 0 : 1
  }
  if (watching && !stopping) await new Promise(resolve => setTimeout(resolve, 400))
} while (watching && !stopping)
