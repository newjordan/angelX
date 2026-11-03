#!/usr/bin/env bash
# Operator lock: files in locks/locked.sha256 are settled systems. Any byte
# change fails here until the operator approves the change and the manifest is
# re-sealed by hand (`scripts/check/check-locks.sh --seal`, operator only).
# An agent session must never run --seal: re-sealing without the operator's
# explicit approval naming the change defeats the lock and is forbidden.
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
manifest="$root/locks/locked.sha256"
cd "$root"

seal() {
  # The protected set. Keep in step with CLAUDE.md's locked list.
  local files=(
    cockpit/src/agent/harness/book/d3456_advisories.rs
    cockpit/src/drive/loop_ctl.rs
    cockpit/src/drive/loop_ctl/evidence.rs
  )
  mkdir -p locks
  {
    echo "# Operator-locked systems. Sealed by scripts/check/check-locks.sh --seal."
    echo "# A mismatch means a locked file changed without operator re-seal."
    sha256sum "${files[@]}"
  } > "$manifest.tmp"
  mv "$manifest.tmp" "$manifest"
  echo "check-locks: sealed ${#files[@]} file(s) into locks/locked.sha256"
}

case "${1:-}" in
  --seal) seal ;;
  "")
    [ -f "$manifest" ] || { echo "check-locks: $manifest missing" >&2; exit 1; }
    if ! sha256sum -c --quiet "$manifest" 2>/dev/null; then
      sha256sum -c "$manifest" 2>/dev/null | grep -v ': OK$' >&2 || true
      cat >&2 <<'EOF'
check-locks: OPERATOR LOCK — a locked file changed.
These systems are settled (see CLAUDE.md). Do not re-seal, do not edit around
this failure, and do not proceed: report the diff to the operator and stop.
Only the operator re-seals, after approving the exact change.
EOF
      exit 1
    fi
    echo "check-locks: locked files intact"
    ;;
  *) echo "usage: check-locks.sh [--seal]" >&2; exit 2 ;;
esac
