// One bounded host request per disposable process; no filesystem or network I/O.
import { readFileSync, writeSync } from 'node:fs'
import { dispatch } from '../../lib/repo-search/rules.mjs'
const request = JSON.parse(readFileSync(0, 'utf8'))
const result = Buffer.from(JSON.stringify(dispatch(request)))
for (let offset = 0; offset < result.length;) offset += writeSync(1, result, offset)
