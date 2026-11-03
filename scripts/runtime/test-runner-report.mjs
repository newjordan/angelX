// Request-scoped runner-output interpretation. Process execution and exit status
// remain the native host's responsibility.
import { readFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'
import { project } from '../../lib/test-runner/report.mjs'

export { project }
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const request = JSON.parse(readFileSync(0, 'utf8'))
  process.stdout.write(JSON.stringify(project(request)))
}
