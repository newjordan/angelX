// Source-map analysis and presentation, loaded by the request-scoped worker.
const MODS = ['pub(crate) ', 'pub(super) ', 'pub ', 'public ', 'private ', 'protected ', 'async ', 'unsafe ', 'default ', 'export ', 'abstract ', 'final ']
const KW = ['fn ', 'struct ', 'enum ', 'trait ', 'impl ', 'mod ', 'type ', 'const ', 'static ', 'def ', 'class ', 'function ', 'func ', 'interface ', 'package ', 'macro_rules!']
const trimStart = s => s.replace(/^\p{White_Space}+/u, '')
const trimEnd = s => s.replace(/\p{White_Space}+$/u, '')
const trim = s => trimEnd(trimStart(s))
const lines = s => {
  const a = s.split('\n')
  const endsWithNewline = a.at(-1) === ''
  if (endsWithNewline) a.pop()
  return a.map((s, i) => i < a.length - 1 || endsWithNewline ? s.replace(/\r$/, '') : s)
}
function outlineDecl(line) {
  let s = trimStart(line)
  for (;;) {
    const prefix = MODS.find(m => s.startsWith(m))
    if (!prefix) break
    s = trimStart(s.slice(prefix.length))
  }
  if (KW.some(k => s.startsWith(k))) return true
  while (s.startsWith('static ')) s = trimStart(s.slice(7))
  if (s.includes('(') && !trimEnd(s).endsWith(';')) {
    const head = trim(s.split('(', 1)[0])
    if (head && head.split(/\p{White_Space}+/u).length >= 2) return true
  }
  return false
}
function purpose(src) {
  const parts = []
  let started = false
  for (const line of lines(src).slice(0, 80)) {
    const t = trimStart(line)
    if (t.startsWith('//!')) {
      const rest = trim(t.slice(3))
      if (!rest) { if (started) break; continue }
      started = true
      parts.push(rest)
    } else if (started) break
  }
  let s = trim(parts.join(' '))
  const m = /\.(?: |$)/u.exec(s)
  if (m) s = trim(s.slice(0, m.index + 1))
  const chars = Array.from(s)
  return chars.length > 200 ? chars.slice(0, 200).join('') + '…' : s
}
function publicType(line) {
  let s = trimStart(line)
  const mod = ['pub(crate) ', 'pub(super) ', 'pub '].find(p => s.startsWith(p))
  if (!mod) return null
  s = trimStart(s.slice(mod.length))
  const keyword = ['struct ', 'enum ', 'trait '].find(k => s.startsWith(k))
  if (!keyword) return null
  const name = trimStart(s.slice(keyword.length)).match(/^[\p{Alphabetic}\p{Number}_]+/u)?.[0]
  return name || null
}
function analyze(src) {
  let count = 0
  const keyTypes = []
  for (const line of lines(src)) {
    if (!outlineDecl(line)) continue
    count++
    const name = publicType(line)
    if (name && keyTypes.length < 8 && !keyTypes.includes(name)) keyTypes.push(name)
  }
  return { purpose: purpose(src), count, keyTypes }
}
const groups = [
  ['Drivers & orchestration', 'swarm swarm_delegate deli overwatch club turn session bootstrap self_loop'],
  ['Agent harness & tooling', 'harness lsp mcp code_mode sandbox pty reinforce skills approval local_command'],
  ['Entry & TUI / display', 'main app app_control draw term input status_view transcript hud glyphs anim loader chart markdown viewer media agent_view turn_event_view approval_view artifacts_view agentviz terminal_art raytrace world_viz'],
  ['Memory & compaction', 'memory memory_store librarian compaction'],
  ['External agents & import', 'codex_import openai_codex agent_profile'],
].map(([label, names]) => [label, new Set(names.split(' '))])
function group(stem) { return groups.find(([, names]) => names.has(stem))?.[0] ?? 'Other' }
const compare = (a, b) => Buffer.compare(Buffer.from(a.stem), Buffer.from(b.stem))
function moduleLine(m) {
  return `- \`${m.rel}\` (${m.count} symbols) — ${m.purpose || '(no module doc — see outline)'}${m.keyTypes.length ? `  [${m.keyTypes.join(', ')}]` : ''}\n`
}
function render(top, tools) {
  top = top.map(m => ({ ...m, ...analyze(m.src) })).sort(compare)
  tools = tools.map(m => ({ ...m, ...analyze(m.src) })).sort(compare)
  let full = '', compact = ''
  for (const label of [...groups.map(([label]) => label), 'Other']) {
    const members = top.filter(m => group(m.stem) === label)
    if (!members.length) continue
    full += `\n### ${label}\n` + members.map(moduleLine).join('')
    compact += `- **${label}**: ` + members.map(m => {
      const words = trim(m.purpose).split(/\p{White_Space}+/u).filter(Boolean).slice(0, 8)
      const clause = words.join(' ').replace(/[,.;:]+$/u, '')
      return clause ? `${m.stem} (${clause})` : m.stem
    }).join('; ') + '\n'
  }
  return { full, compact, tools: tools.map(moduleLine).join(''), toolNames: tools.map(m => m.stem).join(', '), hasTop: top.length > 0, hasTools: tools.length > 0 }
}
function outline(rel, src) {
  const p = purpose(src)
  let out = `# ${rel}\n${p ? p + '\n' : ''}\nSymbols:\n`
  let n = 0
  for (const [i, line] of lines(src).entries()) {
    if (!outlineDecl(line)) continue
    out += `${i + 1}: ${trimEnd(line)}\n`
    if (++n >= 400) { out += '…[truncated at 400 symbols]\n'; break }
  }
  return out + (n ? '' : '(no top-level symbols)\n')
}
export function project(request) {
  let response
  switch (request.op) {
    case 'render': response = render(request.top, request.tools); break
    case 'outline': response = { outline: outline(request.rel, request.src) }; break
    case 'analyze': response = analyze(request.src); break
    case 'publicType': response = { name: publicType(request.line) }; break
    case 'group': response = { group: group(request.stem) }; break
    default: throw new Error('unknown self-map request')
  }
  return { version: 1, ...response }
}
