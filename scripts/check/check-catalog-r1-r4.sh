#!/usr/bin/env bash
# Focused catalog fixture gate; caller must supply a scrubbed fixture environment.
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
for key in HOME CODEX_HOME XDG_CONFIG_HOME XDG_DATA_HOME XDG_STATE_HOME XDG_CACHE_HOME TMPDIR CARGO_HOME RUSTUP_HOME; do
  [[ -n "${!key:-}" && "${!key}" == /* ]] || { printf 'Missing absolute fixture/cache path: %s\n' "$key" >&2; exit 2; }
done
[[ "${USER:-}" == fixture && "${CARGO_NET_OFFLINE:-}" == true ]] || { printf 'Run with fixture USER and offline Cargo.\n' >&2; exit 2; }
bash "$root/scripts/check/check-cockpit-fast.sh" catalog_r \
  --skip repeated_skill_catalog_reads --skip candidate_catalog_round_trip --skip learn_receipts_name \
  --test-threads=1
bash "$root/scripts/check/check-cockpit-fast.sh" agent::openai_codex::tests:: --test-threads=1
bash "$root/scripts/check/check-cockpit-fast.sh" agent::tools::vision::tests:: --test-threads=1
bash "$root/scripts/check/check-cockpit-fast.sh" agent::club::tests::openai_api --test-threads=1
