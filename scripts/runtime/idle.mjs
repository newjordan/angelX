// Tiny idle/heartbeat helpers shared by Habitsmith. Not the Reflex research loop.

export function isIdle({ angelTty, newestLedgerMs, now, idleMs }) {
  if (angelTty) return false
  if (newestLedgerMs == null) return true
  return now - newestLedgerMs >= idleMs
}

export function makeHeartbeat({ now, action, reason, budget, experiment = null }) {
  return {
    v: 1,
    ts: now,
    action,
    reason,
    budget,
    experiment,
  }
}

export function angelTtyRunning(execFileSync) {
  try {
    const out = execFileSync('pgrep', ['-x', 'angel'], { encoding: 'utf8' }).trim()
    if (!out) return false
    for (const pid of out.split('\n').filter(Boolean)) {
      try {
        const tty = execFileSync('ps', ['-o', 'tty=', '-p', pid], { encoding: 'utf8' }).trim()
        if (tty && tty !== '?') return true
      } catch {
        /* pid vanished */
      }
    }
    return false
  } catch {
    return false
  }
}

export function rollBudget(raw, today, cap) {
  const sameDay = raw && raw.date === today
  return { date: today, runs: sameDay && Number.isFinite(raw.runs) ? raw.runs : 0, cap }
}

export function budgetAllows(budget) {
  return budget.runs < budget.cap
}

export function recordRun(budget) {
  return { ...budget, runs: budget.runs + 1 }
}
