// Read one bounded request from the native supervisor; never fetch or persist.
import { readFileSync, writeFileSync } from 'node:fs'
import { analyze, extract, extractBatch, fold } from '../../lib/science/brief.mjs'
const request = JSON.parse(readFileSync(0, 'utf8'))
const result = request.op === 'capabilities' ? {protocol:2, extract_batch:true, batch_source_identity:true, empty_fold:fold([])} : request.op === 'fold' ? fold(request.papers) : request.op === 'analyze' ? analyze(request) : request.op === 'extract' ? extract(request.source, request.body) : request.op === 'extract-batch' ? extractBatch(request.items) : (() => { throw new Error('unknown science operation') })()
writeFileSync(1, JSON.stringify(result))
