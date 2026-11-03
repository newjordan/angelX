// Request-scoped self-map projection; the host owns source reads and the gate.
import { readFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'
import { project } from '../../lib/self-map/report.mjs'

export { project }
if (!process.argv[1] || import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.stdout.write(JSON.stringify(project(JSON.parse(readFileSync(0, 'utf8')))))
}
