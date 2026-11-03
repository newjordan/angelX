#!/usr/bin/env bash
# Start the Mage-VL-Cowork shim as a background user unit; stop with: systemctl --user stop mage-serve
set -eu
MAGE=${MAGE_ROOT:-$HOME/mage}  # the mage working folder: bench/, desk/, bin/, venv/
MODELS=${MAGE_MODELS:-$HOME/models}  # holds Mage-VL-Cowork and its fine-tunes
exec systemd-run --user --unit=mage-serve --collect -p MemoryMax=32G \
  --working-directory=$MAGE \
  -E TMPDIR="${TMPDIR:-/tmp}" -E HF_HUB_OFFLINE=1 \
  -E PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True \
  $MAGE/venv/bin/python -I $MAGE/bin/serve_mage.py \
  --model $MODELS/Mage-VL-Cowork --port 30001
