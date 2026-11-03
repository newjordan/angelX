// Presentation only: the native host owns discovery, liveness and identity.
exports.peerLine = (peer, now) => {
  const age = Math.max(0, now - peer.updated_ms)
  const state = age <= 90_000 ? 'live' : 'stale'
  const detail = peer.detail == null ? '' : ` — ${peer.detail}`
  return `  ${peer.host} pid ${peer.pid} · ${peer.activity} (${state}, ${Math.floor(age / 1000)}s ago) · cpu ${peer.cpu_pct.toFixed(0)}% mem ${peer.mem_pct.toFixed(0)}% gpu ${peer.gpu_pct.toFixed(0)}% · ${peer.workspace}${detail}`
}
