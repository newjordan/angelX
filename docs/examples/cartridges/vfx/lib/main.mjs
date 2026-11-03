// SPDX-License-Identifier: MIT
import { createServer } from 'node:http'
import { createInterface } from 'node:readline'
import { readFile } from 'node:fs/promises'
import { basename, resolve, relative } from 'node:path'
import { randomUUID } from 'node:crypto'
import { catalog, RENDER_SCHEMA } from './recipes.mjs'
import { render, doctor } from './render.mjs'

const MAX_INPUT = 262144
const TOOLS = [
  {
    name: 'vfx_catalog',
    description:
      'List local Bend pixel VFX layers, prompt vocabulary, palettes, limits and an example recipe.',
    inputSchema: { type: 'object', properties: {}, additionalProperties: false },
    annotations: { readOnlyHint: true },
  },
  {
    name: 'vfx_render',
    description:
      'Render pixel VFX locally with Bend. Write transparent PNG sheets, GIFs, atlas JSON, Godot SpriteFrames and a ZIP in a new workspace folder.',
    inputSchema: RENDER_SCHEMA,
    annotations: {
      readOnlyHint: false,
      destructiveHint: false,
      idempotentHint: false,
      openWorldHint: false,
    },
  },
]
function empty(args) {
  if (!args || typeof args !== 'object' || Array.isArray(args) || Object.keys(args).length)
    throw new Error('vfx_catalog takes an empty object')
}
export async function callTool(name, args, options) {
  if (name === 'vfx_catalog') {
    empty(args)
    return catalog()
  }
  if (name === 'vfx_render') return render(args, options)
  throw new Error(`Unknown tool: ${name}`)
}
export async function stdinJson() {
  const parts = []
  let size = 0
  for await (const part of process.stdin) {
    size += part.length
    if (size > MAX_INPUT) throw new Error('Input exceeds 256 KiB')
    parts.push(part)
  }
  return JSON.parse(Buffer.concat(parts).toString('utf8'))
}
export async function toolMain(name) {
  try {
    process.stdout.write(JSON.stringify(await callTool(name, await stdinJson())) + '\n')
  } catch (e) {
    process.stderr.write(`vfx: ${e.message}\n`)
    process.exitCode = 1
  }
}
export async function mcp({ input = process.stdin, output = process.stdout, ...options } = {}) {
  const lines = createInterface({ input, crlfDelay: Infinity })
  const send = (value) => output.write(JSON.stringify(value) + '\n')
  for await (const line of lines) {
    let msg
    try {
      if (Buffer.byteLength(line) > MAX_INPUT) throw new Error('Request exceeds 256 KiB')
      msg = JSON.parse(line)
    } catch {
      send({
        jsonrpc: '2.0',
        id: null,
        error: { code: -32700, message: 'Invalid JSON or request too large' },
      })
      continue
    }
    if (
      !msg ||
      typeof msg !== 'object' ||
      Array.isArray(msg) ||
      msg.jsonrpc !== '2.0' ||
      typeof msg.method !== 'string' ||
      (Object.hasOwn(msg, 'id') && msg.id !== null && !['string', 'number'].includes(typeof msg.id))
    ) {
      send({
        jsonrpc: '2.0',
        id: null,
        error: { code: -32600, message: 'Invalid JSON-RPC request' },
      })
      continue
    }
    if (!Object.hasOwn(msg, 'id')) continue
    const reply = (result) => send({ jsonrpc: '2.0', id: msg.id, result })
    const error = (code, message) => send({ jsonrpc: '2.0', id: msg.id, error: { code, message } })
    if (msg.method === 'initialize')
      reply({
        protocolVersion: '2024-11-05',
        capabilities: { tools: {} },
        serverInfo: { name: 'angel-vfx-bend', version: '1.0.0' },
      })
    else if (msg.method === 'ping') reply({})
    else if (msg.method === 'tools/list') reply({ tools: TOOLS })
    else if (msg.method === 'tools/call') {
      if (!msg.params || typeof msg.params.name !== 'string') {
        error(-32602, 'tools/call needs a tool name')
        continue
      }
      try {
        reply({
          content: [
            {
              type: 'text',
              text: JSON.stringify(
                await callTool(msg.params.name, msg.params.arguments ?? {}, options),
              ),
            },
          ],
        })
      } catch (e) {
        reply({ isError: true, content: [{ type: 'text', text: e.message }] })
      }
    } else error(-32601, `Unknown method: ${msg.method}`)
  }
}
async function body(req) {
  if (!/^application\/json(?:\s*;|$)/i.test(req.headers['content-type'] ?? ''))
    throw Object.assign(new Error('Content-Type must be application/json'), { status: 415 })
  const parts = []
  let size = 0
  for await (const part of req) {
    size += part.length
    if (size > MAX_INPUT) throw Object.assign(new Error('Request exceeds 256 KiB'), { status: 413 })
    parts.push(part)
  }
  return JSON.parse(Buffer.concat(parts).toString('utf8'))
}
export function api({ workspace = process.env.ANGEL_WORKSPACE || process.cwd() } = {}) {
  const jobs = new Map()
  let active = 0
  return createServer(async (req, res) => {
    const json = (status, value) => {
      res.writeHead(status, {
        'Content-Type': 'application/json',
        'Cache-Control': 'no-store',
        'X-Content-Type-Options': 'nosniff',
      })
      res.end(JSON.stringify(value) + '\n')
    }
    try {
      if (req.headers.origin) {
        json(403, { error: 'Cross-origin requests are not supported' })
        return
      }
      // Reject DNS rebinding through browser-controlled hostnames.
      if (!/^(127\.0\.0\.1|localhost)(:\d+)?$/.test(req.headers.host ?? '')) {
        json(403, { error: 'Use a loopback Host header' })
        return
      }
      const url = new URL(req.url, 'http://127.0.0.1')
      if (req.method === 'GET' && url.pathname === '/health') {
        json(200, await doctor())
        return
      }
      if (req.method === 'GET' && url.pathname === '/v1/catalog') {
        json(200, catalog())
        return
      }
      if (req.method === 'POST' && url.pathname === '/v1/render') {
        if (active >= 2) {
          json(429, { error: 'Two renders are already running' })
          return
        }
        active++
        try {
          const result = await render(await body(req), { workspace }),
            id = randomUUID()
          jobs.set(
            id,
            result.files.filter((path) => !relative(result.out, path).startsWith('frames/')),
          )
          if (jobs.size > 128) jobs.delete(jobs.keys().next().value)
          json(201, {
            ...result,
            id,
            downloads: result.files
              .filter((path) => !relative(result.out, path).startsWith('frames/'))
              .map((path) => ({
                file: basename(path),
                url: `/v1/files/${id}/${encodeURIComponent(basename(path))}`,
              })),
          })
        } finally {
          active--
        }
        return
      }
      const match = url.pathname.match(/^\/v1\/files\/([a-f0-9-]+)\/([^/]+)$/)
      if (req.method === 'GET' && match) {
        const name = decodeURIComponent(match[2]),
          paths = jobs.get(match[1]) ?? [],
          path = paths.find((p) => basename(p) === name)
        if (!path) {
          json(404, { error: 'Unknown artifact' })
          return
        }
        const data = await readFile(path),
          ext = name.split('.').pop(),
          mime =
            {
              png: 'image/png',
              gif: 'image/gif',
              json: 'application/json',
              zip: 'application/zip',
              tres: 'text/plain',
            }[ext] ?? 'application/octet-stream'
        res.writeHead(200, {
          'Content-Type': mime,
          'Content-Length': data.length,
          'X-Content-Type-Options': 'nosniff',
          'Cache-Control': 'no-store',
        })
        res.end(data)
        return
      }
      json(404, { error: 'Unknown endpoint' })
    } catch (e) {
      if (!res.headersSent) json(e.status ?? 400, { error: e.message })
      else res.destroy()
    }
  })
}
const USAGE = `angelX VFX (Bend 2, local CPU)
  vfx catalog
  vfx doctor
  vfx render --prompt "holy sword slash" [--out vfx-out/slash] [--seed 7] [--scale 4] [--frames]
  vfx render effect.recipe.json [--out vfx-out/custom]
  vfx mcp                         stdio JSON-RPC; logs stay off stdout
  vfx serve [--port 8787] [--workspace /path/to/project]  binds 127.0.0.1
All paths default to ANGEL_WORKSPACE or the current directory. Output folders must be new.
`
export async function cli(argv = process.argv.slice(2)) {
  try {
    const command = argv.shift()
    if (!command || ['--help', '-h', 'help'].includes(command)) {
      process.stdout.write(USAGE)
      return
    }
    if (command === 'catalog') {
      if (argv.length) throw new Error('catalog takes no arguments')
      console.log(JSON.stringify(catalog(), null, 2))
      return
    }
    if (command === 'doctor') {
      const status = await doctor()
      console.log(JSON.stringify(status, null, 2))
      if (!status.ready) process.exitCode = 1
      return
    }
    if (command === 'mcp') {
      if (argv.length) throw new Error('mcp takes no arguments')
      await mcp()
      return
    }
    if (command === 'serve') {
      let port = 8787,
        workspace = process.env.ANGEL_WORKSPACE || process.cwd()
      for (let i = 0; i < argv.length; i++) {
        if (argv[i] === '--port') port = Number(argv[++i])
        else if (argv[i] === '--workspace') {
          if (!argv[i + 1]) throw new Error('--workspace requires a path')
          workspace = resolve(argv[++i])
        } else throw new Error(`Unknown argument: ${argv[i]}`)
      }
      if (!Number.isInteger(port) || port < 0 || port > 65535)
        throw new Error('port must be 0..65535')
      const server = api({ workspace })
      server.on('error', (e) => {
        process.stderr.write(`vfx: ${e.message}\n`)
        process.exitCode = 1
      })
      server.listen(port, '127.0.0.1', () =>
        process.stderr.write(`angelX VFX listening at http://127.0.0.1:${server.address().port}\n`),
      )
      return
    }
    if (command !== 'render') throw new Error(`Unknown command: ${command}`)
    const args = {}
    let recipePath
    for (let i = 0; i < argv.length; i++) {
      const a = argv[i]
      if (a === '--frames') {
        args.frames = true
        continue
      }
      if (['--prompt', '--out', '--seed', '--scale'].includes(a)) {
        if (argv[i + 1] === undefined) throw new Error(`${a} requires a value`)
        const v = argv[++i]
        args[a.slice(2)] = ['--seed', '--scale'].includes(a) ? Number(v) : v
      } else if (!a.startsWith('-') && !recipePath) recipePath = a
      else throw new Error(`Unknown argument: ${a}`)
    }
    if (recipePath) args.recipe = JSON.parse(await readFile(recipePath, 'utf8'))
    console.log(JSON.stringify(await render(args), null, 2))
  } catch (e) {
    process.stderr.write(`vfx: ${e.message}\n`)
    process.exitCode = 1
  }
}
