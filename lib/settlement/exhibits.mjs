// Pure worker-side manifest rules. Native deposition remains the authority for
// saved loop receipts, site ownership, capacity, immutable keys and admission.
export const MANIFEST_LIMIT = 16 * 1024
export const SOURCE_LIMIT = 64 * 1024
export const SCHEMA = 'angel.settlement-exhibits/v1'

const bytes = (s) => Buffer.byteLength(s, 'utf8')
const clean = (s, max) =>
  typeof s === 'string' && s.trim().length > 0 && bytes(s) <= max &&
  !/[\p{Cc}\p{Cs}]/u.test(s)

export function manifestPath(name) {
  if (typeof name !== 'string' || name.length < 1 || name.length > 64 ||
      !/^[A-Za-z0-9]/.test(name) || /[^A-Za-z0-9_-]/.test(name))
    throw new Error('Name must be 1–64 ASCII letters/digits/_/-, starting with a letter or digit')
  return `.angelX/settlement-exhibits/${name}.json`
}

function fields(value, keys) {
  if (!value || typeof value !== 'object' || Array.isArray(value) ||
      Object.keys(value).length !== keys.length ||
      keys.some((key) => !Object.hasOwn(value, key)))
    throw new Error('Manifest has missing or unknown fields')
}

export function validateManifest(value) {
  fields(value, ['schema', 'site_id', 'loop_id', 'iteration', 'artifacts'])
  if (value.schema !== SCHEMA || !clean(value.site_id, 256) || !clean(value.loop_id, 256) ||
      !Number.isSafeInteger(value.iteration) || value.iteration < 1 ||
      !Array.isArray(value.artifacts) || value.artifacts.length < 1 || value.artifacts.length > 16)
    throw new Error('Manifest needs site/loop IDs, a positive iteration and 1–16 artifacts')
  const keys = new Set()
  const artifacts = value.artifacts.map((item) => {
    fields(item, ['key', 'title', 'source', 'status'])
    if (typeof item.key !== 'string' || item.key.length < 1 || item.key.length > 64 ||
        /[^A-Za-z0-9_.-]/.test(item.key) || keys.has(item.key) ||
        !clean(item.title, 128) || !clean(item.source, 240) || item.source.includes('\\') ||
        item.source.split('/').some((part) => !part || part === '.' || part === '..') ||
        !['reported', 'inconclusive', 'failed'].includes(item.status))
      throw new Error('Invalid/duplicate artifact key, title, relative source or worker claim')
    keys.add(item.key)
    return { key: item.key, title: item.title, source: item.source, status: item.status }
  })
  return { schema: SCHEMA, site_id: value.site_id, loop_id: value.loop_id,
    iteration: value.iteration, artifacts }
}

export function encodeManifest(value) {
  const body = JSON.stringify(validateManifest(value), null, 2) + '\n'
  if (bytes(body) > MANIFEST_LIMIT) throw new Error('Manifest exceeds 16 KiB')
  return body
}
