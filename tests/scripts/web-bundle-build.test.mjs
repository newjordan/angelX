import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const source = (path) => readFileSync(new URL(`../../${path}`, import.meta.url), 'utf8')

test('the browser bundle has an independent Cargo fingerprint and object file', () => {
  assert.match(source('cockpit/Cargo.toml'), /angel-web-bundle\s*=\s*\{\s*path\s*=\s*"web-bundle"/u)
  assert.doesNotMatch(source('cockpit/build.rs'), /fn delve_web|fn renderers|fn wasm_cargo|WASM_BINDGEN/u)
  assert.doesNotMatch(source('cockpit/src/drive/together_web.rs'), /include!\(concat!\(env!\("OUT_DIR"\)/u)
  assert.match(source('cockpit/src/drive/together_web.rs'), /angel_web_bundle/u)
  assert.match(source('cockpit/web-bundle/lib.rs'), /include!\(concat!\(env!\("OUT_DIR"\)/u)
  assert.match(source('cockpit/web-bundle/build.rs'), /fn delve_web/u)
})

test('browser packaging retains shared build policies and all normal asset inputs', () => {
  const build = source('cockpit/web-bundle/build.rs')
  assert.match(build, /path = "\.\.\/build_fast.rs"/u)
  assert.match(build, /path = "\.\.\/bundle_bytes.rs"/u)
  assert.match(build, /skip_web_bundle/u)
  assert.match(build, /nested_web_profile/u)
  for (const input of ['WASM_BINDGEN', 'ANGEL_DEV_SKIP_WEB_BUNDLE', 'ANGEL_PORTABLE_BUILD', 'RA_JIT_SCRIPT_RUSTC', 'RUSTUP_TOOLCHAIN']) {
    assert.ok(build.includes(`cargo:rerun-if-env-changed=${input}`), input)
  }
  for (const input of ['web.join("shell")', 'crates.join("delve").join("src")', 'crates.join("gpu")']) {
    assert.ok(build.includes(input), input)
  }
  assert.doesNotMatch(build, /rerun-if-env-changed=HOME|rerun-if-env-changed=ANGEL_CARTRIDGES/u)
})


test('generated statics stay private behind non-inlined accessors', () => {
  const build = source('cockpit/web-bundle/build.rs')
  assert.doesNotMatch(build, /pub static (?:BUNDLE|PACKED|SOUNDS|BESIDE)/u)
  const library = source('cockpit/web-bundle/lib.rs')
  assert.doesNotMatch(build, /static BUNDLE/u)
  // Both the full and intentionally empty bundle use the same non-inlined API.
  assert.equal((build.match(/pub fn embedded\(\)/gu) ?? []).length, 2)
  assert.equal((build.match(/#\[inline\(never\)\]/gu) ?? []).length, 2)
  for (const accessor of ['packed', 'sounds', 'beside']) {
    assert.match(library, new RegExp(`#\\[inline\\(never\\)\\]\\s*pub fn ${accessor}\\(`, 'u'))
  }
})
