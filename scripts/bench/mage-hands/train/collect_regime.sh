#!/usr/bin/env bash
# Explicitly invoked cloud read only. Never launches/retries a GPU or deploys.
# Historical pilot artifacts are unsupported by the repaired evidence checker.
set -euo pipefail
HERE=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
[[ $# == 2 ]] || { echo 'usage: collect_regime.sh NEW_RUN NEW_DESTINATION' >&2; exit 2; }
RUN=$1
DEST=$2
[[ $RUN =~ ^[A-Za-z0-9][A-Za-z0-9_-]{0,79}$ ]] || { echo 'invalid run name' >&2; exit 2; }
[[ -n $DEST && ! -e $DEST && ! -L $DEST ]] || { echo 'destination already exists or is empty; refusing before cloud access' >&2; exit 2; }
mkdir -p -- "$(dirname -- "$DEST")"
mkdir -- "$DEST"  # Atomic no-overwrite check, even if another collector races us.
modal_cli() { uv tool run --from modal modal "$@"; }
ROOT="/regime-recovery-v1/runs/$RUN"
modal_cli volume get mage-train "$ROOT/status.json" "$DEST/status.json"
STATE=$(python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1]))["state"])' "$DEST/status.json")
echo "observed remote state: $STATE"
if [[ $STATE != complete-not-deployed ]]; then
  echo 'Paired result not complete; no success or promotion claim. Partial destination retained.' >&2
  exit 2
fi
mkdir -- "$DEST/final" "$DEST/evaluation-data"
for FILE in smoke.json dataset-manifest.json eval-selection.json validation-selection.json summary.json training.jsonl \
  execution-configuration.json final-reload.json \
  eval-test-hands-v1.jsonl eval-validation-hands-v1.jsonl eval-test-candidate.jsonl eval-validation-candidate.jsonl \
  executed-modal_regime.py executed-regime.py executed-regime_batch.py executed-summarize_regime.py \
  evaluation-data/test.jsonl evaluation-data/validation.jsonl \
  final/adapter_config.json final/adapter_model.safetensors final/merger.safetensors; do
  modal_cli volume get mage-train "$ROOT/$FILE" "$DEST/$FILE"
done
python3 -I "$HERE/summarize_regime.py" "$DEST"
echo "Candidate retained at mage-train:$ROOT/final and $DEST/final. No server change or promotion."
