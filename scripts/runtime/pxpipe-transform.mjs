#!/usr/bin/env node
import { transformOpenAIChatCompletions, transformOpenAIResponses } from 'pxpipe-proxy'

function envFlag(name, fallback) {
  const raw = process.env[name]
  if (raw === undefined || raw.trim() === '') return fallback
  return !/^(0|false|no|off|none)$/i.test(raw.trim())
}

function envInt(name, fallback) {
  const raw = process.env[name]
  if (raw === undefined || raw.trim() === '') return fallback
  const n = Number.parseInt(raw, 10)
  return Number.isFinite(n) && n >= 0 ? n : fallback
}

function envFloat(name, fallback) {
  const raw = process.env[name]
  if (raw === undefined || raw.trim() === '') return fallback
  const n = Number.parseFloat(raw)
  return Number.isFinite(n) && n > 0 ? n : fallback
}

function transformOptions() {
  return {
    compress: envFlag('ANGEL_PXPIPE_COMPRESS', true),
    compressTools: envFlag('ANGEL_PXPIPE_COMPRESS_TOOLS', true),
    collapseHistory: envFlag('ANGEL_PXPIPE_COLLAPSE_HISTORY', true),
    minCompressChars: envInt('ANGEL_PXPIPE_MIN_CHARS', 2000),
    charsPerToken: envFloat('ANGEL_PXPIPE_CHARS_PER_TOKEN', 4),
    reflow: envFlag('ANGEL_PXPIPE_REFLOW', true),
  }
}

async function transformBody(api, body) {
  const input = new Uint8Array(body)
  return api === 'chat'
    ? await transformOpenAIChatCompletions(input, transformOptions())
    : await transformOpenAIResponses(input, transformOptions())
}

function debugLog(api, label, model, result) {
  if (!envFlag('ANGEL_PXPIPE_DEBUG', false)) return
  const info = result.info ?? {}
  console.error(
    JSON.stringify({
      mode: api,
      label,
      model,
      compressed: info.compressed ?? false,
      reason: info.reason,
      imageCount: info.imageCount ?? 0,
      compressedChars: info.compressedChars ?? 0,
    }),
  )
}

// One-shot mode (the original contract): body on stdin, transformed body on
// stdout, label/model via env. Kept byte-identical for callers that predate
// (or opt out of) serve mode.
async function oneShot(api) {
  const chunks = []
  for await (const chunk of process.stdin) {
    chunks.push(Buffer.from(chunk))
  }
  const body = Buffer.concat(chunks)
  const result = await transformBody(api, body)
  debugLog(api, process.env.ANGEL_PXPIPE_LABEL, process.env.ANGEL_PXPIPE_MODEL, result)
  process.stdout.write(Buffer.from(result.body))
}

// Persistent mode: framed requests on stdin, framed responses on stdout, one
// process for the whole cockpit session (a node spawn + import per request is
// 50-150ms that used to ride every SOTA call). Frame in:
//   {"api":"chat"|"responses","label":"…","model":"…","len":N}\n + N body bytes
// Frame out:
//   {"ok":true,"len":N}\n + N bytes   |   {"ok":false,"err":"…"}\n
// Requests are served strictly in order (the cockpit serializes them anyway).
async function serve() {
  let buf = Buffer.alloc(0)
  let header = null
  const respond = (obj, body) => {
    process.stdout.write(JSON.stringify(obj) + '\n')
    if (body) process.stdout.write(body)
  }
  for await (const chunk of process.stdin) {
    buf = Buffer.concat([buf, Buffer.from(chunk)])
    for (;;) {
      if (!header) {
        const nl = buf.indexOf(0x0a)
        if (nl < 0) break
        const line = buf.subarray(0, nl).toString('utf8')
        buf = buf.subarray(nl + 1)
        try {
          header = JSON.parse(line)
        } catch (e) {
          respond({ ok: false, err: `bad frame header: ${e.message}` })
          continue
        }
      }
      const len = header.len >>> 0
      if (buf.length < len) break
      const body = buf.subarray(0, len)
      buf = buf.subarray(len)
      const { api, label, model } = header
      header = null
      try {
        const result = await transformBody(api, body)
        const out = Buffer.from(result.body)
        debugLog(api, label, model, result)
        respond({ ok: true, len: out.length }, out)
      } catch (e) {
        respond({ ok: false, err: String((e && e.message) || e) })
      }
    }
  }
}

const mode = process.argv[2]
if (mode === 'serve') {
  await serve()
} else if (mode === 'chat' || mode === 'responses') {
  await oneShot(mode)
} else {
  console.error('usage: pxpipe-transform.mjs chat|responses|serve < request.json > request.json')
  process.exit(64)
}
