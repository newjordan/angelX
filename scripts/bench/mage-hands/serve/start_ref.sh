#!/usr/bin/env bash
# Microsoft's reference SGLang container for Mage-VL-Cowork (needs root for docker here).
MAGE=${MAGE_ROOT:-$HOME/mage}  # the mage working folder: bench/, desk/, bin/, venv/
MODELS=${MAGE_MODELS:-$HOME/models}  # holds Mage-VL-Cowork and its fine-tunes
export MODEL_DIR=$MODELS/Mage-VL-Cowork
exec bash $MODELS/Mage-VL-Cowork/osworld_eval/scripts/serve_sglang_docker.sh > $MAGE/ref-serve.log 2>&1
