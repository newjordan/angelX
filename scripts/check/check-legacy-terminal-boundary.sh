#!/usr/bin/env bash
# Prove that quarantined products cannot leak back into active product code.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

legacy_archive='off-limits/legacy-terminal-harness'
browser_archive='off-limits/browser-product'
legacy_word="$(printf '\162\151\157')"
legacy_prefix="$(printf '\122\111\117')_"
failed=0

active_paths=(
  lib
  cockpit/src
  cockpit/Cargo.toml
  cockpit/README.md
  cockpit/docs
  harness
  scripts
  bin
  render-kit
  sidecar
  benchmarks/action-agent
  tests
  package.json
)

runtime_paths=(
  lib
  cockpit/src
  cockpit/Cargo.toml
  harness
  scripts
  bin
  render-kit
  sidecar
  benchmarks/action-agent
  tests
  package.json
)

active_present=()
for path in "${active_paths[@]}"; do
  if [[ -e "$path" || -L "$path" ]]; then
    active_present+=("$path")
  fi
done

runtime_present=()
for path in "${runtime_paths[@]}"; do
  if [[ -e "$path" || -L "$path" ]]; then
    runtime_present+=("$path")
  fi
done

link_roots=()
for path in lib cockpit harness bin render-kit scripts sidecar benchmarks/action-agent tests; do
  if [[ -d "$path" ]]; then
    link_roots+=("$path")
  fi
done

if (( ${#active_present[@]} == 0 || ${#runtime_present[@]} == 0 )); then
  echo "error: no active product paths were available to inspect" >&2
  exit 2
fi

boundary_scan() {
  local status
  if "$@"; then
    return 0
  else
    status=$?
  fi
  if (( status == 1 )); then
    return 1
  fi
  echo "error: boundary scan failed with status $status" >&2
  exit 2
}

# Use the extracted files themselves as the source of truth. This keeps the
# check useful in release archives that intentionally contain neither `.git`
# nor every internal source directory.
git_checkout=0
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  git_checkout=1
fi

if (( git_checkout )); then
  legacy_name_scan=(git grep -I -n -i -w "$legacy_word" -- "${active_present[@]}" ":!scripts/check/check-legacy-terminal-boundary.sh")
  legacy_prefix_scan=(git grep -I -n "$legacy_prefix" -- "${active_present[@]}" ":!scripts/check/check-legacy-terminal-boundary.sh")
else
  legacy_name_scan=(grep -rI -n -i -w --exclude=check-legacy-terminal-boundary.sh -- "$legacy_word" "${active_present[@]}")
  legacy_prefix_scan=(grep -rI -n --exclude=check-legacy-terminal-boundary.sh -- "$legacy_prefix" "${active_present[@]}")
fi

if boundary_scan "${legacy_name_scan[@]}"; then
  echo "error: legacy terminal name remains in active files" >&2
  failed=1
fi

if boundary_scan "${legacy_prefix_scan[@]}"; then
  echo "error: legacy terminal environment coupling remains active" >&2
  failed=1
fi

for archive in "$legacy_archive" "$browser_archive"; do
  if (( git_checkout )); then
    archive_scan=(git grep -I -n -F "$archive" -- "${runtime_present[@]}" ":!scripts/check/check-legacy-terminal-boundary.sh")
  else
    archive_scan=(grep -rI -n -F --exclude=check-legacy-terminal-boundary.sh -- "$archive" "${runtime_present[@]}")
  fi
  if boundary_scan "${archive_scan[@]}"; then
    echo "error: active runtime, test, or launcher references quarantine: $archive" >&2
    failed=1
  fi
done

if (( ${#link_roots[@]} > 0 )); then
  while IFS= read -r link; do
    target="$(readlink "$link")"
    if [[ "$target" == *"$legacy_archive"* || "$target" == *"$browser_archive"* ]]; then
      echo "error: active symlink reaches into quarantine: $link -> $target" >&2
      failed=1
    fi
  done < <(find "${link_roots[@]}" \
    \( -name .git -o -name node_modules -o -name .venv -o -name target \) -prune -o \
    -type l -print)
fi

if (( failed )); then
  exit 1
fi

echo "ok: active product is independent of quarantined products"
