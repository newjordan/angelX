#!/usr/bin/env bash
# Fine-tuned Mage container with a smaller KV reservation (0.70 instead of 0.82 of the
# 3090), leaving ~3 GB for the bench's nested compositor. Needs root for docker here.
MAGE=${MAGE_ROOT:-$HOME/mage}  # the mage working folder: bench/, desk/, bin/, venv/
MODELS=${MAGE_MODELS:-$HOME/models}  # holds Mage-VL-Cowork and its fine-tunes
docker stop mage-vl-cowork 2>/dev/null
sleep 3
MODEL_DIR=$MODELS/Mage-VL-Cowork-hands-v1
exec docker run --rm --name mage-vl-cowork --gpus "device=0" --shm-size=8g \
  -p 127.0.0.1:30001:30001 -e PORT=30001 -e SERVED_MODEL_NAME=Mage-VL-Cowork -e SGLANG_MAX_PIXELS=4000000 \
  -e MEM_FRACTION_STATIC=0.70 \
  --mount "type=bind,src=${MODEL_DIR},dst=/models/Mage-VL-Cowork,readonly" \
  xiaoyizhang/mage-vl-cowork:0.1.0 > $MAGE/ref-serve-tuned.log 2>&1
