const { peerLine } = require('./format.js')

module.exports = {
  apiVersion: 1,
  render({ peers, now_ms, directory, records }) {
    const lines = [`peer directory ${directory}`]
    lines.push(peers.length ? `${peers.length} peer instance(s):` : 'no peer angelX instances are publishing heartbeats here')
    for (const peer of peers) lines.push(peerLine(peer, now_ms))
    if (records.length) {
      lines.push('resurrections:')
      for (const record of records.slice(0, 5)) {
        const summary = Array.from(record.summary).slice(0, 120).join('')
        lines.push(`  ${record.dead_instance} on ${record.dead_host} · session ${record.old_session_id} -> ${record.new_session_id ?? 'none'} · ${record.success ? 'raised' : 'failed'} by guardian ${record.guardian_host} · ${summary}`)
      }
    }
    return lines.join('\n')
  },
}
