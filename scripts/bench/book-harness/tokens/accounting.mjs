// Local text-token estimates. No provider cache or chat-template measurement.
export const SCOPE = {
  total:
    'Sum of separately encoded message text, function names/arguments, and JSON tool schemas; excludes chat framing and non-text payloads.',
  fresh:
    'Estimate under perfect caching of an exact serialized message prefix with unchanged tools; changed tools invalidate the whole represented request. Not observed provider cache usage.',
}

export function contentText(message) {
  if (!message || typeof message !== 'object' || Array.isArray(message))
    throw new TypeError('Invalid message')
  const c = message.content
  let text
  if (c == null) text = ''
  else if (typeof c === 'string') text = c
  else if (Array.isArray(c)) {
    text = c
      .map((p) => {
        if (
          !p ||
          typeof p !== 'object' ||
          Array.isArray(p) ||
          (p.text != null && typeof p.text !== 'string')
        )
          throw new TypeError('Invalid content part')
        return p.text ?? ''
      })
      .join('')
  } else throw new TypeError('Invalid message content')
  const calls = message.tool_calls ?? []
  if (!Array.isArray(calls)) throw new TypeError('Invalid tool calls')
  for (const call of calls) {
    if (!call || typeof call !== 'object' || Array.isArray(call))
      throw new TypeError('Invalid tool call')
    const fn = call.function ?? {}
    if (
      !fn ||
      typeof fn !== 'object' ||
      Array.isArray(fn) ||
      (fn.name != null && typeof fn.name !== 'string') ||
      (fn.arguments != null && typeof fn.arguments !== 'string')
    )
      throw new TypeError('Invalid function text')
    text += `\n${fn.name ?? ''} ${fn.arguments ?? ''}`
  }
  return text
}

export function parseRows(text) {
  if (!text.trim()) throw new TypeError('Empty request log')
  return text
    .trim()
    .split('\n')
    .map((line) => JSON.parse(line))
}

export function prepareRows(rows) {
  if (!Array.isArray(rows) || rows.length === 0) throw new TypeError('Empty request log')
  return rows.map((row) => {
    if (!row || !row.body || !Array.isArray(row.body.messages))
      throw new TypeError('Chat messages are required')
    const tools = row.body.tools ?? []
    if (!Array.isArray(tools)) throw new TypeError('Invalid tool schemas')
    return {
      messages: row.body.messages.map((message) => ({
        serialized: JSON.stringify(message),
        text: contentText(message),
        system: message.role === 'system',
      })),
      tools: JSON.stringify(tools),
    }
  })
}

export function profile(prepared, count) {
  let total = 0,
    fresh = 0,
    first = null,
    previous = null
  const coverage = {
    requests: prepared.length,
    prefixRewrites: 0,
    shortenedContexts: 0,
    toolSchemaChanges: 0,
  }
  const points = []
  for (const [i, row] of prepared.entries()) {
    const tokens = row.messages.map((message) => count(message.text))
    const tools = count(row.tools)
    const input = tools + tokens.reduce((a, b) => a + b, 0)
    let prefix = 0
    const changedTools = previous !== null && row.tools !== previous.tools
    if (previous !== null) {
      while (
        prefix < previous.messages.length &&
        prefix < row.messages.length &&
        previous.messages[prefix].serialized === row.messages[prefix].serialized
      )
        prefix++
      if (prefix < previous.messages.length) coverage.prefixRewrites++
      if (row.messages.length < previous.messages.length) coverage.shortenedContexts++
      if (changedTools) coverage.toolSchemaChanges++
    }
    total += input
    fresh +=
      previous === null || changedTools ? input : tokens.slice(prefix).reduce((a, b) => a + b, 0)
    if (i === 0)
      first = {
        input,
        tools,
        system: row.messages.reduce(
          (sum, message, index) => sum + (message.system ? tokens[index] : 0),
          0,
        ),
        messages: input - tools,
      }
    points.push({ request: i + 1, total, fresh, ctx: input })
    previous = row
  }
  return { requests: prepared.length, first, total, fresh, coverage, points }
}
