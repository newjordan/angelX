#!/usr/bin/env bash
# Give Docker the GPU (NVIDIA Container Toolkit + CDI spec), then start the
# Mage reference container. Run as root.
set -eu
MAGE=${MAGE_ROOT:-$HOME/mage}  # the mage working folder: bench/, desk/, bin/, venv/
pacman -S --needed --noconfirm nvidia-container-toolkit
nvidia-ctk cdi generate --output=/etc/cdi/nvidia.yaml
nvidia-ctk runtime configure --runtime=docker
if [ -z "$(docker ps -q)" ]; then
  systemctl restart docker
else
  echo "containers running; docker NOT restarted (CDI spec may still be enough)"
fi
setsid bash $MAGE/bin/start_ref.sh < /dev/null &
echo "reference container starting; log: $MAGE/ref-serve.log"
