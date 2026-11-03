#!/usr/bin/env bash
# Swap Microsoft's Mage container to the fine-tuned weights (needs root for docker here).
MAGE=${MAGE_ROOT:-$HOME/mage}  # the mage working folder: bench/, desk/, bin/, venv/
MODELS=${MAGE_MODELS:-$HOME/models}  # holds Mage-VL-Cowork and its fine-tunes
docker stop mage-vl-cowork 2>/dev/null
sleep 3
export MODEL_DIR=$MODELS/Mage-VL-Cowork-hands-v1
exec bash $MODELS/Mage-VL-Cowork/osworld_eval/scripts/serve_sglang_docker.sh > $MAGE/ref-serve-tuned.log 2>&1
